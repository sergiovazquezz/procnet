use std::mem::MaybeUninit;

use anyhow::{Context, Result};
use libbpf_rs::skel::{OpenSkel, Skel, SkelBuilder};
use log::LevelFilter;
use log4rs::{
    Config,
    append::file::FileAppender,
    config::{Appender, Root},
    encode::pattern::PatternEncoder,
};

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

use procnet::ProcnetSkelBuilder;

mod app;
mod events;
mod server;
mod stats_map;

fn main() -> Result<()> {
    bump_memlock_rlimit()?;

    let skel_builder = ProcnetSkelBuilder::default();

    let mut open_object = MaybeUninit::uninit();
    let open_skel = skel_builder.open(&mut open_object)?;

    let mut skel = open_skel.load()?;
    skel.attach()?;

    let stats_map = &skel.maps.STATS;
    let events_map = &skel.maps.EVENTS;

    let file = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {m}{n}")))
        .build("logs/app.log")?;

    let config = Config::builder()
        .appender(Appender::builder().build("file", Box::new(file)))
        .build(Root::builder().appender("file").build(LevelFilter::Debug))?;

    log4rs::init_config(config)?;

    app::run(stats_map, events_map)
}

fn bump_memlock_rlimit() -> Result<()> {
    let limit = 128 * 1024 * 1024;
    let rlimit = libc::rlimit {
        rlim_cur: limit,
        rlim_max: limit,
    };

    let ret = unsafe { libc::setrlimit(libc::RLIMIT_MEMLOCK, &raw const rlimit) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error()).context("failed to increase RLIMIT_MEMLOCK");
    }

    Ok(())
}
