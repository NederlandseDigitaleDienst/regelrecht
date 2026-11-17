//! Shapes: two-dimensional geometries a law reasons about (schema v0.8.23).
//!
//! A shape is a string of well-known text (WKT), such as
//! `POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))` or `POINT(155105 463625)`. The
//! engine attaches no coordinate system to it: the numbers are taken as they
//! are written, so two shapes that meet in an operation must be in the same
//! system. Parsing is the `wkt` crate's and the geometry the `geo` crate's;
//! this module only joins the two and says what it refuses.

use geo::{CoordsIter, Geometry, HasDimensions, Relate, Validation};
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;
use wkt::Wkt;
use wkt::types::Dimension;

/// The DE-9IM pattern of `OVERLAPS`: the interiors of the two shapes have at
/// least one point in common, whatever their boundaries do.
const INTERIORS_MEET: &str = "T********";

/// The largest absolute value a coordinate of a shape may have. A position on
/// earth stays far below it in any unit a law would use: the circumference
/// of the earth is 4e10 millimetres. Up to this bound a coordinate is exact
/// to a ten-thousandth of its unit (it stays below 2^53 with room to spare)
/// and the products of coordinate differences the relation is computed from
/// stay near 1e24, far from the overflow that made corners at 1e308 relate
/// to nothing.
const MAX_COORDINATE: f64 = 1e12;

/// The longest piece of an unreadable shape a message quotes. A boundary can
/// run to megabytes, and the start is what tells a reader what was passed.
const QUOTED_CHARS: usize = 60;

/// Read a shape from its well-known text.
///
/// # Errors
///
/// Says why the text is no shape: it does not parse as WKT, it has a third
/// dimension or a measure (`POINT Z`, `POINT M`), or the geometry it
/// describes is not valid, such as a polygon whose boundary crosses itself.
/// An invalid geometry is refused because the relation between two shapes is
/// only defined for valid ones. A shape is a point, a line or a surface, or
/// several of one kind, so a `GEOMETRYCOLLECTION` is refused, and so is an
/// empty geometry (`POINT EMPTY`): a place that is nowhere overlaps nothing,
/// and that would read as an answer. A coordinate beyond
/// [`MAX_COORDINATE`], or one that is no number, is refused because the
/// relation is not reliable there.
pub(crate) fn parse(text: &str) -> Result<Geometry<f64>, String> {
    let quoted = quote(text);
    let parsed =
        Wkt::<f64>::from_str(text).map_err(|e| format!("'{quoted}' does not parse as WKT: {e}"))?;
    if parsed.dimension() != Dimension::XY {
        return Err(format!(
            "'{quoted}' is not two-dimensional; a shape has an x and a y per point"
        ));
    }
    let geometry =
        Geometry::try_from(parsed).map_err(|e| format!("'{quoted}' does not parse as WKT: {e}"))?;
    if matches!(geometry, Geometry::GeometryCollection(_)) {
        return Err(format!(
            "'{quoted}' is a collection of geometries; a shape is a point, a line or a surface"
        ));
    }
    if geometry.is_empty() {
        return Err(format!(
            "'{quoted}' is an empty geometry; a shape has at least one point"
        ));
    }
    // Written so that a coordinate that is no number fails the test too.
    let in_bounds = |value: f64| value.abs() <= MAX_COORDINATE;
    if !geometry
        .coords_iter()
        .all(|c| in_bounds(c.x) && in_bounds(c.y))
    {
        return Err(format!(
            "'{quoted}' has a coordinate beyond {MAX_COORDINATE:e}; the relation between \
             shapes is not reliable that far out"
        ));
    }
    geometry
        .check_validation()
        .map_err(|e| format!("'{quoted}' is not a valid geometry: {e}"))?;
    Ok(geometry)
}

/// What reading a text as a shape came to: the shape, shared, or why it is
/// none.
type Parsed = Result<Rc<Geometry<f64>>, String>;

