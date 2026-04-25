// Semantic Infrastructure Map
// ==========================
// Knowledge graph of infrastructure: hosts, services, databases, queues,
// endpoints, secrets, environments. Agent queries this to understand
// "where is service X, what DB does it use, what secrets does it need".

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Node types in the infrastructure graph
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum MapNode {
    #[serde(rename = "host")]
    Host {
        id: String,
        hostname: String,
        ip: Option<String>,
        os: Option<String>,
        labels: HashMap<String, String>,
    },
    #[serde(rename = "service")]
    Service {
        id: String,
        name: String,
        service_type: String,  // postgres, redis, nginx, custom-app...
        version: Option<String>,
        environment: Option<String>,  // prod, stage, dev
        criticality: Option<Criticality>,
        owner: Option<String>,
        labels: HashMap<String, String>,
    },
    #[serde(rename = "database")]
    Database {
        id: String,
        name: String,
        db_type: String,  // postgres, mysql, redis, mongodb...
        host: Option<String>,
        port: Option<u16>,
        environment: Option<String>,
        labels: HashMap<String, String>,
    },
    #[serde(rename = "queue")]
    Queue {
        id: String,
        name: String,
        queue_type: String,  // kafka, rabbitmq, nats, sqs...
        host: Option<String>,
        port: Option<u16>,
        labels: HashMap<String, String>,
    },
    #[serde(rename = "endpoint")]
    Endpoint {
        id: String,
        url: String,
        protocol: String,  // http, https, grpc, tcp
        service_name: Option<String>,
        health_check_url: Option<String>,
        labels: HashMap<String, String>,
    },
    #[serde(rename = "secret_ref")]
    SecretRef {
        id: String,
        key_name: String,
        secret_type: String,  // password, api_key, certificate, dsn...
        vault_path: Option<String>,
        labels: HashMap<String, String>,
    },
    #[serde(rename = "monitor")]
    Monitor {
        id: String,
        name: String,
        monitor_type: String,  // prometheus, grafana, zabbix, datadog...
        url: Option<String>,
        labels: HashMap<String, String>,
    },
    #[serde(rename = "environment")]
    Environment {
        id: String,
        name: String,  // prod, staging, dev
        region: Option<String>,
        labels: HashMap<String, String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Criticality {
    Low,
    Medium,
    High,
    Critical,
}

/// Edge types — relationships between nodes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapEdge {
    pub id: String,
    pub from_id: String,
    pub to_id: String,
    pub rel_type: RelationType,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationType {
    HostsService,
    ServiceUsesDb,
    ServiceUsesQueue,
    ServiceExposesApi,
    ServiceMonitoredBy,
    ServiceHasSecret,
    ServiceInEnv,
    DependsOn,
    ConnectedTo,
}

/// Snapshot of infrastructure from one agent scan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfrastructureSnapshot {
    pub agent_id: String,
    pub host_id: String,
    pub timestamp: String,
    pub nodes: Vec<MapNode>,
    pub edges: Vec<MapEdge>,
    pub version: u64,
}

/// Query to the semantic map API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "query")]
pub enum MapQuery {
    /// Find service by name or description (fuzzy/semantic search)
    #[serde(rename = "find_service")]
    FindService {
        name: String,
        environment: Option<String>,
    },
    /// Get full topology for a service (dependencies, host, secrets, monitoring)
    #[serde(rename = "service_topology")]
    ServiceTopology {
        service_id: String,
        depth: Option<u32>,  // how many hops, default 2
    },
    /// List all services on a host
    #[serde(rename = "host_services")]
    HostServices {
        host_id: String,
    },
    /// List all services in an environment
    #[serde(rename = "env_services")]
    EnvServices {
        environment: String,
    },
    /// Find what uses a specific database
    #[serde(rename = "db_dependents")]
    DbDependents {
        database_id: String,
    },
    /// Find path between two services (dependency chain)
    #[serde(rename = "path")]
    Path {
        from_id: String,
        to_id: String,
        max_depth: Option<u32>,
    },
    /// What secrets does this service need?
    #[serde(rename = "service_secrets")]
    ServiceSecrets {
        service_id: String,
    },
    /// What actions are safe to perform on this service?
    #[serde(rename = "safe_operations")]
    SafeOperations {
        service_id: String,
    },
}

/// API response for map queries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapResponse {
    pub ok: bool,
    pub query: String,
    pub nodes: Vec<MapNode>,
    pub edges: Vec<MapEdge>,
    pub answer: Option<String>,  // Human-readable summary for agent
}

impl MapNode {
    pub fn id(&self) -> &str {
        match self {
            MapNode::Host { id, .. } => id,
            MapNode::Service { id, .. } => id,
            MapNode::Database { id, .. } => id,
            MapNode::Queue { id, .. } => id,
            MapNode::Endpoint { id, .. } => id,
            MapNode::SecretRef { id, .. } => id,
            MapNode::Monitor { id, .. } => id,
            MapNode::Environment { id, .. } => id,
        }
    }

