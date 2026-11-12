use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use regelrecht_engine::{PathNode, PathNodeType, ResolveType, Value};
use std::cell::Cell;
use std::collections::HashSet;

/// A flattened trace node for rendering.
struct FlatNode {
    depth: usize,
    /// The step's own address, as the engine assigns it (RFC-039). This view
    /// used to compute an index chain of its own for want of one on the step;
    /// it is the same address, so the collapse state it keys on now means the
    /// same thing to anything else reading the trace.
    node_id: String,
    name: String,
    node_type: PathNodeType,
    resolve_type: Option<ResolveType>,
    result: Option<Value>,
    duration_us: Option<u64>,
    message: Option<String>,
    has_children: bool,
    is_collapsed: bool,
}

pub struct TraceView {
    trace: Option<PathNode>,
    flat_nodes: Vec<FlatNode>,
    collapsed: HashSet<String>,
    selected: usize,
    scroll_offset: usize,
    last_viewport_height: Cell<usize>,
}

impl TraceView {
    pub fn new() -> Self {
        Self {
            trace: None,
            flat_nodes: Vec::new(),
            collapsed: HashSet::new(),
            selected: 0,
            scroll_offset: 0,
            last_viewport_height: Cell::new(20),
        }
    }

    pub fn set_trace(&mut self, trace: PathNode) {
        self.trace = Some(trace);
        self.collapsed.clear();
        self.selected = 0;
        self.scroll_offset = 0;
        self.rebuild_flat();
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.selected < self.flat_nodes.len().saturating_sub(1) {
                    self.selected += 1;
                    self.ensure_visible();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                self.ensure_visible();
            }
            KeyCode::Char('g') => {
                self.selected = 0;
                self.scroll_offset = 0;
            }
            KeyCode::Char('G') => {
                if !self.flat_nodes.is_empty() {
                    self.selected = self.flat_nodes.len() - 1;
                    self.ensure_visible();
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.toggle_collapse();
            }
            KeyCode::Char('e') => {
                // Expand all
                self.collapsed.clear();
                self.rebuild_flat();
            }
            KeyCode::Char('c') => {
                // Collapse all at depth 1
                self.collapse_all();
            }
            _ => {}
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if self.trace.is_none() {
            let block = super::common::titled_block(" Execution Trace ");
            let content = Paragraph::new(vec![
                Line::from(""),
                Line::from("  No trace loaded."),
                Line::from(""),
                Line::from(Span::styled(
                    "  Evaluate a law in the Engine tab (2) to generate a trace.",
                    Style::default().add_modifier(Modifier::DIM),
                )),
            ])
            .block(block);
            frame.render_widget(content, area);
            return;
        }

        let inner_height = area.height.saturating_sub(2) as usize;
        self.last_viewport_height.set(inner_height);

        let lines: Vec<Line> = self
            .flat_nodes
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(inner_height)
            .map(|(i, node)| {
                let indent = "  ".repeat(node.depth);
                let expand_icon = if node.has_children {
                    if node.is_collapsed {
                        "▸ "
                    } else {
                        "▾ "
                    }
                } else {
                    "  "
                };

                let type_label = type_label(&node.node_type);
                let resolve_label = node
                    .resolve_type
                    .as_ref()
                    .map(|rt| format!(" [{}]", resolve_label(rt)))
                    .unwrap_or_default();

                let result_str = node
                    .result
                    .as_ref()
                    .map(|v| format!(" = {}", format_value_compact(v)))
                    .unwrap_or_default();

                let duration_str = node
                    .duration_us
                    .filter(|&d| d >= 100)
                    .map(|d| format!(" ({d}μs)"))
                    .unwrap_or_default();

                let display_name = node
                    .message
                    .as_deref()
                    .unwrap_or(&node.name);

                let text = format!(
                    "{indent}{expand_icon}{display_name} ({type_label}){resolve_label}{result_str}{duration_str}"
                );

                let style = if i == self.selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    match node.node_type {
                        PathNodeType::Article => Style::default().add_modifier(Modifier::BOLD),
                        PathNodeType::Requirement => Style::default(),
                        PathNodeType::Cached => Style::default().add_modifier(Modifier::DIM),
                        _ => Style::default(),
                    }
                };

                Line::styled(text, style)
            })
            .collect();

        let total = self.flat_nodes.len();
        let title =
            format!(" Execution Trace ({total} nodes) — Space:toggle  e:expand  c:collapse ");

        let block = super::common::titled_block(&title);

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);

