//! De verrijking als beoordelingseenheid.
//!
//! Een verrijking (één enrich-job) levert één review-taak per gewijzigd
//! artikel. Beoordelen gebeurt per artikel — dat is de eenheid waarover een
//! mens iets kan zeggen — maar *schrijven* hoort bij de verrijking als geheel:
//! zeven goedgekeurde artikelen zijn één wijziging aan één wet, geen zeven
//! losse commits met dezelfde titel.
//!
//! Dit is de serverkant daarvan:
//!
//! * [`job_tasks`] — welke onderdelen heeft deze verrijking, en wat is hun
//!   status. De beoordelaar (en straks de beoordelingsview) leest hier wat er
//!   nog openstaat.
//! * [`apply`] — verwerk de verrijking: per onderdeel een oordeel, de
//!   eindstand in één keer samengesteld, **één** `write_file` + `persist` met
//!   één `If-Match`, en alle taken in dezelfde transactie dicht.
//!
//! Waarom het samenstellen hier gebeurt en niet in de browser: alleen hier kan
//! de eindstand tegen precies de bytes gezet worden waartegen de `If-Match` is
//! gecontroleerd, onder de write-lock die de commit erna gebruikt. De client
//! zegt wát hij accordeert; wat dat betekent voor het bestand is een
//! serverbeslissing.

use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use uuid::Uuid;

use regelrecht_corpus::backend::EditorUser;
use regelrecht_pipeline::tasks::{self, BlobKind, Task, TaskStatus};

use crate::accounts::AccountRecord;
use crate::corpus_handlers::{self, SavePrInfo, TrajectLawWrite};
use crate::credentials::{TrajectCredentials, WriteAuthorization};
use crate::state::AppState;
use crate::traject_corpus::TrajectCorpus;

fn get_pool(state: &AppState) -> Result<&sqlx::PgPool, (StatusCode, String)> {
    state.pool.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Takenopslag is niet beschikbaar".to_string(),
    ))
}

fn db_error<E: std::fmt::Display>(e: E) -> (StatusCode, String) {
    tracing::error!(error = %e, "verrijkings-query mislukt");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Verrijking verwerken mislukt".to_string(),
    )
}

/// Het artikelnummer waar een taak over gaat, `None` voor een taak die de hele
/// wet omvat (een `law_create`, of een voorstel dat de worker niet in artikelen
/// kon opsplitsen — zie `changed_articles` in de pipeline-worker).
///
/// De worker schrijft het nummer als string, maar een wet mag `number: 5`
/// schrijven, dus een getal telt net zo goed.
fn task_article(task: &Task) -> Option<String> {
    scalar_to_string(task.payload.as_ref()?.get("article")?)
}

fn scalar_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn payload_str(task: &Task, key: &str) -> Option<String> {
    task.payload
        .as_ref()?
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

// ---------------------------------------------------------------------------
// GET /api/tasks/jobs/{job_id}
// ---------------------------------------------------------------------------

/// Eén onderdeel van een verrijking, zoals de beoordelaar het ziet.
#[derive(Serialize)]
pub struct JobTaskSummary {
    pub id: Uuid,
    pub status: TaskStatus,
    pub title: String,
    /// Artikelnummer, of `null` wanneer dit onderdeel de hele wet is.
    pub article: Option<String>,
}

#[derive(Serialize)]
pub struct JobTasksResponse {
    pub job_id: Uuid,
    /// De wet waar deze verrijking over gaat (uit de taak-payload).
    pub law_id: Option<String>,
    pub tasks: Vec<JobTaskSummary>,
}

/// GET /api/tasks/jobs/{job_id} — de onderdelen van één verrijking.
///
/// Afgehandelde onderdelen komen mee: wie wil weten of hij klaar is, moet ook
/// kunnen zien wat er buiten hem om al dichtging (een verwijderd traject sluit
/// open taken, zie `dismiss_open_tasks_for_traject`).
pub async fn job_tasks(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<JobTasksResponse>, (StatusCode, String)> {
    let pool = get_pool(&state)?;
    let tasks = tasks::list_tasks_for_job_and_account(pool, job_id, account.id)
        .await
        .map_err(db_error)?;
    if tasks.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            "Verrijking niet gevonden".to_string(),
        ));
    }
    let law_id = tasks.iter().find_map(|t| payload_str(t, "law_id"));
    Ok(Json(JobTasksResponse {
        job_id,
        law_id,
        tasks: tasks
            .iter()
            .map(|t| JobTaskSummary {
                id: t.id,
                status: t.status,
                title: t.title.clone(),
                article: task_article(t),
            })
            .collect(),
    }))
}

