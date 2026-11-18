//! Representation scope ("machtiging") for the mocked eHerkenning login.
//!
//! eHerkenning supports (chain) authorizations: under the Wpp (consultation
//! version, explanatory memorandum to art. 27) branches of a centrally
//! organized party are organs without legal personality, but branch board
//! members can be granted a power of attorney to represent the party in a
//! limited way. This module owns the profile listing, the validation against
//! the party register and the server-side scope enforcement, so that the
//! changes in handlers.rs stay minimal.

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::SqlitePool;
use tower_sessions::Session;

use crate::db::Component;
use crate::register;
use crate::state::AppState;

pub const SESSION_KEY_EH_MACHTIGING: &str = "eh_machtiging";

/// The representation profile an eHerkenning session carries.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type")]
pub enum Machtiging {
    /// Signing-authorized board of the legal entity: full access.
    #[default]
    #[serde(rename = "VOLLEDIG")]
    Volledig,
    /// Branch board member with a power of attorney for one area.
    #[serde(rename = "BEPERKT")]
    Beperkt { gebied_code: String },
}

/// Validation outcome for a requested machtiging: a user-facing rejection or
/// an internal (database) error.
pub enum ValidatieFout {
    Ongeldig(String),
    Intern(anyhow::Error),
}

impl Machtiging {
    /// Whether a component key ("LANDELIJK" or "{orgaan}:{gebied_code}")
    /// falls within this machtiging. A limited machtiging never covers the
    /// national component.
    pub fn allows_key(&self, key: &str) -> bool {
        match self {
            Machtiging::Volledig => true,
            Machtiging::Beperkt { gebied_code } => key
                .split_once(':')
                .is_some_and(|(_, code)| code == gebied_code),
        }
    }

    /// Restrict a component list to what this machtiging covers.
    pub fn filter_componenten(&self, componenten: Vec<Component>) -> Vec<Component> {
        match self {
            Machtiging::Volledig => componenten,
            Machtiging::Beperkt { .. } => componenten
                .into_iter()
                .filter(|c| self.allows_key(&c.key))
                .collect(),
        }
    }

    /// Whether every component of an (existing) aanvraag falls within scope.
    /// Used to hide dossiers of the same legal entity that a branch board
    /// member is not authorized for.
    pub fn covers(&self, componenten: &[Component]) -> bool {
        componenten.iter().all(|c| self.allows_key(&c.key))
    }

    /// JSON for the session/status endpoint, enriched with gebied_naam and
    /// orgaan from the register so the UI can label the active machtiging.
    pub async fn to_json(&self, pool: &SqlitePool) -> anyhow::Result<serde_json::Value> {
        Ok(match self {
            Machtiging::Volledig => json!({"type": "VOLLEDIG"}),
            Machtiging::Beperkt { gebied_code } => {
                let gebied = register::gebied_by_code(pool, gebied_code).await?;
                json!({
                    "type": "BEPERKT",
                    "orgaan": gebied.as_ref().map(|g| g.orgaan.clone()),
                    "gebied_code": gebied_code,
                    "gebied_naam": gebied
                        .map(|g| g.naam)
                        .unwrap_or_else(|| gebied_code.clone()),
                })
            }
        })
    }
}

/// Validate a requested machtiging against the party register. BEPERKT is
/// only available for centrally organized parties with an election result in
/// that area; anything else can only log in as VOLLEDIG.
pub async fn valideer(
    pool: &SqlitePool,
    kvk: &str,
    machtiging: &Machtiging,
) -> Result<(), ValidatieFout> {
    let Machtiging::Beperkt { gebied_code } = machtiging else {
        return Ok(());
    };
    let partij = register::partij_by_kvk(pool, kvk)
        .await
        .map_err(ValidatieFout::Intern)?;
    // ONGEKOPPELD = rechtspersoon onbekend: behandelen als niet
    // geregistreerd, dus geen afdelingsmachtigingen.
    let Some(partij) = partij.filter(|p| !p.is_ongekoppeld()) else {
        return Err(ValidatieFout::Ongeldig(
            "Voor dit KVK-nummer zijn geen afdelingsmachtigingen beschikbaar.".to_string(),
        ));
    };
    if partij.organisatiemodel != "CENTRAAL" {
        return Err(ValidatieFout::Ongeldig(format!(
            "{} kent geen afdelingsmachtigingen: afdelingen zijn eigen rechtspersonen en loggen in met hun eigen KVK-nummer.",
            partij.naam
        )));
    }
    let uitslag = register::uitslag_by_kvk_gebied(pool, kvk, gebied_code)
        .await
        .map_err(ValidatieFout::Intern)?;
    if uitslag.is_none() {
        return Err(ValidatieFout::Ongeldig(format!(
            "{} heeft volgens het partijregister geen aanspraak in gebied '{gebied_code}'.",
            partij.naam
        )));
    }
    Ok(())
}

