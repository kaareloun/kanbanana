//! Rate limit gate. After Linear says "rate limited", no call goes to that workspace
//! until the reset time.

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};

pub(crate) const TIMER: Duration = Duration::from_secs(30);
pub(crate) const SLOW_TIMER: Duration = Duration::from_secs(120);
/// Below this many requests left in the hour, the timer slows down.
const LOW_REMAINING: i64 = 100;

#[derive(Default)]
pub(crate) struct Gate {
    until: HashMap<String, DateTime<Utc>>,
}

impl Gate {
    /// Closes the gate for `retry_after_secs`. A later close time wins.
    pub fn block(&mut self, ws: &str, now: DateTime<Utc>, retry_after_secs: u64) -> DateTime<Utc> {
        let secs = i64::try_from(retry_after_secs).unwrap_or(i64::MAX).min(24 * 3600);
        let until = now + TimeDelta::seconds(secs);
        let entry = self.until.entry(ws.to_string()).or_insert(until);
        if until > *entry {
            *entry = until;
        }
        *entry
    }

    /// The reset time while the gate is closed, else None.
    pub fn blocked_until(&self, ws: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        self.until.get(ws).copied().filter(|until| now < *until)
    }

    pub fn remove(&mut self, ws: &str) {
        self.until.remove(ws);
    }
}

/// Timer interval for the active workspace, from Linear's last remaining-requests header.
pub(crate) fn timer_interval(remaining: Option<i64>) -> Duration {
    match remaining {
        Some(n) if n < LOW_REMAINING => SLOW_TIMER,
        _ => TIMER,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(1_760_000_000 + secs, 0).unwrap()
    }

    #[test]
    fn gate_closes_until_reset_then_opens() {
        let mut g = Gate::default();
        assert_eq!(g.blocked_until("a", t(0)), None);
        assert_eq!(g.block("a", t(0), 60), t(60));
        assert_eq!(g.blocked_until("a", t(0)), Some(t(60)));
        assert_eq!(g.blocked_until("a", t(59)), Some(t(60)));
        assert_eq!(g.blocked_until("a", t(60)), None);
        assert_eq!(g.blocked_until("b", t(10)), None);
    }

    #[test]
    fn later_reset_wins() {
        let mut g = Gate::default();
        g.block("a", t(0), 60);
        assert_eq!(g.block("a", t(10), 10), t(60));
        assert_eq!(g.block("a", t(10), 100), t(110));
        g.remove("a");
        assert_eq!(g.blocked_until("a", t(20)), None);
    }

    #[test]
    fn huge_retry_is_capped() {
        let mut g = Gate::default();
        assert_eq!(g.block("a", t(0), u64::MAX), t(24 * 3600));
    }

    #[test]
    fn timer_slows_when_few_requests_left() {
        assert_eq!(timer_interval(None), TIMER);
        assert_eq!(timer_interval(Some(1500)), TIMER);
        assert_eq!(timer_interval(Some(100)), TIMER);
        assert_eq!(timer_interval(Some(99)), SLOW_TIMER);
    }
}
