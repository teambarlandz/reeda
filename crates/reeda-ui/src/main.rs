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

    // Apply the default theme.
    theme::apply_theme(app.window(), reeda_core::Theme::Light);

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
}
