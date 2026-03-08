/// Axis-aligned rectangle for layout calculations.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    #[must_use]
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    #[must_use]
    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0, width: 0.0, height: 0.0 }
    }

    #[must_use]
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }

    /// Returns true if this rectangle overlaps with `other`.
    #[must_use]
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    /// Returns the smallest rectangle that contains both `self` and `other`.
    #[must_use]
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = (self.x + self.width).max(other.x + other.width);
        let bottom = (self.y + self.height).max(other.y + other.height);
        Rect::new(x, y, right - x, bottom - y)
    }

    /// Inset the rectangle by uniform padding.
    #[must_use]
    pub fn inset(&self, amount: f32) -> Self {
        Self {
            x: self.x + amount,
            y: self.y + amount,
            width: (self.width - 2.0 * amount).max(0.0),
            height: (self.height - 2.0 * amount).max(0.0),
        }
    }

    /// Returns the center point of the rectangle as `(cx, cy)`.
    #[must_use]
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Split horizontally at a ratio (0.0..1.0), returning (left, right).
    #[must_use]
    pub fn split_h(&self, ratio: f32) -> (Self, Self) {
        let left_w = self.width * ratio;
        (
            Self { width: left_w, ..*self },
            Self { x: self.x + left_w, width: self.width - left_w, ..*self },
        )
    }

    /// Split vertically at a ratio (0.0..1.0), returning (top, bottom).
    #[must_use]
    pub fn split_v(&self, ratio: f32) -> (Self, Self) {
        let top_h = self.height * ratio;
        (
            Self { height: top_h, ..*self },
            Self { y: self.y + top_h, height: self.height - top_h, ..*self },
        )
    }
}

/// Padding specification for layout calculations.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Padding {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Padding {
    #[must_use]
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self { top, right, bottom, left }
    }

    /// Create uniform padding on all sides.
    #[must_use]
    pub const fn uniform(v: f32) -> Self {
        Self { top: v, right: v, bottom: v, left: v }
    }

    /// Create symmetric padding: `h` for left/right, `v` for top/bottom.
    #[must_use]
    pub const fn symmetric(h: f32, v: f32) -> Self {
        Self { top: v, right: h, bottom: v, left: h }
    }

    /// Total horizontal padding (left + right).
    #[must_use]
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// Total vertical padding (top + bottom).
    #[must_use]
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn rect_new_and_zero() {
        let r = Rect::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(r.x, 1.0);
        assert_eq!(r.y, 2.0);
        assert_eq!(r.width, 3.0);
        assert_eq!(r.height, 4.0);

        let z = Rect::zero();
        assert_eq!(z, Rect::new(0.0, 0.0, 0.0, 0.0));
    }

    #[test]
    fn rect_contains_inside() {
        let r = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert!(r.contains(50.0, 30.0));
    }

    #[test]
    fn rect_contains_edges() {
        let r = Rect::new(10.0, 10.0, 100.0, 50.0);
        // corners
        assert!(r.contains(10.0, 10.0));
        assert!(r.contains(110.0, 60.0));
    }

    #[test]
    fn rect_contains_outside() {
        let r = Rect::new(10.0, 10.0, 100.0, 50.0);
        assert!(!r.contains(9.0, 30.0));
        assert!(!r.contains(111.0, 30.0));
        assert!(!r.contains(50.0, 9.0));
        assert!(!r.contains(50.0, 61.0));
    }

    #[test]
    fn rect_intersects_overlap() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        assert!(a.intersects(&b));
        assert!(b.intersects(&a));
    }

    #[test]
    fn rect_intersects_no_overlap() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(20.0, 20.0, 10.0, 10.0);
        assert!(!a.intersects(&b));
    }

    #[test]
    fn rect_intersects_touching_edges() {
        // Touching edges are NOT overlapping (strict inequality)
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(10.0, 0.0, 10.0, 10.0);
        assert!(!a.intersects(&b));
    }

    #[test]
    fn rect_union() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(5.0, 5.0, 10.0, 10.0);
        let u = a.union(&b);
        assert_eq!(u, Rect::new(0.0, 0.0, 15.0, 15.0));
    }

    #[test]
    fn rect_union_disjoint() {
        let a = Rect::new(0.0, 0.0, 5.0, 5.0);
        let b = Rect::new(10.0, 10.0, 5.0, 5.0);
        let u = a.union(&b);
        assert_eq!(u, Rect::new(0.0, 0.0, 15.0, 15.0));
    }

    #[test]
    fn rect_inset() {
        let r = Rect::new(0.0, 0.0, 100.0, 80.0);
        let i = r.inset(10.0);
        assert_eq!(i, Rect::new(10.0, 10.0, 80.0, 60.0));
    }

    #[test]
    fn rect_inset_clamps_to_zero() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        let i = r.inset(20.0);
        assert_eq!(i.width, 0.0);
        assert_eq!(i.height, 0.0);
    }

    #[test]
    fn rect_center() {
        let r = Rect::new(10.0, 20.0, 100.0, 50.0);
        let (cx, cy) = r.center();
        assert!((cx - 60.0).abs() < f32::EPSILON);
        assert!((cy - 45.0).abs() < f32::EPSILON);
    }

    #[test]
    fn rect_split_h() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        let (left, right) = r.split_h(0.3);
        assert!((left.width - 30.0).abs() < 0.001);
        assert!((right.x - 30.0).abs() < 0.001);
        assert!((right.width - 70.0).abs() < 0.001);
    }

    #[test]
    fn rect_split_v() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        let (top, bottom) = r.split_v(0.4);
        assert!((top.height - 20.0).abs() < f32::EPSILON);
        assert!((bottom.y - 20.0).abs() < f32::EPSILON);
        assert!((bottom.height - 30.0).abs() < f32::EPSILON);
    }

    #[test]
    fn padding_uniform() {
        let p = Padding::uniform(8.0);
        assert_eq!(p.top, 8.0);
        assert_eq!(p.right, 8.0);
        assert_eq!(p.bottom, 8.0);
        assert_eq!(p.left, 8.0);
    }

    #[test]
    fn padding_symmetric() {
        let p = Padding::symmetric(10.0, 5.0);
        assert_eq!(p.left, 10.0);
        assert_eq!(p.right, 10.0);
        assert_eq!(p.top, 5.0);
        assert_eq!(p.bottom, 5.0);
    }

    #[test]
    fn padding_totals() {
        let p = Padding::new(1.0, 2.0, 3.0, 4.0);
        assert!((p.horizontal() - 6.0).abs() < f32::EPSILON);
        assert!((p.vertical() - 4.0).abs() < f32::EPSILON);
    }
}
