//! Whether the geometry of a shape is valid, checked in a bounded number of
//! steps.
//!
//! The relation between two shapes is only defined for valid geometries, so
//! an invalid one is refused when it is read. The `geo` crate has a validity
//! check, but for a surface it compares every segment of a ring with every
//! other, every hole with every other and every polygon of a multipolygon
//! with every other: reading a boundary of n points costs n² comparisons,
//! and a text of a few megabytes then takes hours. This module checks a
//! surface another way:
//!
//! 1. the segments of all rings go into a tree of their bounding boxes, and
//!    only segments whose boxes meet are held against each other, with exact
//!    predicates;
//! 2. where two rings touch in a point, the corners around that point say
//!    whether they cross there and which lies inside which;
//! 3. for rings that do not touch, one point of each is held against the
//!    others with a ray, through the same tree.
//!
//! Every box that is compared and every pair of segments that is held
//! against each other is a step, and a shape that takes more steps than the
//! caller allows is refused. For a real boundary the steps grow with n log n.
//! A text built to make boxes overlap everywhere runs into the limit.
//!
//! What makes a surface valid is what the Simple Features standard asks and
//! `geo` checks: every ring has three distinct points, no ring crosses or
//! touches itself, two rings meet in points only and do not cross, a hole
//! lies inside its polygon and outside the other holes, and the polygons of
//! a multipolygon do not overlap. Like `geo`, it does not check that the
//! interior of a polygon is connected.

use geo::algorithm::kernels::{Kernel, Orientation, RobustKernel};
use geo::{Coord, Geometry, LineString, Polygon, Validation};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// How many boxes a node of the tree holds.
const FANOUT: usize = 8;

/// The steps one pair of segments counts for: it is held against each other
/// with four exact orientation tests, a box is compared with four
/// comparisons.
const PAIR_STEPS: u64 = 8;

/// Why a geometry is not valid.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Invalid {
    /// A ring with fewer than three distinct points.
    TooFewPoints(RingName),
    /// A ring that crosses or touches itself, or runs back over itself.
    SelfCrossing(RingName),
    /// Two rings that cross.
    RingsCross(RingName, RingName),
    /// Two rings that run along each other.
    RingsShareALine(RingName, RingName),
    /// A hole that does not lie inside its polygon.
    HoleOutside(RingName),
    /// A hole that lies inside another hole of its polygon.
    HoleInHole(RingName, RingName),
    /// Two polygons of a multipolygon whose interiors meet, by their
    /// numbers.
    PolygonsOverlap(usize, usize),
    /// The check took more steps than allowed.
    TooCostly { allowed: u64 },
    /// What `geo` says of a geometry that is no surface.
    Other(String),
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Invalid::TooFewPoints(ring) => write!(f, "{ring} has fewer than 3 distinct points"),
            Invalid::SelfCrossing(ring) => write!(f, "{ring} crosses or touches itself"),
            Invalid::RingsCross(a, b) => write!(f, "{a} and {b} cross"),
            Invalid::RingsShareALine(a, b) => write!(f, "{a} and {b} run along each other"),
            Invalid::HoleOutside(hole) => write!(f, "{hole} does not lie inside its polygon"),
            Invalid::HoleInHole(hole, other) => write!(f, "{hole} lies inside {other}"),
            Invalid::PolygonsOverlap(a, b) => write!(f, "polygons {a} and {b} overlap"),
            Invalid::TooCostly { allowed } => write!(
                f,
                "checking it takes more than {allowed} steps: too many of its segments lie \
                 close to each other"
            ),
            Invalid::Other(why) => f.write_str(why),
        }
    }
}

/// A ring by its place in the geometry, counted from one: the outer ring of
/// a polygon, or one of its holes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RingName {
    polygon: usize,
    hole: Option<usize>,
}

impl fmt::Display for RingName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.hole {
            Some(hole) => write!(f, "hole {hole} of polygon {}", self.polygon),
            None => write!(f, "the outer ring of polygon {}", self.polygon),
        }
    }
}

/// Check that `geometry` is valid, in at most `allowed` steps. Gives the
/// steps it took.
///
/// # Errors
///
/// Says what makes the geometry invalid, or that the check took more than
/// `allowed` steps ([`Invalid::TooCostly`]).
pub(crate) fn check(geometry: &Geometry<f64>, allowed: u64) -> Result<u64, Invalid> {
    let polygons: &[Polygon<f64>] = match geometry {
        Geometry::Polygon(polygon) => std::slice::from_ref(polygon),
        Geometry::MultiPolygon(polygons) => &polygons.0,
        // A point or a line is checked point by point, which `geo` does in
        // one pass.
        other => {
            return other
                .check_validation()
                .map(|()| 0)
                .map_err(|e| Invalid::Other(e.to_string()));
        }
    };
    let mut surfaces = Surfaces::of(polygons)?;
    let mut steps = Steps { taken: 0, allowed };
    surfaces.hold_segments(&mut steps)?;
    surfaces.hold_nesting(&mut steps)?;
    Ok(steps.taken)
}

/// The steps taken so far, and how many may be.
struct Steps {
    taken: u64,
    allowed: u64,
}

impl Steps {
    fn take(&mut self, steps: u64) -> Result<(), Invalid> {
        self.taken += steps;
        if self.taken > self.allowed {
            return Err(Invalid::TooCostly {
                allowed: self.allowed,
            });
        }
        Ok(())
    }
}

/// One ring: its distinct points in order, without the closing one.
struct Ring {
    name: RingName,
    /// The polygon it belongs to, counted from zero.
    polygon: usize,
    points: Vec<Coord<f64>>,
    /// Whether it runs counter-clockwise: its interior is then on the left.
    counter_clockwise: bool,
}

impl Ring {
    fn is_hole(&self) -> bool {
        self.name.hole.is_some()
    }

    /// The point `steps` places after the point at `at`, around the ring.
    fn point(&self, at: usize, steps: usize) -> Coord<f64> {
        self.points[(at + steps) % self.points.len()]
    }
}

/// One segment of a ring: from the point at `at` to the next.
#[derive(Clone, Copy)]
struct Segment {
    ring: usize,
    at: usize,
}

/// The rings of a surface and their segments, with what is known of how the
/// rings that touch lie to each other.
struct Surfaces {
    rings: Vec<Ring>,
    segments: Vec<Segment>,
    tree: BoxTree,
    /// The box around all of it.
    around: Bounds,
    /// For two rings that touch: whether the first lies inside the second.
    within: BTreeMap<(usize, usize), bool>,
}

/// How two segments meet.
#[derive(Debug, PartialEq)]
enum Meeting {
    Apart,
    /// In one point that is interior to both.
    Cross,
    /// Along a line.
    Overlap,
    /// In one point that is an end of at least one of them.
    Touch(Coord<f64>),
}

/// The sign of the turn from `a` over `b` to `c`: 1 to the left, -1 to the
/// right, 0 when the three lie on a line. Exact for coordinates whose
/// products do not underflow; a shape of some 1e-150 across reads as a line
/// and is refused.
fn turn(a: Coord<f64>, b: Coord<f64>, c: Coord<f64>) -> i8 {
    match RobustKernel::orient2d(a, b, c) {
        Orientation::CounterClockwise => 1,
        Orientation::Clockwise => -1,
        Orientation::Collinear => 0,
    }
}

