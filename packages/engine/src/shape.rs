//! Shapes: two-dimensional geometries a law reasons about (schema v0.8.23).
//!
//! A shape is a string of well-known text (WKT), such as
//! `POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))` or `POINT(155105 463625)`. The
//! engine attaches no coordinate system to it: the numbers are taken as they
//! are written, so two shapes that meet in an operation must be in the same
//! system. Parsing is the `wkt` crate's and the geometry the `geo` crate's;
//! this module joins the two and says what it refuses. Whether a geometry
//! is valid is checked in [`crate::shape_validity`].
//!
//! The text comes from outside (a parameter, a register, a literal in a
//! rulework), so it is screened before the parser sees it: the parser
//! recurses without a bound on nested collections and reads only the first
//! geometry of a text, leaving the rest unread without a word.

use crate::config::{MAX_SHAPE_CHECK_STEPS, MAX_SHAPE_PAREN_DEPTH, MAX_SHAPE_TEXT_BYTES};
use crate::shape_validity::{self, Invalid};
use geo::algorithm::bool_ops::{BooleanOps, FillRule};
use geo::orient::{Direction, Orient};
use geo::{
    BoundingRect, CoordsIter, Geometry, HasDimensions, Intersects, MultiLineString, MultiPolygon,
    Rect, Relate,
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
/// only defined for valid ones; what makes one invalid is in
/// [`crate::shape_validity`]. A shape whose validity takes more than
/// [`MAX_SHAPE_CHECK_STEPS`] steps to check is refused too, so reading a
/// shape costs a bounded amount of work. A shape is a point, a line or a
/// surface, or several of one kind, so a `GEOMETRYCOLLECTION` is refused,
/// and so is an empty geometry (`POINT EMPTY`) and a geometry with an empty
/// part (`MULTIPOLYGON(EMPTY, ((...)))`): a place that is nowhere overlaps
/// nothing, and that would read as an answer. A coordinate beyond
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
    #[cfg(test)]
    TEXTS_PARSED.with(|parsed| parsed.set(parsed.get() + 1));
    let quoted = quote(text);
    screen(text, &quoted)?;
    let parsed =
        Wkt::<f64>::from_str(text).map_err(|e| format!("'{quoted}' does not parse as WKT: {e}"))?;
    if parsed.dimension() != Dimension::XY {
        return Err(format!(
            "'{quoted}' is not two-dimensional; a shape has an x and a y per point"
        ));
    }
    if has_empty_part(&parsed) {
        return Err(format!(
            "'{quoted}' has an empty part; every part of a shape has at least one point"
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
    let steps = shape_validity::check(&geometry, MAX_SHAPE_CHECK_STEPS).map_err(|e| match e {
        Invalid::TooCostly { .. } => format!("'{quoted}' cannot be checked: {e}"),
        _ => format!("'{quoted}' is not a valid geometry: {e}"),
    })?;
    #[cfg(not(test))]
    let _ = steps;
    Ok(Shape {
        bounds: geometry.bounding_rect(),
        geometry,
        #[cfg(test)]
        check_steps: steps,
    })
}

#[cfg(test)]
thread_local! {
    /// How many texts this thread has parsed, cached or not.
    static TEXTS_PARSED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many texts this thread has parsed so far, for a test that a text is
/// parsed once per execution.
#[cfg(test)]
pub(crate) fn texts_parsed() -> usize {
    TEXTS_PARSED.with(std::cell::Cell::get)
}

/// A shape that was read and found valid: its geometry, and the box around
/// it, computed once. Two shapes whose boxes have no point in common have
/// none in common themselves, which settles most questions about a location
/// and a territory without looking at a boundary of thousands of corners.
#[derive(Debug, PartialEq)]
pub(crate) struct Shape {
    geometry: Geometry<f64>,
    bounds: Option<Rect<f64>>,
    /// The steps the validity check took, for a test of its cost.
    #[cfg(test)]
    check_steps: u64,
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

/// Whether a part of `parsed` is empty while the whole is not: an `EMPTY`
/// among the polygons of a multipolygon, the rings of a polygon or the
/// lines of a multilinestring. The conversion to a geometry keeps such a
/// part, or drops it, without a word. Among the points of a multipoint the
/// parser takes no `EMPTY`.
fn has_empty_part(parsed: &Wkt<f64>) -> bool {
    let empty_line = |line: &wkt::types::LineString<f64>| line.coords().is_empty();
    let empty_surface = |polygon: &wkt::types::Polygon<f64>| {
        polygon.rings().is_empty() || polygon.rings().iter().any(empty_line)
    };
    match parsed {
        // A polygon without rings is empty as a whole, which has a message
        // of its own.
        Wkt::Polygon(polygon) => polygon.rings().iter().any(empty_line),
        Wkt::MultiLineString(lines) => lines.line_strings().iter().any(empty_line),
        Wkt::MultiPolygon(polygons) => polygons.polygons().iter().any(empty_surface),
        _ => false,
    }
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

/// Whether no part of `location` lies outside the `territories` taken
/// together: a location across two territories that share a boundary is
/// covered by the two, and one that reaches past them is not.
///
/// A territory that covers the location by itself settles it, by the exact
/// relation between the two. Otherwise the surfaces among the territories
/// are laid over the location in one overlay, and the location is covered
/// when nothing of it is left. The overlay puts all coordinates on one fine
/// grid, so points that are written the same in two shapes stay the same;
/// two neighbours whose common boundary is not written with the same points
/// leave a gap between them, and a gap is not covered unless it is narrower
/// than the grid, about a billionth of the extent of the shapes. A territory
/// that is a line or a point covers a location only by itself.
pub(crate) fn covered_by(location: &Shape, territories: &[Rc<Shape>]) -> bool {
    let covers =
        |territory: &Shape, part: &Geometry<f64>| part.relate(&territory.geometry).is_coveredby();
    if territories
        .iter()
        .any(|territory| covers(territory, &location.geometry))
    {
        return true;
    }
    if territories.len() < 2 {
        return false;
    }
    let surfaces = || {
        MultiPolygon::new(
            territories
                .iter()
                .flat_map(|territory| match &territory.geometry {
                    Geometry::Polygon(polygon) => vec![polygon.clone()],
                    Geometry::MultiPolygon(polygons) => polygons.0.clone(),
                    _ => Vec::new(),
                })
                .collect(),
        )
        // Outer rings one way round and holes the other, so that a place
        // two territories both hold counts as held.
        .orient(Direction::Default)
    };
    let left_of_lines = |lines: MultiLineString<f64>| {
        surfaces()
            .clip_with_fill_rule(&lines, true, FillRule::NonZero)
            .0
            .iter()
            .all(|line| line.0.is_empty())
    };
    let left_of_surfaces = |polygons: MultiPolygon<f64>| {
        polygons
            .orient(Direction::Default)
            .difference_with_fill_rule(&surfaces(), FillRule::NonZero)
            .0
            .is_empty()
    };
    match &location.geometry {
        // Each point by itself lies in a territory or in none.
        Geometry::MultiPoint(points) => points.iter().all(|point| {
            territories
                .iter()
                .any(|territory| covers(territory, &Geometry::Point(*point)))
        }),
        Geometry::LineString(line) => left_of_lines(MultiLineString::new(vec![line.clone()])),
        Geometry::MultiLineString(lines) => left_of_lines(lines.clone()),
        Geometry::Polygon(polygon) => left_of_surfaces(MultiPolygon::new(vec![polygon.clone()])),
        Geometry::MultiPolygon(polygons) => left_of_surfaces(polygons.clone()),
        // One point is covered by one territory or by none.
        _ => false,
    }
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
    fn an_empty_part_is_refused_wherever_it_stands() {
        for text in [
            "MULTIPOLYGON(EMPTY, ((1 1, 2 1, 2 2, 1 2, 1 1)))",
            "MULTIPOLYGON(((1 1, 2 1, 2 2, 1 2, 1 1)), EMPTY)",
            "MULTIPOLYGON(((1 1, 2 1, 2 2, 1 2, 1 1), EMPTY))",
            "POLYGON((1 1, 2 1, 2 2, 1 2, 1 1), EMPTY)",
            "POLYGON(EMPTY, (1 1, 2 1, 2 2, 1 2, 1 1))",
            "MULTILINESTRING((0 0, 1 1), EMPTY)",
            "MULTILINESTRING(EMPTY, (0 0, 1 1))",
        ] {
            let err = parse(text).unwrap_err();
            assert!(err.contains("has an empty part"), "{text}: {err}");
        }
        // Among the points of a multipoint the parser takes no EMPTY.
        for text in [
            "MULTIPOINT(EMPTY, (1 1))",
            "MULTIPOINT((1 1), EMPTY)",
            "MULTIPOINT(1 1, EMPTY)",
            "MULTIPOINT(EMPTY)",
        ] {
            let err = parse(text).unwrap_err();
            assert!(err.contains("does not parse as WKT"), "{text}: {err}");
        }
        // The same shapes without the empty part are shapes.
        for text in [
            "MULTIPOLYGON(((1 1, 2 1, 2 2, 1 2, 1 1)))",
            "POLYGON((1 1, 2 1, 2 2, 1 2, 1 1))",
            "MULTILINESTRING((0 0, 1 1))",
            "MULTIPOINT((1 1))",
        ] {
            assert!(parse(text).is_ok(), "{text}");
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
    fn the_limits_of_the_screen_hold_at_the_limit_and_fail_one_past_it() {
        // Space after the geometry is not text, so a shape can be made as
        // long as the cap to the byte.
        let at_the_cap = format!("POINT(5 5){}", " ".repeat(MAX_SHAPE_TEXT_BYTES - 10));
        assert_eq!(at_the_cap.len(), MAX_SHAPE_TEXT_BYTES);
        assert!(parse(&at_the_cap).is_ok());
        let err = parse(&format!("{at_the_cap} ")).unwrap_err();
        assert!(
            err.contains("is 4000001 bytes of text; a shape is at most 4000000"),
            "{err}"
        );
        // Three levels of parentheses are a multipolygon, four are none.
        assert_eq!(MAX_SHAPE_PAREN_DEPTH, 3);
        assert!(parse("MULTIPOLYGON(((1 1, 2 1, 2 2, 1 1)))").is_ok());
        let err = parse("MULTIPOLYGON((((1 1, 2 1, 2 2, 1 1))))").unwrap_err();
        assert!(err.contains("nests parentheses deeper than 3"), "{err}");
    }

    /// A polygon of `corners` corners on a circle around the origin.
    fn circle(corners: usize) -> String {
        let mut points: Vec<String> = (0..corners)
            .map(|i| {
                let angle = std::f64::consts::TAU * i as f64 / corners as f64;
                format!("{} {}", 1000.0 * angle.cos(), 1000.0 * angle.sin())
            })
            .collect();
        points.push(points[0].clone());
        format!("POLYGON(({}))", points.join(", "))
    }

    #[test]
    fn reading_a_large_boundary_takes_steps_in_proportion_to_its_size() {
        // The count of the work done, taken where a shape is read: a check
        // that compares every segment with every other takes 32 million
        // comparisons for these 8,000 corners, and leaves no count at all
        // when it is not the one of this crate.
        let shape = parse(&circle(8_000)).unwrap();
        assert!(
            (8_000..800_000).contains(&shape.check_steps),
            "{} steps",
            shape.check_steps
        );
        // A point is not checked in steps.
        assert_eq!(parse("POINT(1 1)").unwrap().check_steps, 0);
    }

    #[test]
    fn a_shape_that_costs_too_much_to_check_is_refused_as_such() {
        // Two thousand long strips that lean the same way: the box of each
        // covers the others, and every side would be held against every
        // other.
        let strips: Vec<String> = (0..2_000)
            .map(|i| {
                let x = 3 * i;
                format!(
                    "(({x} 0, {} 0, {} 1000, {} 1000, {x} 0))",
                    x + 1,
                    x + 1001,
                    x + 1000
                )
            })
            .collect();
        let err = parse(&format!("MULTIPOLYGON({})", strips.join(", "))).unwrap_err();
        assert!(
            err.contains("cannot be checked: checking it takes more than 50000000 steps"),
            "{err}"
        );
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
        const AFTER: &str = "has text after the shape closes";
        for (text, why) in [
            // A second ring set outside the polygon: only the first was read.
            (
                "POLYGON((0 0, 10 0, 10 10, 0 10, 0 0)), ((20 20, 30 20, 30 30, 20 30, 20 20))",
                AFTER,
            ),
            ("POINT(155105 463625) trailing", AFTER),
            ("POINT(1 1) POINT(2 2)", AFTER),
            // The text after the shape is seen before its parenthesis is.
            ("POINT(1 1))", AFTER),
            ("POINT 1 1)", "a parenthesis closes that was not opened"),
        ] {
            let err = parse(text).unwrap_err();
            assert!(err.contains(why), "{text}: {err}");
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
    fn boxes_without_width_or_height_are_related_like_any_other() {
        // A point against the same point: both boxes are that point.
        assert!(overlaps("POINT(5 5)", "POINT(5 5)"));
        assert!(!overlaps("POINT(5 5)", "POINT(5 6)"));
        // Lines on one line: their boxes have no height. They overlap where
        // they run over each other, and not where they only meet end to
        // end or lie apart.
        assert!(overlaps("LINESTRING(0 0, 10 0)", "LINESTRING(5 0, 15 0)"));
        assert!(!overlaps("LINESTRING(0 0, 10 0)", "LINESTRING(10 0, 20 0)"));
        assert!(!overlaps("LINESTRING(0 0, 10 0)", "LINESTRING(11 0, 20 0)"));
        // The same upright, where the boxes have no width.
        assert!(overlaps("LINESTRING(0 0, 0 10)", "LINESTRING(0 5, 0 15)"));
        assert!(!overlaps("LINESTRING(0 0, 0 10)", "LINESTRING(0 10, 0 20)"));
        // A point on a line, and at its end, which is its boundary.
        assert!(overlaps("POINT(5 0)", "LINESTRING(0 0, 10 0)"));
        assert!(!overlaps("POINT(10 0)", "LINESTRING(0 0, 10 0)"));
    }

    /// Whether `location` is covered by `territories` together.
    fn covered(location: &str, territories: &[&str]) -> bool {
        let territories: Vec<Rc<Shape>> = territories
            .iter()
            .map(|text| Rc::new(parse(text).unwrap()))
            .collect();
        covered_by(&parse(location).unwrap(), &territories)
    }

    const NEXT_SQUARE: &str = "POLYGON((10 0, 20 0, 20 10, 10 10, 10 0))";

    #[test]
    fn a_location_is_covered_when_no_part_of_it_lies_outside_the_territories() {
        // By one territory, also where it lies against its boundary or is
        // the territory itself.
        assert!(covered("POLYGON((2 2, 4 2, 4 4, 2 4, 2 2))", &[SQUARE]));
        assert!(covered("POLYGON((0 0, 4 0, 4 4, 0 4, 0 0))", &[SQUARE]));
        assert!(covered(SQUARE, &[SQUARE]));
        assert!(covered("POINT(10 5)", &[SQUARE]));
        // Not when it reaches past it, however little: around a corner of
        // the boundary.
        assert!(!covered(
            "POLYGON((-1 -1, 4 -1, 4 4, -1 4, -1 -1))",
            &[SQUARE]
        ));
        assert!(!covered("POLYGON((8 4, 12 4, 12 6, 8 6, 8 4))", &[SQUARE]));
        assert!(!covered("POINT(11 5)", &[SQUARE]));
        // Nothing covers a location when there is no territory.
        assert!(!covered("POINT(5 5)", &[]));
    }

    #[test]
    fn two_territories_that_share_a_boundary_cover_a_location_across_it() {
        let both = [SQUARE, NEXT_SQUARE];
        assert!(covered("POLYGON((8 4, 12 4, 12 6, 8 6, 8 4))", &both));
        // The two together, to their outer boundary.
        assert!(covered("POLYGON((0 0, 20 0, 20 10, 0 10, 0 0))", &both));
        // Past both.
        assert!(!covered("POLYGON((8 4, 22 4, 22 6, 8 6, 8 4))", &both));
        assert!(!covered("POLYGON((8 4, 12 4, 12 12, 8 12, 8 4))", &both));
        // A gap between the two is not covered.
        let gap = [
            SQUARE,
            "POLYGON((10.001 0, 20 0, 20 10, 10.001 10, 10.001 0))",
        ];
        assert!(!covered("POLYGON((8 4, 12 4, 12 6, 8 6, 8 4))", &gap));
        // Territories that lie over each other cover what either covers.
        let over = [SQUARE, "POLYGON((5 0, 20 0, 20 10, 5 10, 5 0))"];
        assert!(covered("POLYGON((2 4, 18 4, 18 6, 2 6, 2 4))", &over));
        // Three in a row cover a location across all three, and one of them
        // may be written as a multipolygon with a part elsewhere.
        let third = "POLYGON((20 0, 30 0, 30 10, 20 10, 20 0))";
        assert!(covered(
            "POLYGON((8 4, 22 4, 22 6, 8 6, 8 4))",
            &[SQUARE, NEXT_SQUARE, third]
        ));
        let in_two_parts = "MULTIPOLYGON(((10 0, 20 0, 20 10, 10 10, 10 0)), ((50 50, 60 50, 60 60, 50 60, 50 50)))";
        assert!(covered(
            "POLYGON((8 4, 12 4, 12 6, 8 6, 8 4))",
            &[SQUARE, in_two_parts]
        ));
        // A location with a hole, and one in two parts.
        assert!(covered(
            "POLYGON((2 2, 18 2, 18 8, 2 8, 2 2), (9 4, 11 4, 11 6, 9 6, 9 4))",
            &both
        ));
        assert!(covered(
            "MULTIPOLYGON(((1 1, 3 1, 3 3, 1 3, 1 1)), ((15 1, 17 1, 17 3, 15 3, 15 1)))",
            &both
        ));
        assert!(!covered(
            "MULTIPOLYGON(((1 1, 3 1, 3 3, 1 3, 1 1)), ((25 1, 27 1, 27 3, 25 3, 25 1)))",
            &both
        ));
    }

    #[test]
    fn lines_and_points_are_covered_part_by_part() {
        let both = [SQUARE, NEXT_SQUARE];
        assert!(covered("LINESTRING(5 5, 15 5)", &both));
        assert!(!covered("LINESTRING(5 5, 25 5)", &both));
        assert!(covered("MULTILINESTRING((1 1, 4 4), (12 1, 18 4))", &both));
        assert!(!covered("MULTILINESTRING((1 1, 4 4), (12 1, 28 4))", &both));
        assert!(covered("MULTIPOINT((5 5), (15 5))", &both));
        assert!(!covered("MULTIPOINT((5 5), (15 5), (50 50))", &both));
        // A territory that is a line covers nothing but by itself.
        assert!(!covered(
            "POLYGON((8 4, 12 4, 12 6, 8 6, 8 4))",
            &[SQUARE, "LINESTRING(0 0, 20 0)"]
        ));
    }

    #[test]
    fn a_long_text_is_quoted_by_its_start() {
        let text = format!("geen vorm {}", "x".repeat(500));
        let err = parse(&text).unwrap_err();
        assert!(err.contains('…') && err.len() < 250, "{err}");
    }
}
