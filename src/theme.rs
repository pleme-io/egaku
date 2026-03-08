/// Color and typography theme applied to all widgets.
///
/// Default: Nord polar night palette (consistent with tobira and other pleme-io apps).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Theme {
    pub background: [f32; 4],
    pub foreground: [f32; 4],
    pub accent: [f32; 4],
    pub spacing: f32,
    pub font_size: f32,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            // Nord Polar Night #2e3440
            background: [0.180, 0.204, 0.251, 1.0],
            // Nord Snow Storm #eceff4
            foreground: [0.925, 0.937, 0.957, 1.0],
            // Nord Frost #88c0d0
            accent: [0.533, 0.753, 0.816, 1.0],
            spacing: 8.0,
            font_size: 14.0,
        }
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
        // Nord Polar Night background should be dark (all channels < 0.3)
        assert!(t.background[0] < 0.3);
        assert!(t.background[1] < 0.3);
        assert!(t.background[2] < 0.3);
    }

    #[test]
    fn foreground_is_light() {
        let t = Theme::default();
        // Nord Snow Storm foreground should be light (all channels > 0.9)
        assert!(t.foreground[0] > 0.9);
        assert!(t.foreground[1] > 0.9);
        assert!(t.foreground[2] > 0.9);
    }

    #[test]
    fn serde_roundtrip() {
        let t = Theme::default();
        let json = serde_json::to_string(&t).unwrap();
        let t2: Theme = serde_json::from_str(&json).unwrap();
        assert!((t.font_size - t2.font_size).abs() < f32::EPSILON);
        assert!((t.spacing - t2.spacing).abs() < f32::EPSILON);
    }
}
