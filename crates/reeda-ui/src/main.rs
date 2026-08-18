#[cfg(feature = "platform-android")]
mod android;

mod theme;

slint::include_modules!();

fn main() {
    let app = AppRoot::new().unwrap();

    // ── Create core App ─────────────────────────────────────────────
    let mut core = reeda_core::App::new();

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

    // Apply the default theme.
    theme::apply_theme(&app, reeda_core::Theme::Light);

    app.run().unwrap();
}

/// Push a `StateSnapshot` into the Slint UI properties.
fn update_ui(app: &AppRoot, snap: &reeda_core::StateSnapshot) {
    if let Some(ref book) = snap.current_book {
        app.set_show_library(false);
        app.set_show_reader(true);
        app.set_book_title(slint::SharedString::from(&book.title));
        app.set_page_text(slint::SharedString::from(&snap.page_text));

        let progress = if snap.total_pages > 0 {
            (snap.current_page as f32 / snap.total_pages as f32 * 100.0) as i32
        } else {
            0
        };
        app.set_progress_pct(progress as f32 / 100.0);
        app.set_progress_label(slint::SharedString::from(format!("{progress}%")));
    } else {
        app.set_show_library(true);
        app.set_show_reader(false);
    }

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
