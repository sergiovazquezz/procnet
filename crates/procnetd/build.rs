#![expect(clippy::expect_used, clippy::panic)]

use std::{env, fs, path::PathBuf, process::Command};

use libbpf_cargo::SkeletonBuilder;

const SRC: &str = "src/bpf/procnet.bpf.c";

fn main() {
    if let Ok(hardening) = env::var("NIX_HARDENING_ENABLE") {
        let filtered = hardening
            .split_whitespace()
            .filter(|flag| *flag != "zerocallusedregs")
            .collect::<Vec<_>>()
            .join(" ");

        // This Nix hardening flag is not supported by clang's BPF target
        unsafe {
            env::set_var("NIX_HARDENING_ENABLE", filtered);
        }
    }

    let out =
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set")).join("procnet.skel.rs");

    generate_vmlinux_header();

    SkeletonBuilder::new()
        .source(SRC)
        .build_and_generate(&out)
        .expect("Failed to build procnet BPF skeleton");

    println!("cargo:rerun-if-changed={SRC}");
}

fn generate_vmlinux_header() {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set"));
    let vmlinux_h = manifest_dir.join("src/bpf/vmlinux.h");
    let btf_path = PathBuf::from("/sys/kernel/btf/vmlinux");

    let skip = env::var_os("PROCNET_SKIP_VMLINUX_GEN").is_some_and(|v| v != "0");

    if skip || !btf_path.exists() {
        if vmlinux_h.exists() {
            return;
        }
        panic!(
            "procnetd build: vmlinux.h is missing and /sys/kernel/btf/vmlinux is unavailable \
             (or PROCNET_SKIP_VMLINUX_GEN is set). On a Linux host with BTF, regenerate it with:\n  \
             bpftool btf dump file /sys/kernel/btf/vmlinux format c > crates/procnetd/src/bpf/vmlinux.h\n\
             then commit it."
        );
    }

    let output = Command::new("bpftool")
        .args([
            "btf",
            "dump",
            "file",
            "/sys/kernel/btf/vmlinux",
            "format",
            "c",
        ])
        .output()
        .expect("Failed to run bpftool; make sure bpftool is installed");

    assert!(
        output.status.success(),
        "bpftool failed: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    fs::write(&vmlinux_h, output.stdout).expect("Failed to write vmlinux.h");
}
