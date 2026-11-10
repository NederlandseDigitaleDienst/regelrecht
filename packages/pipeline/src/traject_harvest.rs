//! Traject-scoped harvest (taak-flow): haal een wet uit BWB op voor één
//! traject en keten er een taak-flow-enrich aan.
//!
//! Anders dan het corpus-brede [`crate::harvest`]-pad raakt dit de centrale
//! corpus-repo nooit aan: de harvest schrijft in een eigen tijdelijke
//! werkdirectory, de geproduceerde basis-wet-YAML gaat als input-blob mee met
//! een geketende enrich-job (`deliver: "task"`, `new_law: true` — het
//! law_convert-patroon), en het resultaat komt als `law_create`-review-taak
//! terug bij de aanvrager. Goedkeuren landt de wet in de traject-repo via het
//! gewone law-create-pad. Een directe harvest chain't NIET automatisch een
//! enrich — voor deze flow is de keten dus expliciet (productie-geleerd).

use std::path::Path;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{PipelineError, Result};
use crate::harvest::{HarvestPayload, execute_harvest};
use crate::law_convert::{GeneratedLaw, validate_law_yaml};

/// Marker in de schema-validatiefout hieronder. De worker classificeert de
/// fout hiermee als deterministisch (terminaal, geen retry) — door de
/// gedeelde const kan de formulering niet stilletjes uit de pas lopen met de
/// matcher in `worker.rs`.
pub(crate) const SCHEMA_MISMATCH_MARKER: &str = "valideert niet tegen het schema";

/// Payload van een `traject_harvest`-job. Spiegel van
/// [`crate::law_convert::LawConvertPayload`], maar de bron is een BWB-id in
/// plaats van geüploade bytes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrajectHarvestPayload {
    /// BWB-identifier van de op te halen wet (bijv. "BWBR0018451").
    pub bwb_id: String,
    /// Owning traject's id (feeds the chained enrich payload + failure task).
    pub traject_id: Uuid,
    /// Owning traject ref (also mirrored onto `jobs.traject_ref`).
    pub traject_ref: String,
    /// Weergavenaam uit de zoekresultaten, alleen voor titels van taken; de
    /// echte naam/slug komt uit de harvest zelf.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub law_name: Option<String>,
    /// LLM-provider voor de geketende enrich; `None` = worker-default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Account dat de harvest aanvroeg; wordt de assignee van de uiteindelijke
    /// review-taak (via de geketende enrich-job) en van een `job_failed`-taak.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_by: Option<Uuid>,
    /// `"task"` ⇒ taak-flow. Traject-harvest kent alléén de taak-flow; het
    /// veld bestaat zodat de generieke `list_running_task_jobs_for_account`-
    /// query en de reaper-nazorg dit jobtype meepakken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deliver: Option<String>,
}

impl TrajectHarvestPayload {
    /// Taak-flow: resultaat via de geketende enrich-taak i.p.v. een push.
    pub fn deliver_as_task(&self) -> bool {
        self.deliver.as_deref() == Some("task")
    }

    /// Weergavenaam voor taak-titels: de meegegeven naam of het BWB-id.
    pub fn display_name(&self) -> &str {
        self.law_name.as_deref().unwrap_or(&self.bwb_id)
    }
}

/// Voer de harvest uit in een eigen tijdelijke werkdirectory en lever de
/// gevalideerde basis-wet-YAML op. De aanroeper (worker) ketent de enrich en
/// completet de job via [`crate::law_convert::chain_enrich_and_complete`].
pub async fn execute_traject_harvest(
    payload: &TrajectHarvestPayload,
    http_client: &Client,
) -> Result<GeneratedLaw> {
    // Werkdirectory op basis van het server-gegenereerde traject-UUID, nooit
    // de traject-ref: de ref is een URL-pad-segment waarvan alleen het
    // 8-hex-suffix gevalideerd wordt — het slug-deel is vrije (ASCII-)tekst en
    // een percent-encoded `/` overleeft axum's routing. In een pad dat door
    // `remove_dir_all`/`create_dir_all` gaat zou dat path-traversal zijn.
    // Zelfde patroon als docconvert/lawconvert (upload_id-UUID).
    let work_dir = std::env::temp_dir().join(format!(
        "trajectharvest-{}-{}",
        payload.traject_id, payload.bwb_id
    ));
    // Clear any stale directory left by a previous attempt of this same job.
    let _ = tokio::fs::remove_dir_all(&work_dir).await;
    tokio::fs::create_dir_all(&work_dir).await?;

    let result = harvest_law_in_dir(&work_dir, &payload.bwb_id, http_client).await;

    // Always remove the working directory, success or failure.
    let _ = tokio::fs::remove_dir_all(&work_dir).await;
    result
}

