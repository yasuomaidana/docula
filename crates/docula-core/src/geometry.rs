use crate::error::GeometryError;
use serde::{Deserialize, Serialize};

/// Normalized 2D spatial coordinate point within $[0.0, 1.0]$.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    /// Horizontal coordinate, normalized between 0.0 (left) and 1.0 (right).
    pub x: f32,
    /// Vertical coordinate, normalized between 0.0 (top) and 1.0 (bottom).
    pub y: f32,
}

impl Point {
    /// Creates a new `Point`, validating that coordinates fall within $[0.0, 1.0]$.
    pub fn new(x: f32, y: f32) -> Result<Self, GeometryError> {
        if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
            return Err(GeometryError::OutOfBounds {
                msg: format!("Point coordinates ({x}, {y}) exceed [0.0, 1.0]"),
            });
        }
        Ok(Self { x, y })
    }

    /// Creates a new `Point` without bounds validation.
    #[inline]
    pub const fn new_unchecked(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Creates a new `Point` with coordinates clamped to $[0.0, 1.0]$.
    #[inline]
    pub fn clamped(x: f32, y: f32) -> Self {
        Self {
            x: x.clamp(0.0, 1.0),
            y: y.clamp(0.0, 1.0),
        }
    }

    /// Returns `true` if both coordinates lie within the normalized $[0.0, 1.0]$ bounds.
    #[inline]
    pub fn is_normalized(&self) -> bool {
        (0.0..=1.0).contains(&self.x) && (0.0..=1.0).contains(&self.y)
    }

    /// Computes the Euclidean distance to another point in normalized units.
    pub fn distance_to(&self, other: &Point) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Discrete integer pixel bounding rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelRect {
    /// Left horizontal pixel position.
    pub x: u32,
    /// Top vertical pixel position.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl PixelRect {
    /// Constructs a pixel rectangle.
    #[inline]
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Area of the pixel rectangle in square pixels.
    #[inline]
    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    /// Returns the right boundary pixel coordinate ($x + \text{width}$).
    #[inline]
    pub fn right(&self) -> u32 {
        self.x.saturating_add(self.width)
    }

    /// Returns the bottom boundary pixel coordinate ($y + \text{height}$).
    #[inline]
    pub fn bottom(&self) -> u32 {
        self.y.saturating_add(self.height)
    }

    /// Converts pixel coordinates back into normalized $[0.0, 1.0]$ `BoundingBox`.
    pub fn to_bounding_box(
        &self,
        image_width: u32,
        image_height: u32,
    ) -> Result<BoundingBox, GeometryError> {
        if image_width == 0 || image_height == 0 {
            return Err(GeometryError::ZeroImageDimensions {
                width: image_width,
                height: image_height,
            });
        }

        let w_f = image_width as f32;
        let h_f = image_height as f32;

        let top_left = Point::clamped(self.x as f32 / w_f, self.y as f32 / h_f);
        let bottom_right = Point::clamped(self.right() as f32 / w_f, self.bottom() as f32 / h_f);

        BoundingBox::new(top_left, bottom_right)
    }
}

/// Normalized axis-aligned bounding box defined by `top_left` and `bottom_right` corners.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    /// Top-left point ($x_0, y_0$).
    pub top_left: Point,
    /// Bottom-right point ($x_1, y_1$).
    pub bottom_right: Point,
}

impl BoundingBox {
    /// Creates a validated bounding box where coordinates are normalized and $x_0 \le x_1$, $y_0 \le y_1$.
    pub fn new(top_left: Point, bottom_right: Point) -> Result<Self, GeometryError> {
        if !top_left.is_normalized() || !bottom_right.is_normalized() {
            return Err(GeometryError::OutOfBounds {
                msg: format!(
                    "Bounding box corners out of bounds: ({:?}, {:?})",
                    top_left, bottom_right
                ),
            });
        }

        if top_left.x > bottom_right.x || top_left.y > bottom_right.y {
            return Err(GeometryError::InvertedBounds {
                msg: format!(
                    "Inverted bounds: top_left ({}, {}) > bottom_right ({}, {})",
                    top_left.x, top_left.y, bottom_right.x, bottom_right.y
                ),
            });
        }

        Ok(Self {
            top_left,
            bottom_right,
        })
    }

    /// Constructs a bounding box directly from 4 normalized coordinates.
    pub fn from_coords(x0: f32, y0: f32, x1: f32, y1: f32) -> Result<Self, GeometryError> {
        let tl = Point::new(x0, y0)?;
        let br = Point::new(x1, y1)?;
        Self::new(tl, br)
    }

