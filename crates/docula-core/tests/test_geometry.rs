use docula_core::error::GeometryError;
use docula_core::geometry::{BoundingBox, PixelRect, Point, Polygon, SpatialSpan};

#[test]
fn test_point_creation_and_bounds() {
    let valid = Point::new(0.25, 0.75).expect("valid point within [0, 1]");
    assert!((valid.x - 0.25).abs() < f32::EPSILON);
    assert!((valid.y - 0.75).abs() < f32::EPSILON);
    assert!(valid.is_normalized());

    // Out of bounds tests
    assert!(matches!(
        Point::new(-0.01, 0.5),
        Err(GeometryError::OutOfBounds { .. })
    ));
    assert!(matches!(
        Point::new(0.5, 1.01),
        Err(GeometryError::OutOfBounds { .. })
    ));

    // Clamping
    let clamped = Point::clamped(-0.5, 1.5);
    assert_eq!(clamped.x, 0.0);
    assert_eq!(clamped.y, 1.0);
}

#[test]
fn test_point_distance() {
    let p1 = Point::new_unchecked(0.0, 0.0);
    let p2 = Point::new_unchecked(0.3, 0.4);
    assert!((p1.distance_to(&p2) - 0.5).abs() < 1e-5);
}

#[test]
fn test_pixel_rect_computations() {
    let rect = PixelRect::new(100, 200, 300, 400);
    assert_eq!(rect.right(), 400);
    assert_eq!(rect.bottom(), 600);
    assert_eq!(rect.area(), 120_000);

    // Convert to normalized bounding box
    let bbox = rect.to_bounding_box(1000, 1000).expect("valid conversion");
    assert!((bbox.top_left.x - 0.1).abs() < 1e-5);
    assert!((bbox.top_left.y - 0.2).abs() < 1e-5);
    assert!((bbox.bottom_right.x - 0.4).abs() < 1e-5);
    assert!((bbox.bottom_right.y - 0.6).abs() < 1e-5);

    // Zero image dimensions should fail
    assert!(matches!(
        rect.to_bounding_box(0, 1000),
        Err(GeometryError::ZeroImageDimensions { .. })
    ));
    assert!(matches!(
        rect.to_bounding_box(1000, 0),
        Err(GeometryError::ZeroImageDimensions { .. })
    ));
}

#[test]
fn test_bbox_validation_and_invariants() {
    // Valid bbox
    let bbox = BoundingBox::from_coords(0.1, 0.2, 0.6, 0.8).expect("valid coordinates");
    assert!((bbox.width() - 0.5).abs() < 1e-5);
    assert!((bbox.height() - 0.6).abs() < 1e-5);
    assert!((bbox.area() - 0.3).abs() < 1e-5);
    assert!((bbox.aspect_ratio() - (0.5 / 0.6)).abs() < 1e-5);

    // Inverted bounds should error
    let inverted_x = BoundingBox::from_coords(0.7, 0.2, 0.3, 0.8);
    assert!(matches!(
        inverted_x,
        Err(GeometryError::InvertedBounds { .. })
    ));

    let inverted_y = BoundingBox::from_coords(0.1, 0.9, 0.6, 0.2);
    assert!(matches!(
        inverted_y,
        Err(GeometryError::InvertedBounds { .. })
    ));

    // Out of bounds coordinates
    let oob = BoundingBox::from_coords(-0.1, 0.0, 0.5, 0.5);
    assert!(matches!(oob, Err(GeometryError::OutOfBounds { .. })));
}

#[test]
fn test_bbox_intersection_disjoint() {
    let b1 = BoundingBox::from_coords(0.0, 0.0, 0.3, 0.3).unwrap();
    let b2 = BoundingBox::from_coords(0.4, 0.4, 0.8, 0.8).unwrap();

    assert!(!b1.overlaps(&b2));
    assert_eq!(b1.intersection(&b2), None);
}

#[test]
fn test_bbox_intersection_overlapping() {
    let b1 = BoundingBox::from_coords(0.1, 0.1, 0.5, 0.5).unwrap();
    let b2 = BoundingBox::from_coords(0.3, 0.3, 0.7, 0.7).unwrap();

    assert!(b1.overlaps(&b2));
    let inter = b1.intersection(&b2).expect("intersection exists");
    assert!((inter.top_left.x - 0.3).abs() < 1e-5);
    assert!((inter.top_left.y - 0.3).abs() < 1e-5);
    assert!((inter.bottom_right.x - 0.5).abs() < 1e-5);
    assert!((inter.bottom_right.y - 0.5).abs() < 1e-5);
}

#[test]
fn test_bbox_intersection_contained() {
    let outer = BoundingBox::from_coords(0.1, 0.1, 0.9, 0.9).unwrap();
    let inner = BoundingBox::from_coords(0.3, 0.3, 0.6, 0.6).unwrap();

    assert!(outer.contains_box(&inner));
    assert!(!inner.contains_box(&outer));

    let inter = outer.intersection(&inner).expect("intersection exists");
    assert_eq!(inter, inner);
}

