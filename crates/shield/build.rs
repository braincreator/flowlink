// FlowLink Shield build script
// Compiles the eBPF BPF C program when the "ebpf" feature is enabled.

fn main() {
    #[cfg(feature = "ebpf")]
    compile_bpf();
}

#[cfg(feature = "ebpf")]
fn compile_bpf() {
    let bpf_src = std::path::PathBuf::from("src/bpf/shield.bpf.c");
    let bpf_obj = std::path::PathBuf::from("src/bpf/shield.bpf.o");

    if !bpf_src.exists() {
        eprintln!(
            "cargo:warning=shield BPF source not found at {}",
            bpf_src.display()
        );
        return;
    }

    // Try to find clang
    let clang = std::env::var("CLANG").unwrap_or_else(|_| "clang".into());

    let output = std::process::Command::new(&clang)
        .args([
            "-g",
            "-O2",
            "-target",
            "bpf",
            "-D__TARGET_ARCH_x86",
            "-I",
            "/usr/include/bpf",
            "-I",
            "/usr/include/x86_64-linux-gnu",
            "-c",
            bpf_src.to_str().unwrap(),
            "-o",
            bpf_obj.to_str().unwrap(),
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            println!("cargo:rerun-if-changed={}", bpf_src.display());
            println!("cargo:rustc-env=SHIELD_BPF_COMPILED=1");
            eprintln!("cargo:warning=shield BPF compiled successfully");
        }
        Ok(out) => {
            eprintln!("cargo:warning=shield BPF compilation failed (this is OK on non-Linux):");
            eprintln!("cargo:warning={}", String::from_utf8_lossy(&out.stderr));
            // Don't fail the build — eBPF is optional
        }
        Err(e) => {
            eprintln!(
                "cargo:warning=shield BPF compilation skipped (clang not found): {}",
                e
            );
        }
    }
}