    /// Constructs a bounding box from arbitrary coordinates, safely ordering and clamping to $[0.0, 1.0]$.
    pub fn from_coords_clamped(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        let min_x = x0.min(x1).clamp(0.0, 1.0);
        let min_y = y0.min(y1).clamp(0.0, 1.0);
        let max_x = x0.max(x1).clamp(0.0, 1.0);
        let max_y = y0.max(y1).clamp(0.0, 1.0);

        Self {
            top_left: Point::new_unchecked(min_x, min_y),
            bottom_right: Point::new_unchecked(max_x, max_y),
        }
    }

    /// Normalized width ($x_1 - x_0$).
    #[inline]
    pub fn width(&self) -> f32 {
        self.bottom_right.x - self.top_left.x
    }

    /// Normalized height ($y_1 - y_0$).
    #[inline]
    pub fn height(&self) -> f32 {
        self.bottom_right.y - self.top_left.y
    }

    /// Normalized surface area ($w \times h$).
    #[inline]
    pub fn area(&self) -> f32 {
        self.width() * self.height()
    }

    /// Aspect ratio ($\text{width} / \text{height}$).
    pub fn aspect_ratio(&self) -> f32 {
        let h = self.height();
        if h > 0.0 { self.width() / h } else { 0.0 }
    }

    /// Center point (centroid) of the bounding box.
    #[inline]
    pub fn centroid(&self) -> Point {
        Point::new_unchecked(
            (self.top_left.x + self.bottom_right.x) * 0.5,
            (self.top_left.y + self.bottom_right.y) * 0.5,
        )
    }

    /// Checks if a point falls inside the bounding box.
    pub fn contains_point(&self, point: &Point) -> bool {
        point.x >= self.top_left.x
            && point.x <= self.bottom_right.x
            && point.y >= self.top_left.y
            && point.y <= self.bottom_right.y
    }

    /// Checks if another bounding box is completely contained within this box.
    pub fn contains_box(&self, other: &BoundingBox) -> bool {
        other.top_left.x >= self.top_left.x
            && other.top_left.y >= self.top_left.y
            && other.bottom_right.x <= self.bottom_right.x
            && other.bottom_right.y <= self.bottom_right.y
    }

    /// Returns `true` if this box intersects or overlaps with another bounding box.
    pub fn overlaps(&self, other: &BoundingBox) -> bool {
        self.top_left.x < other.bottom_right.x
            && self.bottom_right.x > other.top_left.x
            && self.top_left.y < other.bottom_right.y
            && self.bottom_right.y > other.top_left.y
    }

    /// Computes the intersection of two bounding boxes. Returns `None` if they are disjoint.
    pub fn intersection(&self, other: &BoundingBox) -> Option<BoundingBox> {
        let max_tl_x = self.top_left.x.max(other.top_left.x);
        let max_tl_y = self.top_left.y.max(other.top_left.y);
        let min_br_x = self.bottom_right.x.min(other.bottom_right.x);
        let min_br_y = self.bottom_right.y.min(other.bottom_right.y);

        if max_tl_x < min_br_x && max_tl_y < min_br_y {
            Some(BoundingBox {
                top_left: Point::new_unchecked(max_tl_x, max_tl_y),
                bottom_right: Point::new_unchecked(min_br_x, min_br_y),
            })
        } else {
            None
        }
    }

    /// Computes the minimal bounding box enclosing both this and another bounding box.
    pub fn union(&self, other: &BoundingBox) -> BoundingBox {
        BoundingBox {
            top_left: Point::new_unchecked(
                self.top_left.x.min(other.top_left.x),
                self.top_left.y.min(other.top_left.y),
            ),
            bottom_right: Point::new_unchecked(
                self.bottom_right.x.max(other.bottom_right.x),
                self.bottom_right.y.max(other.bottom_right.y),
            ),
        }
    }

    /// Expands the bounding box by uniform percentage padding (e.g. 0.05 for 5% margin), clamped to $[0.0, 1.0]$.
    pub fn with_padding(&self, padding_fraction: f32) -> Self {
        let w = self.width();
        let h = self.height();
        let pad_x = w * padding_fraction;
        let pad_y = h * padding_fraction;

        let x0 = (self.top_left.x - pad_x).clamp(0.0, 1.0);
        let y0 = (self.top_left.y - pad_y).clamp(0.0, 1.0);
        let x1 = (self.bottom_right.x + pad_x).clamp(0.0, 1.0);
        let y1 = (self.bottom_right.y + pad_y).clamp(0.0, 1.0);

        Self::from_coords_clamped(x0, y0, x1, y1)
    }

