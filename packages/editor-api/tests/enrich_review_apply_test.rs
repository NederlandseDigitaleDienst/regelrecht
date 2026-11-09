//! De verrijking als beoordelingseenheid: `enrich_review::apply` verwerkt
//! álle onderdelen van één enrich-job in één schrijfactie.
//!
//! Wat hier vastligt is de eenheid, niet de vormgeving: één verrijking is één
//! commit, en tot het moment van verwerken verandert er niets aan de wet. De
//! scenario's dekken alles overnemen, deels overnemen, niets overnemen, een
//! `If-Match`-conflict tijdens het verwerken en een verrijking die uit één
//! whole-law-onderdeel bestaat.
//!
//! Zelfde hermetische opzet als `traject_reads_test.rs`: lokale
//! writable-own bron, geen GitHub, geen netwerk, handlers rechtstreeks
//! aangeroepen met inline `axum`-extractors.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::{empty_state, local_traject, seed_account, session_for, traject_ref};

use axum::extract::{Extension, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::Json;
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use tower_sessions::Session;
use uuid::Uuid;

use regelrecht_editor_api::accounts::AccountRecord;
use regelrecht_editor_api::corpus_handlers::get_traject_corpus_law;
use regelrecht_editor_api::enrich_review::{apply, job_tasks};
use regelrecht_editor_api::state::AppState;

use regelrecht_pipeline::test_utils::TestDb;

const LAW_ID: &str = "wet_op_de_zorgtoeslag";
const LAW_FILE: &str = "2025-01-01.yaml";

/// De wet zoals hij op de branch staat vóór het verwerken.
const SAVED_LAW: &str = r#"$id: wet_op_de_zorgtoeslag
name: Testwet
articles:
- number: '1'
  text: artikel een
- number: '2'
  text: artikel twee
"#;

/// Het voorstel dat de verrijking opleverde: beide artikelen aangeraakt.
const PROPOSAL: &str = r#"$id: wet_op_de_zorgtoeslag
name: Testwet
articles:
- number: '1'
  text: artikel een verrijkt
- number: '2'
  text: artikel twee verrijkt
"#;

fn law_path(corpus_dir: &std::path::Path) -> std::path::PathBuf {
    corpus_dir.join("wet").join(LAW_ID).join(LAW_FILE)
}

fn write_law(corpus_dir: &std::path::Path, content: &str) {
    let path = law_path(corpus_dir);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn read_law_file(corpus_dir: &std::path::Path) -> String {
    std::fs::read_to_string(law_path(corpus_dir)).unwrap()
}

fn account_record(id: Uuid) -> AccountRecord {
    AccountRecord {
        id,
        person_sub: "test-sub".to_string(),
        email: "alice@test.local".to_string(),
        name: "Test User".to_string(),
    }
}

fn if_match_headers(etag: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::IF_MATCH,
        HeaderValue::from_str(etag).unwrap(),
    );
    headers
}

/// Een afgeronde enrich-job met het voorstel als result-blob.
async fn seed_job(pool: &PgPool, proposal: &str) -> Uuid {
    let (job_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO jobs (job_type, law_id, status, payload)
         VALUES ('enrich'::job_type, $1, 'completed'::job_status, '{}'::jsonb)
         RETURNING id",
    )
    .bind(LAW_ID)
    .fetch_one(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO job_blobs (job_id, kind, path, content) VALUES ($1, 'result', $2, $3)",
    )
    .bind(job_id)
    .bind(format!("wet/{LAW_ID}/{LAW_FILE}"))
    .bind(proposal)
    .execute(pool)
    .await
    .unwrap();
    job_id
}

/// Eén review-taak van die job. `article = None` modelleert het
/// whole-law-onderdeel (een voorstel dat de worker niet kon opsplitsen).
async fn seed_task(
    pool: &PgPool,
    job_id: Uuid,
    account_id: Uuid,
    traject_id: Uuid,
    tref: &str,
    article: Option<&str>,
) -> Uuid {
    let mut payload = serde_json::json!({
        "law_id": LAW_ID,
        "yaml_path": format!("wet/{LAW_ID}/{LAW_FILE}"),
        "traject_ref": tref,
    });
    if let Some(number) = article {
        payload["article"] = serde_json::json!(number);
    }
    let (task_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO tasks (task_type, assignee_account_id, traject_id, job_id, title, payload)
         VALUES ('job_review', $1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(account_id)
    .bind(traject_id)
    .bind(job_id)
    .bind(format!(
        "Verrijking beoordelen: {LAW_ID} {}",
        article.unwrap_or("hele wet")
    ))
    .bind(&payload)
    .fetch_one(pool)
    .await
    .unwrap();
    task_id
}

async fn task_status(pool: &PgPool, task_id: Uuid) -> String {
    let (status,): (String,) = sqlx::query_as("SELECT status::text FROM tasks WHERE id = $1")
        .bind(task_id)
        .fetch_one(pool)
        .await
        .unwrap();
    status
}

async fn blob_count(pool: &PgPool, job_id: Uuid) -> i64 {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM job_blobs WHERE job_id = $1")
        .bind(job_id)
        .fetch_one(pool)
        .await
        .unwrap();
    count
}

/// De huidige ETag van de wet, zoals de beoordelaar hem meekrijgt op de GET.
async fn law_etag(state: AppState, session: Session, tref: &str, account: AccountRecord) -> String {
    let (status, headers, _body) = get_traject_corpus_law(
        State(state),
        Extension(account),
        session,
        Path((tref.to_string(), LAW_ID.to_string())),
        HeaderMap::new(),
    )
    .await
    .expect("law GET must succeed");
    assert_eq!(status, StatusCode::OK);
    headers
        .iter()
        .find(|(name, _)| name == axum::http::header::ETAG)
        .map(|(_, value)| value.clone())
        .expect("law GET must carry an ETag")
}

fn decision(task_id: Uuid, action: &str, content: Option<&str>) -> serde_json::Value {
    let mut d = serde_json::json!({ "task_id": task_id, "action": action });
    if let Some(c) = content {
        d["content"] = serde_json::json!(c);
    }
    d
}

/// De volledige opzet die elke test deelt: account, traject met een lokale
/// writable-own bron, de opgeslagen wet, en een afgeronde verrijking.
struct Fixture {
    db: TestDb,
    state: AppState,
    corpus: tempfile::TempDir,
    sub: String,
    account: AccountRecord,
    tref: String,
    job_id: Uuid,
}

impl Fixture {
    async fn new() -> Self {
        let db = TestDb::new().await;
        let state = empty_state(db.pool.clone());
        let (owner, sub) = seed_account(&db.pool, "alice@test.local").await;
        let corpus = tempfile::tempdir().unwrap();
        write_law(corpus.path(), SAVED_LAW);
        let traject_id = local_traject(&db.pool, owner, corpus.path()).await;
        let tref = traject_ref(traject_id);
        let job_id = seed_job(&db.pool, PROPOSAL).await;
        Self {
            db,
            state,
            corpus,
            sub,
            account: account_record(owner),
            tref,
            job_id,
        }
    }

    async fn traject_id(&self) -> Uuid {
        let (id,): (Uuid,) = sqlx::query_as("SELECT id FROM trajects LIMIT 1")
            .fetch_one(&self.db.pool)
            .await
            .unwrap();
        id
    }

    async fn task(&self, article: Option<&str>) -> Uuid {
        let traject_id = self.traject_id().await;
        seed_task(
            &self.db.pool,
            self.job_id,
            self.account.id,
            traject_id,
            &self.tref,
            article,
        )
        .await
    }

    async fn etag(&self) -> String {
        law_etag(
            self.state.clone(),
            session_for(&self.sub).await,
            &self.tref,
            self.account.clone(),
        )
        .await
    }

    async fn apply(
        &self,
        headers: HeaderMap,
        decisions: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value, (StatusCode, String)> {
        let body = serde_json::from_value(serde_json::json!({ "decisions": decisions })).unwrap();
        let Json(response) = apply(
            State(self.state.clone()),
            Extension(self.account.clone()),
            session_for(&self.sub).await,
            Path(self.job_id),
            headers,
            Json(body),
        )
        .await?;
        Ok(serde_json::to_value(response).unwrap())
    }

    fn law(&self) -> String {
        read_law_file(self.corpus.path())
    }
}

// ---------------------------------------------------------------------------

#[tokio::test]
async fn job_tasks_lists_every_part_with_its_article_and_status() {
    let f = Fixture::new().await;
    let one = f.task(Some("1")).await;
    let two = f.task(Some("2")).await;

    let Json(response) = job_tasks(
        State(f.state.clone()),
        Extension(f.account.clone()),
        Path(f.job_id),
    )
    .await
    .expect("job_tasks must succeed");

    let value = serde_json::to_value(&response).unwrap();
    assert_eq!(value["law_id"], serde_json::json!(LAW_ID));
    let tasks = value["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 2);
    let ids: Vec<&str> = tasks.iter().map(|t| t["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&one.to_string().as_str()));
    assert!(ids.contains(&two.to_string().as_str()));
    let articles: Vec<&str> = tasks
        .iter()
        .map(|t| t["article"].as_str().unwrap())
        .collect();
    assert_eq!(articles, vec!["1", "2"]);
    assert!(tasks.iter().all(|t| t["status"] == "open"));
}

#[tokio::test]
async fn accepting_everything_writes_the_whole_enrichment_at_once() {
    let f = Fixture::new().await;
    let one = f.task(Some("1")).await;
    let two = f.task(Some("2")).await;
    let etag = f.etag().await;

    let response = f
        .apply(
            if_match_headers(&etag),
            vec![
                decision(
                    one,
                    "approved",
                    Some("number: '1'\ntext: artikel een verrijkt\n"),
                ),
                decision(
                    two,
                    "approved",
                    Some("number: '2'\ntext: artikel twee verrijkt\n"),
                ),
            ],
        )
        .await
        .expect("apply must succeed");

    assert_eq!(response["accepted"], serde_json::json!(2));
    assert_eq!(response["total"], serde_json::json!(2));
    let law = f.law();
    assert!(law.contains("artikel een verrijkt"), "kreeg: {law}");
    assert!(law.contains("artikel twee verrijkt"), "kreeg: {law}");
    assert_eq!(task_status(&f.db.pool, one).await, "approved");
    assert_eq!(task_status(&f.db.pool, two).await, "approved");
    // Dit was het laatste moment waarop iemand het voorstel nodig had.
    assert_eq!(blob_count(&f.db.pool, f.job_id).await, 0);
}

#[tokio::test]
async fn a_rejected_part_leaves_its_article_untouched() {
    let f = Fixture::new().await;
    let one = f.task(Some("1")).await;
    let two = f.task(Some("2")).await;
    let etag = f.etag().await;

    let response = f
        .apply(
            if_match_headers(&etag),
            vec![
                decision(
                    one,
                    "approved",
                    Some("number: '1'\ntext: artikel een verrijkt\n"),
                ),
                decision(two, "rejected", None),
            ],
        )
        .await
        .expect("apply must succeed");

    assert_eq!(response["accepted"], serde_json::json!(1));
    let law = f.law();
    assert!(law.contains("artikel een verrijkt"), "kreeg: {law}");
    assert!(law.contains("artikel twee"), "kreeg: {law}");
    assert!(!law.contains("artikel twee verrijkt"), "kreeg: {law}");
    assert_eq!(task_status(&f.db.pool, one).await, "approved");
    assert_eq!(task_status(&f.db.pool, two).await, "rejected");
}

#[tokio::test]
async fn accepting_nothing_writes_nothing_but_closes_the_parts() {
    let f = Fixture::new().await;
    let one = f.task(Some("1")).await;
    let two = f.task(Some("2")).await;
    let etag = f.etag().await;

    let response = f
        .apply(
            if_match_headers(&etag),
            vec![
                decision(one, "rejected", None),
                decision(two, "rejected", None),
            ],
        )
        .await
        .expect("apply must succeed");

    assert_eq!(response["accepted"], serde_json::json!(0));
    // Geen lege commit: de wet staat er nog bij zoals hij stond.
    assert_eq!(f.law(), SAVED_LAW);
    assert_eq!(response["etag"], serde_json::Value::Null);
    assert_eq!(task_status(&f.db.pool, one).await, "rejected");
    assert_eq!(task_status(&f.db.pool, two).await, "rejected");
    assert_eq!(blob_count(&f.db.pool, f.job_id).await, 0);
}

#[tokio::test]
async fn a_part_without_a_verdict_blocks_the_whole_enrichment() {
    let f = Fixture::new().await;
    let one = f.task(Some("1")).await;
    let two = f.task(Some("2")).await;
    let etag = f.etag().await;

    let (status, _message) = f
        .apply(
            if_match_headers(&etag),
            vec![decision(
                one,
                "approved",
                Some("number: '1'\ntext: artikel een verrijkt\n"),
            )],
        )
        .await
        .expect_err("een onvolledig oordeel hoort geweigerd te worden");

    assert_eq!(status, StatusCode::BAD_REQUEST);
    // Goedkeuren van één artikel schrijft niet meer direct.
    assert_eq!(f.law(), SAVED_LAW);
    assert_eq!(task_status(&f.db.pool, one).await, "open");
    assert_eq!(task_status(&f.db.pool, two).await, "open");
}

#[tokio::test]
async fn an_if_match_conflict_leaves_no_part_resolved() {
    let f = Fixture::new().await;
    let one = f.task(Some("1")).await;
    let two = f.task(Some("2")).await;

    let (status, _message) = f
        .apply(
            if_match_headers("\"een-verouderde-etag\""),
            vec![
                decision(
                    one,
                    "approved",
                    Some("number: '1'\ntext: artikel een verrijkt\n"),
                ),
                decision(two, "rejected", None),
            ],
        )
        .await
        .expect_err("een verouderde If-Match hoort 412 te geven");

    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(f.law(), SAVED_LAW);
    assert_eq!(task_status(&f.db.pool, one).await, "open");
    assert_eq!(task_status(&f.db.pool, two).await, "open");
    // Het voorstel blijft staan: er valt nog steeds iets te beoordelen.
    assert_eq!(blob_count(&f.db.pool, f.job_id).await, 1);
}

#[tokio::test]
async fn a_whole_law_part_is_taken_over_in_one_piece() {
    let f = Fixture::new().await;
    let whole = f.task(None).await;
    let etag = f.etag().await;

    // Geen inhoud meegestuurd: dan geldt het ruwe voorstel uit de job.
    let response = f
        .apply(
            if_match_headers(&etag),
            vec![decision(whole, "approved", None)],
        )
        .await
        .expect("apply must succeed");

    assert_eq!(response["accepted"], serde_json::json!(1));
    assert_eq!(f.law(), PROPOSAL);
    assert_eq!(task_status(&f.db.pool, whole).await, "approved");
}

/// Een whole-law-onderdeel zet de aangeleverde inhoud integraal op de plaats
/// van de wet. Dezelfde daad als de PUT, dus dezelfde poort: kapotte YAML of
/// een `$id` dat een andere wet aanwijst hoort er niet langs te komen. De
/// artikel-route heeft dat niet nodig - daar splicet de server in de
/// opgeslagen wet en blijft `$id` van hem.
#[tokio::test]
async fn een_whole_law_onderdeel_met_onbruikbare_inhoud_wordt_geweigerd() {
    for (label, content) in [
        (
            "kapotte YAML",
            "$id: wet_op_de_zorgtoeslag\narticles: [oops\n",
        ),
        (
            "ander $id",
            "$id: een_andere_wet\narticles:\n- number: '1'\n  text: x\n",
        ),
        ("geen $id", "articles:\n- number: '1'\n  text: x\n"),
    ] {
        let f = Fixture::new().await;
        let whole = f.task(None).await;
        let etag = f.etag().await;

        let (status, _message) = f
            .apply(
                if_match_headers(&etag),
                vec![decision(whole, "approved", Some(content))],
            )
            .await
            .err()
            .unwrap_or_else(|| panic!("{label} hoort geweigerd te worden"));

        assert_eq!(status, StatusCode::BAD_REQUEST, "{label}");
        assert_eq!(f.law(), SAVED_LAW, "{label}");
        assert_eq!(task_status(&f.db.pool, whole).await, "open", "{label}");
    }
}

#[tokio::test]
async fn an_article_without_content_falls_back_to_the_proposal() {
    let f = Fixture::new().await;
    let one = f.task(Some("1")).await;
    let two = f.task(Some("2")).await;
    let etag = f.etag().await;

    f.apply(
        if_match_headers(&etag),
        vec![
            decision(one, "approved", None),
            decision(two, "rejected", None),
        ],
    )
    .await
    .expect("apply must succeed");

    let law = f.law();
    assert!(law.contains("artikel een verrijkt"), "kreeg: {law}");
    assert!(!law.contains("artikel twee verrijkt"), "kreeg: {law}");
}

#[tokio::test]
async fn a_second_verwerking_of_the_same_enrichment_is_a_conflict() {
    let f = Fixture::new().await;
    let one = f.task(Some("1")).await;
    let etag = f.etag().await;

    f.apply(
        if_match_headers(&etag),
        vec![decision(one, "rejected", None)],
    )
    .await
    .expect("apply must succeed");

    let (status, _message) = f
        .apply(HeaderMap::new(), vec![decision(one, "rejected", None)])
        .await
        .expect_err("een verwerkte verrijking hoort niet nog eens te kunnen");
    assert_eq!(status, StatusCode::CONFLICT);
}

/// Zonder `If-Match` zou het verwerken een blinde overschrijving zijn. Dat is
/// precies de controle die deze flow nog heeft: de staleness-vlag per taak is
/// weg omdat er nog maar één schrijfmoment is, dus dat ene moment moet hem
/// dragen.
#[tokio::test]
async fn overnemen_zonder_if_match_schrijft_niet_en_handelt_niets_af() {
    let f = Fixture::new().await;
    let one = f.task(Some("1")).await;
    let two = f.task(Some("2")).await;

    let (status, _message) = f
        .apply(
            HeaderMap::new(),
            vec![
                decision(
                    one,
                    "approved",
                    Some("number: '1'\ntext: artikel een verrijkt\n"),
                ),
                decision(two, "rejected", None),
            ],
        )
        .await
        .expect_err("verwerken zonder If-Match hoort geweigerd te worden");

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(f.law(), SAVED_LAW);
    assert_eq!(task_status(&f.db.pool, one).await, "open");
    assert_eq!(task_status(&f.db.pool, two).await, "open");
}

/// Niets overnemen schrijft niet, en vraagt dus ook niet om een `If-Match`:
/// er is geen eindstand om tegen iets af te zetten.
#[tokio::test]
async fn niets_overnemen_mag_zonder_if_match() {
    let f = Fixture::new().await;
    let one = f.task(Some("1")).await;

    let response = f
        .apply(HeaderMap::new(), vec![decision(one, "rejected", None)])
        .await
        .expect("niets overnemen hoort te lukken");

    assert_eq!(response["accepted"], serde_json::json!(0));
    assert_eq!(f.law(), SAVED_LAW);
    assert_eq!(task_status(&f.db.pool, one).await, "rejected");
}

/// Een verrijking van iemand anders bestaat niet voor jou - niet om te lezen
/// en niet om te verwerken. De account-filter zit in de query, dus een vreemd
/// account krijgt een lege lijst en daarmee een 404, niet andermans taken.
#[tokio::test]
async fn de_verrijking_van_een_ander_account_is_onvindbaar() {
    let f = Fixture::new().await;
    let one = f.task(Some("1")).await;
    let etag = f.etag().await;
    let (intruder_id, intruder_sub) = seed_account(&f.db.pool, "mallory@test.local").await;
    let intruder = AccountRecord {
        id: intruder_id,
        person_sub: intruder_sub.clone(),
        email: "mallory@test.local".to_string(),
        name: "Test User".to_string(),
    };

    let (read_status, _) = job_tasks(
        State(f.state.clone()),
        Extension(intruder.clone()),
        Path(f.job_id),
    )
    .await
    .err()
    .expect("de onderdelen van andermans verrijking horen niet leesbaar te zijn");
    assert_eq!(read_status, StatusCode::NOT_FOUND);

    let body = serde_json::from_value(
        serde_json::json!({ "decisions": [decision(one, "approved", None)] }),
    )
    .unwrap();
    let (status, _message) = apply(
        State(f.state.clone()),
        Extension(intruder),
        session_for(&intruder_sub).await,
        Path(f.job_id),
        if_match_headers(&etag),
        Json(body),
    )
    .await
    .err()
    .expect("andermans verrijking hoort niet verwerkt te kunnen worden");

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(f.law(), SAVED_LAW);
    assert_eq!(task_status(&f.db.pool, one).await, "open");
}
