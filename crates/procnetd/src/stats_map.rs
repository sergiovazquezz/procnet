use libbpf_rs::{MapCore, MapFlags, MapMut};
use procnet_core::stats::StatsMap;

/// Thin wrapper so we can implement the shared `StatsMap` trait for
/// `libbpf_rs::MapMut` without violating orphan rules.
pub struct MapMutWrapper<'a>(pub &'a MapMut<'a>);

impl StatsMap for MapMutWrapper<'_> {
    fn lookup_percpu(&self, key: &[u8]) -> Option<Vec<Vec<u8>>> {
        self.0.lookup_percpu(key, MapFlags::ANY).ok().flatten()
    }
}
