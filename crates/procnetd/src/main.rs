#![feature(never_type)]

use std::mem::MaybeUninit;

use env_logger::Env;
use libbpf_rs::skel::{OpenSkel, Skel, SkelBuilder};

use procnet::ProcnetSkelBuilder;

#[allow(
    warnings,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo,
    clippy::restriction
)]
mod procnet {
    include!(concat!(env!("OUT_DIR"), "/procnet.skel.rs"));
}

mod app;
mod errors;
mod events;
mod server;
mod signals;
mod state;
mod stats_map;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();

    bump_memlock_rlimit();

    let skel_builder = ProcnetSkelBuilder::default();

    let mut open_object = MaybeUninit::uninit();
    let open_skel = skel_builder.open(&mut open_object)?;

    let mut skel = open_skel.load()?;
    skel.attach()?;

    let stats_map = &skel.maps.STATS;
    let events_map = &skel.maps.EVENTS;

    app::run(stats_map, events_map)?;

    Ok(())
}

fn bump_memlock_rlimit() {
    let limit = 128 * 1024 * 1024;
    let rlimit = libc::rlimit {
        rlim_cur: limit,
        rlim_max: limit,
    };

    let ret = unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &raw const rlimit) };
    if ret != 0 {
        let err = std::io::Error::last_os_error();
        log::info!(
            "failed to increase RLIMIT_MEMLOCK: {err} \
             (on kernel >= 5.11 with CAP_BPF this is harmless)"
        );
    }
}
