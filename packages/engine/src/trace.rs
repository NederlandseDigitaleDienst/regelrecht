//! Execution tracing for audit trails and debugging
//!
//! This module provides structures and utilities for recording the execution
//! path through law evaluation. This is useful for:
//!
//! - **Audit trails**: Documenting exactly how a decision was reached
//! - **Debugging**: Understanding why a particular result was produced
//! - **Explainability**: Providing transparency in automated legal decisions
//!
//! # Example
//!
//! ```ignore
//! use regelrecht_engine::trace::{PathNode, PathNodeType, TraceBuilder};
//!
//! let mut builder = TraceBuilder::new();
//!
//! // Start resolving a variable
//! builder.push("inkomen", PathNodeType::Resolve);
//!
//! // Nested operation
//! builder.push("vergelijk_drempel", PathNodeType::Operation);
//! builder.set_result(Value::Bool(true));
//! builder.pop();
//!
//! // Complete the resolution
//! builder.set_result(Value::Int(50000));
//! builder.pop();
//!
//! let trace = builder.build();
//! ```

use crate::answer::AnswerProvenance;
use crate::types::{PathNodeType, ResolveType, TypeSpec, Value};
use regelrecht_law_model::{Article, ArticleBasedLaw, ProvisionReference};
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// The version of the trace document format this engine emits (RFC-039).
///
/// Bumped when a change would break a consumer; a new optional field does not.
/// It travels inside the document rather than beside it, because an archived
/// trace has to stay readable without whatever handed it over.
pub const TRACE_VERSION: u32 = 1;

/// An execution trace as a document: a version and the outermost step
/// (RFC-039, `schema/trace/v1/trace-schema.json`).
///
/// This is what an engine publishes and what a second engine has to produce.
/// Consumers read `root`; they used to be handed the step itself, and that is
/// the one breaking change in RFC-039.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceDocument {
    /// Format version. See [`TRACE_VERSION`].
    pub trace_version: u32,
    /// The outermost step, which is the evaluation the caller asked for.
    pub root: PathNode,
}

impl TraceDocument {
    /// Wrap a completed trace as a document at the current format version.
    pub fn new(root: PathNode) -> Self {
        Self {
            trace_version: TRACE_VERSION,
            root,
        }
    }
}

/// A node in the execution trace tree.
///
/// Each node represents a single step in the execution process, such as:
/// - Resolving a variable
/// - Executing an operation
/// - Evaluating an action
///
/// Nodes can have children, forming a tree structure that mirrors the
/// nested nature of law evaluation.
/// `PartialEq` compares a node structurally, which is what a test asserting
/// two traces are the same wants. Note that it does not survive a JSON round
/// trip for every value: `Value::Decimal` serializes through `f64` and a whole
/// decimal comes back as `Value::Int`, so compare a round trip as JSON text
/// rather than by node identity (RFC-039).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PathNode {
    /// Type of this execution step
    pub node_type: PathNodeType,

    /// Name or identifier for this step (e.g., variable name, operation type)
    pub name: String,

    /// The result value produced by this step, if any
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,

    /// For resolve nodes, indicates how the value was resolved
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolve_type: Option<ResolveType>,

    /// Child nodes representing nested execution steps
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<PathNode>,

    /// Execution duration in microseconds.
    ///
    /// Always absent under WebAssembly: the traced entry points there force
    /// [`TraceBuilder::new_untimed`], because `Instant::now()` trips RefCell
    /// aliasing in wasm-bindgen. No consumer may depend on it (RFC-039).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_us: Option<u64>,

    /// Free-form message for trace output (e.g., "Resolving from PARAMETERS: 999993653").
    ///
    /// **Presentational.** This string exists to be read by a person in a
    /// terminal, and its phrasing is not a contract. Parsing it is a defect
    /// (RFC-039); every fact it states is available as a field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Stable address of this node within the trace: the chain of child
    /// indices from the root, written `n0.2.1` (RFC-039).
    ///
    /// Deterministic, so the same inputs produce the same id on a later run,
    /// and unique by construction. This is what lets a consumer name a step
    /// without matching on `name`, which cannot tell apart two steps
    /// resolving the same input in different laws.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,

    /// Where the engine was when it took this step (RFC-039).
    ///
    /// Comes from the law model, so it is present whenever the engine knows
    /// the article, independent of what the corpus happens to cite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<LegalAnchor>,

    /// The citation the authored YAML element carries, as written (RFC-039).
    ///
    /// Kept apart from [`Self::anchor`] on purpose: a `legal_basis` hung on
    /// the wrong provision stays visible instead of being normalised into
    /// agreement with where the engine actually was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_basis: Option<LegalAnchor>,

    /// Dotted, name-keyed path to the element inside the article's
    /// `machine_readable` (RFC-039), e.g.
    /// `articles.2.machine_readable.execution.actions.hoogte_zorgtoeslag.value.values.1`.
    ///
    /// The same path language the corpus already speaks: `YamlNode` stamps it
    /// on every rendered node, and `expanded_paths` in `demo-config.yaml`
    /// keys on it. A dot inside a label would split one segment into two, so
    /// labels have their dots replaced by underscores (article `2.34` is
    /// addressed as `2_34`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yaml_path: Option<String>,

    /// The `regelrecht://{law_id}/{output}#{field}` address, on nodes that
    /// denote a value rather than a step (RFC-039).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,

    /// Where the value came from, structured (RFC-039). The field that
    /// replaces reading it out of [`Self::message`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ValueSource>,

    /// Unit and precision of [`Self::result`] (RFC-023), so a renderer can
    /// show an amount as euros instead of a bare count of cents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub type_spec: Option<ValueTypeSpec>,

    /// On the node of a named answer a binding took (`source.reduction`,
    /// RFC-048): who answered and from what. How the answer came about is
    /// not in this trace; the answerer explains it on request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<AnswerProvenance>,
}

/// Unit and precision of a value that has been computed (RFC-039).
///
/// Deliberately narrower than the law schema's `type_spec`, which also carries
/// `min` and `max`. Those constrain what a value *may* be; a trace reports what
/// a value *is*, and a bound the engine never enforced would read on a step as
/// though it had been checked.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ValueTypeSpec {
    /// Unit of measurement, from the same closed set the law schema allows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Number of decimal places for the value, in its own unit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub precision: Option<i64>,
}

impl ValueTypeSpec {
    /// Keep only what describes the value, dropping the declared bounds.
    pub fn from_declaration(spec: &TypeSpec) -> Self {
        Self {
            unit: spec.unit.clone(),
            precision: spec.precision,
        }
    }

    /// Whether this says nothing, in which case it is left off the node rather
    /// than serialized as an empty object.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A reference to a provision, in the shape of the schema's `legal_basis`
/// reference object (RFC-039).
///
/// One type for both [`PathNode::anchor`] and [`PathNode::legal_basis`], so
/// the engine's own account of where it was and the corpus's citation are
/// comparable rather than two dialects of the same idea.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct LegalAnchor {
    /// Law identifier as the corpus knows it (`wet_op_de_zorgtoeslag`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub law_id: Option<String>,

    /// Law name as cited in prose ("Wet op de zorgtoeslag").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub law: Option<String>,

    /// Stable uuid of the law version, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub law_uuid: Option<String>,

    /// BWB identification number (`BWBR0018451`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bwb_id: Option<String>,

    /// Article number as the law writes it (`2`, `1a`, `B1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub article: Option<String>,

    /// Lid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paragraph: Option<String>,

    /// Sentence, for a reference finer than a lid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sentence: Option<String>,

    /// Link to the provision on wetten.overheid.nl.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Juriconnect BWB 1.3 reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub juriconnect: Option<String>,

    /// `valid_from` of the law version this anchor points into.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,

    /// The modeller's note on how the element relates to the text. Present
    /// only on a `legal_basis`, never on an `anchor`: the engine has no
    /// opinion to record here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}

impl LegalAnchor {
    /// Whether this anchor says nothing at all, in which case it is left off
    /// the node rather than serialized as an empty object.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Where the engine is: the provision it is evaluating (RFC-039).
    ///
    /// Reports only what the law header and the article state. `paragraph` and
    /// `sentence` stay absent, because an article is the finest the engine
    /// genuinely knows; guessing a lid from an operation's position would be a
    /// citation the engine cannot support. The article's `url` is the link to
    /// the official text, which is what makes a step clickable through to
    /// wetten.overheid.nl.
    ///
    /// `law` is copied from the header as written, which for some laws is a
    /// `#reference` to an output the law computes about itself rather than a
    /// title. Resolving it would mean evaluating a law output while recording a
    /// step about one, so it is passed through unresolved: `law_id` is always a
    /// usable identifier, and a display that wants a title should prefer the
    /// `law` on a `legal_basis`, which the corpus writes out in full.
    pub fn from_article(law: &ArticleBasedLaw, article: &Article) -> Self {
        Self {
            law_id: Some(law.id.clone()),
            law: law.name.clone(),
            law_uuid: law.uuid.clone(),
            bwb_id: law.bwb_id.clone(),
            article: Some(article.number.clone()),
            url: article.url.clone(),
            valid_from: law.valid_from.clone(),
            ..Self::default()
        }
    }

    /// What the corpus cites, as written (RFC-039).
    ///
    /// Carries the citation across unchanged, down to the lid and the
    /// modeller's explanation. `law_id`, `law_uuid` and `valid_from` stay
    /// absent: a citation names a provision, not the corpus file the engine
    /// happened to load it from, and a reference may well point at a law that
    /// is not in the corpus at all.
    pub fn from_provision(reference: &ProvisionReference) -> Self {
        Self {
            law: reference.law.clone(),
            bwb_id: reference.bwb_id.clone(),
            article: reference.article.clone(),
            paragraph: reference.paragraph.clone(),
            sentence: reference.sentence.clone(),
            url: reference.url.clone(),
            juriconnect: reference.juriconnect.clone(),
            explanation: reference.explanation.clone(),
            ..Self::default()
        }
    }
}

/// Where a resolved value came from (RFC-039).
///
/// The structured form of what the demo's lineage view used to extract from
/// [`PathNode::message`] with a regular expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueSource {
    /// How the value was resolved. Same vocabulary as
    /// [`PathNode::resolve_type`], repeated here so a consumer reading
    /// `source` does not have to correlate two fields.
    pub kind: ResolveType,

