//! Law-convert jobs: turn an uploaded PDF/Word document into a harvested
//! base-law YAML (a law without `machine_readable`), then chain a task-flow
//! enrich job on it.
//!
//! The conversion mirrors [`crate::document_convert`]: known formats are first
//! reduced to text deterministically (`pandoc`/`pdftotext`), after which the
//! LLM agent structures that text into a schema-conformant base-law YAML —
//! choosing the `regulatory_layer` itself, because the upload can be anything
//! from a formal wet to uitvoeringsbeleid or a werkinstructie. The produced
//! YAML is validated in Rust against the JSON schema, with one repair round.
//! On success the worker does NOT create a review task itself: it enqueues a
//! task-flow enrich job with the base YAML as input blob, and the single
//! review task the user sees comes from the existing enrich task flow (see
//! `worker::finish_enrich_task_job`, steered by `EnrichPayload::new_law`).

use std::path::Path;

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use regelrecht_shared::RegulatoryLayer;

use crate::document_convert::{Upload, extension_for, try_deterministic_convert};
use crate::enrich::{EnrichConfig, EnrichPayload, SessionAction, run_llm_subprocess};
use crate::error::{PipelineError, Result};
use crate::job_queue::{self, CreateJobRequest};
use crate::models::{Job, JobType, Priority};
use crate::tasks::{self, BlobKind};

/// Prioriteit van de geketende enrich-job: gelijk aan de interactieve
/// enrich-op-aanvraag (task_requests.rs), want ook hier wacht een mens.
const CHAINED_ENRICH_PRIORITY: i32 = 80;

/// Marker in de dedup-fout van [`chain_enrich_and_complete`] ("er loopt al
/// een verrijking …"). De worker classificeert de fout hiermee als
/// deterministisch (terminaal, geen retry) — door de gedeelde const kan de
/// formulering niet stilletjes uit de pas lopen met de matcher in `worker.rs`.
pub(crate) const ENRICH_IN_PROGRESS_MARKER: &str = "er loopt al een verrijking";

/// Payload carried by a `law_convert` job. The raw bytes live in
/// `document_uploads` (referenced by `upload_id`), same as document-convert.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LawConvertPayload {
    /// Row id in `document_uploads` holding the uploaded bytes.
    pub upload_id: Uuid,
    /// Owning traject's id (feeds the chained enrich payload + failure task).
    pub traject_id: Uuid,
    /// Owning traject ref (also mirrored onto `jobs.traject_ref`).
    pub traject_ref: String,
    /// Original uploaded filename. Only for display: failure-task titles and
    /// the "Bezig"-sectie of the takenpaneel — the slug/$id is chosen by the
    /// conversion itself.
    pub filename: String,
    /// LLM provider override (`"claude"` / `"opencode"`); `None` uses the
    /// worker default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Account dat de upload deed; wordt de assignee van de uiteindelijke
    /// review-taak (via de geketende enrich-job) en van een `job_failed`-taak.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_by: Option<Uuid>,
    /// `"task"` ⇒ taak-flow. Law-convert kent alléén de taak-flow (er is geen
    /// direct-push-pad); het veld bestaat zodat de generieke
    /// `list_running_task_jobs_for_account`-query dit jobtype meepakt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deliver: Option<String>,
}

impl LawConvertPayload {
    /// Taak-flow: resultaat via de geketende enrich-taak i.p.v. een push.
    pub fn deliver_as_task(&self) -> bool {
        self.deliver.as_deref() == Some("task")
    }
}

/// Metadata extracted from a validated base-law YAML. Also consumed by
/// editor-api's law-create endpoint, which derives the corpus path from it —
/// keep validation and extraction in this one place.
#[derive(Debug, Clone)]
pub struct ValidatedLawMeta {
    pub law_id: String,
    pub regulatory_layer: RegulatoryLayer,
    pub valid_from: chrono::NaiveDate,
}

/// A validated, freshly-generated base law.
#[derive(Debug)]
pub struct GeneratedLaw {
    pub yaml: String,
    pub meta: ValidatedLawMeta,
}