/// The representation profiles the mocked eHerkenning offers for a KvK
/// number: always VOLLEDIG; for centrally organized parties additionally one
/// BEPERKT profile per area with an election result (the branch volmacht).
pub async fn profielen_voor(
    pool: &SqlitePool,
    kvk: &str,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut profielen = vec![json!({
        "type": "VOLLEDIG",
        "omschrijving": "De gehele partij (tekenbevoegd bestuur)",
    })];
    let Some(partij) = register::partij_by_kvk(pool, kvk).await? else {
        return Ok(profielen);
    };
    // ONGEKOPPELD record: rechtspersoon onbekend, geen machtigingsprofielen.
    if partij.is_ongekoppeld() || partij.organisatiemodel != "CENTRAAL" {
        return Ok(profielen);
    }
    for u in register::uitslagen_met_gebied(pool, kvk).await? {
        profielen.push(json!({
            "type": "BEPERKT",
            "orgaan": u.orgaan,
            "gebied_code": u.gebied_code,
            "gebied_naam": u.gebied_naam.unwrap_or_else(|| u.gebied_code.clone()),
        }));
    }
    Ok(profielen)
}

#[derive(Deserialize)]
pub struct MachtigingenQuery {
    pub kvk: String,
}

/// GET /api/eherkenning/machtigingen?kvk=... — which representation profiles
/// the mocked eHerkenning would offer for this legal entity. Unknown KvK
/// numbers get only VOLLEDIG (anyone may apply, AWB 4:1).
pub async fn machtigingen(
    State(state): State<AppState>,
    Query(q): Query<MachtigingenQuery>,
) -> Result<Json<serde_json::Value>, crate::handlers::ApiError> {
    let kvk = q.kvk.trim();
    // ONGEKOPPELD: de partijnaam hoort niet bij dit (placeholder)nummer.
    let partij = register::partij_by_kvk(&state.pool, kvk)
        .await
        .map_err(crate::handlers::internal_error)?
        .filter(|p| !p.is_ongekoppeld());
    let profielen = profielen_voor(&state.pool, kvk)
        .await
        .map_err(crate::handlers::internal_error)?;
    Ok(Json(json!({
        "kvk_nummer": kvk,
        "partij_naam": partij.map(|p| p.naam),
        "profielen": profielen,
    })))
}