    /// The registered data source that held the value, which for the demo is
    /// the organisation that keeps the register. Absent for a kind that has
    /// no provider, such as a definition in the law itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,

    /// The law a scoped data source was registered for, when the source is
    /// scoped rather than global.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

impl PathNode {
    /// Create a new PathNode with the given type and name.
    pub fn new(node_type: PathNodeType, name: impl Into<String>) -> Self {
        Self {
            node_type,
            name: name.into(),
            result: None,
            resolve_type: None,
            children: Vec::new(),
            duration_us: None,
            message: None,
            node_id: None,
            anchor: None,
            legal_basis: None,
            yaml_path: None,
            uri: None,
            source: None,
            type_spec: None,
            answer: None,
        }
    }

    /// Set the result value for this node.
    pub fn with_result(mut self, result: Value) -> Self {
        self.result = Some(result);
        self
    }

    /// Set the resolve type for this node.
    pub fn with_resolve_type(mut self, resolve_type: ResolveType) -> Self {
        self.resolve_type = Some(resolve_type);
        self
    }

    /// Add a child node.
    pub fn with_child(mut self, child: PathNode) -> Self {
        self.children.push(child);
        self
    }

    /// Set the duration in microseconds.
    pub fn with_duration(mut self, duration_us: u64) -> Self {
        self.duration_us = Some(duration_us);
        self
    }

    /// Set a free-form message for trace output.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Set the stable address of this node within the trace (RFC-039).
    pub fn with_node_id(mut self, node_id: impl Into<String>) -> Self {
        self.node_id = Some(node_id.into());
        self
    }

    /// Set where the engine was when it took this step (RFC-039).
    pub fn with_anchor(mut self, anchor: LegalAnchor) -> Self {
        self.anchor = Some(anchor);
        self
    }

    /// Set the citation the authored element carries (RFC-039).
    pub fn with_legal_basis(mut self, legal_basis: LegalAnchor) -> Self {
        self.legal_basis = Some(legal_basis);
        self
    }

    /// Set the dotted path to the element inside `machine_readable` (RFC-039).
    pub fn with_yaml_path(mut self, yaml_path: impl Into<String>) -> Self {
        self.yaml_path = Some(yaml_path.into());
        self
    }

    /// Set the `regelrecht://` address of the value this node denotes (RFC-039).
    pub fn with_uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    /// Set where the resolved value came from (RFC-039).
    pub fn with_source(mut self, source: ValueSource) -> Self {
        self.source = Some(source);
        self
    }

    /// Set the unit and precision of the result (RFC-023), keeping the part of
    /// the law's declaration that describes a computed value.
    pub fn with_type_spec(mut self, type_spec: TypeSpec) -> Self {
        self.type_spec = Some(ValueTypeSpec::from_declaration(&type_spec));
        self
    }

    /// Render the trace as a human-readable tree string.
    ///
    /// Produces output like:
    /// ```text
    /// calculate (action)
    /// +-- inkomen (resolve) [parameter] = 50000
    /// +-- drempel (resolve) [definition] = 30000
    /// `-- vergelijk (operation) = true
    ///     +-- $inkomen (resolve) = 50000
    ///     `-- $drempel (resolve) = 30000
    /// ```
    ///
    /// # Arguments
    /// * `indent` - Current indentation level (start with 0)
    /// * `is_last` - Whether this is the last child in its parent (affects line prefix)
    pub fn render(&self, indent: usize, is_last: bool) -> String {
        self.render_internal(indent, is_last, true)
    }

    /// Internal render implementation.
    fn render_internal(&self, indent: usize, is_last: bool, is_top_level: bool) -> String {
        let mut lines = Vec::new();

        // Build the prefix for this node
        let prefix = if is_top_level {
            String::new()
        } else if is_last {
            "`-- ".to_string()
        } else {
            "+-- ".to_string()
        };

        // Build indentation for child nodes
        let child_indent = if is_top_level {
            String::new()
        } else if is_last {
            "    ".to_string()
        } else {
            "|   ".to_string()
        };

        // Format the node type
        let type_str = match self.node_type {
            PathNodeType::Resolve => "resolve",
            PathNodeType::Operation => "operation",
            PathNodeType::Action => "action",
            PathNodeType::Requirement => "requirement",
            PathNodeType::CrossLawReference => "cross_law_reference",
            PathNodeType::Article => "article",
            PathNodeType::Cached => "cached",
            PathNodeType::OpenTermResolution => "open_term",
            PathNodeType::HookResolution => "hook",
            PathNodeType::OverrideResolution => "override",
        };

        // Build the main line
        let mut line = format!("{}{} ({})", prefix, self.name, type_str);

        // Add resolve type if present
        if let Some(ref rt) = self.resolve_type {
            let rt_str = match rt {
                ResolveType::Uri => "uri",
                ResolveType::Parameter => "parameter",
                ResolveType::Definition => "definition",
                ResolveType::Output => "output",
                ResolveType::Input => "input",
                ResolveType::Local => "local",
                ResolveType::Context => "context",
                ResolveType::ResolvedInput => "resolved_input",
                ResolveType::DataSource => "data_source",
                ResolveType::OpenTerm => "open_term",
                ResolveType::OpenTermSilent => "open_term_silent",
                ResolveType::Hook => "hook",
                ResolveType::Override => "override",
            };
            line.push_str(&format!(" [{}]", rt_str));
        }

        // Add result if present
        if let Some(ref result) = self.result {
            line.push_str(&format!(" = {}", format_value_compact(result)));
        }

        // Add duration if present (and significant)
        if let Some(duration) = self.duration_us {
            if duration >= 100 {
                // Only show if >= 0.1ms
                line.push_str(&format!(" ({}μs)", duration));
            }
        }

        lines.push(line);

        // Render children
        let child_count = self.children.len();
        for (i, child) in self.children.iter().enumerate() {
            let is_last_child = i == child_count - 1;
            let child_str = child.render_internal(0, is_last_child, false);

            // Add proper indentation to each line of the child's output. The
            // child already prefixed its own descendants, so every line of its
            // output gets the same indentation here.
            for child_line in child_str.lines() {
                lines.push(format!(
                    "{}{}",
                    " ".repeat(indent * 4) + &child_indent,
                    child_line
                ));
            }
        }

        lines.join("\n")
    }
}

/// One indentation column in the box-drawing trace (4 chars wide).
#[derive(Debug, Clone, Copy)]
enum ScopeColumn {
    Double, // ║
    Single, // │
    Blank,  //
}

impl ScopeColumn {
    fn as_str(self) -> &'static str {
        match self {
            ScopeColumn::Double => "║   ",
            ScopeColumn::Single => "│   ",
            ScopeColumn::Blank => "    ",
        }
    }
}

impl PathNode {
    /// Render the trace using box-drawing characters for human-readable output.
    pub fn render_box_drawing(&self) -> String {
        let mut lines = Vec::new();
        let mut cols = Vec::new();
        self.render_node(&mut lines, &mut cols, true, false);
        lines.join("\n")
    }

    fn prefix(cols: &[ScopeColumn]) -> String {
        cols.iter().map(|c| c.as_str()).collect()
    }

    fn continuation_col(is_last: bool, is_double: bool) -> ScopeColumn {
        if is_last {
            ScopeColumn::Blank
        } else if is_double {
            ScopeColumn::Double
        } else {
            ScopeColumn::Single
        }
    }

    /// Render children in a double (cross-law) scope.
    /// Children get `is_double=true` so they use ╟──/╙── connectors and ║ continuation.
    /// No column manipulation — each child pushes its own continuation column.
    fn render_double_children(&self, lines: &mut Vec<String>, cols: &mut Vec<ScopeColumn>) {
        let child_count = self.children.len();
        for (i, child) in self.children.iter().enumerate() {
            child.render_node(lines, cols, i == child_count - 1, true);
        }
    }

    fn render_single_children(
        &self,
        lines: &mut Vec<String>,
        cols: &mut Vec<ScopeColumn>,
        extra_last: bool,
    ) {
        let child_count = self.children.len();
        for (i, child) in self.children.iter().enumerate() {
            let is_last = i == child_count - 1 && !extra_last;
            child.render_node(lines, cols, is_last, false);
        }
    }

