use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use flowlink_relay::pool::{AgentPool, AgentInfo};
use flowlink_relay::auth::AuthManager;
use flowlink_relay::ratelimit::RateLimiter;
use flowlink_relay::eventbus::EventBus;
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

fn bench_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("pool");
    group.bench_function("register", |b| {
        let pool = AgentPool::new();
        b.iter(|| {
            pool.register(test_agent("bench-agent"));
        })
    });
    group.bench_function("get", |b| {
        let pool = AgentPool::new();
        pool.register(test_agent("bench-agent"));
        b.iter(|| pool.get("bench-agent"))
    });
    group.bench_function("list_100", |b| {
        let pool = AgentPool::new();
        for i in 0..100 { pool.register(test_agent(&format!("a-{i}"))); }
        b.iter(|| pool.list())
    });
    group.finish();
}

fn bench_auth(c: &mut Criterion) {
    let mut group = c.benchmark_group("auth");
    group.bench_function("validate_token", |b| {
        let auth = AuthManager::new();
        auth.register_client(flowlink_relay::auth::Client {
            client_id: "c1".into(), api_token: "secret-token-123".into(),
            name: "c1".into(), active: true,
        });
        b.iter(|| auth.validate_token("secret-token-123"))
    });
    group.finish();
}

fn bench_ratelimit(c: &mut Criterion) {
    let mut group = c.benchmark_group("ratelimit");
    group.bench_function("allow_1000_per_sec", |b| {
        let rl = RateLimiter::new(1000, 1);
        b.iter(|| rl.allow("bench-key"))
    });
    group.finish();
}

fn bench_eventbus(c: &mut Criterion) {
    c.bench_function("eventbus_publish", |b| {
        let bus = EventBus::new();
        let mut rx = bus.subscribe("bench-ch");
        b.iter(|| bus.publish("bench-ch", "benchmark-event"))
    });
}

fn bench_concurrent_pool(c: &mut Criterion) {
    c.bench_function("pool_concurrent_register", |b| {
        let pool = Arc::new(AgentPool::new());
        b.iter(|| {
            let handles: Vec<_> = (0..8)
                .map(|i| {
                    let p = pool.clone();
                    thread::spawn(move || {
                        let id = format!("agent-{i}");
                        p.register(test_agent(&id));
                        p.get(&id).unwrap();
                        p.unregister(&id);
                    })
                })
                .collect();
            for h in handles { h.join().unwrap(); }
        })
    });
}

criterion_group!(benches, bench_pool, bench_auth, bench_ratelimit, bench_eventbus, bench_concurrent_pool);
criterion_main!(benches);