/// Abstraction over the LLM structuring step so the orchestration can be
/// unit-tested without spawning a subprocess (mirrors
/// [`crate::document_convert::DocumentConverter`]).
#[async_trait::async_trait]
pub trait LawStructurer: Send + Sync {
    /// Run the agent with `prompt` in `work_dir`; the prompt names the file
    /// the agent must write its YAML to. `allow_bash` widens the toolset with
    /// een shell — alléén nodig wanneer de agent het rauwe bestand zelf moet
    /// converteren (de fallback zonder deterministische extractie).
    async fn structure(
        &self,
        prompt: &str,
        work_dir: &Path,
        config: &EnrichConfig,
        allow_bash: bool,
    ) -> Result<()>;
}

/// Production structurer: the enrich LLM subprocess. Shell-toegang is
/// least-privilege: de structurering zelf is Read/Write-werk op tekst; alleen
/// de rauwe-bestand-fallback krijgt Bash (voor `pandoc`/`pdftotext`). De
/// agent-lane mét Bash bleek op de kale Alpine-worker bovendien onbetrouwbaar
/// (zie de document-convert-historie), dus de gewone route vermijdt hem.
pub struct LlmLawStructurer;

#[async_trait::async_trait]
impl LawStructurer for LlmLawStructurer {
    async fn structure(
        &self,
        prompt: &str,
        work_dir: &Path,
        config: &EnrichConfig,
        allow_bash: bool,
    ) -> Result<()> {
        // Structuring a law is one call of its own; no window shares it.
        run_llm_subprocess(
            &config.provider,
            prompt,
            None,
            work_dir,
            config,
            // Nothing extra withheld: `allow_bash` already decides the shell,
            // and this lane reads no skill that could ask for more.
            crate::enrich::ToolPolicy {
                allow_bash,
                deny: &[],
            },
            SessionAction::Cold,
        )
        .await
        .map(|_| ())
    }
}

/// Name of the YAML file the agent must produce in its working directory.
const OUTPUT_FILE: &str = "law.yaml";
/// Name of the deterministically extracted text, when extraction succeeded.
const SOURCE_TEXT_FILE: &str = "source.md";

/// `$schema` URL the generated law must carry. Shared with the harvester so the
/// two writers cannot stamp different contracts into one corpus (#1167); the
/// enrichment that follows works on the same version.
use regelrecht_shared::SCHEMA_URL;

/// Sanitize the uploaded filename into something safe for the synthetic
/// `upload://` source-URL (schema `url` is `format: uri`, so no spaces).
fn sanitize_for_url(filename: &str) -> String {
    let cleaned: String = filename
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "document".to_string()
    } else {
        cleaned
    }
}

