#!/bin/bash
# Rebuild BPF + Rust + Deploy — one script for any kernel
# Usage: bash deploy.sh [PORT]
set -e

PORT=${1:-8091}
VPS=${2:-root@93.93.207.44}

echo "=== Deploying FlowLink to $VPS on port $PORT ==="

# 1. Upload source
echo "[1/5] Uploading source..."
cd "$(dirname "$0")/.."
tar czf /tmp/fl-deploy.tar.gz --exclude="target" --exclude=".git" Cargo.toml Cargo.lock crates/ README.md
scp /tmp/fl-deploy.tar.gz "$VPS:/tmp/"

ssh "$VPS" bash << REMOTE
set -e
echo "[2/5] Generating vmlinux.h for current kernel..."
cd /root/flowlink && tar xzf /tmp/fl-deploy.tar.gz 2>/dev/null

# Find bpftool that matches current kernel
KVER=\$(uname -r | cut -d- -f1)
BPFTOOL=""
# Try kernel-matching bpftool first
for bt in /usr/lib/linux-tools/\$KVER-*/bpftool /usr/lib/linux-tools/\$(uname -r)/bpftool; do
    [ -x "\$bt" ] && BPFTOOL="\$bt" && break
done
# Fallback to system bpftool
[ -z "\$BPFTOOL" ] && BPFTOOL=\$(which bpftool 2>/dev/null)

if [ -z "\$BPFTOOL" ]; then
    echo "ERROR: bpftool not found. Install linux-tools for kernel \$(uname -r)"
    exit 1
fi

echo "Using bpftool: \$BPFTOOL (\$(\$BPFTOOL version 2>/dev/null | head -1))"

# Generate vmlinux.h from current kernel BTF
\$BPFTOOL btf dump file /sys/kernel/btf/vmlinux format c > crates/sentinel/bpf/vmlinux_local.h 2>/dev/null
VMLINES=\$(wc -l < crates/sentinel/bpf/vmlinux_local.h)
echo "vmlinux.h: \$VMLINES lines (kernel \$(uname -r))"

if [ "\$VMLINES" -lt 100 ]; then
    echo "ERROR: vmlinux.h too small — BTF not available for this kernel"
    echo "Install: apt-get install linux-tools-\$(uname -r)"
    exit 1
fi

# Check LSM BPF support
if ! cat /sys/kernel/security/lsm | grep -q "bpf"; then
    echo "WARNING: BPF not in LSM list. LSM blocking will be userspace-only."
    echo "To enable: add lsm=bpf to kernel cmdline and reboot"
fi

echo "[3/5] Compiling BPF..."
cd crates/sentinel/bpf
clang -O2 -g -target bpf -D__TARGET_ARCH_x86 -c sentinel_lsm.bpf.c -o sentinel_lsm.bpf.o 2>&1
# Strip DWARF debug (keeps BTF) — needed for aya/libbpf compat
llvm-strip --strip-debug sentinel_lsm.bpf.o 2>/dev/null || true
echo "BPF: \$(stat -c%s sentinel_lsm.bpf.o) bytes"

echo "[4/5] Building Rust..."
cd /root/flowlink
. "\$HOME/.cargo/env"
cargo clean -p flowlink-sentinel --release 2>&1 | tail -1
cargo build --release --features "linux-ebpf" -p flowlink 2>&1 | grep -E "error|Finished"
cp target/release/flowlink /opt/flowlink-playground/

echo "[5/5] Deploying Docker..."
cd /opt/flowlink-playground
rm -rf /sys/fs/bpf/* 2>/dev/null
docker rm -f flowlink-api 2>/dev/null || true
docker build -t flowlink-e2e . 2>&1 | tail -1
docker run -d --name flowlink-api --privileged --pid=host --net=host \\
    -e RUST_LOG=info \\
    -v /sys/kernel/security:/sys/kernel/security \\
    -v /sys/kernel/debug:/sys/kernel/debug \\
    -v /sys/fs/bpf:/sys/fs/bpf \\
    flowlink-e2e api --addr 0.0.0.0:$PORT
sleep 3

echo ""
echo "=== Status ==="
docker logs flowlink-api 2>&1 | grep -E "LSM|blocker|attach" | head -5
echo ""
# Quick health check
curl -sf http://127.0.0.1:$PORT/api/v1/health && echo "" || echo "API not responding"
REMOTE

echo "=== Done ==="
