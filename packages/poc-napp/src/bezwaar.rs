//! Bezwaar tegen een verleningsbeschikking (AWB hoofdstuk 6 en 7).
//!
//! De orchestratie levert feiten (formuliervelden, datumvergelijkingen,
//! herstelhistorie); de oordelen komen uit de AWB-YAML: vereisten (6:5),
//! verzuimherstel (6:6), tijdigheid incl. verzendtheorie (6:9), afzien van
//! horen (7:3) en de beslistermijn (7:10). De heroverweging (7:11) is een
//! her-evaluatie van de Wpp op (eventueel gecorrigeerde) feiten — daarbij
//! vuren dezelfde hooks als bij het oorspronkelijke besluit. Bezwaar
//! schorst de werking van het besluit niet (6:16): de betaalopdracht
//! loopt bewust gewoon door.
//!
//! De hersteltermijn van twee weken is bestuurspraktijk, geen wet: AWB 6:6
//! eist alleen "een daartoe gestelde termijn".

use axum::Json;
use axum::extract::{Path, State};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Row, SqlitePool};
use tower_sessions::Session;
use uuid::Uuid;

use crate::db;
use crate::engine;
use crate::handlers::{
    ApiError, bad_request, forbidden, internal_error, not_found, session_beoordelaar, session_kvk,
};
use crate::machtiging;
use crate::state::AppState;

pub const BESLISSING_NIET_ONTVANKELIJK: &str = "NIET_ONTVANKELIJK";
pub const BESLISSING_ONGEGROND: &str = "ONGEGROND";
pub const BESLISSING_GEGROND: &str = "GEGROND";

/// Hersteltermijn (bestuurspraktijk, zie module-doc): twee weken.
const HERSTELTERMIJN_DAGEN: i64 = 14;

#[derive(Debug, Clone, Serialize)]
pub struct Bezwaar {
    pub id: String,
    pub besluit_id: String,
    pub aanvraag_id: String,
    pub kvk_nummer: String,
    pub naam_indiener: String,
    pub adres_indiener: Option<String>,
    pub gronden: Option<String>,
    pub ondertekend: bool,
    pub ontvangen_op: String,
    /// AWB-oordeel over het bezwaarschrift (outputs 6:5/6:6/6:9).
    pub toets: serde_json::Value,
    pub status: String,
    pub beslistermijn_einddatum: Option<String>,
    pub gehoord: Option<bool>,
    pub afzien_grond: Option<String>,
    pub beslissing: Option<String>,
    pub beslissing_motivering: Option<String>,
    pub beslissing_datum: Option<String>,
    pub created_at: String,
}

fn row_to_bezwaar(row: &sqlx::sqlite::SqliteRow) -> Bezwaar {
    Bezwaar {
        id: row.get("id"),
        besluit_id: row.get("besluit_id"),
        aanvraag_id: row.get("aanvraag_id"),
        kvk_nummer: row.get("kvk_nummer"),
        naam_indiener: row.get("naam_indiener"),
        adres_indiener: row.get("adres_indiener"),
        gronden: row.get("gronden"),
        ondertekend: row.get::<i64, _>("ondertekend") != 0,
        ontvangen_op: row.get("ontvangen_op"),
        toets: row
            .get::<Option<String>, _>("toets")
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(serde_json::Value::Null),
        status: row.get("status"),
        beslistermijn_einddatum: row.get("beslistermijn_einddatum"),
        gehoord: row.get::<Option<i64>, _>("gehoord").map(|v| v != 0),
        afzien_grond: row.get("afzien_grond"),
        beslissing: row.get("beslissing"),
        beslissing_motivering: row.get("beslissing_motivering"),
        beslissing_datum: row.get("beslissing_datum"),
        created_at: row.get("created_at"),
    }
}

