#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use http_body_util::BodyExt;
use pretty_assertions::assert_eq;
use serde_json::Value;
use tower::ServiceExt;

use regelrecht_admin::config::AppConfig;
use regelrecht_admin::handlers;
use regelrecht_admin::metrics;
use regelrecht_admin::metrics::fetch_metrics;
use regelrecht_admin::state::AppState;
use regelrecht_pipeline::job_queue::{self, CreateJobRequest};
use regelrecht_pipeline::test_utils::TestDb;
use regelrecht_pipeline::JobType;

fn test_app(pool: sqlx::PgPool) -> Router {
    let state = AppState {
        pool,
        oidc_client: None,
        end_session_url: None,
        config: Arc::new(AppConfig {
            oidc: None,
            base_url: None,
            api_key: None,
            api_key_hash: None,
            metrics_token_hash: None,
        }),
        metrics_cache: Arc::new(metrics::new_cache()),
        http_client: regelrecht_auth::http_client(),
        corpus: Arc::new(tokio::sync::RwLock::new(
            regelrecht_admin::state::CorpusState::empty(),
        )),
    };
    Router::new()
        .route("/api/law_entries", get(handlers::list_law_entries))
        .route("/api/jobs", get(handlers::list_jobs))
        .route("/api/jobs/summary", get(handlers::list_jobs_summary))
        .route("/api/untranslatables", get(handlers::list_untranslatables))
        .route("/api/markings", get(handlers::list_markings))
        .route(
            "/api/markings/clusters",
            get(handlers::list_marking_clusters),
        )
        .route("/api/dashboard-stats", get(handlers::dashboard_stats))
        .route("/api/harvest-jobs", post(handlers::create_harvest_job))
        .with_state(state)
}

/// Seed one untranslatable row for `law_id`/`provider`. Creates the law_entry
/// (so the join has a `law_name`) and an enrich job (to satisfy the FK) on first
/// use, then inserts the row. Returns nothing — tests query via the handler.
async fn seed_untranslatable(
    pool: &sqlx::PgPool,
    law_id: &str,
    law_name: &str,
    provider: &str,
    construct: &str,
    accepted: bool,
) {
    regelrecht_pipeline::law_status::upsert_law(pool, law_id, Some(law_name), None)
        .await
        .unwrap();
    let job = job_queue::create_job(pool, CreateJobRequest::new(JobType::Enrich, law_id))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO untranslatables \
         (law_id, enrich_job_id, provider, article, construct, reason, accepted) \
         VALUES ($1, $2, $3, '1', $4, 'reason', $5)",
    )
    .bind(law_id)
    .bind(job.id)
    .bind(provider)
    .bind(construct)
    .bind(accepted)
    .execute(pool)
    .await
    .unwrap();
}

async fn get_untranslatables(pool: &sqlx::PgPool, query: &str) -> Value {
    let app = test_app(pool.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/untranslatables{query}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    body_json(response).await
}

// --- markings ---

#[allow(clippy::too_many_arguments)]
async fn seed_marking(
    pool: &sqlx::PgPool,
    law_id: &str,
    law_name: &str,
    provider: &str,
    article: &str,
    about: &str,
    resolution: &str,
    resolved_by: Option<&str>,
    target: &[&str],
    accepted: bool,
) {
    regelrecht_pipeline::law_status::upsert_law(pool, law_id, Some(law_name), None)
        .await
        .unwrap();
    let job = job_queue::create_job(pool, CreateJobRequest::new(JobType::Enrich, law_id))
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO markings \
         (law_id, enrich_job_id, provider, article, about, resolution, \
          resolved_by, target, legal_text_excerpt, accepted) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'de wettekst', $9)",
    )
    .bind(law_id)
    .bind(job.id)
    .bind(provider)
    .bind(article)
    .bind(about)
    .bind(resolution)
    .bind(resolved_by)
    .bind(
        target
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<String>>(),
    )
    .bind(accepted)
    .execute(pool)
    .await
    .unwrap();
}

