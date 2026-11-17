//! Beheer van het partijregister — de registratietaak van de Napp.
//!
//! Het register bevat de koppeling rechtspersoon (KvK) ↔ geregistreerde
//! aanduiding, met organisatiemodel en moederpartij. De verkiezingsuitslagen
//! en inwoneraantallen zijn referentiedata uit authentieke bronnen
//! (Kiesraad, CBS): die zijn hier alleen te raadplegen, nooit te muteren —
//! correcties volgen de bron. Nieuwe koppelingen ontstaan via een claim bij
//! de eerste aanvraag, niet via handmatige invoer.
//!
//! Alle endpoints zijn strikt beoordelaar-only: zonder beoordelaarsessie
//! volgt 403, hetzelfde autorisatiepatroon als `handlers::list_aanvragen`.

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;

use crate::handlers::{
    ApiError, bad_request, forbidden, internal_error, not_found, session_beoordelaar,
};
use crate::register;
use crate::state::AppState;

fn validate_organisatiemodel(model: &str) -> Result<String, ApiError> {
    let model = model.trim().to_uppercase();
    if model != "CENTRAAL" && model != "DECENTRAAL" {
        return Err(bad_request(
            "Organisatiemodel moet CENTRAAL of DECENTRAAL zijn.",
        ));
    }
    Ok(model)
}

fn validate_naam(naam: &str) -> Result<String, ApiError> {
    let naam = naam.trim().to_string();
    if naam.is_empty() {
        return Err(bad_request("Naam is verplicht."));
    }
    Ok(naam)
}

/// Normalizes and validates an optional moederpartij reference: it must be
/// an existing registration and may not point at the party itself.
async fn validate_moederpartij(
    state: &AppState,
    eigen_kvk: &str,
    moederpartij_kvk: Option<&str>,
) -> Result<Option<String>, ApiError> {
    let Some(moeder) = moederpartij_kvk.map(str::trim).filter(|m| !m.is_empty()) else {
        return Ok(None);
    };
    if moeder == eigen_kvk {
        return Err(bad_request(
            "Een partij kan niet haar eigen moederpartij zijn.",
        ));
    }
    if register::partij_by_kvk(&state.pool, moeder)
        .await
        .map_err(internal_error)?
        .is_none()
    {
        return Err(bad_request(&format!(
            "Moederpartij met KvK-nummer {moeder} staat niet in het register."
        )));
    }
    Ok(Some(moeder.to_string()))
}

// ---------------------------------------------------------------------------
// Partijen (koppelingen)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct ZoekParams {
    #[serde(default)]
    pub zoek: Option<String>,
    #[serde(default)]
    pub offset: Option<i64>,
    #[serde(default)]
    pub limit: Option<i64>,
}

/// GET /api/beheer/partijen?zoek=&offset=&limit=
pub async fn list_partijen(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<ZoekParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if session_beoordelaar(&session).await.is_none() {
        return Err(forbidden());
    }
    let zoek = params.zoek.unwrap_or_default();
    let offset = params.offset.unwrap_or(0).max(0);
    let limit = params.limit.unwrap_or(25).clamp(1, 100);
    let (totaal, partijen) = register::zoek_partijen(&state.pool, &zoek, offset, limit)
        .await
        .map_err(internal_error)?;
    Ok(Json(json!({
        "totaal": totaal,
        "offset": offset,
        "limit": limit,
        "partijen": partijen,
    })))
}

async fn partij_detail(state: &AppState, kvk: &str) -> Result<Option<serde_json::Value>, ApiError> {
    let Some(partij) = register::partij_by_kvk(&state.pool, kvk)
        .await
        .map_err(internal_error)?
    else {
        return Ok(None);
    };
    let uitslagen = register::uitslagen_met_gebied(&state.pool, kvk)
        .await
        .map_err(internal_error)?;
    let moederpartij_naam = match &partij.moederpartij_kvk {
        Some(moeder) => register::partij_by_kvk(&state.pool, moeder)
            .await
            .map_err(internal_error)?
            .map(|p| p.naam),
        None => None,
    };
    Ok(Some(json!({
        "kvk_nummer": partij.kvk_nummer,
        "naam": partij.naam,
        "organisatiemodel": partij.organisatiemodel,
        "kamerzetels": partij.kamerzetels,
        "moederpartij_kvk": partij.moederpartij_kvk,
        "moederpartij_naam": moederpartij_naam,
        "status": partij.status,
        "uitslagen": uitslagen,
    })))
}

/// GET /api/beheer/partijen/{kvk} — detail incl. uitslagen (read-only
/// referentiedata) met gebied-namen en inwoneraantallen.
pub async fn get_partij(
    State(state): State<AppState>,
    session: Session,
    Path(kvk): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if session_beoordelaar(&session).await.is_none() {
        return Err(forbidden());
    }
    match partij_detail(&state, &kvk).await? {
        Some(detail) => Ok(Json(detail)),
        None => Err(not_found()),
    }
}