#[test]
fn test_bbox_union() {
    let b1 = BoundingBox::from_coords(0.1, 0.2, 0.4, 0.5).unwrap();
    let b2 = BoundingBox::from_coords(0.3, 0.1, 0.7, 0.8).unwrap();

    let u = b1.union(&b2);
    assert!((u.top_left.x - 0.1).abs() < 1e-5);
    assert!((u.top_left.y - 0.1).abs() < 1e-5);
    assert!((u.bottom_right.x - 0.7).abs() < 1e-5);
    assert!((u.bottom_right.y - 0.8).abs() < 1e-5);
}

#[test]
fn test_bbox_to_pixel_rect_clamping() {
    let bbox = BoundingBox::from_coords(0.0, 0.0, 1.0, 1.0).unwrap();
    let rect = bbox.to_pixel_rect(1920, 1080);

    assert_eq!(rect.x, 0);
    assert_eq!(rect.y, 0);
    assert_eq!(rect.width, 1920);
    assert_eq!(rect.height, 1080);

    // Box clamped from extreme coordinates
    let clamped_box = BoundingBox::from_coords_clamped(-0.5, -0.2, 1.5, 2.0);
    let rect_clamped = clamped_box.to_pixel_rect(800, 600);
    assert_eq!(rect_clamped.x, 0);
    assert_eq!(rect_clamped.y, 0);
    assert_eq!(rect_clamped.width, 800);
    assert_eq!(rect_clamped.height, 600);
}

#[test]
fn test_bbox_with_padding() {
    let bbox = BoundingBox::from_coords(0.2, 0.2, 0.4, 0.4).unwrap();
    // Width = 0.2, height = 0.2. 10% padding = 0.02 expansion on each side.
    let padded = bbox.with_padding(0.1);

    assert!((padded.top_left.x - 0.18).abs() < 1e-5);
    assert!((padded.top_left.y - 0.18).abs() < 1e-5);
    assert!((padded.bottom_right.x - 0.42).abs() < 1e-5);
    assert!((padded.bottom_right.y - 0.42).abs() < 1e-5);

    // Padding clamped at boundary
    let edge_box = BoundingBox::from_coords(0.05, 0.05, 0.95, 0.95).unwrap();
    let over_padded = edge_box.with_padding(0.5);
    assert_eq!(over_padded.top_left.x, 0.0);
    assert_eq!(over_padded.top_left.y, 0.0);
    assert_eq!(over_padded.bottom_right.x, 1.0);
    assert_eq!(over_padded.bottom_right.y, 1.0);
}

#[test]
fn test_bbox_centroid_and_containment() {
    let bbox = BoundingBox::from_coords(0.2, 0.4, 0.6, 0.8).unwrap();
    let center = bbox.centroid();
    assert!((center.x - 0.4).abs() < 1e-5);
    assert!((center.y - 0.6).abs() < 1e-5);

    assert!(bbox.contains_point(&Point::new_unchecked(0.3, 0.5)));
    assert!(!bbox.contains_point(&Point::new_unchecked(0.1, 0.5)));
    assert!(!bbox.contains_point(&Point::new_unchecked(0.3, 0.9)));
}

#[test]
fn test_polygon_shoelace_and_bounding_box() {
    let p0 = Point::new_unchecked(0.1, 0.1);
    let p1 = Point::new_unchecked(0.5, 0.1);
    let p2 = Point::new_unchecked(0.5, 0.5);
    let p3 = Point::new_unchecked(0.1, 0.5);

    let poly = Polygon::from_quad(p0, p1, p2, p3);
    assert_eq!(poly.vertices.len(), 4);

    // Shoelace area: 0.4 x 0.4 = 0.16
    assert!((poly.area() - 0.16).abs() < 1e-5);

    // Centroid: (0.3, 0.3)
    let c = poly.centroid();
    assert!((c.x - 0.3).abs() < 1e-5);
    assert!((c.y - 0.3).abs() < 1e-5);

    // Enclosing bounding box
    let poly_bbox = poly.bounding_box();
    assert!((poly_bbox.top_left.x - 0.1).abs() < 1e-5);
    assert!((poly_bbox.top_left.y - 0.1).abs() < 1e-5);
    assert!((poly_bbox.bottom_right.x - 0.5).abs() < 1e-5);
    assert!((poly_bbox.bottom_right.y - 0.5).abs() < 1e-5);

    // Degenerate polygon (< 3 vertices)
    let bad_poly = Polygon::new(vec![p0, p1]);
    assert!(matches!(
        bad_poly,
        Err(GeometryError::DegeneratePolygon { .. })
    ));
}

#[test]
fn test_spatial_span_creation_and_confidence() {
    let bbox = BoundingBox::from_coords(0.1, 0.1, 0.5, 0.2).unwrap();
    let span = SpatialSpan::new("Invoice #12345", bbox, 0, 0.98).expect("valid span");

    assert_eq!(span.text, "Invoice #12345");
    assert_eq!(span.page_index, 0);
    assert!((span.confidence - 0.98).abs() < 1e-5);

    // Confidence out of range
    let bad_span = SpatialSpan::new("Test", bbox, 0, 1.05);
    assert!(matches!(
        bad_span,
        Err(GeometryError::InvalidConfidence { .. })
    ));

    let clamped_span = SpatialSpan::with_clamped_confidence("Test", bbox, 0, 1.5);
    assert_eq!(clamped_span.confidence, 1.0);
}