async fn get_markings(pool: &sqlx::PgPool, query: &str) -> Value {
    let app = test_app(pool.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/markings{query}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    body_json(response).await
}

async fn get_clusters(pool: &sqlx::PgPool, query: &str) -> Value {
    let app = test_app(pool.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/markings/clusters{query}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    // The endpoint wraps its rows in the standard {data,total} envelope; the
    // tests below are about the clusters themselves.
    body_json(response).await["data"].clone()
}

#[tokio::test]
async fn markings_list_is_empty_without_rows() {
    let db = TestDb::new().await;
    let json = get_markings(&db.pool, "").await;
    assert_eq!(json["total"], 0);
    assert_eq!(json["data"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn a_marking_comes_back_whole() {
    let db = TestDb::new().await;
    seed_marking(
        &db.pool,
        "test_wet",
        "Testwet",
        "opencode",
        "5",
        "de eerstvolgende werkdag",
        "operation",
        Some("een WORKING_DAY-bewerking"),
        &["datum_van_betaling"],
        false,
    )
    .await;

    let json = get_markings(&db.pool, "").await;
    assert_eq!(json["total"], 1);
    let row = &json["data"][0];
    assert_eq!(row["law_id"], "test_wet");
    assert_eq!(row["law_name"], "Testwet", "the LEFT JOIN must fill this");
    assert_eq!(row["about"], "de eerstvolgende werkdag");
    assert_eq!(row["resolution"], "operation");
    assert_eq!(row["resolved_by"], "een WORKING_DAY-bewerking");
    assert_eq!(row["legal_text_excerpt"], "de wettekst");
    // The array survives as an array rather than a stringified Vec.
    assert_eq!(row["target"][0], "datum_van_betaling");
    assert_eq!(row["target"].as_array().unwrap().len(), 1);
}

/// An empty `target` is a claim, not a missing value: the article stays
/// executable. It has to reach the client as `[]`, because a reader tells that
/// apart from "we do not know what is blocked".
#[tokio::test]
async fn an_empty_target_arrives_as_an_empty_array() {
    let db = TestDb::new().await;
    seed_marking(
        &db.pool,
        "test_wet",
        "Testwet",
        "opencode",
        "1",
        "iets",
        "model",
        None,
        &[],
        false,
    )
    .await;

    let json = get_markings(&db.pool, "").await;
    let row = &json["data"][0];
    assert!(row["target"].as_array().unwrap().is_empty());
    assert!(row["resolved_by"].is_null());
}

#[tokio::test]
async fn markings_filter_on_the_closed_resolution_vocabulary() {
    let db = TestDb::new().await;
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "opencode",
        "1",
        "x",
        "operation",
        None,
        &[],
        false,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_b",
        "B",
        "opencode",
        "1",
        "y",
        "model",
        None,
        &[],
        false,
    )
    .await;

    let json = get_markings(&db.pool, "?resolution=operation").await;
    assert_eq!(json["total"], 1);
    assert_eq!(json["data"][0]["resolution"], "operation");
}

#[tokio::test]
async fn markings_filter_by_law_is_a_partial_match() {
    let db = TestDb::new().await;
    seed_marking(
        &db.pool,
        "wet_op_de_zorgtoeslag",
        "Zorgtoeslag",
        "opencode",
        "1",
        "x",
        "model",
        None,
        &[],
        false,
    )
    .await;
    seed_marking(
        &db.pool,
        "participatiewet",
        "P",
        "opencode",
        "1",
        "y",
        "model",
        None,
        &[],
        false,
    )
    .await;

    let json = get_markings(&db.pool, "?law_id=zorgtoeslag").await;
    assert_eq!(json["total"], 1);
    assert_eq!(json["data"][0]["law_id"], "wet_op_de_zorgtoeslag");
}

#[tokio::test]
async fn markings_reject_a_sort_column_outside_the_allowlist() {
    let db = TestDb::new().await;
    let app = test_app(db.pool.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/markings?sort=law_name")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    // 400, not merely "not 200". The sort column is interpolated into the SQL,
    // so the allowlist is what keeps that safe; asserting `!= OK` would pass
    // just as well on the 500 an unguarded column name produces, and a test
    // that accepts the crash it is meant to prevent pins nothing.
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// The allowlist is the only thing between a query parameter and the SQL it is
/// interpolated into, so it has to reject rather than pass through. A name
/// that would be syntactically valid inside an ORDER BY is the case worth
/// proving: it cannot reach the query at all.
#[tokio::test]
async fn markings_reject_a_sort_column_that_would_be_valid_sql() {
    let db = TestDb::new().await;
    // Percent-encoded by hand rather than pulling in a dependency for four
    // literals.
    for sort in [
        "law_name",
        "1",
        "m.id%3B%20DROP%20TABLE%20markings",
        "%28SELECT%201%29",
    ] {
        let app = test_app(db.pool.clone());
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/markings?sort={sort}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::BAD_REQUEST,
            "sort={sort} must be refused, not interpolated"
        );
    }

    // And the table is still there.
    let json = get_markings(&db.pool, "").await;
    assert_eq!(json["total"], 0);
}

/// Every filter at once, which is where hand-rolled dynamic SQL goes wrong.
///
/// The placeholders are numbered while the clauses are built and the values
/// are pushed in the same step, so a desync would need the two to drift apart
/// within one block. The existing untranslatable tests each exercise a single
/// filter, so this combination was untested on either endpoint; a swapped pair
/// would have returned plausible-looking rows rather than an error.
#[tokio::test]
async fn every_filter_at_once_still_binds_in_the_right_order() {
    let db = TestDb::new().await;
    // The row that matches all five.
    seed_marking(
        &db.pool,
        "wet_op_de_zorgtoeslag",
        "Zorgtoeslag",
        "claude",
        "5",
        "de eerstvolgende werkdag",
        "operation",
        Some("een WORKING_DAY-bewerking"),
        &[],
        true,
    )
    .await;
    // Each of these differs from it in exactly one filtered field.
    seed_marking(
        &db.pool,
        "participatiewet",
        "P",
        "claude",
        "5",
        "werkdag",
        "operation",
        None,
        &[],
        true,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_op_de_zorgtoeslag",
        "Zorgtoeslag",
        "opencode",
        "5",
        "werkdag",
        "operation",
        None,
        &[],
        true,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_op_de_zorgtoeslag",
        "Zorgtoeslag",
        "claude",
        "5",
        "werkdag",
        "model",
        None,
        &[],
        true,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_op_de_zorgtoeslag",
        "Zorgtoeslag",
        "claude",
        "5",
        "iets anders",
        "operation",
        None,
        &[],
        true,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_op_de_zorgtoeslag",
        "Zorgtoeslag",
        "claude",
        "5",
        "werkdag",
        "operation",
        None,
        &[],
        false,
    )
    .await;

    let json = get_markings(
        &db.pool,
        "?law_id=zorgtoeslag&provider=claude&resolution=operation&about=werkdag&accepted=true",
    )
    .await;

    assert_eq!(json["total"], 1, "exactly the row that matches all five");
    assert_eq!(json["data"][0]["about"], "de eerstvolgende werkdag");
}

/// The same combination on the cluster endpoint, which shares the WHERE
/// builder. If the backlog grouped a different set than the list, the page
/// would state a count for rows the reader cannot see.
#[tokio::test]
async fn the_backlog_narrows_with_the_same_filters_as_the_list() {
    let db = TestDb::new().await;
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "claude",
        "1",
        "werkdag",
        "operation",
        Some("de gezochte wijziging"),
        &[],
        false,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "opencode",
        "2",
        "werkdag",
        "operation",
        Some("een andere wijziging"),
        &[],
        false,
    )
    .await;

    let list = get_markings(&db.pool, "?provider=claude&resolution=operation").await;
    let clusters = get_clusters(&db.pool, "?provider=claude&resolution=operation").await;

    assert_eq!(list["total"], 1);
    assert_eq!(
        clusters.as_array().unwrap().len(),
        1,
        "the backlog describes the same selection as the rows"
    );
    assert_eq!(clusters[0]["resolved_by"], "de gezochte wijziging");
    assert_eq!(clusters[0]["markings"], 1);
}

/// `resolved_by` is nullable, and a marking that cannot say what would fix it
/// still has to appear: it is exactly the one a reader should look at. Postgres
/// groups NULLs together, so those land in one cluster rather than vanishing.
#[tokio::test]
async fn markings_without_a_named_change_still_form_a_cluster() {
    let db = TestDb::new().await;
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "opencode",
        "1",
        "x",
        "model",
        None,
        &[],
        false,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_b",
        "B",
        "opencode",
        "1",
        "y",
        "model",
        None,
        &[],
        false,
    )
    .await;

    let json = get_clusters(&db.pool, "").await;
    let clusters = json.as_array().unwrap();
    assert_eq!(clusters.len(), 1);
    assert!(clusters[0]["resolved_by"].is_null());
    assert_eq!(clusters[0]["markings"], 2);
    assert_eq!(clusters[0]["laws"], 2);
}

/// A marking whose law has no `law_entries` row still has to appear. The join
/// is a LEFT JOIN for that reason, and an inner one would hide exactly the
/// markings worth seeing: a law that failed partway through harvesting is more
/// likely to carry gaps, not less.
///
/// Reachable in practice because `markings` has no foreign key to
/// `law_entries`, only to `jobs`.
#[tokio::test]
async fn a_marking_survives_a_missing_law_entry() {
    let db = TestDb::new().await;
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "opencode",
        "1",
        "x",
        "model",
        None,
        &[],
        false,
    )
    .await;

    // Drop the law_entries row, leaving the marking behind.
    sqlx::query("DELETE FROM law_entries WHERE law_id = $1")
        .bind("wet_a")
        .execute(&db.pool)
        .await
        .unwrap();

    let json = get_markings(&db.pool, "").await;
    assert_eq!(json["total"], 1, "the marking is still listed");
    assert!(
        json["data"][0]["law_name"].is_null(),
        "with no name to show for it"
    );
    assert_eq!(json["data"][0]["law_id"], "wet_a");
}

/// Clicking a cluster narrows the list to the markings behind it, which needs
/// an exact match on the change it names. `resolution` alone has two possible
/// values, so it would land the reader on half the corpus.
#[tokio::test]
async fn markings_filter_on_the_change_a_cluster_names() {
    let db = TestDb::new().await;
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "opencode",
        "1",
        "werkdag",
        "operation",
        Some("een WORKING_DAY-bewerking"),
        &[],
        false,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_b",
        "B",
        "opencode",
        "1",
        "jaardeel",
        "operation",
        Some("een YEAR-bewerking"),
        &[],
        false,
    )
    .await;

    let json = get_markings(&db.pool, "?resolved_by=een%20WORKING_DAY-bewerking").await;
    assert_eq!(json["total"], 1, "exact match, not a partial one");
    assert_eq!(json["data"][0]["law_id"], "wet_a");
}

// --- the backlog: clusters ---

/// The point of the whole view: the same change asked for by several articles
/// across several laws is one item of work, not four observations.
#[tokio::test]
async fn markings_that_name_the_same_change_form_one_cluster() {
    let db = TestDb::new().await;
    let same = Some("een WORKING_DAY-bewerking");
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "opencode",
        "1",
        "werkdag",
        "operation",
        same,
        &[],
        false,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "opencode",
        "2",
        "werkdag",
        "operation",
        same,
        &[],
        false,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_b",
        "B",
        "opencode",
        "7",
        "werkdag",
        "operation",
        same,
        &[],
        false,
    )
    .await;

    let json = get_clusters(&db.pool, "").await;
    let clusters = json.as_array().unwrap();
    assert_eq!(clusters.len(), 1, "one change, one cluster");
    let c = &clusters[0];
    assert_eq!(c["markings"], 3);
    assert_eq!(c["laws"], 2, "two distinct laws");
    assert_eq!(c["articles"], 3, "three distinct (law, article) pairs");
    assert_eq!(c["resolution"], "operation");
}

