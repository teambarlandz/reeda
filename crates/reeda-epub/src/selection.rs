/// Text selection & intersection engine.
///
/// Converts text ranges (block + char offsets over the global block
/// sequence) into CFI ranges for persistence, and intersects them with
/// pages so the reader can render only the visible parts of a highlight.
///
/// Conventions (matching `paginator` and `cfi_of_page_start`):
/// - `block_index` is a **global** block index across the whole document
///   (chapters concatenated in spine order), NOT chapter-local.
/// - CFI strings encode `Locator { spine_index: 0, block_index: global,
///   char_offset }` — an approximation documented in EPUB_SPEC.md §7;
///   identical inputs always produce identical CFI and vice versa.
///
/// See [HIGHLIGHTS_SPEC.md section 6](../../docs/HIGHLIGHTS_SPEC.md).
use crate::cfi::{Cfi, CfiRange, Locator};
use crate::paginator::Page;

/// A text range over the global block sequence.
///
/// `char_start`/`char_end` are offsets **within** `block_start`/`block_end`
/// (end is exclusive). Valid ranges satisfy `block_start <= block_end` and,
/// when the range is a single block, `char_start < char_end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalRange {
    /// Global index of the first block in the range.
    pub block_start: usize,
    /// Character offset (inclusive) within `block_start`.
    pub char_start: usize,
    /// Global index of the last block in the range.
    pub block_end: usize,
    /// Character offset (exclusive) within `block_end`.
    pub char_end: usize,
}

impl GlobalRange {
    /// Create a new range.
    pub fn new(block_start: usize, char_start: usize, block_end: usize, char_end: usize) -> Self {
        Self {
            block_start,
            char_start,
            block_end,
            char_end,
        }
    }

    /// Whether the range is structurally valid.
    pub fn is_valid(&self) -> bool {
        if self.block_start > self.block_end {
            return false;
        }
        if self.block_start == self.block_end {
            self.char_start < self.char_end
        } else {
            true
        }
    }

    /// Serialize this range to a CFI range for persistence.
    pub fn to_cfi(&self) -> CfiRange {
        Cfi::range(
            &Locator::new(0, self.block_start as u32, self.char_start as u32),
            &Locator::new(0, self.block_end as u32, self.char_end as u32),
        )
    }

    /// Parse a CFI range back into a `GlobalRange`.
    ///
    /// Returns `None` if either end cannot be parsed (e.g. orphaned CFI
    /// from a re-parsed book).
    pub fn from_cfi(range: &CfiRange, spine_length: u32) -> Option<GlobalRange> {
        let start = range.start.to_locator(spine_length)?;
        let end = range.end.to_locator(spine_length)?;
        Some(GlobalRange::new(
            start.block_index as usize,
            start.char_offset as usize,
            end.block_index as usize,
            end.char_offset as usize,
        ))
    }
}

/// A clipped piece of a highlight that is visible on a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClippedSegment {
    /// Global block index this segment belongs to.
    pub block_index: usize,
    /// Inclusive character offset within the block.
    pub char_start: usize,
    /// Exclusive character offset within the block.
    pub char_end: usize,
}