        // Scrollbar
        if total > inner_height {
            let mut state = ScrollbarState::new(total.saturating_sub(inner_height))
                .position(self.scroll_offset);
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight),
                area.inner(Margin {
                    vertical: 1,
                    horizontal: 0,
                }),
                &mut state,
            );
        }
    }

    fn toggle_collapse(&mut self) {
        if let Some(node) = self.flat_nodes.get(self.selected)
            && node.has_children
        {
            let id = node.node_id.clone();
            if self.collapsed.contains(&id) {
                self.collapsed.remove(&id);
            } else {
                self.collapsed.insert(id);
            }
            self.rebuild_flat();
        }
    }

    fn collapse_all(&mut self) {
        // Collect the ids of every node that has children
        if let Some(ref trace) = self.trace {
            let mut ids_to_collapse = Vec::new();
            collect_parent_ids(trace, &mut ids_to_collapse);
            for id in ids_to_collapse {
                self.collapsed.insert(id);
            }
            self.rebuild_flat();
        }
    }

    fn rebuild_flat(&mut self) {
        self.flat_nodes.clear();
        if let Some(ref trace) = self.trace {
            flatten_node(trace, 0, &self.collapsed, &mut self.flat_nodes);
        }
        // Clamp selection
        if self.selected >= self.flat_nodes.len() && !self.flat_nodes.is_empty() {
            self.selected = self.flat_nodes.len() - 1;
        }
    }

    fn ensure_visible(&mut self) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
        let h = self.last_viewport_height.get();
        if self.selected >= self.scroll_offset + h {
            self.scroll_offset = self.selected - h + 1;
        }
    }
}

fn flatten_node(
    node: &PathNode,
    depth: usize,
    collapsed: &HashSet<String>,
    out: &mut Vec<FlatNode>,
) {
    // A step the engine did not address falls back to its name, so a trace
    // recorded before RFC-039 still collapses and expands.
    let node_id = node.node_id.clone().unwrap_or_else(|| node.name.clone());
    let is_collapsed = collapsed.contains(&node_id);

    out.push(FlatNode {
        depth,
        node_id,
        name: node.name.clone(),
        node_type: node.node_type.clone(),
        resolve_type: node.resolve_type.clone(),
        result: node.result.clone(),
        duration_us: node.duration_us,
        message: node.message.clone(),
        has_children: !node.children.is_empty(),
        is_collapsed,
    });

    if !is_collapsed {
        for child in &node.children {
            flatten_node(child, depth + 1, collapsed, out);
        }
    }
}

fn collect_parent_ids(node: &PathNode, out: &mut Vec<String>) {
    if !node.children.is_empty() {
        out.push(node.node_id.clone().unwrap_or_else(|| node.name.clone()));
    }
    for child in &node.children {
        collect_parent_ids(child, out);
    }
}

fn type_label(t: &PathNodeType) -> &'static str {
    match t {
        PathNodeType::Resolve => "resolve",
        PathNodeType::Operation => "op",
        PathNodeType::Action => "action",
        PathNodeType::Requirement => "req",
        PathNodeType::CrossLawReference => "xlaw",
        PathNodeType::Article => "article",
        PathNodeType::Cached => "cached",
        PathNodeType::OpenTermResolution => "open_term",
        PathNodeType::HookResolution => "hook",
        PathNodeType::OverrideResolution => "override",
    }
}

fn resolve_label(rt: &ResolveType) -> &'static str {
    match rt {
        ResolveType::Uri => "uri",
        ResolveType::Parameter => "param",
        ResolveType::Definition => "def",
        ResolveType::Output => "output",
        ResolveType::Input => "input",
        ResolveType::Local => "local",
        ResolveType::Context => "ctx",
        ResolveType::ResolvedInput => "res_input",
        ResolveType::DataSource => "data",
        ResolveType::OpenTerm => "open_term",
        ResolveType::OpenTermSilent => "open_term_silent",
        ResolveType::Hook => "hook",
        ResolveType::Override => "override",
    }
}

fn format_value_compact(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Decimal(d) => format!("{d:.2}"),
        Value::String(s) => {
            if s.chars().count() > 30 {
                let truncated: String = s.chars().take(27).collect();
                format!("\"{truncated}...\"")
            } else {
                format!("\"{s}\"")
            }
        }
        Value::Array(arr) => format!("[{} items]", arr.len()),
        Value::Object(obj) => format!("{{{} keys}}", obj.len()),
        Value::Untranslatable { article, .. } => format!("UNTRANSLATABLE(art. {})", article),
        // RFC-036: an unknown names the facts nobody supplied.
        Value::Unknown(_) => value.to_string(),
    }
}