/// An article number repeats across laws. Counting articles by number alone
/// would read "art. 1 in two laws" as one article and undercount the reach of
/// a change, so the count is over (law, article) pairs.
#[tokio::test]
async fn the_same_article_number_in_two_laws_counts_twice() {
    let db = TestDb::new().await;
    let same = Some("dezelfde wijziging");
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "opencode",
        "1",
        "x",
        "model",
        same,
        &[],
        false,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_b",
        "B",
        "opencode",
        "1",
        "x",
        "model",
        same,
        &[],
        false,
    )
    .await;

    let json = get_clusters(&db.pool, "").await;
    assert_eq!(json[0]["articles"], 2);
    assert_eq!(json[0]["laws"], 2);
}

/// The triage signal. A change only one provider ever asks for, while another
/// ran over the same corpus without complaint, points at the enricher rather
/// than at the format. The cluster carries the providers so a reader can see
/// that difference instead of guessing at it.
#[tokio::test]
async fn a_cluster_names_the_providers_that_asked_for_it() {
    let db = TestDb::new().await;
    let same = Some("een WORKING_DAY-bewerking");
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "opencode",
        "1",
        "x",
        "operation",
        same,
        &[],
        false,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_b",
        "B",
        "claude",
        "1",
        "x",
        "operation",
        same,
        &[],
        false,
    )
    .await;

    let json = get_clusters(&db.pool, "").await;
    let providers = json[0]["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 2);
    assert!(providers.iter().any(|p| p == "opencode"));
    assert!(providers.iter().any(|p| p == "claude"));
}

/// Several markings from one provider must collapse to one entry, or the
/// triage signal inverts: `providerHint` only fires when a single provider
/// asked, so an unaggregated list of three identical names would read as three
/// providers and hide exactly the case the hint exists for.
#[tokio::test]
async fn one_provider_asking_repeatedly_stays_one_provider() {
    let db = TestDb::new().await;
    for article in ["1", "2", "3"] {
        seed_marking(
            &db.pool,
            "wet_a",
            "A",
            "opencode",
            article,
            "x",
            "operation",
            Some("dezelfde wijziging"),
            &[],
            false,
        )
        .await;
    }

    let json = get_clusters(&db.pool, "").await;
    let providers = json[0]["providers"].as_array().unwrap();
    assert_eq!(providers.len(), 1, "three markings, one provider");
    assert_eq!(providers[0], "opencode");
    assert_eq!(json[0]["markings"], 3);
}

/// `providers_on_these_laws` is what makes the triage hint mean anything: the
/// frontend only says "only opencode asked" when another provider went over
/// the same laws and did not. Until now it was exercised only against a mocked
/// value in the frontend tests, so the query that computes it was unpinned.
///
/// One provider in the corpus: every cluster has exactly one, so the hint must
/// stay quiet.
#[tokio::test]
async fn one_provider_over_these_laws_is_counted_as_one() {
    let db = TestDb::new().await;
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "opencode",
        "1",
        "x",
        "operation",
        Some("a"),
        &[],
        false,
    )
    .await;

    let json = get_clusters(&db.pool, "").await;
    assert_eq!(json[0]["providers_on_these_laws"], 1);
}

