// Secret Discovery — локальный сканер сервисов, конфигов и credentials
// Запускается ТОЛЬКО по запросу администратора через relay.
// AI-агент НЕ может инициировать discovery самостоятельно.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::process::Command;

/// Scope определяет что именно сканировать
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryScope {
    /// Список директорий для сканирования (e.g. ["/etc", "/opt", "/home"])
    pub directories: Vec<String>,
    /// Типы файлов для поиска (e.g. ["env", "conf", "yml", "yaml", "json", "toml", "ini"])
    pub file_types: Vec<String>,
    /// Типы сервисов для обнаружения (e.g. ["postgres", "redis", "mysql", "prometheus"])
    pub service_types: Vec<String>,
    /// Маски путей которые НЕ сканировать
    pub exclude_paths: Vec<String>,
    /// Маски секретов которые НЕ собирать (e.g. ["*.pem", "*.key"] если не нужны)
    pub exclude_secrets: Vec<String>,
}

impl Default for DiscoveryScope {
    fn default() -> Self {
        Self {
            directories: vec![
                "/etc".into(),
                "/opt".into(),
                "/home".into(),
                "/var".into(),
                "/srv".into(),
                "/root".into(),
            ],
            file_types: vec![
                "env".into(),
                "conf".into(),
                "yml".into(),
                "yaml".into(),
                "json".into(),
                "toml".into(),
                "ini".into(),
                "cnf".into(),
                "cfg".into(),
            ],
            service_types: vec![
                "postgres".into(),
                "mysql".into(),
                "redis".into(),
                "mongodb".into(),
                "rabbitmq".into(),
                "prometheus".into(),
                "grafana".into(),
                "zabbix".into(),
                "docker".into(),
                "kubernetes".into(),
            ],
            exclude_paths: vec![
                "/proc".into(),
                "/sys".into(),
                "/dev".into(),
                "/run".into(),
                "*/node_modules/*".into(),
                "*/.git/*".into(),
            ],
            exclude_secrets: vec![],
        }
    }
}

