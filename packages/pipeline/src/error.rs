use thiserror::Error;

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("job not found: {0}")]
    JobNotFound(uuid::Uuid),

    #[error("job {0} is not in processing state")]
    JobNotProcessing(uuid::Uuid),

    #[error("law not found: {0}")]
    LawNotFound(String),

    #[error("invalid state transition: {0}")]
    InvalidStateTransition(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("harvester error: {0}")]
    Harvester(#[from] regelrecht_harvester::HarvesterError),

    #[error("corpus error: {0}")]
    Corpus(#[from] regelrecht_corpus::CorpusError),

    #[error("enrichment error: {0}")]
    Enrich(String),

    /// De uploader gaf geen toestemming voor een taalmodel en de deterministische
    /// route leverde niets op, dus stopt de conversie. Een eigen variant en niet
    /// [`Self::Enrich`], om twee redenen:
    ///
    /// 1. De boodschap is al een afgeronde uitleg voor de gebruiker (ze belandt
    ///    in de "Conversie mislukt"-taak), dus zonder `enrichment error:`-prefix —
    ///    er is hier ook niets verrijkt.
    /// 2. Ze bevat de door de gebruiker gekozen bestandsnaam. De worker
    ///    classificeert mislukkingen op tekst (fork/EAGAIN/OOM-markers), en die
    ///    classificatie mag nooit op door een gebruiker te kiezen tekst
    ///    afgaan — zie `worker::document_convert_outcome`.
    #[error("{0}")]
    LlmNotPermitted(String),

    #[error(
        "base drift for {yaml_path}: base branch {base} changed (was {expected}, now {actual}); human review / re-enrich required"
    )]
    BaseDrift {
        yaml_path: String,
        base: String,
        expected: String,
        actual: String,
    },

    #[error("worker error: {0}")]
    Worker(String),

    #[error("task join error: {0}")]
    Join(#[from] tokio::task::JoinError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
}

pub type Result<T> = std::result::Result<T, PipelineError>;
