/// Theme management for the Slint UI.
///
/// For M0, the theme is applied via the global `Theme.slint` palette.
/// Full integration: set `Theme.current` property from Rust after window creation.
pub fn apply_theme(_window: &slint::Window, _theme: reeda_core::Theme) {
    // Theme colors are defined in Theme.slint as hardcoded palette structs.
    // The Rust side will swap between them via the generated Theme global.
    // For M0, the default (Light) palette is used out of the box.
}
