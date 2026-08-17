use serde::{Deserialize, Serialize};

/// Visual theme for the reader.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    /// Light theme (paper white).
    #[default]
    Light,
    /// Sepia theme (warm paper).
    Sepia,
    /// Dark / night theme.
    Dark,
}

impl Theme {
    /// Background hex color for this theme.
    pub fn background(&self) -> &'static str {
        match self {
            Theme::Light => "#F7F4EC",
            Theme::Sepia => "#F1E8D8",
            Theme::Dark => "#101418",
        }
    }

    /// Foreground (text) hex color for this theme.
    pub fn foreground(&self) -> &'static str {
        match self {
            Theme::Light => "#1A1A1A",
            Theme::Sepia => "#3B2E1E",
            Theme::Dark => "#D8D8D8",
        }
    }
}

/// Tap zone layout for page-turn gestures.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TapZonesLayout {
    /// Default: left 25% = prev, right 25% = next, center 50% = chrome.
    #[default]
    Default,
    /// Swapped: left = next, right = prev (for left-handed users).
    Swapped,
}

/// Typography settings for reading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Typography {
    /// Font family name (mapped to system/bundled fonts).
    pub font_family: String,
    /// Font size in points.
    pub font_size_pt: f32,
    /// Line height multiplier (1.0–3.0).
    pub line_height: f32,
    /// Horizontal margin in dp.
    pub margin: f32,
    /// Whether to justify text.
    pub justify: bool,
}

impl Default for Typography {
    fn default() -> Self {
        Self {
            font_family: String::from("serif"),
            font_size_pt: 18.0,
            line_height: 1.5,
            margin: 24.0,
            justify: true,
        }
    }
}

/// Complete application settings (persisted in the `settings` KV table).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Visual theme.
    pub theme: Theme,
    /// Default typography for reading.
    pub typography: Typography,
    /// Tap zones layout.
    pub tap_zones_layout: TapZonesLayout,
    /// TTS playback speed (0.5–2.5).
    pub tts_speed: f32,
    /// TTS pitch (0.5–2.0).
    pub tts_pitch: f32,
    /// Whether to keep screen on during narration.
    pub tts_wakelock: bool,
    /// Locale override (None = system default).
    pub locale: Option<String>,
    /// Whether the first-run onboarding has been completed.
    pub first_run_done: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            typography: Typography::default(),
            tap_zones_layout: TapZonesLayout::default(),
            tts_speed: 1.0,
            tts_pitch: 1.0,
            tts_wakelock: true,
            locale: None,
            first_run_done: false,
        }
    }
}
