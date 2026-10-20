use serde::{Deserialize, Serialize};

/// A point in world (canvas) coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Pos {
    pub x: f32,
    pub y: f32,
}

impl Pos {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn offset(self, dx: f32, dy: f32) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }

    /// Round to the nearest multiple of `grid`.
    pub fn snapped(self, grid: f32) -> Self {
        if grid <= 0.0 {
            return self;
        }
        Self::new((self.x / grid).round() * grid, (self.y / grid).round() * grid)
    }
}

/// Axis-aligned rectangle in world coordinates (`min` is the top-left corner).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub min: Pos,
    pub size: Pos,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { min: Pos::new(x, y), size: Pos::new(w, h) }
    }

    pub fn max(&self) -> Pos {
        self.min.offset(self.size.x, self.size.y)
    }

    pub fn contains(&self, p: Pos) -> bool {
        let max = self.max();
        p.x >= self.min.x && p.x <= max.x && p.y >= self.min.y && p.y <= max.y
    }

    pub fn contains_rect(&self, other: &Rect) -> bool {
        self.contains(other.min) && self.contains(other.max())
    }

    /// Smallest rectangle covering both.
    /// Overlap with a positive area (touching edges don't count).
    pub fn intersects(&self, other: &Rect) -> bool {
        let (a, b) = (self.max(), other.max());
        self.min.x < b.x && other.min.x < a.x && self.min.y < b.y && other.min.y < a.y
    }

    pub fn union(&self, other: &Rect) -> Rect {
        let (a, b) = (self.max(), other.max());
        let min = Pos::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y));
        let max = Pos::new(a.x.max(b.x), a.y.max(b.y));
        Rect { min, size: Pos::new(max.x - min.x, max.y - min.y) }
    }
}