/// Build the structuring prompt. `source_file` is the file the agent reads:
/// the deterministically extracted text when available, otherwise the raw
/// upload (and the agent converts it itself, mirroring document-convert's
/// agentic fallback).
fn build_structure_prompt(source_file: &str, source_is_text: bool, source_url: &str) -> String {
    let read_step = if source_is_text {
        format!(
            "1. Read `{source_file}` in your current working directory: the extracted text of \
             the source document."
        )
    } else {
        format!(
            "1. The file `{source_file}` in your current working directory is the raw uploaded \
             document. First convert it to text yourself — the tools `pandoc` (Word/HTML/… → \
             Markdown) and `pdftotext` (PDF → text) are installed; do NOT rely on network \
             access. If no tool fits, read and transcribe it directly."
        )
    };
    let layers = [
        ("GRONDWET", "constitutional law"),
        ("WET", "a formal law (wet in formele zin)"),
        ("AMVB", "algemene maatregel van bestuur"),
        ("KONINKLIJK_BESLUIT", "koninklijk besluit"),
        ("MINISTERIELE_REGELING", "ministeriële regeling"),
        (
            "BELEIDSREGEL",
            "beleidsregel (policy rule of an bestuursorgaan)",
        ),
        (
            "UITVOERINGSBELEID",
            "uitvoeringsbeleid, werkinstructies or other internal implementation policy of a \
             ministry or organisation — the default for internal documents",
        ),
        ("EU_VERORDENING", "EU regulation"),
        ("EU_RICHTLIJN", "EU directive"),
        ("VERDRAG", "international treaty"),
        ("GEMEENTELIJKE_VERORDENING", "municipal ordinance"),
        ("PROVINCIALE_VERORDENING", "provincial ordinance"),
        ("WATERSCHAPS_VERORDENING", "water board ordinance"),
    ]
    .into_iter()
    .map(|(k, v)| format!("   - `{k}`: {v}"))
    .collect::<Vec<_>>()
    .join("\n");
    let today = regelrecht_shared::dates::today_str();
    format!(
        "You are converting a source document into a regelrecht base-law YAML file — the same \
         article-based format the BWB harvester produces. The document is not necessarily a \
         formal law: it can also be ministry policy, werkinstructies, or other regulations.\n\n\
         {read_step}\n\
         2. Determine the regulatory level of the document and pick the matching \
         `regulatory_layer` value:\n{layers}\n\
         3. Write a YAML document with exactly this top-level structure (base law only — do \
         NOT add `machine_readable`; that is a later, separate step):\n\
         ```yaml\n\
         ---\n\
         $schema: {SCHEMA_URL}\n\
         $id: <snake_case slug of the document title, only [a-z0-9_]>\n\
         regulatory_layer: <value from step 2>\n\
         publication_date: '<YYYY-MM-DD from the document; if unknown, use the valid_from date>'\n\
         valid_from: '<YYYY-MM-DD the rules take effect per the document; if unknown, use {today}>'\n\
         url: {source_url}\n\
         articles:\n\
           - number: '<article/section number>'\n\
             text: >-\n\
               <the verbatim text of the article>\n\
             url: {source_url}#<number>\n\
         ```\n\
         Optionally add a top-level `preamble` (string) for introductory text that precedes \
         the articles, and `officiele_titel`/`organisation` when the document states them.\n\
         4. Articles: when the document has numbered articles or sections, keep that numbering \
         (as strings, e.g. '1', '2a'). Otherwise split the document into logical sections and \
         number them '1', '2', …. Transcribe the text of each article verbatim — do NOT \
         summarize, translate, add commentary, or omit content.\n\
         5. Write ONLY the YAML document to a file named `{OUTPUT_FILE}` in your current \
         working directory — no surrounding code fences, no explanations.\n\n\
         The final deliverable is the file `{OUTPUT_FILE}`."
    )
}

