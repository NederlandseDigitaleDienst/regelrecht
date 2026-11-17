//! Shapes: two-dimensional geometries a law reasons about (schema v0.8.23).
//!
//! A shape is a string of well-known text (WKT), such as
//! `POLYGON((0 0, 10 0, 10 10, 0 10, 0 0))` or `POINT(155105 463625)`. The
//! engine attaches no coordinate system to it: the numbers are taken as they
//! are written, so two shapes that meet in an operation must be in the same
//! system. Parsing is the `wkt` crate's and the geometry the `geo` crate's;
//! this module only joins the two and says what it refuses.

use geo::{Geometry, Relate, Validation};
use std::str::FromStr;
use wkt::Wkt;
use wkt::types::Dimension;

/// The DE-9IM pattern of `OVERLAPS`: the interiors of the two shapes have at
/// least one point in common, whatever their boundaries do.
const INTERIORS_MEET: &str = "T********";

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
/// only defined for valid ones.
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
    geometry
        .check_validation()
        .map_err(|e| format!("'{quoted}' is not a valid geometry: {e}"))?;
    Ok(geometry)
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
fn quote(text: &str) -> String {
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
    fn an_empty_shape_overlaps_nothing() {
        assert!(!overlaps("POINT EMPTY", SQUARE));
        assert!(!overlaps("POLYGON EMPTY", SQUARE));
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