    fn render_node(
        &self,
        lines: &mut Vec<String>,
        cols: &mut Vec<ScopeColumn>,
        is_last: bool,
        is_double: bool,
    ) {
        let pfx = Self::prefix(cols);
        let connector = match (is_double, is_last) {
            (true, true) => "╙──",
            (true, false) => "╟──",
            (false, true) => "└──",
            (false, false) => "├──",
        };
        match self.node_type {
            PathNodeType::Article => {
                if let Some(ref msg) = self.message {
                    lines.push(msg.clone());
                } else {
                    lines.push(self.name.clone());
                }
                lines.push(format!("{}╟──Evaluating rules for {}", pfx, self.name));
                cols.push(ScopeColumn::Double);
                self.render_double_children(lines, cols);
                cols.pop();
                if let Some(ref result) = self.result {
                    let pfx = Self::prefix(cols);
                    let output_name = self
                        .name
                        .split_once(' ')
                        .map(|(_, rest)| rest.trim_matches(|c| c == '(' || c == ')'))
                        .unwrap_or(&self.name);
                    lines.push(format!(
                        "{}╙──Result: {}",
                        pfx,
                        format_outputs(output_name, result)
                    ));
                }
            }
            PathNodeType::Requirement => {
                if let Some(ref msg) = self.message {
                    lines.push(format!("{}{}{}", pfx, connector, msg));
                } else {
                    lines.push(format!("{}{}Requirements", pfx, connector));
                }
                let has_result = self.result.is_some();
                cols.push(Self::continuation_col(is_last, is_double));
                self.render_single_children(lines, cols, has_result);
                if let Some(ref result) = self.result {
                    let pfx = Self::prefix(cols);
                    // A requirement that could not be decided is neither met
                    // nor not met: an unknown names the facts it lacks and an
                    // untranslatable names the construct (RFC-036, RFC-012).
                    let verdict = match result {
                        Value::Unknown(missing) => {
                            format!("Requirement unknown (missing: {})", missing_names(missing))
                        }
                        Value::Untranslatable { article, .. } => {
                            format!("Requirement untranslatable (art. {})", article)
                        }
                        _ if result.to_bool() => "Requirement met".to_string(),
                        _ => "Requirement NOT met".to_string(),
                    };
                    lines.push(format!("{}└──{}", pfx, verdict));
                }
                cols.pop();
            }
            PathNodeType::Resolve => {
                let child_count = self.children.len();
                if child_count == 0 {
                    if let Some(ref rt) = self.resolve_type {
                        let rt_name = resolve_type_name(rt);
                        let val_str = self
                            .result
                            .as_ref()
                            .map(format_value_display)
                            .unwrap_or_else(|| "?".to_string());
                        lines.push(format!(
                            "{}{}Resolving from {}: ${} = {}",
                            pfx,
                            connector,
                            rt_name,
                            self.name.to_uppercase(),
                            val_str
                        ));
                    } else if let Some(ref msg) = self.message {
                        lines.push(format!("{}{}{}", pfx, connector, msg));
                    } else {
                        lines.push(format!(
                            "{}{}Resolving ${}",
                            pfx,
                            connector,
                            self.name.to_uppercase()
                        ));
                    }
                } else {
                    lines.push(format!(
                        "{}{}Resolving ${}",
                        pfx,
                        connector,
                        self.name.to_uppercase()
                    ));
                    cols.push(Self::continuation_col(is_last, is_double));
                    let child_pfx = Self::prefix(cols);
                    if let Some(ref rt) = self.resolve_type {
                        let rt_name = resolve_type_name(rt);
                        let val_str = self
                            .result
                            .as_ref()
                            .map(format_value_display)
                            .unwrap_or_else(|| "?".to_string());
                        lines.push(format!(
                            "{}├──Resolving from {}: {}",
                            child_pfx, rt_name, val_str
                        ));
                    } else if let Some(ref msg) = self.message {
                        lines.push(format!("{}├──{}", child_pfx, msg));
                    }
                    self.render_single_children(lines, cols, false);
                    cols.pop();
                }
            }
            PathNodeType::Operation => {
                if let Some(ref msg) = self.message {
                    lines.push(format!("{}{}{}", pfx, connector, msg));
                } else {
                    let result_str = self
                        .result
                        .as_ref()
                        .map(format_value_display)
                        .unwrap_or_else(|| "?".to_string());
                    lines.push(format!(
                        "{}{}Compute {} = {}",
                        pfx, connector, self.name, result_str
                    ));
                }
                cols.push(Self::continuation_col(is_last, is_double));
                self.render_single_children(lines, cols, false);
                cols.pop();
            }
            PathNodeType::Action => {
                if let Some(ref msg) = self.message {
                    lines.push(format!("{}{}{}", pfx, connector, msg));
                } else {
                    lines.push(format!("{}{}Computing {}", pfx, connector, self.name));
                }
                let has_result = self.result.is_some();
                cols.push(Self::continuation_col(is_last, is_double));
                self.render_single_children(lines, cols, has_result);
                if let Some(ref result) = self.result {
                    let pfx = Self::prefix(cols);
                    lines.push(format!(
                        "{}└──Result: {} = {}",
                        pfx,
                        self.name,
                        format_value_display(result)
                    ));
                }
                cols.pop();
            }
            PathNodeType::CrossLawReference => {
                if let Some(ref msg) = self.message {
                    lines.push(format!("{}{}{}", pfx, connector, msg));
                } else {
                    lines.push(format!("{}{}Reference: {}", pfx, connector, self.name));
                }
                cols.push(Self::continuation_col(is_last, is_double));
                self.render_double_children(lines, cols);
                cols.pop();
            }
            PathNodeType::Cached => {
                let result_str = self
                    .result
                    .as_ref()
                    .map(|v| format!(": {}", format_value_display(v)))
                    .unwrap_or_default();
                lines.push(format!(
                    "{}{}Cached: {}{}",
                    pfx, connector, self.name, result_str
                ));
            }
            PathNodeType::OpenTermResolution => {
                let result_str = self
                    .result
                    .as_ref()
                    .map(|v| format!(": {}", format_value_display(v)))
                    .unwrap_or_default();
                let msg = self.message.as_deref().unwrap_or(&self.name);
                lines.push(format!(
                    "{}{}Delegation: {}{}",
                    pfx, connector, msg, result_str
                ));
                cols.push(Self::continuation_col(is_last, is_double));
                self.render_double_children(lines, cols);
                cols.pop();
            }
            PathNodeType::HookResolution => {
                let result_str = self
                    .result
                    .as_ref()
                    .map(|v| format!(": {}", format_value_display(v)))
                    .unwrap_or_default();
                let msg = self.message.as_deref().unwrap_or(&self.name);
                lines.push(format!("{}{}HOOK: {}{}", pfx, connector, msg, result_str));
                cols.push(Self::continuation_col(is_last, is_double));
                self.render_double_children(lines, cols);
                cols.pop();
            }
            PathNodeType::OverrideResolution => {
                let result_str = self
                    .result
                    .as_ref()
                    .map(|v| format!(": {}", format_value_display(v)))
                    .unwrap_or_default();
                let msg = self.message.as_deref().unwrap_or(&self.name);
                lines.push(format!(
                    "{}{}Lex specialis: {}{}",
                    pfx, connector, msg, result_str
                ));
                cols.push(Self::continuation_col(is_last, is_double));
                self.render_single_children(lines, cols, false);
                cols.pop();
            }
        }
    }

    /// Render the trace as a compact single-line summary.
    pub fn render_compact(&self) -> String {
        let type_str = match self.node_type {
            PathNodeType::Resolve => "res",
            PathNodeType::Operation => "op",
            PathNodeType::Action => "act",
            PathNodeType::Requirement => "req",
            PathNodeType::CrossLawReference => "xlaw",
            PathNodeType::Article => "art",
            PathNodeType::Cached => "cache",
            PathNodeType::OpenTermResolution => "ot",
            PathNodeType::HookResolution => "hook",
            PathNodeType::OverrideResolution => "ovr",
        };

        let result_str = self
            .result
            .as_ref()
            .map(|v| format!("={}", format_value_compact(v)))
            .unwrap_or_default();

        format!("{}:{}{}", type_str, self.name, result_str)
    }
}

/// Format a Value compactly for trace output.
fn format_value_compact(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Decimal(d) => {
            // Limit decimal places
            if d.fract().is_zero() {
                format!("{:.1}", d)
            } else {
                format!("{:.2}", d)
            }
        }
        Value::String(s) => {
            // Truncate long strings (use chars() for UTF-8 safety)
            if s.chars().count() > 20 {
                let truncated: String = s.chars().take(17).collect();
                format!("\"{}...\"", truncated)
            } else {
                format!("\"{}\"", s)
            }
        }
        Value::Array(arr) => {
            if arr.len() <= 3 {
                let items: Vec<String> = arr.iter().map(format_value_compact).collect();
                format!("[{}]", items.join(", "))
            } else {
                format!("[{} items]", arr.len())
            }
        }
        Value::Object(obj) => {
            format!("{{...{} keys}}", obj.len())
        }
        Value::Untranslatable { article, .. } => {
            format!("UNTRANSLATABLE(art. {})", article)
        }
        Value::Unknown(missing) => format!("UNKNOWN({})", missing_names(missing)),
    }
}

