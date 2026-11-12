//! Claim-flow: nieuwe koppelingen in het partijregister.
//!
//! Een partij die zetels haalde maar waarvan de rechtspersoon nog onbekend
//! is, staat in het register als ONGEKOPPELDE aanduiding met een
//! placeholder-KvK-nummer. De rechtspersoon logt in met eHerkenning
//! (geverifieerd KvK-nummer), claimt haar aanduiding uit de uitslag, en een
//! Napp-beoordelaar bevestigt de claim na de (gemockte)
//! Handelsregister-toets — zie `handelsregister.rs`. Juridische basis:
//! Kieswet G-1 eist bij registratie van een aanduiding bewijs van
//! KvK-inschrijving als vereniging met volledige rechtsbevoegdheid.
//!
//! Claims zijn alleen voor VOLLEDIG-machtigingen (tekenbevoegd bestuur):
//! een afdelingsvolmacht kan geen rechtspersoon aan een aanduiding binden.
//! De beheer-endpoints volgen het beoordelaar-only 403-patroon van
//! `beheer.rs`.

use axum::Json;
use axum::extract::{Path, Query, State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Row, SqlitePool};
use tower_sessions::Session;
use uuid::Uuid;

use crate::handelsregister;
use crate::handlers::{
    ApiError, bad_request, forbidden, forbidden_with, internal_error, not_found,
    session_beoordelaar, session_kvk,
};
use crate::machtiging::{self, Machtiging};
use crate::register;
use crate::state::AppState;

pub const STATUS_OPEN: &str = "OPEN";
pub const STATUS_BEVESTIGD: &str = "BEVESTIGD";
pub const STATUS_AFGEWEZEN: &str = "AFGEWEZEN";

/// Maximum number of ONGEKOPPELDE aanduidingen returned per search.
const AANDUIDINGEN_LIMIT: i64 = 20;

#[derive(Debug, Clone, Serialize)]
pub struct Claim {
    pub id: String,
    /// The verified KvK number of the claiming legal entity (eHerkenning).
    pub kvk_nummer: String,
    /// The placeholder KvK number of the claimed ONGEKOPPELD record.
    pub doel_kvk: String,
    pub aanduiding: String,
    /// Stored result of the (mocked) Handelsregister check.
    pub hr_toets: serde_json::Value,
    /// OPEN | BEVESTIGD | AFGEWEZEN
    pub status: String,
    pub reden_afwijzing: Option<String>,
    pub created_at: String,
    pub beoordeeld_door: Option<String>,
    pub beoordeeld_at: Option<String>,
}