// ---------------------------------------------------------------------------
// POST /api/tasks/jobs/{job_id}/apply
// ---------------------------------------------------------------------------

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Overnemen.
    Approved,
    /// Niet overnemen.
    Rejected,
}

/// Het oordeel over één onderdeel van de verrijking.
#[derive(Deserialize)]
pub struct Decision {
    pub task_id: Uuid,
    pub action: Verdict,
    /// Bij overnemen: de inhoud die de gebruiker daadwerkelijk accordeert —
    /// het artikel zoals het na eventueel bijschaven in de editor staat, als
    /// YAML-mapping (of de hele wet-YAML voor een onderdeel zonder
    /// artikelnummer).
    ///
    /// Weggelaten betekent "neem het ruwe voorstel over": de server pakt dan
    /// het artikel uit de opgeslagen result-blob van de job.
    #[serde(default)]
    pub content: Option<String>,
}

#[derive(Deserialize)]
pub struct ApplyRequest {
    pub decisions: Vec<Decision>,
}

#[derive(Serialize)]
pub struct ApplyResponse {
    pub law_id: String,
    /// Aantal overgenomen onderdelen.
    pub accepted: usize,
    /// Aantal onderdelen waarover een oordeel is geveld.
    pub total: usize,
    /// ETag van de wet na het schrijven; `null` wanneer er niets is
    /// overgenomen en er dus niet geschreven is.
    pub etag: Option<String>,
    pub pr: Option<SavePrInfo>,
}

/// Wat er van één goedgekeurd onderdeel de eindstand in gaat.
struct Accepted {
    /// `None` = dit onderdeel is de hele wet.
    article: Option<String>,
    content: String,
}

