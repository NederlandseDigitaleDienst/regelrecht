//! Configuration constants for the RegelRecht engine
//!
//! Centralized configuration values used throughout the engine for:
//! - Security limits (prevent DoS attacks)
//! - Resource constraints (memory, CPU)
//! - Recursion depth limits (prevent stack overflow)
//!
//! # Security Considerations
//!
//! These limits are designed to prevent:
//! - YAML bombs (deeply nested or very large documents)
//! - Infinite recursion (circular references)
//! - Memory exhaustion (too many laws or large arrays)
//!
//! # Customization
//!
//! Currently these are compile-time constants. Future versions may
//! support runtime configuration via environment variables or a
//! configuration file.

/// Maximum number of laws that can be loaded simultaneously.
///
/// Prevents memory exhaustion from loading too many laws.
/// 100 laws is sufficient for most use cases (Dutch legal system
/// typically involves ~10-20 interconnected regulations).
pub const MAX_LOADED_LAWS: usize = 100;

/// Maximum YAML document size in bytes (1 MB).
///
/// Prevents YAML bomb attacks and excessive memory usage during parsing.
/// 1 MB is sufficient for any reasonable law document (typical laws are 10-100 KB).
pub const MAX_YAML_SIZE: usize = 1_000_000;

/// Maximum number of elements in any array within a law document.
///
/// Prevents DoS via documents with extremely large arrays.
/// 1000 elements is sufficient for any reasonable law structure.
pub const MAX_ARRAY_SIZE: usize = 1_000;

/// Maximum combined depth for reference resolution in the service layer.
///
/// A single shared counter governs cross-law reference chains and same-law
/// (internal) article-reference chains *together*: every hop of either kind
/// draws on this one budget within a resolution chain, not on separate
/// per-kind budgets. Prevents infinite loops and stack overflow.
/// 20 levels is conservative: cross-law chains in Dutch regulations are
/// typically 3-5 levels (Wet -> Ministeriele Regeling -> Gemeentelijke
/// Verordening), with internal article chains within a law adding only a few
/// more — well under the shared budget.
pub const MAX_CROSS_LAW_DEPTH: usize = 20;

/// Maximum nesting depth for operations during evaluation.
///
/// Prevents stack overflow from deeply nested operation expressions.
/// 100 levels is sufficient for complex calculations while preventing abuse.
pub const MAX_OPERATION_DEPTH: usize = 100;

/// Schema versions supported by this engine version (RFC-013).
///
/// A regulation referencing a schema version outside this list will be
/// rejected at load time. This list must match the `supported-schemas`
/// metadata in Cargo.toml.
pub const SUPPORTED_SCHEMAS: &[&str] = &[
    "v0.2.0", "v0.3.0", "v0.3.1", "v0.3.2", "v0.4.0", "v0.5.0", "v0.5.1", "v0.5.2", "v0.5.3",
    "v0.5.4", "v0.5.5", "v0.5.6", "v0.5.7", "v0.5.8", "v0.5.9", "v0.6.0", "v0.7.0", "v0.7.1",
    "v0.8.0", "v0.8.1", "v0.8.2", "v0.8.3", "v0.8.4", "v0.8.5", "v0.8.6", "v0.8.7", "v0.8.8",
    "v0.8.9", "v0.8.10", "v0.8.11", "v0.8.12", "v0.8.13", "v0.8.14", "v0.8.15", "v0.8.16",
    "v0.8.17", "v0.8.18", "v0.8.19", "v0.8.20", "v0.8.21", "v0.8.22", "v0.8.23",
];

/// Maximum quote length (in `char`s) for the fuzzy annotation scan.
///
/// The sliding-window fuzzy match in `annotation::resolver` is cubic in the
/// quote length; unbounded it freezes the browser main thread for minutes on
/// a long quote (the resolver also runs synchronously via WASM). Above this
/// length only the exact match runs and the result is reported as
/// [`crate::annotation::MatchStatus::Skipped`], so a user can tell "not
/// searched" apart from "not found". 120 chars is well past the RFC-018
/// write-path context (40) and covers any quote a note reasonably anchors on.
pub const MAX_FUZZY_QUOTE_CHARS: usize = 120;

/// Maximum law text (in `char`s) one fuzzy resolve may scan.
///
/// Bounds the sliding-window scan across articles: articles that no longer
/// fit in the budget are not fuzzily searched and the result reports
/// `Skipped` instead of pretending the whole law was searched. 250k chars is
/// roughly twice the Zorgverzekeringswet (~120k), so ordinary laws are fully
/// scanned.
pub const MAX_FUZZY_SCAN_CHARS: usize = 250_000;

/// Maximum number of candidate windows one fuzzy resolve may score.
///
/// Scoring a window costs three Levenshtein computations (quadratic in the
/// quote length). The pre-filter is cheap, but on a law that repeats the
/// quote's words everywhere it passes almost every window; this cap bounds
/// the total work regardless. Exceeding it truncates the scan and reports
/// `Skipped` when nothing was found by then.
pub const MAX_FUZZY_SCORED_WINDOWS: usize = 10_000;