/// The order of two points by x and then by y. Coordinates are numbers (the
/// caller has refused the others), so two always compare.
fn by_x_then_y(a: Coord<f64>, b: Coord<f64>) -> Ordering {
    let by = |a: f64, b: f64| a.partial_cmp(&b).unwrap_or(Ordering::Equal);
    by(a.x, b.x).then(by(a.y, b.y))
}

/// Whether `c`, which lies on the line through `a` and `b`, lies between
/// them.
fn between(c: Coord<f64>, a: Coord<f64>, b: Coord<f64>) -> bool {
    let within = |c: f64, a: f64, b: f64| a.min(b) <= c && c <= a.max(b);
    within(c.x, a.x, b.x) && within(c.y, a.y, b.y)
}

/// How the segment from `p1` to `p2` meets the one from `q1` to `q2`.
fn meeting(p1: Coord<f64>, p2: Coord<f64>, q1: Coord<f64>, q2: Coord<f64>) -> Meeting {
    let (q1_side, q2_side) = (turn(p1, p2, q1), turn(p1, p2, q2));
    let (p1_side, p2_side) = (turn(q1, q2, p1), turn(q1, q2, p2));
    if q1_side * q2_side < 0 && p1_side * p2_side < 0 {
        return Meeting::Cross;
    }
    if q1_side == 0 && q2_side == 0 {
        // On one line, where the order by x and then y is the order along
        // the line.
        let ordered = |a, b| match by_x_then_y(a, b) {
            Ordering::Greater => (b, a),
            _ => (a, b),
        };
        let ((p_low, p_high), (q_low, q_high)) = (ordered(p1, p2), ordered(q1, q2));
        let low = ordered(p_low, q_low).1;
        let high = ordered(p_high, q_high).0;
        return match by_x_then_y(low, high) {
            Ordering::Less => Meeting::Overlap,
            Ordering::Equal => Meeting::Touch(low),
            Ordering::Greater => Meeting::Apart,
        };
    }
    if q1_side == 0 && between(q1, p1, p2) {
        return Meeting::Touch(q1);
    }
    if q2_side == 0 && between(q2, p1, p2) {
        return Meeting::Touch(q2);
    }
    if p1_side == 0 && between(p1, q1, q2) {
        return Meeting::Touch(p1);
    }
    if p2_side == 0 && between(p2, q1, q2) {
        return Meeting::Touch(p2);
    }
    Meeting::Apart
}

/// Whether `d`, which lies on the line through `p` and `u`, lies on the same
/// side of `p` as `u` does.
fn same_side(p: Coord<f64>, u: Coord<f64>, d: Coord<f64>) -> bool {
    let side = |of: f64, p: f64| of.partial_cmp(&p).unwrap_or(Ordering::Equal);
    if u.x == p.x {
        side(u.y, p.y) == side(d.y, p.y)
    } else {
        side(u.x, p.x) == side(d.x, p.x)
    }
}

/// Whether the ray from `p` through `d` lies strictly inside the sector that
/// turns counter-clockwise from the ray through `from` to the ray through
/// `to`. `None` when it lies along one of those two rays.
fn within_sector(p: Coord<f64>, from: Coord<f64>, to: Coord<f64>, d: Coord<f64>) -> Option<bool> {
    let (after_from, before_to) = (turn(p, from, d), turn(p, d, to));
    if (after_from == 0 && same_side(p, from, d)) || (before_to == 0 && same_side(p, to, d)) {
        return None;
    }
    // With the two rays themselves out of the way, a turn of 0 is a ray
    // that points straight away from one of them.
    Some(match turn(p, from, to) {
        // Less than half a turn: left of the first ray and right of the last.
        1 => after_from == 1 && before_to == 1,
        // More than half a turn: everything but the sector on the other
        // side, which is less than half a turn and lies right of the first
        // ray and left of the last.
        -1 => after_from != -1 || before_to != -1,
        // Half a turn: the side left of the first ray.
        _ => after_from == 1,
    })
}

impl Surfaces {
    /// The rings of `polygons`, with a tree of their segments.
    fn of(polygons: &[Polygon<f64>]) -> Result<Self, Invalid> {
        let mut rings = Vec::new();
        for (polygon, surface) in polygons.iter().enumerate() {
            let lines = std::iter::once(surface.exterior()).chain(surface.interiors());
            for (line_number, line) in lines.enumerate() {
                let name = RingName {
                    polygon: polygon + 1,
                    hole: line_number.checked_sub(1).map(|hole| hole + 1),
                };
                rings.push(Self::ring(name, polygon, line)?);
            }
        }
        let segments: Vec<Segment> = rings
            .iter()
            .enumerate()
            .flat_map(|(ring, points)| (0..points.points.len()).map(move |at| Segment { ring, at }))
            .collect();
        let boxes: Vec<Bounds> = segments
            .iter()
            .map(|segment| {
                let ring = &rings[segment.ring];
                bounds_of(ring.point(segment.at, 0), ring.point(segment.at, 1))
            })
            .collect();
        let around = boxes
            .iter()
            .fold(NOTHING, |around, bounds| widened(&around, bounds));
        Ok(Surfaces {
            rings,
            segments,
            around,
            tree: BoxTree::pack(&boxes),
            within: BTreeMap::new(),
        })
    }

    /// One ring of a polygon: its points without the repeated ones, and the
    /// way it turns.
    fn ring(name: RingName, polygon: usize, line: &LineString<f64>) -> Result<Ring, Invalid> {
        let mut points = line.0.clone();
        points.dedup();
        // A ring is written closed: its last point is its first.
        if points.first() == points.last() {
            points.pop();
        }
        if points.len() < 3 {
            return Err(Invalid::TooFewPoints(name));
        }
        // The lowest point by x and then y is a corner the ring turns
        // around, and the turn there is the turn of the ring. Three points
        // on a line there are a ring that runs back over itself.
        let lowest = (0..points.len())
            .min_by(|a, b| by_x_then_y(points[*a], points[*b]))
            .unwrap_or(0);
        let before = points[(lowest + points.len() - 1) % points.len()];
        let after = points[(lowest + 1) % points.len()];
        let counter_clockwise = match turn(before, points[lowest], after) {
            1 => true,
            -1 => false,
            _ => return Err(Invalid::SelfCrossing(name)),
        };
        Ok(Ring {
            name,
            polygon,
            points,
            counter_clockwise,
        })
    }

    /// The two ends of segment `index`.
    fn ends(&self, index: usize) -> (Coord<f64>, Coord<f64>) {
        let segment = self.segments[index];
        let ring = &self.rings[segment.ring];
        (ring.point(segment.at, 0), ring.point(segment.at, 1))
    }