/// POST /api/tasks/jobs/{job_id}/apply — verwerk een verrijking.
///
/// Volgorde en atomiciteit:
///
/// 1. De oordelen moeten alle nog-open onderdelen dekken. Ontbreekt er één,
///    dan is de verrijking niet af en verandert er niets aan de wet.
/// 2. De taken gaan dicht binnen een transactie die pas commit als de
///    schrijfactie is gelukt. Faalt de `If-Match` (iemand anders wijzigde de
///    wet), dan rolt die transactie terug: er is niets geschreven en geen
///    enkele taak blijft afgehandeld achter.
/// 3. Niets overnemen schrijft niets — geen lege commit. De taken gaan wel
///    dicht.
///
/// Wat die transactie **niet** kan: de git-commit terugdraaien. Twee systemen
/// zonder gedeelde transactie hebben één moment waarop ze uiteen kunnen lopen,
/// en hier ligt dat tussen een geslaagde `persist` en `tx.commit()`. Valt de
/// databaseverbinding precies dáár weg, dan staat de wijziging in de wet
/// terwijl de taken open blijven; de beoordelaar ziet de verrijking dan
/// opnieuw in zijn lijst. Dat is de kant waar deze volgorde bewust op faalt —
/// een verrijking die nog een keer beoordeeld moet worden is te herstellen,
/// een taak die dicht staat terwijl er niets geschreven is niet, want dan is
/// het voorstel weg. Die uitkomst wordt hard gelogd, want hij is aan niets
/// anders te zien.
///
/// De `If-Match`-header draagt de ETag van de wet zoals de beoordelaar hem
/// zag. Dat is meteen de enige staleness-bepaling die deze flow nog kent: één
/// schrijfmoment, één controle. Hij is daarom **verplicht** zodra er iets
/// overgenomen wordt — de permissieve blinde write die de gewone PUT om
/// historische redenen toestaat, zou hier de enige controle wegnemen die er
/// nog is. Verwerken zonder overnemen schrijft niets en vraagt er dus ook niet
/// om.
pub async fn apply(
    State(state): State<AppState>,
    Extension(account): Extension<AccountRecord>,
    session: Session,
    Path(job_id): Path<Uuid>,
    headers: HeaderMap,
    Json(req): Json<ApplyRequest>,
) -> Result<Json<ApplyResponse>, (StatusCode, String)> {
    let pool = get_pool(&state)?;
    let all_tasks = tasks::list_tasks_for_job_and_account(pool, job_id, account.id)
        .await
        .map_err(db_error)?;
    if all_tasks.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            "Verrijking niet gevonden".to_string(),
        ));
    }
    let open: Vec<&Task> = all_tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Open)
        .collect();
    if open.is_empty() {
        return Err((
            StatusCode::CONFLICT,
            "Deze verrijking is al verwerkt.".to_string(),
        ));
    }

    let law_id = open.iter().find_map(|t| payload_str(t, "law_id")).ok_or((
        StatusCode::BAD_REQUEST,
        "Deze taak wijst geen regelwerk aan".to_string(),
    ))?;
    let traject_ref = open
        .iter()
        .find_map(|t| payload_str(t, "traject_ref"))
        .ok_or((
            StatusCode::BAD_REQUEST,
            "Deze taak hoort niet bij een traject".to_string(),
        ))?;
    // Een `law_create` bestaat nog niet als bestand: die gaat langs het
    // aanmaakpad (POST .../corpus/laws), waar het pad uit de YAML wordt
    // afgeleid en er niets is om een `If-Match` tegen te houden. Hier
    // binnenlaten zou een tweede aanmaakimplementatie betekenen.
    if open
        .iter()
        .any(|t| payload_str(t, "kind").as_deref() == Some("law_create"))
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "Een nieuw regelwerk wordt via het aanmaakpad opgeslagen, niet als verrijking"
                .to_string(),
        ));
    }

    // --- de oordelen uitpaken en tegen de open onderdelen leggen ---------
    let mut seen: HashSet<Uuid> = HashSet::new();
    let mut approved_ids: Vec<Uuid> = Vec::new();
    let mut rejected_ids: Vec<Uuid> = Vec::new();
    let mut accepted: Vec<Accepted> = Vec::new();
    for decision in &req.decisions {
        let task = open.iter().find(|t| t.id == decision.task_id).ok_or((
            StatusCode::BAD_REQUEST,
            "Een oordeel wijst een onderdeel aan dat niet (meer) openstaat".to_string(),
        ))?;
        if !seen.insert(decision.task_id) {
            return Err((
                StatusCode::BAD_REQUEST,
                "Twee oordelen over hetzelfde onderdeel".to_string(),
            ));
        }
        match decision.action {
            Verdict::Rejected => rejected_ids.push(task.id),
            Verdict::Approved => {
                approved_ids.push(task.id);
                accepted.push(Accepted {
                    article: task_article(task),
                    content: decision.content.clone().unwrap_or_default(),
                });
            }
        }
    }
    if seen.len() != open.len() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Nog niet elk onderdeel van deze verrijking heeft een oordeel".to_string(),
        ));
    }

    // Onderdelen zonder inhoud vallen terug op het ruwe voorstel uit de job.
    if accepted.iter().any(|a| a.content.is_empty()) {
        let proposal = load_proposal(pool, job_id, &open).await?;
        for part in accepted.iter_mut().filter(|a| a.content.is_empty()) {
            part.content = match &part.article {
                None => proposal.clone(),
                Some(number) => article_from_proposal(&proposal, number)?,
            };
        }
    }

    // Een onderdeel zonder artikelnummer is per definitie het énige onderdeel
    // van zijn verrijking (de worker maakt óf één taak per gewijzigd artikel,
    // óf één taak voor het geheel). Een mengeling zou betekenen dat "de hele
    // versie" en "artikel 3" tegelijk overgenomen worden, en dan is niet te zeggen
    // wat er wint.
    let whole_law = accepted.iter().any(|a| a.article.is_none());
    if whole_law && open.len() > 1 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Een voorstel voor de hele versie kan niet samen met losse artikelen worden verwerkt"
                .to_string(),
        ));
    }

    // Een whole-law-onderdeel zet de aangeleverde inhoud integraal op de
    // plaats van de wet — precies wat de PUT doet, dus precies dezelfde poort.
    // De artikel-route heeft hem niet nodig: daar wordt in de opgeslagen wet
    // gespliced, dus blijven `$id` en de rest van het bestand van de server.
    //
    // Dat de editor hier in de praktijk het onbewerkte voorstel terugstuurt,
    // is geen bescherming: het endpoint accepteert geaccordeerde inhoud (dat
    // is het punt van criterium 2) en moet zelf weten wat het aanneemt.
    if whole_law {
        for part in accepted.iter().filter(|a| a.article.is_none()) {
            corpus_handlers::validate_whole_law_body(&part.content, &law_id)?;
        }
    }

    let accepted_count = accepted.len();
    let total = open.len();

    // De schrijfactie helemaal klaarzetten *voordat* de taak-transactie
    // opengaat. Zie [`PreparedWrite`]: elke stap eronder leent zelf een
    // verbinding uit dezelfde pool, en de write-lock op de backend hoort in
    // dezelfde volgorde genomen te worden als `save_law` hem neemt.
    //
    // Alleen wanneer er iets overgenomen wordt: "niets overnemen" schrijft
    // niet, en hoeft dus ook geen schrijfrecht, geen GitHub-koppeling en geen
    // `If-Match`.
    let prepared = if accepted.is_empty() {
        None
    } else {
        Some(prepare_write(&state, &account, &session, &headers, &traject_ref, &law_id).await?)
    };

    // --- taken dicht + schrijven, of terugrollen -------------------------
    //
    // Deze transactie blijft openstaan over de schrijfactie heen, en houdt dus
    // één poolverbinding vast zolang GitHub erover doet. Dat is de prijs van
    // "alle taken in dezelfde transactie als de write": zonder die overlap kan
    // een mislukte `If-Match` de al afgehandelde taken niet meer terugdraaien.
    //
    // Wat de prijs begrenst is de volgorde hierboven. De write-lock op de
    // backend wordt vóór deze transactie genomen, dus twee verwerkingen op
    // hetzelfde traject staan op die mutex te wachten en niet op een
    // verbinding. Wat er tegelijk openstaat is daarmee hooguit één transactie
    // per traject dat op dit moment geschreven wordt — niet één per
    // beoordelaar.
    let mut tx = pool.begin().await.map_err(db_error)?;
    let closed = tasks::resolve_tasks(&mut *tx, &approved_ids, account.id, TaskStatus::Approved)
        .await
        .map_err(db_error)?
        + tasks::resolve_tasks(&mut *tx, &rejected_ids, account.id, TaskStatus::Rejected)
            .await
            .map_err(db_error)?;
    if closed as usize != total {
        let _ = tx.rollback().await;
        return Err((
            StatusCode::CONFLICT,
            "Een onderdeel van deze verrijking is intussen afgehandeld.".to_string(),
        ));
    }

    let written = match prepared {
        // Niets overnemen: de wet blijft zoals hij is. Een commit die niets
        // verandert zou het traject-log vervuilen met een gebeurtenis die
        // alleen in de takenlijst thuishoort.
        None => None,
        Some(prepared) => {
            match write_accepted(prepared, &law_id, &accepted, whole_law, total).await {
                Ok(written) => Some(written),
                Err(e) => {
                    let _ = tx.rollback().await;
                    return Err(e);
                }
            }
        }
    };
    if let Err(e) = tx.commit().await {
        // Het enige punt waarop wet en takenlijst uiteen kunnen lopen: is er
        // geschreven, dan staat die commit er en komt hij hier niet meer weg.
        // Loggen op error-niveau, met de wet erbij — aan de takenlijst alleen
        // is niet te zien dat het voorstel al geland is.
        if written.is_some() {
            tracing::error!(
                error = %e,
                job_id = %job_id,
                law_id = %law_id,
                "verrijking is weggeschreven maar de taken bleven open staan; \
                 de wet is bijgewerkt, de beoordelaar ziet de verrijking opnieuw"
            );
        }
        return Err(db_error(e));
    }

    // Pas nu de blobs opruimen: dit was het laatste moment waarop iemand het
    // voorstel nog nodig had. Best-effort, net als na een losse resolve — de
    // 7-dagen-GC vangt het restje.
    if let Err(e) = tasks::delete_blobs_for_finished_job(pool, job_id).await {
        tracing::warn!(error = %e, job_id = %job_id, "blob-cleanup na verwerken mislukt");
    }
    tracing::info!(
        job_id = %job_id,
        law_id = %law_id,
        accepted = accepted_count,
        total,
        "verrijking verwerkt"
    );

    let (etag, pr) = match written {
        Some(w) => (Some(w.etag), w.response.pr),
        None => (None, None),
    };
    Ok(Json(ApplyResponse {
        law_id,
        accepted: accepted_count,
        total,
        etag,
        pr,
    }))
}

