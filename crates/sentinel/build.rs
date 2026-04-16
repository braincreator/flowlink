// Build script: compile LSM BPF program with proper headers
// Downloads bpf_helpers.h, bpf_tracing.h from kernel tree if not present
// Generates vmlinux.h from /sys/kernel/btf/vmlinux

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

const BPF_HEADERS: &[&str] = &[
    "bpf_helpers.h",
    "bpf_tracing.h", 
    "bpf_core_read.h",
    "bpf_endian.h",
    "libbpf_common.h",
    "libbpf_internal.h",
    "bpf.h",
    "btf.h",
    "libbpf.h",
    "skel_internal.h",
    "strset.h",
    "str_error.h",
    "inner_array.h",
    "relo_core.h",
    "usdt.bpf.h",
];

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let bpf_dir = Path::new(&manifest_dir).join("bpf");
    let headers_dir = bpf_dir.join("bpf_headers");
    let bpf_c = bpf_dir.join("sentinel_lsm.bpf.c");
    let bpf_o = bpf_dir.join("sentinel_lsm.bpf.o");
    let vmlinux_h = bpf_dir.join("vmlinux_local.h");

    if !cfg!(target_os = "linux") {
        println!("cargo:warning=Not Linux — skipping BPF compilation");
        return;
    }

    // Step 1: Ensure bpf_headers directory exists
    let _ = fs::create_dir_all(&headers_dir);
    let _ = fs::create_dir_all(headers_dir.join("linux"));
    let _ = fs::create_dir_all(headers_dir.join("uapi").join("linux"));

    // Step 2: Download bpf headers from kernel if missing
    let base_url = "https://raw.githubusercontent.com/torvalds/linux/master";
    let libbpf_url = format!("{}/tools/lib/bpf", base_url);
    let uapi_url = format!("{}/include/uapi/linux", base_url);

    for header in BPF_HEADERS {
        let path = headers_dir.join(header);
        if !path.exists() {
            let url = format!("{}/{}", libbpf_url, header);
            let _ = Command::new("curl").args(["-sL", "-o"])
                .arg(&path).arg(&url).output();
        }
    }

    // Download uapi headers
    for (name, subdir) in &[("bpf.h", "uapi/linux"), ("bpf_common.h", "uapi/linux")] {
        let path = headers_dir.join(subdir).join(name);
        if !path.exists() {
            let url = format!("{}/{}", uapi_url, name);
            let _ = Command::new("curl").args(["-sL", "-o"])
                .arg(&path).arg(&url).output();
        }
    }

    // Step 3: Generate vmlinux.h
    if Path::new("/sys/kernel/btf/vmlinux").exists() && 
        (!vmlinux_h.exists() || fs::metadata(&vmlinux_h).map(|m| m.len() < 100).unwrap_or(true)) {
        
        let kernel_release = Command::new("uname").arg("-r").output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        let bpftools = [
            format!("/usr/lib/linux-tools/{}/bpftool", kernel_release),
            format!("/usr/lib/linux-tools/{}/bpftool", kernel_release.split('-').next().unwrap_or("")),
            "/usr/local/bin/bpftool".to_string(),
            "/usr/bin/bpftool".to_string(),
        ];

        for bpftool in &bpftools {
            if !Path::new(bpftool).exists() { continue; }
            let out = Command::new(bpftool)
                .args(["btf", "dump", "file", "/sys/kernel/btf/vmlinux", "format", "c"])
                .output();
            if let Ok(out) = out {
                if out.status.success() && !out.stdout.is_empty() {
                    let _ = fs::write(&vmlinux_h, &out.stdout);
                    println!("cargo:warning=vmlinux.h generated ({} bytes)", out.stdout.len());
                    break;
                }
            }
        }
    }

    // Step 4: Compile BPF
    let has_vmlinux = vmlinux_h.exists() && fs::metadata(&vmlinux_h).map(|m| m.len() > 100).unwrap_or(false);
    if !has_vmlinux {
        println!("cargo:warning=vmlinux.h not available — skipping BPF compilation");
        return;
    }

    let has_headers = headers_dir.join("bpf_helpers.h").exists() && 
                       headers_dir.join("bpf_tracing.h").exists();

    let mut cmd = Command::new("clang");
    cmd.args(["-O2", "-g", "-target", "bpf", "-D__TARGET_ARCH_x86", "-DHAS_VMLINUX"]);
    
    if has_headers {
        cmd.args(["-I", headers_dir.to_str().unwrap()]);
    } else {
        // Try system headers
        cmd.args(["-I", "/usr/include"]);
    }
    
    cmd.args(["-c", bpf_c.to_str().unwrap(), "-o", bpf_o.to_str().unwrap()]);

    match cmd.output() {
        Ok(out) if out.status.success() => {
            let _ = Command::new("llvm-strip")
                .args(["--strip-debug", bpf_o.to_str().unwrap()])
                .output();
            println!("cargo:warning=BPF compiled: {} bytes", fs::metadata(&bpf_o).map(|m| m.len()).unwrap_or(0));
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            println!("cargo:warning=BPF compilation failed: {}", stderr);
            // Check if pre-compiled .o exists
            if !bpf_o.exists() {
                println!("cargo:warning=No BPF object — LSM blocking unavailable");
            }
        }
        Err(e) => {
            println!("cargo:warning=clang not found: {}", e);
        }
    }

    println!("cargo:rerun-if-changed=bpf/sentinel_lsm.bpf.c");
    println!("cargo:rerun-if-changed=bpf/vmlinux_local.h");
}
