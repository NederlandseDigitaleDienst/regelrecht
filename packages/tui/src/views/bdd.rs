use crate::backend::process_runner::{ProcessMessage, ProcessMessageKind, ProcessRunner};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, Wrap,
};
use std::path::Path;

const MAX_OUTPUT_LINES: usize = 10_000;

struct FeatureEntry {
    name: String,
    #[allow(dead_code)]
    filename: String,
}

#[derive(Clone, Copy, PartialEq)]
enum Focus {
    Features,
    Output,
}

pub struct BddView {
    features: Vec<FeatureEntry>,
    list_state: ListState,
    output: Vec<OutputLine>,
    output_scroll: usize,
    running: bool,
    passed: usize,
    failed: usize,
    skipped: usize,
    scanned: bool,
    focus: Focus,
    follow: bool,
}

#[derive(Clone)]
struct OutputLine {
    text: String,
    is_stderr: bool,
}

impl BddView {
    pub fn new() -> Self {
        Self {
            features: Vec::new(),
            list_state: ListState::default(),
            output: Vec::new(),
            output_scroll: 0,
            running: false,
            passed: 0,
            failed: 0,
            skipped: 0,
            scanned: false,
            focus: Focus::Features,
            follow: true,
        }
    }

    pub fn scan_features(&mut self, project_root: &Path) {
        let features_dir = project_root.join("features");
        if features_dir.is_dir()
            && let Ok(entries) = std::fs::read_dir(&features_dir)
        {
            let mut features: Vec<FeatureEntry> = entries
                .flatten()
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "feature"))
                .map(|e| {
                    let filename = e.file_name().to_string_lossy().to_string();
                    let name = filename.trim_end_matches(".feature").replace('_', " ");
                    FeatureEntry { name, filename }
                })
                .collect();
            features.sort_by(|a, b| a.name.cmp(&b.name));
            self.features = features;
        }
        self.scanned = true;
        if !self.features.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, runner: &mut ProcessRunner) {
        if key.code == KeyCode::Tab {
            self.focus = match self.focus {
                Focus::Features => Focus::Output,
                Focus::Output => Focus::Features,
            };
            return;
        }

        match self.focus {
            Focus::Features => match key.code {
                KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
                KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
                KeyCode::Char('g') => self.list_state.select(Some(0)),
                KeyCode::Char('G') => {
                    if !self.features.is_empty() {
                        self.list_state.select(Some(self.features.len() - 1));
                    }
                }
                KeyCode::Char('a') => {
                    if !self.running && !runner.is_running() {
                        self.start_run(runner);
                    }
                }
                KeyCode::Enter if !self.running && !runner.is_running() => {
                    self.start_run(runner);
                }
                _ => {}
            },
            Focus::Output => match key.code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.output_scroll = self.output_scroll.saturating_add(3);
                    self.follow = false;
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.output_scroll = self.output_scroll.saturating_sub(3);
                    self.follow = false;
                }
                KeyCode::Char('g') => {
                    self.output_scroll = 0;
                    self.follow = false;
                }
                KeyCode::Char('G') => {
                    self.scroll_to_bottom();
                    self.follow = true;
                }
                _ => {}
            },
        }
    }

    fn start_run(&mut self, runner: &mut ProcessRunner) {
        self.output.clear();
        self.output_scroll = 0;
        self.passed = 0;
        self.failed = 0;
        self.skipped = 0;
        self.running = true;
        self.follow = true;
        self.focus = Focus::Output;
        runner.run_just("bdd:all".to_string(), "bdd");
    }

    pub fn handle_process_message(&mut self, msg: ProcessMessage) {
        match msg.kind {
            ProcessMessageKind::Stdout(line) => {
                self.parse_cucumber_line(&line);
                self.push_output(line, false);
            }
            ProcessMessageKind::Stderr(line) => {
                self.parse_cucumber_line(&line);
                self.push_output(line, true);
            }
            ProcessMessageKind::Done { .. } => {
                self.running = false;
            }
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        // Vertical: features top 1/3, output bottom 2/3
        let layout =
            Layout::vertical([Constraint::Percentage(30), Constraint::Percentage(70)]).split(area);

        self.render_feature_list(frame, layout[0]);
        self.render_output(frame, layout[1]);
    }

    fn render_feature_list(&self, frame: &mut Frame, area: Rect) {
        let dim = Style::default().add_modifier(Modifier::DIM);
        let items: Vec<ListItem> = self
            .features
            .iter()
            .map(|f| ListItem::new(Line::styled(format!("  {}", f.name), dim)))
            .collect();

        let border_style = if self.focus == Focus::Features {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default().add_modifier(Modifier::DIM)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(
                " Features ",
                Style::default().add_modifier(Modifier::BOLD),
            ));

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut state = self.list_state;
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn render_output(&self, frame: &mut Frame, area: Rect) {
        let inner_height = area.height.saturating_sub(2) as usize;
        let dim = Style::default().add_modifier(Modifier::DIM);
        let max_scroll = self.output.len().saturating_sub(inner_height);
        let effective_scroll = self.output_scroll.min(max_scroll);

        let visible: Vec<Line> = self
            .output
            .iter()
            .skip(effective_scroll)
            .take(inner_height)
            .map(|l| {
                let style = if l.is_stderr { Style::default() } else { dim };
                Line::styled(&*l.text, style)
            })
            .collect();

        let title = if self.running {
            " ● Running BDD tests... ".to_string()
        } else if self.passed > 0 || self.failed > 0 {
            format!(
                " ✓ {} passed  ✗ {} failed  ○ {} skipped ",
                self.passed, self.failed, self.skipped
            )
        } else {
            " Output — Enter/a:run all ".to_string()
        };

        let border_style = if self.focus == Focus::Output {
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

        let paragraph = Paragraph::new(visible)
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);

        if self.output.len() > inner_height {
            let mut state = ScrollbarState::new(max_scroll).position(effective_scroll);
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

    fn move_selection(&mut self, delta: i32) {
        let len = self.features.len();
        if len == 0 {
            return;
        }
        let current = self.list_state.selected().unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, len as i32 - 1) as usize;
        self.list_state.select(Some(next));
    }

    fn parse_cucumber_line(&mut self, line: &str) {
        let trimmed = line.trim();
        // Match summary lines like "3 passed" or "3 passed, 1 failed, 0 skipped"
        if trimmed.contains(" passed") || trimmed.contains(" failed") {
            // Split on commas and whitespace-separated tokens
            for part in trimmed.split([',', '(', ')']) {
                let part = part.trim();
                if let Some(num_str) = part.strip_suffix(" passed") {
                    if let Ok(n) = num_str.trim().parse::<usize>() {
                        self.passed = n;
                    }
                } else if let Some(num_str) = part.strip_suffix(" failed") {
                    if let Ok(n) = num_str.trim().parse::<usize>() {
                        self.failed = n;
                    }
                } else if let Some(num_str) = part.strip_suffix(" skipped")
                    && let Ok(n) = num_str.trim().parse::<usize>()
                {
                    self.skipped = n;
                }
            }
        }
    }

    fn scroll_to_bottom(&mut self) {
        self.output_scroll = self.output.len().saturating_sub(1);
    }

    fn push_output(&mut self, text: String, is_stderr: bool) {
        self.output.push(OutputLine { text, is_stderr });
        if self.output.len() > MAX_OUTPUT_LINES {
            self.output.drain(..self.output.len() - MAX_OUTPUT_LINES);
            self.output_scroll = self.output_scroll.saturating_sub(1);
        }
        if self.follow {
            self.scroll_to_bottom();
        }
    }
}
