use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, sqlx::Type,
)]
#[sqlx(type_name = "job_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum JobType {
    Harvest,
    Enrich,
    /// Convert an uploaded document (PDF/Word) to a markdown werkdocument via
    /// the LLM agent. Scoped to a traject, named in the job payload
    /// (`traject_ref`) rather than on [`Job`] itself.
    #[sqlx(rename = "document_convert")]
    #[serde(rename = "document_convert")]
    DocumentConvert,
    /// Convert an uploaded document (PDF/Word) to a harvested base-law YAML
    /// and chain a task-flow enrich job on it. Scoped to a traject.
    #[sqlx(rename = "law_convert")]
    #[serde(rename = "law_convert")]
    LawConvert,
    /// Harvest a law from BWB for one traject (task flow): download + parse
    /// the base-law YAML and chain a task-flow enrich job on it, exactly like
    /// [`JobType::LawConvert`] does after its conversion. Unlike
    /// [`JobType::Harvest`] this never touches the central corpus repo — the
    /// result travels as job blobs and lands via the review task's approve.
    #[sqlx(rename = "traject_harvest")]
    #[serde(rename = "traject_harvest")]
    TrajectHarvest,
}

impl JobType {
    /// Every variant, in declaration order. A consumer that wants "one bucket
    /// per job type" iterates this instead of writing the list out again, so a
    /// new variant lands in that consumer without anyone remembering to add it.
    pub const ALL: &'static [JobType] = &[
        JobType::Harvest,
        JobType::Enrich,
        JobType::DocumentConvert,
        JobType::LawConvert,
        JobType::TrajectHarvest,
    ];

    /// The name this variant carries in the database enum and on the wire.
    /// The match is exhaustive on purpose: a new variant does not compile
    /// until it is named here, and `job_type_names_match_serde` pins the
    /// names to the `serde`/`sqlx` renames.
    pub const fn as_str(&self) -> &'static str {
        match self {
            JobType::Harvest => "harvest",
            JobType::Enrich => "enrich",
            JobType::DocumentConvert => "document_convert",
            JobType::LawConvert => "law_convert",
            JobType::TrajectHarvest => "traject_harvest",
        }
    }

    /// Parse the database/wire name back into a variant.
    pub fn from_str_name(name: &str) -> Option<JobType> {
        JobType::ALL.iter().copied().find(|t| t.as_str() == name)
    }

    /// Whether this job type's `law_id` column actually names a law.
    ///
    /// [`JobType::DocumentConvert`] and [`JobType::LawConvert`] put a synthetic
    /// key there (`doc:{traject}/{path}` and `lawdoc:{traject}/{filename}`)
    /// because the real identity is only chosen by the conversion, so anything
    /// that counts or groups "per law" has to leave those two out (#1162).
    /// [`JobType::TrajectHarvest`] carries a BWB identifier rather than a
    /// corpus `$id`; that still denotes one law, so it stays in.
    pub fn law_id_names_a_law(&self) -> bool {
        match self {
            JobType::Harvest | JobType::Enrich | JobType::TrajectHarvest => true,
            JobType::DocumentConvert | JobType::LawConvert => false,
        }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    sqlx::Type,
    strum::EnumIter,
    strum::Display,
)]
#[sqlx(type_name = "job_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum JobStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    /// Stopped at the request of a member of its traject: a pending run was
    /// never started, a processing one had its agent killed. Terminal, never
    /// retried, and no task follows.
    Cancelled,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    sqlx::Type,
    strum::EnumIter,
    strum::Display,
)]
#[sqlx(type_name = "law_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "snake_case")]
pub enum LawStatusValue {
    Unknown,
    Queued,
    Harvesting,
    Harvested,
    #[sqlx(rename = "harvest_failed")]
    #[serde(rename = "harvest_failed")]
    HarvestFailed,
    #[sqlx(rename = "harvest_exhausted")]
    #[serde(rename = "harvest_exhausted")]
    HarvestExhausted,
    Enriching,
    Enriched,
    #[sqlx(rename = "enrich_failed")]
    #[serde(rename = "enrich_failed")]
    EnrichFailed,
    #[sqlx(rename = "enrich_exhausted")]
    #[serde(rename = "enrich_exhausted")]
    EnrichExhausted,
    /// No consolidated text is available to harvest (the work is withdrawn, not
    /// yet in force, or only announced). Terminal — the precise reason and date
    /// are recorded in the harvest job's result. Future laws can be re-harvested
    /// manually once their text appears.
    #[sqlx(rename = "not_harvestable")]
    #[serde(rename = "not_harvestable")]
    NotHarvestable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Priority(i32);

impl Priority {
    pub fn new(value: i32) -> Self {
        Self(value.clamp(0, 100))
    }

