use regelrecht_engine::{ArticleResult, LawExecutionService, LawInfo, PathNode, Value};
use std::collections::BTreeMap;
use std::path::Path;
use tokio::sync::mpsc;
use walkdir::WalkDir;

/// Commands sent to the engine thread.
pub enum EngineCommand {
    GetLawInfo(String),
    Evaluate {
        law_id: String,
        output: String,
        params: BTreeMap<String, Value>,
        date: String,
    },
}

/// Responses from the engine thread.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum EngineResponse {
    Loaded {
        law_count: usize,
    },
    LawList(Vec<String>),
    LawInfo(Option<LawInfo>),
    EvalResult {
        result: Result<ArticleResult, String>,
        trace: Option<PathNode>,
    },
    /// The engine thread has panicked or exited unexpectedly.
    ThreadDied,
}

/// Handle to communicate with the engine thread.
pub struct EngineHandle {
    cmd_tx: std::sync::mpsc::Sender<EngineCommand>,
    resp_rx: mpsc::UnboundedReceiver<EngineResponse>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
    dead: bool,
}

impl EngineHandle {
    /// Spawn the engine thread and load all laws from the corpus.
    pub fn spawn(project_root: &Path) -> Self {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
        let (resp_tx, resp_rx) = mpsc::unbounded_channel();

        let corpus_root = project_root.to_path_buf();

        let thread_handle = std::thread::spawn(move || {
            engine_thread(corpus_root, cmd_rx, resp_tx);
        });

        Self {
            cmd_tx,
            resp_rx,
            thread_handle: Some(thread_handle),
            dead: false,
        }
    }

    pub fn send(&self, cmd: EngineCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn try_recv(&mut self) -> Option<EngineResponse> {
        // Check if the engine thread has died (panicked)
        if !self.dead
            && let Some(ref handle) = self.thread_handle
            && handle.is_finished()
        {
            self.dead = true;
            return Some(EngineResponse::ThreadDied);
        }
        self.resp_rx.try_recv().ok()
    }
}

fn engine_thread(
    project_root: std::path::PathBuf,
    cmd_rx: std::sync::mpsc::Receiver<EngineCommand>,
    resp_tx: mpsc::UnboundedSender<EngineResponse>,
) {
    let mut service = LawExecutionService::new();

    // Load all YAML files from corpus
    let law_count = load_corpus(&mut service, &project_root);
    let _ = resp_tx.send(EngineResponse::Loaded { law_count });

    // Send law list immediately after loading
    let laws: Vec<String> = service.list_laws().into_iter().map(String::from).collect();
    let _ = resp_tx.send(EngineResponse::LawList(laws));

    // Process commands
    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            EngineCommand::GetLawInfo(id) => {
                let info = service.get_law_info(&id);
                let _ = resp_tx.send(EngineResponse::LawInfo(info));
            }
            EngineCommand::Evaluate {
                law_id,
                output,
                params,
                date,
            } => {
                let result =
                    service.evaluate_law_output_with_trace(&law_id, &output, params, &date);
                let (result, trace) = match result {
                    Ok(mut r) => {
                        let trace = r.trace.take();
                        (Ok(r), trace)
                    }
                    Err(e) => (Err(e.to_string()), None),
                };
                let _ = resp_tx.send(EngineResponse::EvalResult { result, trace });
            }
        }
    }
}

fn load_corpus(service: &mut LawExecutionService, project_root: &Path) -> usize {
    let candidates = [
        project_root.join("corpus/regulation/nl"),
        project_root.join("corpus/regulation"),
        project_root.join("corpus/central/nl"),
        project_root.join("corpus/central"),
    ];

    let corpus_dir = candidates.iter().find(|p| p.is_dir());
    let corpus_dir = match corpus_dir {
        Some(d) => d,
        None => return 0,
    };

    let mut count = 0;
    for entry in WalkDir::new(corpus_dir).into_iter().flatten().filter(|e| {
        e.file_type().is_file()
            && e.path()
                .extension()
                .is_some_and(|ext| ext == "yaml" || ext == "yml")
    }) {
        if let Ok(content) = std::fs::read_to_string(entry.path()) {
            match service.load_law(&content) {
                Ok(_) => count += 1,
                Err(e) => {
                    tracing::warn!("Failed to load {}: {}", entry.path().display(), e);
                }
            }
        }
    }

    count
}
