use criterion::{criterion_group, criterion_main, Criterion};
use flowlink_agent::policy::PolicyEngine;
use flowlink_agent::sandbox::Sandbox;
use flowlink_core::ExecRequestPayload;
use tempfile::TempDir;

fn make_payload(cmd: &str) -> ExecRequestPayload {
    ExecRequestPayload {
        command: cmd.into(),
        shell: None,
        env: None,
        dir: None,
        timeout_sec: 30,
        request_id: "bench-req".into(),
    }
}

fn bench_policy_allow(c: &mut Criterion) {
    let policy = PolicyEngine::new(false, false);
    c.bench_function("agent/policy_check_allow", |b| {
        b.iter(|| policy.check(&make_payload("ls -la /tmp")))
    });
}

fn bench_policy_deny(c: &mut Criterion) {
    let policy = PolicyEngine::new(false, false);
    c.bench_function("agent/policy_check_deny", |b| {
        b.iter(|| policy.check(&make_payload("rm -rf /")))
    });
}

fn bench_policy_ask(c: &mut Criterion) {
    let policy = PolicyEngine::new(false, false);
    c.bench_function("agent/policy_check_ask", |b| {
        b.iter(|| policy.check(&make_payload("curl -s https://example.com/api")))
    });
}

fn bench_fileops_validate(c: &mut Criterion) {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().to_str().unwrap().to_string();
    let fileops = flowlink_agent::fileops::FileOps::new(vec![dir.clone()], 10 * 1024 * 1024);

    let safe_file = tmp.path().join("test.txt");
    std::fs::write(&safe_file, b"hello").unwrap();
    let safe_path = safe_file.to_str().unwrap().to_string();

    c.bench_function("agent/fileops_validate_safe", |b| {
        b.iter(|| fileops.read(&safe_path))
    });
}

fn bench_fileops_read_write(c: &mut Criterion) {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().to_str().unwrap().to_string();
    let fileops = flowlink_agent::fileops::FileOps::new(vec![dir.clone()], 10 * 1024 * 1024);

    let path = tmp.path().join("bench_rw.bin");
    let path_str = path.to_str().unwrap().to_string();

    let small_data = vec![0xAB_u8; 1024];
    let medium_data = vec![0xAB_u8; 10240];

    let mut group = c.benchmark_group("agent/fileops");

    group.bench_function("write_1kb", |b| {
        b.iter(|| fileops.write(&path_str, &small_data))
    });
    group.bench_function("write_10kb", |b| {
        b.iter(|| fileops.write(&path_str, &medium_data))
    });
    group.bench_function("read_1kb", |b| {
        b.iter(|| fileops.read(&path_str))
    });

    group.finish();
}

fn bench_sandbox_validate_command(c: &mut Criterion) {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().to_str().unwrap().to_string();
    let sandbox = Sandbox::new(
        vec![dir],
        vec!["rm -rf".into()],
        10 * 1024 * 1024,
        30,
        false,
    );

    c.bench_function("agent/sandbox_validate_safe", |b| {
        b.iter(|| sandbox.validate_command("ls /tmp"))
    });

    c.bench_function("agent/sandbox_validate_blocked", |b| {
        b.iter(|| sandbox.validate_command("rm -rf /"))
    });
}

fn bench_dispatch_routing(c: &mut Criterion) {
    // We can't easily bench full dispatch without all deps, so bench the policy check
    // which is the hot path in dispatch
    let policy = PolicyEngine::new(false, false);
    c.bench_function("agent/dispatch_policy_hotpath", |b| {
        b.iter_batched(
            || make_payload("echo hello"),
            |payload| policy.check(&payload),
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(benches, bench_policy_allow, bench_policy_deny, bench_policy_ask, bench_fileops_validate, bench_fileops_read_write, bench_sandbox_validate_command, bench_dispatch_routing);
criterion_main!(benches);
