//! Reeda UI — Slint frontend shared by the desktop binary and the Android
//! app (android-activity `android_main`). See `src/main.rs` (desktop entry)
//! and `src/android/` (Android JNI bridge).

#[cfg(feature = "platform-android")]
mod android;

#[cfg(not(feature = "platform-android"))]
mod diag;

mod theme;

slint::include_modules!();

use slint::Model;

/// Android app entry (android-activity NativeActivity): initialize the
/// Slint Android platform, then run the same app logic as the desktop
/// binary. The `#[no_mangle]` + cdylib export is how the Android runtime
/// finds `android_main` (see slint's android.rs docs).
#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: slint::android::AndroidApp) {
    crate::android::log::init();
    crate::android::log::trace("android_main entered");
    match slint::android::init(app) {
        Ok(()) => crate::android::log::trace("slint android init ok"),
        Err(e) => crate::android::log::trace(&format!("slint android init FAILED: {e}")),
    }
    run();
}

pub fn run() {
    #[cfg(not(feature = "platform-android"))]
    diag::init();

    #[cfg(feature = "platform-android")]
    crate::android::log::trace("creating window");
    let app = AppRoot::new().unwrap();
    #[cfg(feature = "platform-android")]
    crate::android::log::trace("window created");

    // ── Create core App ─────────────────────────────────────────────
    let mut core = reeda_core::App::new();

    // Android: swap in the JNI TextToSpeech host (TTS_SPEC §2).
    // The host self-initializes the TtsShim singleton; foreground service,
    // audio focus and wake-lock land with device verification (M5.5).
    #[cfg(feature = "platform-android")]
    {
        match crate::android::create_tts_host() {
            Ok(host) => {
                core.set_tts_host(host);
                crate::android::log::trace("tts host ready");
            }
            Err(e) => {
                eprintln!("TTS unavailable: {e} (narration disabled)");
                crate::android::log::trace(&format!("tts host FAILED: {e}"));
            }
        }
    }

    // Desktop: swap in the platform-native TTS host (chunk-level narration;
    // word-boundary Range events stay Android-only for now).
    #[cfg(not(feature = "platform-android"))]
    {
        if let Some(host) = reeda_core::create_platform_tts_host() {
            core.set_tts_host(host);
            diag::log("TTS host ready");
        } else {
            diag::log("TTS unavailable (narration disabled)");
        }
    }

    // Persistent storage + full-text search index. Desktop: ./reeda_data.
    // Android: app-private filesDir (see android::data_dir).
    // Books imported this session are searchable immediately; re-indexing
    // of books loaded from storage is a P2 follow-up (see TODO M4.7).
    #[cfg(feature = "platform-android")]
    let data_root = crate::android::data_dir();
    #[cfg(not(feature = "platform-android"))]
    let data_root = String::from("reeda_data");
    #[cfg(feature = "platform-android")]
    crate::android::log::trace(&format!("data root resolved: {data_root}"));
    if let Ok(store) = reeda_core::BookStore::new(&data_root) {
        if let Ok(db) = reeda_core::Database::open(store.root().join("reeda.sqlite")) {
            core.set_db(db);
        }
        if let Ok(search) = reeda_core::search::SearchService::open(store.root()) {
            core.set_search(search);
        }
        core.set_store(store);
        #[cfg(feature = "platform-android")]
        crate::android::log::trace("store + db + search opened");
    } else {
        #[cfg(feature = "platform-android")]
        crate::android::log::trace("store open FAILED");
    }

    // Restore persisted state (library + settings) from SQLite so books
    // imported in earlier sessions reappear on the next launch.
    if let Err(e) = core.load_books() {
        diag::log(format!("Warning: failed to restore library: {e}"));
    }
    if let Err(e) = core.load_settings_from_db() {
        diag::log(format!("Warning: failed to restore settings: {e}"));
    }

    // If a file path is provided as CLI arg, import it and open.
    let args: Vec<String> = std::env::args().collect();
    if let Some(path) = args.get(1) {
        if path.to_lowercase().ends_with(".pdf") {
            let events = core.dispatch(reeda_core::Command::ImportPdf { path: path.clone() });
            show_error_events(&app, &events);
            if let Some(reeda_core::Event::ImportFinished { book_id }) = events.first() {
                let _ = core.dispatch(reeda_core::Command::OpenBook { book_id: *book_id });
            }
        } else {
            let epub_data = match std::fs::read(path) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Failed to read {path}: {e}");
                    return;
                }
            };
            let events = core.import_from_bytes(epub_data, path.clone());
            show_error_events(&app, &events);
            if let Some(reeda_core::Event::ImportFinished { book_id }) = events.first() {
                let _ = core.dispatch(reeda_core::Command::OpenBook { book_id: *book_id });
            }
        }
    }

    // Push initial state into the Slint UI.
    let snap = core.snapshot();
    update_ui(&app, &snap);
    #[cfg(feature = "platform-android")]
    crate::android::log::trace("initial UI state pushed");

    // ── Wire callbacks ──────────────────────────────────────────────
    let weak = app.as_weak();
    let core_cell = std::rc::Rc::new(std::cell::RefCell::new(core));

    // Device pixel ratio drives the PDF render scale (PDF_SPEC §3).
    PDF_UI.with(|cell| cell.borrow_mut().dpr = app.window().scale_factor());

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

    // ── Open a book from the library grid ───────────────────────────
    app.on_open_book({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move |book_id: slint::SharedString| {
            let events = {
                let mut core = core_cell.borrow_mut();
                match reeda_core::BookId::try_from(book_id.as_str()) {
                    Ok(id) => core.dispatch(reeda_core::Command::OpenBook { book_id: id }),
                    Err(_) => vec![reeda_core::Event::Error {
                        message: format!("Invalid book id: {book_id}"),
                    }],
                }
            };
            let app = weak.unwrap();
            show_error_events(&app, &events);
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    // ── Delete a book from the library grid context menu ────────────
    app.on_book_delete_confirmed({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move |book_id: slint::SharedString| {
            let events = {
                let mut core = core_cell.borrow_mut();
                match reeda_core::BookId::try_from(book_id.as_str()) {
                    Ok(id) => core.dispatch(reeda_core::Command::DeleteBook { book_id: id }),
                    Err(_) => vec![reeda_core::Event::Error {
                        message: format!("Invalid book id: {book_id}"),
                    }],
                }
            };
            let app = weak.unwrap();
            show_error_events(&app, &events);
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
            // Re-render PDF pages with the matching night/sepia filter.
            PDF_UI.with(|cell| {
                let mut pdf = cell.borrow_mut();
                pdf.theme = pdf_theme(theme);
                pdf.cache.clear();
                pdf.images = std::rc::Rc::new(slint::VecModel::from(vec![
                    slint::Image::default();
                    pdf.page_count as usize
                ]));
                app.set_pdf_images(slint::ModelRc::from(pdf.images.clone()));
                pdf_render_visible(&app, &mut pdf);
            });
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
            app.set_show_bookmarks(false);
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    // ── Import file (PDF/EPUB) ────────────────────────────────────────
    app.on_import_requested({
        let weak = weak.clone();
        #[cfg(feature = "platform-desktop")]
        {
            let core_cell = core_cell.clone();
            move || {
                let Some(path) = rfd::FileDialog::new()
                    .add_filter("EPUB & PDF", &["epub", "pdf"])
                    .pick_file()
                else {
                    return; // User cancelled the dialog.
                };
                let path_str = path.to_string_lossy().to_string();
                let events = if path_str.to_lowercase().ends_with(".pdf") {
                    core_cell
                        .borrow_mut()
                        .dispatch(reeda_core::Command::ImportPdf { path: path_str })
                } else {
                    let epub_data = match std::fs::read(&path) {
                        Ok(d) => d,
                        Err(e) => {
                            eprintln!("Failed to read {path_str}: {e}");
                            return;
                        }
                    };
                    core_cell
                        .borrow_mut()
                        .import_from_bytes(epub_data, path_str)
                };
                show_error_events(&weak.unwrap(), &events);
                if let Some(reeda_core::Event::ImportFinished { book_id }) = events.first() {
                    let _ = core_cell
                        .borrow_mut()
                        .dispatch(reeda_core::Command::OpenBook { book_id: *book_id });
                }
                let app = weak.unwrap();
                let snap = core_cell.borrow().snapshot();
                update_ui(&app, &snap);
            }
        }
        #[cfg(not(feature = "platform-desktop"))]
        {
            move || {
                // File-picker integration lands with the Android storage
                // Access Framework (SAF); be explicit instead of silent.
                let app = weak.unwrap();
                app.set_error_message(slint::SharedString::from(
                    "Importing books is not supported on this platform yet.",
                ));
                app.set_error_open(true);
            }
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
                let events = {
                    let mut core = core_cell.borrow_mut();
                    core.dispatch(reeda_core::Command::Search { query })
                };
                let app = weak.unwrap();
                show_error_events(&app, &events);
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
                let events = {
                    let mut core = core_cell.borrow_mut();
                    core.dispatch(reeda_core::Command::ReaderSearch { query })
                };
                let app = weak.unwrap();
                show_error_events(&app, &events);
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

    // ── Narration (TTS bar) ───────────────────────────────────────────
    app.on_narration_play_pause({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move || {
            // State machine per TTS_SPEC §4: start when idle, resume when
            // paused, pause while speaking. (Previously the idle state fell
            // through to PauseNarration — narration could never be started
            // from the UI.)
            let command = match core_cell.borrow().snapshot().narration_state {
                reeda_core::NarrationState::Paused => reeda_core::Command::ResumeNarration,
                reeda_core::NarrationState::Speaking | reeda_core::NarrationState::Loading => {
                    reeda_core::Command::PauseNarration
                }
                _ => reeda_core::Command::StartNarration { chapter_id: None },
            };
            {
                let mut core = core_cell.borrow_mut();
                core.dispatch(command);
            }
            let app = weak.unwrap();
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    app.on_narration_stop({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move || {
            {
                let mut core = core_cell.borrow_mut();
                core.dispatch(reeda_core::Command::StopNarration);
            }
            let app = weak.unwrap();
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    fn narration_skip(
        delta: isize,
        core_cell: &std::rc::Rc<std::cell::RefCell<reeda_core::App>>,
        weak: &slint::Weak<crate::AppRoot>,
    ) {
        {
            let mut core = core_cell.borrow_mut();
            core.dispatch(reeda_core::Command::NarrationSkip { delta });
        }
        let app = weak.unwrap();
        let snap = core_cell.borrow().snapshot();
        update_ui(&app, &snap);
    }

    app.on_narration_skip_back({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move || narration_skip(-1, &core_cell, &weak)
    });

    app.on_narration_skip_forward({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move || narration_skip(1, &core_cell, &weak)
    });

    app.on_narration_speed_cycle({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move || {
            let current = core_cell.borrow().snapshot().settings.tts_speed;
            let next = {
                let step = (current * 10.0).round() / 10.0;
                let bumped = step + 0.1;
                if bumped > 2.5 + f32::EPSILON {
                    0.5
                } else {
                    bumped
                }
            };
            {
                let mut core = core_cell.borrow_mut();
                core.dispatch(reeda_core::Command::SetTtsSpeed(next));
            }
            let app = weak.unwrap();
            let snap = core_cell.borrow().snapshot();
            update_ui(&app, &snap);
        }
    });

    // Poll narration host callbacks (word highlights / chunk completion).
    // Runs always; polling an idle engine is a no-op.
    let narration_timer = slint::Timer::default();
    narration_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(300),
        {
            let weak = weak.clone();
            let core_cell = core_cell.clone();
            move || {
                let active = {
                    let snap = core_cell.borrow().snapshot();
                    matches!(
                        snap.narration_state,
                        reeda_core::NarrationState::Speaking | reeda_core::NarrationState::Paused
                    )
                };
                if active {
                    {
                        let mut core = core_cell.borrow_mut();
                        core.dispatch(reeda_core::Command::PollNarration);
                    }
                    let app = weak.unwrap();
                    let snap = core_cell.borrow().snapshot();
                    update_ui(&app, &snap);
                }
            }
        },
    );

    // ── PDF viewer (M6.4) ─────────────────────────────────────────────
    app.on_pdf_scrolled({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move || {
            let app = weak.unwrap();
            let mut core = core_cell.borrow_mut();
            PDF_UI.with(|cell| {
                let mut pdf = cell.borrow_mut();
                pdf_scrolled(&app, &mut pdf, &mut core);
            });
        }
    });

    app.on_pdf_visible_changed({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move |width: f32, height: f32| {
            let app = weak.unwrap();
            let mut core = core_cell.borrow_mut();
            PDF_UI.with(|cell| {
                let mut pdf = cell.borrow_mut();
                pdf_visible_changed(&app, &mut pdf, &mut core, width, height);
            });
        }
    });

    app.on_pdf_zoom_by({
        let weak = weak.clone();
        move |delta: f32| {
            let app = weak.unwrap();
            PDF_UI.with(|cell| {
                let mut pdf = cell.borrow_mut();
                pdf_zoom_to(&app, &mut pdf, delta);
            });
        }
    });

    app.on_pdf_toggle_zoom({
        let weak = weak.clone();
        move || {
            let app = weak.unwrap();
            PDF_UI.with(|cell| {
                let mut pdf = cell.borrow_mut();
                pdf_zoom_to(&app, &mut pdf, 0.0);
            });
        }
    });

    app.on_pdf_jumped({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move |page: slint::SharedString| {
            let app = weak.unwrap();
            let mut core = core_cell.borrow_mut();
            let Ok(num) = page.trim().parse::<u32>() else {
                return;
            };
            PDF_UI.with(|cell| {
                let mut pdf = cell.borrow_mut();
                pdf_jump(&app, &mut pdf, &mut core, num);
            });
        }
    });

    app.on_pdf_outline_jumped({
        let weak = weak.clone();
        let core_cell = core_cell.clone();
        move |page: i32| {
            let app = weak.unwrap();
            let mut core = core_cell.borrow_mut();
            PDF_UI.with(|cell| {
                let mut pdf = cell.borrow_mut();
                pdf_jump(&app, &mut pdf, &mut core, page as u32);
            });
        }
    });

    // Apply the persisted theme (falls back to Light for a fresh install).
    theme::apply_theme(&app, snap.settings.theme);

    #[cfg(feature = "platform-android")]
    crate::android::log::trace("event loop starting");
    app.run().unwrap();
    #[cfg(feature = "platform-android")]
    crate::android::log::trace("event loop ended");
}

/// Open the modal error dialog for every `Event::Error` / `Event::ImportFailed`
/// in `events` (UI_UX.md §3: error dialog with cause). Other events are
/// ignored here.
fn show_error_events(app: &AppRoot, events: &[reeda_core::Event]) {
    for event in events {
        let message = match event {
            reeda_core::Event::Error { message } => message,
            reeda_core::Event::ImportFailed { error } => error,
            _ => continue,
        };
        app.set_error_message(slint::SharedString::from(message.as_str()));
        app.set_error_open(true);
    }
}

/// Push a `StateSnapshot` into the Slint UI properties.
fn update_ui(app: &AppRoot, snap: &reeda_core::StateSnapshot) {
    let notes_open = app.get_show_notes();
    let search_open = app.get_show_search();

    // PDF reader state (M6.4).
    match snap.pdf.as_ref() {
        Some(view) => {
            app.set_pdf_mode(true);
            app.set_pdf_page_count(view.page_count as i32);
            // Outline panel model (M6.5): 1-based pages match the jump dialog.
            let outline_model: Vec<OutlineItem> = view
                .outline
                .iter()
                .map(|item| OutlineItem {
                    title: slint::SharedString::from(&item.title),
                    page: (item.page_index + 1) as i32,
                    depth: item.depth as i32,
                })
                .collect();
            app.set_pdf_outline(slint::ModelRc::from(outline_model.as_slice()));
            PDF_UI.with(|cell| {
                let mut pdf = cell.borrow_mut();
                pdf_open(app, view, &mut pdf);
            });
        }
        None => {
            app.set_pdf_mode(false);
            app.set_pdf_page_count(0);
            app.set_pdf_outline(slint::ModelRc::from(Vec::<OutlineItem>::new().as_slice()));
            PDF_UI.with(|cell| {
                let mut pdf = cell.borrow_mut();
                pdf_close(app, &mut pdf);
            });
        }
    }

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

    // Narration bar state.
    let narrating = matches!(
        snap.narration_state,
        reeda_core::NarrationState::Speaking
            | reeda_core::NarrationState::Paused
            | reeda_core::NarrationState::Loading
    );
    app.set_narration_active(narrating);
    app.set_narration_paused(snap.narration_state == reeda_core::NarrationState::Paused);
    app.set_narration_speed_text(slint::SharedString::from(format!(
        "{:.1}x",
        snap.settings.tts_speed
    )));

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

/// Gap (logical px) between stacked PDF pages in the canvas.
const PDF_PAGE_GAP: f32 = 8.0;

/// UI-side state for the PDF viewer (M6.4, PDF_SPEC §3–§5).
///
/// Raster pages are produced by `reeda_pdf` at `dpr × fit × zoom` scale,
/// cached in an LRU raster cache, and pushed into the Slint `[image]`
/// property as RGBA images. Only the pages intersecting the viewport are
/// rasterized.
struct PdfUiState {
    /// Path of the currently open PDF (empty = none).
    path: String,
    /// Page count of the open document.
    page_count: u32,
    /// Per-page `(width, height)` in PDF points (72 dpi).
    page_sizes_pt: Vec<(f32, f32)>,
    /// Zoom multiplier on fit-to-width (1.0 = fit).
    zoom: f32,
    /// Fit factor: visible width / page width at 96 dpi (0 until known).
    fit_factor: f32,
    /// Visible viewport size (logical px, reported by the UI).
    visible_width: f32,
    visible_height: f32,
    /// Device pixel ratio (from the window).
    dpr: f32,
    /// Render-time theme filter.
    theme: reeda_pdf::theme::Theme,
    /// LRU raster cache (PDF_SPEC §5, ≤128 MB).
    cache: reeda_pdf::cache::RasterCache,
    /// Per-page raster images pushed to the UI (default image = not rendered).
    images: std::rc::Rc<slint::VecModel<slint::Image>>,
    /// Whether the open document has been reset but not yet rendered
    /// (first layout may still be unknown).
    pending_layout: bool,
}

impl Default for PdfUiState {
    fn default() -> Self {
        Self {
            path: String::new(),
            page_count: 0,
            page_sizes_pt: Vec::new(),
            zoom: 1.0,
            fit_factor: 0.0,
            visible_width: 0.0,
            visible_height: 0.0,
            dpr: 1.0,
            theme: reeda_pdf::theme::Theme::Normal,
            cache: reeda_pdf::cache::RasterCache::new(),
            images: std::rc::Rc::new(slint::VecModel::default()),
            pending_layout: true,
        }
    }
}

impl PdfUiState {
    /// Logical width of a page at the current zoom (all pages share width
    /// after fit-to-width; 0 before the visible width is known).
    fn logical_page_width(&self) -> f32 {
        self.visible_width * self.zoom
    }

    /// Logical height of page `i` at the current zoom.
    fn logical_page_height(&self, i: usize) -> f32 {
        let (_, h_pt) = self.page_sizes_pt[i];
        h_pt * reeda_pdf::render::PIXELS_PER_POINT * self.fit_factor * self.zoom
    }

    /// Total canvas height: pages + gaps + margins.
    fn content_height(&self) -> f32 {
        let sum: f32 = (0..self.page_count as usize)
            .map(|i| self.logical_page_height(i))
            .sum();
        let gaps = if self.page_count > 1 {
            (self.page_count as f32 - 1.0) * PDF_PAGE_GAP
        } else {
            0.0
        };
        sum + gaps + 16.0
    }

    /// Render scale (device px per PDF point) for the current zoom.
    fn render_scale(&self) -> f32 {
        self.dpr * self.fit_factor * self.zoom
    }

    /// Page index under the viewport's vertical center.
    fn page_at_scroll(&self, offset: f32) -> usize {
        let center = offset + self.visible_height * 0.5 - 8.0;
        let mut y = 0.0f32;
        for i in 0..self.page_count as usize {
            let h = self.logical_page_height(i);
            if center < y + h {
                return i;
            }
            y += h + PDF_PAGE_GAP;
        }
        self.page_count.saturating_sub(1) as usize
    }

    /// Scroll offset (viewport y) that centers page `index`.
    fn scroll_for_page(&self, index: u32) -> f32 {
        let mut y = 0.0f32;
        for i in 0..index as usize {
            y += self.logical_page_height(i) + PDF_PAGE_GAP;
        }
        (y - (self.visible_height - self.logical_page_height(index as usize)) * 0.5).max(0.0)
    }
}

/// Theme mapping: core theme → PDF render-time filter.
fn pdf_theme(theme: reeda_core::Theme) -> reeda_pdf::theme::Theme {
    match theme {
        reeda_core::Theme::Light => reeda_pdf::theme::Theme::Normal,
        reeda_core::Theme::Sepia => reeda_pdf::theme::Theme::Sepia,
        reeda_core::Theme::Dark => reeda_pdf::theme::Theme::Night,
    }
}

thread_local! {
    /// PDF viewer UI state (M6.4). The UI runs on a single thread, so a
    /// thread-local is safe here.
    static PDF_UI: std::cell::RefCell<PdfUiState> =
        std::cell::RefCell::new(PdfUiState::default());
}

/// Reset the PDF viewer for a newly opened document.
fn pdf_open(ui: &AppRoot, pdf: &reeda_core::PdfView, state: &mut PdfUiState) {
    if state.path == pdf.path && state.page_count == pdf.page_count {
        return;
    }
    state.path = pdf.path.clone();
    state.page_count = pdf.page_count;
    state.page_sizes_pt = pdf.page_sizes.clone();
    state.zoom = 1.0;
    state.fit_factor = 0.0;
    state.cache.clear();
    state.images = std::rc::Rc::new(slint::VecModel::from(vec![
        slint::Image::default();
        pdf.page_count as usize
    ]));
    ui.set_pdf_images(slint::ModelRc::from(state.images.clone()));
    state.pending_layout = true;

    // Start at the top of the document.
    ui.set_pdf_scroll_target(0.0);
}

/// Clear the PDF viewer when the open book is not a PDF.
fn pdf_close(ui: &AppRoot, state: &mut PdfUiState) {
    if state.path.is_empty() && state.page_count == 0 {
        return;
    }
    state.path.clear();
    state.page_count = 0;
    state.page_sizes_pt.clear();
    state.zoom = 1.0;
    state.fit_factor = 0.0;
    state.pending_layout = true;
    state.cache.clear();
    state.images = std::rc::Rc::new(slint::VecModel::default());
    ui.set_pdf_images(slint::ModelRc::from(state.images.clone()));
    ui.set_pdf_page_heights(slint::ModelRc::from(Vec::<f32>::new().as_slice()));
    ui.set_pdf_page_width(0.0);
    ui.set_pdf_content_height(0.0);
    ui.set_pdf_page_label(slint::SharedString::from(""));
    ui.set_pdf_zoom_text(slint::SharedString::from(""));
}

/// Jump to a page (1-based number) and center it in the viewport.
fn pdf_jump(ui: &AppRoot, state: &mut PdfUiState, core: &mut reeda_core::App, page: u32) {
    if state.path.is_empty() {
        return;
    }
    let index = page
        .saturating_sub(1)
        .min(state.page_count.saturating_sub(1));
    core.dispatch(reeda_core::Command::PdfPage { page_index: index });
    ui.set_pdf_scroll_target(state.scroll_for_page(index));
    pdf_render_visible(ui, state);
}

/// Rasterize the pages intersecting the viewport and refresh the layout
/// properties (page sizes, content height, label, zoom text).
fn pdf_render_visible(ui: &AppRoot, state: &mut PdfUiState) {
    if state.path.is_empty() || state.page_count == 0 {
        return;
    }
    if state.visible_width <= 0.0 {
        return; // First layout not reported yet; pdf-visible-changed will retry.
    }
    let Some((w0_pt, _)) = state.page_sizes_pt.first().copied() else {
        return;
    };
    let base_width = w0_pt * reeda_pdf::render::PIXELS_PER_POINT;
    let new_fit = (state.visible_width / base_width).max(0.05);
    if state.fit_factor > 0.0 && (new_fit - state.fit_factor).abs() / state.fit_factor > 0.02 {
        // Viewport size changed: fit-to-width rasters at the old fit are
        // stale (the FitWidth bucket does not capture the viewport width).
        state.cache.clear();
        state.images = std::rc::Rc::new(slint::VecModel::from(vec![
            slint::Image::default();
            state.page_count as usize
        ]));
        ui.set_pdf_images(slint::ModelRc::from(state.images.clone()));
    }
    state.fit_factor = new_fit;

    // Layout properties.
    let page_width = state.logical_page_width();
    let heights: Vec<f32> = (0..state.page_count as usize)
        .map(|i| state.logical_page_height(i))
        .collect();
    ui.set_pdf_page_width(page_width);
    ui.set_pdf_page_heights(slint::ModelRc::from(heights.as_slice()));
    ui.set_pdf_content_height(state.content_height());

    // Page label + zoom text.
    let current = ui.get_pdf_scroll_offset();
    let page = state
        .page_at_scroll(current)
        .min(state.page_count as usize - 1);
    ui.set_pdf_page_label(slint::SharedString::from(format!(
        "{} / {}",
        page + 1,
        state.page_count
    )));
    ui.set_pdf_zoom_text(slint::SharedString::from(if state.zoom == 1.0 {
        "Fit".into()
    } else {
        format!("{:.0}%", state.zoom * 100.0)
    }));

    // Rasterize the visible window (+1 page margin) from the bounded LRU
    // cache (PDF_SPEC §5, ≤128 MB). Pages outside the window are dropped
    // from the image model so memory stays within the cache budget; scrolling
    // back re-blits them from the cache instead of re-rasterizing via PDFium.
    let scale = state.render_scale();
    let first = state.page_at_scroll(current).saturating_sub(1);
    let mut last = first + 2;
    while last < state.page_count as usize
        && (state.page_at_scroll(current + state.visible_height * 0.5) + 1) > last
    {
        last += 1;
    }
    last = last.min(state.page_count as usize - 1);

    let empty_image = slint::Image::default();
    for i in 0..state.page_count as usize {
        if (i < first || i > last) && state.images.row_data(i) != Some(empty_image.clone()) {
            state.images.set_row_data(i, empty_image.clone());
        }
    }
    let bucket = if state.zoom == 1.0 {
        reeda_pdf::cache::ScaleBucket::FitWidth
    } else {
        reeda_pdf::cache::ScaleBucket::bucket_for_zoom(state.zoom)
    };

    let path = std::path::Path::new(&state.path);
    for i in first..=last {
        if state
            .images
            .row_data(i)
            .is_some_and(|img| img != empty_image)
        {
            continue;
        }
        let key = reeda_pdf::cache::RasterKey {
            page: i as u32,
            scale: bucket,
            theme: state.theme,
        };
        let page = match state.cache.get(&key) {
            Some(cached) => cached.clone(),
            None => match reeda_pdf::render::render_page(path, i as u32, scale, state.theme) {
                Ok(page) => {
                    state.cache.insert(key, page.clone());
                    page
                }
                Err(e) => {
                    ui.set_pdf_page_label(slint::SharedString::from(format!("PDF error: {e}")));
                    return;
                }
            },
        };
        let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
            &page.rgba,
            page.width,
            page.height,
        );
        let image = slint::Image::from_rgba8(buffer);
        state.images.set_row_data(i, image);
    }
    state.pending_layout = false;
}

/// Apply a zoom delta (0.25 steps) or a fit/100% toggle, then re-render.
fn pdf_zoom_to(ui: &AppRoot, state: &mut PdfUiState, delta: f32) {
    if state.path.is_empty() {
        return;
    }
    if delta == 0.0 {
        // Toggle fit-width ↔ 100% (1 buffer px per device px).
        let Some((w0_pt, _)) = state.page_sizes_pt.first().copied() else {
            return;
        };
        let base_width = w0_pt * reeda_pdf::render::PIXELS_PER_POINT;
        let zoom_100 = if state.visible_width > 0.0 {
            (base_width / state.visible_width).clamp(0.25, 5.0)
        } else {
            1.0
        };
        state.zoom = if state.zoom == 1.0 { zoom_100 } else { 1.0 };
    } else {
        state.zoom = (state.zoom + delta).clamp(0.25, 5.0);
    }
    // Zoom changes invalidate the rasters (different scale bucket).
    state.cache.clear();
    state.images = std::rc::Rc::new(slint::VecModel::from(vec![
        slint::Image::default();
        state.page_count as usize
    ]));
    ui.set_pdf_images(slint::ModelRc::from(state.images.clone()));
    // Keep the current page centered.
    let page = state.page_at_scroll(ui.get_pdf_scroll_offset());
    pdf_render_visible(ui, state);
    ui.set_pdf_scroll_target(state.scroll_for_page(page as u32));
}

/// Viewport moved: sync the current page with the core and rasterize
/// anything newly visible.
fn pdf_scrolled(ui: &AppRoot, state: &mut PdfUiState, core: &mut reeda_core::App) {
    if state.path.is_empty() {
        return;
    }
    let offset = ui.get_pdf_scroll_offset();
    let page = state.page_at_scroll(offset);
    let current = core.snapshot().current_page as usize;
    if page != current && page < state.page_count as usize {
        core.dispatch(reeda_core::Command::PdfPage {
            page_index: page as u32,
        });
    }
    pdf_render_visible(ui, state);
}

/// The viewport size changed (first layout or window resize): recompute the
/// fit factor, restore the reading position on first layout, and re-raster.
fn pdf_visible_changed(
    ui: &AppRoot,
    state: &mut PdfUiState,
    core: &mut reeda_core::App,
    width: f32,
    height: f32,
) {
    state.visible_width = width;
    state.visible_height = height;
    if state.pending_layout && !state.path.is_empty() {
        let page = core.snapshot().current_page;
        pdf_render_visible(ui, state);
        ui.set_pdf_scroll_target(state.scroll_for_page(page));
    } else {
        pdf_render_visible(ui, state);
    }
}
