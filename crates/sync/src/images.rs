//! In-memory image cache keyed by URL, with a byte budget and least-recently-used eviction.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

pub(crate) const CACHE_BYTES: usize = 64 * 1024 * 1024;

pub(crate) struct Lru {
    cap: usize,
    total: usize,
    tick: u64,
    entries: HashMap<String, (Arc<[u8]>, u64)>,
    /// Use tick to URL. The first entry is the least recently used.
    order: BTreeMap<u64, String>,
}

impl Lru {
    pub fn new(cap: usize) -> Lru {
        Lru { cap, total: 0, tick: 0, entries: HashMap::new(), order: BTreeMap::new() }
    }

    pub fn get(&mut self, url: &str) -> Option<Arc<[u8]>> {
        self.tick += 1;
        let tick = self.tick;
        let (bytes, used) = self.entries.get_mut(url)?;
        self.order.remove(used);
        *used = tick;
        self.order.insert(tick, url.to_string());
        Some(bytes.clone())
    }

    /// Evicts old entries until the new one fits. An entry bigger than the budget is not kept.
    pub fn insert(&mut self, url: &str, bytes: Arc<[u8]>) {
        self.remove(url);
        if bytes.len() > self.cap {
            return;
        }
        while self.total + bytes.len() > self.cap {
            let Some((_, oldest)) = self.order.pop_first() else { break };
            if let Some((b, _)) = self.entries.remove(&oldest) {
                self.total -= b.len();
            }
        }
        self.tick += 1;
        self.total += bytes.len();
        self.order.insert(self.tick, url.to_string());
        self.entries.insert(url.to_string(), (bytes, self.tick));
    }

    fn remove(&mut self, url: &str) {
        if let Some((b, used)) = self.entries.remove(url) {
            self.order.remove(&used);
            self.total -= b.len();
        }
    }

    #[cfg(test)]
    pub fn total(&self) -> usize {
        self.total
    }
}

/// Cache plus the URLs that a pool thread fetches now.
pub(crate) struct ImageState {
    inner: Mutex<(Lru, HashSet<String>)>,
}

impl Default for ImageState {
    fn default() -> ImageState {
        ImageState { inner: Mutex::new((Lru::new(CACHE_BYTES), HashSet::new())) }
    }
}

/// What a pool thread does with an image request.
pub(crate) enum Claim {
    Cached(Arc<[u8]>),
    /// Another thread fetches it. Its event answers this request too.
    InFlight,
    Fetch,
}

impl ImageState {
    pub fn claim(&self, url: &str) -> Claim {
        let mut guard = self.lock();
        let (lru, in_flight) = &mut *guard;
        if let Some(bytes) = lru.get(url) {
            return Claim::Cached(bytes);
        }
        if in_flight.insert(url.to_string()) { Claim::Fetch } else { Claim::InFlight }
    }

    /// Ends a fetch. Keeps the bytes when it succeeded.
    pub fn finish(&self, url: &str, bytes: Option<Arc<[u8]>>) {
        let mut guard = self.lock();
        let (lru, in_flight) = &mut *guard;
        in_flight.remove(url);
        if let Some(b) = bytes {
            lru.insert(url, b);
        }
    }

    fn lock(&self) -> MutexGuard<'_, (Lru, HashSet<String>)> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(n: usize) -> Arc<[u8]> {
        vec![0u8; n].into()
    }

    #[test]
    fn evicts_least_recently_used() {
        let mut lru = Lru::new(10);
        lru.insert("a", bytes(4));
        lru.insert("b", bytes(4));
        assert!(lru.get("a").is_some());
        lru.insert("c", bytes(4));
        assert!(lru.get("b").is_none(), "b was the oldest");
        assert!(lru.get("a").is_some());
        assert!(lru.get("c").is_some());
        assert_eq!(lru.total(), 8);
    }

    #[test]
    fn big_entry_evicts_many_and_too_big_is_skipped() {
        let mut lru = Lru::new(10);
        lru.insert("a", bytes(3));
        lru.insert("b", bytes(3));
        lru.insert("c", bytes(9));
        assert!(lru.get("a").is_none());
        assert!(lru.get("b").is_none());
        assert_eq!(lru.total(), 9);
        lru.insert("d", bytes(11));
        assert!(lru.get("d").is_none());
        assert!(lru.get("c").is_some());
    }

    #[test]
    fn reinsert_replaces_size() {
        let mut lru = Lru::new(10);
        lru.insert("a", bytes(8));
        lru.insert("a", bytes(2));
        assert_eq!(lru.total(), 2);
        lru.insert("b", bytes(8));
        assert!(lru.get("a").is_some());
    }

    #[test]
    fn in_flight_requests_are_deduped() {
        let state = ImageState::default();
        assert!(matches!(state.claim("u"), Claim::Fetch));
        assert!(matches!(state.claim("u"), Claim::InFlight));
        state.finish("u", Some(bytes(3)));
        assert!(matches!(state.claim("u"), Claim::Cached(b) if b.len() == 3));
        assert!(matches!(state.claim("x"), Claim::Fetch));
        state.finish("x", None);
        assert!(matches!(state.claim("x"), Claim::Fetch));
    }
}
