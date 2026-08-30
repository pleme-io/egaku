use crate::layout::Rect;

use ishou_tokens::{Bounds, Refined};

/// The unit interval, with NaN falling to a centred split.
///
/// ★ `Refined::new` tests for IN-range first, so NaN — which compares false
/// against everything — lands on `default()`. That ordering is the entire NaN
/// fix; a bounds check written as "reject if out of range" would let NaN
/// through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SplitRatioBounds;

impl Bounds<f32> for SplitRatioBounds {
    fn min() -> f32 {
        0.0
    }
    fn max() -> f32 {
        1.0
    }
    fn default() -> f32 {
        0.5
    }
}

/// A minimum-ratio, capped at 0.5.
///
/// ★ THE CAP IS THE PANIC FIX. The old code computed
/// `ratio.clamp(min_ratio, 1.0 - min_ratio)`, and `f32::clamp` **panics** when
/// `min > max`. Any `min_ratio > 0.5` makes `1.0 - min_ratio < min_ratio`, so
/// `SplitPane::new(o, 0.5, 0.6)` was a reachable panic in a drawing library.
/// Capping the minimum at 0.5 makes the inverted window unconstructible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MinRatioBounds;

impl Bounds<f32> for MinRatioBounds {
    fn min() -> f32 {
        0.0
    }
    fn max() -> f32 {
        0.5
    }
    fn default() -> f32 {
        0.0
    }
}

/// Refine FIRST, then narrow — the ordering is load-bearing.
///
/// Refining both values against static bounds removes NaN and guarantees
/// `lo <= hi`, so the narrowing `clamp` below can no longer panic. Doing it the
/// other way round — narrowing a raw `f32` and refining after — is exactly the
/// old code, and preserves both defects.
fn refine_ratio(ratio: f32, min_ratio: f32) -> f32 {
    let min = Refined::<f32, MinRatioBounds>::new(min_ratio).get();
    let r = Refined::<f32, SplitRatioBounds>::new(ratio).get();
    // `min <= 0.5 <= 1.0 - min` holds by construction, so this cannot panic.
    r.clamp(min, 1.0 - min)
}

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
        let clamped = refine_ratio(ratio, min_ratio);
        Self {
            ratio: clamped,
            orientation,
            min_ratio,
        }
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
        self.ratio = refine_ratio(ratio, self.min_ratio);
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
                Rect::new(
                    bounds.x + first_w,
                    bounds.y,
                    bounds.width - first_w,
                    bounds.height,
                )
            }
            Orientation::Vertical => {
                let first_h = bounds.height * self.ratio;
                Rect::new(
                    bounds.x,
                    bounds.y + first_h,
                    bounds.width,
                    bounds.height - first_h,
                )
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

    /// ★ THE NaN DEFECT. `f32::NAN.clamp(0.1, 0.9)` returns NaN, and the old
    /// `new` clamped exactly that way — so a NaN ratio flowed into
    /// `bounds.width * self.ratio` and every downstream rect became NaN. A
    /// NaN width does not panic and does not draw; it silently renders
    /// nothing, which is the worst failure shape available.
    #[test]
    fn a_nan_ratio_cannot_reach_geometry() {
        let sp = SplitPane::new(Orientation::Horizontal, f32::NAN, 0.1);
        assert!(sp.ratio().is_finite(), "NaN survived into the ratio");
        assert_eq!(sp.ratio(), 0.5, "NaN should land on a centred split");

        let mut sp = SplitPane::new(Orientation::Vertical, 0.5, 0.1);
        sp.resize(f32::NAN);
        assert!(sp.ratio().is_finite(), "NaN survived a resize");
    }

    /// ★ THE PANIC DEFECT. `f32::clamp` PANICS when `min > max`, and the old
    /// code computed `ratio.clamp(min_ratio, 1.0 - min_ratio)` — so any
    /// `min_ratio > 0.5` inverted the window and took the process down. A
    /// drawing library panicking on a plausible argument is not a bad value,
    /// it is a crash.
    #[test]
    fn a_min_ratio_above_one_half_does_not_panic() {
        let sp = SplitPane::new(Orientation::Horizontal, 0.5, 0.6);
        assert!(sp.ratio().is_finite());
        let sp = SplitPane::new(Orientation::Horizontal, 0.5, 42.0);
        assert!(sp.ratio().is_finite());
    }

    /// A NaN minimum is the other `clamp` panic arm (`clamp(_, NaN, NaN)`).
    #[test]
    fn a_nan_min_ratio_does_not_panic() {
        let sp = SplitPane::new(Orientation::Vertical, 0.5, f32::NAN);
        assert!(sp.ratio().is_finite());
    }

    /// The refinement must not disturb ordinary values — a guard that changes
    /// correct inputs has traded one defect for another.
    #[test]
    fn ordinary_ratios_are_untouched() {
        let sp = SplitPane::new(Orientation::Horizontal, 0.25, 0.1);
        assert!((sp.ratio() - 0.25).abs() < f32::EPSILON);
        let sp = SplitPane::new(Orientation::Horizontal, 0.05, 0.1);
        assert!(
            (sp.ratio() - 0.1).abs() < f32::EPSILON,
            "still narrows to min"
        );
    }
}