/// The shapes one execution has read, by their text, so a text is parsed
/// and validated once however often an operation or an open term meets it.
/// A text that is no shape is remembered with its refusal.
#[derive(Debug, Default)]
pub struct ShapeCache(RefCell<HashMap<String, Parsed>>);

impl ShapeCache {
    /// The shape `text` describes, parsed on its first use.
    ///
    /// # Errors
    ///
    /// When `text` is no shape, for the reasons [`parse`] gives.
    pub(crate) fn parse(&self, text: &str) -> Parsed {
        if let Some(known) = self.0.borrow().get(text) {
            return known.clone();
        }
        let parsed = parse(text).map(Rc::new);
        self.0.borrow_mut().insert(text.to_string(), parsed.clone());
        parsed
    }

    /// How many texts were read, for a test of the cache.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.0.borrow().len()
    }
}

/// What identifies the text of a shape in a record: `sha256:` and the hash
/// of the text as it was given. A boundary can run to megabytes, so a trace
/// and a receipt carry this in its place.
pub(crate) fn text_hash(text: &str) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(text.as_bytes())))
}

/// Whether the interiors of two shapes meet (DE-9IM `T********`). A polygon
/// that lies half on another overlaps it; two polygons that share only an
/// edge do not; a point overlaps a polygon when it lies inside it and not on
/// its boundary.
///
/// # Errors
///
/// When the pattern is refused, which a fixed pattern of nine characters
/// never is; the error is passed on rather than assumed away.
pub(crate) fn interiors_meet(a: &Geometry<f64>, b: &Geometry<f64>) -> Result<bool, String> {
    a.relate(b)
        .matches(INTERIORS_MEET)
        .map_err(|e| e.to_string())
}

