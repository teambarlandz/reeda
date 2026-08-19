//! M7d performance benchmarks for the PDF pipeline (PERFORMANCE.md §1, §5).
//!
//! Measures the two desktop-measurable PDF budgets on a synthetic 12-page
//! document:
//!   - first raster p95 < 250 ms @ fit-width-ish scale (PERFORMANCE.md §1)
//!   - cached raster (LRU blit) p95 < 8 ms (PERFORMANCE.md §1)
//!
//! Run with `cargo test --release -p reeda-pdf --test perf_bench -- --nocapture`
//! for realistic numbers; debug builds stay under generous smoke thresholds
//! so `cargo test` stays fast.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use pdfium_render::prelude::{PdfPagePaperSize, PdfPagePaperStandardSize};

use reeda_pdf::cache::{RasterCache, RasterKey, ScaleBucket};
use reeda_pdf::document::pdfium;
use reeda_pdf::render::render_page;
use reeda_pdf::theme::Theme;

const PAGE_COUNT: usize = 12;
/// Fit-width-ish scale on a typical phone-width window (device px per pt).
const SCALE: f32 = 2.0;

#[cfg(debug_assertions)]
mod profile {
    pub const FIRST_RASTER_SAMPLES: usize = 5;
    pub const CACHED_RASTER_SAMPLES: usize = 200;
    pub const MAX_FIRST_RASTER_P95_MS: u64 = 2_000;
    pub const MAX_CACHED_P95_MS: u64 = 50;
}

#[cfg(not(debug_assertions))]
mod profile {
    pub const FIRST_RASTER_SAMPLES: usize = 15;
    pub const CACHED_RASTER_SAMPLES: usize = 2_000;
    pub const MAX_FIRST_RASTER_P95_MS: u64 = 250;
    pub const MAX_CACHED_P95_MS: u64 = 8;
}

fn p95(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    let idx = (samples.len() as f64 * 0.95).ceil() as usize;
    samples[idx.min(samples.len() - 1)]
}

/// A synthetic multi-page PDF built through PDFium (no fixture files).
fn synthetic_pdf() -> PathBuf {
    let pdfium = pdfium().expect("PDFium must be loadable (PDFIUM_LIBRARY_PATH or DLL beside exe)");
    let mut doc = pdfium.create_new_pdf().expect("create PDF");
    {
        let pages = doc.pages_mut();
        for _ in 0..PAGE_COUNT {
            pages
                .create_page_at_end(PdfPagePaperSize::Portrait(PdfPagePaperStandardSize::A4))
                .expect("create page");
        }
    }
    let path = std::env::temp_dir().join(format!(
        "reeda-perf-{}-{}.pdf",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    doc.save_to_file(&path).expect("save PDF");
    path
}

#[test]
fn first_raster_and_cached_blit_p95() {
    let path = synthetic_pdf();
    let key = RasterKey {
        page: 0,
        scale: ScaleBucket::Percent200,
        theme: Theme::Normal,
    };

    // First-raster p95: fresh PDFium raster at scale 2.0 (fit-width-ish).
    let mut first_samples = Vec::with_capacity(profile::FIRST_RASTER_SAMPLES);
    for i in 0..profile::FIRST_RASTER_SAMPLES {
        let start = Instant::now();
        let page =
            render_page(&path, (i % PAGE_COUNT) as u32, SCALE, Theme::Normal).expect("render page");
        first_samples.push(start.elapsed());
        assert!(page.width > 0 && page.height > 0, "raster must have size");
    }
    let first_p95 = p95(first_samples);
    assert!(
        first_p95 < Duration::from_millis(profile::MAX_FIRST_RASTER_P95_MS),
        "first raster p95 too slow: {first_p95:?} (budget < {} ms)",
        profile::MAX_FIRST_RASTER_P95_MS
    );

    // Cached blit p95: LRU get after insert must not touch PDFium.
    let mut cache = RasterCache::new();
    let page = render_page(&path, 0, SCALE, Theme::Normal).expect("render page");
    cache.insert(key, page);
    let mut cached_samples = Vec::with_capacity(profile::CACHED_RASTER_SAMPLES);
    for _ in 0..profile::CACHED_RASTER_SAMPLES {
        let start = Instant::now();
        let cached = cache.get(&key).expect("cache hit");
        std::hint::black_box(cached.rgba.len());
        cached_samples.push(start.elapsed());
    }
    let cached_p95 = p95(cached_samples);
    assert!(
        cached_p95 < Duration::from_millis(profile::MAX_CACHED_P95_MS),
        "cached raster p95 too slow: {cached_p95:?} (budget < {} ms)",
        profile::MAX_CACHED_P95_MS
    );

    println!(
        "first raster p95: {first_p95:?} (budget < {} ms)\ncached blit p95: {cached_p95:?} (budget < {} ms)",
        profile::MAX_FIRST_RASTER_P95_MS, profile::MAX_CACHED_P95_MS
    );

    std::fs::remove_file(&path).ok();
}
