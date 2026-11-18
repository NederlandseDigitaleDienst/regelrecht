//! HTTP handlers: auth (mock eHerkenning + mock SSO fallback), aanvragen,
//! besluiten, bekendmaking, register en betaalopdrachten.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::Utc;
use regelrecht_auth::{
    SESSION_KEY_AUTHENTICATED, SESSION_KEY_EMAIL, SESSION_KEY_NAME, SESSION_KEY_SUB,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use crate::db;
use crate::engine;
use crate::machtiging;
use crate::state::AppState;

pub(crate) const SESSION_KEY_EH_KVK: &str = "eh_kvk";
const SESSION_KEY_EH_PARTIJ: &str = "eh_partij";

pub(crate) type ApiError = (StatusCode, Json<serde_json::Value>);

pub(crate) fn internal_error(e: impl std::fmt::Display) -> ApiError {
    tracing::error!(error = %e, "interne fout");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"fout": "Er is een interne fout opgetreden."})),
    )
}

pub(crate) fn bad_request(msg: &str) -> ApiError {
    (StatusCode::BAD_REQUEST, Json(json!({"fout": msg})))
}

pub(crate) fn not_found() -> ApiError {
    (
        StatusCode::NOT_FOUND,
        Json(json!({"fout": "Niet gevonden."})),
    )
}

pub(crate) fn forbidden() -> ApiError {
    forbidden_with("Geen toegang.")
}

pub(crate) fn forbidden_with(msg: &str) -> ApiError {
    (StatusCode::FORBIDDEN, Json(json!({"fout": msg})))
}

pub(crate) async fn session_kvk(session: &Session) -> Option<String> {
    session.get(SESSION_KEY_EH_KVK).await.ok().flatten()
}

/// Het subsidiejaar waarop een aanvraag van vandaag betrekking heeft: het
/// eerstvolgende jaar waarvoor de aanvraagtermijn van de wet nog niet is
/// verstreken. De norm (uiterlijk 1 november voorafgaand aan het
/// subsidiejaar, Wpp art. 17) komt volledig uit de wet; hier wordt alleen
/// een kandidaatjaar gekozen en worden twee door de wet geleverde
/// ISO-datums vergeleken. Die ene vergelijking is gedocumenteerde
/// orchestratie: de engine kent geen datumvergelijking (numeriek only).
async fn subsidiejaar_voor(state: &AppState, datum: &str) -> Result<i64, ApiError> {
    let parsed = chrono::NaiveDate::parse_from_str(datum, "%Y-%m-%d")
        .unwrap_or_else(|_| Utc::now().date_naive());
    let kandidaat = chrono::Datelike::year(&parsed) as i64 + 1;
    let termijnen = engine::evaluate_termijnen(state.corpus.clone(), kandidaat, datum.to_string())
        .await
        .map_err(internal_error)?;
    // ISO-datums (YYYY-MM-DD) vergelijken lexicografisch correct. Na de
    // termijn schuift de aanvraag één jaar op; de termijn van dat jaar kan
    // vandaag nooit ook al verstreken zijn (hij ligt in november van het
    // lopende jaar of later).
    if datum <= termijnen.aanvraagtermijn_einddatum.as_str() {
        Ok(kandidaat)
    } else {
        Ok(kandidaat + 1)
    }
}