#[derive(Debug, Deserialize)]
pub struct PartijWijziging {
    pub naam: String,
    pub organisatiemodel: String,
    #[serde(default)]
    pub moederpartij_kvk: Option<String>,
}

/// PUT /api/beheer/partijen/{kvk} — de koppeling wijzigen:
/// aanduiding/organisatiemodel/moederpartij.
pub async fn update_partij(
    State(state): State<AppState>,
    session: Session,
    Path(kvk): Path<String>,
    Json(body): Json<PartijWijziging>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if session_beoordelaar(&session).await.is_none() {
        return Err(forbidden());
    }
    let naam = validate_naam(&body.naam)?;
    let model = validate_organisatiemodel(&body.organisatiemodel)?;
    let moeder = validate_moederpartij(&state, &kvk, body.moederpartij_kvk.as_deref()).await?;
    let updated = register::update_partij(&state.pool, &kvk, &naam, &model, moeder.as_deref())
        .await
        .map_err(internal_error)?;
    if !updated {
        return Err(not_found());
    }
    match partij_detail(&state, &kvk).await? {
        Some(detail) => Ok(Json(detail)),
        None => Err(not_found()),
    }
}

// ---------------------------------------------------------------------------
// Demo-reset
// ---------------------------------------------------------------------------

/// POST /api/beheer/demo/reset — maak alle dossiers leeg en herseed het
/// partijregister uit de snapshot. Alleen geregistreerd in demo-mode (zelfde
/// schakelaar als de mock-logins, zie main.rs): dit is gereedschap om de
/// demo-omgeving herhaalbaar te kunnen vullen, geen productiefunctie.
pub async fn demo_reset(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<serde_json::Value>, ApiError> {
    if session_beoordelaar(&session).await.is_none() {
        return Err(forbidden());
    }
    let mut tx = state.pool.begin().await.map_err(internal_error)?;
    // Volgorde volgt de foreign keys: eerst de verwijzende tabellen.
    for tabel in [
        "bezwaren",
        "betaalopdrachten",
        "besluiten",
        "aanvragen",
        "register_claims",
        "register_uitslagen",
        "register_partijen",
        "register_gebieden",
    ] {
        sqlx::query(&format!("DELETE FROM {tabel}"))
            .execute(&mut *tx)
            .await
            .map_err(internal_error)?;
    }
    tx.commit().await.map_err(internal_error)?;
    register::seed_if_empty(&state.pool)
        .await
        .map_err(internal_error)?;
    tracing::warn!("demo-reset uitgevoerd: dossiers gewist, register herseed uit snapshot");
    Ok(Json(json!({"status": "gereset"})))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::state::LawCorpus;
    use crate::{db, register};
    use axum::http::StatusCode;
    use regelrecht_auth::{SESSION_KEY_AUTHENTICATED, SESSION_KEY_NAME};
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::Arc;
    use tower_sessions::Session;
    use tower_sessions_memory_store::MemoryStore;

    async fn test_state() -> AppState {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        db::init(&pool).await.expect("schema");
        // Minimal register fixture instead of the full JSON seed.
        sqlx::query(
            "INSERT INTO register_gebieden (orgaan, code, naam, inwoneraantal)
             VALUES ('GEMEENTERAAD', 'GM0344', 'Utrecht', 367984)",
        )
        .execute(&pool)
        .await
        .expect("gebied fixture");
        register::insert_partij(&pool, "11111111", "Testpartij", "CENTRAAL", None)
            .await
            .expect("partij fixture");
        register::insert_partij(&pool, "22222222", "Afdeling Utrecht", "DECENTRAAL", None)
            .await
            .expect("afdeling fixture");
        AppState {
            pool,
            corpus: Arc::new(LawCorpus::embedded()),
            procedure: Arc::new(
                crate::engine::beschikking_procedure(include_str!(
                    "../../../corpus-poc/napp/law/wet_op_de_politieke_partijen/2026-01-01.yaml"
                ))
                .expect("procedure uit de wet"),
            ),
            bezwaar_procedure: Arc::new(
                crate::engine::bezwaar_procedure(include_str!(
                    "../../../corpus-poc/napp/law/algemene_wet_bestuursrecht/1994-01-01.yaml"
                ))
                .expect("bezwaarprocedure uit de AWB"),
            ),
            oidc_client: None,
            oidc_config: None,
            end_session_url: None,
            base_url: None,
            // http_client() en niet Client::new(): het installeert de
            // rustls-provider, die anders per test ontbreekt ("No provider set").
            http_client: regelrecht_auth::http_client(),
        }
    }

    fn anonymous_session() -> Session {
        Session::new(None, Arc::new(MemoryStore::default()), None)
    }

    async fn beoordelaar_session() -> Session {
        let session = anonymous_session();
        session
            .insert(SESSION_KEY_AUTHENTICATED, true)
            .await
            .expect("session insert");
        session
            .insert(SESSION_KEY_NAME, "Testbeoordelaar".to_string())
            .await
            .expect("session insert");
        session
    }

    fn fout(err: &ApiError) -> String {
        err.1["fout"].as_str().unwrap_or_default().to_string()
    }

    #[tokio::test]
    async fn demo_reset_vereist_beoordelaarsessie_en_wist_dossiers() {
        let state = test_state().await;
        let err = demo_reset(State(state.clone()), anonymous_session())
            .await
            .expect_err("zonder sessie hoort 403");
        assert_eq!(err.0, StatusCode::FORBIDDEN);

        // Een dossier dat na de reset weg moet zijn.
        db::insert_aanvraag(
            &state.pool,
            "aanvraag-1",
            "11111111",
            "Testpartij",
            2027,
            "[]",
            "{}",
            "BEHANDELING",
            "2026-06-10",
            None,
        )
        .await
        .expect("aanvraag fixture");

        let _ = demo_reset(State(state.clone()), beoordelaar_session().await)
            .await
            .expect("reset met beoordelaarsessie");

        let aanvragen: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM aanvragen")
            .fetch_one(&state.pool)
            .await
            .expect("count");
        assert_eq!(aanvragen, 0);
        // Het register is herseed uit de snapshot (niet de testfixtures).
        let partijen: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM register_partijen")
            .fetch_one(&state.pool)
            .await
            .expect("count");
        assert!(partijen > 1000, "snapshot-seed verwacht, kreeg {partijen}");
    }

    #[tokio::test]
    async fn beheer_vereist_beoordelaarsessie() {
        let state = test_state().await;
        let err = list_partijen(
            State(state.clone()),
            anonymous_session(),
            Query(ZoekParams::default()),
        )
        .await
        .expect_err("zonder sessie hoort 403");
        assert_eq!(err.0, StatusCode::FORBIDDEN);

        let err = update_partij(
            State(state),
            anonymous_session(),
            Path("11111111".into()),
            Json(PartijWijziging {
                naam: "Partij".into(),
                organisatiemodel: "CENTRAAL".into(),
                moederpartij_kvk: None,
            }),
        )
        .await
        .expect_err("zonder sessie hoort 403");
        assert_eq!(err.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn update_partij_valideert_invoer() {
        let state = test_state().await;

        // Lege naam.
        let err = update_partij(
            State(state.clone()),
            beoordelaar_session().await,
            Path("22222222".into()),
            Json(PartijWijziging {
                naam: "  ".into(),
                organisatiemodel: "DECENTRAAL".into(),
                moederpartij_kvk: None,
            }),
        )
        .await
        .expect_err("naam is verplicht");
        assert!(fout(&err).contains("Naam"));

        // Onbekend organisatiemodel.
        let err = update_partij(
            State(state.clone()),
            beoordelaar_session().await,
            Path("22222222".into()),
            Json(PartijWijziging {
                naam: "Afdeling".into(),
                organisatiemodel: "FEDERAAL".into(),
                moederpartij_kvk: None,
            }),
        )
        .await
        .expect_err("organisatiemodel beperkt tot CENTRAAL|DECENTRAAL");
        assert!(fout(&err).contains("Organisatiemodel"));

        // Niet-bestaande moederpartij.
        let err = update_partij(
            State(state.clone()),
            beoordelaar_session().await,
            Path("22222222".into()),
            Json(PartijWijziging {
                naam: "Afdeling".into(),
                organisatiemodel: "DECENTRAAL".into(),
                moederpartij_kvk: Some("99999999".into()),
            }),
        )
        .await
        .expect_err("moederpartij moet bestaan");
        assert!(fout(&err).contains("Moederpartij"));

        // Zichzelf als moederpartij.
        let err = update_partij(
            State(state.clone()),
            beoordelaar_session().await,
            Path("22222222".into()),
            Json(PartijWijziging {
                naam: "Afdeling".into(),
                organisatiemodel: "DECENTRAAL".into(),
                moederpartij_kvk: Some("22222222".into()),
            }),
        )
        .await
        .expect_err("eigen moederpartij hoort te falen");
        assert!(fout(&err).contains("eigen moederpartij"));

        // Geldige wijziging slaagt.
        let detail = update_partij(
            State(state.clone()),
            beoordelaar_session().await,
            Path("22222222".into()),
            Json(PartijWijziging {
                naam: "Afdeling Utrecht".into(),
                organisatiemodel: "DECENTRAAL".into(),
                moederpartij_kvk: Some("11111111".into()),
            }),
        )
        .await
        .expect("geldige wijziging");
        assert_eq!(detail.0["naam"], "Afdeling Utrecht");
        assert_eq!(detail.0["moederpartij_naam"], "Testpartij");

        // Onbekende partij → 404.
        let err = update_partij(
            State(state),
            beoordelaar_session().await,
            Path("33333333".into()),
            Json(PartijWijziging {
                naam: "Spookpartij".into(),
                organisatiemodel: "CENTRAAL".into(),
                moederpartij_kvk: None,
            }),
        )
        .await
        .expect_err("onbekende partij hoort 404 te geven");
        assert_eq!(err.0, StatusCode::NOT_FOUND);
    }
}
