fn main() {
    let config = slint_build::CompilerConfiguration::new()
        // Bundled gettext translations (LOCALIZATION.md §1): <lang>/LC_MESSAGES/<crate>.po
        .with_bundled_translations("translations")
        // Flat msgids: no per-component context (our .po files carry no msgctxt).
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    slint_build::compile_with_config("ui/AppRoot.slint", config).unwrap();
}
