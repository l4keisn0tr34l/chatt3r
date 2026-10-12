//! BlueZ lists cached peripherals and btleplug synthesizes discovery events for
//! them. A cached UUID alone is not proof of a nearby, currently active app.
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;

pub struct ProbeOrder<I> {
    recent: VecDeque<I>,
}

impl<I: Eq> ProbeOrder<I> {
    pub fn new() -> Self {
        Self {
            recent: VecDeque::new(),
        }
    }
    pub fn priority(&self, id: &I) -> usize {
        self.recent
            .iter()
            .position(|seen| seen == id)
            .map_or(0, |index| index + 1)
    }
    pub fn record(&mut self, id: I) {
        self.recent.retain(|seen| seen != &id);
        self.recent.push_back(id);
        if self.recent.len() > 512 {
            self.recent.pop_front();
        }
    }
}

#[derive(Clone, Copy)]
pub enum Evidence {
    DeviceDiscovered,
    RadioUpdate,
    MatchingService,
}

pub struct FreshScan<I> {
    initial: HashSet<I>,
    live: HashMap<I, (u8, bool)>,
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
        let matching_service = matches!(evidence, Evidence::MatchingService);
        self.live
            .entry(id)
            .and_modify(|(_, advertised)| *advertised |= matching_service)
            .or_insert((u8::from(existed), matching_service));
    }

    pub fn rank(&self, id: &I, has_service: bool) -> Option<u8> {
        if self.rejected.contains(id) {
            return None;
        }
        let (rank, advertised) = *self.live.get(id)?;
        (has_service || advertised).then_some(rank)
    }

    pub fn counts(&self) -> (usize, usize) {
        (
            self.live.len(),
            self.live
                .values()
                .filter(|(_, advertised)| *advertised)
                .count(),
        )
    }

    /// A probe is only a candidate; live GATT verification must precede chat.
    pub fn probe_rank(&self, id: &I) -> Option<u8> {
        if self.rejected.contains(id) {
            return None;
        }
        self.live.get(id).map(|(rank, _)| rank + 2)
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
    fn uuidless_probe_requires_fresh_evidence_and_respects_rejections() {
        let mut scan = FreshScan::new([1]);
        scan.observe(1, Evidence::DeviceDiscovered);
        assert_eq!(scan.probe_rank(&1), None);
        scan.observe(1, Evidence::RadioUpdate);
        assert_eq!(scan.rank(&1, false), None);
        assert_eq!(scan.probe_rank(&1), Some(3));
        scan.observe(2, Evidence::MatchingService);
        assert!(scan.rank(&2, false).unwrap() < scan.probe_rank(&1).unwrap());
        scan.reject(1);
        assert_eq!(scan.probe_rank(&1), None);
        for id in 3..1024 {
            scan.observe(id, Evidence::RadioUpdate);
        }
        assert_eq!(scan.probe_rank(&1023), None);
        assert_eq!(FreshScan::new([1]).probe_rank(&1), None);
    }

    #[test]
    fn failed_probes_rotate_across_windows_without_unbounded_history() {
        let mut order = ProbeOrder::new();
        order.record(1);
        assert!(order.priority(&2) < order.priority(&1));
        order.record(2);
        assert!(order.priority(&1) < order.priority(&2));
        order.record(1);
        assert!(order.priority(&2) < order.priority(&1));
        assert_eq!(order.recent.len(), 2);
        for id in 3..1024 {
            order.record(id);
        }
        assert_eq!(order.recent.len(), 512);
        assert_eq!(order.priority(&1), 0);
    }

    #[test]
    fn matching_advertisement_survives_missing_cached_service_properties() {
        let mut scan = FreshScan::new([1]);
        scan.observe(1, Evidence::DeviceDiscovered);
        assert_eq!(scan.rank(&1, false), None); // cached synthetic sighting
        scan.observe(1, Evidence::RadioUpdate);
        assert_eq!(scan.rank(&1, false), None); // fresh, but wrong/absent service
        scan.observe(1, Evidence::MatchingService);
        assert_eq!(scan.rank(&1, false), Some(1));
        scan.observe(1, Evidence::RadioUpdate);
        assert_eq!(scan.rank(&1, false), Some(1)); // retain within this window
        scan.reject(1);
        assert_eq!(scan.rank(&1, false), None);
    }

    #[test]
    fn advertisement_service_evidence_is_window_scoped_and_bounded() {
        let mut scan = FreshScan::new([]);
        for id in 0..1024 {
            scan.observe(id, Evidence::MatchingService);
        }
        assert_eq!(scan.counts(), (512, 512));
        assert_eq!(scan.rank(&1023, true), None);
        let mut next = FreshScan::new([1]);
        next.observe(1, Evidence::RadioUpdate);
        assert_eq!(next.rank(&1, false), None);
    }

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
    fn returning_candidate_needs_fresh_radio_evidence_in_a_new_window() {
        let mut first = FreshScan::new([]);
        first.observe(1, Evidence::DeviceDiscovered);
        first.reject(1);
        assert_eq!(first.rank(&1, true), None);
        let mut next = FreshScan::new([1]);
        next.observe(1, Evidence::DeviceDiscovered);
        assert_eq!(next.rank(&1, true), None); // cached synthetic event
        next.observe(1, Evidence::RadioUpdate);
        assert_eq!(next.rank(&1, true), Some(1));
        assert_eq!(next.rank(&1, false), None); // still requires service match
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