/// Two providers over the same law, each asking for something different. The
/// count is over the laws, not over the cluster, which is exactly the point: a
/// cluster of one provider only says something when another one was there.
#[tokio::test]
async fn a_second_provider_on_the_same_law_is_counted() {
    let db = TestDb::new().await;
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "opencode",
        "1",
        "x",
        "operation",
        Some("a"),
        &[],
        false,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "claude",
        "2",
        "y",
        "model",
        Some("b"),
        &[],
        false,
    )
    .await;

    let json = get_clusters(&db.pool, "").await;
    let clusters = json.as_array().unwrap();
    assert_eq!(clusters.len(), 2, "two changes, two clusters");
    for cluster in clusters {
        assert_eq!(
            cluster["providers_on_these_laws"], 2,
            "both clusters sit on a law two providers went over"
        );
        assert_eq!(
            cluster["providers"].as_array().unwrap().len(),
            1,
            "but each change was asked for by one of them"
        );
    }
}

/// A provider working on a different law must not raise the count: the
/// question is who went over *these* laws, not who ran at all. Counting every
/// provider in the corpus would turn the hint on everywhere the moment a
/// second one exists.
#[tokio::test]
async fn a_provider_on_another_law_does_not_count() {
    let db = TestDb::new().await;
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "opencode",
        "1",
        "x",
        "operation",
        Some("a"),
        &[],
        false,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_b",
        "B",
        "claude",
        "1",
        "y",
        "model",
        Some("b"),
        &[],
        false,
    )
    .await;

    let json = get_clusters(&db.pool, "").await;
    for cluster in json.as_array().unwrap() {
        assert_eq!(
            cluster["providers_on_these_laws"], 1,
            "each cluster sits on a law only its own provider touched"
        );
    }
}

/// Reach decides the order, and reach means laws before sheer count: five
/// markings inside one law may be that law's peculiarity, while three spread
/// over three laws is a property of the format.
#[tokio::test]
async fn clusters_are_ordered_by_how_far_they_reach() {
    let db = TestDb::new().await;
    // Narrow: four markings, one law.
    for article in ["1", "2", "3", "4"] {
        seed_marking(
            &db.pool,
            "wet_a",
            "A",
            "opencode",
            article,
            "x",
            "model",
            Some("smalle wijziging"),
            &[],
            false,
        )
        .await;
    }
    // Wide: three markings, three laws.
    for (law, name) in [("wet_b", "B"), ("wet_c", "C"), ("wet_d", "D")] {
        seed_marking(
            &db.pool,
            law,
            name,
            "opencode",
            "1",
            "y",
            "operation",
            Some("brede wijziging"),
            &[],
            false,
        )
        .await;
    }

    let json = get_clusters(&db.pool, "").await;
    assert_eq!(
        json[0]["resolved_by"], "brede wijziging",
        "three laws outranks four markings in one law"
    );
    assert_eq!(json[0]["laws"], 3);
    assert_eq!(json[1]["resolved_by"], "smalle wijziging");
    assert_eq!(json[1]["markings"], 4);
}

/// Free text does not cluster itself. Two agents asking for the same operation
/// in different words land in two clusters, and that is visible rather than
/// hidden: a cluster of one is what a reader should look at twice. Closing
/// this needs grouping that understands the words, which needs real markings
/// to calibrate against.
#[tokio::test]
async fn differently_worded_requests_do_not_merge_yet() {
    let db = TestDb::new().await;
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "opencode",
        "1",
        "werkdag",
        "operation",
        Some("Een WORKING_DAY-bewerking die weekenden overslaat"),
        &[],
        false,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_b",
        "B",
        "opencode",
        "1",
        "werkdag",
        "operation",
        Some("Een bewerking die de eerstvolgende werkdag bepaalt"),
        &[],
        false,
    )
    .await;

    let json = get_clusters(&db.pool, "").await;
    assert_eq!(
        json.as_array().unwrap().len(),
        2,
        "same need, two wordings, two clusters — the known limit of exact grouping"
    );
}

/// The cluster view answers the same question as the list above it, so a
/// filter has to reach both. Otherwise the backlog describes a different set
/// of markings than the rows a reader is looking at.
#[tokio::test]
async fn a_filter_reaches_the_clusters_too() {
    let db = TestDb::new().await;
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "opencode",
        "1",
        "x",
        "operation",
        Some("a"),
        &[],
        false,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_b",
        "B",
        "claude",
        "1",
        "y",
        "model",
        Some("b"),
        &[],
        false,
    )
    .await;

    let json = get_clusters(&db.pool, "?provider=claude").await;
    let clusters = json.as_array().unwrap();
    assert_eq!(clusters.len(), 1);
    assert_eq!(clusters[0]["resolved_by"], "b");
}

/// `all_accepted` says whether a human has been past every marking in the
/// cluster. A cluster nobody has reviewed reads differently from one that has
/// been weighed and accepted as a real gap.
#[tokio::test]
async fn a_cluster_reports_whether_all_its_markings_are_accepted() {
    let db = TestDb::new().await;
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "opencode",
        "1",
        "x",
        "model",
        Some("a"),
        &[],
        true,
    )
    .await;
    seed_marking(
        &db.pool,
        "wet_a",
        "A",
        "opencode",
        "2",
        "x",
        "model",
        Some("a"),
        &[],
        false,
    )
    .await;

    let json = get_clusters(&db.pool, "").await;
    assert_eq!(json[0]["all_accepted"], false, "one is still unreviewed");
}

/// The positive path, which the mixed case above cannot reach: with only that
/// test, an implementation that always answered `false` would pass, and the
/// backlog would never mark a cluster as weighed and accepted.
#[tokio::test]
async fn a_cluster_whose_markings_are_all_accepted_says_so() {
    let db = TestDb::new().await;
    for article in ["1", "2"] {
        seed_marking(
            &db.pool,
            "wet_a",
            "A",
            "opencode",
            article,
            "x",
            "model",
            Some("a"),
            &[],
            true,
        )
        .await;
    }

    let json = get_clusters(&db.pool, "").await;
    assert_eq!(json[0]["all_accepted"], true);
}

