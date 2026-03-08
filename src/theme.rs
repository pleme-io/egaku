/// Color and typography theme applied to all widgets.
///
/// Supports full base16 color schemes for Stylix integration.
/// Default: Nord polar night palette (consistent with all pleme-io apps).
///
/// ## Stylix Integration
///
/// The home-manager module for each app maps `config.lib.stylix.colors` to
/// these fields in the generated YAML config. The base16 slots (base00–base0f)
/// map directly to Stylix's color scheme. Semantic aliases (background,
/// foreground, etc.) are derived from base16 slots for convenience.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Theme {
    // -- Base16 color palette (Stylix maps directly to these) --
    /// base00: Default background
    pub base00: [f32; 4],
    /// base01: Lighter background (status bars, line numbers)
    pub base01: [f32; 4],
    /// base02: Selection background
    pub base02: [f32; 4],
    /// base03: Comments, invisibles, line highlighting
    pub base03: [f32; 4],
    /// base04: Dark foreground (status bars)
    pub base04: [f32; 4],
    /// base05: Default foreground
    pub base05: [f32; 4],
    /// base06: Light foreground
    pub base06: [f32; 4],
    /// base07: Lightest foreground
    pub base07: [f32; 4],
    /// base08: Red (errors, deletions)
    pub base08: [f32; 4],
    /// base09: Orange (integers, constants)
    pub base09: [f32; 4],
    /// base0a: Yellow (warnings, classes)
    pub base0a: [f32; 4],
    /// base0b: Green (strings, success)
    pub base0b: [f32; 4],
    /// base0c: Cyan (support, regex)
    pub base0c: [f32; 4],
    /// base0d: Blue (functions, methods)
    pub base0d: [f32; 4],
    /// base0e: Purple (keywords, tags)
    pub base0e: [f32; 4],
    /// base0f: Brown (deprecated, embedded)
    pub base0f: [f32; 4],

    // -- Semantic aliases (derived from base16, can be overridden) --
    /// Primary background (= base00)
    pub background: [f32; 4],
    /// Primary foreground (= base05)
    pub foreground: [f32; 4],
    /// Accent color for focus, links, highlights (= base0d)
    pub accent: [f32; 4],
    /// Error/danger color (= base08)
    pub error: [f32; 4],
    /// Warning color (= base0a)
    pub warning: [f32; 4],
    /// Success/ok color (= base0b)
    pub success: [f32; 4],
    /// Selection/highlight background (= base02)
    pub selection: [f32; 4],
    /// Muted text/comments (= base03)
    pub muted: [f32; 4],
    /// Border/separator color (= base01)
    pub border: [f32; 4],

    // -- Typography --
    pub spacing: f32,
    pub font_size: f32,
}

/// Parse a hex color string like "#2E3440" to [f32; 4] RGBA.
#[must_use]
pub fn hex_to_rgba(hex: &str) -> Option<[f32; 4]> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some([
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        1.0,
    ])
}

/// Convert [f32; 4] RGBA to hex string "#RRGGBB".
#[must_use]
pub fn rgba_to_hex(rgba: &[f32; 4]) -> String {
    let r = (rgba[0] * 255.0).round() as u8;
    let g = (rgba[1] * 255.0).round() as u8;
    let b = (rgba[2] * 255.0).round() as u8;
    format!("#{r:02X}{g:02X}{b:02X}")
}

// Nord base16 constants
const NORD00: [f32; 4] = [0.180, 0.204, 0.251, 1.0]; // #2E3440 polar night
const NORD01: [f32; 4] = [0.231, 0.259, 0.322, 1.0]; // #3B4252
const NORD02: [f32; 4] = [0.263, 0.298, 0.369, 1.0]; // #434C5E
const NORD03: [f32; 4] = [0.298, 0.337, 0.416, 1.0]; // #4C566A
const NORD04: [f32; 4] = [0.847, 0.871, 0.914, 1.0]; // #D8DEE9 snow storm
const NORD05: [f32; 4] = [0.898, 0.914, 0.941, 1.0]; // #E5E9F0
const NORD06: [f32; 4] = [0.925, 0.937, 0.957, 1.0]; // #ECEFF4
const NORD07: [f32; 4] = [0.925, 0.937, 0.957, 1.0]; // #ECEFF4 (same as 06 in Nord)
const NORD08: [f32; 4] = [0.749, 0.380, 0.416, 1.0]; // #BF616A red
const NORD09: [f32; 4] = [0.816, 0.529, 0.439, 1.0]; // #D08770 orange
const NORD0A: [f32; 4] = [0.922, 0.796, 0.545, 1.0]; // #EBCB8B yellow
const NORD0B: [f32; 4] = [0.639, 0.745, 0.549, 1.0]; // #A3BE8C green
const NORD0C: [f32; 4] = [0.561, 0.737, 0.733, 1.0]; // #8FBCBB teal
const NORD0D: [f32; 4] = [0.533, 0.753, 0.816, 1.0]; // #88C0D0 frost blue
const NORD0E: [f32; 4] = [0.506, 0.631, 0.757, 1.0]; // #81A1C1 blue
const NORD0F: [f32; 4] = [0.369, 0.506, 0.675, 1.0]; // #5E81AC dark blue

