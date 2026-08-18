#[cfg(feature = "platform-android")]
mod android;

mod theme;

slint::include_modules!();

fn main() {
    let app = AppRoot::new().unwrap();

    // ── Create core App ─────────────────────────────────────────────
    let mut core = reeda_core::App::new();

    // Persistent storage + full-text search index (desktop: ./reeda_data).
    // Books imported this session are searchable immediately; re-indexing
    // of books loaded from storage is a P2 follow-up (see TODO M4.7).
    if let Ok(store) = reeda_core::BookStore::new("reeda_data") {
        if let Ok(db) = reeda_core::Database::open(store.root().join("reeda.sqlite")) {
            core.set_db(db);
        }
        if let Ok(search) = reeda_core::search::SearchService::open(store.root()) {
            core.set_search(search);
        }
        core.set_store(store);
    }

    // If a file path is provided as CLI arg, import it and open.
    let args: Vec<String> = std::env::args().collect();
    if let Some(path) = args.get(1) {
        let epub_data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Failed to read {path}: {e}");
                return;
            }
        };
        let events = core.import_from_bytes(epub_data, path.clone());
        if let Some(reeda_core::Event::ImportFinished { book_id }) = events.first() {
            let _ = core.dispatch(reeda_core::Command::OpenBook { book_id: *book_id });
        }
    }

    // Push initial state into the Slint UI.
    let snap = core.snapshot();
    update_ui(&app, &snap);

    // ── Wire callbacks ──────────────────────────────────────────────
    let weak = app.as_weak();
    let core_cell = std::rc::Rc::new(std::cell::RefCell::new(core));

    app.on_next_page({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move || {
            {
                let mut core = core_cell.borrow_mut();
                core.dispatch(reeda_core::Command::TurnPage { forward: true });
            }
            let app = weak.unwrap();
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    app.on_prev_page({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move || {
            {
                let mut core = core_cell.borrow_mut();
                core.dispatch(reeda_core::Command::TurnPage { forward: false });
            }
            let app = weak.unwrap();
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    app.on_back_pressed({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move || {
            {
                let mut core = core_cell.borrow_mut();
                core.dispatch(reeda_core::Command::CloseBook);
            }
            let app = weak.unwrap();
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    // ── Metadata edit dialog ────────────────────────────────────────
    app.on_metadata_requested({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move |book_id: slint::SharedString| {
            let app = weak.unwrap();
            let core = core_cell.borrow();
            let id = book_id.to_string();
            if let Some(book) = core
                .snapshot()
                .library
                .iter()
                .find(|b| b.id.to_string() == id)
            {
                app.set_metadata_title(slint::SharedString::from(&book.title));
                app.set_metadata_author(slint::SharedString::from(
                    book.author.as_deref().unwrap_or(""),
                ));
                app.set_metadata_open(true);
            }
        }
    });

    app.on_metadata_saved({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move |title: slint::SharedString, author: slint::SharedString| {
            let app = weak.unwrap();
            let id = app.get_metadata_book_id().to_string();
            {
                let mut core = core_cell.borrow_mut();
                if let Ok(book_id) = reeda_core::BookId::try_from(id.as_str()) {
                    core.dispatch(reeda_core::Command::EditMetadata {
                        book_id,
                        title: title.to_string(),
                        author: Some(author.to_string()),
                    });
                }
            }
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    app.on_metadata_cancelled({
        let weak = weak.clone();
        move || {
            let app = weak.unwrap();
            app.set_metadata_open(false);
        }
    });

    // ── Settings callbacks ──────────────────────────────────────────
    app.on_settings_back({
        let weak = weak.clone();
        move || {
            let app = weak.unwrap();
            app.set_show_settings(false);
            app.set_show_library(true);
        }
    });

    app.on_theme_selected({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move |index: i32| {
            let theme = match index {
                0 => reeda_core::Theme::Light,
                1 => reeda_core::Theme::Sepia,
                _ => reeda_core::Theme::Dark,
            };
            {
                let mut core = core_cell.borrow_mut();
                core.dispatch(reeda_core::Command::SetTheme(theme));
            }
            let app = weak.unwrap();
            theme::apply_theme(&app, theme);
            app.set_settings_theme_index(index);
        }
    });

    app.on_font_size_changed({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move |delta: i32| {
            let app = weak.unwrap();
            let current = app.get_settings_font_size();
            let new_size = (current + delta).clamp(12, 32);
            if new_size != current {
                {
                    let mut core = core_cell.borrow_mut();
                    let mut settings = core.settings();
                    settings.typography.font_size_pt = new_size as f32;
                    core.dispatch(reeda_core::Command::UpdateSettings(settings));
                }
                app.set_settings_font_size(new_size);
                let snap = core_cell.borrow().snapshot();
                update_ui(&app, &snap);
            }
        }
    });

    app.on_line_height_changed({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move |delta: i32| {
            let app = weak.unwrap();
            let current = app.get_settings_line_height();
            let new_value = (current + delta).clamp(10, 30);
            if new_value != current {
                {
                    let mut core = core_cell.borrow_mut();
                    let mut settings = core.settings();
                    settings.typography.line_height = new_value as f32 / 10.0;
                    core.dispatch(reeda_core::Command::UpdateSettings(settings));
                }
                app.set_settings_line_height(new_value);
                let snap = core_cell.borrow().snapshot();
                update_ui(&app, &snap);
            }
        }
    });

    // ── Highlight editing ───────────────────────────────────────────
    let selected_highlight: std::rc::Rc<std::cell::RefCell<Option<String>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));

    app.on_highlight_tapped({
        let weak = weak.clone();
        let selected = selected_highlight.clone();
        move |id: slint::SharedString| {
            *selected.borrow_mut() = Some(id.to_string());
            let app = weak.unwrap();
            app.set_reader_edit_popover_visible(true);
        }
    });

    app.on_edit_color({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        let selected = selected_highlight.clone();
        move |index: i32| {
            let color = match index {
                0 => reeda_core::HighlightColor::Yellow,
                1 => reeda_core::HighlightColor::Green,
                2 => reeda_core::HighlightColor::Blue,
                _ => reeda_core::HighlightColor::Pink,
            };
            if let Some(id) = selected.borrow().clone() {
                if let Ok(annotation_id) = id.parse() {
                    let mut core = core_cell.borrow_mut();
                    core.dispatch(reeda_core::Command::EditHighlight {
                        annotation_id,
                        color: Some(color),
                    });
                }
            }
            let app = weak.unwrap();
            app.set_reader_edit_popover_visible(false);
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    app.on_delete_highlight({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        let selected = selected_highlight.clone();
        move || {
            if let Some(id) = selected.borrow().clone() {
                if let Ok(annotation_id) = id.parse() {
                    let mut core = core_cell.borrow_mut();
                    core.dispatch(reeda_core::Command::DeleteAnnotation { annotation_id });
                }
            }
            let app = weak.unwrap();
            app.set_reader_edit_popover_visible(false);
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    // ── Notes ────────────────────────────────────────────────────────
    app.on_note_requested({
        let app = app.as_weak();
        let core_cell = core_cell.clone();
        let selected = selected_highlight.clone();
        move || {
            let existing_note = {
                let core = core_cell.borrow();
                selected
                    .borrow()
                    .clone()
                    .and_then(|id| id.parse::<reeda_core::AnnotationId>().ok())
                    .and_then(|aid| {
                        core.snapshot()
                            .annotations
                            .iter()
                            .find(|a| a.id == aid)
                            .and_then(|a| a.text.clone())
                    })
                    .unwrap_or_default()
            };
            let app = app.unwrap();
            app.set_reader_edit_popover_visible(false);
            app.set_note_dialog_text(slint::SharedString::from(existing_note));
            app.set_note_dialog_open(true);
        }
    });

    app.on_note_saved({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        let selected = selected_highlight.clone();
        move |text: slint::SharedString| {
            let id = selected.borrow().clone();
            let text = text.trim().to_string();
            {
                let mut core = core_cell.borrow_mut();
                let annotation_id = id.and_then(|s| s.parse().ok());
                core.dispatch(reeda_core::Command::AddNote {
                    annotation_id,
                    text,
                });
            }
            let app = weak.unwrap();
            app.set_note_dialog_open(false);
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    app.on_notes_requested({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move || {
            let app = weak.unwrap();
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    app.on_notes_jump({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move |id: slint::SharedString| {
            if let Ok(annotation_id) = id.to_string().parse() {
                let mut core = core_cell.borrow_mut();
                core.dispatch(reeda_core::Command::JumpToAnnotation { annotation_id });
            }
            let app = weak.unwrap();
            app.set_show_notes(false);
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    // ── Bookmarks ───────────────────────────────────────────────────
    app.on_toggle_bookmark({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move || {
            let cfi = core_cell.borrow().snapshot().page_start_cfi;
            if !cfi.is_empty() {
                let mut core = core_cell.borrow_mut();
                core.dispatch(reeda_core::Command::ToggleBookmark { cfi });
            }
            let app = weak.unwrap();
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    app.on_bookmarks_requested({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move || {
            let app = weak.unwrap();
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    app.on_bookmarks_jump({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move |id: slint::SharedString| {
            if let Ok(annotation_id) = id.to_string().parse() {
                let mut core = core_cell.borrow_mut();
                core.dispatch(reeda_core::Command::JumpToAnnotation { annotation_id });
            }
            let app = weak.unwrap();
            app.set_show_bookmarks(false);
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    app.on_bookmark_delete({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move |id: slint::SharedString| {
            if let Ok(annotation_id) = id.to_string().parse() {
                let mut core = core_cell.borrow_mut();
                core.dispatch(reeda_core::Command::DeleteAnnotation { annotation_id });
            }
            let app = weak.unwrap();
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    // ── Export ───────────────────────────────────────────────────────
    app.on_notes_export({
        let core_cell = core_cell.clone();
        move || {
            let (markdown, file_path) = {
                let core = core_cell.borrow();
                let snap = core.snapshot();
                let Some(book) = snap.current_book.as_ref() else {
                    return;
                };
                let Some(md) = core.export_annotations_markdown(book.id) else {
                    return;
                };
                (md, book.file_path.clone())
            };
            let dir = std::path::Path::new(&file_path)
                .parent()
                .unwrap_or(std::path::Path::new("books"));
            let out_path = dir.join("annotations.md");
            match std::fs::write(&out_path, markdown) {
                Ok(()) => eprintln!("Exported highlights & notes to {}", out_path.display()),
                Err(e) => eprintln!("Export failed: {e}"),
            }
        }
    });

    // ── Search ───────────────────────────────────────────────────────
    app.on_search_requested({
        let weak = weak.clone();
        move || {
            let app = weak.unwrap();
            app.set_search_query(slint::SharedString::from(""));
            app.set_search_has_query(false);
            app.set_search_hits(slint::ModelRc::from(Vec::<SearchHit>::new().as_slice()));
        }
    });

    app.on_search_back({
        let weak = weak.clone();
        move || {
            let app = weak.unwrap();
            app.set_search_query(slint::SharedString::from(""));
            app.set_search_has_query(false);
            app.set_search_hits(slint::ModelRc::from(Vec::<SearchHit>::new().as_slice()));
        }
    });

    // Debounce: only dispatch the most recent query after 300ms of inactivity.
    let pending_query: std::rc::Rc<std::cell::RefCell<Option<String>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));

    app.on_search_query_changed({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        let pending = pending_query.clone();
        move |query: slint::SharedString| {
            let query = query.to_string();
            *pending.borrow_mut() = Some(query.clone());
            let weak = weak.clone();
            let core_cell = core_cell.clone();
            let pending = pending.clone();
            slint::Timer::single_shot(std::time::Duration::from_millis(300), move || {
                // Skip if a newer query was typed meanwhile.
                if pending.borrow().as_deref() != Some(query.as_str()) {
                    return;
                }
                {
                    let mut core = core_cell.borrow_mut();
                    core.dispatch(reeda_core::Command::Search { query });
                }
                let app = weak.unwrap();
                let snap = core_cell.borrow().snapshot();
                update_ui(&app, &snap);
            });
        }
    });

    app.on_search_hit_opened({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move |book_id: slint::SharedString,
              cfi: slint::SharedString,
              block_index: i32,
              char_offset: i32,
              term_len: i32| {
            if let Ok(book_id) = book_id.to_string().parse() {
                let mut core = core_cell.borrow_mut();
                core.dispatch(reeda_core::Command::OpenSearchHit {
                    book_id,
                    cfi: cfi.to_string(),
                    block_index: block_index as u32,
                    char_offset: char_offset as u32,
                    term_len: term_len as u32,
                });
            }
            let app = weak.unwrap();
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    // ── In-reader search ──────────────────────────────────────────────
    let reader_pending_query: std::rc::Rc<std::cell::RefCell<Option<String>>> =
        std::rc::Rc::new(std::cell::RefCell::new(None));

    app.on_reader_search_toggled({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move || {
            let app = weak.unwrap();
            app.set_reader_search_query(slint::SharedString::from(""));
            app.set_reader_search_count(slint::SharedString::from(""));
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    app.on_reader_search_query_changed({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        let pending = reader_pending_query.clone();
        move |query: slint::SharedString| {
            let query = query.to_string();
            *pending.borrow_mut() = Some(query.clone());
            let weak = weak.clone();
            let core_cell = core_cell.clone();
            let pending = pending.clone();
            slint::Timer::single_shot(std::time::Duration::from_millis(300), move || {
                if pending.borrow().as_deref() != Some(query.as_str()) {
                    return;
                }
                {
                    let mut core = core_cell.borrow_mut();
                    core.dispatch(reeda_core::Command::ReaderSearch { query });
                }
                let app = weak.unwrap();
                let snap = core_cell.borrow().snapshot();
                update_ui(&app, &snap);
            });
        }
    });

    app.on_reader_search_next({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move || {
            {
                let mut core = core_cell.borrow_mut();
                core.dispatch(reeda_core::Command::ReaderSearchNext);
            }
            let app = weak.unwrap();
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    app.on_reader_search_prev({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move || {
            {
                let mut core = core_cell.borrow_mut();
                core.dispatch(reeda_core::Command::ReaderSearchPrev);
            }
            let app = weak.unwrap();
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    app.on_reader_search_close({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move || {
            {
                let mut core = core_cell.borrow_mut();
                core.dispatch(reeda_core::Command::ReaderSearchClose);
            }
            let app = weak.unwrap();
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    // Apply the default theme.
    theme::apply_theme(&app, reeda_core::Theme::Light);

    app.run().unwrap();
}

/// Push a `StateSnapshot` into the Slint UI properties.
fn update_ui(app: &AppRoot, snap: &reeda_core::StateSnapshot) {
    let notes_open = app.get_show_notes();
    let search_open = app.get_show_search();

    if let Some(ref book) = snap.current_book {
        if !notes_open && !search_open {
            app.set_show_library(false);
            app.set_show_reader(true);
        }
        app.set_book_title(slint::SharedString::from(&book.title));
        app.set_page_text(slint::SharedString::from(&snap.page_text));
        app.set_reader_page_font_size(snap.settings.typography.font_size_pt);
        app.set_reader_page_line_height(snap.settings.typography.line_height);

        let lines_model: Vec<slint::ModelRc<LineRun>> = snap
            .page_lines
            .iter()
            .map(|line| {
                let runs: Vec<LineRun> = line
                    .iter()
                    .map(|r| {
                        let color_index = match r.color {
                            Some(reeda_core::HighlightColor::Yellow) => 0,
                            Some(reeda_core::HighlightColor::Green) => 1,
                            Some(reeda_core::HighlightColor::Blue) => 2,
                            Some(reeda_core::HighlightColor::Pink) => 3,
                            Some(reeda_core::HighlightColor::Cyan) => 4,
                            None => 0,
                        };
                        LineRun {
                            text: slint::SharedString::from(&r.text),
                            highlighted: r.highlighted,
                            color_index,
                            has_note: r.has_note,
                            annotation_id: slint::SharedString::from(
                                r.annotation_id.as_deref().unwrap_or(""),
                            ),
                        }
                    })
                    .collect();
                slint::ModelRc::from(runs.as_slice())
            })
            .collect();
        app.set_reader_page_lines(slint::ModelRc::from(lines_model.as_slice()));

        let progress = if snap.total_pages > 0 {
            (snap.current_page as f32 / snap.total_pages as f32 * 100.0) as i32
        } else {
            0
        };
        app.set_progress_pct(progress as f32 / 100.0);
        app.set_progress_label(slint::SharedString::from(format!("{progress}%")));
    } else {
        if !notes_open && !search_open {
            app.set_show_library(true);
            app.set_show_reader(false);
        }
    }

    // Search results state.
    let search_model: Vec<SearchHit> = snap
        .last_search
        .as_ref()
        .map(|res| {
            res.hits
                .iter()
                .map(|h| SearchHit {
                    book_id: slint::SharedString::from(h.book_id.to_string()),
                    book_title: slint::SharedString::from(&h.book_title),
                    chapter_title: slint::SharedString::from(&h.chapter_title),
                    snippet: slint::SharedString::from(strip_mark_tags(&h.snippet)),
                    cfi: slint::SharedString::from(&h.cfi),
                    block_index: h.block_index as i32,
                    char_offset: h.char_offset as i32,
                    term_len: h.term_len as i32,
                })
                .collect()
        })
        .unwrap_or_default();
    app.set_search_hits(slint::ModelRc::from(search_model.as_slice()));
    if snap.last_search.is_none() {
        app.set_search_has_query(false);
    }

    // In-reader search overlay state.
    if let Some(rs) = snap.reader_search.as_ref() {
        app.set_reader_search_count(slint::SharedString::from(format!(
            "{} / {}",
            rs.index + 1,
            rs.total
        )));
        if rs.total == 0 {
            app.set_reader_search_count(slint::SharedString::from("0 / 0"));
        }
    } else {
        app.set_reader_search_count(slint::SharedString::from(""));
    }

    // Notes list state.
    let notes_model: Vec<NotesEntry> = snap
        .notes_entries
        .iter()
        .map(|e| NotesEntry {
            annotation_id: slint::SharedString::from(&e.annotation_id),
            is_highlight: e.is_highlight,
            color_index: e.color_index,
            snippet: slint::SharedString::from(&e.snippet),
            note_text: slint::SharedString::from(&e.note_text),
            chapter_title: slint::SharedString::from(&e.chapter_title),
            created_at: slint::SharedString::from(&e.created_at),
        })
        .collect();
    app.set_notes_entries(slint::ModelRc::from(notes_model.as_slice()));

    // Bookmarks state.
    let bookmarked = snap.annotations.iter().any(|a| {
        a.kind == reeda_core::AnnotationKind::Bookmark
            && a.deleted_at.is_none()
            && a.cfi
                .as_ref()
                .is_some_and(|r| r.start == snap.page_start_cfi)
    });
    app.set_reader_bookmarked(bookmarked);

    let bookmark_model: Vec<BookmarkEntry> = snap
        .bookmarks_entries
        .iter()
        .map(|e| {
            let short_date: String = e.created_at.chars().take(10).collect();
            BookmarkEntry {
                annotation_id: slint::SharedString::from(&e.annotation_id),
                chapter_title: slint::SharedString::from(&e.chapter_title),
                created_at: slint::SharedString::from(short_date),
            }
        })
        .collect();
    app.set_bookmark_entries(slint::ModelRc::from(bookmark_model.as_slice()));

    // Library state.
    let non_deleted: Vec<_> = snap
        .library
        .iter()
        .filter(|b| b.deleted_at.is_none())
        .collect();
    app.set_library_is_empty(non_deleted.is_empty());
    app.set_library_count_text(slint::SharedString::from(format!(
        "{} books",
        non_deleted.len()
    )));

    let books_model: Vec<BookInfo> = non_deleted
        .iter()
        .map(|b| {
            let initial = b
                .title
                .chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .to_string();
            BookInfo {
                book_id: slint::SharedString::from(b.id.to_string()),
                title: slint::SharedString::from(&b.title),
                author: slint::SharedString::from(b.author.as_deref().unwrap_or("Unknown")),
                cover_path: slint::SharedString::from(b.cover_path.as_deref().unwrap_or("")),
                progress_pct: b.progress_pct as f32,
                initial: slint::SharedString::from(initial),
            }
        })
        .collect();
    app.set_library_books(slint::ModelRc::from(books_model.as_slice()));

    // Settings state.
    let theme_index = match snap.settings.theme {
        reeda_core::Theme::Light => 0,
        reeda_core::Theme::Sepia => 1,
        reeda_core::Theme::Dark => 2,
    };
    app.set_settings_theme_index(theme_index);
    app.set_settings_font_size(snap.settings.typography.font_size_pt as i32);
    app.set_settings_line_height((snap.settings.typography.line_height * 10.0) as i32);
}

/// Strip `<mark>` tags from a search snippet for plain-text rendering.
fn strip_mark_tags(s: &str) -> String {
    s.replace("<mark>", "").replace("</mark>", "")
}