/// Build the repair prompt for the second (and final) agent run after schema
/// validation failed. The invalid `law.yaml` is still in the workdir.
fn build_repair_prompt(errors: &[String]) -> String {
    let list = errors
        .iter()
        .map(|e| format!("- {e}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "The file `{OUTPUT_FILE}` you produced does not validate against the regelrecht JSON \
         schema. Fix `{OUTPUT_FILE}` in place so it validates, changing as little as possible \
         and keeping all article text verbatim. The validation errors:\n{list}\n\n\
         Write the corrected YAML to `{OUTPUT_FILE}` (overwrite it). Write ONLY the YAML \
         document — no code fences, no explanations."
    )
}

/// Validate generated base-law YAML: JSON-schema plus the invariants the rest
/// of the chain relies on ($id slug shape, parseable regulatory_layer, a real
/// `valid_from` date). Returns the extracted identity on success, or the full
/// error list (for the repair prompt / a 400 body) on failure.
pub fn validate_law_yaml(yaml: &str) -> std::result::Result<ValidatedLawMeta, Vec<String>> {
    let value: serde_json::Value = match serde_yaml_ng::from_str(yaml) {
        Ok(v) => v,
        Err(e) => return Err(vec![format!("YAML parse error: {e}")]),
    };

    let mut errors = Vec::new();

    match regelrecht_engine::schema::detect_version(&value) {
        Some(version) => match regelrecht_engine::schema::validation_errors_for(version, &value) {
            Ok(schema_errors) => errors.extend(schema_errors),
            Err(e) => errors.push(format!("schema validation failed: {e}")),
        },
        None => errors.push(format!(
            "$schema: missing or unknown schema version (use exactly `{SCHEMA_URL}`)"
        )),
    }

    let law_id = value
        .get("$id")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let slug_ok = !law_id.is_empty()
        && law_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !slug_ok {
        errors.push(format!(
            "$id: must be a non-empty snake_case slug matching [a-z0-9_]+ (got `{law_id}`)"
        ));
    }

    let layer: Option<RegulatoryLayer> = value
        .get("regulatory_layer")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    if layer.is_none() {
        errors.push("regulatory_layer: missing or not a known layer value".to_string());
    }

    let valid_from = value
        .get("valid_from")
        .and_then(|v| v.as_str())
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    if valid_from.is_none() {
        errors.push("valid_from: missing or not a YYYY-MM-DD date".to_string());
    }

    match (layer, valid_from) {
        (Some(regulatory_layer), Some(valid_from)) if errors.is_empty() => Ok(ValidatedLawMeta {
            law_id,
            regulatory_layer,
            valid_from,
        }),
        _ => Err(errors),
    }
}

/// Load the upload, run the conversion in a fresh working directory, validate
/// (with one repair round), and return the generated law. The caller (worker)
/// chains the enrich job and cleans up the upload row + job.
pub async fn execute_law_convert(
    pool: &PgPool,
    payload: &LawConvertPayload,
    config: &EnrichConfig,
    structurer: &dyn LawStructurer,
) -> Result<GeneratedLaw> {
    let upload = crate::document_convert::load_upload(pool, payload.upload_id).await?;
    let work_dir = std::env::temp_dir().join(format!("lawconvert-{}", payload.upload_id));
    // Clear any stale directory left by a previous attempt of this same job.
    let _ = tokio::fs::remove_dir_all(&work_dir).await;
    tokio::fs::create_dir_all(&work_dir).await?;

    let result = convert_law_in_dir(&work_dir, &upload, config, structurer).await;

    // Always remove the working directory, success or failure.
    let _ = tokio::fs::remove_dir_all(&work_dir).await;
    result
}

/// The filesystem half of the conversion, split out so it can be unit-tested
/// with a fake structurer and a synthetic upload (no DB, no LLM).
pub(crate) async fn convert_law_in_dir(
    work_dir: &Path,
    upload: &Upload,
    config: &EnrichConfig,
    structurer: &dyn LawStructurer,
) -> Result<GeneratedLaw> {
    let ext = extension_for(&upload.filename, &upload.content_type);
    let input_file = work_dir.join(format!("input.{ext}"));
    tokio::fs::write(&input_file, &upload.bytes).await?;

    // Deterministic-first text extraction, same rationale as document-convert:
    // reliable, offline, and it keeps the untrusted binary away from the
    // Bash-enabled LLM where the tools can handle it.
    let source_text_path = work_dir.join(SOURCE_TEXT_FILE);
    let extracted = try_deterministic_convert(&input_file, &ext, &source_text_path)
        .await
        .is_some();
    let (source_file, source_is_text) = if extracted {
        (SOURCE_TEXT_FILE.to_string(), true)
    } else {
        (format!("input.{ext}"), false)
    };

    let source_url = format!("upload://{}", sanitize_for_url(&upload.filename));
    let prompt = build_structure_prompt(&source_file, source_is_text, &source_url);
    // Bash alléén wanneer de agent het rauwe bestand nog zelf moet
    // converteren; op al-geëxtraheerde tekst is de structurering puur
    // Read/Write-werk (least privilege, en de Bash-lane is op de kale
    // Alpine-worker onbetrouwbaar gebleken).
    structurer
        .structure(&prompt, work_dir, config, !source_is_text)
        .await?;

    let output_path = work_dir.join(OUTPUT_FILE);
    let yaml = read_output(&output_path).await?;
    match validate_law_yaml(&yaml) {
        Ok(meta) => Ok(GeneratedLaw { yaml, meta }),
        Err(errors) => {
            // One repair round: feed the validation errors back to the agent.
            // A second failure is deterministic content trouble — the job is
            // single-attempt, so this surfaces as a job_failed task.
            tracing::info!(
                errors = errors.len(),
                "generated law YAML invalid, running repair round"
            );
            let repair = build_repair_prompt(&errors);
            // De reparatieronde bewerkt alleen nog `law.yaml`: nooit Bash.
            structurer
                .structure(&repair, work_dir, config, false)
                .await?;
            let yaml = read_output(&output_path).await?;
            match validate_law_yaml(&yaml) {
                Ok(meta) => Ok(GeneratedLaw { yaml, meta }),
                Err(errors) => Err(PipelineError::Enrich(format!(
                    "generated law YAML still invalid after repair round: {}",
                    errors.join("; ")
                ))),
            }
        }
    }
}

async fn read_output(output_path: &Path) -> Result<String> {
    let yaml = tokio::fs::read_to_string(output_path).await.map_err(|e| {
        PipelineError::Enrich(format!(
            "conversion produced no readable YAML at {}: {e}",
            output_path.display()
        ))
    })?;
    if yaml.trim().is_empty() {
        return Err(PipelineError::Enrich(
            "conversion produced empty YAML".to_string(),
        ));
    }
    Ok(yaml)
}

/// Context for chaining a task-flow enrich job onto the job that produced a
/// base law. Shared by `law_convert` (document → wet) and `traject_harvest`
/// (BWB → wet): both end in the same "enrich as review task" chain.
#[derive(Debug, Clone)]
pub struct EnrichChainContext {
    pub provider: Option<String>,
    pub requested_by: Option<Uuid>,
    pub traject_id: Uuid,
    pub traject_ref: String,
}

/// Chain step, in one transaction: enqueue the task-flow enrich job with the
/// generated base YAML as input blob, and complete the producing job. The
/// single review task the user sees is created later by the enrich task flow
/// (`worker::finish_enrich_task_job`, with `kind: "law_create"`).
pub async fn chain_enrich_and_complete(
    pool: &PgPool,
    source_job: &Job,
    ctx: &EnrichChainContext,
    law: &GeneratedLaw,
) -> Result<()> {
    // Same synthetic repo-relative path convention as `request_enrich` in
    // editor-api: the parent directory name doubles as slug for the
    // feature-file detection in execute_enrich.
    let yaml_path = format!("laws/{}/law.yaml", law.meta.law_id);
    let enrich_payload = EnrichPayload {
        pass: Default::default(),
        law_id: law.meta.law_id.clone(),
        yaml_path: yaml_path.clone(),
        provider: ctx.provider.clone(),
        depth: None,
        requested_by: ctx.requested_by,
        deliver: Some("task".to_string()),
        traject_id: Some(ctx.traject_id),
        traject_ref: Some(ctx.traject_ref.clone()),
        source_etag: None,
        new_law: Some(true),
        chunk_articles: None,
        skip_mvt: None,
        // Queue payload: the session belongs to the run, not to the row.
        session: None,
    };
    let payload_json = serde_json::to_value(&enrich_payload)
        .map_err(|e| PipelineError::Enrich(format!("serialize enrich payload: {e}")))?;

    let mut tx = pool.begin().await?;
    let req = CreateJobRequest::new(JobType::Enrich, &law.meta.law_id)
        .with_traject_ref(&ctx.traject_ref)
        .with_priority(Priority::new(CHAINED_ENRICH_PRIORITY))
        .with_payload(payload_json)
        .with_max_attempts(3);
    let enrich_job = job_queue::create_enrich_job_if_not_exists(&mut *tx, req).await?;
    let Some(enrich_job) = enrich_job else {
        // Er loopt al een actieve enrich voor deze (slug, provider, traject) —
        // kan alleen bij een tweede aanvraag die op dezelfde slug uitkomt.
        // Rollback (impliciet) en laat de aanroeper dit als terminale fout met
        // job_failed-taak afhandelen.
        return Err(PipelineError::Enrich(format!(
            "{ENRICH_IN_PROGRESS_MARKER} voor regelwerk '{}' in dit traject; \
             het nieuwe regelwerk is niet aangemaakt",
            law.meta.law_id
        )));
    };
    tasks::insert_blob(
        &mut *tx,
        enrich_job.id,
        BlobKind::Input,
        &yaml_path,
        &law.yaml,
    )
    .await?;
    job_queue::complete_job(
        &mut *tx,
        source_job.id,
        Some(serde_json::json!({ "law_id": law.meta.law_id })),
    )
    .await?;
    tx.commit().await?;
    tracing::info!(
        source_job = %source_job.id,
        source_job_type = ?source_job.job_type,
        enrich_job = %enrich_job.id,
        law_id = %law.meta.law_id,
        layer = law.meta.regulatory_layer.as_str(),
        "chained into enrich task job"
    );
    Ok(())
}

/// Chain step for law-convert; see [`chain_enrich_and_complete`].
pub async fn finish_law_convert_job(
    pool: &PgPool,
    convert_job: &Job,
    payload: &LawConvertPayload,
    law: &GeneratedLaw,
) -> Result<()> {
    let ctx = EnrichChainContext {
        provider: payload.provider.clone(),
        requested_by: payload.requested_by,
        traject_id: payload.traject_id,
        traject_ref: payload.traject_ref.clone(),
    };
    chain_enrich_and_complete(pool, convert_job, &ctx, law).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enrich::LlmProvider;

    fn test_config() -> EnrichConfig {
        EnrichConfig::for_test(LlmProvider::Claude {
            path: "claude".into(),
            model: None,
        })
    }

    fn valid_yaml() -> String {
        // Regel-voor-regel gejoined: een `\`-continuation in een gewone string
        // stript leidende spaties en zou de YAML-indentatie slopen.
        [
            "---".to_string(),
            format!("$schema: {SCHEMA_URL}"),
            "$id: werkinstructie_toetsing".to_string(),
            "regulatory_layer: UITVOERINGSBELEID".to_string(),
            "publication_date: '2026-01-15'".to_string(),
            "valid_from: '2026-02-01'".to_string(),
            "url: upload://werkinstructie.pdf".to_string(),
            "articles:".to_string(),
            "  - number: '1'".to_string(),
            "    text: De aanvraag wordt binnen acht weken beoordeeld.".to_string(),
            "    url: 'upload://werkinstructie.pdf#1'".to_string(),
        ]
        .join("\n")
            + "\n"
    }

    #[test]
    fn payload_roundtrips_and_backcompat() {
        let account = Uuid::new_v4();
        let payload = LawConvertPayload {
            upload_id: Uuid::nil(),
            traject_id: Uuid::nil(),
            traject_ref: "abcd1234".to_string(),
            filename: "beleid.pdf".to_string(),
            provider: Some("claude".to_string()),
            requested_by: Some(account),
            deliver: Some("task".to_string()),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["filename"], "beleid.pdf");
        let back: LawConvertPayload = serde_json::from_value(json).unwrap();
        assert_eq!(back, payload);
        assert!(back.deliver_as_task());
    }

    #[test]
    fn validate_accepts_valid_law() {
        let meta = validate_law_yaml(&valid_yaml()).unwrap();
        assert_eq!(meta.law_id, "werkinstructie_toetsing");
        assert_eq!(meta.regulatory_layer, RegulatoryLayer::Uitvoeringsbeleid);
        assert_eq!(meta.valid_from.to_string(), "2026-02-01");
    }

    #[test]
    fn validate_rejects_bad_slug_layer_and_date() {
        let yaml = [
            "---".to_string(),
            format!("$schema: {SCHEMA_URL}"),
            "$id: Foute Slug".to_string(),
            "regulatory_layer: NOTULEN".to_string(),
            "publication_date: '2026-01-15'".to_string(),
            "valid_from: binnenkort".to_string(),
            "url: upload://x.pdf".to_string(),
            "articles:".to_string(),
            "  - number: '1'".to_string(),
            "    text: t".to_string(),
            "    url: 'upload://x.pdf#1'".to_string(),
        ]
        .join("\n");
        let errors = validate_law_yaml(&yaml).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("$id")));
        assert!(errors.iter().any(|e| e.contains("regulatory_layer")));
        assert!(errors.iter().any(|e| e.contains("valid_from")));
    }

    #[test]
    fn validate_rejects_unknown_schema() {
        let yaml = "---\n$id: x\narticles: []\n";
        let errors = validate_law_yaml(yaml).unwrap_err();
        assert!(errors.iter().any(|e| e.contains("$schema")));
    }

    #[test]
    fn prompt_names_source_and_output() {
        let prompt = build_structure_prompt("source.md", true, "upload://beleid.pdf");
        assert!(prompt.contains("`source.md`"));
        assert!(prompt.contains("`law.yaml`"));
        assert!(prompt.contains("UITVOERINGSBELEID"));
        assert!(prompt.to_lowercase().contains("do not summarize"));
        // Base law only — enrichment is the separate next step.
        assert!(prompt.contains("do NOT add `machine_readable`"));
    }

    #[test]
    fn sanitize_for_url_strips_unsafe_chars() {
        assert_eq!(
            sanitize_for_url("Mijn Beleid (v2).pdf"),
            "mijn-beleid--v2-.pdf"
        );
        assert_eq!(sanitize_for_url(""), "document");
    }

    /// Fake structurer that writes a fixed sequence of YAML outputs, one per
    /// call — proving the validate + repair-round orchestration. Registreert
    /// per aanroep ook de gevraagde Bash-toegang (least-privilege-check).
    struct FakeStructurer {
        outputs: std::sync::Mutex<Vec<String>>,
        bash_per_call: std::sync::Mutex<Vec<bool>>,
    }

    impl FakeStructurer {
        fn new(outputs: Vec<String>) -> Self {
            Self {
                outputs: std::sync::Mutex::new(outputs),
                bash_per_call: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> usize {
            self.bash_per_call.lock().unwrap().len()
        }

        fn bash_flags(&self) -> Vec<bool> {
            self.bash_per_call.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl LawStructurer for FakeStructurer {
        async fn structure(
            &self,
            _prompt: &str,
            work_dir: &Path,
            _config: &EnrichConfig,
            allow_bash: bool,
        ) -> Result<()> {
            self.bash_per_call.lock().unwrap().push(allow_bash);
            let yaml = self.outputs.lock().unwrap().remove(0);
            std::fs::write(work_dir.join(OUTPUT_FILE), yaml).unwrap();
            Ok(())
        }
    }

    fn test_upload() -> Upload {
        // A `.bin` extension no deterministic tool handles, so the structurer
        // gets the raw-file variant of the prompt.
        Upload {
            filename: "beleid.bin".to_string(),
            content_type: "application/octet-stream".to_string(),
            bytes: b"raw bytes".to_vec(),
        }
    }

    #[tokio::test]
    async fn convert_returns_validated_law_first_try() {
        let dir = tempfile::tempdir().unwrap();
        let structurer = FakeStructurer::new(vec![valid_yaml()]);
        let law = convert_law_in_dir(dir.path(), &test_upload(), &test_config(), &structurer)
            .await
            .unwrap();
        assert_eq!(law.meta.law_id, "werkinstructie_toetsing");
        assert_eq!(
            law.meta.regulatory_layer,
            RegulatoryLayer::Uitvoeringsbeleid
        );
        assert_eq!(structurer.calls(), 1);
        // Rauwe .bin zonder deterministische extractie: de agent moet zelf
        // converteren, dus Bash aan.
        assert_eq!(structurer.bash_flags(), vec![true]);
    }

    #[tokio::test]
    async fn convert_runs_one_repair_round_then_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let structurer = FakeStructurer::new(vec!["not: [valid law".to_string(), valid_yaml()]);
        let law = convert_law_in_dir(dir.path(), &test_upload(), &test_config(), &structurer)
            .await
            .unwrap();
        assert_eq!(law.meta.law_id, "werkinstructie_toetsing");
        assert_eq!(structurer.calls(), 2);
        // De reparatieronde bewerkt alleen law.yaml: nooit Bash.
        assert_eq!(structurer.bash_flags(), vec![true, false]);
    }

    #[tokio::test]
    async fn convert_fails_terminally_after_failed_repair() {
        let dir = tempfile::tempdir().unwrap();
        let structurer = FakeStructurer::new(vec![
            "not: [valid law".to_string(),
            "still: invalid\n".to_string(),
        ]);
        let err = convert_law_in_dir(dir.path(), &test_upload(), &test_config(), &structurer)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("still invalid after repair"));
        assert_eq!(structurer.calls(), 2);
    }
}