/// Maximum size of the text of a shape in bytes (4 MB).
///
/// A shape reaches the engine as well-known text from outside: a parameter,
/// a register, a literal. The one real boundary measured here, the
/// municipality of Putten at the detail the cadastral register gives it, is
/// 2,185 points in 49 kB of text; a province will be many times that. A
/// whole rulework may be 1 MB ([`MAX_YAML_SIZE`]), and four times that holds
/// about 170,000 points written to the millimetre in the national grid.
///
/// The cap bounds the text that is parsed, which costs time in proportion to
/// its length. It does not bound the validity check by itself; that has a
/// limit of its own, [`MAX_SHAPE_CHECK_STEPS`].
pub const MAX_SHAPE_TEXT_BYTES: usize = 4_000_000;

/// Maximum number of steps the validity check of one shape may take.
///
/// A step is one comparison of two bounding boxes; a pair of segments that
/// is held against each other counts for eight
/// (see [`crate::shape_validity`]). The check finds the segments that lie
/// near each other through a tree of boxes, so a boundary takes a number of
/// steps that grows with n log n: measured, 41 per point for the boundary
/// of Putten (2,185 points), and between 50 and 90 per point for generated
/// boundaries of 8,000 to 500,000 points, smooth and jagged. The largest
/// text that fits [`MAX_SHAPE_TEXT_BYTES`] with coordinates to the
/// millimetre then takes about 15 million steps.
///
/// What runs into the limit is a shape in which many segments lie close to
/// many others without touching: a thousand long narrow strips side by side
/// that all lean the same way take 29 million steps, because the box of
/// each strip covers its neighbours. Twice as many are refused. That is a
/// real limit, and the price of a bound: without it such a text costs time
/// with the square of its size. Fifty million steps take about a second and
/// a half in an unoptimised build.
pub const MAX_SHAPE_CHECK_STEPS: u64 = 50_000_000;

/// Maximum depth of parentheses in the text of a shape.
///
/// A shape is a point, a line or a surface, or several of one kind. The
/// deepest of those is a `MULTIPOLYGON(((x y, ...)))`: three levels. Deeper
/// nesting only occurs in a `GEOMETRYCOLLECTION`, which is no shape, and the
/// parser recurses on it without a bound of its own: twenty thousand nested
/// collections overflow the stack, which aborts the process and cannot be
/// caught. So the depth is checked before the parser sees the text.
pub const MAX_SHAPE_PAREN_DEPTH: usize = 3;

/// Maximum recursion depth for dot notation property access.
///
/// Prevents stack overflow on malicious input like "a.a.a.a.a...".
/// 32 levels is far beyond what any legitimate data structure would need.
pub const MAX_PROPERTY_DEPTH: usize = 32;

// Sanity bounds on the limits above, checked at compile time: a limit edited
// out of range fails the build rather than a test.
const _: () = {
    // Sanity checks that limits are within reasonable bounds
    assert!(MAX_LOADED_LAWS >= 10, "Should allow at least 10 laws");
    assert!(MAX_LOADED_LAWS <= 1000, "Should not allow excessive laws");

    assert!(MAX_YAML_SIZE >= 100_000, "Should allow at least 100KB");
    assert!(MAX_YAML_SIZE <= 10_000_000, "Should not allow 10MB+");

    assert!(MAX_ARRAY_SIZE >= 100, "Should allow reasonable arrays");
    assert!(MAX_ARRAY_SIZE <= 10_000, "Should not allow huge arrays");

    assert!(MAX_CROSS_LAW_DEPTH >= 5, "Should allow typical chains");
    assert!(MAX_CROSS_LAW_DEPTH <= 50, "Should limit deep chains");

    assert!(MAX_OPERATION_DEPTH >= 50, "Should allow complex ops");
    assert!(MAX_OPERATION_DEPTH <= 500, "Should limit extreme nesting");

    assert!(MAX_PROPERTY_DEPTH >= 10, "Should allow nested objects");
    assert!(MAX_PROPERTY_DEPTH <= 100, "Should limit extreme depth");

    assert!(
        MAX_FUZZY_QUOTE_CHARS >= 100,
        "Should allow a sentence-length quote"
    );
    assert!(
        MAX_FUZZY_QUOTE_CHARS <= 500,
        "Should keep the cubic scan bounded"
    );
    assert!(
        MAX_FUZZY_SCAN_CHARS >= 119_763,
        "Should cover the Zorgverzekeringswet in one scan"
    );
    assert!(MAX_FUZZY_SCAN_CHARS <= 1_000_000, "Should bound the scan");
    assert!(
        MAX_FUZZY_SCORED_WINDOWS >= 1_000,
        "Should score enough windows to find real fuzzy matches"
    );
    assert!(
        MAX_FUZZY_SCORED_WINDOWS <= 100_000,
        "Should bound the Levenshtein work"
    );
};
