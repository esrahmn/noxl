/// A 2D position in logical pixels.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

impl Position {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    pub fn translated(self, dx: f32, dy: f32) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    pub fn distance_squared(self, other: Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }
}

/// A 2D size in logical pixels.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub fn is_finite(self) -> bool {
        self.width.is_finite() && self.height.is_finite()
    }

    pub fn is_valid(self) -> bool {
        self.is_finite() && self.width >= 0.0 && self.height >= 0.0
    }

    pub fn area(self) -> f32 {
        self.width * self.height
    }

    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

/// A 2D rectangle represented by position and size.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Rect {
    pub position: Position,
    pub size: Size,
}

impl Rect {
    pub const ZERO: Self = Self {
        position: Position::ZERO,
        size: Size::ZERO,
    };

    pub const fn new(position: Position, size: Size) -> Self {
        Self { position, size }
    }

    pub const fn from_xywh(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            position: Position::new(x, y),
            size: Size::new(width, height),
        }
    }

    pub fn is_valid(self) -> bool {
        self.position.is_finite() && self.size.is_valid()
    }

    pub fn left(self) -> f32 {
        self.position.x
    }

    pub fn top(self) -> f32 {
        self.position.y
    }

    pub fn right(self) -> f32 {
        self.position.x + self.size.width
    }

    pub fn bottom(self) -> f32 {
        self.position.y + self.size.height
    }

    pub fn center(self) -> Position {
        Position::new(
            self.position.x + self.size.width / 2.0,
            self.position.y + self.size.height / 2.0,
        )
    }

    /// Checks whether a point is inside this rectangle.
    /// Right and bottom edges are exclusive.
    pub fn contains(self, point: Position) -> bool {
        self.is_valid()
            && point.is_finite()
            && point.x >= self.left()
            && point.x < self.right()
            && point.y >= self.top()
            && point.y < self.bottom()
    }

    /// Checks whether two rectangles overlap.
    pub fn intersects(self, other: Self) -> bool {
        self.is_valid()
            && other.is_valid()
            && self.left() < other.right()
            && self.right() > other.left()
            && self.top() < other.bottom()
            && self.bottom() > other.top()
    }

    /// Returns the overlapping area between two rectangles.
    pub fn intersection(self, other: Self) -> Option<Self> {
        if !self.intersects(other) {
            return None;
        }

        let left = self.left().max(other.left());
        let top = self.top().max(other.top());
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());

        Some(Self::from_xywh(left, top, right - left, bottom - top))
    }

    /// Returns the smallest rectangle containing both rectangles.
    pub fn union(self, other: Self) -> Option<Self> {
        if !self.is_valid() || !other.is_valid() {
            return None;
        }

        let left = self.left().min(other.left());
        let top = self.top().min(other.top());
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());

        Some(Self::from_xywh(left, top, right - left, bottom - top))
    }

    pub fn translated(self, dx: f32, dy: f32) -> Self {
        Self {
            position: self.position.translated(dx, dy),
            size: self.size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_stores_coordinates() {
        let position = Position::new(10.0, 20.0);
        assert_eq!(position.x, 10.0);
        assert_eq!(position.y, 20.0);
    }

    #[test]
    fn position_translation_works() {
        let result = Position::new(10.0, 20.0).translated(5.0, -5.0);
        assert_eq!(result, Position::new(15.0, 15.0));
    }

    #[test]
    fn position_rejects_non_finite_values() {
        assert!(!Position::new(f32::NAN, 0.0).is_finite());
        assert!(!Position::new(0.0, f32::INFINITY).is_finite());
    }

    #[test]
    fn size_stores_dimensions() {
        let size = Size::new(100.0, 50.0);
        assert_eq!(size.width, 100.0);
        assert_eq!(size.height, 50.0);
    }

    #[test]
    fn size_validates_dimensions() {
        assert!(Size::new(100.0, 50.0).is_valid());
        assert!(!Size::new(-1.0, 50.0).is_valid());
        assert!(!Size::new(f32::NAN, 50.0).is_valid());
    }

    #[test]
    fn size_calculates_area() {
        assert_eq!(Size::new(10.0, 20.0).area(), 200.0);
    }

    #[test]
    fn size_detects_empty_dimensions() {
        assert!(Size::new(0.0, 20.0).is_empty());
        assert!(!Size::new(10.0, 20.0).is_empty());
    }

    #[test]
    fn rect_stores_position_and_size() {
        let rect = Rect::from_xywh(10.0, 20.0, 100.0, 50.0);
        assert_eq!(rect.position, Position::new(10.0, 20.0));
        assert_eq!(rect.size, Size::new(100.0, 50.0));
    }

    #[test]
    fn rect_calculates_edges() {
        let rect = Rect::from_xywh(10.0, 20.0, 100.0, 50.0);
        assert_eq!(rect.left(), 10.0);
        assert_eq!(rect.top(), 20.0);
        assert_eq!(rect.right(), 110.0);
        assert_eq!(rect.bottom(), 70.0);
    }

    #[test]
    fn rect_calculates_center() {
        let rect = Rect::from_xywh(10.0, 20.0, 100.0, 50.0);
        assert_eq!(rect.center(), Position::new(60.0, 45.0));
    }

    #[test]
    fn rect_contains_point() {
        let rect = Rect::from_xywh(0.0, 0.0, 100.0, 100.0);
        assert!(rect.contains(Position::new(50.0, 50.0)));
        assert!(!rect.contains(Position::new(100.0, 50.0)));
        assert!(!rect.contains(Position::new(-1.0, 50.0)));
    }

    #[test]
    fn rect_detects_intersection() {
        let a = Rect::from_xywh(0.0, 0.0, 100.0, 100.0);
        let b = Rect::from_xywh(50.0, 50.0, 100.0, 100.0);
        assert!(a.intersects(b));
    }

    #[test]
    fn rect_calculates_intersection() {
        let a = Rect::from_xywh(0.0, 0.0, 100.0, 100.0);
        let b = Rect::from_xywh(50.0, 50.0, 100.0, 100.0);

        assert_eq!(
            a.intersection(b),
            Some(Rect::from_xywh(50.0, 50.0, 50.0, 50.0))
        );
    }

    #[test]
    fn rect_returns_none_without_intersection() {
        let a = Rect::from_xywh(0.0, 0.0, 50.0, 50.0);
        let b = Rect::from_xywh(100.0, 100.0, 50.0, 50.0);
        assert_eq!(a.intersection(b), None);
    }

    #[test]
    fn rect_calculates_union() {
        let a = Rect::from_xywh(0.0, 0.0, 50.0, 50.0);
        let b = Rect::from_xywh(25.0, 25.0, 50.0, 50.0);

        assert_eq!(a.union(b), Some(Rect::from_xywh(0.0, 0.0, 75.0, 75.0)));
    }

    #[test]
    fn rect_translation_works() {
        let rect = Rect::from_xywh(10.0, 20.0, 100.0, 50.0);
        assert_eq!(
            rect.translated(5.0, 10.0),
            Rect::from_xywh(15.0, 30.0, 100.0, 50.0)
        );
    }

    #[test]
    fn invalid_rect_does_not_contain_points() {
        let rect = Rect::from_xywh(0.0, 0.0, -10.0, 20.0);
        assert!(!rect.contains(Position::ZERO));
    }
}