async fn bezwaar_by_id(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Bezwaar>> {
    let row = sqlx::query("SELECT * FROM bezwaren WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(row_to_bezwaar))
}

pub async fn bezwaar_by_besluit(
    pool: &SqlitePool,
    besluit_id: &str,
) -> anyhow::Result<Option<Bezwaar>> {
    let row = sqlx::query(
        "SELECT * FROM bezwaren WHERE besluit_id = ? ORDER BY created_at DESC, rowid DESC LIMIT 1",
    )
    .bind(besluit_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(row_to_bezwaar))
}

async fn list_open_bezwaren(pool: &SqlitePool) -> anyhow::Result<Vec<Bezwaar>> {
    let rows = sqlx::query(
        "SELECT * FROM bezwaren
         ORDER BY (status != 'BESLISSING') DESC, created_at DESC, rowid DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row_to_bezwaar).collect())
}

/// Bouw de feiten voor de AWB-toets uit het bezwaarschrift en het besluit.
/// De datumvergelijkingen (6:9) gebeuren hier: ISO-datums lexicografisch —
/// de engine kent geen datumvergelijking; de regel staat in de wet.
// Acht parameters: de vereisten van AWB 6:5/6:9 die de wet één voor één noemt.
// In een struct proppen zou ze aan het oog onttrekken zonder ze te vereenvoudigen.
#[allow(clippy::too_many_arguments)]
fn bezwaar_feiten(
    naam: &str,
    adres: Option<&str>,
    gronden: Option<&str>,
    ondertekend: bool,
    ontvangen_op: &str,
    bezwaartermijn_einddatum: Option<&str>,
    herstelgelegenheid_geboden: bool,
    binnen_hersteltermijn_aangevuld: bool,
) -> engine::BezwaarFeiten {
    let tijdig_ontvangen = match bezwaartermijn_einddatum {
        Some(einde) => ontvangen_op <= einde,
        None => false,
    };
    engine::BezwaarFeiten {
        naam_en_adres_vermeld: !naam.trim().is_empty()
            && adres.is_some_and(|a| !a.trim().is_empty()),
        // Digitale indiening: dagtekening en besluitomschrijving worden
        // door het portaal zelf meegegeven (ontvangstdatum + besluit-id).
        dagtekening_vermeld: true,
        besluit_omschreven: true,
        gronden_vermeld: gronden.is_some_and(|g| !g.trim().is_empty()),
        ondertekend,
        herstelgelegenheid_geboden,
        binnen_hersteltermijn_aangevuld,
        ontvangen_voor_einde_termijn: tijdig_ontvangen,
        // Digitale indiening kent geen postweg; de verzendtheorie-feiten
        // zijn dan onwaar en de wet valt terug op de ontvangsttheorie.
        ter_post_bezorgd_voor_einde_termijn: false,
        ontvangen_binnen_week_na_termijn: false,
    }
}

// ---------------------------------------------------------------------------
// Aanvrager: indienen en herstellen
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct NieuwBezwaar {
    pub naam_indiener: String,
    #[serde(default)]
    pub adres_indiener: Option<String>,
    #[serde(default)]
    pub gronden: Option<String>,
    #[serde(default)]
    pub ondertekend: bool,
}

/// POST /api/besluiten/{besluit_id}/bezwaar — bezwaarschrift indienen bij
/// het bestuursorgaan dat het besluit nam (AWB 6:4). De AWB toetst de
/// vereisten; bij een vormgebrek volgt herstelgelegenheid (6:6), geen
/// directe weigering.
pub async fn dien_bezwaar_in(
    State(state): State<AppState>,
    session: Session,
    Path(besluit_id): Path<String>,
    Json(body): Json<NieuwBezwaar>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(kvk) = session_kvk(&session).await else {
        return Err(forbidden());
    };
    let Some(besluit) = db::get_besluit_by_id(&state.pool, &besluit_id)
        .await
        .map_err(internal_error)?
    else {
        return Err(not_found());
    };
    let Some(aanvraag) = db::get_aanvraag(&state.pool, &besluit.aanvraag_id)
        .await
        .map_err(internal_error)?
    else {
        return Err(not_found());
    };
    if aanvraag.kvk_nummer != kvk {
        return Err(not_found());
    }
    let m = machtiging::session_machtiging(&session).await;
    if !m.covers(&aanvraag.componenten) {
        return Err(not_found());
    }
    if besluit.bekendmaking_datum.is_none() {
        return Err(bad_request(
            "Het besluit is nog niet bekendgemaakt; de bezwaartermijn is nog niet gestart \
             (AWB 6:8).",
        ));
    }
    if bezwaar_by_besluit(&state.pool, &besluit_id)
        .await
        .map_err(internal_error)?
        .is_some_and(|b| b.beslissing.is_none())
    {
        return Err(bad_request("Er loopt al een bezwaar tegen dit besluit."));
    }

    let vandaag = Utc::now().format("%Y-%m-%d").to_string();
    let feiten = bezwaar_feiten(
        &body.naam_indiener,
        body.adres_indiener.as_deref(),
        body.gronden.as_deref(),
        body.ondertekend,
        &vandaag,
        besluit.bezwaartermijn_einddatum.as_deref(),
        false,
        false,
    );
    let toets = engine::evaluate_bezwaarschrift(state.corpus.clone(), feiten, vandaag.clone())
        .await
        .map_err(internal_error)?;
    // AWB 7:10: de beslistermijn loopt vanaf het einde van de
    // bezwaartermijn, niet vanaf de ontvangst.
    let beslistermijn = match &besluit.bezwaartermijn_einddatum {
        Some(einde) => Some(
            engine::evaluate_beslistermijn_bezwaar(
                state.corpus.clone(),
                einde.clone(),
                vandaag.clone(),
            )
            .await
            .map_err(internal_error)?,
        ),
        None => None,
    };

    // Vormgebrek → herstelfase (6:6); anders direct in behandeling. De
    // overgang wordt tegen de bezwaarprocedure uit de AWB gevalideerd.
    let status = if toets.herstel_vereist {
        engine::BEZWAAR_STAGE_HERSTEL
    } else {
        engine::BEZWAAR_STAGE_BEHANDELING
    };
    state
        .bezwaar_procedure
        .transitie(engine::BEZWAAR_STAGE_BEZWAARSCHRIFT, status)
        .map_err(internal_error)?;

    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO bezwaren (id, besluit_id, aanvraag_id, kvk_nummer, naam_indiener,
         adres_indiener, gronden, ondertekend, ontvangen_op, toets, status,
         beslistermijn_einddatum)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&besluit_id)
    .bind(&besluit.aanvraag_id)
    .bind(&kvk)
    .bind(body.naam_indiener.trim())
    .bind(body.adres_indiener.as_deref().map(str::trim))
    .bind(body.gronden.as_deref().map(str::trim))
    .bind(body.ondertekend as i64)
    .bind(&vandaag)
    .bind(serde_json::to_string(&toets).map_err(internal_error)?)
    .bind(status)
    .bind(&beslistermijn)
    .execute(&state.pool)
    .await
    .map_err(internal_error)?;

    let bezwaar = bezwaar_by_id(&state.pool, &id)
        .await
        .map_err(internal_error)?
        .ok_or_else(not_found)?;
    Ok(Json(serde_json::to_value(bezwaar).map_err(internal_error)?))
}

