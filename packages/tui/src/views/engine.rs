use crate::backend::engine_backend::{EngineCommand, EngineHandle, EngineResponse};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use regelrecht_engine::{ArticleResult, LawInfo, PathNode, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Focus {
    LawList,
    OutputList,
    ParamForm,
    Result,
}

struct ParamEntry {
    name: String,
    value: String,
}

pub struct EngineView {
    focus: Focus,
    laws: Vec<String>,
    law_list_state: ListState,
    current_law_info: Option<LawInfo>,
    output_list_state: ListState,
    params: Vec<ParamEntry>,
    param_cursor: usize,
    date_input: String,
    editing_date: bool,
    result: Option<ArticleResult>,
    #[allow(dead_code)]
    trace: Option<PathNode>,
    error: Option<String>,
    result_scroll: usize,
    loaded: bool,
    law_count: usize,
}

impl EngineView {
    pub fn new() -> Self {
        Self {
            focus: Focus::LawList,
            laws: Vec::new(),
            law_list_state: ListState::default(),
            current_law_info: None,
            output_list_state: ListState::default(),
            params: vec![ParamEntry {
                name: String::new(),
                value: String::new(),
            }],
            param_cursor: 0,
            date_input: regelrecht_shared::dates::today_str(),
            editing_date: false,
            result: None,
            trace: None,
            error: None,
            result_scroll: 0,
            loaded: false,
            law_count: 0,
        }
    }

    pub fn is_text_input(&self) -> bool {
        self.focus == Focus::ParamForm
    }

    pub fn handle_engine_response(&mut self, resp: EngineResponse) {
        match resp {
            EngineResponse::Loaded { law_count } => {
                self.loaded = true;
                self.law_count = law_count;
            }
            EngineResponse::LawList(laws) => {
                self.laws = laws;
                if !self.laws.is_empty() {
                    self.law_list_state.select(Some(0));
                }
            }
            EngineResponse::LawInfo(info) => {
                self.current_law_info = info;
                self.output_list_state.select(Some(0));
            }
            EngineResponse::EvalResult { result, trace } => {
                self.trace = trace;
                match result {
                    Ok(r) => {
                        self.error = None;
                        self.result = Some(r);
                        self.focus = Focus::Result;
                    }
                    Err(e) => {
                        // If a variable is missing, auto-add it as a parameter
                        // and focus on the value field so the user can type it
                        if let Some(var_name) = e.strip_prefix("Variable not found: ") {
                            let var_name = var_name.trim().to_string();
                            // Don't add duplicates — check by name only
                            let already_exists = self.params.iter().any(|p| p.name == var_name);
                            if !already_exists {
                                // Remove empty placeholder entries
                                self.params.retain(|p| !p.name.is_empty());
                                self.params.push(ParamEntry {
                                    name: var_name,
                                    value: String::new(),
                                });
                                self.param_cursor = self.params.len() - 1;
                                self.editing_date = false;
                                self.focus = Focus::ParamForm;
                                self.error = Some(e);
                                self.result = None;
                                self.result_scroll = 0;
                                return;
                            }
                        }
                        self.error = Some(e);
                        self.result = None;
                        self.focus = Focus::Result;
                    }
                }
                self.result_scroll = 0;
            }
            EngineResponse::ThreadDied => {
                self.error = Some("Engine thread crashed — restart rrtui".to_string());
                self.focus = Focus::Result;
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent, engine: &EngineHandle) {
        // Tab cycles focus through the visible panes
        if key.code == KeyCode::Tab {
            self.focus = match self.focus {
                Focus::LawList => {
                    if self.current_law_info.is_some() {
                        Focus::OutputList
                    } else {
                        Focus::LawList
                    }
                }
                Focus::OutputList => Focus::ParamForm,
                Focus::ParamForm => {
                    if self.result.is_some() || self.error.is_some() {
                        Focus::Result
                    } else {
                        Focus::LawList
                    }
                }
                Focus::Result => Focus::LawList,
            };
            return;
        }

        if key.code == KeyCode::Esc {
            self.focus = match self.focus {
                Focus::OutputList => Focus::LawList,
                Focus::ParamForm => Focus::OutputList,
                Focus::Result => Focus::ParamForm,
                Focus::LawList => Focus::LawList,
            };
            return;
        }

        match self.focus {
            Focus::LawList => self.handle_law_list_key(key, engine),
            Focus::OutputList => self.handle_output_list_key(key),
            Focus::ParamForm => self.handle_param_form_key(key, engine),
            Focus::Result => self.handle_result_key(key),
        }
    }

    fn handle_law_list_key(&mut self, key: KeyEvent, engine: &EngineHandle) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let len = self.laws.len();
                if len > 0 {
                    let i = self.law_list_state.selected().unwrap_or(0);
                    self.law_list_state.select(Some((i + 1).min(len - 1)));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let i = self.law_list_state.selected().unwrap_or(0);
                self.law_list_state.select(Some(i.saturating_sub(1)));
            }
            KeyCode::Enter => {
                if let Some(i) = self.law_list_state.selected()
                    && let Some(law_id) = self.laws.get(i)
                {
                    engine.send(EngineCommand::GetLawInfo(law_id.clone()));
                    self.focus = Focus::OutputList;
                }
            }
            KeyCode::Char('g') => self.law_list_state.select(Some(0)),
            KeyCode::Char('G') if !self.laws.is_empty() => {
                self.law_list_state.select(Some(self.laws.len() - 1));
            }
            _ => {}
        }
    }

    fn handle_output_list_key(&mut self, key: KeyEvent) {
        let output_count = self
            .current_law_info
            .as_ref()
            .map(|i| i.outputs.len())
            .unwrap_or(0);

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if output_count > 0 {
                    let i = self.output_list_state.selected().unwrap_or(0);
                    self.output_list_state
                        .select(Some((i + 1).min(output_count - 1)));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let i = self.output_list_state.selected().unwrap_or(0);
                self.output_list_state.select(Some(i.saturating_sub(1)));
            }
            KeyCode::Enter => {
                self.focus = Focus::ParamForm;
            }
            _ => {}
        }
    }

    fn handle_param_form_key(&mut self, key: KeyEvent, engine: &EngineHandle) {
        match key.code {
            KeyCode::Enter => {
                self.execute(engine);
            }
            KeyCode::Down => {
                if self.editing_date {
                    // nowhere to go
                } else if self.param_cursor < self.params.len().saturating_sub(1) {
                    self.param_cursor += 1;
                } else {
                    self.editing_date = true;
                }
            }
            KeyCode::Up => {
                if self.editing_date {
                    self.editing_date = false;
                    self.param_cursor = self.params.len().saturating_sub(1);
                } else {
                    self.param_cursor = self.param_cursor.saturating_sub(1);
                }
            }
            KeyCode::Backspace => {
                if self.editing_date {
                    self.date_input.pop();
                } else if let Some(param) = self.params.get_mut(self.param_cursor) {
                    if !param.value.is_empty() {
                        param.value.pop();
                    } else {
                        param.name.pop();
                    }
                }
            }
            KeyCode::Char(':') => {
                if !self.editing_date
                    && let Some(param) = self.params.get_mut(self.param_cursor)
                {
                    if param.value.is_empty() && !param.name.is_empty() {
                        return; // colon is separator, don't add
                    }
                    param.value.push(':');
                }
            }
            KeyCode::Char(c) => {
                if self.editing_date {
                    self.date_input.push(c);
                } else {
                    // +/- are add/remove only when no param is being edited
                    let is_editing = self
                        .params
                        .get(self.param_cursor)
                        .is_some_and(|p| !p.name.is_empty());
                    if c == '+' && !is_editing {
                        self.params.push(ParamEntry {
                            name: String::new(),
                            value: String::new(),
                        });
                        self.param_cursor = self.params.len() - 1;
                    } else if c == '-' && !is_editing && self.params.len() > 1 {
                        self.params.remove(self.param_cursor);
                        if self.param_cursor >= self.params.len() {
                            self.param_cursor = self.params.len() - 1;
                        }
                    } else if let Some(param) = self.params.get_mut(self.param_cursor) {
                        if param.value.is_empty() && !param.name.is_empty() {
                            param.value.push(c);
                        } else if param.value.is_empty() {
                            param.name.push(c);
                        } else {
                            param.value.push(c);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_result_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.result_scroll = self.result_scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.result_scroll = self.result_scroll.saturating_sub(1);
            }
            KeyCode::Char('g') => self.result_scroll = 0,
            _ => {}
        }
    }

    fn execute(&mut self, engine: &EngineHandle) {
        let law_id = match self.law_list_state.selected() {
            Some(i) => match self.laws.get(i) {
                Some(id) => id.clone(),
                None => return,
            },
            None => return,
        };

        let output = match self.output_list_state.selected() {
            Some(i) => {
                match self
                    .current_law_info
                    .as_ref()
                    .and_then(|info| info.outputs.get(i))
                {
                    Some(name) => name.clone(),
                    None => return,
                }
            }
            None => return,
        };

        let mut params = BTreeMap::new();
        for p in &self.params {
            if !p.name.is_empty() && !p.value.is_empty() {
                params.insert(p.name.clone(), parse_value(&p.value));
            }
        }

        engine.send(EngineCommand::Evaluate {
            law_id,
            output,
            params,
            date: self.date_input.clone(),
        });
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.loaded {
            let block = super::common::titled_block(" Interactive Engine ");
            let content = Paragraph::new("  Loading laws from corpus...").block(block);
            frame.render_widget(content, area);
            return;
        }

        // Always vertical layout: works at any width
        // Top: law list (1/3)
        // Middle: outputs + params (1/3)
        // Bottom: result (1/3)
        let rows = Layout::vertical([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);

        self.render_law_list(frame, rows[0]);

        // Middle: outputs left, params right (if wide enough)
        if area.width >= 80 {
            let mid = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(rows[1]);
            self.render_output_list(frame, mid[0]);
            self.render_param_form(frame, mid[1]);
        } else {
            // Stack vertically
            let mid = Layout::vertical([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(rows[1]);
            self.render_output_list(frame, mid[0]);
            self.render_param_form(frame, mid[1]);
        }

        self.render_result(frame, rows[2]);
    }

    fn render_law_list(&self, frame: &mut Frame, area: Rect) {
        let dim = Style::default().add_modifier(Modifier::DIM);
        let title = format!(" Laws ({}) — Enter to select ", self.law_count);
        let items: Vec<ListItem> = self
            .laws
            .iter()
            .map(|law| ListItem::new(Line::styled(format!("  {law}"), dim)))
            .collect();

        let border_style = if self.focus == Focus::LawList {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            dim
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(Span::styled(
                        title,
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut state = self.law_list_state;
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn render_output_list(&self, frame: &mut Frame, area: Rect) {
        let dim = Style::default().add_modifier(Modifier::DIM);
        let items: Vec<ListItem> = self
            .current_law_info
            .as_ref()
            .map(|info| {
                info.outputs
                    .iter()
                    .map(|o| ListItem::new(Line::styled(format!("  {o}"), dim)))
                    .collect()
            })
            .unwrap_or_default();

        let border_style = if self.focus == Focus::OutputList {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            dim
        };

        let title = if self.current_law_info.is_none() {
            " Outputs — select a law first "
        } else {
            " Outputs — Enter to select "
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(Span::styled(
                        title,
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        let mut state = self.output_list_state;
        frame.render_stateful_widget(list, area, &mut state);
    }

    fn render_param_form(&self, frame: &mut Frame, area: Rect) {
        let is_focused = self.focus == Focus::ParamForm;
        let dim = Style::default().add_modifier(Modifier::DIM);
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let mut lines: Vec<Line> = Vec::new();

        lines.push(Line::styled(
            " Enter:evaluate  ↑↓:navigate  +/-:params",
            dim,
        ));

        for (i, param) in self.params.iter().enumerate() {
            let is_selected = is_focused && !self.editing_date && i == self.param_cursor;
            let cursor = if is_selected { "▸ " } else { "  " };

            if param.name.is_empty() {
                let style = if is_selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    dim
                };
                lines.push(Line::styled(format!("{cursor}(type name:value)"), style));
            } else if param.value.is_empty() {
                // Highlight: we're asking for this value
                lines.push(Line::from(vec![
                    Span::styled(cursor, if is_selected { bold } else { dim }),
                    Span::styled(format!("{}: ", param.name), bold),
                    Span::styled(
                        "▏",
                        if is_selected {
                            Style::default().add_modifier(Modifier::BOLD)
                        } else {
                            dim
                        },
                    ),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::styled(cursor, dim),
                    Span::styled(format!("{}: ", param.name), dim),
                    Span::styled(&param.value, if is_selected { bold } else { dim }),
                ]));
            }
        }

        // Date input
        let date_style = if is_focused && self.editing_date {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            dim
        };
        let date_cursor = if is_focused && self.editing_date {
            "▸ "
        } else {
            "  "
        };
        lines.push(Line::styled(
            format!("{date_cursor}date:{}", self.date_input),
            date_style,
        ));

        let border_style = if is_focused {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            dim
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(
                " Parameters ",
                Style::default().add_modifier(Modifier::BOLD),
            ));

        let paragraph = Paragraph::new(lines).block(block);
        frame.render_widget(paragraph, area);
    }

    fn render_result(&self, frame: &mut Frame, area: Rect) {
        let inner_height = area.height.saturating_sub(2) as usize;
        let dim = Style::default().add_modifier(Modifier::DIM);
        let bold = Style::default().add_modifier(Modifier::BOLD);
        let mut lines: Vec<Line> = Vec::new();

        if let Some(ref err) = self.error {
            if err.starts_with("Variable not found:") {
                lines.push(Line::styled(
                    format!("  ● {err} — fill in the value above, then Enter"),
                    bold,
                ));
            } else {
                lines.push(Line::styled(format!("  ✗ {err}"), bold));
            }
        } else if let Some(ref result) = self.result {
            lines.push(Line::styled(" Outputs:", bold));
            for (name, value) in &result.outputs {
                lines.push(Line::from(vec![
                    Span::styled(format!("  {name}: "), bold),
                    Span::styled(format_value(value), dim),
                ]));
            }

            if !result.resolved_inputs.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::styled(" Resolved Inputs:", bold));
                for (name, value) in &result.resolved_inputs {
                    lines.push(Line::from(vec![
                        Span::styled(format!("  {name}: "), dim),
                        Span::styled(format_value(value), dim),
                    ]));
                }
            }

            lines.push(Line::from(""));
            lines.push(Line::styled(
                format!(" art.{} of {}", result.article_number, result.law_id),
                dim,
            ));
        } else {
            lines.push(Line::from(""));
            lines.push(Line::styled(
                "  Select law → output → fill params → Enter",
                dim,
            ));
        }

        let visible: Vec<Line> = lines
            .into_iter()
            .skip(self.result_scroll)
            .take(inner_height)
            .collect();

        let border_style = if self.focus == Focus::Result {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            dim
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(Span::styled(
                " Result ",
                Style::default().add_modifier(Modifier::BOLD),
            ));

        let paragraph = Paragraph::new(visible)
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }
}

fn parse_value(s: &str) -> Value {
    if let Ok(i) = s.parse::<i64>() {
        return Value::Int(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        return Value::from(f);
    }
    match s.to_lowercase().as_str() {
        "true" | "ja" | "yes" => return Value::Bool(true),
        "false" | "nee" | "no" => return Value::Bool(false),
        "null" | "none" => return Value::Null,
        _ => {}
    }
    Value::String(s.to_string())
}

fn format_value(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Decimal(d) => format!("{d:.2}"),
        Value::String(s) => format!("\"{s}\""),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_value).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(obj) => {
            let items: Vec<String> = obj
                .iter()
                .map(|(k, v)| format!("{k}: {}", format_value(v)))
                .collect();
            format!("{{{}}}", items.join(", "))
        }
        Value::Untranslatable { article, construct } => {
            format!("UNTRANSLATABLE(art. {article}: {construct})")
        }
        // RFC-036: an unknown names the facts nobody supplied.
        Value::Unknown(_) => value.to_string(),
    }
}