/// Het voorstel van de job: de wet-YAML uit de result-blobs.
///
/// Zelfde keuze als de editor maakt bij het laden van een review-taak: eerst
/// het exacte pad uit de payload, anders het eerste bestand dat geen
/// dot-prefixed sidecar is (de worker staged ook `features/*.feature`).
async fn load_proposal(
    pool: &sqlx::PgPool,
    job_id: Uuid,
    open: &[&Task],
) -> Result<String, (StatusCode, String)> {
    let yaml_path = open.iter().find_map(|t| payload_str(t, "yaml_path"));
    let blobs = tasks::load_blobs(pool, job_id, BlobKind::Result)
        .await
        .map_err(db_error)?;
    blobs
        .iter()
        .find(|b| Some(b.path.as_str()) == yaml_path.as_deref())
        .or_else(|| {
            blobs
                .iter()
                .find(|b| !b.path.rsplit('/').next().unwrap_or("").starts_with('.'))
        })
        .map(|b| b.content.clone())
        .ok_or((
            StatusCode::GONE,
            "Het voorstel bij deze verrijking is niet meer beschikbaar".to_string(),
        ))
}

/// Eén artikel uit het voorstel lichten, als YAML.
fn article_from_proposal(proposal: &str, number: &str) -> Result<String, (StatusCode, String)> {
    let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(proposal).map_err(|e| {
        tracing::warn!(error = %e, "voorstel is geen geldige YAML");
        (
            StatusCode::UNPROCESSABLE_ENTITY,
            "Het voorstel bij deze verrijking is niet leesbaar".to_string(),
        )
    })?;
    let article = doc
        .get("articles")
        .and_then(|a| a.as_sequence())
        .and_then(|list| {
            list.iter()
                .find(|a| article_number(a).as_deref() == Some(number))
        })
        .ok_or((
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("Het voorstel bevat geen artikel {number}"),
        ))?;
    serde_yaml_ng::to_string(article).map_err(|e| {
        tracing::error!(error = %e, "artikel uit voorstel serialiseren mislukt");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Verrijking verwerken mislukt".to_string(),
        )
    })
}

