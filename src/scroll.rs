/// Virtualized scrollable container.
///
/// Tracks scroll offset and visible range for efficient rendering
/// of large content (thousands of items).
#[derive(Debug, Clone)]
pub struct ScrollView {
    pub offset: f32,
    pub content_height: f32,
    pub viewport_height: f32,
}

impl ScrollView {
    #[must_use]
    pub fn new(content_height: f32, viewport_height: f32) -> Self {
        Self { offset: 0.0, content_height, viewport_height }
    }

    /// Scroll by a delta, clamping to valid range.
    pub fn scroll_by(&mut self, delta: f32) {
        self.offset += delta;
        self.clamp();
    }

    /// Scroll to an absolute offset, clamping to valid range.
    pub fn scroll_to(&mut self, offset: f32) {
        self.offset = offset;
        self.clamp();
    }

    /// Returns true if scrolled to the top.
    #[must_use]
    pub fn is_at_top(&self) -> bool {
        self.offset <= 0.0
    }

    /// Returns true if scrolled to the bottom (or content fits in viewport).
    #[must_use]
    pub fn is_at_bottom(&self) -> bool {
        self.offset >= self.max_scroll()
    }

    /// Maximum scroll offset (0 if content fits in viewport).
    #[must_use]
    pub fn max_scroll(&self) -> f32 {
        (self.content_height - self.viewport_height).max(0.0)
    }

    /// Returns `(start_y, end_y)` of the visible range in content coordinates.
    #[must_use]
    pub fn visible_range(&self) -> (f32, f32) {
        (self.offset, self.offset + self.viewport_height)
    }

    /// Returns the scroll position as a fraction from 0.0 (top) to 1.0 (bottom).
    #[must_use]
    pub fn scroll_fraction(&self) -> f32 {
        let max = self.max_scroll();
        if max <= 0.0 {
            0.0
        } else {
            self.offset / max
        }
    }

    fn clamp(&mut self) {
        self.offset = self.offset.clamp(0.0, self.max_scroll());
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_top() {
        let sv = ScrollView::new(500.0, 100.0);
        assert_eq!(sv.offset, 0.0);
        assert!(sv.is_at_top());
        assert!(!sv.is_at_bottom());
    }

    #[test]
    fn scroll_by_positive() {
        let mut sv = ScrollView::new(500.0, 100.0);
        sv.scroll_by(50.0);
        assert!((sv.offset - 50.0).abs() < f32::EPSILON);
        assert!(!sv.is_at_top());
    }

    #[test]
    fn scroll_by_clamps_negative() {
        let mut sv = ScrollView::new(500.0, 100.0);
        sv.scroll_by(-100.0);
        assert_eq!(sv.offset, 0.0);
        assert!(sv.is_at_top());
    }

    #[test]
    fn scroll_by_clamps_past_max() {
        let mut sv = ScrollView::new(500.0, 100.0);
        sv.scroll_by(9999.0);
        assert!((sv.offset - 400.0).abs() < f32::EPSILON);
        assert!(sv.is_at_bottom());
    }

    #[test]
    fn scroll_to_clamps() {
        let mut sv = ScrollView::new(500.0, 100.0);
        sv.scroll_to(-50.0);
        assert_eq!(sv.offset, 0.0);

        sv.scroll_to(1000.0);
        assert!((sv.offset - 400.0).abs() < f32::EPSILON);
    }

    #[test]
    fn max_scroll_content_smaller_than_viewport() {
        let sv = ScrollView::new(50.0, 100.0);
        assert_eq!(sv.max_scroll(), 0.0);
        assert!(sv.is_at_top());
        assert!(sv.is_at_bottom());
    }

    #[test]
    fn visible_range() {
        let mut sv = ScrollView::new(500.0, 100.0);
        sv.scroll_to(100.0);
        let (start, end) = sv.visible_range();
        assert!((start - 100.0).abs() < f32::EPSILON);
        assert!((end - 200.0).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_fraction_at_top() {
        let sv = ScrollView::new(500.0, 100.0);
        assert!((sv.scroll_fraction()).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_fraction_at_bottom() {
        let mut sv = ScrollView::new(500.0, 100.0);
        sv.scroll_to(400.0);
        assert!((sv.scroll_fraction() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_fraction_midway() {
        let mut sv = ScrollView::new(500.0, 100.0);
        sv.scroll_to(200.0); // max=400, so 200/400 = 0.5
        assert!((sv.scroll_fraction() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_fraction_no_scrollable_content() {
        let sv = ScrollView::new(50.0, 100.0);
        assert!((sv.scroll_fraction()).abs() < f32::EPSILON);
    }
}
