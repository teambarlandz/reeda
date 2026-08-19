//! Theme filters applied to rendered page bitmaps (PDF_SPEC §3).
//!
//! PDF pages are rasterized in the normal (light) colors and the night /
//! sepia variants are produced here as render-time RGBA filters, so the
//! cache only ever needs to re-render on theme/zoom changes.

/// Display theme of a rendered page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Theme {
    /// Unfiltered page colors.
    #[default]
    Normal,
    /// Luminance-preserving dark palette (dark background, light text).
    Night,
    /// Warm sepia curve.
    Sepia,
}

/// Background color of a theme in RGBA (used for compositing page margins).
pub fn theme_background(theme: Theme) -> [u8; 4] {
    match theme {
        Theme::Normal => [0xF7, 0xF4, 0xEC, 0xFF],
        Theme::Night => [0x10, 0x14, 0x1A, 0xFF],
        Theme::Sepia => [0xF1, 0xE8, 0xD8, 0xFF],
    }
}

/// Apply the theme filter to an RGBA bitmap in place.
///
/// `rgba` must contain `width * height * 4` bytes in row-major order.
/// Alpha is preserved untouched.
pub fn apply_theme_filter(rgba: &mut [u8], theme: Theme) {
    match theme {
        Theme::Normal => {}
        Theme::Night => filter_night(rgba),
        Theme::Sepia => filter_sepia(rgba),
    }
}

fn filter_night(rgba: &mut [u8]) {
    for px in rgba.chunks_exact_mut(4) {
        let (r, g, b) = (px[0] as f32, px[1] as f32, px[2] as f32);
        // Relative luminance of the source pixel.
        let luma = 0.299 * r + 0.587 * g + 0.114 * b;
        // Invert: dark ink becomes light text, white page becomes a dark
        // background. Map the inverted luminance onto a dark palette
        // (16..224) with a cool tint.
        let v = 16.0 + (255.0 - luma) * (224.0 - 16.0) / 255.0;
        px[0] = (v * 0.92) as u8;
        px[1] = (v * 0.98) as u8;
        px[2] = v as u8;
    }
}

fn filter_sepia(rgba: &mut [u8]) {
    for px in rgba.chunks_exact_mut(4) {
        let (r, g, b) = (px[0] as f32, px[1] as f32, px[2] as f32);
        px[0] = (0.393 * r + 0.769 * g + 0.189 * b).min(255.0) as u8;
        px[1] = (0.349 * r + 0.686 * g + 0.168 * b).min(255.0) as u8;
        px[2] = (0.272 * r + 0.534 * g + 0.131 * b).min(255.0) as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(rgba: &[u8]) -> [u8; 4] {
        [rgba[0], rgba[1], rgba[2], rgba[3]]
    }

    #[test]
    fn normal_leaves_pixels_untouched() {
        let mut buf = [10, 20, 30, 40, 250, 240, 230, 220];
        apply_theme_filter(&mut buf, Theme::Normal);
        assert_eq!(buf, [10, 20, 30, 40, 250, 240, 230, 220]);
    }

    #[test]
    fn night_maps_white_to_dark_background() {
        let mut buf = [255, 255, 255, 255];
        apply_theme_filter(&mut buf, Theme::Night);
        let px = pixel(&buf);
        // Light page becomes a dark background with a cool tint.
        assert!(px[0] < 60 && px[1] < 60 && px[2] < 60, "got {px:?}");
        assert!(px[2] >= px[0], "cool tint expected, got {px:?}");
        assert_eq!(px[3], 255, "alpha preserved");
    }

    #[test]
    fn night_maps_black_to_light_text() {
        let mut buf = [0, 0, 0, 255];
        apply_theme_filter(&mut buf, Theme::Night);
        let px = pixel(&buf);
        // Dark ink becomes light text.
        assert!(px[0] > 140 && px[2] > 140, "got {px:?}");
    }

    #[test]
    fn night_preserves_relative_contrast_inverted() {
        let mut dark = [30, 30, 30, 255];
        let mut light = [200, 200, 200, 255];
        apply_theme_filter(&mut dark, Theme::Night);
        apply_theme_filter(&mut light, Theme::Night);
        // Brighter source pixel becomes the darker output (inversion keeps
        // relative contrast intact).
        let luma_dark = dark[0] as u32 + dark[1] as u32 + dark[2] as u32;
        let luma_light = light[0] as u32 + light[1] as u32 + light[2] as u32;
        assert!(
            luma_dark > luma_light,
            "inverted ordering expected: {luma_dark} vs {luma_light}"
        );
    }

    #[test]
    fn sepia_warms_white() {
        let mut buf = [255, 255, 255, 255];
        apply_theme_filter(&mut buf, Theme::Sepia);
        let px = pixel(&buf);
        assert!(
            px[0] >= px[1] && px[1] >= px[2],
            "warm ramp expected, got {px:?}"
        );
        assert_eq!(px[3], 255);
    }

    #[test]
    fn sepia_keeps_black_black() {
        let mut buf = [0, 0, 0, 255];
        apply_theme_filter(&mut buf, Theme::Sepia);
        assert_eq!(pixel(&buf), [0, 0, 0, 255]);
    }
}
