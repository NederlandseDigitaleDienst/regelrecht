use crate::backend::corpus_scanner;
use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use ratatui::widgets::{Paragraph, Wrap};
use std::collections::HashMap;
use std::path::Path;

pub struct DashboardView {
    stats: DashboardStats,
    scanned: bool,
}

struct DashboardStats {
    law_count: usize,
    article_count: usize,
    #[allow(dead_code)]
    output_count: usize,
    feature_count: usize,
    scenario_count: usize,
    laws_by_layer: Vec<(String, usize)>,
    most_referenced: Vec<(String, usize)>,
    most_implementing: Vec<(String, usize)>,
}

impl DashboardView {
    pub fn new() -> Self {
        Self {
            stats: DashboardStats {
                law_count: 0,
                article_count: 0,
                output_count: 0,
                feature_count: 0,
                scenario_count: 0,
                laws_by_layer: Vec::new(),
                most_referenced: Vec::new(),
                most_implementing: Vec::new(),
            },
            scanned: false,
        }
    }

    pub fn scan(&mut self, project_root: &Path) {
        let yaml_files = corpus_scanner::corpus_yaml_files(project_root);

        let mut layer_counts: HashMap<String, usize> = HashMap::new();
        let mut ref_counts: HashMap<String, usize> = HashMap::new();
        let mut impl_counts: HashMap<String, usize> = HashMap::new();
        let mut total_articles = 0;
        let mut total_outputs = 0;

        for path in &yaml_files {
            if let Ok(content) = std::fs::read_to_string(path) {
                let meta = corpus_scanner::extract_metadata(&content);

                if let Some(ref layer) = meta.regulatory_layer {
                    *layer_counts.entry(layer.clone()).or_default() += 1;
                }

                total_articles += meta.article_count;

                // Count outputs (lines matching "output:" pattern under machine_readable)
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("- name:") {
                        // Could be output or input — rough count
                    }
                    if trimmed.starts_with("output:") && !trimmed.contains("regulation") {
                        total_outputs += 1;
                    }
                }

                // Count references to this law
                for line in content.lines() {
                    let trimmed = line.trim();
                    if let Some(val) = trimmed.strip_prefix("regulation:") {
                        let val = val.trim().trim_matches('\'').trim_matches('"');
                        if !val.is_empty() {
                            *ref_counts.entry(val.to_string()).or_default() += 1;
                        }
                    }
                    if let Some(val) = trimmed
                        .strip_prefix("- law:")
                        .or_else(|| trimmed.strip_prefix("law:"))
                    {
                        let val = val.trim().trim_matches('\'').trim_matches('"');
                        if !val.is_empty() {
                            *impl_counts.entry(val.to_string()).or_default() += 1;
                        }
                    }
                }
            }
        }

        // Count features and scenarios
        let features_dir = project_root.join("features");
        let mut feature_count = 0;
        let mut scenario_count = 0;
        if features_dir.is_dir()
            && let Ok(entries) = std::fs::read_dir(&features_dir)
        {
            for entry in entries.flatten() {
                if entry.path().extension().is_some_and(|ext| ext == "feature") {
                    feature_count += 1;
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        scenario_count += content
                            .lines()
                            .filter(|l| {
                                let t = l.trim();
                                t.starts_with("Scenario:") || t.starts_with("Scenario Outline:")
                            })
                            .count();
                    }
                }
            }
        }

        // Sort and take top items
        let mut laws_by_layer: Vec<(String, usize)> = layer_counts.into_iter().collect();
        laws_by_layer.sort_by_key(|b| std::cmp::Reverse(b.1));

        let mut most_referenced: Vec<(String, usize)> = ref_counts.into_iter().collect();
        most_referenced.sort_by_key(|b| std::cmp::Reverse(b.1));
        most_referenced.truncate(5);

        let mut most_implementing: Vec<(String, usize)> = impl_counts.into_iter().collect();
        most_implementing.sort_by_key(|b| std::cmp::Reverse(b.1));
        most_implementing.truncate(5);

        self.stats = DashboardStats {
            law_count: yaml_files.len(),
            article_count: total_articles,
            output_count: total_outputs,
            feature_count,
            scenario_count,
            laws_by_layer,
            most_referenced,
            most_implementing,
        };
        self.scanned = true;
    }

    pub fn handle_key(&mut self, _key: KeyEvent) {
        // Dashboard is read-only
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let dim = Style::default().add_modifier(Modifier::DIM);
        let bold = Style::default().add_modifier(Modifier::BOLD);

        let mut lines: Vec<Line> = Vec::new();

        // Header
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  RegelRecht", bold),
            Span::styled(" — Machine-Readable Dutch Law", dim),
        ]));
        lines.push(Line::from(""));

        // Stats grid
        lines.push(Line::styled("  Overview", bold));
        lines.push(Line::from(vec![
            Span::styled("    Laws:       ", dim),
            Span::styled(format!("{}", self.stats.law_count), bold),
        ]));
        lines.push(Line::from(vec![
            Span::styled("    Articles:   ", dim),
            Span::styled(format!("{}", self.stats.article_count), bold),
        ]));
        lines.push(Line::from(vec![
            Span::styled("    Features:   ", dim),
            Span::styled(format!("{}", self.stats.feature_count), bold),
        ]));
        lines.push(Line::from(vec![
            Span::styled("    Scenarios:  ", dim),
            Span::styled(format!("{}", self.stats.scenario_count), bold),
        ]));
        lines.push(Line::from(""));

        // Laws by layer
        if !self.stats.laws_by_layer.is_empty() {
            lines.push(Line::styled("  Laws by type", bold));
            for (layer, count) in &self.stats.laws_by_layer {
                let bar = "█".repeat((*count).min(20));
                lines.push(Line::from(vec![
                    Span::styled(format!("    {layer:<30} "), dim),
                    Span::styled(format!("{count:>3} "), bold),
                    Span::styled(bar, dim),
                ]));
            }
            lines.push(Line::from(""));
        }

        // Most referenced
        if !self.stats.most_referenced.is_empty() {
            lines.push(Line::styled("  Most referenced laws", bold));
            for (law_id, count) in &self.stats.most_referenced {
                lines.push(Line::from(vec![
                    Span::styled(format!("    {law_id:<30} "), dim),
                    Span::styled(format!("{count} refs"), bold),
                ]));
            }
            lines.push(Line::from(""));
        }

        // Most implementing
        if !self.stats.most_implementing.is_empty() {
            lines.push(Line::styled("  Laws with most implementations", bold));
            for (law_id, count) in &self.stats.most_implementing {
                lines.push(Line::from(vec![
                    Span::styled(format!("    {law_id:<30} "), dim),
                    Span::styled(format!("{count} impls"), bold),
                ]));
            }
        }

        let block = super::common::titled_block(" Dashboard ");

        let paragraph = Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }
}