/// Alles wat de schrijfactie nodig heeft, opgehaald *voordat* de
/// taak-transactie opengaat.
///
/// Waarom vooraf, en niet gewoon binnen de transactie: elk van die stappen
/// leent zelf een verbinding uit dezelfde pool waar de transactie er al een
/// van vasthoudt (de traject-lookup, de sessie-store, de feature-flag). Dat
/// binnen de transactie doen laat één verzoek op twee verbindingen tegelijk
/// wachten, en de pool telt er vijf — een handvol gelijktijdige verwerkingen
/// wacht dan op zichzelf, met een schrijfactie naar GitHub als duur van het
/// venster.
///
/// En het houdt de slotvolgorde gelijk aan die van [`corpus_handlers::save_law`]:
/// eerst de write-lock op de backend, dan de database. Andersom (transactie
/// eerst, dan de lock) staan de twee schrijfpaden precies omgekeerd in de rij
/// en kunnen ze elkaar vasthouden.
///
/// Wat er ná dit punt nog in de transactie gebeurt raakt alleen `tasks` en de
/// git-backend.
struct PreparedWrite {
    traject: Arc<TrajectCorpus>,
    write: TrajectLawWrite,
    auth: WriteAuthorization,
    author: Option<EditorUser>,
    relative_path: std::path::PathBuf,
    /// De ETag van de wet zoals de beoordelaar hem zag. Verplicht: dit is het
    /// enige moment waarop deze flow nog op verouderdheid controleert, dus
    /// zonder `If-Match` zou het verwerken een blinde overschrijving zijn.
    if_match: String,
}