// --- list_untranslatables ---

#[tokio::test]
async fn list_untranslatables_empty() {
    let db = TestDb::new().await;
    let json = get_untranslatables(&db.pool, "").await;
    assert_eq!(json["total"], 0);
    assert_eq!(json["data"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn list_untranslatables_returns_rows_with_law_name() {
    let db = TestDb::new().await;
    seed_untranslatable(
        &db.pool, "test_law", "Test Law", "opencode", "rounding", false,
    )
    .await;
    seed_untranslatable(
        &db.pool,
        "test_law",
        "Test Law",
        "opencode",
        "table_lookup",
        true,
    )
    .await;

    let json = get_untranslatables(&db.pool, "").await;
    assert_eq!(json["total"], 2);
    // law_name comes from the LEFT JOIN on law_entries.
    assert_eq!(json["data"][0]["law_name"], "Test Law");
}

#[tokio::test]
async fn list_untranslatables_filters_by_accepted() {
    let db = TestDb::new().await;
    seed_untranslatable(
        &db.pool, "test_law", "Test Law", "opencode", "rounding", false,
    )
    .await;
    seed_untranslatable(
        &db.pool,
        "test_law",
        "Test Law",
        "opencode",
        "table_lookup",
        true,
    )
    .await;

    let json = get_untranslatables(&db.pool, "?accepted=true").await;
    assert_eq!(json["total"], 1);
    assert_eq!(json["data"][0]["construct"], "table_lookup");
}

#[tokio::test]
async fn list_untranslatables_filters_by_law_id_partial() {
    let db = TestDb::new().await;
    seed_untranslatable(
        &db.pool,
        "wet_alpha",
        "Wet Alpha",
        "opencode",
        "rounding",
        false,
    )
    .await;
    seed_untranslatable(
        &db.pool, "wet_beta", "Wet Beta", "opencode", "rounding", false,
    )
    .await;

    let json = get_untranslatables(&db.pool, "?law_id=alpha").await;
    assert_eq!(json["total"], 1);
    assert_eq!(json["data"][0]["law_id"], "wet_alpha");
}

#[tokio::test]
async fn list_untranslatables_filters_by_provider() {
    let db = TestDb::new().await;
    seed_untranslatable(
        &db.pool, "test_law", "Test Law", "opencode", "rounding", false,
    )
    .await;
    seed_untranslatable(
        &db.pool, "test_law", "Test Law", "claude", "rounding", false,
    )
    .await;

    let json = get_untranslatables(&db.pool, "?provider=claude").await;
    assert_eq!(json["total"], 1);
    assert_eq!(json["data"][0]["provider"], "claude");
}

#[tokio::test]
async fn list_untranslatables_rejects_invalid_sort() {
    let db = TestDb::new().await;
    let app = test_app(db.pool.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/untranslatables?sort=bogus")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_untranslatables_pagination() {
    let db = TestDb::new().await;
    for construct in ["a", "b", "c"] {
        seed_untranslatable(
            &db.pool, "test_law", "Test Law", "opencode", construct, false,
        )
        .await;
    }

    let json = get_untranslatables(&db.pool, "?limit=1&offset=1").await;
    assert_eq!(json["total"], 3);
    assert_eq!(json["data"].as_array().unwrap().len(), 1);
}

async fn body_json(response: axum::http::Response<Body>) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

// --- create_harvest_job ---

#[tokio::test]
async fn create_harvest_job_returns_created() {
    let db = TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "BWBR0018451"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let json = body_json(response).await;
    assert_eq!(json["law_id"], "BWBR0018451");
    assert!(json["job_id"].as_str().is_some());
}

#[tokio::test]
async fn create_harvest_job_links_harvest_job_id() {
    let db = TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "BWBR0018451"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let json = body_json(response).await;
    let job_id: uuid::Uuid = json["job_id"].as_str().unwrap().parse().unwrap();

    // Verify the law entry has the harvest_job_id linked
    let row: (Option<uuid::Uuid>,) =
        sqlx::query_as("SELECT harvest_job_id FROM law_entries WHERE law_id = $1")
            .bind("BWBR0018451")
            .fetch_one(&db.pool)
            .await
            .unwrap();

    assert_eq!(row.0, Some(job_id));
}

#[tokio::test]
async fn create_harvest_job_rejects_duplicate() {
    let db = TestDb::new().await;
    let pool = db.pool.clone();
    let app = test_app(pool.clone());

    // First request succeeds
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "BWBR0018451"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Second request for same law_id should be rejected
    let app2 = test_app(pool.clone());
    let response = app2
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "BWBR0018451"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn create_harvest_job_allows_after_completion() {
    let db = TestDb::new().await;
    let pool = db.pool.clone();
    let app = test_app(pool.clone());

    // Create first job
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "BWBR0018451"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let json = body_json(response).await;
    let job_id: uuid::Uuid = json["job_id"].as_str().unwrap().parse().unwrap();

    // Simulate job completion: claim then complete
    let claimed = job_queue::claim_job(&pool, Some(JobType::Harvest))
        .await
        .unwrap()
        .unwrap();
    job_queue::complete_job(&pool, claimed.id, None)
        .await
        .unwrap();
    assert_eq!(claimed.id, job_id);

    // Now creating another harvest job for the same law_id should succeed
    let app2 = test_app(pool.clone());
    let response = app2
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "BWBR0018451"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn create_harvest_job_rejects_empty_bwb_id() {
    let db = TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "  "}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_harvest_job_with_priority_and_date() {
    let db = TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"bwb_id": "BWBR0018451", "priority": 80, "date": "2024-01-01"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    // Verify priority was set on the job
    let job: (i32,) = sqlx::query_as("SELECT priority FROM jobs WHERE law_id = $1")
        .bind("BWBR0018451")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(job.0, 80);
}

#[tokio::test]
async fn create_harvest_job_rejects_invalid_bwb_id() {
    let db = TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "INVALID"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_harvest_job_rejects_invalid_date() {
    let db = TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"bwb_id": "BWBR0018451", "date": "not-a-date"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_harvest_job_rejects_impossible_date() {
    let db = TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"bwb_id": "BWBR0018451", "date": "2025-13-01"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_harvest_job_rejects_exhausted_law() {
    let db = TestDb::new().await;
    let app = test_app(db.pool.clone());

    regelrecht_pipeline::law_status::upsert_law(&db.pool, "BWBR0018451", None, None)
        .await
        .unwrap();
    regelrecht_pipeline::law_status::update_status(
        &db.pool,
        "BWBR0018451",
        regelrecht_pipeline::LawStatusValue::HarvestExhausted,
    )
    .await
    .unwrap();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "BWBR0018451"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CONFLICT);
}

// --- list endpoints ---

#[tokio::test]
async fn list_jobs_empty() {
    let db = TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/jobs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["total"], 0);
    assert_eq!(json["data"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn list_law_entries_empty() {
    let db = TestDb::new().await;
    let app = test_app(db.pool.clone());

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/law_entries")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["total"], 0);
}

#[tokio::test]
async fn list_jobs_after_creation() {
    let db = TestDb::new().await;
    let pool = db.pool.clone();

    // Create a job via the handler
    let app = test_app(pool.clone());
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/harvest-jobs")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"bwb_id": "BWBR0018451"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // List jobs
    let app2 = test_app(pool.clone());
    let response = app2
        .oneshot(
            Request::builder()
                .uri("/api/jobs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let json = body_json(response).await;
    assert_eq!(json["total"], 1);
    assert_eq!(json["data"][0]["law_id"], "BWBR0018451");
}

// --- fetch_metrics ---

#[tokio::test]
async fn fetch_metrics_on_empty_db() {
    let db = TestDb::new().await;
    let snapshot = fetch_metrics(&db.pool).await.unwrap();

    assert!(snapshot.jobs_by_status.is_empty());
    assert!(snapshot.laws_by_status.is_empty());
    assert_eq!(snapshot.avg_job_duration_secs, None);
}

#[tokio::test]
async fn fetch_metrics_with_only_pending_jobs() {
    let db = TestDb::new().await;
    let pool = db.pool.clone();

    // Create a pending job but don't complete it - AVG query returns NULL
    // because no jobs match status='completed'. This is the scenario that
    // triggered a NUMERIC vs float8 mismatch in production.
    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0018451");
    job_queue::create_job(&pool, req).await.unwrap();

    let snapshot = fetch_metrics(&pool).await.unwrap();

    assert_eq!(snapshot.avg_job_duration_secs, None);
    assert!(
        snapshot
            .jobs_by_status
            .iter()
            .any(|(s, c)| s == "pending" && *c == 1),
        "should have 1 pending job"
    );
}

#[tokio::test]
async fn fetch_metrics_avg_duration_with_completed_jobs() {
    let db = TestDb::new().await;
    let pool = db.pool.clone();

    // Create and complete a job so AVG(EXTRACT(EPOCH ...)) returns a value.
    let req = CreateJobRequest::new(JobType::Harvest, "BWBR0018451");
    job_queue::create_job(&pool, req).await.unwrap();
    let job = job_queue::claim_job(&pool, Some(JobType::Harvest))
        .await
        .unwrap()
        .unwrap();
    job_queue::complete_job(&pool, job.id, None).await.unwrap();

    let snapshot = fetch_metrics(&pool).await.unwrap();

    assert!(
        snapshot.avg_job_duration_secs.is_some(),
        "should have avg duration for completed jobs"
    );
    assert!(
        snapshot
            .jobs_by_status
            .iter()
            .any(|(s, c)| s == "completed" && *c == 1),
        "should have 1 completed job"
    );
}

// --- dashboard_stats ---

/// Fetch the dashboard stats through the handler and return the parsed JSON.
async fn get_dashboard_stats(pool: &sqlx::PgPool) -> Value {
    let app = test_app(pool.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/dashboard-stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    body_json(response).await
}

async fn get_jobs_summary(pool: &sqlx::PgPool) -> Value {
    let app = test_app(pool.clone());
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/jobs/summary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    body_json(response).await
}

/// Force a job into `failed` with a given result JSON and completion time.
async fn mark_job_failed(pool: &sqlx::PgPool, job_id: uuid::Uuid, result: Value, completed: &str) {
    sqlx::query(
        "UPDATE jobs SET status = 'failed', result = $2, \
         completed_at = $3::timestamptz WHERE id = $1",
    )
    .bind(job_id)
    .bind(result)
    .bind(completed)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn dashboard_stats_on_empty_db() {
    let db = TestDb::new().await;
    let json = get_dashboard_stats(&db.pool).await;

    assert_eq!(json["jobs"]["total"], 0);
    assert_eq!(json["jobs"]["by_status"]["pending"], 0);
    assert_eq!(json["jobs"]["by_type"]["harvest"], 0);
    assert_eq!(json["executed"]["today"]["total"], 0);
    assert_eq!(json["open_untranslatables"], 0);
    assert_eq!(json["recent_failures"].as_array().unwrap().len(), 0);
    // The daily series always spans 14 days, all zeros on an empty DB.
    let daily = json["daily"].as_array().unwrap();
    assert_eq!(daily.len(), 14);
    assert!(daily.iter().all(|d| {
        ["harvest", "enrich"].iter().all(|t| {
            ["added", "succeeded", "failed"]
                .iter()
                .all(|k| d[t][k] == 0)
        })
    }));
}

#[tokio::test]
async fn dashboard_stats_counts_by_type_and_status() {
    let db = TestDb::new().await;
    let pool = db.pool.clone();

    // Two pending harvest jobs, one pending enrich job.
    job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "BWBR0000001"),
    )
    .await
    .unwrap();
    job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "BWBR0000002"),
    )
    .await
    .unwrap();
    job_queue::create_job(&pool, CreateJobRequest::new(JobType::Enrich, "BWBR0000003"))
        .await
        .unwrap();

    // Complete the enrich one.
    let claimed = job_queue::claim_job(&pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    job_queue::complete_job(&pool, claimed.id, None)
        .await
        .unwrap();

    // And one document-convert job, a type the dashboard used to drop into a
    // warn! arm: its row existed, counted nowhere, and made `total` smaller
    // than the number of rows in `jobs`.
    job_queue::create_job(
        &pool,
        CreateJobRequest::new(
            JobType::DocumentConvert,
            "doc:zorgtoeslag-ab12cd34/notitie.md",
        ),
    )
    .await
    .unwrap();

    let json = get_dashboard_stats(&pool).await;

    assert_eq!(json["jobs"]["total"], 4);
    assert_eq!(json["jobs"]["by_type"]["harvest"], 2);
    assert_eq!(json["jobs"]["by_type"]["enrich"], 1);
    assert_eq!(json["jobs"]["by_type"]["document_convert"], 1);
    assert_eq!(json["jobs"]["by_status"]["pending"], 3);
    assert_eq!(json["jobs"]["by_status"]["completed"], 1);
    assert_eq!(json["jobs"]["by_type_status"]["harvest"]["pending"], 2);
    assert_eq!(json["jobs"]["by_type_status"]["enrich"]["completed"], 1);
    assert_eq!(json["jobs"]["by_type_status"]["enrich"]["pending"], 0);
    assert_eq!(
        json["jobs"]["by_type_status"]["document_convert"]["pending"],
        1
    );

    // `total` is the sum over the buckets, and the buckets cover the whole
    // enum: no row in `jobs` may fall outside them.
    let by_type = json["jobs"]["by_type"].as_object().unwrap();
    let summed: i64 = by_type.values().map(|v| v.as_i64().unwrap()).sum();
    assert_eq!(summed, json["jobs"]["total"].as_i64().unwrap());
    for job_type in regelrecht_pipeline::JobType::ALL {
        assert!(
            by_type.contains_key(job_type.as_str()),
            "job type {} has no bucket in by_type",
            job_type.as_str()
        );
    }

    // The daily series carries the same buckets; it used to pivot on two
    // hardcoded types inside the SQL.
    let today = json["daily"].as_array().unwrap().last().unwrap();
    assert_eq!(today["document_convert"]["added"], 1);
    assert_eq!(today["harvest"]["added"], 2);
}

/// The law-grouped overview must leave out the job types whose `law_id` is a
/// synthetic key: an uploaded document is not a law (#1162).
#[tokio::test]
async fn jobs_summary_excludes_synthetic_law_ids() {
    let db = TestDb::new().await;
    let pool = db.pool.clone();

    job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "wet_op_de_zorgtoeslag"),
    )
    .await
    .unwrap();
    job_queue::create_job(
        &pool,
        CreateJobRequest::new(
            JobType::DocumentConvert,
            "doc:zorgtoeslag-ab12cd34/notitie.md",
        ),
    )
    .await
    .unwrap();
    job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::LawConvert, "lawdoc:zorgtoeslag-ab12cd34/wet.pdf"),
    )
    .await
    .unwrap();

    let json = get_jobs_summary(&pool).await;

    assert_eq!(json["total"], 1, "only the harvest job names a law");
    let ids: Vec<&str> = json["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["law_id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["wet_op_de_zorgtoeslag"]);
}

#[tokio::test]
async fn dashboard_stats_executed_windows_use_effective_timestamp() {
    let db = TestDb::new().await;
    let pool = db.pool.clone();

    // A pending job that was created today counts via created_at (it has no
    // completed_at yet).
    job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "BWBR0000010"),
    )
    .await
    .unwrap();

    // A completed enrich job finished today.
    let enrich =
        job_queue::create_job(&pool, CreateJobRequest::new(JobType::Enrich, "BWBR0000011"))
            .await
            .unwrap();
    let claimed = job_queue::claim_job(&pool, Some(JobType::Enrich))
        .await
        .unwrap()
        .unwrap();
    job_queue::complete_job(&pool, claimed.id, None)
        .await
        .unwrap();
    assert_eq!(enrich.job_type, JobType::Enrich);

    // A completed harvest job created AND completed 10 days ago: outside both
    // "today" and "last 7 days".
    let old = job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "BWBR0000012"),
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE jobs SET status = 'completed', \
         created_at = now() - interval '10 days', \
         completed_at = now() - interval '10 days' WHERE id = $1",
    )
    .bind(old.id)
    .execute(&pool)
    .await
    .unwrap();

    let json = get_dashboard_stats(&pool).await;

    // today: the pending harvest (via created_at) + the completed enrich.
    assert_eq!(json["executed"]["today"]["total"], 2);
    assert_eq!(json["executed"]["today"]["harvest"], 1);
    assert_eq!(json["executed"]["today"]["enrich"], 1);
    // last 7 days: same two; the 10-day-old harvest is excluded.
    assert_eq!(json["executed"]["last_7d"]["total"], 2);
    assert_eq!(json["executed"]["last_7d"]["harvest"], 1);
}

#[tokio::test]
async fn dashboard_stats_open_untranslatables_counts_only_unaccepted() {
    let db = TestDb::new().await;

    seed_untranslatable(&db.pool, "wet_a", "Wet A", "opencode", "rounding", false).await;
    seed_untranslatable(&db.pool, "wet_b", "Wet B", "opencode", "rounding", false).await;
    seed_untranslatable(&db.pool, "wet_c", "Wet C", "opencode", "rounding", true).await;

    let json = get_dashboard_stats(&db.pool).await;

    assert_eq!(json["open_untranslatables"], 2);
}

#[tokio::test]
async fn dashboard_stats_recent_failures_carry_reason() {
    let db = TestDb::new().await;
    let pool = db.pool.clone();

    let older = job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "BWBR0000020"),
    )
    .await
    .unwrap();
    let newer = job_queue::create_job(&pool, CreateJobRequest::new(JobType::Enrich, "BWBR0000021"))
        .await
        .unwrap();

    mark_job_failed(
        &pool,
        older.id,
        serde_json::json!({ "error": "harvest boom" }),
        "2026-07-01T10:00:00Z",
    )
    .await;
    mark_job_failed(
        &pool,
        newer.id,
        serde_json::json!({ "error": "job timed out after 300s" }),
        "2026-07-03T10:00:00Z",
    )
    .await;

    let json = get_dashboard_stats(&pool).await;

    assert_eq!(json["jobs"]["by_status"]["failed"], 2);
    let failures = json["recent_failures"].as_array().unwrap();
    assert_eq!(failures.len(), 2);
    // Ordered by failed_at DESC — the newer failure comes first.
    assert_eq!(failures[0]["law_id"], "BWBR0000021");
    assert_eq!(failures[0]["job_type"], "enrich");
    assert_eq!(failures[0]["error"], "job timed out after 300s");
    assert_eq!(failures[1]["law_id"], "BWBR0000020");
    assert_eq!(failures[1]["error"], "harvest boom");
}

#[tokio::test]
async fn dashboard_stats_failure_reason_falls_back_when_no_error_key() {
    let db = TestDb::new().await;
    let pool = db.pool.clone();

    // A failed job whose result JSON lacks an `error` key falls back to the
    // placeholder rather than serialising null.
    let job = job_queue::create_job(
        &pool,
        CreateJobRequest::new(JobType::Harvest, "BWBR0000030"),
    )
    .await
    .unwrap();
    mark_job_failed(
        &pool,
        job.id,
        serde_json::json!({ "note": "something else" }),
        "2026-07-02T10:00:00Z",
    )
    .await;

    let json = get_dashboard_stats(&pool).await;
    let failures = json["recent_failures"].as_array().unwrap();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0]["error"], "onbekend");
}