    /// Hold every segment against the segments whose boxes meet its own: no
    /// ring crosses or touches itself, and two rings meet in points only,
    /// without crossing.
    fn hold_segments(&mut self, steps: &mut Steps) -> Result<(), Invalid> {
        let mut near = Vec::new();
        for index in 0..self.segments.len() {
            let (from, to) = self.ends(index);
            self.tree
                .each_meeting(&bounds_of(from, to), steps, &mut |other| {
                    // Each pair once: from the lower of the two.
                    if other > index {
                        near.push(other);
                    }
                })?;
            for other in near.drain(..) {
                steps.take(PAIR_STEPS)?;
                self.hold_pair(index, other)?;
            }
        }
        Ok(())
    }

    /// Hold the segments `one` and `other` against each other.
    fn hold_pair(&mut self, one: usize, other: usize) -> Result<(), Invalid> {
        let (a, b) = (self.segments[one], self.segments[other]);
        let ((p1, p2), (q1, q2)) = (self.ends(one), self.ends(other));
        let met = meeting(p1, p2, q1, q2);
        let (name_a, name_b) = (self.rings[a.ring].name, self.rings[b.ring].name);
        if a.ring == b.ring {
            let size = self.rings[a.ring].points.len();
            let follow = (a.at + 1) % size == b.at || (b.at + 1) % size == a.at;
            return match met {
                Meeting::Apart => Ok(()),
                // Two segments in a row meet in the point they share, and
                // only there.
                Meeting::Touch(_) if follow => Ok(()),
                Meeting::Touch(_) | Meeting::Cross | Meeting::Overlap => {
                    Err(Invalid::SelfCrossing(name_a))
                }
            };
        }
        match met {
            Meeting::Apart => Ok(()),
            Meeting::Cross => Err(Invalid::RingsCross(name_a, name_b)),
            Meeting::Overlap => Err(Invalid::RingsShareALine(name_a, name_b)),
            Meeting::Touch(point) => self.hold_touch(a, b, point),
        }
    }

    /// The points of its ring on either side of `point`, which lies on
    /// `segment`: before it and after it, in the order of the ring.
    fn around(&self, segment: Segment, point: Coord<f64>) -> (Coord<f64>, Coord<f64>) {
        let ring = &self.rings[segment.ring];
        let (from, to) = (ring.point(segment.at, 0), ring.point(segment.at, 1));
        if point == from {
            (ring.point(segment.at, ring.points.len() - 1), to)
        } else if point == to {
            (from, ring.point(segment.at, 2))
        } else {
            (from, to)
        }
    }

    /// Two rings touch in `point`, on the segments `a` and `b`. They may:
    /// what they may not do is cross there. Which lies inside which is
    /// kept, because the point they touch in cannot say it later.
    fn hold_touch(&mut self, a: Segment, b: Segment, point: Coord<f64>) -> Result<(), Invalid> {
        let (around_a, around_b) = (self.around(a, point), self.around(b, point));
        let b_in_a = self.side(a.ring, b.ring, point, around_a, around_b)?;
        let a_in_b = self.side(b.ring, a.ring, point, around_b, around_a)?;
        // Two touches of the same two rings say the same. If they did
        // not, the rings would cross in between, and that crossing is
        // refused where it is found.
        self.within.insert((b.ring, a.ring), b_in_a);
        self.within.insert((a.ring, b.ring), a_in_b);
        Ok(())
    }

    /// Whether the ring `other`, which passes through `point` between the
    /// points `passing`, lies inside the ring `ring` there, which passes
    /// through it between the points `around`.
    ///
    /// # Errors
    ///
    /// When `other` goes from the inside of `ring` to its outside in
    /// `point`, or leaves `point` along `ring`.
    fn side(
        &self,
        ring: usize,
        other: usize,
        point: Coord<f64>,
        around: (Coord<f64>, Coord<f64>),
        passing: (Coord<f64>, Coord<f64>),
    ) -> Result<bool, Invalid> {
        let (before, after) = around;
        // The interior of a ring that runs counter-clockwise is on its
        // left: at a point, from the way onward round to the way back.
        let (from, to) = if self.rings[ring].counter_clockwise {
            (after, before)
        } else {
            (before, after)
        };
        let names = (self.rings[ring].name, self.rings[other].name);
        let inside =
            |d| within_sector(point, from, to, d).ok_or(Invalid::RingsShareALine(names.0, names.1));
        let (first, second) = (inside(passing.0)?, inside(passing.1)?);
        if first != second {
            return Err(Invalid::RingsCross(names.0, names.1));
        }
        Ok(first)
    }

    /// With no two rings crossing, a ring lies wholly inside another or
    /// wholly outside it. Hold each ring against the others: a hole lies
    /// inside its polygon and outside the other holes, and a polygon lies
    /// outside the other polygons or inside one of their holes.
    fn hold_nesting(&self, steps: &mut Steps) -> Result<(), Invalid> {
        if self.rings.len() < 2 {
            return Ok(());
        }
        let mut near = Vec::new();
        for ring in 0..self.rings.len() {
            let mut inside = self.rings_touched_from_inside(ring);
            // For the rings it does not touch, its first point lies off
            // them, and a ray from it crosses a ring it lies inside an odd
            // number of times. The ray goes the shortest way out of the
            // shape, so that it meets few segments: rings in a row or a
            // column are then not all held against each other.
            let point = self.rings[ring].points[0];
            let ray = Ray::shortest_out(point, &self.around);
            self.tree
                .each_meeting(&ray.bounds(point), steps, &mut |segment| {
                    near.push(segment);
                })?;
            let mut crossed: BTreeMap<usize, bool> = BTreeMap::new();
            for segment in near.drain(..) {
                steps.take(PAIR_STEPS)?;
                let other = self.segments[segment].ring;
                if other == ring || self.within.contains_key(&(ring, other)) {
                    continue;
                }
                if self.ray_crosses(ray, point, segment)? {
                    let odd = crossed.entry(other).or_insert(false);
                    *odd = !*odd;
                }
            }
            inside.extend(
                crossed
                    .into_iter()
                    .filter_map(|(other, odd)| odd.then_some(other)),
            );
            self.hold_place(ring, &inside)?;
        }
        Ok(())
    }

    /// The rings `ring` touches and lies inside of.
    fn rings_touched_from_inside(&self, ring: usize) -> BTreeSet<usize> {
        self.within
            .range((ring, 0)..=(ring, usize::MAX))
            .filter_map(|((_, other), inside)| inside.then_some(*other))
            .collect()
    }

    /// Whether `ray` from `point` crosses segment `segment`. Seen with the
    /// ray going up, a segment counts from its left end up to, not
    /// including, its right end, so a ray through a corner crosses one of
    /// the two segments that meet there; a segment along the ray counts for
    /// nothing.
    ///
    /// # Errors
    ///
    /// When `point` lies on the segment, which the rings that touch were
    /// taken out for.
    fn ray_crosses(&self, ray: Ray, point: Coord<f64>, segment: usize) -> Result<bool, Invalid> {
        let (from, to) = self.ends(segment);
        let (point, from, to) = (ray.upward(point), ray.upward(from), ray.upward(to));
        let (left, right) = match by_x_then_y(from, to) {
            Ordering::Greater => (to, from),
            _ => (from, to),
        };
        if point.x < left.x || right.x <= point.x {
            return Ok(false);
        }
        match turn(left, right, point) {
            // To the right of a segment that runs to the right: below it.
            -1 => Ok(true),
            1 => Ok(false),
            _ => {
                let name = self.rings[self.segments[segment].ring].name;
                Err(Invalid::RingsCross(name, name))
            }
        }
    }

