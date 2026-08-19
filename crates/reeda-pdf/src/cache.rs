//! LRU raster cache for rendered pages (PDF_SPEC §5).
//!
//! Keyed by `(page, scale_bucket, theme)` with a configurable byte budget
//! (default 128 MB). Zoomed pages reuse the nearest bucket ≥ the requested
//! zoom, so the key only ever holds one of the predefined bucket values.

use std::collections::HashMap;

use crate::render::RenderedPage;
use crate::theme::Theme;

/// Default raster cache budget (PDF_SPEC §5).
pub const DEFAULT_BUDGET_BYTES: usize = 128 * 1024 * 1024;

/// Predefined zoom buckets for cache keys (PDF_SPEC §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScaleBucket {
    /// Fit-to-width: actual scale is viewport-dependent, computed by the UI.
    FitWidth,
    /// 100 % zoom.
    Percent100,
    /// 150 % zoom.
    Percent150,
    /// 200 % zoom.
    Percent200,
    /// 300 % zoom.
    Percent300,
    /// 400 % zoom.
    Percent400,
    /// 500 % zoom.
    Percent500,
}

impl ScaleBucket {
    /// The zoom multiplier this bucket represents (None for fit-to-width,
    /// whose scale depends on the viewport).
    pub fn zoom(self) -> Option<f32> {
        match self {
            Self::FitWidth => None,
            Self::Percent100 => Some(1.0),
            Self::Percent150 => Some(1.5),
            Self::Percent200 => Some(2.0),
            Self::Percent300 => Some(3.0),
            Self::Percent400 => Some(4.0),
            Self::Percent500 => Some(5.0),
        }
    }

    /// Nearest bucket at or above `zoom` (spec: "reuse nearest bucket ≥
    /// target" so zooming in never shows a blurry raster).
    ///
    /// Falls back to [`ScaleBucket::Percent500`] for zoom beyond the top
    /// bucket.
    pub fn bucket_for_zoom(zoom: f32) -> Self {
        if zoom <= 1.0 {
            Self::Percent100
        } else if zoom <= 1.5 {
            Self::Percent150
        } else if zoom <= 2.0 {
            Self::Percent200
        } else if zoom <= 3.0 {
            Self::Percent300
        } else if zoom <= 4.0 {
            Self::Percent400
        } else {
            Self::Percent500
        }
    }
}

/// Cache key: one rasterized page at one scale bucket in one theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RasterKey {
    /// Zero-based page index.
    pub page: u32,
    /// Scale bucket (zoom group) this raster was rendered at.
    pub scale: ScaleBucket,
    /// Theme filter applied at render time.
    pub theme: Theme,
}

#[derive(Debug)]
struct Entry {
    /// Monotonic access order; eviction removes the smallest.
    order: u64,
    page: RenderedPage,
}

/// LRU cache of rendered pages with a byte budget.
#[derive(Debug, Default)]
pub struct RasterCache {
    entries: HashMap<RasterKey, Entry>,
    budget_bytes: usize,
    used_bytes: usize,
    next_order: u64,
}

impl RasterCache {
    /// A cache with the default 128 MB budget.
    pub fn new() -> Self {
        Self::with_budget(DEFAULT_BUDGET_BYTES)
    }