/// Найденный сервис
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredService {
    pub service_type: String,
    pub name: String,
    pub version: Option<String>,
    pub config_paths: Vec<String>,
    pub listen_addresses: Vec<String>,
    pub status: ServiceStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ServiceStatus {
    Running,
    Stopped,
    Unknown,
}

/// Найденный секрет
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredSecret {
    pub source_path: String,
    pub service_type: String,
    pub key_type: SecretType,
    pub key_name: String,
    /// SHA-256 хеш значения (для дедупликации, не само значение!)
    pub value_hash: String,
    /// Метаданные: line number, surrounding context
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SecretType {
    /// Database connection string (DSN, DATABASE_URL, etc.)
    DatabaseUrl,
    /// Username/login
    Username,
    /// Password
    Password,
    /// API key/token
    ApiKey,
    /// TLS/SSL certificate or key
    Certificate,
    /// OAuth/JWT token
    OAuthToken,
    /// Generic secret (env var, config value)
    Generic,
}

/// Результат сканирования
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryResult {
    pub host: String,
    pub timestamp: String,
    pub scope: DiscoveryScope,
    pub services: Vec<DiscoveredService>,
    pub secrets: Vec<DiscoveredSecret>,
    pub scan_duration_ms: u64,
    pub errors: Vec<String>,
}

/// Паттерн для поиска секретов в конфигах
struct SecretPattern {
    key_type: SecretType,
    patterns: Vec<&'static str>,
}

fn secret_patterns() -> Vec<SecretPattern> {
    vec![
        SecretPattern {
            key_type: SecretType::DatabaseUrl,
            patterns: vec![
                "DATABASE_URL",
                "DB_HOST",
                "DB_PASSWORD",
                "MYSQL_HOST",
                "POSTGRES_URL",
                "REDIS_URL",
                "MONGODB_URI",
                "AMQP_URL",
                "CONNECTION_STRING",
                "DSN",
            ],
        },
        SecretPattern {
            key_type: SecretType::Password,
            patterns: vec![
                "PASSWORD",
                "PASSWD",
                "SECRET_KEY",
                "SECRET",
                "DB_PASS",
                "REDIS_PASSWORD",
                "MYSQL_PASSWORD",
                "PGPASSWORD",
                "SMTP_PASSWORD",
                "API_SECRET",
            ],
        },
        SecretPattern {
            key_type: SecretType::ApiKey,
            patterns: vec![
                "API_KEY",
                "API_TOKEN",
                "ACCESS_TOKEN",
                "AUTH_TOKEN",
                "BEARER_TOKEN",
                "X_API_KEY",
                "PRIVATE_KEY",
                "AWS_ACCESS_KEY_ID",
                "AWS_SECRET_ACCESS_KEY",
                "GRAFANA_API_KEY",
            ],
        },
        SecretPattern {
            key_type: SecretType::Username,
            patterns: vec![
                "DB_USER",
                "DB_USERNAME",
                "MYSQL_USER",
                "POSTGRES_USER",
                "REDIS_USER",
                "ADMIN_USER",
                "SMTP_USERNAME",
            ],
        },
        SecretPattern {
            key_type: SecretType::OAuthToken,
            patterns: vec![
                "OAUTH_TOKEN",
                "JWT_SECRET",
                "JWT_TOKEN",
                "REFRESH_TOKEN",
                "CLIENT_SECRET",
                "CLIENT_ID",
            ],
        },
        SecretPattern {
            key_type: SecretType::Certificate,
            patterns: vec![
                "SSL_CERT",
                "SSL_KEY",
                "TLS_CERT",
                "TLS_KEY",
                "CERTIFICATE",
                "CERT_PATH",
                "KEY_PATH",
            ],
        },
    ]
}

/// Основной сканер
pub struct DiscoveryScanner {
    scope: DiscoveryScope,
}

impl DiscoveryScanner {
    pub fn new(scope: DiscoveryScope) -> Self {
        Self { scope }
    }

    /// Запуск полного сканирования
    pub async fn scan(&self) -> Result<DiscoveryResult> {
        let start = std::time::Instant::now();
        let hostname = hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".into());

        let mut errors = Vec::new();
        let mut all_services = Vec::new();
        let mut all_secrets = Vec::new();

        // Step 1: Detect running services
        match self.detect_services().await {
            Ok(services) => all_services = services,
            Err(e) => errors.push(format!("Service detection error: {e}")),
        }

        // Step 2: Scan config files for secrets
        match self.scan_config_files().await {
            Ok(secrets) => all_secrets = secrets,
            Err(e) => errors.push(format!("Config scan error: {e}")),
        }

        // Step 3: Check environment of running processes (if permissions allow)
        match self.scan_process_environments().await {
            Ok(secrets) => all_secrets.extend(secrets),
            Err(e) => errors.push(format!("Process env scan error: {e}")),
        }

        // Step 4: Check Docker secrets
        match self.scan_docker_configs().await {
            Ok(secrets) => all_secrets.extend(secrets),
            Err(e) => errors.push(format!("Docker config scan error: {e}")),
        }

        // Deduplicate secrets by (source_path, key_name, value_hash)
        all_secrets = deduplicate_secrets(all_secrets);

        Ok(DiscoveryResult {
            host: hostname,
            timestamp: chrono::Utc::now().to_rfc3339(),
            scope: self.scope.clone(),
            services: all_services,
            secrets: all_secrets,
            scan_duration_ms: start.elapsed().as_millis() as u64,
            errors,
        })
    }

    /// Обнаружение запущенных сервисов через systemd, process list, listening ports
    async fn detect_services(&self) -> Result<Vec<DiscoveredService>> {
        let mut services = Vec::new();

        // Check systemd services
        if let Ok(output) = Command::new("systemctl")
            .args(["list-units", "--type=service", "--state=running", "--no-pager", "--no-legend"])
            .output()
            .await
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.is_empty() { continue; }
                let unit_name = parts[0];

                if let Some(svc) = identify_service(unit_name) {
                    if self.scope.service_types.contains(&svc.service_type)
                        || self.scope.service_types.is_empty()
                    {
                        let version = detect_service_version(unit_name).await;
                        let config_paths = find_service_configs(&svc.service_type).await;
                        let listen_addresses = get_service_ports(unit_name).await;

                        services.push(DiscoveredService {
                            service_type: svc.service_type,
                            name: unit_name.to_string(),
                            version,
                            config_paths,
                            listen_addresses,
                            status: ServiceStatus::Running,
                        });
                    }
                }
            }
        }

        // Check Docker containers
        if let Ok(output) = Command::new("docker")
            .args(["ps", "--format", "{{.Names}}\t{{.Image}}\t{{.Ports}}"])
            .output()
            .await
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() < 2 { continue; }
                let name = parts[0];
                let image = parts[1];
                let ports = parts.get(2).unwrap_or(&"").to_string();

                if let Some(svc) = identify_docker_service(image) {
                    let listen_addresses: Vec<String> = ports
                        .split(',')
                        .map(|p| p.trim().to_string())
                        .filter(|p| !p.is_empty())
                        .collect();

                    services.push(DiscoveredService {
                        service_type: svc,
                        name: name.to_string(),
                        version: Some(image.to_string()),
                        config_paths: vec![],
                        listen_addresses,
                        status: ServiceStatus::Running,
                    });
                }
            }
        }

        // Check listening ports via ss
        if let Ok(output) = Command::new("ss")
            .args(["-tlnp", "--no-header"])
            .output()
            .await
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 4 { continue; }
                let local_addr = parts[3];
                let process = parts.get(5).unwrap_or(&"").to_string();

                // Try to identify service by port
                if let Some(svc) = identify_service_by_address(local_addr) {
                    // Don't add duplicates
                    if !services.iter().any(|s| s.listen_addresses.contains(&local_addr.to_string())) {
                        services.push(DiscoveredService {
                            service_type: svc.service_type,
                            name: process,
                            version: None,
                            config_paths: vec![],
                            listen_addresses: vec![local_addr.to_string()],
                            status: ServiceStatus::Running,
                        });
                    }
                }
            }
        }

        Ok(services)
    }

    /// Сканирование конфигурационных файлов
    async fn scan_config_files(&self) -> Result<Vec<DiscoveredSecret>> {
        let mut secrets = Vec::new();
        let patterns = secret_patterns();

        for dir in &self.scope.directories {
            if !Path::new(dir).exists() { continue; }

            let files = find_config_files(dir, &self.scope.file_types, &self.scope.exclude_paths).await?;
            for file_path in files {
                if self.is_excluded(&file_path) { continue; }

                match fs::read_to_string(&file_path).await {
                    Ok(content) => {
                        let found = parse_secrets_from_content(
                            &file_path,
                            &content,
                            &patterns,
                            &self.scope.exclude_secrets,
                        );
                        secrets.extend(found);
                    }
                    Err(_) => continue, // Permission denied, skip silently
                }
            }
        }

        Ok(secrets)
    }

    /// Сканирование /proc/*/environ для запущенных процессов
    async fn scan_process_environments(&self) -> Result<Vec<DiscoveredSecret>> {
        let mut secrets = Vec::new();
        let patterns = secret_patterns();

        let proc_dir = fs::read_dir("/proc").await;
        if let Ok(mut entries) = proc_dir {
            while let Some(entry) = entries.next_entry().await? {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                // Only numeric directories (PIDs)
                if !name_str.chars().all(|c| c.is_ascii_digit()) { continue; }

                let environ_path = PathBuf::from(format!("/proc/{}/environ", name_str));
                match fs::read_to_string(&environ_path).await {
                    Ok(content) => {
                        // environ uses null bytes as separators
                        for pair in content.split('\0') {
                            if let Some((key, value)) = pair.split_once('=') {
                                for pattern in &patterns {
                                    for pat in &pattern.patterns {
                                        if key.to_uppercase().contains(pat) && !value.is_empty() {
                                            let hash = sha256_hash(value.as_bytes());
                                            secrets.push(DiscoveredSecret {
                                                source_path: format!("/proc/{}/environ", name_str),
                                                service_type: "process".into(),
                                                key_type: pattern.key_type.clone(),
                                                key_name: key.to_string(),
                                                value_hash: hash,
                                                metadata: {
                                                    let mut m = HashMap::new();
                                                    m.insert("pid".into(), name_str.to_string());
                                                    m
                                                },
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => continue,
                }

                // Limit to first 200 processes to avoid excessive scanning
                if secrets.len() > 500 { break; }
            }
        }

        Ok(secrets)
    }

    /// Сканирование Docker конфигураций
    async fn scan_docker_configs(&self) -> Result<Vec<DiscoveredSecret>> {
        let mut secrets = Vec::new();
        let patterns = secret_patterns();

        // Check ~/.docker/config.json
        let docker_config = PathBuf::from("/root/.docker/config.json");
        if docker_config.exists() {
            if let Ok(content) = fs::read_to_string(&docker_config).await {
                let found = parse_secrets_from_content(
                    &docker_config.to_string_lossy(),
                    &content,
                    &patterns,
                    &self.scope.exclude_secrets,
                );
                secrets.extend(found);
            }
        }

        // Check docker-compose files in common locations
        let compose_paths = [
            "/opt/docker-compose.yml",
            "/opt/docker-compose.yaml",
            "/srv/docker-compose.yml",
            "/srv/docker-compose.yaml",
            "/root/docker-compose.yml",
            "/root/docker-compose.yaml",
        ];

        for path in &compose_paths {
            let p = Path::new(path);
            if p.exists() {
                if let Ok(content) = fs::read_to_string(p).await {
                    let found = parse_secrets_from_content(
                        path,
                        &content,
                        &patterns,
                        &self.scope.exclude_secrets,
                    );
                    secrets.extend(found);
                }
            }
        }

        // Inspect running containers for env vars
        if let Ok(output) = Command::new("docker")
            .args(["ps", "--format", "{{.Names}}"])
            .output()
            .await
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for container_name in stdout.lines() {
                let container_name = container_name.trim();
                if container_name.is_empty() { continue; }

                if let Ok(env_output) = Command::new("docker")
                    .args(["exec", container_name, "env"])
                    .output()
                    .await
                {
                    let env_str = String::from_utf8_lossy(&env_output.stdout);
                    for line in env_str.lines() {
                        if let Some((key, value)) = line.split_once('=') {
                            for pattern in &patterns {
                                for pat in &pattern.patterns {
                                    if key.to_uppercase().contains(pat) && !value.is_empty() {
                                        let hash = sha256_hash(value.as_bytes());
                                        secrets.push(DiscoveredSecret {
                                            source_path: format!("docker:{container_name}"),
                                            service_type: "docker".into(),
                                            key_type: pattern.key_type.clone(),
                                            key_name: key.to_string(),
                                            value_hash: hash,
                                            metadata: {
                                                let mut m = HashMap::new();
                                                m.insert("container".into(), container_name.to_string());
                                                m
                                            },
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(secrets)
    }

    fn is_excluded(&self, path: &str) -> bool {
        self.scope.exclude_paths.iter().any(|mask| {
            path.contains(&mask.replace("*/", "").replace("/*", ""))
        })
    }
}

/// Парсинг секретов из содержимого файла
fn parse_secrets_from_content(
    path: &str,
    content: &str,
    patterns: &[SecretPattern],
    exclude: &[String],
) -> Vec<DiscoveredSecret> {
    let mut secrets = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let line = line.trim();

        // Skip comments
        if line.starts_with('#') || line.starts_with("//") || line.starts_with(';') {
            continue;
        }

        // Try KEY=VALUE format
        if let Some((key, value)) = parse_key_value(line) {
            if value.is_empty() || value.len() < 2 { continue; }
            if exclude.iter().any(|e| key.contains(e)) { continue; }

            for pattern in patterns {
                for pat in &pattern.patterns {
                    if key.to_uppercase().contains(pat) {
                        let hash = sha256_hash(value.as_bytes());
                        secrets.push(DiscoveredSecret {
                            source_path: path.to_string(),
                            service_type: identify_source_service(path),
                            key_type: pattern.key_type.clone(),
                            key_name: key.to_string(),
                            value_hash: hash,
                            metadata: {
                                let mut m = HashMap::new();
                                m.insert("line".into(), (line_num + 1).to_string());
                                m
                            },
                        });
                        break;
                    }
                }
            }
        }

        // Try YAML/TOML key: value format
        if let Some((key, value)) = parse_yaml_line(line) {
            if value.is_empty() || value.len() < 2 { continue; }

            for pattern in patterns {
                for pat in &pattern.patterns {
                    if key.to_uppercase().contains(pat) {
                        let hash = sha256_hash(value.as_bytes());
                        secrets.push(DiscoveredSecret {
                            source_path: path.to_string(),
                            service_type: identify_source_service(path),
                            key_type: pattern.key_type.clone(),
                            key_name: key.to_string(),
                            value_hash: hash,
                            metadata: {
                                let mut m = HashMap::new();
                                m.insert("line".into(), (line_num + 1).to_string());
                                m.insert("format".into(), "yaml".into());
                                m
                            },
                        });
                        break;
                    }
                }
            }
        }

        // Try JSON key-value (simple, for inline JSON)
        if line.contains('"') && line.contains(':') {
            if let Some((key, value)) = parse_json_line(line) {
                if value.is_empty() || value.len() < 2 { continue; }

                for pattern in patterns {
                    for pat in &pattern.patterns {
                        if key.to_uppercase().contains(pat) {
                            let hash = sha256_hash(value.as_bytes());
                            secrets.push(DiscoveredSecret {
                                source_path: path.to_string(),
                                service_type: identify_source_service(path),
                                key_type: pattern.key_type.clone(),
                                key_name: key.to_string(),
                                value_hash: hash,
                                metadata: {
                                    let mut m = HashMap::new();
                                    m.insert("line".into(), (line_num + 1).to_string());
                                    m.insert("format".into(), "json".into());
                                    m
                                },
                            });
                            break;
                        }
                    }
                }
            }
        }
    }

    secrets
}

/// Parse KEY=VALUE (env file style)
fn parse_key_value(line: &str) -> Option<(String, String)> {
    let line = line.trim_start();
    if line.contains('=') && !line.starts_with('=') {
        let (key, value) = line.split_once('=')?;
        let key = key.trim().to_string();
        let value = value.trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();
        if key.is_empty() || value.is_empty() { return None; }
        Some((key, value))
    } else {
        None
    }
}

/// Parse YAML key: value
fn parse_yaml_line(line: &str) -> Option<(String, String)> {
    if !line.contains(':') { return None; }
    let (key, value) = line.split_once(':')?;
    let key = key.trim().to_string();
    let value = value.trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_string();
    if key.is_empty() || value.is_empty() || value == "null" || value == "~" {
        return None;
    }
    // Skip keys that look like YAML structure (contain spaces or are just "-")
    if key.contains(' ') || key == "-" { return None; }
    Some((key, value))
}

/// Parse simple JSON "key": "value" on a single line
fn parse_json_line(line: &str) -> Option<(String, String)> {
    // Match "key": "value" or "key": value patterns
    let re = regex::Regex::new(r#""([^"]+)"\s*:\s*"?([^",}\s]+)"?"#).ok()?;
    if let Some(caps) = re.captures(line) {
        let key: String = caps.get(1)?.as_str().to_string();
        let value: String = caps.get(2)?.as_str().trim_matches('"').to_string();
        if key.is_empty() || value.is_empty() { return None; }
        Some((key, value))
    } else {
        None
    }
}

/// Find config files matching extensions in a directory tree
async fn find_config_files(
    dir: &str,
    extensions: &[String],
    _exclude: &[String],
) -> Result<Vec<String>> {
    let mut results = Vec::new();

    // Use find command for efficiency
    let ext_args: Vec<String> = extensions
        .iter()
        .flat_map(|ext| vec!["-name".into(), format!("*.{ext}")])
        .collect();

    let mut cmd = Command::new("find");
    cmd.arg(dir)
        .args(["-maxdepth", "4", "-type", "f", "(", ])
        .args(&ext_args)
        .arg(")");

    if let Ok(output) = cmd.output().await {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            results.push(line.to_string());
        }
    }

    // Also find dotfiles like .env
    if extensions.contains(&"env".to_string()) {
        if let Ok(output) = Command::new("find")
            .args([dir, "-maxdepth", "4", "-name", ".env*", "-type", "f"])
            .output()
            .await
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if !results.contains(&line.to_string()) {
                    results.push(line.to_string());
                }
            }
        }
    }

    Ok(results)
}

/// Identify service type from systemd unit name
fn identify_service(unit_name: &str) -> Option<ServiceIdentity> {
    let lower = unit_name.to_lowercase();

    let map: &[(&str, &str)] = &[
        ("postgres", "postgres"),
        ("pgbouncer", "postgres"),
        ("mysql", "mysql"),
        ("mariadb", "mysql"),
        ("redis", "redis"),
        ("mongodb", "mongodb"),
        ("mongod", "mongodb"),
        ("rabbitmq", "rabbitmq"),
        ("prometheus", "prometheus"),
        ("grafana", "grafana"),
        ("zabbix", "zabbix"),
        ("nginx", "nginx"),
        ("apache", "apache"),
        ("httpd", "apache"),
        ("docker", "docker"),
        ("kubelet", "kubernetes"),
        ("vault", "vault"),
        ("consul", "consul"),
        ("etcd", "etcd"),
        ("elasticsearch", "elasticsearch"),
        ("kibana", "kibana"),
        ("logstash", "logstash"),
        ("kafka", "kafka"),
        ("zookeeper", "zookeeper"),
    ];

    for (pattern, svc_type) in map {
        if lower.contains(pattern) {
            return Some(ServiceIdentity {
                service_type: (*svc_type).to_string(),
            });
        }
    }

    None
}

struct ServiceIdentity {
    service_type: String,
}

/// Identify service from Docker image name
fn identify_docker_service(image: &str) -> Option<String> {
    let lower = image.to_lowercase();

    let map: &[(&str, &str)] = &[
        ("postgres", "postgres"),
        ("mysql", "mysql"),
        ("redis", "redis"),
        ("mongo", "mongodb"),
        ("rabbitmq", "rabbitmq"),
        ("prom", "prometheus"),
        ("grafana", "grafana"),
        ("zabbix", "zabbix"),
        ("nginx", "nginx"),
        ("vault", "vault"),
        ("consul", "consul"),
        ("etcd", "etcd"),
        ("elastic", "elasticsearch"),
        ("kibana", "kibana"),
        ("logstash", "logstash"),
        ("kafka", "kafka"),
        ("mariadb", "mysql"),
        ("postal", "postal"),
    ];

    for (pattern, svc_type) in map {
        if lower.contains(pattern) {
            return Some((*svc_type).to_string());
        }
    }

    None
}

/// Identify service by listening address/port
fn identify_service_by_address(addr: &str) -> Option<ServiceIdentity> {
    let port_map: &[(u16, &str)] = &[
        (5432, "postgres"),
        (3306, "mysql"),
        (6379, "redis"),
        (27017, "mongodb"),
        (5672, "rabbitmq"),
        (9090, "prometheus"),
        (3000, "grafana"),
        (8200, "vault"),
        (8500, "consul"),
        (2379, "etcd"),
        (9200, "elasticsearch"),
        (5601, "kibana"),
        (9092, "kafka"),
        (2181, "zookeeper"),
        (25, "smtp"),
        (80, "http"),
        (443, "https"),
    ];

    // Extract port from address like "127.0.0.1:5432" or "[::]:5432"
    let port = addr.rsplit(':').next()
        .and_then(|p| p.parse::<u16>().ok())?;

    for (p, svc) in port_map {
        if port == *p {
            return Some(ServiceIdentity {
                service_type: (*svc).to_string(),
            });
        }
    }

    None
}

/// Detect service version
async fn detect_service_version(unit_name: &str) -> Option<String> {
    let binary = unit_name.trim_end_matches(".service");
    if let Ok(output) = Command::new(binary)
        .arg("--version")
        .output()
        .await
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let first_line = stdout.lines().next()?;
        // Take first 100 chars of version output
        Some(first_line.chars().take(100).collect())
    } else {
        None
    }
}

/// Find config file paths for a service type
async fn find_service_configs(service_type: &str) -> Vec<String> {
    let candidates: Vec<&str> = match service_type {
        "postgres" => vec!["/etc/postgresql", "/var/lib/postgresql"],
        "mysql" => vec!["/etc/mysql/my.cnf", "/etc/mysql/conf.d/", "/etc/my.cnf"],
        "redis" => vec!["/etc/redis/redis.conf", "/etc/redis.conf"],
        "mongodb" => vec!["/etc/mongod.conf", "/etc/mongodb.conf"],
        "rabbitmq" => vec!["/etc/rabbitmq/rabbitmq.conf", "/etc/rabbitmq/enabled_plugins"],
        "prometheus" => vec!["/etc/prometheus/prometheus.yml", "/etc/prometheus/alertmanager.yml"],
        "grafana" => vec!["/etc/grafana/grafana.ini", "/etc/grafana/provisioning/"],
        "nginx" => vec!["/etc/nginx/nginx.conf", "/etc/nginx/sites-enabled/"],
        _ => return vec![],
    };

    let mut found: Vec<String> = Vec::new();
    for p in &candidates {
        if p.ends_with('/') {
            if let Ok(mut entries) = fs::read_dir(*p).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    found.push(entry.path().to_string_lossy().to_string());
                }
            }
        } else if Path::new(*p).exists() {
            found.push((*p).to_string());
        }
    }
    found
}

/// Get listening ports for a systemd service
async fn get_service_ports(unit_name: &str) -> Vec<String> {
    // Get main PID from systemctl
    let output = Command::new("systemctl")
        .args(["show", unit_name, "--property=MainPID", "--value"])
        .output()
        .await;

    if let Ok(out) = output {
        let pid = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if pid != "0" && !pid.is_empty() {
            // Use ss to find ports for this PID
            if let Ok(ss_out) = Command::new("ss")
                .args(["-tlnp", "--no-header"])
                .output()
                .await
            {
                let stdout = String::from_utf8_lossy(&ss_out.stdout);
                return stdout.lines()
                    .filter(|l| l.contains(&format!("pid={pid}")))
                    .filter_map(|l| l.split_whitespace().nth(3).map(|s| s.to_string()))
                    .collect();
            }
        }
    }
    vec![]
}

/// Identify source service from file path
fn identify_source_service(path: &str) -> String {
    let lower = path.to_lowercase();
    let map: &[(&str, &str)] = &[
        ("postgres", "postgres"),
        ("pgbouncer", "postgres"),
        ("mysql", "mysql"),
        ("redis", "redis"),
        ("mongo", "mongodb"),
        ("rabbitmq", "rabbitmq"),
        ("prometheus", "prometheus"),
        ("grafana", "grafana"),
        ("zabbix", "zabbix"),
        ("nginx", "nginx"),
        ("docker", "docker"),
        ("kube", "kubernetes"),
        ("vault", "vault"),
        ("postal", "postal"),
    ];
    for (pattern, svc) in map {
        if lower.contains(pattern) {
            return (*svc).to_string();
        }
    }
    "unknown".to_string()
}

/// SHA-256 hash (for deduplication — NEVER stores actual secret value)
fn sha256_hash(data: &[u8]) -> String {
    use std::fmt::Write;
    let hash = <sha2::Sha256 as sha2::Digest>::digest(data);
    let mut hex = String::with_capacity(64);
    for byte in hash {
        write!(&mut hex, "{byte:02x}").unwrap();
    }
    hex
}

/// Deduplicate secrets by (source_path, key_name, value_hash)
fn deduplicate_secrets(secrets: Vec<DiscoveredSecret>) -> Vec<DiscoveredSecret> {
    let mut seen = std::collections::HashSet::new();
    secrets.into_iter().filter(|s| {
        let key = format!("{}:{}:{}", s.source_path, s.key_name, s.value_hash);
        seen.insert(key)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_value() {
        assert_eq!(parse_key_value("DB_PASSWORD=secret123"), Some(("DB_PASSWORD".into(), "secret123".into())));
        assert_eq!(parse_key_value("DB_HOST=\"localhost\""), Some(("DB_HOST".into(), "localhost".into())));
        assert_eq!(parse_key_value("EMPTY="), None);
        assert_eq!(parse_key_value("=value"), None);
    }

    #[test]
    fn test_parse_yaml_line() {
        assert_eq!(parse_yaml_line("password: mysecret"), Some(("password".into(), "mysecret".into())));
        assert_eq!(parse_yaml_line("api_key: \"abc123\""), Some(("api_key".into(), "abc123".into())));
        assert_eq!(parse_yaml_line("key: null"), None);
        assert_eq!(parse_yaml_line("- item"), None);
    }

    #[test]
    fn test_identify_service() {
        assert_eq!(identify_service("postgresql.service").unwrap().service_type, "postgres");
        assert_eq!(identify_service("redis-server.service").unwrap().service_type, "redis");
        assert_eq!(identify_service("nginx.service").unwrap().service_type, "nginx");
        assert!(identify_service("random-app.service").is_none());
    }

    #[test]
    fn test_identify_docker_service() {
        assert_eq!(identify_docker_service("postgres:15"), Some("postgres".into()));
        assert_eq!(identify_docker_service("redis:7-alpine"), Some("redis".into()));
        assert!(identify_docker_service("my-custom-app:v1").is_none());
    }

    #[test]
    fn test_identify_service_by_address() {
        assert_eq!(identify_service_by_address("127.0.0.1:5432").unwrap().service_type, "postgres");
        assert_eq!(identify_service_by_address("0.0.0.0:6379").unwrap().service_type, "redis");
        assert!(identify_service_by_address("0.0.0.0:12345").is_none());
    }

    #[test]
    fn test_parse_secrets_from_env_file() {
        let content = r#"
DB_HOST=localhost
DB_PASSWORD=super_secret_123
REDIS_URL=redis://default:password@localhost:6379
APP_NAME=myapp
# Comment line
API_KEY=sk-12345abcdef
NORMAL_VAR=some_value
"#;
        let patterns = secret_patterns();
        let secrets = parse_secrets_from_content("/app/.env", content, &patterns, &[]);

        assert!(secrets.iter().any(|s| s.key_name == "DB_PASSWORD" && s.key_type == SecretType::Password));
        assert!(secrets.iter().any(|s| s.key_name == "API_KEY" && s.key_type == SecretType::ApiKey));
        assert!(secrets.iter().any(|s| s.key_name == "REDIS_URL" && s.key_type == SecretType::DatabaseUrl));
        assert!(!secrets.iter().any(|s| s.key_name == "APP_NAME"));
        assert!(!secrets.iter().any(|s| s.key_name == "NORMAL_VAR"));
    }

    #[test]
    fn test_parse_secrets_from_yaml() {
        let content = r#"
database:
  host: localhost
  password: pg_secret_pass
  user: admin
redis:
  password: redis_pass_123
logging:
  level: info
"#;
        let patterns = secret_patterns();
        let secrets = parse_secrets_from_content("/etc/app/config.yml", content, &patterns, &[]);

        assert!(secrets.iter().any(|s| s.key_name == "password" && s.source_path.contains("config.yml")));
        assert!(secrets.len() >= 2); // At least 2 password entries
    }

    #[test]
    fn test_deduplication() {
        let secrets = vec![
            DiscoveredSecret {
                source_path: "/app/.env".into(),
                service_type: "app".into(),
                key_type: SecretType::Password,
                key_name: "DB_PASS".into(),
                value_hash: "abc123".into(),
                metadata: HashMap::new(),
            },
            DiscoveredSecret {
                source_path: "/app/.env".into(),
                service_type: "app".into(),
                key_type: SecretType::Password,
                key_name: "DB_PASS".into(),
                value_hash: "abc123".into(),
                metadata: HashMap::new(),
            },
        ];
        let deduped = deduplicate_secrets(secrets);
        assert_eq!(deduped.len(), 1);
    }

    #[test]
    fn test_sha256_hash() {
        let h1 = sha256_hash(b"secret");
        let h2 = sha256_hash(b"secret");
        let h3 = sha256_hash(b"other");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn test_discovery_scope_default() {
        let scope = DiscoveryScope::default();
        assert!(scope.directories.contains(&"/etc".to_string()));
        assert!(scope.file_types.contains(&"env".to_string()));
        assert!(scope.service_types.contains(&"postgres".to_string()));
        assert!(scope.exclude_paths.contains(&"/proc".to_string()));
    }

    #[test]
    fn test_identify_source_service() {
        assert_eq!(identify_source_service("/etc/postgresql/15/main/postgresql.conf"), "postgres");
        assert_eq!(identify_source_service("/opt/redis/redis.conf"), "redis");
        assert_eq!(identify_source_service("/home/app/.env"), "unknown");
    }
}
