//! BlueZ lists cached peripherals and btleplug synthesizes discovery events for
//! them. A cached UUID alone is not proof of a nearby, currently active app.
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

#[derive(Clone, Copy)]
pub enum Evidence {
    DeviceDiscovered,
    RadioUpdate,
}

pub struct FreshScan<I> {
    initial: HashSet<I>,
    live: HashMap<I, u8>,
    rejected: HashSet<I>,
    reported: HashSet<I>,
}

impl<I: Eq + Hash + Clone> FreshScan<I> {
    pub fn new(initial: impl IntoIterator<Item = I>) -> Self {
        Self {
            initial: initial.into_iter().collect(),
            live: HashMap::new(),
            rejected: HashSet::new(),
            reported: HashSet::new(),
        }
    }

    pub fn observe(&mut self, id: I, evidence: Evidence) {
        let existed = self.initial.contains(&id);
        // Initial DeviceDiscovered events can be synthesized from the cache.
        if matches!(evidence, Evidence::DeviceDiscovered) && existed {
            return;
        }
        if self.live.len() >= 512 && !self.live.contains_key(&id) {
            return;
        }
        // Prefer newly seen addresses over old cached/system profiles.
        self.live.insert(id, u8::from(existed));
    }

    pub fn rank(&self, id: &I, has_service: bool) -> Option<u8> {
        if !has_service || self.rejected.contains(id) {
            return None;
        }
        self.live.get(id).copied()
    }

    pub fn reject(&mut self, id: I) {
        self.rejected.insert(id);
    }
    pub fn report_once(&mut self, id: I) -> bool {
        self.reported.insert(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_uuid_and_synthetic_discovery_are_not_live_evidence() {
        let mut scan = FreshScan::new([1]);
        assert_eq!(scan.rank(&1, true), None);
        scan.observe(1, Evidence::DeviceDiscovered);
        assert_eq!(scan.rank(&1, true), None);
        scan.observe(1, Evidence::RadioUpdate);
        assert_eq!(scan.rank(&1, true), Some(1));
    }

    #[test]
    fn fresh_addresses_beat_cached_profiles_but_still_need_service_match() {
        let mut scan = FreshScan::new([1]);
        scan.observe(1, Evidence::RadioUpdate);
        scan.observe(2, Evidence::DeviceDiscovered);
        assert!(scan.rank(&2, true) < scan.rank(&1, true));
        assert_eq!(scan.rank(&2, false), None);
    }

    #[test]
    fn missing_app_service_is_not_retried_as_the_same_scan_candidate() {
        let mut scan = FreshScan::new([]);
        scan.observe(1, Evidence::DeviceDiscovered);
        scan.reject(1);
        scan.observe(1, Evidence::RadioUpdate);
        assert_eq!(scan.rank(&1, true), None);
        scan.observe(2, Evidence::DeviceDiscovered);
        assert_eq!(scan.rank(&2, true), Some(0));
    }

    #[test]
    fn observed_addresses_are_bounded() {
        let mut scan = FreshScan::new([]);
        for id in 0..1024 {
            scan.observe(id, Evidence::DeviceDiscovered);
        }
        assert_eq!(scan.live.len(), 512);
        assert_eq!(scan.rank(&1023, true), None);
    }
}
