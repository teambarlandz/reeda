/// EPUB CFI (Canonical Fragment Identifier) position model.
///
/// CFI is the canonical locator for EPUB content positions.
/// Internally we use a normalized `Locator`; CFI strings are the
/// persistence format.
///
/// See [EPUB_SPEC.md section 7](../../docs/EPUB_SPEC.md).
use serde::{Deserialize, Serialize};

/// An internal document locator.
///
/// This is the internal representation; CFI strings are used for
/// persistence and interchange.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Locator {
    /// Spine index (0-based position in the reading order).
    pub spine_index: u32,
    /// Block index within the chapter (0-based paragraph/element index).
    pub block_index: u32,
    /// Character offset within the block.
    pub char_offset: u32,
}

impl Locator {
    /// Create a new locator.
    pub fn new(spine_index: u32, block_index: u32, char_offset: u32) -> Self {
        Self {
            spine_index,
            block_index,
            char_offset,
        }
    }
}

/// A CFI string — the canonical EPUB position format.
///
/// Example: `epubcfi(/6/4[chap03]!/4/2/1:42)`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cfi(pub String);

impl Cfi {
    /// Parse a CFI string into a `Locator`.
    ///
    /// Handles the common EPUB CFI format:
    /// `epubcfi(/6/<spine_index*2+4>[...]!/4/<block_index*2+2>:<char_offset>)`
    pub fn to_locator(&self, spine_length: u32) -> Option<Locator> {
        let s = &self.0;
        let s = s.strip_prefix("epubcfi(")?.strip_suffix(')')?;

        let parts: Vec<&str> = s.split('!').collect();
        if parts.is_empty() {
            return None;
        }

        // First part: /6/<spine_step>[...] — extract last numeric step.
        let spine_step = extract_last_numeric_step(parts[0])?;
        let spine_index = spine_step.saturating_sub(4) / 2;

        if spine_index >= spine_length {
            return None;
        }

        // Second part (after !): /4/<block_step>:<char_offset>
        if parts.len() < 2 {
            return Some(Locator::new(spine_index, 0, 0));
        }

        let second = parts[1];
        let (step_str, char_str) = if let Some(colon_pos) = second.rfind(':') {
            (&second[..colon_pos], &second[colon_pos + 1..])
        } else {
            (second, "0")
        };

        let block_step = extract_last_numeric_step(step_str)?;
        let block_index = block_step.saturating_sub(2) / 2;
        let char_offset: u32 = char_str.parse().ok()?;

        Some(Locator::new(spine_index, block_index, char_offset))
    }

    /// Create a CFI from a `Locator`.
    pub fn from_locator(loc: &Locator) -> Self {
        let spine_step = loc.spine_index * 2 + 4;
        let block_step = loc.block_index * 2 + 2;
        let s = format!(
            "epubcfi(/6/{spine_step}!/4/{block_step}:{})",
            loc.char_offset
        );
        Cfi(s)
    }

    /// Create a CFI range for highlights/annotations.
    pub fn range(start: &Locator, end: &Locator) -> CfiRange {
        CfiRange {
            start: Self::from_locator(start),
            end: Self::from_locator(end),
        }
    }
}

/// A CFI range — used for highlights and annotations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CfiRange {
    /// Start position.
    pub start: Cfi,
    /// End position (exclusive).
    pub end: Cfi,
}

/// Extract the last numeric step from a CFI path segment like `/6/8` -> 8.
fn extract_last_numeric_step(s: &str) -> Option<u32> {
    s.rsplit('/').find(|seg| !seg.is_empty()).and_then(|seg| {
        let seg = seg.trim_start_matches('0');
        if seg.is_empty() {
            Some(0)
        } else {
            seg.parse().ok()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cfi_round_trip() {
        let loc = Locator::new(2, 5, 42);
        let cfi = Cfi::from_locator(&loc);
        assert_eq!(cfi.0, "epubcfi(/6/8!/4/12:42)");

        let recovered = cfi.to_locator(10).unwrap();
        assert_eq!(recovered, loc);
    }

    #[test]
    fn cfi_first_chapter() {
        let loc = Locator::new(0, 0, 0);
        let cfi = Cfi::from_locator(&loc);
        assert_eq!(cfi.0, "epubcfi(/6/4!/4/2:0)");

        let recovered = cfi.to_locator(5).unwrap();
        assert_eq!(recovered, loc);
    }

    #[test]
    fn cfi_range() {
        let start = Locator::new(1, 3, 10);
        let end = Locator::new(1, 4, 5);
        let range = Cfi::range(&start, &end);
        assert_eq!(range.start.0, "epubcfi(/6/6!/4/8:10)");
        assert_eq!(range.end.0, "epubcfi(/6/6!/4/10:5)");
    }

    #[test]
    fn cfi_invalid_format() {
        let cfi = Cfi("not-a-cfi".into());
        assert!(cfi.to_locator(5).is_none());
    }

    #[test]
    fn cfi_out_of_range_spine() {
        let cfi = Cfi("epubcfi(/6/20!/4/2:0)".into());
        assert!(cfi.to_locator(3).is_none());
    }
}