/// Intersect a range with a page, returning the visible segments.
///
/// Segments are clipped to the page's `first_block`..`last_block` and its
/// `first_char`/`last_char` page-boundaries. Returns `None` when the range
/// and page do not overlap at all.
pub fn intersect_range_with_page(range: &GlobalRange, page: &Page) -> Option<Vec<ClippedSegment>> {
    // No overlap at block level.
    if range.block_end < page.first_block || range.block_start > page.last_block {
        return None;
    }

    let first = range.block_start.max(page.first_block);
    let last = range.block_end.min(page.last_block);

    let mut segments = Vec::new();
    for block in first..=last {
        let seg_start = if block == range.block_start {
            range.char_start
        } else {
            0
        };
        let seg_end = if block == range.block_end {
            range.char_end
        } else {
            usize::MAX
        };

        // Clip to the page's visible char range.
        let start = if block == page.first_block {
            seg_start.max(page.first_char as usize)
        } else {
            seg_start
        };
        let end = if block == page.last_block {
            seg_end.min(page.last_char as usize)
        } else {
            seg_end
        };

        if start < end {
            segments.push(ClippedSegment {
                block_index: block,
                char_start: start,
                char_end: end,
            });
        }
    }

    if segments.is_empty() {
        None
    } else {
        Some(segments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_validity() {
        assert!(GlobalRange::new(2, 5, 2, 9).is_valid());
        assert!(GlobalRange::new(2, 5, 3, 0).is_valid());
        assert!(!GlobalRange::new(3, 5, 2, 9).is_valid());
        assert!(!GlobalRange::new(2, 9, 2, 9).is_valid());
    }

    #[test]
    fn cfi_round_trip() {
        let range = GlobalRange::new(4, 12, 7, 3);
        let cfi = range.to_cfi();
        let back = GlobalRange::from_cfi(&cfi, 10).unwrap();
        assert_eq!(back, range);
        assert_eq!(cfi.start.0, "epubcfi(/6/4!/4/10:12)");
        assert_eq!(cfi.end.0, "epubcfi(/6/4!/4/16:3)");
    }

    #[test]
    fn cfi_round_trip_single_block() {
        let range = GlobalRange::new(0, 0, 0, 5);
        let cfi = range.to_cfi();
        let back = GlobalRange::from_cfi(&cfi, 3).unwrap();
        assert_eq!(back, range);
    }

    #[test]
    fn orphaned_cfi_returns_none() {
        let cfi = CfiRange {
            start: Cfi("epubcfi(/6/20!/4/2:0)".into()),
            end: Cfi("epubcfi(/6/20!/4/2:9)".into()),
        };
        assert!(GlobalRange::from_cfi(&cfi, 3).is_none());
    }

    #[test]
    fn intersect_fully_contained_in_page() {
        let range = GlobalRange::new(2, 5, 2, 10);
        let page = Page {
            first_block: 1,
            first_char: 0,
            last_block: 3,
            last_char: 50,
            progress: 0.5,
        };
        let segs = intersect_range_with_page(&range, &page).unwrap();
        assert_eq!(
            segs,
            vec![ClippedSegment {
                block_index: 2,
                char_start: 5,
                char_end: 10,
            }]
        );
    }

    #[test]
    fn intersect_clipped_to_page_start() {
        let range = GlobalRange::new(2, 0, 2, 40);
        let page = Page {
            first_block: 2,
            first_char: 10,
            last_block: 2,
            last_char: 30,
            progress: 0.5,
        };
        let segs = intersect_range_with_page(&range, &page).unwrap();
        assert_eq!(
            segs,
            vec![ClippedSegment {
                block_index: 2,
                char_start: 10,
                char_end: 30,
            }]
        );
    }

    #[test]
    fn intersect_page_starts_mid_range() {
        let range = GlobalRange::new(2, 0, 4, 10);
        let page = Page {
            first_block: 2,
            first_char: 20,
            last_block: 3,
            last_char: 5,
            progress: 0.5,
        };
        let segs = intersect_range_with_page(&range, &page).unwrap();
        assert_eq!(segs.len(), 2);
        assert_eq!(
            segs[0],
            ClippedSegment {
                block_index: 2,
                char_start: 20,
                char_end: usize::MAX,
            }
        );
        assert_eq!(
            segs[1],
            ClippedSegment {
                block_index: 3,
                char_start: 0,
                char_end: 5,
            }
        );
    }

    #[test]
    fn intersect_multiple_full_blocks() {
        let range = GlobalRange::new(1, 0, 5, 10);
        let page = Page {
            first_block: 1,
            first_char: 0,
            last_block: 5,
            last_char: 10,
            progress: 0.5,
        };
        let segs = intersect_range_with_page(&range, &page).unwrap();
        assert_eq!(segs.len(), 5);
        assert_eq!(segs[0].block_index, 1);
        assert_eq!(segs[4].block_index, 5);
        assert_eq!(segs[4].char_end, 10);
    }

    #[test]
    fn intersect_range_ending_at_block_boundary() {
        // char_end == 0 on the last block → that block contributes nothing.
        let range = GlobalRange::new(1, 0, 5, 0);
        let page = Page {
            first_block: 1,
            first_char: 0,
            last_block: 5,
            last_char: 0,
            progress: 0.5,
        };
        let segs = intersect_range_with_page(&range, &page).unwrap();
        assert_eq!(segs.len(), 4);
        assert_eq!(segs.last().unwrap().block_index, 4);
    }

    #[test]
    fn intersect_no_overlap() {
        let range = GlobalRange::new(10, 0, 10, 5);
        let page = Page {
            first_block: 0,
            first_char: 0,
            last_block: 5,
            last_char: 20,
            progress: 0.1,
        };
        assert!(intersect_range_with_page(&range, &page).is_none());
    }

    #[test]
    fn intersect_range_before_page() {
        let range = GlobalRange::new(0, 0, 0, 5);
        let page = Page {
            first_block: 3,
            first_char: 0,
            last_block: 4,
            last_char: 10,
            progress: 0.5,
        };
        assert!(intersect_range_with_page(&range, &page).is_none());
    }

    #[test]
    fn intersect_fully_clipped_away() {
        let range = GlobalRange::new(2, 0, 2, 5);
        let page = Page {
            first_block: 2,
            first_char: 6,
            last_block: 2,
            last_char: 10,
            progress: 0.5,
        };
        assert!(intersect_range_with_page(&range, &page).is_none());
    }
}