    /// Projects normalized $[0.0, 1.0]$ coordinates into integer pixel coordinates.
    pub fn to_pixel_rect(&self, image_width: u32, image_height: u32) -> PixelRect {
        let w_f = image_width as f32;
        let h_f = image_height as f32;

        let x0 = (self.top_left.x * w_f).round().clamp(0.0, w_f) as u32;
        let y0 = (self.top_left.y * h_f).round().clamp(0.0, h_f) as u32;
        let x1 = (self.bottom_right.x * w_f).round().clamp(0.0, w_f) as u32;
        let y1 = (self.bottom_right.y * h_f).round().clamp(0.0, h_f) as u32;

        PixelRect {
            x: x0,
            y: y0,
            width: x1.saturating_sub(x0).max(1),
            height: y1.saturating_sub(y0).max(1),
        }
    }
}

/// 2D polygonal contour represented by ordered vertices.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Polygon {
    /// Ordered vertices forming the polygon boundary.
    pub vertices: Vec<Point>,
}

impl Polygon {
    /// Creates a new polygon, requiring at least 3 vertices.
    pub fn new(vertices: Vec<Point>) -> Result<Self, GeometryError> {
        if vertices.len() < 3 {
            return Err(GeometryError::DegeneratePolygon {
                msg: format!(
                    "Polygon requires at least 3 vertices, got {}",
                    vertices.len()
                ),
            });
        }
        Ok(Self { vertices })
    }

    /// Constructs a quadrilateral polygon from 4 corner points.
    pub fn from_quad(p0: Point, p1: Point, p2: Point, p3: Point) -> Self {
        Self {
            vertices: vec![p0, p1, p2, p3],
        }
    }

    /// Computes the minimal axis-aligned `BoundingBox` enclosing all vertices.
    pub fn bounding_box(&self) -> BoundingBox {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for p in &self.vertices {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }

        BoundingBox::from_coords_clamped(min_x, min_y, max_x, max_y)
    }

    /// Computes the polygon area using the Shoelace formula.
    pub fn area(&self) -> f32 {
        let n = self.vertices.len();
        if n < 3 {
            return 0.0;
        }

        let mut sum = 0.0;
        for i in 0..n {
            let j = (i + 1) % n;
            sum += self.vertices[i].x * self.vertices[j].y;
            sum -= self.vertices[j].x * self.vertices[i].y;
        }

        (sum * 0.5).abs()
    }

    /// Computes the centroid point of the polygon.
    pub fn centroid(&self) -> Point {
        let n = self.vertices.len();
        if n == 0 {
            return Point::new_unchecked(0.0, 0.0);
        }

        let mut cx = 0.0;
        let mut cy = 0.0;
        for p in &self.vertices {
            cx += p.x;
            cy += p.y;
        }

        Point::clamped(cx / n as f32, cy / n as f32)
    }
}

/// Text token or line associated with a spatial bounding box on a document page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpatialSpan {
    /// Extracted textual content.
    pub text: String,
    /// Spatial bounding box on the page.
    pub bbox: BoundingBox,
    /// 0-indexed page number.
    pub page_index: usize,
    /// Recognition or extraction confidence score $[0.0, 1.0]$.
    pub confidence: f32,
}

impl SpatialSpan {
    /// Constructs a validated spatial span.
    pub fn new(
        text: impl Into<String>,
        bbox: BoundingBox,
        page_index: usize,
        confidence: f32,
    ) -> Result<Self, GeometryError> {
        if !(0.0..=1.0).contains(&confidence) {
            return Err(GeometryError::InvalidConfidence {
                score: confidence.to_string(),
            });
        }

        Ok(Self {
            text: text.into(),
            bbox,
            page_index,
            confidence,
        })
    }

    /// Constructs a spatial span with confidence clamped to $[0.0, 1.0]$.
    pub fn with_clamped_confidence(
        text: impl Into<String>,
        bbox: BoundingBox,
        page_index: usize,
        confidence: f32,
    ) -> Self {
        Self {
            text: text.into(),
            bbox,
            page_index,
            confidence: confidence.clamp(0.0, 1.0),
        }
    }
}
