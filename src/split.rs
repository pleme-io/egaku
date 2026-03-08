use crate::layout::Rect;

/// Orientation of a split pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

/// Resizable split pane dividing a region into two parts.
#[derive(Debug, Clone)]
pub struct SplitPane {
    ratio: f32,
    orientation: Orientation,
    min_ratio: f32,
}

impl SplitPane {
    /// Create a new split pane with the given orientation, ratio, and minimum ratio.
    #[must_use]
    pub fn new(orientation: Orientation, ratio: f32, min_ratio: f32) -> Self {
        let clamped = ratio.clamp(min_ratio, 1.0 - min_ratio);
        Self { ratio: clamped, orientation, min_ratio }
    }

    /// Create a horizontal split at 50%.
    #[must_use]
    pub fn horizontal() -> Self {
        Self::new(Orientation::Horizontal, 0.5, 0.1)
    }

    /// Create a vertical split at 50%.
    #[must_use]
    pub fn vertical() -> Self {
        Self::new(Orientation::Vertical, 0.5, 0.1)
    }

    /// Resize the split, clamping to `[min_ratio, 1.0 - min_ratio]`.
    pub fn resize(&mut self, ratio: f32) {
        self.ratio = ratio.clamp(self.min_ratio, 1.0 - self.min_ratio);
    }

    /// Returns the current ratio.
    #[must_use]
    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    /// Returns the orientation.
    #[must_use]
    pub fn orientation(&self) -> Orientation {
        self.orientation
    }

    /// Calculate the first pane's rectangle within the given bounds.
    #[must_use]
    pub fn first_rect(&self, bounds: &Rect) -> Rect {
        match self.orientation {
            Orientation::Horizontal => {
                Rect::new(bounds.x, bounds.y, bounds.width * self.ratio, bounds.height)
            }
            Orientation::Vertical => {
                Rect::new(bounds.x, bounds.y, bounds.width, bounds.height * self.ratio)
            }
        }
    }

    /// Calculate the second pane's rectangle within the given bounds.
    #[must_use]
    pub fn second_rect(&self, bounds: &Rect) -> Rect {
        match self.orientation {
            Orientation::Horizontal => {
                let first_w = bounds.width * self.ratio;
                Rect::new(bounds.x + first_w, bounds.y, bounds.width - first_w, bounds.height)
            }
            Orientation::Vertical => {
                let first_h = bounds.height * self.ratio;
                Rect::new(bounds.x, bounds.y + first_h, bounds.width, bounds.height - first_h)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_split_default() {
        let sp = SplitPane::horizontal();
        assert_eq!(sp.orientation(), Orientation::Horizontal);
        assert!((sp.ratio() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn vertical_split_default() {
        let sp = SplitPane::vertical();
        assert_eq!(sp.orientation(), Orientation::Vertical);
        assert!((sp.ratio() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn resize_clamps_to_min() {
        let mut sp = SplitPane::new(Orientation::Horizontal, 0.5, 0.2);
        sp.resize(0.05);
        assert!((sp.ratio() - 0.2).abs() < f32::EPSILON);
    }

    #[test]
    fn resize_clamps_to_max() {
        let mut sp = SplitPane::new(Orientation::Horizontal, 0.5, 0.2);
        sp.resize(0.95);
        assert!((sp.ratio() - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn horizontal_first_rect() {
        let sp = SplitPane::new(Orientation::Horizontal, 0.3, 0.1);
        let bounds = Rect::new(0.0, 0.0, 100.0, 50.0);
        let first = sp.first_rect(&bounds);
        assert!((first.x).abs() < 0.001);
        assert!((first.width - 30.0).abs() < 0.001);
        assert!((first.height - 50.0).abs() < 0.001);
    }

    #[test]
    fn horizontal_second_rect() {
        let sp = SplitPane::new(Orientation::Horizontal, 0.3, 0.1);
        let bounds = Rect::new(0.0, 0.0, 100.0, 50.0);
        let second = sp.second_rect(&bounds);
        assert!((second.x - 30.0).abs() < 0.001);
        assert!((second.width - 70.0).abs() < 0.001);
    }

    #[test]
    fn vertical_first_rect() {
        let sp = SplitPane::new(Orientation::Vertical, 0.4, 0.1);
        let bounds = Rect::new(0.0, 0.0, 100.0, 200.0);
        let first = sp.first_rect(&bounds);
        assert!((first.height - 80.0).abs() < f32::EPSILON);
        assert!((first.width - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn vertical_second_rect() {
        let sp = SplitPane::new(Orientation::Vertical, 0.4, 0.1);
        let bounds = Rect::new(0.0, 0.0, 100.0, 200.0);
        let second = sp.second_rect(&bounds);
        assert!((second.y - 80.0).abs() < f32::EPSILON);
        assert!((second.height - 120.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rects_cover_full_bounds() {
        let sp = SplitPane::new(Orientation::Horizontal, 0.6, 0.1);
        let bounds = Rect::new(10.0, 20.0, 200.0, 100.0);
        let first = sp.first_rect(&bounds);
        let second = sp.second_rect(&bounds);
        // Combined widths should equal bounds width
        assert!((first.width + second.width - bounds.width).abs() < f32::EPSILON);
        // Second starts where first ends
        assert!((second.x - (first.x + first.width)).abs() < f32::EPSILON);
    }
}
