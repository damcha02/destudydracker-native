//! A bounded cache of server images by URL (Stage 22a: Skribbl drawings, friend avatars).
//!
//! Policy: at most `capacity` entries; when full, the least recently used *settled* entry
//! (loaded or failed) is evicted - a request in flight is never evicted. Decoded pixels move to
//! the view once (`take_pixels`), so a picture is held once, not twice; the view drops its copy
//! for every key the cache no longer has (`keys`). A failed load is remembered (production shows
//! the broken state rather than retrying in a loop) until evicted.

use std::collections::HashMap;

use crate::net::images::DecodedImage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageState {
    Loading,
    /// Decoded, pixels not yet handed to the view.
    Ready(DecodedImage),
    /// Pixels are with the view.
    Shown,
    Failed,
}

#[derive(Debug)]
struct Entry {
    state: ImageState,
    used: u64,
}

#[derive(Debug)]
pub struct ImageCache {
    entries: HashMap<String, Entry>,
    capacity: usize,
    clock: u64,
    pub evictions: u64,
}

impl ImageCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::new(),
            capacity: capacity.max(1),
            clock: 0,
            evictions: 0,
        }
    }

    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    pub fn state(&self, url: &str) -> Option<&ImageState> {
        self.entries.get(url).map(|e| &e.state)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Marks a URL as in use; returns true when it must be fetched (unknown so far).
    pub fn want(&mut self, url: &str) -> bool {
        let now = self.tick();
        if let Some(e) = self.entries.get_mut(url) {
            e.used = now;
            return false;
        }
        self.make_room();
        self.entries.insert(
            url.to_string(),
            Entry {
                state: ImageState::Loading,
                used: now,
            },
        );
        true
    }

    fn make_room(&mut self) {
        while self.entries.len() >= self.capacity {
            let victim = self
                .entries
                .iter()
                .filter(|(_, e)| e.state != ImageState::Loading)
                .min_by_key(|(_, e)| e.used)
                .map(|(k, _)| k.clone());
            match victim {
                Some(k) => {
                    self.entries.remove(&k);
                    self.evictions += 1;
                }
                None => break, // everything is in flight: allow a temporary overshoot
            }
        }
    }

    pub fn loaded(&mut self, url: &str, image: DecodedImage) {
        if let Some(e) = self.entries.get_mut(url) {
            e.state = ImageState::Ready(image);
        }
    }

    pub fn failed(&mut self, url: &str) {
        if let Some(e) = self.entries.get_mut(url) {
            e.state = ImageState::Failed;
        }
    }

    /// A cancelled fetch: forget it so it can be requested again later.
    pub fn forget(&mut self, url: &str) {
        if matches!(
            self.entries.get(url).map(|e| &e.state),
            Some(ImageState::Loading)
        ) {
            self.entries.remove(url);
        }
    }

    /// Hands decoded pixels to the view exactly once.
    pub fn take_pixels(&mut self, url: &str) -> Option<DecodedImage> {
        let e = self.entries.get_mut(url)?;
        match std::mem::replace(&mut e.state, ImageState::Shown) {
            ImageState::Ready(img) => Some(img),
            other => {
                e.state = other;
                None
            }
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }

    /// Decoded bytes still held here (not yet with the view).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn held_bytes(&self) -> usize {
        self.entries
            .values()
            .map(|e| match &e.state {
                ImageState::Ready(img) => img.byte_size(),
                _ => 0,
            })
            .sum()
    }

    pub fn clear_failed(&mut self) {
        self.entries.retain(|_, e| e.state != ImageState::Failed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img() -> DecodedImage {
        DecodedImage {
            width: 2,
            height: 2,
            rgba: vec![0; 16],
        }
    }

    #[test]
    fn bounded_lru_never_evicts_in_flight_requests() {
        let mut c = ImageCache::new(3);
        assert!(c.want("a"));
        assert!(!c.want("a"), "already known");
        assert!(c.want("b"));
        assert!(c.want("c"));
        // all three loading: a fourth overshoots rather than dropping an in-flight fetch
        assert!(c.want("d"));
        assert_eq!(c.len(), 4);
        c.loaded("a", img());
        c.loaded("b", img());
        c.failed("c");
        c.loaded("d", img());
        c.want("b"); // b is now the most recent
        assert!(c.want("e"));
        assert!(
            c.state("a").is_none(),
            "least recently used settled entry evicted"
        );
        assert!(c.len() <= 4);
        assert_eq!(c.take_pixels("b"), Some(img()));
        assert_eq!(c.take_pixels("b"), None, "pixels move once");
        assert_eq!(c.state("b"), Some(&ImageState::Shown));
        c.forget("e");
        assert!(c.state("e").is_none());
        c.forget("b");
        assert!(c.state("b").is_some(), "only loading entries are forgotten");
    }

    #[test]
    fn stress_keeps_the_cache_bounded() {
        let mut c = ImageCache::new(24);
        for i in 0..10_000 {
            let url = format!("u{i}");
            if c.want(&url) {
                c.loaded(&url, img());
                c.take_pixels(&url);
            }
        }
        assert!(c.len() <= 24);
        assert_eq!(c.held_bytes(), 0);
        assert!(c.evictions >= 10_000 - 24);
    }
}
