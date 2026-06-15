use std::mem::MaybeUninit;
use std::thread;
use std::time::Duration;

use libbpf_rs::skel::{OpenSkel, Skel, SkelBuilder};

use anyhow::Result;

mod procnet {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bpf/procnet.skel.rs"
    ));
}

use procnet::ProcnetSkelBuilder;

fn main() -> Result<()> {
    let skel_builder = ProcnetSkelBuilder::default();

    let mut open_object = MaybeUninit::uninit();
    let open_skel = skel_builder.open(&mut open_object)?;

    let mut skel = open_skel.load()?;
    skel.attach()?;

    println!("procnet eBPF program loaded. Press Ctrl-C to exit.");

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