#[derive(Debug, Deserialize)]
pub struct Herstel {
    #[serde(default)]
    pub naam_indiener: Option<String>,
    #[serde(default)]
    pub adres_indiener: Option<String>,
    #[serde(default)]
    pub gronden: Option<String>,
    #[serde(default)]
    pub ondertekend: Option<bool>,
}

/// PUT /api/bezwaren/{id}/herstel — het verzuim herstellen binnen de
/// gestelde termijn (AWB 6:6); de AWB toetst opnieuw.
pub async fn herstel_bezwaar(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
    Json(body): Json<Herstel>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let Some(kvk) = session_kvk(&session).await else {
        return Err(forbidden());
    };
    let Some(bezwaar) = bezwaar_by_id(&state.pool, &id)
        .await
        .map_err(internal_error)?
    else {
        return Err(not_found());
    };
    if bezwaar.kvk_nummer != kvk {
        return Err(not_found());
    }
    if bezwaar.status != engine::BEZWAAR_STAGE_HERSTEL {
        return Err(bad_request("Dit bezwaar wacht niet op herstel."));
    }
    let Some(besluit) = db::get_besluit_by_id(&state.pool, &bezwaar.besluit_id)
        .await
        .map_err(internal_error)?
    else {
        return Err(not_found());
    };

    let vandaag = Utc::now().format("%Y-%m-%d").to_string();
    let hersteldeadline = chrono::NaiveDate::parse_from_str(&bezwaar.ontvangen_op, "%Y-%m-%d")
        .map(|d| {
            (d + chrono::Duration::days(HERSTELTERMIJN_DAGEN))
                .format("%Y-%m-%d")
                .to_string()
        })
        .unwrap_or_else(|_| vandaag.clone());
    let binnen_termijn = vandaag <= hersteldeadline;

    let naam = body
        .naam_indiener
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| bezwaar.naam_indiener.clone());
    let adres = body.adres_indiener.or(bezwaar.adres_indiener.clone());
    let gronden = body.gronden.or(bezwaar.gronden.clone());
    let ondertekend = body.ondertekend.unwrap_or(bezwaar.ondertekend);

    let feiten = bezwaar_feiten(
        &naam,
        adres.as_deref(),
        gronden.as_deref(),
        ondertekend,
        &bezwaar.ontvangen_op,
        besluit.bezwaartermijn_einddatum.as_deref(),
        true,
        binnen_termijn,
    );
    let toets = engine::evaluate_bezwaarschrift(state.corpus.clone(), feiten, vandaag.clone())
        .await
        .map_err(internal_error)?;

    // Compleet → behandeling; nog steeds onvolledig → blijft in herstel
    // (de wet bepaalt via 6:6 of niet-ontvankelijkverklaring nu mág; de
    // beslissing zelf blijft aan de beoordelaar).
    let status = if toets.herstel_vereist {
        engine::BEZWAAR_STAGE_HERSTEL
    } else {
        engine::BEZWAAR_STAGE_BEHANDELING
    };
    sqlx::query(
        "UPDATE bezwaren SET naam_indiener = ?, adres_indiener = ?, gronden = ?,
         ondertekend = ?, toets = ?, status = ? WHERE id = ?",
    )
    .bind(&naam)
    .bind(&adres)
    .bind(&gronden)
    .bind(ondertekend as i64)
    .bind(serde_json::to_string(&toets).map_err(internal_error)?)
    .bind(status)
    .bind(&id)
    .execute(&state.pool)
    .await
    .map_err(internal_error)?;

    let bezwaar = bezwaar_by_id(&state.pool, &id)
        .await
        .map_err(internal_error)?
        .ok_or_else(not_found)?;
    Ok(Json(serde_json::to_value(bezwaar).map_err(internal_error)?))
}

