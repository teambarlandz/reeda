use serde::{Deserialize, Serialize};

use super::BookId;

/// A single search hit, ready for UI display (SEARCH_SPEC SEA-02).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHitView {
    /// The book containing the match.
    pub book_id: BookId,
    /// Book title (library metadata).
    pub book_title: String,
    /// Chapter title of the match.
    pub chapter_title: String,
    /// Snippet with `<mark>`-wrapped matches (HTML-escaped).
    pub snippet: String,
    /// CFI string of the first match (open-at-match).
    pub cfi: String,
    /// Global block index of the match.
    pub block_index: u32,
    /// Character offset of the match within the block.
    pub char_offset: u32,
    /// Length in chars of the first matching term (transient highlight).
    pub term_len: u32,
}

/// Search results summary for the UI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchResultsView {
    /// Ranked hits.
    pub hits: Vec<SearchHitView>,
    /// Total matching documents (uncapped).
    pub total: usize,
}
