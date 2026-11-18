use crate::backend::engine_backend::EngineHandle;
use crate::backend::process_runner::ProcessRunner;
use crate::views::{
    actions::ActionsView, bdd::BddView, corpus::CorpusView, dashboard::DashboardView,
    deps::DepsView, engine::EngineView, logs::LogsView, pipeline::PipelineView, trace::TraceView,
    validation::ValidationView,
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    Bdd,
    Engine,
    Corpus,
    Pipeline,
    Validation,
    Trace,
    Dependencies,
    Logs,
    Actions,
}

impl Tab {
    pub const ALL: [Tab; 10] = [
        Tab::Dashboard,
        Tab::Bdd,
        Tab::Engine,
        Tab::Corpus,
        Tab::Pipeline,
        Tab::Validation,
        Tab::Trace,
        Tab::Dependencies,
        Tab::Logs,
        Tab::Actions,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Tab::Dashboard => "Home",
            Tab::Bdd => "BDD",
            Tab::Engine => "Eng",
            Tab::Corpus => "Corp",
            Tab::Pipeline => "Pipe",
            Tab::Validation => "Val",
            Tab::Trace => "Trace",
            Tab::Dependencies => "Deps",
            Tab::Logs => "Logs",
            Tab::Actions => "Act",
        }
    }

    /// Display key for this tab (0-9).
    pub fn key_label(&self) -> &'static str {
        match self {
            Tab::Dashboard => "0",
            Tab::Bdd => "1",
            Tab::Engine => "2",
            Tab::Corpus => "3",
            Tab::Pipeline => "4",
            Tab::Validation => "5",
            Tab::Trace => "6",
            Tab::Dependencies => "7",
            Tab::Logs => "8",
            Tab::Actions => "9",
        }
    }

    pub fn index(&self) -> usize {
        Tab::ALL.iter().position(|t| t == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> Option<Tab> {
        Tab::ALL.get(i).copied()
    }

    pub fn from_key(c: char) -> Option<Tab> {
        match c {
            '0' => Some(Tab::Dashboard),
            '1' => Some(Tab::Bdd),
            '2' => Some(Tab::Engine),
            '3' => Some(Tab::Corpus),
            '4' => Some(Tab::Pipeline),
            '5' => Some(Tab::Validation),
            '6' => Some(Tab::Trace),
            '7' => Some(Tab::Dependencies),
            '8' => Some(Tab::Logs),
            '9' => Some(Tab::Actions),
            _ => None,
        }
    }

    pub fn next(&self) -> Tab {
        let i = self.index();
        Tab::from_index((i + 1) % Tab::ALL.len()).unwrap_or(Tab::Dashboard)
    }

    pub fn prev(&self) -> Tab {
        let i = self.index();
        let len = Tab::ALL.len();
        Tab::from_index((i + len - 1) % len).unwrap_or(Tab::Actions)
    }
}

pub struct App {
    pub active_tab: Tab,
    pub should_quit: bool,
    pub show_help: bool,
    #[allow(dead_code)]
    pub project_root: PathBuf,
    pub process_runner: ProcessRunner,
    pub engine_handle: EngineHandle,
    pub repo_branch: String,
    pub corpus_branch: Option<String>,

    // Views
    pub dashboard: DashboardView,
    pub bdd: BddView,
    pub engine: EngineView,
    pub corpus: CorpusView,
    pub pipeline: PipelineView,
    pub validation: ValidationView,
    pub trace: TraceView,
    pub deps: DepsView,
    pub logs: LogsView,
    pub actions: ActionsView,
}

impl App {
    pub fn new() -> Self {
        let project_root = find_project_root();
        let repo_branch = git_branch(&project_root);
        let corpus_branch = find_corpus_dir(&project_root).and_then(|d| {
            let b = git_branch(&d);
            // Only show separately if it differs from repo branch
            if b != repo_branch { Some(b) } else { None }
        });
        let process_runner = ProcessRunner::new(project_root.clone());
        let engine_handle = EngineHandle::spawn(&project_root);

        let mut dashboard = DashboardView::new();
        dashboard.scan(&project_root);

        let mut corpus = CorpusView::new();
        corpus.scan(&project_root);

        let mut bdd = BddView::new();
        bdd.scan_features(&project_root);

        let mut validation = ValidationView::new();
        validation.scan_files(&project_root);

        let mut deps = DepsView::new();
        deps.scan_deps(&project_root);

        Self {
            active_tab: Tab::Dashboard,
            should_quit: false,
            show_help: false,
            project_root,
            process_runner,
            engine_handle,
            repo_branch,
            corpus_branch,
            dashboard,
            bdd,
            engine: EngineView::new(),
            corpus,
            pipeline: PipelineView::new(),
            validation,
            trace: TraceView::new(),
            deps,
            logs: LogsView::new(),
            actions: ActionsView::new(),
        }
    }
}

/// Walk up from CWD to find the project root (directory containing `justfile`).
fn find_project_root() -> PathBuf {
    let mut dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    loop {
        if dir.join("justfile").exists() {
            return dir;
        }
        if !dir.pop() {
            return std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        }
    }
}

/// Get the current git branch for a directory.
fn git_branch(dir: &Path) -> String {
    std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string())
}

/// Find the corpus regulation directory.
fn find_corpus_dir(project_root: &Path) -> Option<PathBuf> {
    let candidates = [
        project_root.join("corpus/regulation"),
        project_root.join("corpus/central"),
        project_root.join("corpus"),
    ];
    candidates.into_iter().find(|p| p.is_dir())
}