pub(crate) async fn session_beoordelaar(session: &Session) -> Option<String> {
    let authenticated: bool = session
        .get(SESSION_KEY_AUTHENTICATED)
        .await
        .ok()
        .flatten()
        .unwrap_or(false);
    if !authenticated {
        return None;
    }
    let name: Option<String> = session.get(SESSION_KEY_NAME).await.ok().flatten();
    Some(name.unwrap_or_else(|| "beoordelaar".to_string()))
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct EherkenningLogin {
    pub kvk_nummer: String,
    /// Optional representation profile; absent means VOLLEDIG (backwards
    /// compatible with existing clients and the seed script).
    #[serde(default)]
    pub machtiging: Option<machtiging::Machtiging>,
}

/// MOCK eHerkenning-login voor aanvragers. eHerkenning levert alleen de
/// identiteit van de rechtspersoon (KvK); de partijnaam volgt uit het
/// partijregister van de Napp. Een onbekend KvK-nummer kan gewoon inloggen
/// en aanvragen — de wet wijst dan af (AWB 4:1: iedereen mag aanvragen).
/// De optionele machtiging beperkt de sessie tot één afdeling (volmacht).
pub async fn eherkenning_login(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<EherkenningLogin>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let kvk = body.kvk_nummer.trim().to_string();
    if !kvk.chars().all(|c| c.is_ascii_digit()) || kvk.len() != 8 {
        return Err(bad_request("Vul een geldig KVK-nummer in (8 cijfers)."));
    }
    let m = body.machtiging.unwrap_or_default();
    machtiging::valideer(&state.pool, &kvk, &m)
        .await
        .map_err(|e| match e {
            machtiging::ValidatieFout::Ongeldig(msg) => bad_request(&msg),
            machtiging::ValidatieFout::Intern(e) => internal_error(e),
        })?;
    // Een ONGEKOPPELD record draagt een placeholder-nummer: de echte
    // rechtspersoon is onbekend, dus de sessie krijgt de partijnaam niet.
    let naam = match crate::register::partij_by_kvk(&state.pool, &kvk)
        .await
        .map_err(internal_error)?
    {
        Some(partij) if !partij.is_ongekoppeld() => partij.naam,
        _ => format!("Organisatie {kvk}"),
    };
    session
        .insert(SESSION_KEY_EH_KVK, kvk.clone())
        .await
        .map_err(internal_error)?;
    session
        .insert(SESSION_KEY_EH_PARTIJ, naam.clone())
        .await
        .map_err(internal_error)?;
    session
        .insert(machtiging::SESSION_KEY_EH_MACHTIGING, m.clone())
        .await
        .map_err(internal_error)?;
    let machtiging_json = m.to_json(&state.pool).await.map_err(internal_error)?;
    Ok(Json(json!({
        "rol": "aanvrager",
        "kvk_nummer": kvk,
        "partij_naam": naam,
        "machtiging": machtiging_json,
        "mock": true,
    })))
}

/// Aanspraken van de ingelogde rechtspersoon volgens het partijregister,
/// met per onderdeel de beschikbaarheid voor het subsidiejaar. De aanvraag
/// volgt de rechtspersoon: een centraal georganiseerde partij ziet hier al
/// haar landelijke en decentrale aanspraken; een afdeling met eigen
/// rechtspersoon alleen de hare.
pub async fn mijn_registratie(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(kvk) = session_kvk(&session).await else {
        return Err(forbidden());
    };
    let vandaag = Utc::now().format("%Y-%m-%d").to_string();
    let jaar = subsidiejaar_voor(&state, &vandaag).await?;
    // ONGEKOPPELD telt als niet geregistreerd: de rechtspersoon achter de
    // aanduiding is onbekend, dus dit placeholder-nummer heeft geen
    // registratie (en de frontend biedt de claim-flow aan).
    let partij = crate::register::partij_by_kvk(&state.pool, &kvk)
        .await
        .map_err(internal_error)?
        .filter(|p| !p.is_ongekoppeld());
    let feiten = db::onderdeel_feiten(&state.pool, &kvk, jaar)
        .await
        .map_err(internal_error)?;
    let termijnen = engine::evaluate_termijnen(state.corpus.clone(), jaar, vandaag)
        .await
        .map_err(internal_error)?;

    // A limited machtiging (branch volmacht) only sees its own area.
    let m = machtiging::session_machtiging(&session).await;
    let componenten = m.filter_componenten(
        aanspraken_voor(&state.pool, &kvk)
            .await
            .map_err(internal_error)?,
    );
    let aanspraken: Vec<serde_json::Value> = componenten
        .iter()
        .map(|c| {
            // Weergavelabel uit de rauwe feiten; wat een nieuwe aanvraag
            // blokkeert beslist art. 13 bij het indienen.
            let feit = feiten.get(&c.key).copied().unwrap_or_default();
            let status = if feit.in_behandeling {
                "IN_BEHANDELING"
            } else if feit.eerder_toegekend {
                "TOEGEKEND"
            } else {
                "BESCHIKBAAR"
            };
            let mut v = serde_json::to_value(c).unwrap_or_default();
            v["status"] = json!(status);
            v
        })
        .collect();

    Ok(Json(json!({
        "partij": partij.map(|p| json!({
            "naam": p.naam,
            "kamerzetels": p.kamerzetels,
            "organisatiemodel": p.organisatiemodel,
        })),
        "subsidiejaar": jaar,
        "aanvraagtermijn_einddatum": termijnen.aanvraagtermijn_einddatum,
        "beslistermijn_einddatum": termijnen.beslistermijn_einddatum,
        "aanspraken": aanspraken,
    })))
}

/// Bouw de componenten (aanspraken) van een rechtspersoon uit het register.
/// Een onbekende organisatie krijgt één landelijke component met nul zetels:
/// de aanvraagroute blijft open (AWB 4:1), de wet wijst af.
async fn aanspraken_voor(pool: &sqlx::SqlitePool, kvk: &str) -> anyhow::Result<Vec<db::Component>> {
    // Expliciete check: een ONGEKOPPELD record geeft géén aanspraken. Het
    // kvk_nummer is daar een placeholder; wie ermee inlogt is niet de
    // rechtspersoon achter de aanduiding (die is juist nog onbekend).
    let partij = match crate::register::partij_by_kvk(pool, kvk).await? {
        Some(p) if !p.is_ongekoppeld() => p,
        _ => {
            return Ok(vec![db::Component {
                key: "LANDELIJK".into(),
                soort: "LANDELIJK".into(),
                orgaan: None,
                gebied_code: None,
                gebied: None,
                zetels: 0,
                inwoneraantal: 0,
            }]);
        }
    };
    let uitslagen = crate::register::uitslagen_met_gebied(pool, kvk).await?;
    let mut componenten = Vec::new();
    // Landelijke component alleen voor partijen met kamerzetels; een
    // afdeling of lokale partij heeft die aanspraak niet.
    if partij.kamerzetels > 0 || uitslagen.is_empty() {
        componenten.push(db::Component {
            key: "LANDELIJK".into(),
            soort: "LANDELIJK".into(),
            orgaan: None,
            gebied_code: None,
            gebied: None,
            zetels: partij.kamerzetels,
            inwoneraantal: 0,
        });
    }
    for u in uitslagen {
        componenten.push(db::Component {
            key: format!("{}:{}", u.orgaan, u.gebied_code),
            soort: "DECENTRAAL".into(),
            orgaan: Some(u.orgaan),
            gebied_code: Some(u.gebied_code),
            gebied: u.gebied_naam,
            zetels: u.zetels,
            inwoneraantal: u.inwoneraantal,
        });
    }
    Ok(componenten)
}

/// Upper bound for self-reported member counts. Generous (the largest Dutch
/// party has ~100k members), but keeps the engine arithmetic within the safe
/// integer range: ledenbudget × leden must stay far below 2^53.
const MAX_LEDEN: i64 = 10_000_000;

/// Eigen opgaven voor de landelijke component: ledental en de aangewezen
/// neveninstellingen (Wpp art. 3, 4 en 14, onderdelen b-d).
fn neem_landelijke_opgaven_over(
    bron: &serde_json::Map<String, serde_json::Value>,
    eigen: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<(), ApiError> {
    let leden = bron
        .get("aantal_betalende_leden")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if !(0..=MAX_LEDEN).contains(&leden) {
        return Err(bad_request(
            "Het opgegeven ledental is geen aannemelijk aantal (tussen 0 en 10.000.000).",
        ));
    }
    eigen.insert("aantal_betalende_leden".to_string(), json!(leden));
    for key in [
        "heeft_wetenschappelijk_instituut",
        "heeft_jongerenorganisatie",
        "heeft_instelling_buitenland",
    ] {
        let waarde = bron.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
        eigen.insert(key.to_string(), json!(waarde));
    }
    let pjo_leden = bron
        .get("aantal_leden_jongerenorganisatie")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if !(0..=MAX_LEDEN).contains(&pjo_leden) {
        return Err(bad_request(
            "Het opgegeven ledental van de jongerenorganisatie is geen aannemelijk aantal (tussen 0 en 10.000.000).",
        ));
    }
    eigen.insert(
        "aantal_leden_jongerenorganisatie".to_string(),
        json!(pjo_leden),
    );
    Ok(())
}

/// De noemer van de ledencomponent voor een (proef)berekening: de opgaven
/// die al in de aanvragentabel staan, plus — als de eigen aanvraag daar nog
/// niet bij zit — de eigen opgave. Of een opgave meetelt bepaalt de wet
/// (art. 6 jo. art. 14); hier wordt alleen verzameld en opgeteld.
async fn ledentotaal_voor(
    state: &AppState,
    jaar: i64,
    eigen: Option<db::LandelijkeOpgave>,
    peildatum: &str,
) -> Result<i64, ApiError> {
    let mut opgaven = db::landelijke_opgaven(&state.pool, jaar)
        .await
        .map_err(internal_error)?;
    if let Some(opgave) = eigen {
        opgaven.push(opgave);
    }
    engine::ledentotaal(state.corpus.clone(), opgaven, peildatum.to_string())
        .await
        .map_err(internal_error)
}

/// De peildatum voor de gegevens en de berekening van een subsidiejaar:
/// 1 januari van dat jaar (Wpp art. 17). Hiermee selecteert de engine ook
/// de wetversies die voor het subsidiejaar gelden.
fn peildatum_voor(subsidiejaar: i64) -> String {
    format!("{subsidiejaar}-01-01")
}

/// Demo-voorbeelden voor de gemockte eHerkenning-login (alleen metadata).
/// Het claim-demo-nummer is dynamisch: zodra een nummer via de claim-flow
/// aan een aanduiding is gekoppeld, schuift het voorbeeld door naar het
/// eerstvolgende vrije nummer, zodat de flow demonstreerbaar blijft.
pub async fn register_demo(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut voorbeelden =
        serde_json::to_value(crate::register::demo_voorbeelden()).map_err(internal_error)?;
    if let Some(lijst) = voorbeelden.as_array_mut() {
        for v in lijst {
            let is_claim_demo = v["profiel"]
                .as_str()
                .is_some_and(|p| p.contains("claim-flow"));
            if !is_claim_demo {
                continue;
            }
            let kvk = v["kvk_nummer"].as_str().unwrap_or_default().to_string();
            if let Some(vrij) = crate::register::vrij_demo_kvk(&state.pool, &kvk)
                .await
                .map_err(internal_error)?
            {
                v["kvk_nummer"] = json!(vrij);
            }
        }
    }
    Ok(Json(voorbeelden))
}

pub async fn eherkenning_logout(session: Session) -> Result<Json<serde_json::Value>, ApiError> {
    session
        .remove::<String>(SESSION_KEY_EH_KVK)
        .await
        .map_err(internal_error)?;
    session
        .remove::<String>(SESSION_KEY_EH_PARTIJ)
        .await
        .map_err(internal_error)?;
    session
        .remove::<machtiging::Machtiging>(machtiging::SESSION_KEY_EH_MACHTIGING)
        .await
        .map_err(internal_error)?;
    Ok(Json(json!({"uitgelogd": true})))
}

#[derive(Deserialize)]
pub struct MockSsoLogin {
    pub naam: String,
}

/// MOCK SSO-login voor beoordelaars — alleen actief wanneer OIDC niet is
/// geconfigureerd (lokale ontwikkeling). Zet dezelfde sessiesleutels als de
/// echte regelrecht-auth callback, zodat /auth/status gewoon werkt.
pub async fn sso_mock_login(
    session: Session,
    Json(body): Json<MockSsoLogin>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let naam = body.naam.trim();
    if naam.is_empty() {
        return Err(bad_request("Naam is verplicht."));
    }
    session
        .insert(SESSION_KEY_AUTHENTICATED, true)
        .await
        .map_err(internal_error)?;
    session
        .insert(SESSION_KEY_NAME, naam.to_string())
        .await
        .map_err(internal_error)?;
    session
        .insert(
            SESSION_KEY_EMAIL,
            format!("{}@napp.nl", naam.replace(' ', ".").to_lowercase()),
        )
        .await
        .map_err(internal_error)?;
    session
        .insert(SESSION_KEY_SUB, format!("mock-{naam}"))
        .await
        .map_err(internal_error)?;
    Ok(Json(
        json!({"rol": "beoordelaar", "naam": naam, "mock": true}),
    ))
}

/// De weergavenaam van een ingelogde rechtspersoon. De sessie draagt alleen
/// de identiteit (KvK); de naam komt live uit het register, zodat hij ook
/// klopt direct nadat een claim is bevestigd (zonder opnieuw inloggen).
pub(crate) async fn partijnaam_voor(state: &AppState, kvk: &str) -> Result<String, ApiError> {
    Ok(
        match crate::register::partij_by_kvk(&state.pool, kvk)
            .await
            .map_err(internal_error)?
        {
            Some(partij) if !partij.is_ongekoppeld() => partij.naam,
            _ => format!("Organisatie {kvk}"),
        },
    )
}

/// Gecombineerde sessie-status voor de frontend: welke rol(len) zijn actief.
pub async fn me(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<serde_json::Value>, ApiError> {
    let kvk = session_kvk(&session).await;
    let partij = match &kvk {
        Some(k) => Some(partijnaam_voor(&state, k).await?),
        None => None,
    };
    let m = machtiging::session_machtiging(&session).await;
    let machtiging_json = m.to_json(&state.pool).await.map_err(internal_error)?;
    let beoordelaar = session_beoordelaar(&session).await;
    Ok(Json(json!({
        "aanvrager": kvk.map(|k| json!({
            "kvk_nummer": k,
            "partij_naam": partij,
            "machtiging": machtiging_json,
        })),
        "beoordelaar": beoordelaar.map(|n| json!({"naam": n})),
    })))
}

// ---------------------------------------------------------------------------
// Aanvragen (aanvrager)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct NieuweAanvraag {
    /// Sleutels van de aan te vragen onderdelen ("LANDELIJK" of "{orgaan}:{code}").
    pub componenten: Vec<String>,
    /// Eigen opgaven: aantal_betalende_leden en de vier transparantieverklaringen.
    pub parameters: serde_json::Map<String, serde_json::Value>,
}

pub async fn create_aanvraag(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<NieuweAanvraag>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(kvk) = session_kvk(&session).await else {
        return Err(forbidden());
    };
    let partij = partijnaam_voor(&state, &kvk).await?;
    let vandaag = Utc::now().format("%Y-%m-%d").to_string();
    let jaar = subsidiejaar_voor(&state, &vandaag).await?;

    if body.componenten.is_empty() {
        return Err(bad_request(
            "Kies ten minste één onderdeel om aan te vragen.",
        ));
    }

    // De componenten komen uit het register, nooit uit de client: de client
    // stuurt alleen sleutels; gegevens (zetels, inwoneraantallen) worden hier
    // bevroren op registerwaarden.
    let m = machtiging::session_machtiging(&session).await;
    let beschikbaar = aanspraken_voor(&state.pool, &kvk)
        .await
        .map_err(internal_error)?;
    let mut componenten: Vec<db::Component> = Vec::new();
    for key in &body.componenten {
        if !m.allows_key(key) {
            return Err(forbidden_with(&format!(
                "Uw machtiging als afdelingsbestuurder geldt niet voor onderdeel '{key}'. \
                 Log in namens de gehele partij om dit onderdeel aan te vragen."
            )));
        }
        let Some(c) = beschikbaar.iter().find(|c| &c.key == key) else {
            return Err(bad_request(&format!(
                "Onderdeel '{key}' hoort niet bij uw registratie."
            )));
        };
        componenten.push(c.clone());
    }

    // Eenmalige verstrekking per subsidiejaar (art. 13): de rauwe feiten
    // komen uit de aanvragentabel, het oordeel per onderdeel komt uit de
    // wet — inclusief de regel dat een eerdere afwijzing niet blokkeert.
    let feiten = db::onderdeel_feiten(&state.pool, &kvk, jaar)
        .await
        .map_err(internal_error)?;
    let per_onderdeel: Vec<db::OnderdeelFeiten> = componenten
        .iter()
        .map(|c| feiten.get(&c.key).copied().unwrap_or_default())
        .collect();
    let beschikbaar =
        engine::beschikbare_onderdelen(state.corpus.clone(), per_onderdeel, vandaag.clone())
            .await
            .map_err(internal_error)?;
    if let Some(c) = componenten
        .iter()
        .zip(&beschikbaar)
        .find(|(_, ok)| !**ok)
        .map(|(c, _)| c)
    {
        return Err(bad_request(&format!(
            "Onderdeel '{}' is voor {jaar} al aangevraagd of toegekend (artikel 13 Wpp).",
            c.key
        )));
    }

    // Eigen opgaven: verklaringen verplicht; ledental en neveninstellingen
    // optioneel (alleen relevant voor de landelijke component).
    let mut eigen = serde_json::Map::new();
    for key in [
        "ontvangt_anonieme_giften",
        "ontvangt_giften_niet_ingezetenen",
        "voldoet_aan_meldplicht_giften",
        "financien_openbaar_op_website",
    ] {
        let Some(waarde) = body.parameters.get(key).and_then(|v| v.as_bool()) else {
            return Err(bad_request(&format!(
                "Verplichte verklaring '{key}' ontbreekt."
            )));
        };
        eigen.insert(key.to_string(), json!(waarde));
    }
    neem_landelijke_opgaven_over(&body.parameters, &mut eigen)?;

    let id = Uuid::new_v4().to_string();
    // Wpp art. 17 (lex specialis t.o.v. AWB 4:13): de Napp besluit voor
    // 1 januari van het subsidiejaar.
    let termijnen = engine::evaluate_termijnen(state.corpus.clone(), jaar, vandaag.clone())
        .await
        .map_err(internal_error)?;
    let beslistermijn = termijnen.beslistermijn_einddatum;
    // Een ingediende aanvraag belandt in de stage na de indiening; welke dat
    // is bepaalt de procedure in de wet (AANVRAAG → BEHANDELING).
    let status = state.procedure.na_indiening().map_err(internal_error)?;
    db::insert_aanvraag(
        &state.pool,
        &id,
        &kvk,
        &partij,
        jaar,
        &serde_json::to_string(&componenten).map_err(internal_error)?,
        &serde_json::to_string(&eigen).map_err(internal_error)?,
        status,
        &vandaag,
        Some(&beslistermijn),
    )
    .await
    .map_err(internal_error)?;

    Ok(Json(json!({
        "id": id,
        "status": status,
        "subsidiejaar": jaar,
        "beslistermijn_einddatum": beslistermijn,
    })))
}

/// Indicatieve berekening voor de aanvrager: de wet uitgevoerd op de
/// gekozen onderdelen, zonder iets vast te leggen. Zo ziet de partij bij het
/// invullen al wat de wet zou beslissen.
pub async fn proef_aanspraken(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<NieuweAanvraag>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(kvk) = session_kvk(&session).await else {
        return Err(forbidden());
    };
    let m = machtiging::session_machtiging(&session).await;
    let beschikbaar = m.filter_componenten(
        aanspraken_voor(&state.pool, &kvk)
            .await
            .map_err(internal_error)?,
    );
    let componenten: Vec<db::Component> = beschikbaar
        .into_iter()
        .filter(|c| body.componenten.contains(&c.key))
        .collect();
    if componenten.is_empty() {
        return Err(bad_request("Geen onderdelen gekozen."));
    }
    let mut eigen = serde_json::Map::new();
    for key in [
        "ontvangt_anonieme_giften",
        "ontvangt_giften_niet_ingezetenen",
        "voldoet_aan_meldplicht_giften",
        "financien_openbaar_op_website",
    ] {
        let waarde = body
            .parameters
            .get(key)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        eigen.insert(key.to_string(), json!(waarde));
    }
    neem_landelijke_opgaven_over(&body.parameters, &mut eigen)?;

    let vandaag = Utc::now().format("%Y-%m-%d").to_string();
    let jaar = subsidiejaar_voor(&state, &vandaag).await?;
    // De eigen opgave telt mee in de noemer van de ledencomponent als de
    // landelijke component onderdeel is van deze (proef)aanvraag; of zij
    // werkelijk meetelt beslist de wet in ledentotaal_voor.
    let eigen_opgave = componenten
        .iter()
        .find(|c| c.soort == "LANDELIJK")
        .map(|c| db::LandelijkeOpgave {
            zetels: c.zetels,
            parameters: eigen.clone(),
        });
    let peildatum = peildatum_voor(jaar);
    let totaal_leden = ledentotaal_voor(&state, jaar, eigen_opgave, &peildatum).await?;
    let uitkomst = engine::evaluate_jaaraanvraag(
        state.corpus.clone(),
        componenten,
        eigen,
        peildatum,
        jaar,
        totaal_leden,
    )
    .await
    .map_err(internal_error)?;
    Ok(Json(json!({
        "subsidie_toegekend": uitkomst.subsidie_toegekend,
        "subsidiebedrag": uitkomst.subsidiebedrag,
        "voorschot_bedrag": uitkomst.betaalopdracht_bedrag,
        "subsidiejaar": jaar,
        "onderdelen_toegekend": uitkomst.componenten.iter().filter(|c| c.toegekend).count(),
        "onderdelen_totaal": uitkomst.componenten.len(),
        // De motivering legt per afgewezen onderdeel uit waarom; zo ziet de
        // aanvrager in de indicatie ook wát er zou sneuvelen.
        "motivering": uitkomst.motivering,
    })))
}

async fn aanvragen_met_besluit(
    state: &AppState,
    aanvragen: Vec<db::Aanvraag>,
) -> Result<Vec<serde_json::Value>, ApiError> {
    let mut result = Vec::with_capacity(aanvragen.len());
    for aanvraag in aanvragen {
        let besluit = db::get_besluit_by_aanvraag(&state.pool, &aanvraag.id)
            .await
            .map_err(internal_error)?;
        result.push(json!({"aanvraag": aanvraag, "besluit": besluit}));
    }
    Ok(result)
}

/// Aanvragersportaal: uitsluitend de aanvragen van de eigen rechtspersoon.
/// Bewust een eigen endpoint: een sessie kan beide rollen tegelijk dragen
/// en mag in dit portaal nooit andermans dossiers zien.
pub async fn list_mijn_aanvragen(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(kvk) = session_kvk(&session).await else {
        return Err(forbidden());
    };
    // A limited machtiging only sees dossiers that fall entirely within its
    // scope; other dossiers of the same legal entity stay hidden.
    let m = machtiging::session_machtiging(&session).await;
    let aanvragen: Vec<db::Aanvraag> = db::list_aanvragen(&state.pool, Some(&kvk))
        .await
        .map_err(internal_error)?
        .into_iter()
        .filter(|a| m.covers(&a.componenten))
        .collect();
    Ok(Json(json!(aanvragen_met_besluit(&state, aanvragen).await?)))
}

pub async fn get_mijn_aanvraag(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(kvk) = session_kvk(&session).await else {
        return Err(forbidden());
    };
    let Some(aanvraag) = db::get_aanvraag(&state.pool, &id)
        .await
        .map_err(internal_error)?
    else {
        return Err(not_found());
    };
    if aanvraag.kvk_nummer != kvk {
        return Err(not_found());
    }
    // Outside the machtiging = does not exist for this session (404, not 403:
    // a branch board member should not learn about other dossiers).
    let m = machtiging::session_machtiging(&session).await;
    if !m.covers(&aanvraag.componenten) {
        return Err(not_found());
    }
    let besluit = db::get_besluit_by_aanvraag(&state.pool, &id)
        .await
        .map_err(internal_error)?;
    let betaalopdracht = betaalopdracht_voor(&state, besluit.as_ref()).await?;
    let bezwaar = bezwaar_voor(&state, besluit.as_ref()).await?;
    Ok(Json(json!({
        "aanvraag": aanvraag,
        "besluit": besluit,
        "betaalopdracht": betaalopdracht,
        "bezwaar": bezwaar,
    })))
}

/// Het (laatste) bezwaar tegen een besluit, voor het dossierbeeld.
async fn bezwaar_voor(
    state: &AppState,
    besluit: Option<&db::Besluit>,
) -> Result<Option<crate::bezwaar::Bezwaar>, ApiError> {
    let Some(besluit) = besluit else {
        return Ok(None);
    };
    crate::bezwaar::bezwaar_by_besluit(&state.pool, &besluit.id)
        .await
        .map_err(internal_error)
}

/// De betaalopdracht die uit een besluit voortvloeit, voor het
/// dossierbeeld (aanvrager en beoordelaar).
async fn betaalopdracht_voor(
    state: &AppState,
    besluit: Option<&db::Besluit>,
) -> Result<Option<db::Betaalopdracht>, ApiError> {
    let Some(besluit) = besluit else {
        return Ok(None);
    };
    db::get_betaalopdracht_by_besluit(&state.pool, &besluit.id)
        .await
        .map_err(internal_error)
}

/// Beoordelingsomgeving: de volledige werkvoorraad.
pub async fn list_aanvragen(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<serde_json::Value>, ApiError> {
    if session_beoordelaar(&session).await.is_none() {
        return Err(forbidden());
    }
    let aanvragen = db::list_aanvragen(&state.pool, None)
        .await
        .map_err(internal_error)?;
    Ok(Json(json!(aanvragen_met_besluit(&state, aanvragen).await?)))
}

pub async fn get_aanvraag(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if session_beoordelaar(&session).await.is_none() {
        return Err(forbidden());
    }
    let Some(aanvraag) = db::get_aanvraag(&state.pool, &id)
        .await
        .map_err(internal_error)?
    else {
        return Err(not_found());
    };
    let besluit = db::get_besluit_by_aanvraag(&state.pool, &id)
        .await
        .map_err(internal_error)?;
    let betaalopdracht = betaalopdracht_voor(&state, besluit.as_ref()).await?;
    let bezwaar = bezwaar_voor(&state, besluit.as_ref()).await?;
    Ok(Json(json!({
        "aanvraag": aanvraag,
        "besluit": besluit,
        "betaalopdracht": betaalopdracht,
        "bezwaar": bezwaar,
    })))
}

// ---------------------------------------------------------------------------
// Beoordelen (beoordelaar)
// ---------------------------------------------------------------------------

/// Proefberekening: voer de wet per onderdeel uit zonder iets vast te leggen.
pub async fn proefberekening(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if session_beoordelaar(&session).await.is_none() {
        return Err(forbidden());
    }
    let Some(aanvraag) = db::get_aanvraag(&state.pool, &id)
        .await
        .map_err(internal_error)?
    else {
        return Err(not_found());
    };
    let uitkomst = run_engine(&state, &aanvraag).await?;
    Ok(Json(
        serde_json::to_value(uitkomst).map_err(internal_error)?,
    ))
}

/// Besluit vaststellen: de wet bepaalt per onderdeel; samen één beschikking.
/// Bij toekenning ontstaat één betaalopdracht aan de rechtspersoon (art. 27).
pub async fn stel_besluit_vast(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(beoordelaar) = session_beoordelaar(&session).await else {
        return Err(forbidden());
    };
    let Some(aanvraag) = db::get_aanvraag(&state.pool, &id)
        .await
        .map_err(internal_error)?
    else {
        return Err(not_found());
    };
    // De overgang naar BESLUIT moet passen in de procedure van de wet; een
    // aanvraag die al in of voorbij BESLUIT is, kan niet opnieuw.
    if state
        .procedure
        .transitie(&aanvraag.status, engine::STAGE_BESLUIT)
        .is_err()
    {
        return Err(bad_request(
            "Deze aanvraag is al beoordeeld of nog niet in behandeling.",
        ));
    }

    let uitkomst = run_engine(&state, &aanvraag).await?;
    let besluit_id = Uuid::new_v4().to_string();
    let vandaag = Utc::now().format("%Y-%m-%d").to_string();

    db::insert_besluit(
        &state.pool,
        &besluit_id,
        &id,
        uitkomst.subsidie_toegekend,
        uitkomst.subsidiebedrag,
        &serde_json::to_string(&uitkomst.componenten).map_err(internal_error)?,
        &uitkomst.motivering,
        &vandaag,
        &beoordelaar,
        &serde_json::to_string(&uitkomst.bewijs).map_err(internal_error)?,
    )
    .await
    .map_err(internal_error)?;
    db::set_aanvraag_status(&state.pool, &id, engine::STAGE_BESLUIT)
        .await
        .map_err(internal_error)?;

    // Side-effect uit art. 16/27: één betaalopdracht aan de rechtspersoon.
    // De rekening komt uit het partijregister (eigen opgave van het
    // tekenbevoegd bestuur, zie rekening.rs). Of de uitbetaling zonder
    // bekende rekening wordt aangehouden, beslist art. 27; de orchestratie
    // levert alleen het feit dat er (g)een rekening bekend is.
    let mut betaalopdracht_status: Option<&str> = None;
    if uitkomst.betaalopdracht_vereist {
        let partij = crate::register::partij_by_kvk(&state.pool, &aanvraag.kvk_nummer)
            .await
            .map_err(internal_error)?;
        let rekening = partij
            .as_ref()
            .and_then(|p| p.iban.clone().zip(p.iban_tenaamstelling.clone()));
        // Het naam-feit wordt hier opnieuw vastgesteld tegen de huidige
        // registratie, niet afgeleid uit "er is een rekening opgeslagen".
        let op_naam = match (&partij, &rekening) {
            (Some(p), Some((_, tenaamstelling))) => {
                crate::rekening::naam_komt_overeen(tenaamstelling, Some(&p.naam))
            }
            _ => false,
        };
        let toets = engine::evaluate_rekening(
            state.corpus.clone(),
            op_naam,
            false,
            rekening.is_some(),
            vandaag.clone(),
        )
        .await
        .map_err(internal_error)?;
        let status = if toets.uitbetaling_aangehouden {
            db::BETAAL_AANGEHOUDEN
        } else {
            db::BETAAL_AANGEMAAKT
        };
        let opdracht_id = Uuid::new_v4().to_string();
        db::insert_betaalopdracht(
            &state.pool,
            &opdracht_id,
            &besluit_id,
            &aanvraag.partij_naam,
            uitkomst.betaalopdracht_bedrag,
            rekening.as_ref().map(|(iban, _)| iban.as_str()),
            rekening.as_ref().map(|(_, naam)| naam.as_str()),
            status,
        )
        .await
        .map_err(internal_error)?;
        betaalopdracht_status = Some(status);
        tracing::info!(
            besluit = %besluit_id,
            bedrag = uitkomst.betaalopdracht_bedrag,
            status,
            "betaalopdracht aangemaakt (mock betaalsysteem)"
        );
    }

    Ok(Json(json!({
        "besluit_id": besluit_id,
        "uitkomst": uitkomst,
        // AANGEMAAKT, AANGEHOUDEN (geen rekening bekend) of null (geen
        // betaalopdracht); de UI kan een aangehouden opdracht hiermee melden.
        "betaalopdracht_status": betaalopdracht_status,
    })))
}

async fn run_engine(
    state: &AppState,
    aanvraag: &db::Aanvraag,
) -> Result<engine::JaaruitkomstUitkomst, ApiError> {
    let serde_json::Value::Object(eigen) = aanvraag.parameters.clone() else {
        return Err(internal_error("aanvraagparameters zijn geen object"));
    };
    let peildatum = peildatum_voor(aanvraag.subsidiejaar);
    // De eigen aanvraag staat al in de aanvragentabel en telt dus al mee in
    // het ledentotaal van het subsidiejaar.
    let totaal_leden = ledentotaal_voor(state, aanvraag.subsidiejaar, None, &peildatum).await?;
    engine::evaluate_jaaraanvraag(
        state.corpus.clone(),
        aanvraag.componenten.clone(),
        eigen,
        peildatum,
        aanvraag.subsidiejaar,
        totaal_leden,
    )
    .await
    .map_err(internal_error)
}

/// Bekendmaking: het besluit wordt bekendgemaakt; de AWB (6:8) bepaalt de
/// bezwaartermijn vanaf de dag na bekendmaking.
pub async fn bekendmaking(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if session_beoordelaar(&session).await.is_none() {
        return Err(forbidden());
    }
    let Some(aanvraag) = db::get_aanvraag(&state.pool, &id)
        .await
        .map_err(internal_error)?
    else {
        return Err(not_found());
    };
    // Bekendmaking moet volgens de procedure ná het besluit komen; daarna
    // gaat het dossier door naar de bezwaarperiode (momentane stage).
    if state
        .procedure
        .transitie(&aanvraag.status, engine::STAGE_BEKENDMAKING)
        .is_err()
    {
        return Err(bad_request("Er is nog geen besluit om bekend te maken."));
    }
    let Some(besluit) = db::get_besluit_by_aanvraag(&state.pool, &id)
        .await
        .map_err(internal_error)?
    else {
        return Err(not_found());
    };

    let vandaag = Utc::now().format("%Y-%m-%d").to_string();
    let termijn = engine::evaluate_bezwaartermijn(state.corpus.clone(), vandaag.clone())
        .await
        .map_err(internal_error)?;

    db::set_bekendmaking(
        &state.pool,
        &besluit.id,
        &vandaag,
        &termijn.startdatum,
        &termijn.einddatum,
    )
    .await
    .map_err(internal_error)?;
    // Met de bekendmaking gaat de betalingstermijn van het voorschot lopen
    // (AWB 4:87: binnen zes weken). De betaalopdracht draagt die termijn.
    let betaaltermijn = engine::evaluate_betaaltermijn(state.corpus.clone(), vandaag.clone())
        .await
        .map_err(internal_error)?;
    db::set_betaaltermijn(&state.pool, &besluit.id, &betaaltermijn)
        .await
        .map_err(internal_error)?;
    db::set_aanvraag_status(&state.pool, &id, engine::STAGE_BEZWAAR)
        .await
        .map_err(internal_error)?;

    Ok(Json(json!({
        "bekendmaking_datum": vandaag,
        "bezwaartermijn_startdatum": termijn.startdatum,
        "bezwaartermijn_einddatum": termijn.einddatum,
    })))
}

// ---------------------------------------------------------------------------
// Betaalopdrachten (beoordelaar)
// ---------------------------------------------------------------------------

pub async fn list_betaalopdrachten(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<serde_json::Value>, ApiError> {
    if session_beoordelaar(&session).await.is_none() {
        return Err(forbidden());
    }
    let opdrachten = db::list_betaalopdrachten(&state.pool)
        .await
        .map_err(internal_error)?;
    Ok(Json(json!(opdrachten)))
}

/// POST /api/betaalopdrachten/{id}/uitbetalen — verstuur een aangemaakte
/// opdracht naar het (gesimuleerde) betaalsysteem. De uitbetaling is een
/// feitelijke handeling, geen rechtshandeling: het recht op het voorschot
/// volgt al uit het besluit (art. 16/17 Wpp) en de betalingstermijn uit
/// AWB 4:87. Een aangehouden opdracht kan niet worden verstuurd: art. 27
/// houdt de uitbetaling aan zolang geen rekening van de rechtspersoon
/// bekend is.
pub async fn betaal_uit(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if session_beoordelaar(&session).await.is_none() {
        return Err(forbidden());
    }
    let Some(opdracht) = db::get_betaalopdracht(&state.pool, &id)
        .await
        .map_err(internal_error)?
    else {
        return Err(not_found());
    };
    if opdracht.status == db::BETAAL_AANGEHOUDEN {
        return Err(bad_request(
            "Deze betaalopdracht is aangehouden: er is geen rekening van de rechtspersoon \
             bekend (artikel 27 Wpp). Zodra het bestuur een rekening opgeeft, wordt de \
             opdracht automatisch klaargezet.",
        ));
    }
    if !db::markeer_uitbetaald(&state.pool, &id)
        .await
        .map_err(internal_error)?
    {
        return Err(bad_request("Deze betaalopdracht is al uitbetaald."));
    }
    tracing::info!(opdracht = %id, bedrag = opdracht.bedrag, "voorschot uitbetaald (mock betaalsysteem)");
    let opdracht = db::get_betaalopdracht(&state.pool, &id)
        .await
        .map_err(internal_error)?
        .ok_or_else(not_found)?;
    Ok(Json(json!(opdracht)))
}

// ---------------------------------------------------------------------------
// Openbaar register (geen login)
// ---------------------------------------------------------------------------

pub async fn register(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let entries = db::list_register(&state.pool)
        .await
        .map_err(internal_error)?;
    Ok(Json(json!(entries)))
}

pub async fn statistieken(
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let stats = db::statistieken(&state.pool)
        .await
        .map_err(internal_error)?;
    Ok(Json(serde_json::to_value(stats).map_err(internal_error)?))
}

#[derive(Serialize)]
pub struct Health {
    pub status: &'static str,
}

pub async fn health() -> Json<Health> {
    Json(Health { status: "ok" })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    /// Een ONGEKOPPELD record geeft géén aanspraken: het placeholder-nummer
    /// hoort niet bij de (onbekende) rechtspersoon achter de aanduiding.
    #[tokio::test]
    async fn aanspraken_voor_ongekoppeld_record_zijn_leeg() {
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
        sqlx::query(
            "INSERT INTO register_partijen (kvk_nummer, naam, organisatiemodel, kamerzetels, status)
             VALUES ('98765432', 'Zwolse Stadspartij', 'CENTRAAL', 0, 'ONGEKOPPELD')",
        )
        .execute(&pool)
        .await
        .expect("ongekoppeld fixture");
        crate::register::insert_uitslag(&pool, "98765432", "GEMEENTERAAD", "GM0193", 3)
            .await
            .expect("uitslag fixture");

        // Zelfde uitkomst als een onbekend nummer: één lege landelijke
        // component (de wet wijst af), géén decentrale aanspraken.
        let componenten = aanspraken_voor(&pool, "98765432")
            .await
            .expect("aanspraken");
        assert_eq!(componenten.len(), 1);
        assert_eq!(componenten[0].key, "LANDELIJK");
        assert_eq!(componenten[0].zetels, 0);
    }
}