    pub fn value(self) -> i32 {
        self.0
    }
}

impl Default for Priority {
    fn default() -> Self {
        Self(50)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Job {
    pub id: Uuid,
    pub job_type: JobType,
    pub law_id: String,
    pub status: JobStatus,
    pub priority: i32,
    pub payload: Option<serde_json::Value>,
    pub result: Option<serde_json::Value>,
    pub progress: Option<serde_json::Value>,
    pub attempts: i32,
    pub max_attempts: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    /// Earliest moment the job may be claimed. `None` means claimable
    /// immediately; set by the retry-backoff logic in `fail_job`.
    pub scheduled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LawEntry {
    pub law_id: String,
    pub law_name: Option<String>,
    pub slug: Option<String>,
    pub status: LawStatusValue,
    pub harvest_job_id: Option<Uuid>,
    pub enrich_job_id: Option<Uuid>,
    /// Completeness metric: fraction of articles that received a
    /// `machine_readable` section during enrichment. This measures COVERAGE,
    /// not CORRECTNESS — a score of 1.0 means every article was modelled, not
    /// that the modelling is legally faithful. Correctness is checked elsewhere
    /// (schema/cross-law gates, BDD, and the methodological drift/desk-review).
    pub coverage_score: Option<f64>,
    pub harvest_fail_count: i32,
    pub enrich_fail_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single untranslatable construct captured during enrichment (RFC-012),
/// one row per (law, provider, article, construct). Mirrors the
/// `untranslatables` table; refreshed per (law_id, provider) on each enrich.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Untranslatable {
    pub id: Uuid,
    pub law_id: String,
    pub enrich_job_id: Uuid,
    pub provider: String,
    pub article: String,
    pub construct: String,
    pub reason: String,
    pub suggestion: Option<String>,
    pub legal_text_excerpt: Option<String>,
    pub accepted: bool,
    pub created_at: DateTime<Utc>,
}

/// A single marking captured during enrichment (schema v0.7.0), one row per
/// (law, provider, article, construct). Mirrors the `markings` table;
/// refreshed per (law_id, provider) on each enrich.
///
/// The successor of [`Untranslatable`]. Both exist at once because a law
/// pinned to schema v0.5.x still carries the old field and the engine still
/// reads it, so the UI shows the two channels merged rather than making a
/// reader work out which schema version a law happens to be on.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Marking {
    pub id: Uuid,
    pub law_id: String,
    pub enrich_job_id: Uuid,
    pub provider: String,
    pub article: String,
    /// The construct the format cannot express, in the article's own words.
    pub about: String,
    /// `operation` (the operation must be built) or `model` (the format has no
    /// shape for this construct). Constrained in the database.
    pub resolution: String,
    /// The change that would resolve it. Nullable because the capture struct
    /// carries it as an `Option`, unlike the schema, which requires it.
    pub resolved_by: Option<String>,
    /// The values this article cannot produce. Empty means the article stays
    /// executable, which is a claim rather than a missing value.
    pub target: Vec<String>,
    pub legal_text_excerpt: String,
    pub accepted: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FeatureFlag {
    pub key: String,
    pub enabled: bool,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `as_str` must agree with the `serde` rename, because the dashboard keys
    /// its per-type buckets on one and the frontend reads the other.
    #[test]
    fn job_type_names_match_serde() {
        for job_type in JobType::ALL {
            let via_serde = serde_json::to_value(job_type).expect("JobType serializes");
            assert_eq!(
                via_serde.as_str(),
                Some(job_type.as_str()),
                "as_str and the serde rename disagree for {job_type:?}"
            );
            assert_eq!(JobType::from_str_name(job_type.as_str()), Some(*job_type));
        }
    }

    /// `ALL` must list every variant. Without this the list can silently lag a
    /// new variant, which is exactly the failure the dashboard had (#1161).
    #[test]
    fn job_type_all_covers_every_variant() {
        // Exhaustive match: adding a variant breaks compilation here first.
        for job_type in JobType::ALL {
            match job_type {
                JobType::Harvest
                | JobType::Enrich
                | JobType::DocumentConvert
                | JobType::LawConvert
                | JobType::TrajectHarvest => {}
            }
        }
        let mut names: Vec<&str> = JobType::ALL.iter().map(|t| t.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), JobType::ALL.len(), "duplicate name in ALL");
        assert_eq!(
            JobType::ALL.len(),
            5,
            "a job type was added or removed; check every per-type consumer \
             (admin dashboard, metrics, law-scoped counts) before bumping this"
        );
    }
}
