//! Shapes: two-dimensional geometries a law reasons about (schema v0.8.23).
//!
//! A shape is a string of well-known text (WKT), such as
//! `POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))` or `POINT(155105 463625)`. The
//! engine attaches no coordinate system to it: the numbers are taken as they
//! are written, so two shapes that meet in an operation must be in the same
//! system. Parsing is the `wkt` crate's and the geometry the `geo` crate's;
//! this module only joins the two and says what it refuses.
//!
//! The text comes from outside (a parameter, a register, a literal in a
//! rulework), so it is screened before the parser sees it: the parser
//! recurses without a bound on nested collections and reads only the first
//! geometry of a text, leaving the rest unread without a word.

use crate::config::{MAX_SHAPE_PAREN_DEPTH, MAX_SHAPE_TEXT_BYTES};
use geo::{
    BoundingRect, CoordsIter, Geometry, HasDimensions, Intersects, Rect, Relate, Validation,
};
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

/// The kinds of geometry that are a shape: a point, a line or a surface, or
/// several of one kind.
const KINDS: [&str; 6] = [
    "POINT",
    "LINESTRING",
    "POLYGON",
    "MULTIPOINT",
    "MULTILINESTRING",
    "MULTIPOLYGON",
];

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
///
/// The text is one geometry and nothing else: text after the geometry
/// closes is refused, and so is a ring that does not end where it starts.
/// The parser would drop the first and close the second without a word, and
/// a mis-transcribed area would then be another area. A text longer than
/// [`MAX_SHAPE_TEXT_BYTES`], or with parentheses nested deeper than
/// [`MAX_SHAPE_PAREN_DEPTH`], is refused before it is parsed.
pub(crate) fn parse(text: &str) -> Result<Shape, String> {
    let quoted = quote(text);
    screen(text, &quoted)?;
    let parsed =
        Wkt::<f64>::from_str(text).map_err(|e| format!("'{quoted}' does not parse as WKT: {e}"))?;
    if parsed.dimension() != Dimension::XY {
        return Err(format!(
            "'{quoted}' is not two-dimensional; a shape has an x and a y per point"
        ));
    }
    if has_open_ring(&parsed) {
        return Err(format!(
            "'{quoted}' has a ring that does not end where it starts; the boundary of a \
             surface is closed"
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
    Ok(Shape {
        bounds: geometry.bounding_rect(),
        geometry,
    })
}

/// A shape that was read and found valid: its geometry, and the box around
/// it, computed once. Two shapes whose boxes have no point in common have
/// none in common themselves, which settles most questions about a location
/// and a territory without looking at a boundary of thousands of corners.
#[derive(Debug, PartialEq)]
pub(crate) struct Shape {
    geometry: Geometry<f64>,
    bounds: Option<Rect<f64>>,
}

/// Refuse a text the parser must not be given, or would read only part of.
/// `quoted` is the start of the text, for the message.
///
/// The parser is recursive on nested collections and has no limit of its
/// own, so this runs first and walks the text once, without recursion: the
/// length, the kind of geometry the text starts with, the depth of its
/// parentheses, and that nothing follows the geometry once it closes.
fn screen(text: &str, quoted: &str) -> Result<(), String> {
    if text.len() > MAX_SHAPE_TEXT_BYTES {
        return Err(format!(
            "'{quoted}' is {} bytes of text; a shape is at most {MAX_SHAPE_TEXT_BYTES}",
            text.len()
        ));
    }
    let kind: String = text
        .trim_start()
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .collect::<String>()
        .to_ascii_uppercase();
    if kind == "GEOMETRYCOLLECTION" {
        return Err(format!(
            "'{quoted}' is a collection of geometries; a shape is a point, a line or a surface"
        ));
    }
    if !KINDS.contains(&kind.as_str()) {
        return Err(format!(
            "'{quoted}' does not parse as WKT: a shape starts with {}",
            KINDS.join(", ")
        ));
    }
    let mut depth = 0usize;
    let mut closed = false;
    for c in text.chars() {
        if closed && !c.is_whitespace() {
            return Err(format!(
                "'{quoted}' has text after the shape closes; a shape is one geometry and \
                 nothing else"
            ));
        }
        match c {
            '(' => {
                depth += 1;
                if depth > MAX_SHAPE_PAREN_DEPTH {
                    return Err(format!(
                        "'{quoted}' nests parentheses deeper than {MAX_SHAPE_PAREN_DEPTH}; a \
                         shape is a point, a line or a surface, or several of one kind"
                    ));
                }
            }
            ')' => {
                let Some(shallower) = depth.checked_sub(1) else {
                    return Err(format!(
                        "'{quoted}' does not parse as WKT: a parenthesis closes that was not \
                         opened"
                    ));
                };
                depth = shallower;
                closed = depth == 0;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(format!(
            "'{quoted}' does not parse as WKT: a parenthesis is not closed"
        ));
    }
    Ok(())
}

/// Whether a surface in `parsed` has a ring whose last point is not its
/// first. The conversion to a geometry would close such a ring by itself.
fn has_open_ring(parsed: &Wkt<f64>) -> bool {
    let open = |ring: &wkt::types::LineString<f64>| {
        let coords = ring.coords();
        coords
            .first()
            .zip(coords.last())
            .is_some_and(|(first, last)| first.x != last.x || first.y != last.y)
    };
    match parsed {
        Wkt::Polygon(polygon) => polygon.rings().iter().any(open),
        Wkt::MultiPolygon(polygons) => polygons
            .polygons()
            .iter()
            .flat_map(wkt::types::Polygon::rings)
            .any(open),
        _ => false,
    }
}

/// What reading a text as a shape came to: the shape, shared, or why it is
/// none.
type Parsed = Result<Rc<Shape>, String>;

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
pub(crate) fn interiors_meet(a: &Shape, b: &Shape) -> Result<bool, String> {
    // Two shapes whose bounding boxes have no point in common have none in
    // common themselves, so the relation need not be computed: a location
    // far from a territory costs a comparison of two boxes. Boxes that only
    // touch go on to the relation, which says what the boundaries do.
    if let (Some(box_a), Some(box_b)) = (&a.bounds, &b.bounds)
        && !box_a.intersects(box_b)
    {
        return Ok(false);
    }
    a.geometry
        .relate(&b.geometry)
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
    fn deep_nesting_is_refused_before_the_parser_sees_it() {
        // Twenty thousand nested collections overflow the stack of the
        // parser, which aborts the process. Refused on the keyword, and on
        // the depth for any other text that nests that deep.
        let nested = format!(
            "{}POINT(1 1){}",
            "GEOMETRYCOLLECTION(".repeat(20_000),
            ")".repeat(20_000)
        );
        let err = parse(&nested).unwrap_err();
        assert!(err.contains("is a collection of geometries"), "{err}");

        let deep = format!("POLYGON{}0 0{}", "(".repeat(20_000), ")".repeat(20_000));
        let err = parse(&deep).unwrap_err();
        assert!(err.contains("nests parentheses deeper than 3"), "{err}");
        // A collection inside a kind that is a shape gets no further either.
        let hidden = format!(
            "MULTIPOLYGON((({}POINT(1 1){})))",
            "GEOMETRYCOLLECTION(".repeat(20_000),
            ")".repeat(20_000)
        );
        let err = parse(&hidden).unwrap_err();
        assert!(err.contains("nests parentheses deeper than 3"), "{err}");
        // The deepest shape there is still reads.
        assert!(overlaps(
            "MULTIPOLYGON(((1 1, 2 1, 2 2, 1 2, 1 1)))",
            SQUARE
        ));
    }

    #[test]
    fn a_text_longer_than_the_cap_is_refused_unread() {
        let long = format!(
            "MULTIPOINT({}(1 1))",
            "(1 1), ".repeat(MAX_SHAPE_TEXT_BYTES / 7 + 1)
        );
        let err = parse(&long).unwrap_err();
        assert!(
            err.contains("bytes of text; a shape is at most 4000000"),
            "{err}"
        );
    }

    #[test]
    fn text_after_the_geometry_is_refused_and_never_dropped() {
        for text in [
            // A second ring set outside the polygon: only the first was read.
            "POLYGON((0 0, 10 0, 10 10, 0 10, 0 0)), ((20 20, 30 20, 30 30, 20 30, 20 20))",
            "POINT(155105 463625) trailing",
            "POINT(1 1) POINT(2 2)",
            "POINT(1 1))",
        ] {
            let err = parse(text).unwrap_err();
            assert!(
                err.contains("has text after the shape closes")
                    || err.contains("a parenthesis closes that was not opened"),
                "{text}: {err}"
            );
        }
        // Space around the geometry is not text.
        assert!(overlaps("  POINT(5 5)\n", SQUARE));
    }

    #[test]
    fn a_ring_that_does_not_close_is_refused() {
        for text in [
            "POLYGON((0 0, 10 0, 10 10, 0 10))",
            "POLYGON((0 0, 10 0, 10 10, 0 10, 0 0), (3 3, 7 3, 7 7, 3 7))",
            "MULTIPOLYGON(((0 0, 10 0, 10 10, 0 10, 0 0)), ((20 20, 30 20, 30 30, 20 30)))",
        ] {
            let err = parse(text).unwrap_err();
            assert!(
                err.contains("has a ring that does not end where it starts"),
                "{text}: {err}"
            );
        }
        // A line need not close.
        assert!(overlaps("LINESTRING(-5 5, 15 5)", SQUARE));
    }

    #[test]
    fn a_kind_that_is_no_shape_is_refused_on_its_name() {
        for text in ["TRIANGLE((0 0, 1 0, 0 1, 0 0))", "CIRCLE(1 1, 5)", "(1 1)"] {
            let err = parse(text).unwrap_err();
            assert!(
                err.contains("does not parse as WKT: a shape starts with POINT"),
                "{text}: {err}"
            );
        }
        // The names are read in either case, as the parser reads them.
        assert!(overlaps("point(5 5)", SQUARE));
    }

    #[test]
    fn shapes_whose_boxes_are_apart_do_not_overlap_and_touching_boxes_are_related() {
        // Far apart: decided on the boxes.
        assert!(!overlaps("POINT(1000 1000)", SQUARE));
        // Boxes that overlap while the shapes do not: an L around a square.
        let l = "POLYGON((0 0, 30 0, 30 10, 10 10, 10 30, 0 30, 0 0))";
        assert!(!overlaps(l, "POLYGON((20 20, 28 20, 28 28, 20 28, 20 20))"));
        // Boxes that touch on an edge: the interiors do not meet.
        assert!(!overlaps(
            "POLYGON((10 0, 20 0, 20 10, 10 10, 10 0))",
            SQUARE
        ));
        // A point is its own box.
        assert!(overlaps("POINT(5 5)", SQUARE));
        assert!(!overlaps("POINT(10 5)", SQUARE));
    }

    #[test]
    fn a_long_text_is_quoted_by_its_start() {
        let text = format!("geen vorm {}", "x".repeat(500));
        let err = parse(&text).unwrap_err();
        assert!(err.contains('…') && err.len() < 250, "{err}");
    }
}
