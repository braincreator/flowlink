use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use flowlink_relay::approval::ApprovalQueue;
use flowlink_relay::auth::AuthManager;
use flowlink_relay::devices::DeviceManager;
use flowlink_relay::eventbus::EventBus;
use flowlink_relay::pool::{AgentInfo, AgentPool};
use flowlink_relay::ratelimit::RateLimiter;
use std::sync::Arc;
use std::thread;

fn test_agent(id: &str) -> AgentInfo {
    AgentInfo {
        agent_id: id.into(),
        hostname: format!("host-{id}"),
        os: "linux".into(),
        arch: "x86_64".into(),
        connected_at: 1000,
        last_heartbeat: 1000,
        labels: vec![],
        capabilities: vec![],
    }
}

fn bench_pool_register(c: &mut Criterion) {
    c.bench_function("relay/pool_register", |b| {
        let pool = AgentPool::new();
        b.iter(|| pool.register(test_agent("bench-agent")))
    });
}

fn bench_pool_lookup(c: &mut Criterion) {
    let pool = AgentPool::new();
    pool.register(test_agent("bench-agent"));
    c.bench_function("relay/pool_lookup", |b| b.iter(|| pool.get("bench-agent")));
}

fn bench_eventbus(c: &mut Criterion) {
    let mut group = c.benchmark_group("relay/eventbus");

    group.bench_function("publish_subscribe_1000", |b| {
        let bus = EventBus::new();
        let _rx = bus.subscribe("bench-ch");
        b.iter(|| {
            for _ in 0..1000 {
                bus.publish("bench-ch", "benchmark-event");
            }
        })
    });

    group.bench_function("publish_subscribe_10000", |b| {
        let bus = EventBus::new();
        let _rx = bus.subscribe("bench-ch");
        b.iter(|| {
            for _ in 0..10000 {
                bus.publish("bench-ch", "benchmark-event");
            }
        })
    });

    group.finish();
}

fn bench_approval_queue(c: &mut Criterion) {
    c.bench_function("relay/approval_enqueue_dequeue", |b| {
        let queue = ApprovalQueue::new();
        b.iter(|| {
            let (tx, mut rx) = tokio::sync::oneshot::channel();
            queue.enqueue(
                flowlink_relay::approval::ApprovalRequest {
                    id: uuid::Uuid::new_v4().to_string(),
                    agent_id: "bench-agent".into(),
                    command: "ls /tmp".into(),
                    risk_level: "low".into(),
                    created_at: 1000,
                },
                tx,
            );
            queue.resolve(
                "nonexistent",
                flowlink_relay::approval::ApprovalDecision::Approved,
            );
            let _ = rx.try_recv();
        })
    });
}

fn bench_ratelimit(c: &mut Criterion) {
    c.bench_function("relay/ratelimit_1000_requests", |b| {
        let rl = RateLimiter::new(1000, 1);
        b.iter(|| {
            for _ in 0..1000 {
                rl.allow("bench-key");
            }
        })
    });
}

fn bench_mcp_parsing(c: &mut Criterion) {
    let raw = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}"#;
    c.bench_function("relay/mcp_parse_tools_list", |b| {
        b.iter(|| {
            let _v: serde_json::Value = serde_json::from_str(raw).unwrap();
        })
    });

    let call_raw = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"exec","arguments":{"command":"ls /tmp"}}}"#;
    c.bench_function("relay/mcp_parse_tools_call", |b| {
        b.iter(|| {
            let _v: serde_json::Value = serde_json::from_str(call_raw).unwrap();
        })
    });
}

fn bench_device_pairing(c: &mut Criterion) {
    let dm = DeviceManager::new(flowlink_relay::devices::PushConfig::default());
    c.bench_function("relay/device_generate_pairing_code", |b| {
        b.iter(|| dm.generate_pairing_code("user-1"))
    });
}

criterion_group!(
    benches,
    bench_pool_register,
    bench_pool_lookup,
    bench_eventbus,
    bench_approval_queue,
    bench_ratelimit,
    bench_mcp_parsing,
    bench_device_pairing
);
criterion_main!(benches);