/// De bestandssysteem-helft van de traject-harvest: harvest naar `work_dir`,
/// lees de geproduceerde YAML terug en valideer hem met dezelfde validator
/// als law-convert/law-create — zo is wat de reviewer straks goedkeurt per
/// constructie ook wat `create_traject_law` accepteert.
async fn harvest_law_in_dir(
    work_dir: &Path,
    bwb_id: &str,
    http_client: &Client,
) -> Result<GeneratedLaw> {
    let harvest_payload = HarvestPayload::for_law(bwb_id, None);
    let (result, files) =
        execute_harvest(&harvest_payload, work_dir, "regulation", http_client).await?;
    tracing::info!(
        bwb_id = %bwb_id,
        law_name = %result.law_name,
        slug = %result.slug,
        articles = result.article_count,
        "traject harvest downloaded law"
    );

    let yaml = tokio::fs::read_to_string(&files.content).await?;
    match validate_law_yaml(&yaml) {
        Ok(meta) => Ok(GeneratedLaw { yaml, meta }),
        // Deterministisch: dezelfde bron produceert dezelfde YAML, dus dit
        // hoort als terminale fout in het job_failed-pad te eindigen. De
        // harvester hoort schema-conform te schrijven; dit is het vangnet.
        Err(errors) => Err(PipelineError::Enrich(format!(
            "geharveste YAML voor {bwb_id} {SCHEMA_MISMATCH_MARKER}: {}",
            errors.join("; ")
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_roundtrips_and_backcompat() {
        let account = Uuid::new_v4();
        let payload = TrajectHarvestPayload {
            bwb_id: "BWBR0002399".to_string(),
            traject_id: Uuid::nil(),
            traject_ref: "voorbeeld-abcd1234".to_string(),
            law_name: Some("Voorbeeldwet".to_string()),
            provider: Some("claude".to_string()),
            requested_by: Some(account),
            deliver: Some("task".to_string()),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["bwb_id"], "BWBR0002399");
        assert_eq!(json["deliver"], "task");
        let back: TrajectHarvestPayload = serde_json::from_value(json).unwrap();
        assert_eq!(back, payload);
        assert!(back.deliver_as_task());
    }

    #[test]
    fn minimal_payload_deserializes_without_optionals() {
        let json = serde_json::json!({
            "bwb_id": "BWBR0002399",
            "traject_id": Uuid::nil(),
            "traject_ref": "voorbeeld-abcd1234",
        });
        let payload: TrajectHarvestPayload = serde_json::from_value(json).unwrap();
        assert!(payload.law_name.is_none());
        assert!(payload.provider.is_none());
        assert!(payload.requested_by.is_none());
        assert!(!payload.deliver_as_task());
        assert_eq!(payload.display_name(), "BWBR0002399");
    }

    #[test]
    fn display_name_prefers_law_name() {
        let payload = TrajectHarvestPayload {
            bwb_id: "BWBR0002399".to_string(),
            traject_id: Uuid::nil(),
            traject_ref: "voorbeeld-abcd1234".to_string(),
            law_name: Some("Voorbeeldwet".to_string()),
            provider: None,
            requested_by: None,
            deliver: None,
        };
        assert_eq!(payload.display_name(), "Voorbeeldwet");
    }
}