    pub fn node_type(&self) -> &str {
        match self {
            MapNode::Host { .. } => "host",
            MapNode::Service { .. } => "service",
            MapNode::Database { .. } => "database",
            MapNode::Queue { .. } => "queue",
            MapNode::Endpoint { .. } => "endpoint",
            MapNode::SecretRef { .. } => "secret_ref",
            MapNode::Monitor { .. } => "monitor",
            MapNode::Environment { .. } => "environment",
        }
    }

    pub fn name(&self) -> &str {
        match self {
            MapNode::Host { hostname, .. } => hostname,
            MapNode::Service { name, .. } => name,
            MapNode::Database { name, .. } => name,
            MapNode::Queue { name, .. } => name,
            MapNode::Endpoint { url, .. } => url,
            MapNode::SecretRef { key_name, .. } => key_name,
            MapNode::Monitor { name, .. } => name,
            MapNode::Environment { name, .. } => name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_node_host() {
        let node = MapNode::Host {
            id: "host-1".into(),
            hostname: "prod-web-01".into(),
            ip: Some("10.0.1.5".into()),
            os: Some("Ubuntu 22.04".into()),
            labels: HashMap::new(),
        };
        assert_eq!(node.id(), "host-1");
        assert_eq!(node.node_type(), "host");
        assert_eq!(node.name(), "prod-web-01");
    }

    #[test]
    fn test_map_node_service() {
        let node = MapNode::Service {
            id: "svc-1".into(),
            name: "billing-api".into(),
            service_type: "custom-app".into(),
            version: Some("v2.3.1".into()),
            environment: Some("prod".into()),
            criticality: Some(Criticality::Critical),
            owner: Some("payments-team".into()),
            labels: HashMap::new(),
        };
        assert_eq!(node.node_type(), "service");
        assert_eq!(node.name(), "billing-api");
    }

    #[test]
    fn test_map_node_database() {
        let node = MapNode::Database {
            id: "db-1".into(),
            name: "payments-db".into(),
            db_type: "postgres".into(),
            host: Some("db.internal".into()),
            port: Some(5432),
            environment: Some("prod".into()),
            labels: HashMap::new(),
        };
        assert_eq!(node.node_type(), "database");
    }

    #[test]
    fn test_map_edge() {
        let edge = MapEdge {
            id: "edge-1".into(),
            from_id: "host-1".into(),
            to_id: "svc-1".into(),
            rel_type: RelationType::HostsService,
            metadata: HashMap::new(),
        };
        assert_eq!(edge.rel_type, RelationType::HostsService);
    }

    #[test]
    fn test_relation_types() {
        let types = vec![
            RelationType::HostsService,
            RelationType::ServiceUsesDb,
            RelationType::ServiceUsesQueue,
            RelationType::ServiceExposesApi,
            RelationType::ServiceMonitoredBy,
            RelationType::ServiceHasSecret,
            RelationType::ServiceInEnv,
            RelationType::DependsOn,
            RelationType::ConnectedTo,
        ];
        // Verify serialization roundtrip
        for rt in &types {
            let json = serde_json::to_string(rt).unwrap();
            let back: RelationType = serde_json::from_str(&json).unwrap();
            assert_eq!(rt, &back);
        }
    }

    #[test]
    fn test_map_query_find_service() {
        let query = MapQuery::FindService {
            name: "billing".into(),
            environment: Some("prod".into()),
        };
        let json = serde_json::to_string(&query).unwrap();
        assert!(json.contains("find_service"));
        assert!(json.contains("billing"));
    }

    #[test]
    fn test_map_query_service_topology() {
        let query = MapQuery::ServiceTopology {
            service_id: "svc-1".into(),
            depth: Some(3),
        };
        let json = serde_json::to_string(&query).unwrap();
        assert!(json.contains("service_topology"));
    }

    #[test]
    fn test_criticality_serialization() {
        let levels = vec![Criticality::Low, Criticality::Medium, Criticality::High, Criticality::Critical];
        for c in &levels {
            let json = serde_json::to_string(c).unwrap();
            let back: Criticality = serde_json::from_str(&json).unwrap();
            assert_eq!(c, &back);
        }
    }

    #[test]
    fn test_snapshot_serialization() {
        let snapshot = InfrastructureSnapshot {
            agent_id: "agent-1".into(),
            host_id: "host-1".into(),
            timestamp: "2026-04-25T19:00:00Z".into(),
            nodes: vec![
                MapNode::Host {
                    id: "host-1".into(),
                    hostname: "prod-01".into(),
                    ip: Some("10.0.1.1".into()),
                    os: None,
                    labels: HashMap::new(),
                },
            ],
            edges: vec![],
            version: 1,
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let back: InfrastructureSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(back.agent_id, "agent-1");
        assert_eq!(back.nodes.len(), 1);
    }
}