impl Default for Theme {
    fn default() -> Self {
        Self {
            base00: NORD00,
            base01: NORD01,
            base02: NORD02,
            base03: NORD03,
            base04: NORD04,
            base05: NORD05,
            base06: NORD06,
            base07: NORD07,
            base08: NORD08,
            base09: NORD09,
            base0a: NORD0A,
            base0b: NORD0B,
            base0c: NORD0C,
            base0d: NORD0D,
            base0e: NORD0E,
            base0f: NORD0F,
            // Semantic aliases
            background: NORD00,
            foreground: NORD05,
            accent: NORD0D,
            error: NORD08,
            warning: NORD0A,
            success: NORD0B,
            selection: NORD02,
            muted: NORD03,
            border: NORD01,
            spacing: 8.0,
            font_size: 14.0,
        }
    }
}

impl Theme {
    /// Create a theme from a base16 hex color array (16 strings like "#2E3440").
    /// Semantic aliases are automatically derived.
    #[must_use]
    pub fn from_base16(colors: &[&str; 16]) -> Option<Self> {
        let c: Vec<[f32; 4]> = colors.iter().filter_map(|h| hex_to_rgba(h)).collect();
        if c.len() != 16 {
            return None;
        }
        Some(Self {
            base00: c[0],
            base01: c[1],
            base02: c[2],
            base03: c[3],
            base04: c[4],
            base05: c[5],
            base06: c[6],
            base07: c[7],
            base08: c[8],
            base09: c[9],
            base0a: c[10],
            base0b: c[11],
            base0c: c[12],
            base0d: c[13],
            base0e: c[14],
            base0f: c[15],
            background: c[0],
            foreground: c[5],
            accent: c[13],
            error: c[8],
            warning: c[10],
            success: c[11],
            selection: c[2],
            muted: c[3],
            border: c[1],
            spacing: 8.0,
            font_size: 14.0,
        })
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let t = Theme::default();
        assert!((t.font_size - 14.0).abs() < f32::EPSILON);
        assert!((t.spacing - 8.0).abs() < f32::EPSILON);
        assert_eq!(t.background[3], 1.0);
        assert_eq!(t.foreground[3], 1.0);
        assert_eq!(t.accent[3], 1.0);
    }

    #[test]
    fn background_is_dark() {
        let t = Theme::default();
        assert!(t.background[0] < 0.3);
        assert!(t.background[1] < 0.3);
        assert!(t.background[2] < 0.3);
    }

    #[test]
    fn foreground_is_light() {
        let t = Theme::default();
        assert!(t.foreground[0] > 0.8);
        assert!(t.foreground[1] > 0.8);
        assert!(t.foreground[2] > 0.8);
    }

    #[test]
    fn serde_roundtrip() {
        let t = Theme::default();
        let json = serde_json::to_string(&t).unwrap();
        let t2: Theme = serde_json::from_str(&json).unwrap();
        assert!((t.font_size - t2.font_size).abs() < f32::EPSILON);
        assert!((t.spacing - t2.spacing).abs() < f32::EPSILON);
    }

    #[test]
    fn hex_to_rgba_valid() {
        let c = hex_to_rgba("#2E3440").unwrap();
        assert!((c[0] - 0.180).abs() < 0.01);
        assert!((c[1] - 0.204).abs() < 0.01);
        assert!((c[2] - 0.251).abs() < 0.01);
        assert_eq!(c[3], 1.0);
    }

    #[test]
    fn hex_to_rgba_no_hash() {
        let c = hex_to_rgba("ECEFF4").unwrap();
        assert!(c[0] > 0.9);
    }

    #[test]
    fn hex_to_rgba_invalid() {
        assert!(hex_to_rgba("invalid").is_none());
        assert!(hex_to_rgba("#FFF").is_none());
    }

    #[test]
    fn rgba_to_hex_roundtrip() {
        let hex = "#2E3440";
        let rgba = hex_to_rgba(hex).unwrap();
        let back = rgba_to_hex(&rgba);
        assert_eq!(back, hex);
    }

    #[test]
    fn from_base16_nord() {
        let colors = [
            "#2E3440", "#3B4252", "#434C5E", "#4C566A",
            "#D8DEE9", "#E5E9F0", "#ECEFF4", "#ECEFF4",
            "#BF616A", "#D08770", "#EBCB8B", "#A3BE8C",
            "#8FBCBB", "#88C0D0", "#81A1C1", "#5E81AC",
        ];
        let theme = Theme::from_base16(&colors).unwrap();
        // Semantic aliases should be derived
        assert_eq!(theme.background, theme.base00);
        assert_eq!(theme.foreground, theme.base05);
        assert_eq!(theme.accent, theme.base0d);
        assert_eq!(theme.error, theme.base08);
    }

    #[test]
    fn from_base16_invalid() {
        let colors = [
            "#2E3440", "#3B4252", "#434C5E", "#4C566A",
            "#D8DEE9", "#E5E9F0", "#ECEFF4", "#ECEFF4",
            "#BF616A", "#D08770", "#EBCB8B", "#A3BE8C",
            "#8FBCBB", "#88C0D0", "#81A1C1", "invalid",
        ];
        assert!(Theme::from_base16(&colors).is_none());
    }

    #[test]
    fn all_base16_slots_populated() {
        let t = Theme::default();
        // All 16 base slots should have alpha = 1.0
        for slot in [
            t.base00, t.base01, t.base02, t.base03,
            t.base04, t.base05, t.base06, t.base07,
            t.base08, t.base09, t.base0a, t.base0b,
            t.base0c, t.base0d, t.base0e, t.base0f,
        ] {
            assert_eq!(slot[3], 1.0);
        }
    }
}