    /// A cache with a custom byte budget.
    pub fn with_budget(budget_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            budget_bytes,
            used_bytes: 0,
            next_order: 0,
        }
    }

    /// Borrow a cached page, marking it most-recently-used.
    pub fn get(&mut self, key: &RasterKey) -> Option<&RenderedPage> {
        let entry = self.entries.get_mut(key)?;
        entry.order = self.next_order;
        self.next_order += 1;
        Some(&entry.page)
    }

    /// Insert a rendered page, evicting least-recently-used entries until
    /// the budget is satisfied. Pages larger than the budget are not cached.
    pub fn insert(&mut self, key: RasterKey, page: RenderedPage) {
        if page.size_bytes() > self.budget_bytes {
            return;
        }
        if let Some(prev) = self.entries.remove(&key) {
            self.used_bytes = self.used_bytes.saturating_sub(prev.page.size_bytes());
        }
        self.used_bytes += page.size_bytes();
        self.entries.insert(
            key,
            Entry {
                order: self.next_order,
                page,
            },
        );
        self.next_order += 1;
        self.evict_until(self.budget_bytes);
    }

    /// Evict least-recently-used entries until usage is at or below
    /// `fraction` of the budget (e.g. 0.5 on an OOM signal, PDF_SPEC §5).
    pub fn drop_to(&mut self, fraction: f32) {
        let target = (self.budget_bytes as f32 * fraction) as usize;
        self.evict_until(target);
    }

    fn evict_until(&mut self, target: usize) {
        while self.used_bytes > target {
            let Some(oldest_key) = self
                .entries
                .iter()
                .min_by_key(|(_, e)| e.order)
                .map(|(k, _)| *k)
            else {
                break;
            };
            if let Some(entry) = self.entries.remove(&oldest_key) {
                self.used_bytes = self.used_bytes.saturating_sub(entry.page.size_bytes());
            }
        }
    }

    /// Number of cached pages.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache holds no pages.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Total bytes held by cached pages.
    pub fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    /// The configured byte budget.
    pub fn budget_bytes(&self) -> usize {
        self.budget_bytes
    }

    /// Drop every cached page.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.used_bytes = 0;
        self.next_order = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(bytes: usize) -> RenderedPage {
        RenderedPage {
            width: 1,
            height: 1,
            rgba: vec![0u8; bytes],
        }
    }

    fn key(page: u32, scale: ScaleBucket) -> RasterKey {
        RasterKey {
            page,
            scale,
            theme: Theme::Normal,
        }
    }

    #[test]
    fn bucket_for_zoom_rounds_up_to_nearest_bucket() {
        assert_eq!(ScaleBucket::bucket_for_zoom(0.5), ScaleBucket::Percent100);
        assert_eq!(ScaleBucket::bucket_for_zoom(1.0), ScaleBucket::Percent100);
        assert_eq!(ScaleBucket::bucket_for_zoom(1.49), ScaleBucket::Percent150);
        assert_eq!(ScaleBucket::bucket_for_zoom(1.5), ScaleBucket::Percent150);
        assert_eq!(ScaleBucket::bucket_for_zoom(2.0), ScaleBucket::Percent200);
        assert_eq!(ScaleBucket::bucket_for_zoom(4.0), ScaleBucket::Percent400);
        assert_eq!(ScaleBucket::bucket_for_zoom(9.0), ScaleBucket::Percent500);
    }

    #[test]
    fn get_returns_cached_page_and_hit_orders_it() {
        let mut cache = RasterCache::with_budget(1024);
        assert!(cache.get(&key(0, ScaleBucket::Percent100)).is_none());

        cache.insert(key(0, ScaleBucket::Percent100), page(100));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.used_bytes(), 100);
        assert_eq!(
            cache
                .get(&key(0, ScaleBucket::Percent100))
                .unwrap()
                .rgba
                .len(),
            100
        );
    }

    #[test]
    fn theme_and_scale_partition_keys() {
        let mut cache = RasterCache::with_budget(1024);
        cache.insert(key(0, ScaleBucket::Percent100), page(100));
        cache.insert(
            RasterKey {
                page: 0,
                scale: ScaleBucket::Percent100,
                theme: Theme::Night,
            },
            page(100),
        );
        cache.insert(key(0, ScaleBucket::Percent200), page(100));
        assert_eq!(cache.len(), 3);
    }

    #[test]
    fn evicts_lru_first_when_over_budget() {
        let mut cache = RasterCache::with_budget(300);
        cache.insert(key(0, ScaleBucket::Percent100), page(100));
        cache.insert(key(1, ScaleBucket::Percent100), page(100));

        // Touch page 0 so page 1 becomes the LRU.
        cache.get(&key(0, ScaleBucket::Percent100));

        cache.insert(key(2, ScaleBucket::Percent100), page(200));
        assert!(
            cache.get(&key(1, ScaleBucket::Percent100)).is_none(),
            "LRU evicted"
        );
        assert!(
            cache.get(&key(0, ScaleBucket::Percent100)).is_some(),
            "recent survives"
        );
        assert!(cache.get(&key(2, ScaleBucket::Percent100)).is_some());
        assert!(cache.used_bytes() <= cache.budget_bytes());
    }

    #[test]
    fn replacing_same_key_does_not_grow_usage() {
        let mut cache = RasterCache::with_budget(1024);
        cache.insert(key(0, ScaleBucket::Percent100), page(100));
        cache.insert(key(0, ScaleBucket::Percent100), page(150));
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.used_bytes(), 150);
    }

    #[test]
    fn oversized_page_is_not_cached() {
        let mut cache = RasterCache::with_budget(100);
        cache.insert(key(0, ScaleBucket::Percent100), page(101));
        assert!(cache.is_empty());
    }

    #[test]
    fn drop_to_halves_budget_usage() {
        let mut cache = RasterCache::with_budget(1000);
        for i in 0..8 {
            cache.insert(key(i, ScaleBucket::Percent100), page(100));
        }
        assert_eq!(cache.used_bytes(), 800);
        cache.drop_to(0.5);
        // Evicted until at or below 50 % of the budget (500 bytes).
        assert!(cache.used_bytes() <= 500);
        assert!(
            cache.used_bytes() >= 400,
            "should have kept most-recent pages"
        );
        assert!(
            cache.get(&key(7, ScaleBucket::Percent100)).is_some(),
            "most-recent survives"
        );
    }

    #[test]
    fn clear_drops_everything() {
        let mut cache = RasterCache::with_budget(1024);
        cache.insert(key(0, ScaleBucket::Percent100), page(100));
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.used_bytes(), 0);
    }
}