/// The names of the facts an Unknown misses, comma-separated (RFC-036).
fn missing_names(missing: &[crate::types::MissingFact]) -> String {
    missing
        .iter()
        .map(|m| m.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// `name = value` for the outputs an article evaluation was asked for. Several
/// outputs come back as one object; each gets its own `name = value`, in the
/// order they were asked, rather than the object as a dict.
fn format_outputs(output_names: &str, result: &Value) -> String {
    let names: Vec<&str> = output_names.split(", ").collect();
    if let (true, Value::Object(obj)) = (names.len() > 1, result) {
        if names.iter().all(|name| obj.contains_key(*name)) {
            return names
                .iter()
                .map(|name| format!("{} = {}", name, format_value_display(&obj[*name])))
                .collect::<Vec<_>>()
                .join(", ");
        }
    }
    format!("{} = {}", output_names, format_value_display(result))
}

/// Format a Value for box-drawing trace output.
///
/// Uses display formatting: True/False for bools, quoted strings, etc. A date
/// the engine carries as an object (`{iso, year, month, day}`, the
/// `$referencedate`) shows as the date it is.
fn format_value_display(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(b) => {
            if *b {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Value::Int(i) => i.to_string(),
        Value::Decimal(d) => {
            if d.fract().is_zero() {
                format!("{:.1}", d)
            } else {
                format!("{}", d)
            }
        }
        Value::String(s) => format!("'{}'", s),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_value_display).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(obj) if matches!(obj.get("iso"), Some(Value::String(_))) => {
            format_value_display(&obj["iso"])
        }
        Value::Object(obj) => {
            let mut keys: Vec<&String> = obj.keys().collect();
            keys.sort();
            let items: Vec<String> = keys
                .iter()
                .map(|k| format!("'{}': {}", k, format_value_display(&obj[k.as_str()])))
                .collect();
            format!("{{{}}}", items.join(", "))
        }
        Value::Untranslatable { article, construct } => {
            format!("UNTRANSLATABLE(art. {}: {})", article, construct)
        }
        Value::Unknown(missing) => format!("UNKNOWN({})", missing_names(missing)),
    }
}

/// Get a human-readable name for a ResolveType.
fn resolve_type_name(rt: &ResolveType) -> &'static str {
    match rt {
        ResolveType::Uri => "URI",
        ResolveType::Parameter => "PARAMETERS",
        ResolveType::Definition => "DEFINITION",
        ResolveType::Output => "OUTPUT",
        ResolveType::Input => "INPUT",
        ResolveType::Local => "LOCAL",
        ResolveType::Context => "CONTEXT",
        ResolveType::ResolvedInput => "RESOLVED_INPUT",
        ResolveType::DataSource => "DATA_SOURCE",
        ResolveType::OpenTerm => "OPEN_TERM",
        ResolveType::OpenTermSilent => "OPEN_TERM_SILENT",
        ResolveType::Hook => "HOOK",
        ResolveType::Override => "OVERRIDE",
    }
}

/// A node being built, with timing information.
#[derive(Debug)]
struct BuildingNode {
    node: PathNode,
    start_time: Option<Instant>,
    /// Chain of child indices from the root down to this node, which
    /// [`format_node_id`] renders as the node's `node_id` (RFC-039).
    path: Vec<usize>,
}

/// Render a node's index chain as its `node_id`: `n`, `n0`, `n0.2.1`.
///
/// The root is `n` (an empty chain). Depth and sibling order are the whole
/// address, which is why it is stable across runs of the same inputs.
fn format_node_id(path: &[usize]) -> String {
    let mut id = String::with_capacity(1 + path.len() * 2);
    id.push('n');
    for (i, index) in path.iter().enumerate() {
        if i > 0 {
            id.push('.');
        }
        id.push_str(&index.to_string());
    }
    id
}

/// Builder for constructing execution traces using a stack-based approach.
///
/// The builder maintains a stack of nodes being constructed. As execution
/// proceeds, nodes are pushed when entering a new scope and popped when
/// leaving. This naturally produces the tree structure of nested execution.
#[derive(Debug)]
pub struct TraceBuilder {
    /// Stack of nodes being built (last is current)
    stack: Vec<BuildingNode>,

    /// Whether tracing is enabled
    enabled: bool,

    /// Whether to record timing (Instant::now on push/pop)
    timed: bool,
}

impl Default for TraceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceBuilder {
    /// Create a new TraceBuilder with tracing enabled and timing.
    ///
    /// Under WebAssembly it times nothing, as [`TraceBuilder::new_untimed`]:
    /// `Instant::now()` panics on `wasm32-unknown-unknown`, which has no
    /// clock, so a traced evaluation there would abort. `duration_us` is
    /// absent under WebAssembly either way (RFC-039).
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            enabled: true,
            timed: !cfg!(target_arch = "wasm32"),
        }
    }

    /// Create a new TraceBuilder with tracing enabled but no timing.
    ///
    /// Skips `Instant::now()` syscalls on push/pop — useful when only
    /// the trace tree structure matters, not microsecond durations.
    pub fn new_untimed() -> Self {
        Self {
            stack: Vec::new(),
            enabled: true,
            timed: false,
        }
    }

    /// Create a new TraceBuilder with tracing disabled (no-op).
    pub fn disabled() -> Self {
        Self {
            stack: Vec::new(),
            enabled: false,
            timed: false,
        }
    }

    /// Check if tracing is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Push a new node onto the stack.
    ///
    /// Call this when entering a new execution scope (resolving a variable,
    /// starting an operation, etc.).
    pub fn push(&mut self, name: impl Into<String>, node_type: PathNodeType) {
        if !self.enabled {
            return;
        }

        // A node's index among its siblings is the number of siblings already
        // popped, so it is final the moment the node is pushed and there is no
        // need to wait for pop to address it (RFC-039).
        let path = match self.stack.last() {
            Some(parent) => {
                let mut path = parent.path.clone();
                path.push(parent.node.children.len());
                path
            }
            None => Vec::new(),
        };

        let node = PathNode::new(node_type, name).with_node_id(format_node_id(&path));
        self.stack.push(BuildingNode {
            node,
            start_time: if self.timed {
                Some(Instant::now())
            } else {
                None
            },
            path,
        });
    }

    /// Set the result value for the current node.
    pub fn set_result(&mut self, result: Value) {
        if !self.enabled {
            return;
        }

        if let Some(current) = self.stack.last_mut() {
            current.node.result = Some(result);
        }
    }

    /// Set a free-form message on the current node.
    pub fn set_message(&mut self, msg: impl Into<String>) {
        if !self.enabled {
            return;
        }

        if let Some(current) = self.stack.last_mut() {
            current.node.message = Some(msg.into());
        }
    }

    /// Get the message on the current node, if set.
    pub fn get_message(&self) -> Option<&str> {
        if !self.enabled {
            return None;
        }
        self.stack.last().and_then(|s| s.node.message.as_deref())
    }

    /// Set the resolve type for the current node.
    pub fn set_resolve_type(&mut self, resolve_type: ResolveType) {
        if !self.enabled {
            return;
        }

        if let Some(current) = self.stack.last_mut() {
            current.node.resolve_type = Some(resolve_type);
        }
    }

    /// Set where the engine was when it took the current step (RFC-039).
    ///
    /// An anchor that says nothing is dropped rather than recorded as an
    /// empty object.
    pub fn set_anchor(&mut self, anchor: LegalAnchor) {
        if !self.enabled || anchor.is_empty() {
            return;
        }

        if let Some(current) = self.stack.last_mut() {
            current.node.anchor = Some(anchor);
        }
    }

    /// Set the citation the authored element carries on the current node
    /// (RFC-039).
    pub fn set_legal_basis(&mut self, legal_basis: LegalAnchor) {
        if !self.enabled || legal_basis.is_empty() {
            return;
        }

        if let Some(current) = self.stack.last_mut() {
            current.node.legal_basis = Some(legal_basis);
        }
    }

    /// Set the dotted path into `machine_readable` for the current node
    /// (RFC-039).
    pub fn set_yaml_path(&mut self, yaml_path: impl Into<String>) {
        if !self.enabled {
            return;
        }

        if let Some(current) = self.stack.last_mut() {
            current.node.yaml_path = Some(yaml_path.into());
        }
    }

    /// Set the `regelrecht://` address of the value the current node denotes
    /// (RFC-039).
    pub fn set_uri(&mut self, uri: impl Into<String>) {
        if !self.enabled {
            return;
        }

        if let Some(current) = self.stack.last_mut() {
            current.node.uri = Some(uri.into());
        }
    }

    /// Set who answered the open term of the current node, and from what
    /// (RFC-048).
    pub fn set_answer(&mut self, answer: AnswerProvenance) {
        if !self.enabled {
            return;
        }

        if let Some(current) = self.stack.last_mut() {
            current.node.answer = Some(answer);
        }
    }

    /// Set where the current node's value came from (RFC-039).
    pub fn set_source(&mut self, source: ValueSource) {
        if !self.enabled {
            return;
        }

        if let Some(current) = self.stack.last_mut() {
            current.node.source = Some(source);
        }
    }

    /// Set the unit and precision of the current node's result (RFC-023).
    ///
    /// Takes the law's declaration and keeps the part that describes a computed
    /// value. A declaration that says nothing about unit or precision is left
    /// off entirely, so an absent `type_spec` means "nothing was declared"
    /// rather than "an empty object was declared".
    pub fn set_type_spec(&mut self, type_spec: TypeSpec) {
        if !self.enabled {
            return;
        }

        let spec = ValueTypeSpec::from_declaration(&type_spec);
        if spec.is_empty() {
            return;
        }

        if let Some(current) = self.stack.last_mut() {
            current.node.type_spec = Some(spec);
        }
    }

    /// The `node_id` of the current node, for a caller that needs to refer to
    /// a step it is in the middle of recording.
    pub fn current_node_id(&self) -> Option<&str> {
        if !self.enabled {
            return None;
        }
        self.stack.last().and_then(|s| s.node.node_id.as_deref())
    }

    /// Pop the current node from the stack, making it a child of the parent.
    ///
    /// Returns the popped node. If this was the last node on the stack,
    /// returns the completed root node.
    pub fn pop(&mut self) -> Option<PathNode> {
        if !self.enabled {
            return None;
        }

        let building = self.stack.pop()?;

        let mut completed = building.node;
        completed.duration_us = building.start_time.map(|t| t.elapsed().as_micros() as u64);

        // If there's a parent, add this as a child (move, not clone)
        if let Some(parent) = self.stack.last_mut() {
            parent.node.children.push(completed);
            return None;
        }

        Some(completed)
    }

    /// Build the final trace, consuming the builder.
    ///
    /// Pops all remaining nodes from the stack, returning the root node.
    /// Returns None if the stack is empty or tracing was disabled.
    pub fn build(mut self) -> Option<PathNode> {
        if !self.enabled {
            return None;
        }

        // Pop all nodes to build complete tree
        let mut result = None;
        while !self.stack.is_empty() {
            result = self.pop();
        }
        result
    }

    /// Get the current depth of the trace stack.
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Check if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MissingFact, MissingKind};

    fn missing(name: &str) -> MissingFact {
        MissingFact {
            law: "wet_x".to_string(),
            name: name.to_string(),
            kind: MissingKind::NoData,
        }
    }

    #[test]
    fn an_unknown_names_its_missing_facts_in_both_trace_forms() {
        // RFC-036: the trace says which facts are missing, in order, and
        // nothing else; a reader must be able to tell "unknown for lack of
        // huur" from "unknown for lack of partner_bsn".
        let one = Value::Unknown(vec![missing("huur")]);
        let two = Value::Unknown(vec![missing("huur"), missing("partner_bsn")]);
        assert_eq!(format_value_compact(&one), "UNKNOWN(huur)");
        assert_eq!(format_value_compact(&two), "UNKNOWN(huur, partner_bsn)");
        assert_eq!(format_value_display(&two), "UNKNOWN(huur, partner_bsn)");
        assert_eq!(missing_names(&[]), "");
    }

    #[test]
    fn test_path_node_creation() {
        let node = PathNode::new(PathNodeType::Resolve, "test_var");
        assert_eq!(node.name, "test_var");
        assert!(matches!(node.node_type, PathNodeType::Resolve));
        assert!(node.result.is_none());
        assert!(node.children.is_empty());
    }

    #[test]
    fn test_path_node_builder_pattern() {
        let node = PathNode::new(PathNodeType::Operation, "ADD")
            .with_result(Value::Int(42))
            .with_child(PathNode::new(PathNodeType::Resolve, "a"))
            .with_child(PathNode::new(PathNodeType::Resolve, "b"));

        assert_eq!(node.result, Some(Value::Int(42)));
        assert_eq!(node.children.len(), 2);
    }

    #[test]
    fn test_trace_builder_simple() {
        let mut builder = TraceBuilder::new();
        assert!(builder.is_enabled());
        assert!(builder.is_empty());

        builder.push("root", PathNodeType::Resolve);
        assert_eq!(builder.depth(), 1);

        builder.set_result(Value::Int(100));
        let node = builder.pop().unwrap();

        assert_eq!(node.name, "root");
        assert_eq!(node.result, Some(Value::Int(100)));
        assert!(node.duration_us.is_some());
    }

    #[test]
    fn test_trace_builder_nested() {
        let mut builder = TraceBuilder::new();

        // Root node
        builder.push("calculate", PathNodeType::Action);

        // Nested operation
        builder.push("add_values", PathNodeType::Operation);
        builder.set_result(Value::Int(30));
        builder.pop();

        // Complete root
        builder.set_result(Value::Int(30));
        let root = builder.build().unwrap();

        assert_eq!(root.name, "calculate");
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].name, "add_values");
        assert_eq!(root.children[0].result, Some(Value::Int(30)));
    }

    /// A trace the shape of a small cross-law evaluation, for the addressing
    /// tests below. Built twice by the id-stability test, so it has to be a
    /// function and not a constant.
    fn addressed_trace() -> PathNode {
        let mut builder = TraceBuilder::new_untimed();
        builder.push(
            "wet_op_de_zorgtoeslag (hoogte_zorgtoeslag)",
            PathNodeType::Article,
        );

        builder.push("bsn", PathNodeType::Resolve);
        builder.set_resolve_type(ResolveType::Parameter);
        builder.set_result(Value::String("999993653".to_string()));
        builder.pop();

        builder.push(
            "zorgverzekeringswet#is_verzekerd",
            PathNodeType::CrossLawReference,
        );
        builder.push("polis_status", PathNodeType::Resolve);
        builder.set_resolve_type(ResolveType::DataSource);
        builder.set_source(ValueSource {
            kind: ResolveType::DataSource,
            provider: Some("Zorgverzekeraar".to_string()),
            scope: Some("zorgverzekeringswet".to_string()),
        });
        builder.set_result(Value::String("ACTIEF".to_string()));
        builder.pop();
        builder.set_result(Value::Bool(true));
        builder.pop();

        builder.set_result(Value::Int(209692));
        builder.build().unwrap()
    }

    /// Every `node_id` in a trace is different. The ids are the index chain,
    /// so this holds by construction; the test is here because a consumer
    /// addressing a step by id has no way to notice a collision.
    #[test]
    fn node_ids_are_unique_within_a_trace() {
        fn collect(node: &PathNode, out: &mut Vec<String>) {
            out.push(
                node.node_id
                    .clone()
                    .expect("every built node carries a node_id"),
            );
            for child in &node.children {
                collect(child, out);
            }
        }

        let mut ids = Vec::new();
        collect(&addressed_trace(), &mut ids);

        assert_eq!(ids.len(), 4, "root, bsn, the reference, and polis_status");
        let unique: std::collections::BTreeSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "duplicate node_id in {ids:?}");
    }

    /// The root is `n`, a first child `n0`, and depth and sibling order are
    /// the whole address. A consumer builds a URL fragment out of this, so the
    /// spelling is part of the contract (RFC-039).
    #[test]
    fn node_ids_spell_the_index_chain_from_the_root() {
        let root = addressed_trace();

        assert_eq!(root.node_id.as_deref(), Some("n"));
        assert_eq!(root.children[0].node_id.as_deref(), Some("n0"));
        assert_eq!(root.children[1].node_id.as_deref(), Some("n1"));
        assert_eq!(
            root.children[1].children[0].node_id.as_deref(),
            Some("n1.0")
        );
    }

    /// The same evaluation traced twice comes out identical, ids included,
    /// which is what lets a fixture pin one and a URL fragment point at one.
    /// Timing is the one thing that would differ, and an untimed builder
    /// records none.
    #[test]
    fn the_same_evaluation_traces_identically() {
        assert_eq!(addressed_trace(), addressed_trace());
    }

    /// A whole decimal does not survive a round trip, in the value tree or in
    /// the document: `Value::Decimal` serializes through `f64` as `100.0`,
    /// reads back as `Value::Int`, and re-serializes as `100`.
    ///
    /// Pinned because it bounds what "a recorded trace is a fixture" can mean
    /// (RFC-039). A golden JSON fixture has to be compared against a freshly
    /// serialized trace, in one direction. Comparing it against a
    /// re-serialization of its own parse would fail on any trace that carries a
    /// whole decimal, for a reason that has nothing to do with the trace.
    #[test]
    fn a_whole_decimal_does_not_survive_a_round_trip() {
        let mut builder = TraceBuilder::new_untimed();
        builder.push("bedrag", PathNodeType::Resolve);
        builder.set_result(Value::Decimal(rust_decimal::Decimal::from(100)));
        let original = builder.build().unwrap();

        let json = serde_json::to_string(&original).expect("serializes");
        assert!(json.contains("\"result\":100.0"), "got {json}");

        let parsed: PathNode = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(parsed.result, Some(Value::Int(100)));
        assert_ne!(parsed, original);

        let reserialized = serde_json::to_string(&parsed).expect("serializes again");
        assert!(
            reserialized.contains("\"result\":100"),
            "got {reserialized}"
        );
        assert_ne!(
            json, reserialized,
            "if these ever match, Value round-trips and this test can go"
        );
    }

    /// `PathNode` deserializes, so a recorded trace is a fixture: a renderer
    /// can be built against one with no engine in the loop, and a stored trace
    /// can be read back (RFC-039).
    ///
    /// The sample holds integers, booleans and strings. A decimal is the
    /// exception and has its own test below.
    #[test]
    fn a_trace_survives_a_json_round_trip() {
        let original = addressed_trace();
        let json = serde_json::to_string(&original).expect("serializes");

        let parsed: PathNode = serde_json::from_str(&json).expect("deserializes");
        let reserialized = serde_json::to_string(&parsed).expect("serializes again");

        assert_eq!(json, reserialized, "round trip is not stable");
        assert_eq!(parsed.node_id.as_deref(), Some("n"));
        assert_eq!(
            parsed.children[1].children[0]
                .source
                .as_ref()
                .and_then(|s| s.provider.as_deref()),
            Some("Zorgverzekeraar"),
            "the structured source survives the round trip"
        );
    }

    /// `from_article` reports every field the law header and the article
    /// carry. Each is asserted separately because a dropped one is invisible:
    /// the anchor still looks populated, and a reader silently loses the link
    /// to the statute or the version it was read at.
    #[test]
    fn an_anchor_reports_what_the_law_and_article_state() {
        let law: ArticleBasedLaw = serde_yaml_ng::from_str(
            r#"
$id: zorgverzekeringswet
uuid: 11111111-2222-3333-4444-555555555555
name: Zorgverzekeringswet
bwb_id: BWBR0018450
regulatory_layer: WET
publication_date: '2024-12-20'
valid_from: '2025-01-01'
articles:
  - number: '2'
    text: De verzekering is verplicht.
    url: https://wetten.overheid.nl/BWBR0018450/2025-01-01#Artikel2
"#,
        )
        .expect("fixture law parses");
        let article = &law.articles[0];

        let anchor = LegalAnchor::from_article(&law, article);

        assert_eq!(anchor.law_id.as_deref(), Some("zorgverzekeringswet"));
        assert_eq!(anchor.law.as_deref(), Some("Zorgverzekeringswet"));
        assert_eq!(
            anchor.law_uuid.as_deref(),
            Some("11111111-2222-3333-4444-555555555555")
        );
        assert_eq!(anchor.bwb_id.as_deref(), Some("BWBR0018450"));
        assert_eq!(anchor.article.as_deref(), Some("2"));
        assert_eq!(
            anchor.url.as_deref(),
            Some("https://wetten.overheid.nl/BWBR0018450/2025-01-01#Artikel2")
        );
        assert_eq!(anchor.valid_from.as_deref(), Some("2025-01-01"));

        // The engine has no opinion to record about how an element relates to
        // the text, and it does not guess a lid from a step's position.
        assert_eq!(anchor.explanation, None);
        assert_eq!(anchor.paragraph, None);
        assert_eq!(anchor.sentence, None);
    }

    /// A declaration that constrains a value without describing it leaves no
    /// spec on the step at all. `min`/`max` alone say nothing about what a
    /// value became, and an empty object would read as a declaration that was
    /// made and turned out blank.
    #[test]
    fn a_declaration_without_unit_or_precision_is_not_recorded() {
        let mut builder = TraceBuilder::new_untimed();
        builder.push("bedrag", PathNodeType::Action);
        builder.set_type_spec(TypeSpec {
            min: Some(rust_decimal::Decimal::ZERO),
            max: Some(rust_decimal::Decimal::ONE_HUNDRED),
            ..TypeSpec::default()
        });
        let root = builder.build().expect("a built trace");

        assert!(
            root.type_spec.is_none(),
            "bounds alone are not a description of a value, got {:?}",
            root.type_spec
        );

        // And a declaration that does describe the value is kept.
        let mut builder = TraceBuilder::new_untimed();
        builder.push("bedrag", PathNodeType::Action);
        builder.set_type_spec(TypeSpec {
            unit: Some("eurocent".to_string()),
            min: Some(rust_decimal::Decimal::ZERO),
            ..TypeSpec::default()
        });
        let root = builder.build().expect("a built trace");
        assert_eq!(
            root.type_spec.as_ref().and_then(|t| t.unit.as_deref()),
            Some("eurocent")
        );
    }

    /// `from_provision` carries the corpus citation across field for field,
    /// including the ones the zorgtoeslag corpus happens not to write. Each is
    /// asserted separately for the same reason as the anchor's: a dropped
    /// field leaves a citation that still looks complete, and the reader
    /// silently loses the lid, the sentence, or the modeller's wording.
    #[test]
    fn a_citation_carries_what_the_corpus_wrote() {
        let reference = ProvisionReference {
            law: Some("Wet op de zorgtoeslag".to_string()),
            bwb_id: Some("BWBR0018451".to_string()),
            article: Some("2".to_string()),
            paragraph: Some("1".to_string()),
            sentence: Some("2".to_string()),
            url: Some("https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel2".to_string()),
            juriconnect: Some("jci1.3:c:BWBR0018451&artikel=2&lid=1".to_string()),
            explanation: Some("Het verschil tussen standaardpremie en normpremie.".to_string()),
        };

        let basis = LegalAnchor::from_provision(&reference);

        assert_eq!(basis.law.as_deref(), Some("Wet op de zorgtoeslag"));
        assert_eq!(basis.bwb_id.as_deref(), Some("BWBR0018451"));
        assert_eq!(basis.article.as_deref(), Some("2"));
        assert_eq!(basis.paragraph.as_deref(), Some("1"));
        assert_eq!(basis.sentence.as_deref(), Some("2"));
        assert_eq!(
            basis.url.as_deref(),
            Some("https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel2")
        );
        assert_eq!(
            basis.juriconnect.as_deref(),
            Some("jci1.3:c:BWBR0018451&artikel=2&lid=1")
        );
        assert_eq!(
            basis.explanation.as_deref(),
            Some("Het verschil tussen standaardpremie en normpremie.")
        );

        // A citation names a provision, not the corpus file it was read from:
        // the reference may well point at a law the corpus does not hold.
        assert_eq!(basis.law_id, None);
        assert_eq!(basis.law_uuid, None);
        assert_eq!(basis.valid_from, None);
    }

    /// The addressing setters put their value on the current step, and each is
    /// a no-op once tracing is off.
    #[test]
    fn the_addressing_setters_record_on_the_current_step() {
        let mut builder = TraceBuilder::new_untimed();
        builder.push("hoogte_zorgtoeslag", PathNodeType::Action);
        builder.set_yaml_path("articles.2.machine_readable.execution.actions.hoogte_zorgtoeslag");
        builder.set_uri("regelrecht://wet_op_de_zorgtoeslag/hoogte_zorgtoeslag");
        builder.set_legal_basis(LegalAnchor {
            article: Some("2".to_string()),
            paragraph: Some("1".to_string()),
            ..LegalAnchor::default()
        });
        builder.set_type_spec(TypeSpec {
            unit: Some("eurocent".to_string()),
            ..TypeSpec::default()
        });
        assert_eq!(builder.current_node_id(), Some("n"));

        let root = builder.build().expect("a built trace");

        assert_eq!(
            root.yaml_path.as_deref(),
            Some("articles.2.machine_readable.execution.actions.hoogte_zorgtoeslag")
        );
        assert_eq!(
            root.uri.as_deref(),
            Some("regelrecht://wet_op_de_zorgtoeslag/hoogte_zorgtoeslag")
        );
        assert_eq!(
            root.legal_basis
                .as_ref()
                .and_then(|b| b.paragraph.as_deref()),
            Some("1")
        );
        assert_eq!(
            root.type_spec.as_ref().and_then(|t| t.unit.as_deref()),
            Some("eurocent")
        );
    }

    /// A disabled builder records nothing and has no current step, so a caller
    /// that sets an address on every node costs nothing when tracing is off.
    #[test]
    fn a_disabled_builder_records_no_address() {
        let mut builder = TraceBuilder::disabled();
        builder.push("x", PathNodeType::Action);
        builder.set_yaml_path("articles.1");
        builder.set_uri("regelrecht://a/b");
        builder.set_type_spec(TypeSpec::default());
        builder.set_legal_basis(LegalAnchor {
            article: Some("2".to_string()),
            ..LegalAnchor::default()
        });

        assert_eq!(builder.current_node_id(), None);
        assert!(builder.build().is_none());
    }

    /// An empty `legal_basis` is dropped like an empty anchor: absent means the
    /// element cites nothing, which a reader must be able to rely on.
    #[test]
    fn an_empty_legal_basis_is_not_recorded() {
        let mut builder = TraceBuilder::new_untimed();
        builder.push("artikel_1", PathNodeType::Article);
        builder.set_legal_basis(LegalAnchor::default());
        let root = builder.build().expect("a built trace");

        assert!(root.legal_basis.is_none());
    }

    /// An anchor with nothing in it is left off the node instead of appearing
    /// as an empty object, so a consumer can read "absent" as "the corpus does
    /// not cite this element" rather than having to check every field. A
    /// non-empty `legal_basis` on the same step is kept: the two are recorded
    /// independently.
    #[test]
    fn an_empty_anchor_is_not_recorded() {
        let mut builder = TraceBuilder::new_untimed();
        builder.push("artikel_1", PathNodeType::Article);
        builder.set_anchor(LegalAnchor::default());
        builder.set_legal_basis(LegalAnchor {
            article: Some("2".to_string()),
            paragraph: Some("1".to_string()),
            ..LegalAnchor::default()
        });
        let root = builder.build().unwrap();

        assert!(root.anchor.is_none(), "empty anchor was recorded");
        assert_eq!(
            root.legal_basis
                .as_ref()
                .and_then(|b| b.paragraph.as_deref()),
            Some("1")
        );
    }

    #[test]
    fn test_trace_builder_untimed_records_no_durations() {
        let mut builder = TraceBuilder::new_untimed();
        assert!(builder.is_enabled(), "untimed builder still traces");

        builder.push("root", PathNodeType::Resolve);
        builder.push("child", PathNodeType::Operation);
        builder.set_result(Value::Int(1));
        builder.pop();
        builder.set_result(Value::Int(1));
        let root = builder.build().unwrap();

        assert_eq!(
            root.duration_us, None,
            "untimed builder must not record a duration"
        );
        assert_eq!(
            root.children[0].duration_us, None,
            "untimed builder must not record a duration on nested nodes"
        );
    }

    #[test]
    fn test_trace_builder_depth_follows_stack() {
        let mut builder = TraceBuilder::new();
        assert_eq!(builder.depth(), 0, "a fresh builder has no open scopes");

        builder.push("level1", PathNodeType::Action);
        assert_eq!(builder.depth(), 1);

        builder.push("level2", PathNodeType::Operation);
        assert_eq!(builder.depth(), 2, "nested push deepens the stack");

        builder.pop();
        assert_eq!(builder.depth(), 1, "pop leaves the parent open");

        builder.pop();
        assert_eq!(builder.depth(), 0);
    }

    #[test]
    fn test_trace_builder_is_empty_follows_stack() {
        let mut builder = TraceBuilder::new();
        assert!(builder.is_empty());

        builder.push("root", PathNodeType::Resolve);
        assert!(
            !builder.is_empty(),
            "a builder with an open scope is not empty"
        );

        builder.pop();
        assert!(
            builder.is_empty(),
            "popping the last scope empties the stack"
        );
    }

    #[test]
    fn test_trace_builder_disabled() {
        let mut builder = TraceBuilder::disabled();
        assert!(!builder.is_enabled());

        builder.push("should_be_ignored", PathNodeType::Resolve);
        builder.set_result(Value::Int(42));

        assert!(builder.is_empty()); // Nothing was actually pushed
        assert!(builder.pop().is_none());
        assert!(builder.build().is_none());
    }

    #[test]
    fn test_trace_builder_resolve_type() {
        let mut builder = TraceBuilder::new();

        builder.push("param", PathNodeType::Resolve);
        builder.set_resolve_type(ResolveType::Parameter);
        builder.set_result(Value::String("test".to_string()));

        let node = builder.pop().unwrap();
        assert_eq!(node.resolve_type, Some(ResolveType::Parameter));
    }

    #[test]
    fn test_path_node_serialization() {
        let node = PathNode::new(PathNodeType::Resolve, "test")
            .with_result(Value::Int(42))
            .with_resolve_type(ResolveType::Definition);

        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("\"name\":\"test\""));
        assert!(json.contains("\"result\":42"));
    }

    #[test]
    fn test_deeply_nested_trace() {
        let mut builder = TraceBuilder::new();

        // Build a 3-level deep trace
        builder.push("level1", PathNodeType::Action);
        builder.push("level2", PathNodeType::Operation);
        builder.push("level3", PathNodeType::Resolve);
        builder.set_result(Value::Int(1));
        builder.pop(); // level3

        builder.set_result(Value::Int(2));
        builder.pop(); // level2

        builder.set_result(Value::Int(3));
        let root = builder.build().unwrap();

        assert_eq!(root.name, "level1");
        assert_eq!(root.result, Some(Value::Int(3)));
        assert_eq!(root.children.len(), 1);

        let level2 = &root.children[0];
        assert_eq!(level2.name, "level2");
        assert_eq!(level2.result, Some(Value::Int(2)));
        assert_eq!(level2.children.len(), 1);

        let level3 = &level2.children[0];
        assert_eq!(level3.name, "level3");
        assert_eq!(level3.result, Some(Value::Int(1)));
        assert!(level3.children.is_empty());
    }

    // -------------------------------------------------------------------------
    // Render Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_render_simple_node() {
        let node = PathNode::new(PathNodeType::Resolve, "inkomen")
            .with_result(Value::Int(50000))
            .with_resolve_type(ResolveType::Parameter);

        let rendered = node.render(0, false);
        assert!(rendered.contains("inkomen"));
        assert!(rendered.contains("resolve"));
        assert!(rendered.contains("parameter"));
        assert!(rendered.contains("50000"));
    }

    #[test]
    fn test_render_nested_trace() {
        let child1 = PathNode::new(PathNodeType::Resolve, "a")
            .with_result(Value::Int(10))
            .with_resolve_type(ResolveType::Parameter);

        let child2 = PathNode::new(PathNodeType::Resolve, "b")
            .with_result(Value::Int(20))
            .with_resolve_type(ResolveType::Definition);

        let root = PathNode::new(PathNodeType::Operation, "ADD")
            .with_result(Value::Int(30))
            .with_child(child1)
            .with_child(child2);

        let rendered = root.render(0, false);

        // Check structure
        assert!(rendered.contains("ADD (operation)"));
        assert!(rendered.contains("a (resolve)"));
        assert!(rendered.contains("b (resolve)"));

        // Check tree characters
        assert!(rendered.contains("+--") || rendered.contains("`--"));
    }

    #[test]
    fn test_render_marks_only_the_last_child_with_a_corner() {
        let root = PathNode::new(PathNodeType::Operation, "ADD")
            .with_child(PathNode::new(PathNodeType::Resolve, "first"))
            .with_child(PathNode::new(PathNodeType::Resolve, "middle"))
            .with_child(PathNode::new(PathNodeType::Resolve, "last"));

        let rendered = root.render(0, false);
        let line_for = |name: &str| {
            rendered
                .lines()
                .find(|l| l.contains(name))
                .unwrap_or_else(|| panic!("no line for {} in:\n{}", name, rendered))
                .to_string()
        };

        assert!(
            line_for("first").starts_with("+-- "),
            "a non-last child branches with +-- in:\n{}",
            rendered
        );
        assert!(
            line_for("middle").starts_with("+-- "),
            "a non-last child branches with +-- in:\n{}",
            rendered
        );
        assert!(
            line_for("last").starts_with("`-- "),
            "the last child closes the branch with `-- in:\n{}",
            rendered
        );
    }

    #[test]
    fn test_render_only_shows_significant_durations() {
        let quick = PathNode::new(PathNodeType::Operation, "quick").with_duration(99);
        assert!(
            !quick.render(0, false).contains("μs"),
            "durations under 0.1ms are noise and stay out of the trace: {}",
            quick.render(0, false)
        );

        let slow = PathNode::new(PathNodeType::Operation, "slow").with_duration(100);
        assert!(
            slow.render(0, false).contains("(100μs)"),
            "durations from 0.1ms up are shown: {}",
            slow.render(0, false)
        );
    }

    #[test]
    fn test_render_complex_tree() {
        // Build a more complex tree
        let resolve_a = PathNode::new(PathNodeType::Resolve, "var_a").with_result(Value::Int(100));

        let resolve_b = PathNode::new(PathNodeType::Resolve, "var_b").with_result(Value::Int(200));

        let add_op = PathNode::new(PathNodeType::Operation, "ADD")
            .with_result(Value::Int(300))
            .with_child(resolve_a)
            .with_child(resolve_b);

        let article = PathNode::new(PathNodeType::Article, "artikel_1").with_child(add_op);

        let rendered = article.render(0, false);

        // Verify the structure is readable
        let lines: Vec<&str> = rendered.lines().collect();
        assert!(lines[0].contains("artikel_1"));
        assert!(lines.iter().any(|l| l.contains("ADD")));
        assert!(lines.iter().any(|l| l.contains("var_a")));
        assert!(lines.iter().any(|l| l.contains("var_b")));
    }

    #[test]
    fn test_render_compact() {
        let node = PathNode::new(PathNodeType::Operation, "MULTIPLY").with_result(Value::Int(42));

        let compact = node.render_compact();
        assert_eq!(compact, "op:MULTIPLY=42");
    }

    #[test]
    fn test_render_compact_resolve() {
        let node = PathNode::new(PathNodeType::Resolve, "param")
            .with_result(Value::String("test".to_string()));

        let compact = node.render_compact();
        assert!(compact.starts_with("res:param="));
    }

    #[test]
    fn test_format_value_compact_truncates_long_strings() {
        let long_string = "this is a very long string that should be truncated";
        let formatted = format_value_compact(&Value::String(long_string.to_string()));
        assert!(formatted.len() < long_string.len());
        assert!(formatted.contains("..."));
    }

    #[test]
    fn test_format_value_compact_truncation_boundary() {
        // Exactly 20 characters still fits and is shown in full.
        let exactly_twenty = "12345678901234567890";
        assert_eq!(exactly_twenty.chars().count(), 20);
        assert_eq!(
            format_value_compact(&Value::String(exactly_twenty.to_string())),
            "\"12345678901234567890\""
        );

        // One character more and the value is truncated to 17 characters.
        let twenty_one = "123456789012345678901";
        assert_eq!(twenty_one.chars().count(), 21);
        assert_eq!(
            format_value_compact(&Value::String(twenty_one.to_string())),
            "\"12345678901234567...\""
        );
    }

    #[test]
    fn test_format_value_compact_utf8_safety() {
        // Dutch legal text with diacritics - would panic with byte slicing at position 17
        // if using &s[..17] instead of chars().take(17)
        let dutch_text = "éénentwintigste regeling met diakritische tekens";
        assert!(
            dutch_text.chars().count() > 20,
            "Test string must be > 20 chars"
        );
        let formatted = format_value_compact(&Value::String(dutch_text.to_string()));
        assert!(
            formatted.contains("..."),
            "Expected truncation for: {}",
            formatted
        );
        // Verify it produces valid output (doesn't panic on UTF-8 boundary)
        assert!(!formatted.is_empty());

        // String with multi-byte chars throughout - old code would panic
        // Each letter with accent is 2 bytes, slicing at byte 17 would cut mid-character
        let accented = "àáâãäåæçèéêëìíîïðñòóôõö";
        assert!(
            accented.chars().count() > 20,
            "Test string must be > 20 chars"
        );
        let formatted = format_value_compact(&Value::String(accented.to_string()));
        assert!(
            formatted.contains("..."),
            "Expected truncation for accented: {}",
            formatted
        );
    }

    #[test]
    fn test_format_value_compact_array() {
        let small_array = Value::Array(vec![Value::Int(1), Value::Int(2)]);
        let formatted = format_value_compact(&small_array);
        assert_eq!(formatted, "[1, 2]");

        let large_array = Value::Array(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
            Value::Int(4),
            Value::Int(5),
        ]);
        let formatted = format_value_compact(&large_array);
        assert!(formatted.contains("5 items"));
    }

    #[test]
    fn test_render_with_all_resolve_types() {
        let types = vec![
            (ResolveType::Uri, "uri"),
            (ResolveType::Parameter, "parameter"),
            (ResolveType::Definition, "definition"),
            (ResolveType::Output, "output"),
            (ResolveType::Input, "input"),
            (ResolveType::Local, "local"),
            (ResolveType::Context, "context"),
            (ResolveType::ResolvedInput, "resolved_input"),
            (ResolveType::DataSource, "data_source"),
        ];

        for (rt, expected_str) in types {
            let node = PathNode::new(PathNodeType::Resolve, "test").with_resolve_type(rt);
            let rendered = node.render(0, false);
            assert!(
                rendered.contains(&format!("[{}]", expected_str)),
                "Expected [{}] in: {}",
                expected_str,
                rendered
            );
        }
    }

    // --- Box-drawing rendering tests ---

    #[test]
    fn test_box_drawing_last_child_uses_corner() {
        let node = PathNode::new(PathNodeType::Operation, "ADD")
            .with_result(Value::Int(3))
            .with_message("Compute ADD(...) = 3")
            .with_child(PathNode::new(PathNodeType::Resolve, "a").with_result(Value::Int(1)))
            .with_child(PathNode::new(PathNodeType::Resolve, "b").with_result(Value::Int(2)));

        let rendered = node.render_box_drawing();
        let lines: Vec<&str> = rendered.lines().collect();
        // First child uses ├──, last child uses └──
        assert!(
            lines.iter().any(|l| l.contains("├──")),
            "Expected ├── for non-last child in:\n{}",
            rendered
        );
        assert!(
            lines.iter().any(|l| l.contains("└──")),
            "Expected └── for last child in:\n{}",
            rendered
        );
    }

    #[test]
    fn test_box_drawing_single_child_uses_corner() {
        // Wrap in a parent to avoid root-node rendering artifacts
        let child = PathNode::new(PathNodeType::Resolve, "x").with_result(Value::Null);
        let op = PathNode::new(PathNodeType::Operation, "ISNULL")
            .with_result(Value::Bool(true))
            .with_message("Compute ISNULL(...) = True")
            .with_child(child);
        let parent = PathNode::new(PathNodeType::Operation, "wrapper")
            .with_result(Value::Bool(true))
            .with_message("Compute wrapper(...) = True")
            .with_child(op);

        let rendered = parent.render_box_drawing();
        let lines: Vec<&str> = rendered.lines().collect();
        // The ISNULL's single child (x) should use └──
        let x_line = lines.iter().find(|l| l.contains("Resolving $X")).unwrap();
        assert!(
            x_line.contains("└──"),
            "Single child should use └── in:\n{}",
            rendered
        );
    }

    #[test]
    fn test_box_drawing_no_children() {
        // Wrap in parent to test leaf rendering within a tree
        let leaf = PathNode::new(PathNodeType::Resolve, "simple_var").with_result(Value::Int(42));
        let parent = PathNode::new(PathNodeType::Operation, "wrapper")
            .with_result(Value::Int(42))
            .with_message("Compute wrapper(...) = 42")
            .with_child(leaf);

        let rendered = parent.render_box_drawing();
        let lines: Vec<&str> = rendered.lines().collect();
        // The leaf node line itself should use └── (last child of parent)
        let leaf_line = lines.iter().find(|l| l.contains("SIMPLE_VAR")).unwrap();
        assert!(
            leaf_line.contains("└──"),
            "Leaf should use └── as last child in:\n{}",
            rendered
        );
    }

    #[test]
    fn test_box_drawing_cross_law_reference_double_lines() {
        let child = PathNode::new(PathNodeType::Resolve, "param").with_result(Value::Int(1));
        let uri_node = PathNode::new(PathNodeType::CrossLawReference, "other_law#output")
            .with_message("Reference: other_law#output")
            .with_child(child);

        let rendered = uri_node.render_box_drawing();
        // CrossLawReference children should use double-line connectors
        assert!(
            rendered.contains("╙──") || rendered.contains("╟──"),
            "CrossLawReference children should use double-line connectors in:\n{}",
            rendered
        );
    }

    #[test]
    fn test_box_drawing_article_scope_continues() {
        let action = PathNode::new(PathNodeType::Action, "compute_x")
            .with_result(Value::Int(10))
            .with_message("Computing compute_x")
            .with_child(PathNode::new(PathNodeType::Resolve, "a").with_result(Value::Int(10)));

        let article = PathNode::new(PathNodeType::Article, "test_law (output)")
            .with_result(Value::Int(10))
            .with_child(action);

        let rendered = article.render_box_drawing();
        // Article should have ╟── for "Evaluating rules" and ╙── for "Result"
        assert!(
            rendered.contains("╟──Evaluating"),
            "Article should have ╟──Evaluating in:\n{}",
            rendered
        );
        assert!(
            rendered.contains("╙──Result:"),
            "Article should have ╙──Result in:\n{}",
            rendered
        );
    }

    #[test]
    fn test_box_drawing_continuation_not_interrupted() {
        // Two CrossLawReferences under an Article: the continuation between
        // them should use ║ (double-scope parent). The second (last) one
        // uses ╙ and its subtree has blank continuation at that column.
        let child1 = PathNode::new(PathNodeType::Resolve, "a").with_result(Value::Int(1));
        let uri1 = PathNode::new(PathNodeType::CrossLawReference, "law1#out")
            .with_message("Reference: law1#out")
            .with_child(child1);
        let child2 = PathNode::new(PathNodeType::Resolve, "b").with_result(Value::Int(2));
        let uri2 = PathNode::new(PathNodeType::CrossLawReference, "law2#out")
            .with_message("Reference: law2#out")
            .with_child(child2);
        let article = PathNode::new(PathNodeType::Article, "test (out)")
            .with_message("test (2025-01-01 {} out)")
            .with_child(uri1)
            .with_child(uri2)
            .with_result(Value::Int(42));

        let rendered = article.render_box_drawing();
        let lines: Vec<&str> = rendered.lines().collect();
        // Between the two references, the ║ continuation should be present
        let has_double_continuation = lines.iter().any(|l| l.contains("║   ║"));
        assert!(
            has_double_continuation,
            "Non-last CrossLawReference inside Article should show ║ continuation in:\n{}",
            rendered
        );
    }

    #[test]
    fn test_box_drawing_continuation_between_siblings_of_one_cross_law_ref() {
        // The original bug (#471): several children under a *single*
        // CrossLawReference inside an Article. The test above restructured to
        // two references, which exercises a different column; the case with one
        // reference and multiple children is only covered by the zorgtoeslag
        // trace snapshot, so this is its unit-level regression anchor (#474).
        let uri = PathNode::new(PathNodeType::CrossLawReference, "other_law#out")
            .with_message("Reference: other_law#out")
            .with_child(PathNode::new(PathNodeType::Resolve, "a").with_result(Value::Int(1)))
            .with_child(PathNode::new(PathNodeType::Resolve, "b").with_result(Value::Int(2)))
            .with_child(PathNode::new(PathNodeType::Resolve, "c").with_result(Value::Int(3)));
        let article = PathNode::new(PathNodeType::Article, "test (out)")
            .with_message("test (2025-01-01 {} out)")
            .with_child(uri)
            .with_result(Value::Int(42));

        let rendered = article.render_box_drawing();
        let lines: Vec<&str> = rendered.lines().collect();

        // Every non-last child of the reference is joined with ╟── and the last
        // one with ╙──: the continuation must not break between a and b.
        let joins = lines
            .iter()
            .filter(|l| l.contains("╟──Resolving $A") || l.contains("╟──Resolving $B"))
            .count();
        assert_eq!(
            joins, 2,
            "children a and b are not last and should use ╟── in:\n{rendered}"
        );
        assert!(
            lines.iter().any(|l| l.contains("╙──Resolving $C")),
            "last child c should use ╙── in:\n{rendered}"
        );
        // And the reference's own subtree stays inside the double-line scope of
        // the Article: every child line carries the ║ continuation column.
        assert_eq!(
            lines
                .iter()
                .filter(|l| l.contains("║       ╟──") || l.contains("║       ╙──"))
                .count(),
            3,
            "all three children should sit under the ║ continuation in:\n{rendered}"
        );
    }

    #[test]
    fn test_box_drawing_cached_node() {
        let cached =
            PathNode::new(PathNodeType::Cached, "some_law#output").with_result(Value::Bool(false));

        let rendered = cached.render_box_drawing();
        assert!(
            rendered.contains("Cached:"),
            "Cached node should show Cached label in:\n{}",
            rendered
        );
    }

    #[test]
    fn test_box_drawing_result_format() {
        let action = PathNode::new(PathNodeType::Action, "my_output").with_result(Value::Int(42));

        let rendered = action.render_box_drawing();
        assert!(
            rendered.contains("Result: my_output = 42"),
            "Action result should show 'Result: name = value' in:\n{}",
            rendered
        );
    }

    fn object(entries: &[(&str, Value)]) -> Value {
        Value::Object(
            entries
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        )
    }

    #[test]
    fn an_object_without_an_iso_date_still_renders_as_a_dict() {
        let adres = object(&[("straat", Value::String("Dorpsstraat".into()))]);
        assert_eq!(format_value_display(&adres), "{'straat': 'Dorpsstraat'}");
    }

    #[test]
    fn a_single_output_that_is_an_object_keeps_its_whole_value() {
        // One output whose value happens to hold a key with its own name: the
        // value is the object, not that key.
        let adres = object(&[("adres", Value::Int(1)), ("postcode", Value::Int(2))]);
        assert_eq!(
            format_outputs("adres", &adres),
            "adres = {'adres': 1, 'postcode': 2}"
        );
    }

    /// Render one Requirement node carrying `result` and return its verdict
    /// line, the line the four tests below all turn on.
    fn requirement_verdict(result: Value) -> String {
        let requirement = PathNode::new(PathNodeType::Requirement, "req")
            .with_result(result)
            .with_child(PathNode::new(PathNodeType::Resolve, "a").with_result(Value::Int(1)));
        let rendered = requirement.render_box_drawing();
        // The verdict is the node's last line: the header ("Requirements") and
        // the children come first.
        rendered
            .lines()
            .next_back()
            .unwrap_or_else(|| panic!("nothing rendered for:\n{rendered}"))
            .to_string()
    }

    #[test]
    fn an_undecidable_requirement_names_the_facts_it_lacks() {
        // RFC-036: an Unknown is falsy, so without its own arm the requirement
        // would read "NOT met" — a decision the engine never made. The verdict
        // must name which facts are missing, so a reader can go get them.
        let verdict = requirement_verdict(Value::Unknown(vec![
            missing("huur"),
            missing("partner_bsn"),
        ]));
        assert!(
            verdict.ends_with("Requirement unknown (missing: huur, partner_bsn)"),
            "got {verdict}"
        );
    }

    #[test]
    fn an_untranslatable_requirement_names_the_article_it_comes_from() {
        // RFC-012: an Untranslatable is falsy too, and would likewise be
        // rendered as "NOT met". The verdict names the article whose construct
        // could not be translated, so the open norm is traceable.
        let verdict = requirement_verdict(Value::Untranslatable {
            article: "5".to_string(),
            construct: "naar redelijkheid".to_string(),
        });
        assert!(
            verdict.ends_with("Requirement untranslatable (art. 5)"),
            "got {verdict}"
        );
    }

    #[test]
    fn a_met_requirement_is_distinguished_from_a_failed_one() {
        // The guard on the verdict: a truthy result reads "met", a falsy one
        // "NOT met". Both directions are asserted, because a guard stuck on
        // either constant renders every requirement the same way.
        assert!(
            requirement_verdict(Value::Bool(true)).ends_with("Requirement met"),
            "a true requirement is met"
        );
        assert!(
            requirement_verdict(Value::Bool(false)).ends_with("Requirement NOT met"),
            "a false requirement is not met"
        );
    }

    #[test]
    fn a_non_boolean_requirement_result_is_judged_on_truthiness() {
        // Requirements are not always Bool: an empty list or a zero is falsy,
        // a non-empty one truthy, and the verdict follows to_bool rather than
        // the variant.
        assert!(
            requirement_verdict(Value::Int(1)).ends_with("Requirement met"),
            "a non-zero number is truthy"
        );
        assert!(
            requirement_verdict(Value::Array(vec![])).ends_with("Requirement NOT met"),
            "an empty array is falsy"
        );
    }
}
