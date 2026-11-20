use crate::backend::corpus_scanner;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{List, ListItem, ListState, Paragraph};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone)]
struct DepNode {
    law_id: String,
    layer: String,
    depends_on: Vec<String>,  // laws this law references via source
    depended_by: Vec<String>, // laws that reference this law
    implements: Vec<String>,  // open terms this law implements
    open_terms: Vec<String>,  // open terms this law declares
}

pub struct DepsView {
    nodes: Vec<DepNode>,
    list_state: ListState,
    detail_scroll: usize,
    scanned: bool,
}

impl DepsView {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            list_state: ListState::default(),
            detail_scroll: 0,
            scanned: false,
        }
    }

    pub fn scan_deps(&mut self, project_root: &Path) {
        let yaml_files = corpus_scanner::corpus_yaml_files(project_root);

        // First pass: collect all law IDs and their source references
        let mut law_layers: HashMap<String, String> = HashMap::new();
        let mut source_refs: HashMap<String, Vec<String>> = HashMap::new(); // law_id -> [referenced_law_ids]
        let mut implements_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut open_terms_map: HashMap<String, Vec<String>> = HashMap::new();

        for path in &yaml_files {
            if let Ok(content) = std::fs::read_to_string(path) {
                let meta = corpus_scanner::extract_metadata(&content);
                if let Some(ref id) = meta.id {
                    law_layers.insert(
                        id.clone(),
                        meta.regulatory_layer.clone().unwrap_or_default(),
                    );

                    // Parse source references
                    let refs = extract_source_refs(&content);
                    if !refs.is_empty() {
                        source_refs.insert(id.clone(), refs);
                    }

                    // Parse implements
                    let impls = extract_implements(&content);
                    if !impls.is_empty() {
                        implements_map.insert(id.clone(), impls);
                    }

                    // Parse open_terms
                    let terms = extract_open_terms(&content);
                    if !terms.is_empty() {
                        open_terms_map.insert(id.clone(), terms);
                    }
                }
            }
        }

        // Build reverse map (depended_by)
        let mut reverse_deps: HashMap<String, Vec<String>> = HashMap::new();
        for (law_id, refs) in &source_refs {
            for dep in refs {
                reverse_deps
                    .entry(dep.clone())
                    .or_default()
                    .push(law_id.clone());
            }
        }

        // Build nodes
        let mut nodes: Vec<DepNode> = law_layers
            .iter()
            .map(|(id, layer)| DepNode {
                law_id: id.clone(),
                layer: layer.clone(),
                depends_on: source_refs.get(id).cloned().unwrap_or_default(),
                depended_by: reverse_deps.get(id).cloned().unwrap_or_default(),
                implements: implements_map.get(id).cloned().unwrap_or_default(),
                open_terms: open_terms_map.get(id).cloned().unwrap_or_default(),
            })
            .collect();

        nodes.sort_by(|a, b| a.law_id.cmp(&b.law_id));
        self.nodes = nodes;
        self.scanned = true;

        if !self.nodes.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let len = self.nodes.len();
                if len > 0 {
                    let i = self.list_state.selected().unwrap_or(0);
                    self.list_state.select(Some((i + 1).min(len - 1)));
                    self.detail_scroll = 0;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let i = self.list_state.selected().unwrap_or(0);
                self.list_state.select(Some(i.saturating_sub(1)));
                self.detail_scroll = 0;
            }
            KeyCode::Char('g') => {
                self.list_state.select(Some(0));
                self.detail_scroll = 0;
            }
            KeyCode::Char('G') => {
                if !self.nodes.is_empty() {
                    self.list_state.select(Some(self.nodes.len() - 1));
                    self.detail_scroll = 0;
                }
            }
            KeyCode::Char('J') | KeyCode::PageDown => {
                self.detail_scroll = self.detail_scroll.saturating_add(5);
            }
            KeyCode::Char('K') | KeyCode::PageUp => {
                self.detail_scroll = self.detail_scroll.saturating_sub(5);
            }
            _ => {}
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.scanned {
            let block = super::common::titled_block(" Dependencies ");
            let content = Paragraph::new("  Scanning corpus...").block(block);
            frame.render_widget(content, area);
            return;
        }

        if area.width >= 80 {
            let layout =
                Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
                    .split(area);

            self.render_law_list(frame, layout[0]);
            self.render_detail(frame, layout[1]);
        } else {
            self.render_law_list(frame, area);
        }
    }

    fn render_law_list(&self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .nodes
            .iter()
            .map(|n| {
                let dep_count = n.depends_on.len() + n.depended_by.len();
                let indicator = if dep_count > 0 {
                    format!(" ({dep_count})")
                } else {
                    String::new()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("  {}", n.law_id),
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                    Span::styled(indicator, Style::default().add_modifier(Modifier::DIM)),
                ]))
            })
            .collect();

        let edges: usize = self.nodes.iter().map(|n| n.depends_on.len()).sum();
        let title = format!(" Laws ({}) — {} cross-references ", self.nodes.len(), edges);

        let block = super::common::titled_block(&title);

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut state = self.list_state;
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn render_detail(&self, frame: &mut Frame, area: Rect) {
        let inner_height = area.height.saturating_sub(2) as usize;
        let mut lines: Vec<Line> = Vec::new();

        if let Some(i) = self.list_state.selected() {
            if let Some(node) = self.nodes.get(i) {
                lines.push(Line::styled(
                    format!(" {} ({})", node.law_id, node.layer),
                    Style::default().add_modifier(Modifier::BOLD),
                ));
                lines.push(Line::from(""));

                if !node.depends_on.is_empty() {
                    lines.push(Line::styled(
                        " Depends on:",
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                    for dep in &node.depends_on {
                        lines.push(Line::styled(
                            format!("   → {dep}"),
                            Style::default().add_modifier(Modifier::DIM),
                        ));
                    }
                    lines.push(Line::from(""));
                }

                if !node.depended_by.is_empty() {
                    lines.push(Line::styled(
                        " Referenced by:",
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                    for dep in &node.depended_by {
                        lines.push(Line::styled(
                            format!("   ← {dep}"),
                            Style::default().add_modifier(Modifier::DIM),
                        ));
                    }
                    lines.push(Line::from(""));
                }

                if !node.open_terms.is_empty() {
                    lines.push(Line::styled(
                        " Declares open terms:",
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                    for term in &node.open_terms {
                        lines.push(Line::styled(
                            format!("   ◇ {term}"),
                            Style::default().add_modifier(Modifier::DIM),
                        ));
                    }
                    lines.push(Line::from(""));
                }

                if !node.implements.is_empty() {
                    lines.push(Line::styled(
                        " Implements:",
                        Style::default().add_modifier(Modifier::BOLD),
                    ));
                    for imp in &node.implements {
                        lines.push(Line::styled(
                            format!("   ◆ {imp}"),
                            Style::default().add_modifier(Modifier::DIM),
                        ));
                    }
                }

                if node.depends_on.is_empty()
                    && node.depended_by.is_empty()
                    && node.open_terms.is_empty()
                    && node.implements.is_empty()
                {
                    lines.push(Line::styled(
                        " No cross-law references",
                        Style::default().add_modifier(Modifier::DIM),
                    ));
                }
            }
        } else {
            lines.push(Line::styled(
                "  Select a law to see its dependencies",
                Style::default().add_modifier(Modifier::DIM),
            ));
        }

        let visible: Vec<Line> = lines
            .into_iter()
            .skip(self.detail_scroll)
            .take(inner_height)
            .collect();

        let block = super::common::titled_block(" Detail (J/K to scroll) ");

        let paragraph = Paragraph::new(visible).block(block);
        frame.render_widget(paragraph, area);
    }
}

/// Extract `source.regulation` values from YAML content (simple line parsing).
fn extract_source_refs(content: &str) -> Vec<String> {
    let mut refs = HashSet::new();
    let mut in_source = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("source:") {
            in_source = true;
            continue;
        }
        if in_source {
            if let Some(val) = trimmed.strip_prefix("regulation:") {
                let val = val.trim().trim_matches('\'').trim_matches('"');
                if !val.is_empty() {
                    refs.insert(val.to_string());
                }
                in_source = false;
            } else if !trimmed.starts_with('-')
                && !trimmed.is_empty()
                && !trimmed.starts_with("output:")
                && !trimmed.starts_with("parameters:")
            {
                in_source = false;
            }
        }
    }

    refs.into_iter().collect()
}

/// Extract `implements[].law` values from YAML content.
fn extract_implements(content: &str) -> Vec<String> {
    let mut impls = Vec::new();
    let mut in_implements = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "implements:" {
            in_implements = true;
            continue;
        }
        if in_implements {
            if let Some(val) = trimmed
                .strip_prefix("- law:")
                .or_else(|| trimmed.strip_prefix("law:"))
            {
                let val = val.trim().trim_matches('\'').trim_matches('"');
                if !val.is_empty() {
                    impls.push(val.to_string());
                }
            }
            // Stop when we hit a non-indented, non-list line
            if !trimmed.is_empty()
                && !trimmed.starts_with('-')
                && !trimmed.starts_with("law:")
                && !trimmed.starts_with("article:")
                && !trimmed.starts_with("open_term:")
            {
                in_implements = false;
            }
        }
    }

    impls
}

/// Extract `open_terms[]` from YAML content.
fn extract_open_terms(content: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut in_open_terms = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "open_terms:" {
            in_open_terms = true;
            continue;
        }
        if in_open_terms {
            if let Some(val) = trimmed
                .strip_prefix("- name:")
                .or_else(|| trimmed.strip_prefix("name:"))
            {
                let val = val.trim().trim_matches('\'').trim_matches('"');
                if !val.is_empty() {
                    terms.push(val.to_string());
                }
            } else if let Some(val) = trimmed.strip_prefix("- ") {
                let val = val.trim().trim_matches('\'').trim_matches('"');
                if !val.is_empty() && !val.contains(':') {
                    terms.push(val.to_string());
                }
            }
            if !trimmed.is_empty()
                && !trimmed.starts_with('-')
                && !trimmed.starts_with("name:")
                && !trimmed.starts_with("type:")
                && !trimmed.starts_with("description:")
            {
                in_open_terms = false;
            }
        }
    }

    terms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_refs_are_the_regulations_named_under_source() {
        let yaml = "\
- name: inkomen
  source:
    regulation: wet_inkomstenbelasting_2001
    output: verzamelinkomen
- name: leeftijd
  source:
    output: leeftijd
- name: toetsingsinkomen
  source:
    regulation: 'algemene_wet_inkomensafhankelijke_regelingen'
";
        let mut refs = extract_source_refs(yaml);
        refs.sort();
        assert_eq!(
            refs,
            [
                "algemene_wet_inkomensafhankelijke_regelingen",
                "wet_inkomstenbelasting_2001"
            ]
        );
    }

    #[test]
    fn a_regulation_named_twice_is_one_source_ref() {
        let yaml = "source:\n  regulation: a\nsource:\n  regulation: a\n";
        assert_eq!(extract_source_refs(yaml), ["a"]);
    }

    #[test]
    fn implements_lists_each_law_until_the_block_ends() {
        let yaml = "\
implements:
  - law: participatiewet
    article: '8'
    open_term: verlaging
  - law: \"wet_x\"
articles:
  - law: not_an_implementation
";
        assert_eq!(extract_implements(yaml), ["participatiewet", "wet_x"]);
    }

    #[test]
    fn open_terms_reads_named_entries_and_bare_list_items() {
        let yaml = "\
open_terms:
  - name: verlaging
    type: amount
    description: de verlaging
  - normbedrag
articles: []
";
        assert_eq!(extract_open_terms(yaml), ["verlaging", "normbedrag"]);
    }

    #[test]
    fn content_without_these_blocks_yields_nothing() {
        let yaml = "$id: wet\narticles: []\n";
        assert!(extract_source_refs(yaml).is_empty());
        assert!(extract_implements(yaml).is_empty());
        assert!(extract_open_terms(yaml).is_empty());
    }
}