    /// Hold `ring` against the rings it lies inside of.
    fn hold_place(&self, ring: usize, inside: &BTreeSet<usize>) -> Result<(), Invalid> {
        let this = &self.rings[ring];
        // The polygons one of whose holes it lies in.
        let in_hole_of: BTreeSet<usize> = inside
            .iter()
            .map(|other| &self.rings[*other])
            .filter(|other| other.is_hole())
            .map(|other| other.polygon)
            .collect();
        if this.is_hole() {
            let in_own_polygon = inside.iter().any(|other| {
                self.rings[*other].polygon == this.polygon && !self.rings[*other].is_hole()
            });
            if !in_own_polygon {
                return Err(Invalid::HoleOutside(this.name));
            }
            if let Some(other) = inside
                .iter()
                .map(|other| &self.rings[*other])
                .find(|other| other.polygon == this.polygon && other.is_hole())
            {
                return Err(Invalid::HoleInHole(this.name, other.name));
            }
            return Ok(());
        }
        // An outer ring inside the outer ring of another polygon lies in
        // that polygon, unless it lies in one of its holes.
        match inside
            .iter()
            .map(|other| &self.rings[*other])
            .find(|other| {
                !other.is_hole()
                    && other.polygon != this.polygon
                    && !in_hole_of.contains(&other.polygon)
            }) {
            Some(other) => Err(Invalid::PolygonsOverlap(
                other.name.polygon,
                this.name.polygon,
            )),
            None => Ok(()),
        }
    }
}

/// The way a ray goes from a point.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ray {
    Up,
    Down,
    Right,
    Left,
}

impl Ray {
    /// The way from `point` that leaves the box `around` soonest.
    fn shortest_out(point: Coord<f64>, around: &Bounds) -> Self {
        let ways = [
            (Ray::Up, around[3] - point.y),
            (Ray::Down, point.y - around[1]),
            (Ray::Right, around[2] - point.x),
            (Ray::Left, point.x - around[0]),
        ];
        ways.into_iter()
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal))
            .map_or(Ray::Up, |(ray, _)| ray)
    }

    /// The box of the ray from `point`.
    fn bounds(self, point: Coord<f64>) -> Bounds {
        let (x, y) = (point.x, point.y);
        match self {
            Ray::Up => [x, y, x, f64::INFINITY],
            Ray::Down => [x, f64::NEG_INFINITY, x, y],
            Ray::Right => [x, y, f64::INFINITY, y],
            Ray::Left => [f64::NEG_INFINITY, y, x, y],
        }
    }

    /// `point` as it lies when the plane is turned over so that this ray
    /// goes up. Turning the plane over keeps what crosses what.
    fn upward(self, point: Coord<f64>) -> Coord<f64> {
        let (x, y) = (point.x, point.y);
        match self {
            Ray::Up => Coord { x, y },
            Ray::Down => Coord { x, y: -y },
            Ray::Right => Coord { x: y, y: x },
            Ray::Left => Coord { x: y, y: -x },
        }
    }
}

/// A box: its lowest x and y, and its highest x and y.
type Bounds = [f64; 4];

/// The box around nothing: widened by a box, it is that box.
const NOTHING: Bounds = [
    f64::INFINITY,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::NEG_INFINITY,
];

/// The box around two boxes.
fn widened(a: &Bounds, b: &Bounds) -> Bounds {
    [
        a[0].min(b[0]),
        a[1].min(b[1]),
        a[2].max(b[2]),
        a[3].max(b[3]),
    ]
}

/// The box around two points.
fn bounds_of(a: Coord<f64>, b: Coord<f64>) -> Bounds {
    [a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)]
}

/// Whether two boxes have a point in common.
fn boxes_meet(a: &Bounds, b: &Bounds) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

/// A tree of boxes, packed once: boxes that lie near each other share a
/// node, and a node is the box around its boxes. Asking which boxes meet a
/// given one then looks only into the nodes that meet it.
struct BoxTree {
    /// The boxes level by level. Level 0 holds the boxes that were packed,
    /// in the order they were packed in; a box at a higher level is the box
    /// around up to [`FANOUT`] boxes in a row of the level below.
    levels: Vec<Vec<Bounds>>,
    /// For each box of level 0, which of the packed boxes it is.
    items: Vec<usize>,
}

impl BoxTree {
    /// Pack `boxes`: sort them into strips by x and within a strip by y, so
    /// that boxes in a row lie near each other, and build the levels above.
    fn pack(boxes: &[Bounds]) -> Self {
        let middle = |item: &usize, axis: usize| boxes[*item][axis] + boxes[*item][axis + 2];
        let by = |axis: usize| {
            move |a: &usize, b: &usize| {
                middle(a, axis)
                    .partial_cmp(&middle(b, axis))
                    .unwrap_or(Ordering::Equal)
            }
        };
        let mut items: Vec<usize> = (0..boxes.len()).collect();
        items.sort_by(by(0));
        // As many strips as a strip has nodes, so a node is about square.
        let nodes = boxes.len().div_ceil(FANOUT);
        let strip = FANOUT * nodes.isqrt().max(1);
        for strip in items.chunks_mut(strip) {
            strip.sort_by(by(1));
        }
        let mut levels = vec![items.iter().map(|item| boxes[*item]).collect::<Vec<_>>()];
        while let Some(below) = levels.last().filter(|level| level.len() > FANOUT) {
            let above = below
                .chunks(FANOUT)
                .map(|node| {
                    node.iter()
                        .fold(NOTHING, |around, bounds| widened(&around, bounds))
                })
                .collect();
            levels.push(above);
        }
        BoxTree { levels, items }
    }

