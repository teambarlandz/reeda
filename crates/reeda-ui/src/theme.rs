/// Theme management for the Slint UI.
///
/// Applies `reeda_core::Theme` to the `Theme.current` palette in Theme.slint.
pub fn apply_theme(app: &crate::AppRoot, theme: reeda_core::Theme) {
    use slint::Global;
    let global = crate::Theme::get(app);
    let (light, sepia, dark) = (
        global.get_light_palette(),
        global.get_sepia_palette(),
        global.get_dark_palette(),
    );
    let palette = match theme {
        reeda_core::Theme::Light => light,
        reeda_core::Theme::Sepia => sepia,
        reeda_core::Theme::Dark => dark,
    };
    global.set_current(palette);
}