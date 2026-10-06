//! One-central host policy, independent of WinRT so the transient-subscription
//! behavior can be tested on Linux without a Windows radio.
use std::time::Duration;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubscriberState {
    Same,
    TemporarilyMissing,
    Conflicting,
}

pub fn within_disconnect_grace(elapsed: Duration) -> bool {
    elapsed < Duration::from_secs(2)
}

pub fn assess(selected: &str, count: u32, only: Option<&str>) -> SubscriberState {
    match count {
        // Windows can report an empty subscriber snapshot while it is updating
        // the CCCD. The regular link check handles a sustained disconnect.
        0 => SubscriberState::TemporarilyMissing,
        1 if only == Some(selected) => SubscriberState::Same,
        _ => SubscriberState::Conflicting,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transient_empty_snapshot_does_not_blacklist_a_recovered_central() {
        assert_eq!(assess("pc-a", 0, None), SubscriberState::TemporarilyMissing);
        assert_eq!(assess("pc-a", 1, Some("pc-a")), SubscriberState::Same);
    }

    #[test]
    fn only_a_sustained_missing_snapshot_ends_chat() {
        assert!(within_disconnect_grace(Duration::ZERO));
        assert!(within_disconnect_grace(Duration::from_millis(1999)));
        assert!(!within_disconnect_grace(Duration::from_secs(2)));
    }

    #[test]
    fn different_or_multiple_centrals_are_not_selected_implicitly() {
        assert_eq!(
            assess("pc-a", 1, Some("pc-b")),
            SubscriberState::Conflicting
        );
        assert_eq!(assess("pc-a", 2, None), SubscriberState::Conflicting);
        assert_eq!(assess("pc-a", 1, None), SubscriberState::Conflicting);
    }
}