// ---------------------------------------------------------------------------
// Beoordelaar: horen en beslissen
// ---------------------------------------------------------------------------

/// GET /api/bezwaren — werkvoorraad bezwaren (beoordelaar).
pub async fn list_bezwaren(
    State(state): State<AppState>,
    session: Session,
) -> Result<Json<serde_json::Value>, ApiError> {
    if session_beoordelaar(&session).await.is_none() {
        return Err(forbidden());
    }
    let bezwaren = list_open_bezwaren(&state.pool)
        .await
        .map_err(internal_error)?;
    Ok(Json(json!(bezwaren)))
}

#[derive(Debug, Deserialize)]
pub struct HoorActie {
    /// true = gehoord; false = afgezien van horen met een 7:3-grond.
    pub gehoord: bool,
    #[serde(default)]
    pub afzien_grond: Option<String>,
}

/// POST /api/bezwaren/{id}/horen — het horen vastleggen, of het afzien
/// daarvan; afzien kan alleen op een grond die AWB 7:3 toestaat.
pub async fn registreer_horen(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
    Json(body): Json<HoorActie>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if session_beoordelaar(&session).await.is_none() {
        return Err(forbidden());
    }
    let Some(bezwaar) = bezwaar_by_id(&state.pool, &id)
        .await
        .map_err(internal_error)?
    else {
        return Err(not_found());
    };
    if bezwaar.status != engine::BEZWAAR_STAGE_BEHANDELING {
        return Err(bad_request("Dit bezwaar is niet in behandeling."));
    }
    if !body.gehoord {
        let grond = body.afzien_grond.as_deref().unwrap_or_default();
        let vandaag = Utc::now().format("%Y-%m-%d").to_string();
        let mag = engine::evaluate_afzien_van_horen(
            state.corpus.clone(),
            grond == "KENNELIJK_NIET_ONTVANKELIJK",
            grond == "KENNELIJK_ONGEGROND",
            grond == "INDIENER_ZIET_AF",
            grond == "VOLLEDIG_TEGEMOETGEKOMEN",
            vandaag,
        )
        .await
        .map_err(internal_error)?;
        if !mag {
            return Err(bad_request(
                "Van het horen kan alleen worden afgezien op een grond uit artikel 7:3 Awb \
                 (kennelijk niet-ontvankelijk, kennelijk ongegrond, indiener ziet af, of \
                 volledig tegemoetgekomen).",
            ));
        }
    }
    sqlx::query("UPDATE bezwaren SET gehoord = ?, afzien_grond = ? WHERE id = ?")
        .bind(body.gehoord as i64)
        .bind(&body.afzien_grond)
        .bind(&id)
        .execute(&state.pool)
        .await
        .map_err(internal_error)?;
    let bezwaar = bezwaar_by_id(&state.pool, &id)
        .await
        .map_err(internal_error)?
        .ok_or_else(not_found)?;
    Ok(Json(serde_json::to_value(bezwaar).map_err(internal_error)?))
}