/// The start of `text`, for a message.
pub(crate) fn quote(text: &str) -> String {
    let mut quoted: String = text.chars().take(QUOTED_CHARS).collect();
    if text.chars().nth(QUOTED_CHARS).is_some() {
        quoted.push('…');
    }
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: &str = "POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))";

    fn overlaps(a: &str, b: &str) -> bool {
        let (a, b) = (parse(a).unwrap(), parse(b).unwrap());
        interiors_meet(&a, &b).unwrap()
    }

    #[test]
    fn a_polygon_inside_or_half_on_another_overlaps_it() {
        assert!(overlaps("POLYGON((2 2, 4 2, 4 4, 2 4, 2 2))", SQUARE));
        assert!(overlaps("POLYGON((5 5, 15 5, 15 15, 5 15, 5 5))", SQUARE));
        assert!(overlaps(SQUARE, SQUARE));
    }

    #[test]
    fn polygons_that_share_only_an_edge_or_a_corner_do_not_overlap() {
        assert!(!overlaps(
            "POLYGON((10 0, 20 0, 20 10, 10 10, 10 0))",
            SQUARE
        ));
        assert!(!overlaps(
            "POLYGON((10 10, 20 10, 20 20, 10 20, 10 10))",
            SQUARE
        ));
        assert!(!overlaps(
            "POLYGON((20 20, 30 20, 30 30, 20 30, 20 20))",
            SQUARE
        ));
    }

    #[test]
    fn a_point_overlaps_a_polygon_only_strictly_inside_it() {
        assert!(overlaps("POINT(5 5)", SQUARE));
        assert!(overlaps(SQUARE, "POINT(5 5)"));
        assert!(!overlaps("POINT(10 5)", SQUARE));
        assert!(!overlaps("POINT(0 0)", SQUARE));
        assert!(!overlaps("POINT(11 5)", SQUARE));
    }

    #[test]
    fn a_hole_is_not_interior() {
        let ring = "POLYGON((0 0, 10 0, 10 10, 0 10, 0 0), (3 3, 7 3, 7 7, 3 7, 3 3))";
        assert!(!overlaps("POINT(5 5)", ring));
        assert!(overlaps("POINT(1 1)", ring));
    }

    #[test]
    fn a_line_overlaps_a_polygon_it_runs_through() {
        assert!(overlaps("LINESTRING(-5 5, 15 5)", SQUARE));
        assert!(!overlaps("LINESTRING(0 0, 10 0)", SQUARE));
    }

    #[test]
    fn text_that_is_no_wkt_is_refused() {
        for text in ["", "hier", "POLYGON((0 0, 10 0", "POINT(1)"] {
            let err = parse(text).unwrap_err();
            assert!(err.contains("does not parse as WKT"), "{text}: {err}");
        }
    }

    #[test]
    fn an_empty_shape_is_refused() {
        // It would overlap nothing, and "nothing" would read as an answer.
        for text in ["POINT EMPTY", "POLYGON EMPTY", "MULTIPOLYGON EMPTY"] {
            let err = parse(text).unwrap_err();
            assert!(err.contains("is an empty geometry"), "{text}: {err}");
        }
    }

    #[test]
    fn a_collection_of_geometries_is_refused() {
        let err = parse("GEOMETRYCOLLECTION(POINT(1 1), LINESTRING(0 0, 2 2))").unwrap_err();
        assert!(err.contains("is a collection of geometries"), "{err}");
        // Several of one kind are a shape.
        assert!(overlaps("MULTIPOINT((50 50), (5 5))", SQUARE));
        assert!(overlaps(
            "MULTIPOLYGON(((20 20, 30 20, 30 30, 20 30, 20 20)), ((1 1, 2 1, 2 2, 1 2, 1 1)))",
            SQUARE
        ));
    }

    #[test]
    fn a_coordinate_out_of_bounds_is_an_error_and_never_no_overlap() {
        // Corners at 1e308 made the relation overflow, and the polygon that
        // covers the square "overlapped nothing".
        let huge =
            "POLYGON((-1e308 -1e308, 1e308 -1e308, 1e308 1e308, -1e308 1e308, -1e308 -1e308))";
        let err = parse(huge).unwrap_err();
        assert!(err.contains("has a coordinate beyond 1e12"), "{err}");
        let err = parse("POINT(1e12 1.0000001e12)").unwrap_err();
        assert!(err.contains("has a coordinate beyond 1e12"), "{err}");
        // On the bound is in bounds, and a square that large still relates.
        let wide = "POLYGON((-1e12 -1e12, 1e12 -1e12, 1e12 1e12, -1e12 1e12, -1e12 -1e12))";
        assert!(overlaps(wide, SQUARE));
        assert!(overlaps("POINT(999999999999.5 0)", wide));
    }

    #[test]
    fn a_text_is_parsed_once_per_cache() {
        let shapes = ShapeCache::default();
        let first = shapes.parse(SQUARE).unwrap();
        let again = shapes.parse(SQUARE).unwrap();
        assert!(Rc::ptr_eq(&first, &again));
        // A refusal is remembered too.
        assert_eq!(shapes.parse("nergens"), shapes.parse("nergens"));
        assert!(shapes.parse("nergens").is_err());
        assert_eq!(shapes.len(), 2);
    }

    #[test]
    fn the_hash_of_a_text_tells_two_texts_apart() {
        assert_eq!(text_hash(SQUARE), text_hash(SQUARE));
        assert_ne!(text_hash(SQUARE), text_hash("POINT(5 5)"));
        assert!(text_hash(SQUARE).starts_with("sha256:"));
    }

    #[test]
    fn a_third_dimension_is_refused() {
        let err = parse("POINT Z (1 2 3)").unwrap_err();
        assert!(err.contains("not two-dimensional"), "{err}");
    }

    #[test]
    fn an_invalid_geometry_is_refused() {
        // A bow tie: the boundary crosses itself.
        let err = parse("POLYGON((0 0, 10 10, 10 0, 0 10, 0 0))").unwrap_err();
        assert!(err.contains("not a valid geometry"), "{err}");
    }

    #[test]
    fn a_long_text_is_quoted_by_its_start() {
        let text = format!("geen vorm {}", "x".repeat(500));
        let err = parse(&text).unwrap_err();
        assert!(err.contains('…') && err.len() < 250, "{err}");
    }
}
