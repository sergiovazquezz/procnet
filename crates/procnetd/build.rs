use std::env;
use std::path::PathBuf;

use libbpf_cargo::SkeletonBuilder;

const SRC: &str = "src/bpf/procnet.bpf.c";

#[allow(clippy::expect_used)]
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

    SkeletonBuilder::new()
        .source(SRC)
        .build_and_generate(&out)
        .expect("failed to build procnet BPF skeleton");

    println!("cargo:rerun-if-changed={SRC}");
}
