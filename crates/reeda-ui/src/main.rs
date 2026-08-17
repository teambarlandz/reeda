mod theme;

slint::include_modules!();

fn main() {
    let app = AppRoot::new().unwrap();

    // Apply the default theme (Light).
    theme::apply_theme(app.window(), reeda_core::Theme::Light);

    app.run().unwrap();
}