async fn prepare_write(
    state: &AppState,
    account: &AccountRecord,
    session: &Session,
    headers: &HeaderMap,
    traject_ref: &str,
    law_id: &str,
) -> Result<PreparedWrite, (StatusCode, String)> {
    // Niet 428: die code is editor-breed gereserveerd voor de
    // GitHub-koppelflow (zie `frontend/src/lib/apiAuthGuard.js`) en zou de
    // beoordelaar naar een koppelscherm sturen voor iets wat een herlading is.
    let if_match = corpus_handlers::extract_if_match(headers).ok_or((
        StatusCode::BAD_REQUEST,
        "Verwerken vraagt om de versie van het regelwerk zoals je die zag. Herlaad de pagina en \
         beoordeel de verrijking opnieuw."
            .to_string(),
    ))?;
    let author = Some(corpus_handlers::require_editor_user(session).await?);
    let traject =
        corpus_handlers::require_traject_corpus_from_ref(state, session, traject_ref).await?;
    let write = corpus_handlers::resolve_traject_law_write(&traject, law_id).await?;
    let auth = TrajectCredentials::new(state, account.id, headers)
        .for_write(&**write.backend, write.write_source_writable)
        .await?;
    let relative_path = std::path::PathBuf::from(&write.law.relative_path);
    Ok(PreparedWrite {
        traject,
        write,
        auth,
        author,
        relative_path,
        if_match,
    })
}

async fn write_accepted(
    prepared: PreparedWrite,
    law_id: &str,
    accepted: &[Accepted],
    whole_law: bool,
    total: usize,
) -> Result<corpus_handlers::WrittenLaw, (StatusCode, String)> {
    let PreparedWrite {
        traject,
        write,
        auth,
        author,
        relative_path,
        if_match,
    } = prepared;
    let message = commit_message(law_id, accepted.len(), total, whole_law);

    corpus_handlers::write_composed_law(
        &traject,
        write,
        auth,
        &relative_path,
        law_id,
        author,
        Some(if_match),
        true,
        |current| {
            if whole_law {
                // Eén onderdeel dat de hele wet ís: de geaccordeerde inhoud
                // staat er integraal, er valt niets in te splicen.
                return Ok((accepted[0].content.clone(), message));
            }
            let base = current.ok_or((
                StatusCode::NOT_FOUND,
                "Het regelwerk bestaat niet (meer) in dit traject".to_string(),
            ))?;
            let parts: Vec<(&str, &str)> = accepted
                .iter()
                .filter_map(|a| Some((a.article.as_deref()?, a.content.as_str())))
                .collect();
            let body = compose_enriched_law(base, &parts)
                .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e))?;
            Ok((body, message))
        },
    )
    .await
}

/// De commit-message van een verwerkte verrijking. Noemt wat het was, hoeveel
/// ervan is overgenomen en welke wet het betreft — een verrijking van zeven
/// artikelen is één gebeurtenis, en het log hoort dat te laten zien in plaats
/// van zeven regels `Update law <id>`.
fn commit_message(law_id: &str, accepted: usize, total: usize, whole_law: bool) -> String {
    if whole_law {
        return format!("Verrijking verwerkt: het hele voorstel overgenomen in {law_id}");
    }
    // Het zelfstandig naamwoord hoort bij het getal dat er direct voor staat -
    // het totaal, niet het aantal overgenomen. "1 van de 4 artikel" leest als
    // een schrijffout in een commit-log dat je later terugleest.
    let artikelen = if total == 1 { "artikel" } else { "artikelen" };
    format!("Verrijking verwerkt: {accepted} van de {total} {artikelen} overgenomen in {law_id}")
}