    /// Call `visit` with every packed box that meets `query`, by its number.
    /// Each box that is compared is a step.
    fn each_meeting(
        &self,
        query: &Bounds,
        steps: &mut Steps,
        visit: &mut dyn FnMut(usize),
    ) -> Result<(), Invalid> {
        let top = self.levels.len() - 1;
        let mut open = vec![(top, 0, self.levels[top].len())];
        while let Some((level, from, to)) = open.pop() {
            for at in from..to {
                steps.take(1)?;
                if !boxes_meet(&self.levels[level][at], query) {
                    continue;
                }
                match level.checked_sub(1) {
                    None => visit(self.items[at]),
                    Some(below) => {
                        let end = ((at + 1) * FANOUT).min(self.levels[below].len());
                        open.push((below, at * FANOUT, end));
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Steps enough for any shape of a test that is not about the limit.
    const AMPLE: u64 = 10_000_000;

    fn geometry(text: &str) -> Geometry<f64> {
        let parsed = wkt::Wkt::<f64>::from_str(text).unwrap_or_else(|e| panic!("{text}: {e}"));
        Geometry::try_from(parsed).unwrap()
    }

    fn checked(text: &str) -> Result<u64, Invalid> {
        check(&geometry(text), AMPLE)
    }

    /// What makes `text` invalid, in words.
    fn refusal(text: &str) -> String {
        checked(text).unwrap_err().to_string()
    }

    /// A polygon of `corners` corners on a circle.
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

    fn at(x: f64, y: f64) -> Coord<f64> {
        Coord { x, y }
    }

    /// How the segment between the two points of `p` meets the one of `q`.
    fn met(p: [(f64, f64); 2], q: [(f64, f64); 2]) -> Meeting {
        let ends = |s: [(f64, f64); 2]| (at(s[0].0, s[0].1), at(s[1].0, s[1].1));
        let ((p1, p2), (q1, q2)) = (ends(p), ends(q));
        meeting(p1, p2, q1, q2)
    }

    #[test]
    fn two_segments_cross_overlap_touch_or_lie_apart() {
        let flat = [(0.0, 0.0), (4.0, 0.0)];
        // In a point interior to both.
        assert_eq!(
            met([(0.0, 0.0), (4.0, 4.0)], [(0.0, 4.0), (4.0, 0.0)]),
            Meeting::Cross
        );
        assert_eq!(met(flat, [(2.0, -1.0), (2.0, 1.0)]), Meeting::Cross);

        // The end of one on the other: whichever end, whichever of the two,
        // from above and from below.
        let touch = Meeting::Touch(at(2.0, 0.0));
        for upright in [
            [(2.0, 0.0), (2.0, 3.0)],
            [(2.0, 3.0), (2.0, 0.0)],
            [(2.0, 0.0), (2.0, -3.0)],
            [(2.0, -3.0), (2.0, 0.0)],
        ] {
            assert_eq!(met(flat, upright), touch, "{upright:?}");
            assert_eq!(met(upright, flat), touch, "{upright:?}");
        }
        // End to end, on one line and around a corner.
        assert_eq!(
            met([(0.0, 0.0), (2.0, 0.0)], [(2.0, 0.0), (4.0, 0.0)]),
            touch
        );
        assert_eq!(
            met([(2.0, 0.0), (4.0, 0.0)], [(0.0, 0.0), (2.0, 0.0)]),
            touch
        );
        assert_eq!(
            met([(0.0, 0.0), (2.0, 0.0)], [(2.0, 0.0), (2.0, 2.0)]),
            touch
        );
        assert_eq!(
            met([(0.0, 0.0), (0.0, 4.0)], [(0.0, 4.0), (0.0, 6.0)]),
            Meeting::Touch(at(0.0, 4.0))
        );

        // Along a line: partly, one within the other, the same, the other
        // way round, and upright.
        for other in [
            [(2.0, 0.0), (6.0, 0.0)],
            [(1.0, 0.0), (2.0, 0.0)],
            [(0.0, 0.0), (4.0, 0.0)],
            [(4.0, 0.0), (0.0, 0.0)],
            [(-3.0, 0.0), (1.0, 0.0)],
        ] {
            assert_eq!(met(flat, other), Meeting::Overlap, "{other:?}");
            assert_eq!(met(other, flat), Meeting::Overlap, "{other:?}");
        }
        assert_eq!(
            met([(0.0, 0.0), (0.0, 4.0)], [(0.0, 2.0), (0.0, 6.0)]),
            Meeting::Overlap
        );

        // Apart: on one line with a gap, side by side, past the end of the
        // other on its line, short of it, and beside it.
        for other in [
            [(5.0, 0.0), (7.0, 0.0)],
            [(-3.0, 0.0), (-1.0, 0.0)],
            [(0.0, 1.0), (4.0, 1.0)],
            [(6.0, 0.0), (6.0, 3.0)],
            [(6.0, 3.0), (6.0, 0.0)],
            [(-2.0, 0.0), (-2.0, 3.0)],
            [(2.0, 1.0), (2.0, 3.0)],
            [(5.0, -1.0), (5.0, 1.0)],
        ] {
            assert_eq!(met(flat, other), Meeting::Apart, "{other:?}");
            assert_eq!(met(other, flat), Meeting::Apart, "{other:?}");
        }
        assert_eq!(
            met([(0.0, 0.0), (0.0, 1.0)], [(0.0, 2.0), (0.0, 3.0)]),
            Meeting::Apart
        );
    }

    #[test]
    fn a_ray_lies_inside_a_sector_outside_it_or_along_its_edge() {
        let p = at(10.0, 10.0);
        // Eight ways from p, counter-clockwise from east.
        let ways = [
            (1.0, 0.0),
            (1.0, 1.0),
            (0.0, 1.0),
            (-1.0, 1.0),
            (-1.0, 0.0),
            (-1.0, -1.0),
            (0.0, -1.0),
            (1.0, -1.0),
        ]
        .map(|(x, y)| at(10.0 + x, 10.0 + y));
        let (east, north, west) = (ways[0], ways[2], ways[4]);
        let sector = |from, to| ways.map(|way| within_sector(p, from, to, way));
        let (inside, outside) = (Some(true), Some(false));
        // A quarter turn, from east to north: north-east lies inside.
        assert_eq!(
            sector(east, north),
            [
                None, inside, None, outside, outside, outside, outside, outside
            ]
        );
        // Three quarters, from north round to east: north-east lies outside.
        assert_eq!(
            sector(north, east),
            [None, outside, None, inside, inside, inside, inside, inside]
        );
        // Half a turn, from east to west and from west to east.
        assert_eq!(
            sector(east, west),
            [
                None, inside, inside, inside, None, outside, outside, outside
            ]
        );
        assert_eq!(
            sector(west, east),
            [
                None, outside, outside, outside, None, inside, inside, inside
            ]
        );
        // From an upright ray, where what lies along it is told by y.
        assert_eq!(
            sector(north, west),
            [
                outside, outside, None, inside, None, outside, outside, outside
            ]
        );
        // And when the points lie further along the rays than the corners.
        assert_eq!(within_sector(p, east, north, at(40.0, 10.0)), None);
        assert_eq!(within_sector(p, east, north, at(10.0, 90.0)), None);
        assert_eq!(within_sector(p, east, north, at(-40.0, 10.0)), outside);
    }

    #[test]
    fn boxes_meet_when_they_have_a_point_in_common() {
        let unit = [0.0, 0.0, 1.0, 1.0];
        // Over each other, edge to edge, corner to corner.
        for other in [
            [0.5, 0.5, 2.0, 2.0],
            [1.0, 0.0, 2.0, 1.0],
            [0.0, 1.0, 1.0, 2.0],
            [-1.0, 0.0, 0.0, 1.0],
            [0.0, -1.0, 1.0, 0.0],
            [1.0, 1.0, 2.0, 2.0],
            [-1.0, -1.0, 0.0, 0.0],
            [0.25, 0.25, 0.75, 0.75],
        ] {
            assert!(boxes_meet(&unit, &other), "{other:?}");
            assert!(boxes_meet(&other, &unit), "{other:?}");
        }
        // Apart on one side at a time.
        for other in [
            [1.5, 0.0, 2.0, 1.0],
            [-2.0, 0.0, -0.5, 1.0],
            [0.0, 1.5, 1.0, 2.0],
            [0.0, -2.0, 1.0, -0.5],
        ] {
            assert!(!boxes_meet(&unit, &other), "{other:?}");
            assert!(!boxes_meet(&other, &unit), "{other:?}");
        }
        assert_eq!(
            bounds_of(at(3.0, -1.0), at(1.0, 5.0)),
            [1.0, -1.0, 3.0, 5.0]
        );
        assert_eq!(
            widened(&unit, &[-2.0, 0.5, 0.5, 3.0]),
            [-2.0, 0.0, 1.0, 3.0]
        );
        assert_eq!(widened(&NOTHING, &unit), unit);
    }

    #[test]
    fn the_tree_gives_exactly_the_boxes_that_meet_a_box() {
        // Three hundred boxes of several sizes, some over each other, on
        // both sides of zero.
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = |below: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % below) as f64
        };
        let boxes: Vec<Bounds> = (0..300)
            .map(|_| {
                let (x, y) = (next(1000) - 500.0, next(1000) - 500.0);
                [x, y, x + next(60), y + next(60)]
            })
            .collect();
        let tree = BoxTree::pack(&boxes);
        let mut steps = Steps {
            taken: 0,
            allowed: AMPLE,
        };
        let queries = boxes.iter().copied().chain([
            [-600.0, -600.0, 600.0, 600.0],
            [5000.0, 5000.0, 6000.0, 6000.0],
        ]);
        for query in queries {
            let mut found = Vec::new();
            tree.each_meeting(&query, &mut steps, &mut |item| found.push(item))
                .unwrap();
            found.sort_unstable();
            let meeting: Vec<usize> = (0..boxes.len())
                .filter(|item| boxes_meet(&boxes[*item], &query))
                .collect();
            assert_eq!(found, meeting, "{query:?}");
        }
        // How the boxes are packed decides how many of them a question
        // compares, and so what reading a shape costs. For these boxes that
        // is this many, far fewer than there are pairs of boxes; a change of
        // the packing changes the number, and is a change of that cost.
        assert_eq!(steps.taken, 9_686);
        // As many boxes as a node holds are one node: a question compares
        // each of them, and nothing above them.
        let node = BoxTree::pack(&boxes[..FANOUT]);
        let mut compared = Steps {
            taken: 0,
            allowed: AMPLE,
        };
        node.each_meeting(
            &[5000.0, 5000.0, 6000.0, 6000.0],
            &mut compared,
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(compared.taken, FANOUT as u64);
        // A tree of one box, and of none.
        let one = BoxTree::pack(&boxes[..1]);
        let mut found = Vec::new();
        one.each_meeting(&boxes[0], &mut steps, &mut |item| found.push(item))
            .unwrap();
        assert_eq!(found, [0]);
        let none = BoxTree::pack(&[]);
        none.each_meeting(&boxes[0], &mut steps, &mut |item| found.push(item))
            .unwrap();
        assert_eq!(found, [0]);
    }

    #[test]
    fn a_ray_goes_the_shortest_way_out_of_the_shape() {
        // A box of forty by forty that does not start at zero.
        let around = [10.0, 20.0, 50.0, 60.0];
        assert_eq!(Ray::shortest_out(at(30.0, 56.0), &around), Ray::Up);
        assert_eq!(Ray::shortest_out(at(30.0, 23.0), &around), Ray::Down);
        assert_eq!(Ray::shortest_out(at(47.0, 40.0), &around), Ray::Right);
        assert_eq!(Ray::shortest_out(at(13.0, 40.0), &around), Ray::Left);
        let point = at(3.0, 7.0);
        assert_eq!(Ray::Up.bounds(point), [3.0, 7.0, 3.0, f64::INFINITY]);
        assert_eq!(Ray::Down.bounds(point), [3.0, f64::NEG_INFINITY, 3.0, 7.0]);
        assert_eq!(Ray::Right.bounds(point), [3.0, 7.0, f64::INFINITY, 7.0]);
        assert_eq!(Ray::Left.bounds(point), [f64::NEG_INFINITY, 7.0, 3.0, 7.0]);
        // Turned over, each ray goes up: what lay ahead of it has a higher y.
        assert_eq!(Ray::Up.upward(point), at(3.0, 7.0));
        assert_eq!(Ray::Down.upward(point), at(3.0, -7.0));
        assert_eq!(Ray::Right.upward(point), at(7.0, 3.0));
        assert_eq!(Ray::Left.upward(point), at(7.0, -3.0));
    }

    #[test]
    fn nesting_is_found_whichever_way_the_ray_goes() {
        // A small square near each side of a large one: the ray from it
        // leaves the large one through that side.
        const LARGE: &str = "(0 0, 100 0, 100 100, 0 100, 0 0)";
        const HOLE: &str = "(4 4, 96 4, 96 96, 4 96, 4 4)";
        let square = |x: i32, y: i32| {
            format!(
                "({x} {y}, {} {y}, {} {}, {x} {}, {x} {y})",
                x + 2,
                x + 2,
                y + 2,
                y + 2
            )
        };
        for (x, y) in [(48, 90), (48, 6), (90, 48), (6, 48)] {
            let small = square(x, y);
            // In the large one it overlaps it; in its hole it does not.
            assert_eq!(
                refusal(&format!("MULTIPOLYGON(({LARGE}), ({small}))")),
                "polygons 1 and 2 overlap",
                "{x} {y}"
            );
            assert_eq!(
                refusal(&format!("MULTIPOLYGON(({small}), ({LARGE}))")),
                "polygons 2 and 1 overlap",
                "{x} {y}"
            );
            let in_hole = format!("MULTIPOLYGON(({LARGE}, {HOLE}), ({small}))");
            assert!(checked(&in_hole).is_ok(), "{in_hole}");
            // As a hole of the large one it lies inside it.
            let holed = format!("POLYGON({LARGE}, {small})");
            assert!(checked(&holed).is_ok(), "{holed}");
        }
        // Beyond each side of the large one, a square lies outside it.
        for (x, y) in [(48, 110), (48, -20), (110, 48), (-20, 48)] {
            let small = square(x, y);
            let beside = format!("MULTIPOLYGON(({LARGE}), ({small}))");
            assert!(checked(&beside).is_ok(), "{beside}");
            assert_eq!(
                refusal(&format!("POLYGON({LARGE}, {small})")),
                "hole 1 of polygon 1 does not lie inside its polygon",
                "{x} {y}"
            );
        }
    }

    #[test]
    fn a_ray_through_a_corner_or_along_a_side_counts_what_it_crosses() {
        // A square on its point. The first point of a small triangle lies
        // straight below its top, above its bottom, left of its right
        // corner or right of its left one: the ray from it goes through
        // that corner, where two sides meet, and crosses the ring once.
        const ON_ITS_POINT: &str = "(50 100, 0 50, 50 0, 100 50, 50 100)";
        for (x, y) in [(50, 90), (50, 10), (90, 50), (10, 50)] {
            let small = format!("(({x} {y}, {} {y}, {x} {}, {x} {y}))", x + 1, y + 1);
            assert_eq!(
                refusal(&format!("MULTIPOLYGON(({ON_ITS_POINT}), {small})")),
                "polygons 1 and 2 overlap",
                "{x} {y}"
            );
        }
        // A step: the ray up from 50 70 meets the corner at 50 80, runs
        // along the upright side to 50 100 and leaves through that corner.
        const STEP: &str = "(0 0, 100 0, 100 100, 50 100, 50 80, 0 80, 0 0)";
        assert_eq!(
            refusal(&format!(
                "MULTIPOLYGON(({STEP}), ((50 70, 51 70, 50 71, 50 70)))"
            )),
            "polygons 1 and 2 overlap"
        );
        // A triangle with a corner on that upright side touches the step
        // there: inside it on one side, outside it on the other.
        assert_eq!(
            refusal(&format!(
                "MULTIPOLYGON(({STEP}), ((50 85, 60 85, 60 90, 50 85)))"
            )),
            "polygons 1 and 2 overlap"
        );
        let outside = format!("MULTIPOLYGON(({STEP}), ((50 85, 40 85, 40 90, 50 85)))");
        assert!(checked(&outside).is_ok(), "{:?}", checked(&outside));
    }

    #[test]
    fn a_hole_inside_another_polygon_is_still_outside_its_own() {
        assert_eq!(
            refusal(
                "MULTIPOLYGON(((0 0, 10 0, 10 10, 0 10, 0 0), (20 2, 22 2, 22 4, 20 4, 20 2)), \
                 ((15 0, 30 0, 30 10, 15 10, 15 0)))"
            ),
            "hole 1 of polygon 1 does not lie inside its polygon"
        );
    }

    #[test]
    fn a_corner_of_one_ring_on_a_side_of_another_is_a_touch() {
        // The corner 4 4 of the first polygon lies on a side of the second,
        // which comes later in the text, and the other way round.
        for text in [
            "MULTIPOLYGON(((0 0, 4 0, 4 4, 0 4, 0 0)), ((2 6, 6 2, 8 8, 2 6)))",
            "MULTIPOLYGON(((2 6, 6 2, 8 8, 2 6)), ((0 0, 4 0, 4 4, 0 4, 0 0)))",
        ] {
            assert!(checked(text).is_ok(), "{text}: {:?}", checked(text));
        }
        // A ring that goes through the side at its corner crosses it.
        assert!(
            refusal("MULTIPOLYGON(((0 0, 4 0, 4 4, 5 6, 0 4, 0 0)), ((2 6, 6 2, 8 8, 2 6)))")
                .ends_with("cross")
        );
    }

    #[test]
    fn a_ring_needs_three_distinct_points() {
        for text in [
            "POLYGON((0 0, 1 1, 0 0))",
            "POLYGON((0 0, 0 0, 1 1, 1 1, 0 0))",
            "POLYGON((0 0, 10 0, 10 10, 0 10, 0 0), (2 2, 3 3, 2 2))",
        ] {
            assert!(
                refusal(text).contains("has fewer than 3 distinct points"),
                "{text}"
            );
        }
        // A point written twice in a row is one point.
        assert!(checked("POLYGON((0 0, 0 0, 10 0, 10 10, 10 10, 0 0))").is_ok());
    }

    #[test]
    fn a_ring_that_crosses_touches_or_retraces_itself_is_refused() {
        for text in [
            // A bow tie.
            "POLYGON((0 0, 10 10, 10 0, 0 10, 0 0))",
            // Two triangles that meet in a point of the ring.
            "POLYGON((0 0, 5 5, 10 0, 10 10, 5 5, 0 10, 0 0))",
            // A corner that lies on another side.
            "POLYGON((0 0, 10 0, 10 10, 5 0, 0 10, 0 0))",
            // A spike: out and back along the same line.
            "POLYGON((0 0, 10 0, 10 10, 15 10, 10 10, 0 10, 0 0))",
            // Three points on a line.
            "POLYGON((0 0, 5 0, 10 0, 0 0))",
            // A spike at the lowest point, where the turn is read.
            "POLYGON((0 0, 5 5, 0 0, 5 0, 5 5, 0 0))",
        ] {
            assert_eq!(
                refusal(text),
                "the outer ring of polygon 1 crosses or touches itself",
                "{text}"
            );
        }
    }

    #[test]
    fn rings_may_touch_in_a_point_and_not_cross_or_run_along_each_other() {
        // A hole that touches the outer ring in one corner, on a side and
        // on a corner of it; two holes that touch in a corner.
        for text in [
            "POLYGON((0 0, 10 0, 10 10, 0 10, 0 0), (0 5, 3 3, 3 7, 0 5))",
            "POLYGON((0 0, 10 0, 10 10, 0 10, 0 0), (0 0, 3 1, 1 3, 0 0))",
            "POLYGON((0 0, 10 0, 10 10, 0 10, 0 0), (2 2, 4 2, 4 4, 2 4, 2 2), (4 4, 6 4, 6 6, 4 6, 4 4))",
            // Either way round.
            "POLYGON((0 0, 0 10, 10 10, 10 0, 0 0), (0 5, 3 7, 3 3, 0 5))",
        ] {
            assert!(checked(text).is_ok(), "{text}: {:?}", checked(text));
        }
        assert_eq!(
            refusal("POLYGON((0 0, 10 0, 10 10, 0 10, 0 0), (5 5, 15 5, 15 8, 5 8, 5 5))"),
            "the outer ring of polygon 1 and hole 1 of polygon 1 cross"
        );
        // A hole that goes through a corner of the outer ring to the outside.
        assert_eq!(
            refusal("POLYGON((0 0, 10 0, 10 10, 0 10, 0 0), (5 5, 10 10, 12 14, 5 5))"),
            "the outer ring of polygon 1 and hole 1 of polygon 1 cross"
        );
        // And one that goes through a point of a side.
        assert!(
            refusal("POLYGON((0 0, 10 0, 10 10, 0 10, 0 0), (5 5, 10 5, 14 6, 5 5))")
                .ends_with("cross")
        );
        assert_eq!(
            refusal("POLYGON((0 0, 10 0, 10 10, 0 10, 0 0), (0 2, 3 2, 3 8, 0 8, 0 2))"),
            "the outer ring of polygon 1 and hole 1 of polygon 1 run along each other"
        );
    }

    #[test]
    fn a_hole_lies_inside_its_polygon_and_outside_the_other_holes() {
        assert_eq!(
            refusal("POLYGON((0 0, 10 0, 10 10, 0 10, 0 0), (20 20, 30 20, 30 30, 20 30, 20 20))"),
            "hole 1 of polygon 1 does not lie inside its polygon"
        );
        // A hole around the polygon.
        assert_eq!(
            refusal("POLYGON((0 0, 10 0, 10 10, 0 10, 0 0), (-5 -5, 15 -5, 15 15, -5 15, -5 -5))"),
            "hole 1 of polygon 1 does not lie inside its polygon"
        );
        // One that touches the polygon from the outside.
        assert_eq!(
            refusal("POLYGON((0 0, 10 0, 10 10, 0 10, 0 0), (10 5, 13 3, 13 7, 10 5))"),
            "hole 1 of polygon 1 does not lie inside its polygon"
        );
        assert_eq!(
            refusal(
                "POLYGON((0 0, 10 0, 10 10, 0 10, 0 0), (1 1, 9 1, 9 9, 1 9, 1 1), (3 3, 6 3, 6 6, 3 6, 3 3))"
            ),
            "hole 2 of polygon 1 lies inside hole 1 of polygon 1"
        );
        // Also where the inner one touches the outer one.
        assert_eq!(
            refusal(
                "POLYGON((0 0, 10 0, 10 10, 0 10, 0 0), (1 1, 9 1, 9 9, 1 9, 1 1), (1 5, 4 3, 4 7, 1 5))"
            ),
            "hole 2 of polygon 1 lies inside hole 1 of polygon 1"
        );
    }

    #[test]
    fn the_polygons_of_a_multipolygon_do_not_overlap() {
        for text in [
            // Side by side, touching in a corner; a board of squares that
            // touch in every corner; one in the hole of another.
            "MULTIPOLYGON(((0 0, 5 0, 5 5, 0 5, 0 0)), ((5 5, 9 5, 9 9, 5 9, 5 5)))",
            "MULTIPOLYGON(((0 0, 1 0, 1 1, 0 1, 0 0)), ((1 1, 2 1, 2 2, 1 2, 1 1)), ((0 2, 1 2, 1 3, 0 3, 0 2)), ((2 0, 3 0, 3 1, 2 1, 2 0)))",
            "MULTIPOLYGON(((0 0, 10 0, 10 10, 0 10, 0 0), (2 2, 8 2, 8 8, 2 8, 2 2)), ((3 3, 7 3, 7 7, 3 7, 3 3)))",
            // In the hole and touching its ring.
            "MULTIPOLYGON(((0 0, 10 0, 10 10, 0 10, 0 0), (2 2, 8 2, 8 8, 2 8, 2 2)), ((2 2, 7 3, 3 7, 2 2)))",
        ] {
            assert!(checked(text).is_ok(), "{text}: {:?}", checked(text));
        }
        assert_eq!(
            refusal("MULTIPOLYGON(((0 0, 10 0, 10 10, 0 10, 0 0)), ((3 3, 7 3, 7 7, 3 7, 3 3)))"),
            "polygons 1 and 2 overlap"
        );
        // The inner one first.
        assert_eq!(
            refusal("MULTIPOLYGON(((3 3, 7 3, 7 7, 3 7, 3 3)), ((0 0, 10 0, 10 10, 0 10, 0 0)))"),
            "polygons 2 and 1 overlap"
        );
        // Inside and touching the outer ring.
        assert_eq!(
            refusal("MULTIPOLYGON(((0 0, 10 0, 10 10, 0 10, 0 0)), ((0 0, 7 3, 3 7, 0 0)))"),
            "polygons 1 and 2 overlap"
        );
        assert_eq!(
            refusal(
                "MULTIPOLYGON(((0 0, 10 0, 10 10, 0 10, 0 0)), ((5 5, 15 5, 15 15, 5 15, 5 5)))"
            ),
            "the outer ring of polygon 1 and the outer ring of polygon 2 cross"
        );
        assert_eq!(
            refusal(
                "MULTIPOLYGON(((0 0, 10 0, 10 10, 0 10, 0 0)), ((10 0, 20 0, 20 10, 10 10, 10 0)))"
            ),
            "the outer ring of polygon 1 and the outer ring of polygon 2 run along each other"
        );
    }

    #[test]
    fn a_point_or_a_line_is_checked_as_before() {
        assert_eq!(checked("POINT(1 1)"), Ok(0));
        assert_eq!(checked("LINESTRING(0 0, 5 5)"), Ok(0));
        assert_eq!(
            refusal("LINESTRING(1 1, 1 1)"),
            "line string must have at least 2 distinct points"
        );
    }

    #[test]
    fn it_agrees_with_geo_on_small_shapes() {
        // Rings drawn from a small grid meet each other in every way there
        // is. Whatever this check accepts, `geo` accepts too. It refuses
        // more: a ring that runs back over itself, which `geo` lets pass.
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        let mut next = |below: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % below
        };
        let ring = |next: &mut dyn FnMut(u64) -> u64| {
            let corners = 3 + next(3);
            let mut points: Vec<String> = (0..corners)
                .map(|_| format!("{} {}", next(5), next(5)))
                .collect();
            points.push(points[0].clone());
            format!("({})", points.join(", "))
        };
        let (mut accepted, mut refused) = (0, 0);
        for round in 0..6000 {
            let text = match round % 3 {
                0 => format!("POLYGON({})", ring(&mut next)),
                1 => format!("POLYGON({}, {})", ring(&mut next), ring(&mut next)),
                _ => format!("MULTIPOLYGON(({}), ({}))", ring(&mut next), ring(&mut next)),
            };
            let shape = geometry(&text);
            match check(&shape, AMPLE) {
                Ok(_) => {
                    accepted += 1;
                    assert!(shape.is_valid(), "{text} is accepted, and geo refuses it");
                }
                Err(_) => refused += 1,
            }
        }
        // Both happen, or the comparison says nothing.
        assert!(accepted > 100 && refused > 100, "{accepted} {refused}");
    }

    #[test]
    fn the_steps_grow_with_the_size_and_not_with_its_square() {
        let steps = |corners| checked(&circle(corners)).unwrap();
        let (small, large) = (steps(2_000), steps(32_000));
        // Sixteen times the corners: with n log n about twenty times the
        // steps, with n² two hundred and fifty-six times.
        assert!(
            large < 32 * small,
            "{small} steps for 2000, {large} for 32000"
        );
        assert!(large < 200 * 32_000, "{large} steps for 32000 corners");
    }

    #[test]
    fn a_shape_that_takes_too_many_steps_is_refused() {
        // Long thin strips that lean the same way: the box of each covers
        // the others, so every side is held against every other.
        let strips: Vec<String> = (0..200)
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
        let text = format!("MULTIPOLYGON({})", strips.join(", "));
        let shape = geometry(&text);
        let needed = check(&shape, AMPLE).unwrap();
        assert!(needed > 400_000, "{needed}");
        assert_eq!(
            check(&shape, needed - 1),
            Err(Invalid::TooCostly {
                allowed: needed - 1
            })
        );
        assert_eq!(check(&shape, needed), Ok(needed));
        assert!(
            Invalid::TooCostly { allowed: 7 }
                .to_string()
                .starts_with("checking it takes more than 7 steps")
        );
    }

    #[test]
    fn a_ray_through_a_corner_counts_once() {
        // The first point of the inner ring lies straight below a corner of
        // the hole and of the outer ring.
        assert!(
            checked(
                "MULTIPOLYGON(((0 0, 10 0, 5 10, 0 0), (3 2, 7 2, 5 6, 3 2)), ((5 3, 6 3, 5 4, 5 3)))"
            )
            .is_ok()
        );
        assert_eq!(
            refusal("MULTIPOLYGON(((0 0, 10 0, 5 10, 0 0)), ((5 3, 6 3, 5 4, 5 3)))"),
            "polygons 1 and 2 overlap"
        );
        // Below an upright side, which counts for nothing.
        assert!(
            checked("MULTIPOLYGON(((0 5, 4 5, 4 9, 0 9, 0 5)), ((4 0, 6 0, 6 2, 4 2, 4 0)))")
                .is_ok()
        );
    }
}