fn row_to_claim(row: &sqlx::sqlite::SqliteRow) -> Claim {
    Claim {
        id: row.get("id"),
        kvk_nummer: row.get("kvk_nummer"),
        doel_kvk: row.get("doel_kvk"),
        aanduiding: row.get("aanduiding"),
        hr_toets: serde_json::from_str(row.get::<String, _>("hr_toets").as_str())
            .unwrap_or(serde_json::Value::Null),
        status: row.get("status"),
        reden_afwijzing: row.get("reden_afwijzing"),
        created_at: row.get("created_at"),
        beoordeeld_door: row.get("beoordeeld_door"),
        beoordeeld_at: row.get("beoordeeld_at"),
    }
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

async fn claim_by_id(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Claim>> {
    let row = sqlx::query("SELECT * FROM register_claims WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(row_to_claim))
}

/// The most recent claim of a legal entity (any status); the UI shows an
/// open claim as pending and a rejected one with reason + retry.
async fn laatste_claim_voor(pool: &SqlitePool, kvk: &str) -> anyhow::Result<Option<Claim>> {
    let row = sqlx::query(
        "SELECT * FROM register_claims WHERE kvk_nummer = ?
         ORDER BY created_at DESC, rowid DESC LIMIT 1",
    )
    .bind(kvk)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(row_to_claim))
}

async fn heeft_open_claim(pool: &SqlitePool, kvk: &str) -> anyhow::Result<bool> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM register_claims WHERE kvk_nummer = ? AND status = ?",
    )
    .bind(kvk)
    .bind(STATUS_OPEN)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

/// All claims, open ones first, newest first within each group.
async fn list_claims(pool: &SqlitePool) -> anyhow::Result<Vec<Claim>> {
    let rows = sqlx::query(
        "SELECT * FROM register_claims
         ORDER BY (status = 'OPEN') DESC, created_at DESC, rowid DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row_to_claim).collect())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The aanvrager session for claim endpoints: eHerkenning required, and only
/// the signing-authorized board (VOLLEDIG) may bind the legal entity.
async fn session_volledige_aanvrager(session: &Session) -> Result<String, ApiError> {
    let Some(kvk) = session_kvk(session).await else {
        return Err(forbidden());
    };
    if machtiging::session_machtiging(session).await != Machtiging::Volledig {
        return Err(forbidden_with(
            "Een aanduiding claimen kan alleen namens de gehele partij \
             (tekenbevoegd bestuur). Uw beperkte machtiging als \
             afdelingsbestuurder volstaat niet; log opnieuw in namens de \
             gehele partij.",
        ));
    }
    Ok(kvk)
}

fn orgaan_label(orgaan: &str) -> &str {
    match orgaan {
        "GEMEENTERAAD" => "Gemeenteraad",
        "PROVINCIALE_STATEN" => "Provinciale staten",
        "EILANDSRAAD" => "Eilandsraad",
        "WATERSCHAP" => "Waterschap",
        anders => anders,
    }
}

/// Short result summary per uitslag, e.g. "Gemeenteraad Zwolle: 3 zetels".
async fn uitslag_samenvatting(state: &AppState, kvk: &str) -> Result<Vec<String>, ApiError> {
    let uitslagen = register::uitslagen_met_gebied(&state.pool, kvk)
        .await
        .map_err(internal_error)?;
    Ok(uitslagen
        .iter()
        .map(|u| {
            let gebied = u
                .gebied_naam
                .clone()
                .unwrap_or_else(|| u.gebied_code.clone());
            let zetels = if u.zetels == 1 { "zetel" } else { "zetels" };
            format!(
                "{} {}: {} {}",
                orgaan_label(&u.orgaan),
                gebied,
                u.zetels,
                zetels
            )
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Aanvrager-endpoints
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
pub struct AanduidingenParams {
    #[serde(default)]
    pub zoek: Option<String>,
}

/// GET /api/claim/aanduidingen?zoek= — ONGEKOPPELDE aanduidingen uit de
/// uitslag die de ingelogde rechtspersoon kan claimen, met een korte
/// uitslag-samenvatting per aanduiding.
pub async fn list_aanduidingen(
    State(state): State<AppState>,
    session: Session,
    Query(params): Query<AanduidingenParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    session_volledige_aanvrager(&session).await?;
    let zoek = params.zoek.unwrap_or_default();
    let partijen = register::zoek_ongekoppelde_partijen(&state.pool, &zoek, AANDUIDINGEN_LIMIT)
        .await
        .map_err(internal_error)?;
    let mut aanduidingen = Vec::with_capacity(partijen.len());
    for p in &partijen {
        aanduidingen.push(json!({
            "doel_kvk": p.kvk_nummer,
            "aanduiding": p.naam,
            "uitslagen": uitslag_samenvatting(&state, &p.kvk_nummer).await?,
        }));
    }
    Ok(Json(json!({ "aanduidingen": aanduidingen })))
}

#[derive(Debug, Deserialize)]
pub struct NieuweClaim {
    pub doel_kvk: String,
}

/// POST /api/claim — de ingelogde rechtspersoon claimt een ONGEKOPPELDE
/// aanduiding. Voert direct de (gemockte) Handelsregister-toets uit en legt
/// het resultaat bij de claim vast; een beoordelaar bevestigt daarna.
pub async fn create_claim(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<NieuweClaim>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let kvk = session_volledige_aanvrager(&session).await?;
    let doel_kvk = body.doel_kvk.trim().to_string();

    let Some(doel) = register::partij_by_kvk(&state.pool, &doel_kvk)
        .await
        .map_err(internal_error)?
    else {
        return Err(bad_request(
            "De gekozen aanduiding staat niet in het register.",
        ));
    };
    if !doel.is_ongekoppeld() {
        return Err(bad_request(
            "Deze aanduiding is al aan een rechtspersoon gekoppeld.",
        ));
    }
    if register::partij_by_kvk(&state.pool, &kvk)
        .await
        .map_err(internal_error)?
        .is_some()
    {
        return Err(bad_request(
            "Uw rechtspersoon staat al in het partijregister en kan geen aanduiding claimen.",
        ));
    }
    if heeft_open_claim(&state.pool, &kvk)
        .await
        .map_err(internal_error)?
    {
        return Err(bad_request(
            "Er loopt al een claim voor uw rechtspersoon. Wacht op de beoordeling door de Napp.",
        ));
    }

    // Handelsregister-raadpleging (mock, databron) plus het oordeel van
    // Kieswet G 1 daarover (wet, engine). Beide worden bij de claim
    // opgeslagen: de beoordelaar ziet de feiten en het wettelijk advies,
    // en beslist zelf.
    let toets = handelsregister::raadpleeg(&kvk, &doel.naam);
    let vandaag = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let oordeel =
        crate::engine::evaluate_registratie_eisen(state.corpus.clone(), toets.clone(), vandaag)
            .await
            .map_err(internal_error)?;
    let mut toets_json = serde_json::to_value(&toets).map_err(internal_error)?;
    toets_json["wettelijke_toets"] = serde_json::to_value(&oordeel).map_err(internal_error)?;
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO register_claims (id, kvk_nummer, doel_kvk, aanduiding, hr_toets)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&kvk)
    .bind(&doel_kvk)
    .bind(&doel.naam)
    .bind(serde_json::to_string(&toets_json).map_err(internal_error)?)
    .execute(&state.pool)
    .await
    .map_err(internal_error)?;

    let claim = claim_by_id(&state.pool, &id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| internal_error("claim verdween direct na aanmaken"))?;
    Ok(Json(serde_json::to_value(claim).map_err(internal_error)?))
}

/// GET /api/mijn-claim — de (laatste) claim van de ingelogde rechtspersoon,
/// inclusief het opgeslagen HR-toets-resultaat en de status.
pub async fn mijn_claim(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<serde_json::Value>, ApiError> {
    let kvk = session_volledige_aanvrager(&session).await?;
    let claim = laatste_claim_voor(&state.pool, &kvk)
        .await
        .map_err(internal_error)?;
    Ok(Json(json!({ "claim": claim })))
}

// ---------------------------------------------------------------------------
// Beoordelaar-endpoints
// ---------------------------------------------------------------------------

/// GET /api/beheer/claims — alle claims: open eerst, dan afgehandeld.
pub async fn beheer_list_claims(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<serde_json::Value>, ApiError> {
    if session_beoordelaar(&session).await.is_none() {
        return Err(forbidden());
    }
    let claims = list_claims(&state.pool).await.map_err(internal_error)?;
    Ok(Json(json!({ "claims": claims })))
}

/// POST /api/beheer/claims/{id}/bevestig — bevestig een open claim. In één
/// transactie wordt het ONGEKOPPELDE record (placeholder-nummer) omgezet
/// naar de geverifieerde rechtspersoon: register_partijen.kvk_nummer wordt
/// het echte nummer, status GEVERIFIEERD, en de uitslag-rijen verhuizen mee.
pub async fn bevestig_claim(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(beoordelaar) = session_beoordelaar(&session).await else {
        return Err(forbidden());
    };
    let Some(claim) = claim_by_id(&state.pool, &id)
        .await
        .map_err(internal_error)?
    else {
        return Err(not_found());
    };
    if claim.status != STATUS_OPEN {
        return Err(bad_request("Deze claim is al beoordeeld."));
    }

    let mut tx = state.pool.begin().await.map_err(internal_error)?;
    // De uitslag-rijen verwijzen naar register_partijen.kvk_nummer; tijdens
    // het omhangen van het nummer kloppen die verwijzingen tijdelijk niet.
    // Stel de FK-controle uit tot de commit (geldt alleen binnen deze tx).
    sqlx::query("PRAGMA defer_foreign_keys = ON")
        .execute(&mut *tx)
        .await
        .map_err(internal_error)?;
    // Het doelrecord moet nog steeds ONGEKOPPELD zijn en het claimende
    // nummer mag intussen niet alsnog in het register zijn beland.
    let doel_status: Option<String> =
        sqlx::query_scalar("SELECT status FROM register_partijen WHERE kvk_nummer = ?")
            .bind(&claim.doel_kvk)
            .fetch_optional(&mut *tx)
            .await
            .map_err(internal_error)?;
    if doel_status.as_deref() != Some(register::STATUS_ONGEKOPPELD) {
        return Err(bad_request(
            "De geclaimde aanduiding is intussen al aan een rechtspersoon gekoppeld.",
        ));
    }
    let al_geregistreerd: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM register_partijen WHERE kvk_nummer = ?")
            .bind(&claim.kvk_nummer)
            .fetch_one(&mut *tx)
            .await
            .map_err(internal_error)?;
    if al_geregistreerd > 0 {
        return Err(bad_request(
            "De claimende rechtspersoon staat intussen al in het register.",
        ));
    }

    sqlx::query("UPDATE register_partijen SET kvk_nummer = ?, status = ? WHERE kvk_nummer = ?")
        .bind(&claim.kvk_nummer)
        .bind(register::STATUS_GEVERIFIEERD)
        .bind(&claim.doel_kvk)
        .execute(&mut *tx)
        .await
        .map_err(internal_error)?;
    sqlx::query("UPDATE register_uitslagen SET kvk_nummer = ? WHERE kvk_nummer = ?")
        .bind(&claim.kvk_nummer)
        .bind(&claim.doel_kvk)
        .execute(&mut *tx)
        .await
        .map_err(internal_error)?;
    sqlx::query(
        "UPDATE register_claims
         SET status = ?, beoordeeld_door = ?, beoordeeld_at = datetime('now')
         WHERE id = ?",
    )
    .bind(STATUS_BEVESTIGD)
    .bind(&beoordelaar)
    .bind(&id)
    .execute(&mut *tx)
    .await
    .map_err(internal_error)?;
    tx.commit().await.map_err(internal_error)?;

    let claim = claim_by_id(&state.pool, &id)
        .await
        .map_err(internal_error)?
        .ok_or_else(not_found)?;
    Ok(Json(serde_json::to_value(claim).map_err(internal_error)?))
}

#[derive(Debug, Deserialize)]
pub struct Afwijzing {
    pub reden: String,
}

/// POST /api/beheer/claims/{id}/afwijzen — wijs een open claim af met een
/// reden. De partij kan daarna opnieuw claimen.
pub async fn wijs_claim_af(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
    Json(body): Json<Afwijzing>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(beoordelaar) = session_beoordelaar(&session).await else {
        return Err(forbidden());
    };
    let reden = body.reden.trim().to_string();
    if reden.is_empty() {
        return Err(bad_request("Een reden voor de afwijzing is verplicht."));
    }
    let Some(claim) = claim_by_id(&state.pool, &id)
        .await
        .map_err(internal_error)?
    else {
        return Err(not_found());
    };
    if claim.status != STATUS_OPEN {
        return Err(bad_request("Deze claim is al beoordeeld."));
    }
    sqlx::query(
        "UPDATE register_claims
         SET status = ?, reden_afwijzing = ?, beoordeeld_door = ?, beoordeeld_at = datetime('now')
         WHERE id = ?",
    )
    .bind(STATUS_AFGEWEZEN)
    .bind(&reden)
    .bind(&beoordelaar)
    .bind(&id)
    .execute(&state.pool)
    .await
    .map_err(internal_error)?;

    let claim = claim_by_id(&state.pool, &id)
        .await
        .map_err(internal_error)?
        .ok_or_else(not_found)?;
    Ok(Json(serde_json::to_value(claim).map_err(internal_error)?))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::handlers::SESSION_KEY_EH_KVK;
    use crate::state::LawCorpus;
    use crate::{db, register};
    use axum::http::StatusCode;
    use regelrecht_auth::{SESSION_KEY_AUTHENTICATED, SESSION_KEY_NAME};
    use sqlx::sqlite::SqlitePoolOptions;
    use std::sync::Arc;
    use tower_sessions::Session;
    use tower_sessions_memory_store::MemoryStore;

    /// Placeholder number of the ONGEKOPPELD fixture record.
    const PLACEHOLDER: &str = "98765432";
    /// Verified KvK of the registered fixture party.
    const GEREGISTREERD: &str = "11111111";
    /// The fresh legal entity that will claim the aanduiding.
    const NIEUW: &str = "90000001";

    async fn test_state() -> AppState {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        db::init(&pool).await.expect("schema");
        sqlx::query(
            "INSERT INTO register_gebieden (orgaan, code, naam, inwoneraantal)
             VALUES ('GEMEENTERAAD', 'GM0193', 'Zwolle', 133839)",
        )
        .execute(&pool)
        .await
        .expect("gebied fixture");
        register::insert_partij(&pool, GEREGISTREERD, "Testpartij", "CENTRAAL", None)
            .await
            .expect("partij fixture");
        // ONGEKOPPELDE aanduiding met placeholder-nummer en één uitslag.
        sqlx::query(
            "INSERT INTO register_partijen (kvk_nummer, naam, organisatiemodel, kamerzetels, status)
             VALUES (?, 'Zwolse Stadspartij', 'CENTRAAL', 0, 'ONGEKOPPELD')",
        )
        .bind(PLACEHOLDER)
        .execute(&pool)
        .await
        .expect("ongekoppeld fixture");
        register::insert_uitslag(&pool, PLACEHOLDER, "GEMEENTERAAD", "GM0193", 3)
            .await
            .expect("uitslag fixture");
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

    async fn aanvrager_session(kvk: &str) -> Session {
        let session = anonymous_session();
        session
            .insert(SESSION_KEY_EH_KVK, kvk.to_string())
            .await
            .expect("session insert");
        session
    }

    async fn beperkte_session(kvk: &str) -> Session {
        let session = aanvrager_session(kvk).await;
        session
            .insert(
                machtiging::SESSION_KEY_EH_MACHTIGING,
                Machtiging::Beperkt {
                    gebied_code: "GM0193".to_string(),
                },
            )
            .await
            .expect("session insert");
        session
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

    async fn claim_voor(state: &AppState, kvk: &str) -> Claim {
        let result = create_claim(
            State(state.clone()),
            aanvrager_session(kvk).await,
            Json(NieuweClaim {
                doel_kvk: PLACEHOLDER.into(),
            }),
        )
        .await
        .expect("claim aanmaken");
        let id = result.0["id"].as_str().expect("claim heeft id").to_string();
        claim_by_id(&state.pool, &id)
            .await
            .expect("claim ophalen")
            .expect("claim bestaat")
    }

    #[tokio::test]
    async fn claim_endpoints_vereisen_sessie() {
        let state = test_state().await;
        for err in [
            list_aanduidingen(
                State(state.clone()),
                anonymous_session(),
                Query(AanduidingenParams::default()),
            )
            .await
            .expect_err("zonder sessie hoort 403"),
            create_claim(
                State(state.clone()),
                anonymous_session(),
                Json(NieuweClaim {
                    doel_kvk: PLACEHOLDER.into(),
                }),
            )
            .await
            .expect_err("zonder sessie hoort 403"),
            mijn_claim(State(state.clone()), anonymous_session())
                .await
                .expect_err("zonder sessie hoort 403"),
        ] {
            assert_eq!(err.0, StatusCode::FORBIDDEN);
        }
    }

    #[tokio::test]
    async fn beperkte_machtiging_krijgt_403_met_uitleg() {
        let state = test_state().await;
        let err = create_claim(
            State(state.clone()),
            beperkte_session(GEREGISTREERD).await,
            Json(NieuweClaim {
                doel_kvk: PLACEHOLDER.into(),
            }),
        )
        .await
        .expect_err("beperkte machtiging hoort 403");
        assert_eq!(err.0, StatusCode::FORBIDDEN);
        assert!(fout(&err).contains("tekenbevoegd bestuur"));
    }

    #[tokio::test]
    async fn aanduidingen_lijst_toont_ongekoppelde_met_samenvatting() {
        let state = test_state().await;
        let result = list_aanduidingen(
            State(state.clone()),
            aanvrager_session(NIEUW).await,
            Query(AanduidingenParams::default()),
        )
        .await
        .expect("lijst");
        let aanduidingen = result.0["aanduidingen"].as_array().expect("array").clone();
        assert_eq!(aanduidingen.len(), 1);
        assert_eq!(aanduidingen[0]["doel_kvk"], PLACEHOLDER);
        assert_eq!(aanduidingen[0]["aanduiding"], "Zwolse Stadspartij");
        assert_eq!(
            aanduidingen[0]["uitslagen"][0],
            "Gemeenteraad Zwolle: 3 zetels"
        );

        // Zoekfilter dat niets matcht.
        let leeg = list_aanduidingen(
            State(state),
            aanvrager_session(NIEUW).await,
            Query(AanduidingenParams {
                zoek: Some("Rotterdam".into()),
            }),
        )
        .await
        .expect("lijst");
        assert!(leeg.0["aanduidingen"].as_array().expect("array").is_empty());
    }

    #[tokio::test]
    async fn claim_valideert_doel_en_aanvrager() {
        let state = test_state().await;

        // Onbekend doelrecord.
        let err = create_claim(
            State(state.clone()),
            aanvrager_session(NIEUW).await,
            Json(NieuweClaim {
                doel_kvk: "00000000".into(),
            }),
        )
        .await
        .expect_err("onbekend doel hoort te falen");
        assert!(fout(&err).contains("niet in het register"));

        // Doelrecord is al gekoppeld (GEVERIFIEERD).
        let err = create_claim(
            State(state.clone()),
            aanvrager_session(NIEUW).await,
            Json(NieuweClaim {
                doel_kvk: GEREGISTREERD.into(),
            }),
        )
        .await
        .expect_err("geverifieerd doel hoort te falen");
        assert!(fout(&err).contains("al aan een rechtspersoon gekoppeld"));

        // De ingelogde rechtspersoon staat zelf al in het register.
        let err = create_claim(
            State(state.clone()),
            aanvrager_session(GEREGISTREERD).await,
            Json(NieuweClaim {
                doel_kvk: PLACEHOLDER.into(),
            }),
        )
        .await
        .expect_err("geregistreerde partij hoort te falen");
        assert!(fout(&err).contains("staat al in het partijregister"));

        // Tweede claim terwijl er al één open staat.
        claim_voor(&state, NIEUW).await;
        let err = create_claim(
            State(state),
            aanvrager_session(NIEUW).await,
            Json(NieuweClaim {
                doel_kvk: PLACEHOLDER.into(),
            }),
        )
        .await
        .expect_err("open claim hoort te blokkeren");
        assert!(fout(&err).contains("loopt al een claim"));
    }

    #[tokio::test]
    async fn claim_slaat_hr_toets_op_en_is_zichtbaar_via_mijn_claim() {
        let state = test_state().await;
        let claim = claim_voor(&state, NIEUW).await;
        assert_eq!(claim.status, STATUS_OPEN);
        assert_eq!(claim.aanduiding, "Zwolse Stadspartij");
        assert_eq!(claim.hr_toets["gevonden"], true);
        assert_eq!(claim.hr_toets["sbi_code"], "94.92");
        assert_eq!(
            claim.hr_toets["rechtsvorm"],
            "Vereniging met volledige rechtsbevoegdheid"
        );
        // Het oordeel van Kieswet G 1 (engine) wordt bij de claim bewaard.
        assert_eq!(
            claim.hr_toets["wettelijke_toets"]["voldoet_aan_registratie_eisen"],
            true
        );
        assert_eq!(
            claim.hr_toets["wettelijke_toets"]["voldoet_eis_rechtsvorm"],
            true
        );

        let result = mijn_claim(State(state), aanvrager_session(NIEUW).await)
            .await
            .expect("mijn claim");
        assert_eq!(result.0["claim"]["id"], claim.id.as_str());
        assert_eq!(result.0["claim"]["status"], "OPEN");
    }

    #[tokio::test]
    async fn beheer_claims_vereist_beoordelaarsessie() {
        let state = test_state().await;
        let err = beheer_list_claims(State(state.clone()), anonymous_session())
            .await
            .expect_err("zonder sessie hoort 403");
        assert_eq!(err.0, StatusCode::FORBIDDEN);
        let err = bevestig_claim(State(state.clone()), anonymous_session(), Path("x".into()))
            .await
            .expect_err("zonder sessie hoort 403");
        assert_eq!(err.0, StatusCode::FORBIDDEN);
        let err = wijs_claim_af(
            State(state),
            anonymous_session(),
            Path("x".into()),
            Json(Afwijzing {
                reden: "n.v.t.".into(),
            }),
        )
        .await
        .expect_err("zonder sessie hoort 403");
        assert_eq!(err.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn bevestigen_koppelt_partij_en_uitslagen_in_een_transactie() {
        let state = test_state().await;
        let claim = claim_voor(&state, NIEUW).await;

        let result = bevestig_claim(
            State(state.clone()),
            beoordelaar_session().await,
            Path(claim.id.clone()),
        )
        .await
        .expect("bevestigen");
        assert_eq!(result.0["status"], "BEVESTIGD");
        assert_eq!(result.0["beoordeeld_door"], "Testbeoordelaar");

        // Het register draagt nu het echte KvK-nummer, geverifieerd.
        let partij = register::partij_by_kvk(&state.pool, NIEUW)
            .await
            .expect("query")
            .expect("partij bestaat onder nieuw nummer");
        assert_eq!(partij.naam, "Zwolse Stadspartij");
        assert_eq!(partij.status, register::STATUS_GEVERIFIEERD);
        assert_eq!(partij.decentrale_uitslagen.len(), 1, "uitslagen verhuisd");
        assert!(
            register::partij_by_kvk(&state.pool, PLACEHOLDER)
                .await
                .expect("query")
                .is_none()
        );

        // Een al beoordeelde claim kan niet nogmaals worden bevestigd.
        let err = bevestig_claim(State(state), beoordelaar_session().await, Path(claim.id))
            .await
            .expect_err("dubbel bevestigen hoort te falen");
        assert!(fout(&err).contains("al beoordeeld"));
    }

    #[tokio::test]
    async fn afwijzen_vereist_reden_en_maakt_opnieuw_claimen_mogelijk() {
        let state = test_state().await;
        let claim = claim_voor(&state, NIEUW).await;

        // Reden verplicht.
        let err = wijs_claim_af(
            State(state.clone()),
            beoordelaar_session().await,
            Path(claim.id.clone()),
            Json(Afwijzing { reden: "  ".into() }),
        )
        .await
        .expect_err("lege reden hoort te falen");
        assert!(fout(&err).contains("reden"));

        let result = wijs_claim_af(
            State(state.clone()),
            beoordelaar_session().await,
            Path(claim.id.clone()),
            Json(Afwijzing {
                reden: "Statutaire naam wijkt af van de aanduiding.".into(),
            }),
        )
        .await
        .expect("afwijzen");
        assert_eq!(result.0["status"], "AFGEWEZEN");
        assert_eq!(
            result.0["reden_afwijzing"],
            "Statutaire naam wijkt af van de aanduiding."
        );

        // Na afwijzing kan de partij opnieuw claimen; mijn-claim toont de
        // nieuwste (weer open) claim.
        let tweede = claim_voor(&state, NIEUW).await;
        assert_eq!(tweede.status, STATUS_OPEN);
        let result = mijn_claim(State(state.clone()), aanvrager_session(NIEUW).await)
            .await
            .expect("mijn claim");
        assert_eq!(result.0["claim"]["id"], tweede.id.as_str());

        // Open eerst in het beheer-overzicht.
        let lijst = beheer_list_claims(State(state), beoordelaar_session().await)
            .await
            .expect("lijst");
        let claims = lijst.0["claims"].as_array().expect("array").clone();
        assert_eq!(claims.len(), 2);
        assert_eq!(claims[0]["status"], "OPEN");
        assert_eq!(claims[1]["status"], "AFGEWEZEN");
    }
}
