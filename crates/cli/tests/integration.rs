// Integration test: agent→relay message roundtrip
// Tests that protocol messages serialize/deserialize correctly end-to-end

use flowlink_core::*;
use flowlink_crypto::KeyPair;

#[test]
fn test_connect_message_roundtrip() {
    let payload = ConnectPayload {
        agent_id: "test-agent-001".into(),
        token: "secret-token".into(),
        hostname: "testbox".into(),
        os: "linux".into(),
        arch: "x86_64".into(),
        client_version: Some("0.1.0".into()),
        public_key: None,
        protocol_version: Some(PROTOCOL_VERSION),
    };

    let msg = Message::new(MessageType::Connect)
        .with_agent_id("test-agent-001")
        .with_payload(&payload);

    // Serialize
    let json = serde_json::to_string(&msg).unwrap();

    // Deserialize
    let decoded: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.msg_type, MessageType::Connect);
    assert_eq!(decoded.agent_id.as_deref(), Some("test-agent-001"));

    let decoded_payload: ConnectPayload = serde_json::from_value(decoded.payload.unwrap()).unwrap();
    assert_eq!(decoded_payload.agent_id, "test-agent-001");
    assert_eq!(decoded_payload.hostname, "testbox");
}

#[test]
fn test_exec_request_roundtrip() {
    let payload = ExecRequestPayload {
        command: "ls -la /tmp".into(),
        shell: Some("/bin/bash".into()),
        env: None,
        dir: Some("/home/user".into()),
        timeout_sec: 30,
        request_id: "req-123".into(),
    };

    let msg = Message::new(MessageType::ExecRequest)
        .with_agent_id("agent-1")
        .with_payload(&payload);

    let json = serde_json::to_string(&msg).unwrap();
    let decoded: Message = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded.msg_type, MessageType::ExecRequest);
    let p: ExecRequestPayload = serde_json::from_value(decoded.payload.unwrap()).unwrap();
    assert_eq!(p.command, "ls -la /tmp");
    assert_eq!(p.timeout_sec, 30);
}

#[test]
fn test_exec_done_roundtrip() {
    let payload = ExecDonePayload {
        request_id: "req-123".into(),
        exit_code: 0,
        duration_ms: 150,
        error: None,
        stdout: "".into(),
        stderr: "".into(),
    };

    let msg = Message::new(MessageType::ExecDone)
        .with_agent_id("agent-1")
        .with_payload(&payload);

    let json = serde_json::to_string(&msg).unwrap();
    let decoded: Message = serde_json::from_str(&json).unwrap();

    let p: ExecDonePayload = serde_json::from_value(decoded.payload.unwrap()).unwrap();
    assert_eq!(p.exit_code, 0);
}

#[test]
fn test_error_payload_roundtrip() {
    let payload = ErrorPayload {
        code: "EXEC_BLOCKED".into(),
        message: "Command blocked by policy".into(),
    };

    let msg = Message::new(MessageType::Error)
        .with_agent_id("agent-1")
        .with_payload(&payload);

    let json = serde_json::to_string(&msg).unwrap();
    let decoded: Message = serde_json::from_str(&json).unwrap();

    let p: ErrorPayload = serde_json::from_value(decoded.payload.unwrap()).unwrap();
    assert_eq!(p.code, "EXEC_BLOCKED");
}

#[test]
fn test_e2ee_encrypt_decrypt() {
    let alice = KeyPair::generate();
    let bob = KeyPair::generate();

    let plaintext = b"Hello, FlowLink! This is a secret message.";

    // Alice encrypts for Bob
    let envelope = flowlink_crypto::encrypt(&alice, &bob.public_key, plaintext).unwrap();

    // Bob decrypts
    let decrypted = flowlink_crypto::decrypt(&bob, &envelope).unwrap();

    assert_eq!(decrypted, plaintext);
}

#[test]
fn test_e2ee_wrong_key_fails() {
    let alice = KeyPair::generate();
    let bob = KeyPair::generate();
    let eve = KeyPair::generate();

    let plaintext = b"Secret data";

    let envelope = flowlink_crypto::encrypt(&alice, &bob.public_key, plaintext).unwrap();

    // Eve tries to decrypt — should fail
    let result = flowlink_crypto::decrypt(&eve, &envelope);
    assert!(result.is_err());
}

#[test]
fn test_policy_blocks_dangerous_commands() {
    use flowlink_agent::policy::{PolicyEngine, RiskLevel};

    let engine = PolicyEngine::new(false, false);

    let dangerous = vec![
        "rm -rf /",
        "mkfs.ext4 /dev/sda",
        "dd if=/dev/zero of=/dev/sda",
        "sudo rm -rf /",
        ":(){ :|:& };:",
    ];

    for cmd in &dangerous {
        let payload = ExecRequestPayload {
            command: cmd.to_string(),
            shell: None,
            env: None,
            dir: None,
            timeout_sec: 30,
            request_id: "test".into(),
        };
        let result = engine.check(&payload);
        assert!(result.blocked, "Should block: {cmd}");
    }
}

#[test]
fn test_policy_allows_safe_commands() {
    use flowlink_agent::policy::{PolicyEngine, RiskLevel};

    let engine = PolicyEngine::new(false, false);

    let safe = vec![
        "ls -la",
        "cat /etc/hosts",
        "git status",
        "npm install",
        "pip list",
        "docker ps",
        "echo hello",
    ];

    for cmd in &safe {
        let payload = ExecRequestPayload {
            command: cmd.to_string(),
            shell: None,
            env: None,
            dir: None,
            timeout_sec: 30,
            request_id: "test".into(),
        };
        let result = engine.check(&payload);
        assert!(result.allowed, "Should allow: {cmd}");
    }
}

#[test]
fn test_readonly_mode() {
    use flowlink_agent::policy::{PolicyEngine, RiskLevel};

    let engine = PolicyEngine::new(true, false); // read-only mode

    let blocked_in_readonly = vec!["rm file.txt", "mkdir newdir", "cp src dst", "mv old new"];

    for cmd in &blocked_in_readonly {
        let payload = ExecRequestPayload {
            command: cmd.to_string(),
            shell: None,
            env: None,
            dir: None,
            timeout_sec: 30,
            request_id: "test".into(),
        };
        let result = engine.check(&payload);
        assert!(result.blocked, "Read-only should block: {cmd}");
    }
}

#[test]
fn test_config_roundtrip() {
    let config = flowlink_core::config::AgentConfig {
        agent_id: "test-agent".into(),
        token: "secret".into(),
        relay_url: "wss://relay.example.com".into(),
        heartbeat_sec: 30,
        label: "test".into(),
        work_dir: "/tmp".into(),
        read_only: false,
        use_relay_llm: false,
        sandbox: Default::default(),
        approval: Default::default(),
        backup: Default::default(),
        shield: Default::default(),
        tls: Default::default(),
    };

    let json = serde_json::to_string_pretty(&config).unwrap();
    let parsed: flowlink_core::config::AgentConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.agent_id, "test-agent");
    assert_eq!(parsed.relay_url, "wss://relay.example.com");
}