/// Het nummer van een artikel als string; `None` wanneer het ontbreekt of geen
/// scalar is. Een wet mag `number: 5` schrijven, dus een getal telt mee.
fn article_number(article: &serde_yaml_ng::Value) -> Option<String> {
    match article.get("number")? {
        serde_yaml_ng::Value::String(s) => Some(s.clone()),
        serde_yaml_ng::Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Stel de eindstand samen: de huidige wet met de overgenomen artikelen erin.
///
/// Elk geaccordeerd artikel vervangt het gelijkgenummerde artikel in de wet;
/// een nummer dat de wet niet heeft komt er achteraan bij (een verrijking die
/// een artikel toevoegt is zeldzaam, en de plek raden zou stiller fout gaan dan
/// het zichtbaar achteraan zetten). Het nummer in de aangeleverde inhoud moet
/// kloppen met het onderdeel waarover geoordeeld is — anders zou een oordeel
/// over artikel 3 een heel ander artikel kunnen wegschrijven.
///
/// Net als de bestaande save-paden gaat dit door een YAML-round-trip, die
/// commentaar en sleutelvolgorde niet garandeert. Dat is dezelfde bekende
/// beperking als in de editor (`currentLawYaml`): het corpus is
/// harvester-gegenereerd en commentaarloos.
fn compose_enriched_law(current: &str, accepted: &[(&str, &str)]) -> Result<String, String> {
    let mut doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(current)
        .map_err(|e| format!("Het opgeslagen regelwerk is geen geldige YAML: {e}"))?;
    let articles = doc
        .get_mut("articles")
        .and_then(|a| a.as_sequence_mut())
        .ok_or_else(|| "Het opgeslagen regelwerk heeft geen artikelen".to_string())?;

    for (number, content) in accepted {
        let article: serde_yaml_ng::Value = serde_yaml_ng::from_str(content)
            .map_err(|e| format!("Artikel {number} is geen geldige YAML: {e}"))?;
        if !article.is_mapping() {
            return Err(format!("Artikel {number} is geen YAML-mapping"));
        }
        match article_number(&article) {
            Some(found) if found == *number => {}
            _ => {
                return Err(format!(
                    "De aangeleverde inhoud hoort niet bij artikel {number}"
                ));
            }
        }
        match articles
            .iter()
            .position(|a| article_number(a).as_deref() == Some(*number))
        {
            Some(idx) => articles[idx] = article,
            None => articles.push(article),
        }
    }

    serde_yaml_ng::to_string(&doc).map_err(|e| format!("Eindstand serialiseren mislukt: {e}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const CURRENT: &str = "$id: test_wet\n\
                           articles:\n\
                           - number: '1'\n  text: een\n\
                           - number: '2'\n  text: twee\n";

    fn articles_of(yaml: &str) -> Vec<(String, String)> {
        let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(yaml).unwrap();
        doc.get("articles")
            .unwrap()
            .as_sequence()
            .unwrap()
            .iter()
            .map(|a| {
                (
                    article_number(a).unwrap(),
                    a.get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn splices_an_accepted_article_in_place() {
        let out =
            compose_enriched_law(CURRENT, &[("2", "number: '2'\ntext: twee verrijkt\n")]).unwrap();
        assert_eq!(
            articles_of(&out),
            vec![
                ("1".to_string(), "een".to_string()),
                ("2".to_string(), "twee verrijkt".to_string()),
            ]
        );
        // De rest van de wet blijft staan.
        assert!(out.contains("test_wet"));
    }

    #[test]
    fn leaves_the_law_alone_when_nothing_is_accepted() {
        let out = compose_enriched_law(CURRENT, &[]).unwrap();
        assert_eq!(articles_of(&out), articles_of(CURRENT));
    }

    #[test]
    fn appends_an_article_the_law_does_not_have_yet() {
        let out = compose_enriched_law(CURRENT, &[("3", "number: '3'\ntext: drie\n")]).unwrap();
        assert_eq!(
            articles_of(&out)
                .iter()
                .map(|(n, _)| n.clone())
                .collect::<Vec<_>>(),
            vec!["1", "2", "3"]
        );
    }

    #[test]
    fn matches_an_unquoted_article_number() {
        let current = "$id: test_wet\narticles:\n- number: 2\n  text: twee\n";
        let out = compose_enriched_law(current, &[("2", "number: 2\ntext: verrijkt\n")]).unwrap();
        assert_eq!(
            articles_of(&out),
            vec![("2".to_string(), "verrijkt".to_string())]
        );
    }

    #[test]
    fn refuses_content_whose_number_does_not_match_the_verdict() {
        let err =
            compose_enriched_law(CURRENT, &[("2", "number: '1'\ntext: sluipt\n")]).unwrap_err();
        assert!(err.contains("artikel 2"), "onverwachte fout: {err}");
    }

    #[test]
    fn refuses_content_that_is_not_a_mapping() {
        let err = compose_enriched_law(CURRENT, &[("2", "- een lijst\n")]).unwrap_err();
        assert!(err.contains("mapping"), "onverwachte fout: {err}");
    }

    #[test]
    fn refuses_a_law_without_articles() {
        let err = compose_enriched_law("$id: test_wet\n", &[]).unwrap_err();
        assert!(err.contains("geen artikelen"), "onverwachte fout: {err}");
    }

    #[test]
    fn commit_message_names_the_enrichment_the_count_and_the_law() {
        assert_eq!(
            commit_message("test_wet", 3, 7, false),
            "Verrijking verwerkt: 3 van de 7 artikelen overgenomen in test_wet"
        );
        // Het meervoud volgt het totaal, niet het aantal overgenomen: "1 van
        // de 4 artikel" zou een schrijffout in het log zijn.
        assert_eq!(
            commit_message("test_wet", 1, 4, false),
            "Verrijking verwerkt: 1 van de 4 artikelen overgenomen in test_wet"
        );
        assert_eq!(
            commit_message("test_wet", 0, 1, false),
            "Verrijking verwerkt: 0 van de 1 artikel overgenomen in test_wet"
        );
        assert_eq!(
            commit_message("test_wet", 1, 1, true),
            "Verrijking verwerkt: het hele voorstel overgenomen in test_wet"
        );
    }

    #[test]
    fn takes_the_article_from_the_proposal_when_the_client_sends_none() {
        let proposal = "$id: test_wet\narticles:\n- number: '1'\n  text: voorstel een\n";
        let article = article_from_proposal(proposal, "1").unwrap();
        assert!(article.contains("voorstel een"), "kreeg: {article}");
        assert_eq!(
            article_from_proposal(proposal, "9").unwrap_err().0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    /// De verrijkings-routes hangen onder `/api/tasks/`, waar `{task_id}` al
    /// een parameter op dezelfde plek is als het statische `jobs`. Dat mág van
    /// de router, maar het is precies het soort ding dat stil verkeerd gaat:
    /// slokt `{task_id}` het pad op, dan komt een verrijking als taak-detail
    /// binnen en verdwijnt het verwerken zonder foutmelding.
    #[tokio::test]
    async fn the_enrichment_routes_are_not_swallowed_by_the_task_id_route() {
        use axum::body::Body;
        use axum::http::Request;
        use axum::routing::{get, post};
        use tower::ServiceExt;

        async fn code(status: StatusCode) -> StatusCode {
            status
        }
        let app = axum::Router::new()
            .route(
                "/api/tasks/{task_id}",
                get(|| code(StatusCode::IM_A_TEAPOT)),
            )
            .route(
                "/api/tasks/{task_id}/resolve",
                post(|| code(StatusCode::ACCEPTED)),
            )
            .route("/api/tasks/jobs/{job_id}", get(|| code(StatusCode::OK)))
            .route(
                "/api/tasks/jobs/{job_id}/apply",
                post(|| code(StatusCode::CREATED)),
            );

        let hit = |method: &'static str, path: String| {
            let app = app.clone();
            async move {
                app.oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap()
                .status()
            }
        };
        let id = Uuid::new_v4();
        assert_eq!(
            hit("GET", format!("/api/tasks/{id}")).await,
            StatusCode::IM_A_TEAPOT
        );
        assert_eq!(
            hit("POST", format!("/api/tasks/{id}/resolve")).await,
            StatusCode::ACCEPTED
        );
        assert_eq!(
            hit("GET", format!("/api/tasks/jobs/{id}")).await,
            StatusCode::OK
        );
        assert_eq!(
            hit("POST", format!("/api/tasks/jobs/{id}/apply")).await,
            StatusCode::CREATED
        );
    }

    #[test]
    fn verdict_deserializes_lowercase_and_rejects_anything_else() {
        assert_eq!(
            serde_json::from_str::<Verdict>("\"approved\"").unwrap(),
            Verdict::Approved
        );
        assert_eq!(
            serde_json::from_str::<Verdict>("\"rejected\"").unwrap(),
            Verdict::Rejected
        );
        assert!(serde_json::from_str::<Verdict>("\"dismissed\"").is_err());
    }
}