#[derive(Debug, Deserialize)]
pub struct Beslissing {
    /// NIET_ONTVANKELIJK | ONGEGROND | GEGROND
    pub beslissing: String,
    #[serde(default)]
    pub motivering: Option<String>,
    /// Bij GEGROND: de gecorrigeerde eigen opgaven waarop de wet opnieuw
    /// wordt uitgevoerd (AWB 7:11, volledige heroverweging).
    #[serde(default)]
    pub gecorrigeerde_parameters: Option<serde_json::Map<String, serde_json::Value>>,
}

/// POST /api/bezwaren/{id}/beslissen — de beslissing op bezwaar. Bij
/// GEGROND wordt de Wpp opnieuw uitgevoerd op de gecorrigeerde feiten
/// (AWB 7:11) en vervangt de uitkomst het besluit; het bewijs wordt
/// opnieuw vastgelegd. Horen (of een geldige 7:3-grond) is vereist.
pub async fn beslis_bezwaar(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<String>,
    Json(body): Json<Beslissing>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if session_beoordelaar(&session).await.is_none() {
        return Err(forbidden());
    }
    let Some(bezwaar) = bezwaar_by_id(&state.pool, &id)
        .await
        .map_err(internal_error)?
    else {
        return Err(not_found());
    };
    state
        .bezwaar_procedure
        .transitie(&bezwaar.status, engine::BEZWAAR_STAGE_BESLISSING)
        .map_err(|_| bad_request("Dit bezwaar is al beslist of nog niet in behandeling."))?;
    // Hoorplicht (AWB 7:2): er moet gehoord zijn, of geldig afgezien (7:3).
    if bezwaar.gehoord.is_none() {
        return Err(bad_request(
            "Leg eerst het horen vast, of zie er op een grond uit artikel 7:3 Awb van af.",
        ));
    }

    let beslissing = body.beslissing.trim().to_uppercase();
    if ![
        BESLISSING_NIET_ONTVANKELIJK,
        BESLISSING_ONGEGROND,
        BESLISSING_GEGROND,
    ]
    .contains(&beslissing.as_str())
    {
        return Err(bad_request(
            "Beslissing moet NIET_ONTVANKELIJK, ONGEGROND of GEGROND zijn.",
        ));
    }

    let vandaag = Utc::now().format("%Y-%m-%d").to_string();
    let mut motivering = body.motivering.unwrap_or_default().trim().to_string();

    // AWB 7:11: volledige heroverweging bij gegrond bezwaar — de wet
    // opnieuw uitvoeren op de gecorrigeerde feiten; dezelfde hooks (art. 16
    // betaalopdracht, AWB 3:46/6:7) vuren daarbij opnieuw.
    if beslissing == BESLISSING_GEGROND {
        let Some(aanvraag) = db::get_aanvraag(&state.pool, &bezwaar.aanvraag_id)
            .await
            .map_err(internal_error)?
        else {
            return Err(not_found());
        };
        let serde_json::Value::Object(mut eigen) = aanvraag.parameters.clone() else {
            return Err(internal_error("aanvraagparameters zijn geen object"));
        };
        if let Some(correcties) = body.gecorrigeerde_parameters {
            for (k, v) in correcties {
                eigen.insert(k, v);
            }
        }
        let peildatum = format!("{}-01-01", aanvraag.subsidiejaar);
        let opgaven = db::landelijke_opgaven(&state.pool, aanvraag.subsidiejaar)
            .await
            .map_err(internal_error)?;
        let totaal_leden = engine::ledentotaal(state.corpus.clone(), opgaven, peildatum.clone())
            .await
            .map_err(internal_error)?;
        let uitkomst = engine::evaluate_jaaraanvraag(
            state.corpus.clone(),
            aanvraag.componenten.clone(),
            eigen.clone(),
            peildatum,
            aanvraag.subsidiejaar,
            totaal_leden,
        )
        .await
        .map_err(internal_error)?;

        // De gecorrigeerde feiten worden de feitenbasis van het dossier en
        // het herziene besluit vervangt het oude (zelfde beschikking,
        // heroverwogen) — met nieuw bewijs.
        db::update_aanvraag_parameters(
            &state.pool,
            &bezwaar.aanvraag_id,
            &serde_json::to_string(&eigen).map_err(internal_error)?,
        )
        .await
        .map_err(internal_error)?;
        db::herzie_besluit(
            &state.pool,
            &bezwaar.besluit_id,
            uitkomst.subsidie_toegekend,
            uitkomst.subsidiebedrag,
            &serde_json::to_string(&uitkomst.componenten).map_err(internal_error)?,
            &format!(
                "Heroverwogen na bezwaar (artikel 7:11 Awb). {}",
                uitkomst.motivering
            ),
            &serde_json::to_string(&uitkomst.bewijs).map_err(internal_error)?,
        )
        .await
        .map_err(internal_error)?;
        if motivering.is_empty() {
            motivering = format!(
                "Het bezwaar is gegrond. Na volledige heroverweging (artikel 7:11 Awb) is het \
                 besluit herzien. {}",
                uitkomst.motivering
            );
        }
    } else if motivering.is_empty() {
        motivering = match beslissing.as_str() {
            BESLISSING_NIET_ONTVANKELIJK => {
                "Het bezwaar is niet-ontvankelijk (artikelen 6:5 tot en met 6:9 Awb).".to_string()
            }
            _ => "Het bezwaar is ongegrond; het besluit blijft in stand.".to_string(),
        };
    }

    sqlx::query(
        "UPDATE bezwaren SET status = ?, beslissing = ?, beslissing_motivering = ?,
         beslissing_datum = ? WHERE id = ?",
    )
    .bind(engine::BEZWAAR_STAGE_BESLISSING)
    .bind(&beslissing)
    .bind(&motivering)
    .bind(&vandaag)
    .bind(&id)
    .execute(&state.pool)
    .await
    .map_err(internal_error)?;

    let bezwaar = bezwaar_by_id(&state.pool, &id)
        .await
        .map_err(internal_error)?
        .ok_or_else(not_found)?;
    Ok(Json(serde_json::to_value(bezwaar).map_err(internal_error)?))
}