/// Inserts a job and pins its timestamps, so a test controls which calendar day
/// it falls on instead of inheriting the moment the test happens to run.
async fn insert_job_at(
    pool: &sqlx::PgPool,
    job_type: JobType,
    law_id: &str,
    status: &str,
    created_at: &str,
    completed_at: Option<&str>,
) {
    let job = job_queue::create_job(pool, CreateJobRequest::new(job_type, law_id))
        .await
        .unwrap();
    sqlx::query(
        "UPDATE jobs SET status = $2::job_status, \
         created_at = $3::timestamptz, completed_at = $4::timestamptz WHERE id = $1",
    )
    .bind(job.id)
    .bind(status)
    .bind(created_at)
    .bind(completed_at)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn dashboard_stats_daily_series_fourteen_days() {
    let db = TestDb::new().await;
    let pool = db.pool.clone();

    // A fixed moment late in the Amsterdam evening: 22:17 UTC on the Monday
    // after the switch to winter time is 23:17 in Amsterdam, and the 14-day
    // window (13 to 26 October) spans that switch. The CI failure this replaces
    // came from this hour: fixtures built as `now() - interval '5 days'` are 120
    // hours back, which crosses the switch and lands one Amsterdam day off.
    // Every timestamp below is pinned instead.
    let as_of: chrono::DateTime<chrono::Utc> = "2026-10-26T22:17:00Z".parse().unwrap();

    // Today (index 13). 23:30 UTC on the 25th is 00:30 on the 26th in Amsterdam:
    // a UTC grouping would put this job on index 12.
    insert_job_at(
        &pool,
        JobType::Harvest,
        "BWBR0000040",
        "pending",
        "2026-10-25T23:30:00Z",
        None,
    )
    .await;
    // Added and succeeded today, late in the Amsterdam evening.
    insert_job_at(
        &pool,
        JobType::Enrich,
        "BWBR0000041",
        "completed",
        "2026-10-26T22:05:00Z",
        Some("2026-10-26T22:10:00Z"),
    )
    .await;

    // Added five Amsterdam days ago, failed three Amsterdam days ago. Both fall
    // just past midnight in summer time (UTC+2), so the UTC date is a day
    // earlier each time.
    insert_job_at(
        &pool,
        JobType::Harvest,
        "BWBR0000042",
        "failed",
        "2026-10-20T22:30:00Z",       // 21 Oct 00:30 Amsterdam → index 8
        Some("2026-10-22T22:15:00Z"), // 23 Oct 00:15 Amsterdam → index 10
    )
    .await;

    // The first day of the window starts at Amsterdam midnight: one second before
    // it is outside, midnight itself is inside (index 0).
    insert_job_at(
        &pool,
        JobType::Harvest,
        "BWBR0000043",
        "pending",
        "2026-10-12T21:59:59Z", // 12 Oct 23:59:59 Amsterdam
        None,
    )
    .await;
    insert_job_at(
        &pool,
        JobType::Harvest,
        "BWBR0000044",
        "pending",
        "2026-10-12T22:00:00Z", // 13 Oct 00:00 Amsterdam
        None,
    )
    .await;

    // Created and completed 20 days ago → outside the window.
    insert_job_at(
        &pool,
        JobType::Harvest,
        "BWBR0000045",
        "completed",
        "2026-10-06T12:00:00Z",
        Some("2026-10-06T12:00:00Z"),
    )
    .await;

    let daily = serde_json::to_value(handlers::daily_series(&pool, as_of).await.unwrap()).unwrap();
    let daily = daily.as_array().unwrap();
    assert_eq!(daily.len(), 14);

    // Dates ascend and run over Amsterdam calendar days, ending on the day of
    // `as_of` in Amsterdam (the UTC date is the same here; 23:30 UTC would not be).
    assert_eq!(daily[0]["date"], "2026-10-13");
    assert_eq!(daily[13]["date"], "2026-10-26");
    for pair in daily.windows(2) {
        assert!(pair[0]["date"].as_str().unwrap() < pair[1]["date"].as_str().unwrap());
    }

    // Today (index 13): 1 harvest added, 1 enrich added + succeeded. Nothing on
    // index 12, where the UTC date of the harvest job would have put it.
    assert_eq!(daily[13]["harvest"]["added"], 1);
    assert_eq!(daily[13]["harvest"]["succeeded"], 0);
    assert_eq!(daily[13]["harvest"]["failed"], 0);
    assert_eq!(daily[13]["enrich"]["added"], 1);
    assert_eq!(daily[13]["enrich"]["succeeded"], 1);
    assert_eq!(daily[12]["harvest"]["added"], 0);

    // The failed harvest: added on index 8, failed on index 10, and not on the
    // neighbouring days its UTC dates point at.
    assert_eq!(daily[8]["harvest"]["added"], 1);
    assert_eq!(daily[7]["harvest"]["added"], 0);
    assert_eq!(daily[10]["harvest"]["failed"], 1);
    assert_eq!(daily[9]["harvest"]["failed"], 0);

    // Window start: only the job at Amsterdam midnight counts.
    assert_eq!(daily[0]["harvest"]["added"], 1);

    // Today's, the failed one and the one at window start; the job a second
    // before the window and the 20-day-old job appear nowhere.
    let total_harvest_added: i64 = daily
        .iter()
        .map(|d| d["harvest"]["added"].as_i64().unwrap())
        .sum();
    assert_eq!(total_harvest_added, 3);
}
