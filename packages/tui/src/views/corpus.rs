use crate::backend::corpus_scanner::{self, CorpusNode, LawMetadata};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
};
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq)]
enum Focus {
    Tree,
    Preview,
}

pub struct CorpusView {
    nodes: Vec<CorpusNode>,
    selected: usize,
    collapsed: HashSet<PathBuf>,
    preview_content: Option<String>,
    preview_metadata: Option<LawMetadata>,
    preview_scroll: usize,
    tree_scroll: usize,
    scanned: bool,
    focus: Focus,
}

impl CorpusView {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            selected: 0,
            collapsed: HashSet::new(),
            preview_content: None,
            preview_metadata: None,
            preview_scroll: 0,
            tree_scroll: 0,
            scanned: false,
            focus: Focus::Tree,
        }
    }

    pub fn scan(&mut self, project_root: &Path) {
        self.nodes = corpus_scanner::scan_corpus(project_root);
        self.scanned = true;
        if !self.nodes.is_empty() {
            self.selected = 0;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Tab {
            self.focus = match self.focus {
                Focus::Tree => Focus::Preview,
                Focus::Preview => Focus::Tree,
            };
            return;
        }

        match self.focus {
            Focus::Tree => self.handle_tree_key(key),
            Focus::Preview => self.handle_preview_key(key),
        }
    }

    fn handle_tree_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::Char('g') => {
                self.selected = 0;
                self.tree_scroll = 0;
                self.load_preview();
            }
            KeyCode::Char('G') => {
                let visible = self.visible_nodes();
                if !visible.is_empty() {
                    self.selected = visible.len() - 1;
                    self.load_preview();
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                let visible = self.visible_nodes();
                if let Some(node) = visible.get(self.selected) {
                    let path = node.path.clone();
                    if node.is_dir {
                        if self.collapsed.contains(&path) {
                            self.collapsed.remove(&path);
                        } else {
                            self.collapsed.insert(path);
                        }
                    } else {
                        self.load_preview();
                        self.focus = Focus::Preview;
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_preview_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.preview_scroll = self.preview_scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.preview_scroll = self.preview_scroll.saturating_sub(1);
            }
            KeyCode::Char('g') => self.preview_scroll = 0,
            KeyCode::Char('G') => {
                if let Some(ref content) = self.preview_content {
                    let lines = content.lines().count();
                    self.preview_scroll = lines.saturating_sub(20);
                }
            }
            KeyCode::Esc => {
                self.focus = Focus::Tree;
            }
            _ => {}
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.scanned {
            let msg = Paragraph::new("Corpus Browser — scanning...").block(
                Block::default().borders(Borders::ALL).title(Span::styled(
                    " Corpus ",
                    Style::default().add_modifier(Modifier::BOLD),
                )),
            );
            frame.render_widget(msg, area);
            return;
        }

        if area.width >= 80 {
            let layout =
                Layout::horizontal([Constraint::Percentage(35), Constraint::Percentage(65)])
                    .split(area);

            self.render_tree(frame, layout[0]);
            self.render_preview(frame, layout[1]);
        } else {
            self.render_tree(frame, area);
        }
    }

    fn render_tree(&self, frame: &mut Frame, area: Rect) {
        let inner_height = area.height.saturating_sub(2) as usize;
        let visible = self.visible_nodes();
        let dim = Style::default().add_modifier(Modifier::DIM);

        let lines: Vec<Line> = visible
            .iter()
            .enumerate()
            .skip(self.tree_scroll)
            .take(inner_height)
            .map(|(i, node)| {
                let indent = "  ".repeat(node.depth);
                let (icon, base_style) = if node.is_dir {
                    let icon = if self.collapsed.contains(&node.path) {
                        "▸ "
                    } else {
                        "▾ "
                    };
                    (icon, dim)
                } else {
                    // YAML files get a document icon
                    ("  ", dim)
                };

                let style = if i == self.selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else if node.is_dir {
                    Style::default().add_modifier(Modifier::BOLD)
                } else {
                    base_style
                };

                Line::styled(format!("{indent}{icon}{}", node.name), style)
            })
            .collect();

        let title = format!(" Corpus ({} items) ", visible.len());
        let border_style = if self.focus == Focus::Tree {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(
                title,
                Style::default().add_modifier(Modifier::BOLD),
            ));

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);

        if visible.len() > inner_height {
            let mut state = ScrollbarState::new(visible.len().saturating_sub(inner_height))
                .position(self.tree_scroll);
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

    fn render_preview(&self, frame: &mut Frame, area: Rect) {
        let inner_height = area.height.saturating_sub(2) as usize;
        let dim = Style::default().add_modifier(Modifier::DIM);

        let mut lines: Vec<Line> = Vec::new();

        if let Some(ref meta) = self.preview_metadata {
            let mut meta_parts = Vec::new();
            if let Some(ref id) = meta.id {
                meta_parts.push(Span::styled(
                    format!("id:{id} "),
                    Style::default().add_modifier(Modifier::BOLD),
                ));
            }
            if let Some(ref layer) = meta.regulatory_layer {
                meta_parts.push(Span::styled(format!("{layer} "), dim));
            }
            if let Some(ref date) = meta.valid_from {
                meta_parts.push(Span::styled(format!("valid:{date} "), dim));
            }
            if meta.article_count > 0 {
                meta_parts.push(Span::styled(format!("arts:{} ", meta.article_count), dim));
            }
            if let Some(ref bwb) = meta.bwb_id {
                meta_parts.push(Span::styled(format!("bwb:{bwb}"), dim));
            }
            if !meta_parts.is_empty() {
                lines.push(Line::from(meta_parts));
                lines.push(Line::styled(
                    "─".repeat(area.width.saturating_sub(2) as usize),
                    dim,
                ));
            }
        }

        if let Some(ref content) = self.preview_content {
            for line in content.lines().skip(self.preview_scroll) {
                if lines.len() >= inner_height {
                    break;
                }
                lines.push(highlight_yaml_line(line));
            }
        } else {
            lines.push(Line::from(""));
            lines.push(Line::styled("  Select a YAML file to preview", dim));
        }

        let border_style = if self.focus == Focus::Preview {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(
                " Preview (Tab to focus, j/k scroll) ",
                Style::default().add_modifier(Modifier::BOLD),
            ));

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);

        if let Some(ref content) = self.preview_content {
            let total = content.lines().count();
            if total > inner_height {
                let mut state = ScrollbarState::new(total.saturating_sub(inner_height))
                    .position(self.preview_scroll);
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
    }

    fn visible_nodes(&self) -> Vec<&CorpusNode> {
        let mut result = Vec::new();
        let mut skip_below: Option<&Path> = None;

        for node in &self.nodes {
            if let Some(parent) = skip_below {
                if node.path.starts_with(parent) {
                    continue;
                } else {
                    skip_below = None;
                }
            }

            result.push(node);

            if node.is_dir && self.collapsed.contains(&node.path) {
                skip_below = Some(&node.path);
            }
        }

        result
    }

    fn move_selection(&mut self, delta: i32) {
        let visible = self.visible_nodes();
        let len = visible.len();
        if len == 0 {
            return;
        }
        let current = self.selected as i32;
        self.selected = (current + delta).clamp(0, len as i32 - 1) as usize;

        // Keep selection visible in scroll
        if self.selected < self.tree_scroll {
            self.tree_scroll = self.selected;
        }
        if self.selected >= self.tree_scroll + 20 {
            self.tree_scroll = self.selected.saturating_sub(19);
        }

        self.load_preview();
    }

    fn load_preview(&mut self) {
        let visible = self.visible_nodes();
        if let Some(node) = visible.get(self.selected)
            && !node.is_dir
            && node
                .path
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
            && let Ok(content) = std::fs::read_to_string(&node.path)
        {
            self.preview_metadata = Some(corpus_scanner::extract_metadata(&content));
            self.preview_content = Some(content);
            self.preview_scroll = 0;
        }
    }
}

fn highlight_yaml_line(line: &str) -> Line<'_> {
    let trimmed = line.trim_start();
    let dim = Style::default().add_modifier(Modifier::DIM);

    if trimmed.starts_with('#') {
        return Line::styled(
            line,
            Style::default().add_modifier(Modifier::DIM | Modifier::ITALIC),
        );
    }

    if let Some(rest) = trimmed.strip_prefix("- ")
        && let Some((key, val)) = rest.split_once(':')
    {
        let indent = &line[..line.len() - trimmed.len()];
        return Line::from(vec![
            Span::styled(format!("{indent}- "), dim),
            Span::styled(key.to_string(), Style::default()),
            Span::styled(":", dim),
            Span::styled(val.to_string(), dim),
        ]);
    }

    if let Some((key, val)) = trimmed.split_once(':')
        && !key.contains("//")
        && !key.contains(' ')
    {
        let indent = &line[..line.len() - trimmed.len()];
        return Line::from(vec![
            Span::styled(indent.to_string(), dim),
            Span::styled(key.to_string(), Style::default()),
            Span::styled(":", dim),
            Span::styled(val.to_string(), dim),
        ]);
    }

    Line::styled(line, dim)
}