/// Read the machtiging from the session. Absent (sessions from before this
/// feature, or a login without the field) means VOLLEDIG — backwards
/// compatible with the existing BDD suite and seed script.
pub async fn session_machtiging(session: &Session) -> Machtiging {
    session
        .get::<Machtiging>(SESSION_KEY_EH_MACHTIGING)
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::db;
    use sqlx::sqlite::SqlitePoolOptions;

    const CENTRAAL: &str = "11111111";
    const DECENTRAAL_AFDELING: &str = "22222222";
    const ONBEKEND: &str = "00000000";

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        db::init(&pool).await.expect("schema");
        sqlx::query(
            "INSERT INTO register_gebieden (orgaan, code, naam, inwoneraantal)
             VALUES ('GEMEENTERAAD', 'GM0344', 'Utrecht', 367984)",
        )
        .execute(&pool)
        .await
        .expect("gebied fixture");
        register::insert_partij(&pool, CENTRAAL, "Centrale Partij", "CENTRAAL", None)
            .await
            .expect("partij fixture");
        register::insert_uitslag(&pool, CENTRAAL, "GEMEENTERAAD", "GM0344", 6)
            .await
            .expect("uitslag fixture");
        register::insert_partij(
            &pool,
            DECENTRAAL_AFDELING,
            "Afdeling Eigen Rechtspersoon",
            "DECENTRAAL",
            None,
        )
        .await
        .expect("afdeling fixture");
        pool
    }

    fn beperkt(code: &str) -> Machtiging {
        Machtiging::Beperkt {
            gebied_code: code.to_string(),
        }
    }

    fn component(key: &str) -> Component {
        Component {
            key: key.to_string(),
            soort: if key == "LANDELIJK" {
                "LANDELIJK".into()
            } else {
                "DECENTRAAL".into()
            },
            orgaan: None,
            gebied_code: None,
            gebied: None,
            zetels: 1,
            inwoneraantal: 0,
        }
    }

    #[test]
    fn volledig_allows_every_key() {
        let m = Machtiging::Volledig;
        assert!(m.allows_key("LANDELIJK"));
        assert!(m.allows_key("GEMEENTERAAD:GM0344"));
        assert!(m.allows_key("PROVINCIALE_STATEN:PV26"));
    }

    #[test]
    fn beperkt_allows_only_own_gebied_and_never_landelijk() {
        let m = beperkt("GM0344");
        assert!(m.allows_key("GEMEENTERAAD:GM0344"));
        assert!(!m.allows_key("LANDELIJK"));
        assert!(!m.allows_key("GEMEENTERAAD:GM0034"));
        assert!(!m.allows_key("PROVINCIALE_STATEN:PV26"));
    }

    #[test]
    fn filter_componenten_keeps_only_scope() {
        let alles = vec![
            component("LANDELIJK"),
            component("GEMEENTERAAD:GM0344"),
            component("GEMEENTERAAD:GM0599"),
        ];
        let gefilterd = beperkt("GM0344").filter_componenten(alles.clone());
        assert_eq!(gefilterd.len(), 1);
        assert_eq!(gefilterd[0].key, "GEMEENTERAAD:GM0344");
        assert_eq!(Machtiging::Volledig.filter_componenten(alles).len(), 3);
    }

    #[test]
    fn covers_requires_all_components_in_scope() {
        let m = beperkt("GM0344");
        assert!(m.covers(&[component("GEMEENTERAAD:GM0344")]));
        assert!(!m.covers(&[component("GEMEENTERAAD:GM0344"), component("LANDELIJK")]));
        assert!(Machtiging::Volledig.covers(&[component("LANDELIJK")]));
        // An empty aanvraag cannot exist, but covers should not panic.
        assert!(m.covers(&[]));
    }

    #[tokio::test]
    async fn valideer_volledig_is_always_ok() {
        let pool = test_pool().await;
        assert!(
            valideer(&pool, CENTRAAL, &Machtiging::Volledig)
                .await
                .is_ok()
        );
        assert!(
            valideer(&pool, ONBEKEND, &Machtiging::Volledig)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn valideer_beperkt_requires_centraal_party_with_uitslag() {
        let pool = test_pool().await;
        assert!(valideer(&pool, CENTRAAL, &beperkt("GM0344")).await.is_ok());
        // Unknown legal entity: no branch machtigingen.
        assert!(valideer(&pool, ONBEKEND, &beperkt("GM0344")).await.is_err());
        // Decentrally organized: branches are their own legal entity.
        assert!(
            valideer(&pool, DECENTRAAL_AFDELING, &beperkt("GM0344"))
                .await
                .is_err()
        );
        // Centrally organized but no result in that area.
        assert!(valideer(&pool, CENTRAAL, &beperkt("XX9999")).await.is_err());
    }

    #[tokio::test]
    async fn profielen_volledig_plus_one_per_gebied_for_centraal() {
        let pool = test_pool().await;
        let onbekend = profielen_voor(&pool, ONBEKEND).await.unwrap();
        assert_eq!(onbekend.len(), 1);
        assert_eq!(onbekend[0]["type"], "VOLLEDIG");

        let afdeling = profielen_voor(&pool, DECENTRAAL_AFDELING).await.unwrap();
        assert_eq!(afdeling.len(), 1, "DECENTRAAL gets no branch profiles");

        let centraal = profielen_voor(&pool, CENTRAAL).await.unwrap();
        assert_eq!(centraal.len(), 2);
        assert_eq!(centraal[0]["type"], "VOLLEDIG");
        assert_eq!(centraal[1]["type"], "BEPERKT");
        assert_eq!(centraal[1]["orgaan"], "GEMEENTERAAD");
        assert_eq!(centraal[1]["gebied_naam"], "Utrecht");
    }

    #[test]
    fn machtiging_deserializes_from_login_payload() {
        let volledig: Machtiging = serde_json::from_value(json!({"type": "VOLLEDIG"})).unwrap();
        assert_eq!(volledig, Machtiging::Volledig);
        let beperkt_m: Machtiging =
            serde_json::from_value(json!({"type": "BEPERKT", "gebied_code": "GM0344"})).unwrap();
        assert_eq!(beperkt_m, beperkt("GM0344"));
        assert!(serde_json::from_value::<Machtiging>(json!({"type": "BEPERKT"})).is_err());
    }

    #[tokio::test]
    async fn to_json_enriches_with_gebied_naam() {
        let pool = test_pool().await;
        let v = beperkt("GM0344").to_json(&pool).await.unwrap();
        assert_eq!(v["type"], "BEPERKT");
        assert_eq!(v["gebied_naam"], "Utrecht");
        assert_eq!(v["orgaan"], "GEMEENTERAAD");
        assert_eq!(
            Machtiging::Volledig.to_json(&pool).await.unwrap(),
            json!({"type": "VOLLEDIG"})
        );
    }
}
