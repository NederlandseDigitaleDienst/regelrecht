//! Wrapper around the regelrecht engine.
//!
//! De wet rekent per aanspraak (artikel 6/7 recht, artikel 12/14 bedragen);
//! de orchestratielaag voert de wet per component uit en telt de bedragen
//! op tot één beschikking. Die som is bewust orchestratie en geen wet: de
//! engine kent geen aggregatie over collecties (RFC-012).
//!
//! Het contract tussen wet en uitvoering is fail-loud: elke output waarnaar
//! de orchestratie verwijst wordt bij het opstarten tegen de geladen corpus
//! gecontroleerd (`valideer_contract`), en outputs waarop beslist wordt
//! worden met `req_*` gelezen — ontbreken of taint is een harde fout, nooit
//! een stille default.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use regelrecht_engine::{LawExecutionService, Value};
use serde::Serialize;
use serde_json::json;

use crate::db::{Component, ComponentUitkomst, LandelijkeDelen, OnderdeelFeiten};
use crate::state::LawCorpus;

pub const WPP_ID: &str = "wet_op_de_politieke_partijen";
pub const AWB_ID: &str = "algemene_wet_bestuursrecht";
pub const ATW_ID: &str = "algemene_termijnenwet";
pub const KIESWET_ID: &str = "kieswet";

// Stage-namen waarnaar de orchestratie verwijst. Hun bestaan en volgorde
// worden bij het laden gecontroleerd tegen de proceduredefinitie in de wet
// (RFC-008): hernoemt of herordent de wet de stages, dan faalt de start of
// de transitie — de wet is leidend, niet deze constanten.
pub const STAGE_AANVRAAG: &str = "AANVRAAG";
pub const STAGE_BEHANDELING: &str = "BEHANDELING";
pub const STAGE_BESLUIT: &str = "BESLUIT";
pub const STAGE_BEKENDMAKING: &str = "BEKENDMAKING";
pub const STAGE_BEZWAAR: &str = "BEZWAAR";

// Stages van de bezwaarprocedure (AWB, proceduredefinitie 'bezwaar').
pub const BEZWAAR_STAGE_BEZWAARSCHRIFT: &str = "BEZWAARSCHRIFT";
pub const BEZWAAR_STAGE_HERSTEL: &str = "HERSTEL";
pub const BEZWAAR_STAGE_BEHANDELING: &str = "BEHANDELING";
pub const BEZWAAR_STAGE_BESLISSING: &str = "BESLISSING";

/// Outputs die de orchestratie opvraagt of (via hooks) uitleest, per wet.
/// `valideer_contract` controleert bij het opstarten dat ze allemaal in de
/// geladen corpus bestaan: een hernoemde output in de YAML faalt dan bij de
/// start in plaats van stil een verkeerd besluit op te leveren.
const CONTRACT: &[(&str, &[&str])] = &[
    (
        WPP_ID,
        &[
            "subsidie_toegekend",
            "subsidiebedrag",
            "subsidie_partij",
            "subsidie_wetenschappelijk_instituut",
            "subsidie_jongerenorganisatie",
            "subsidie_buitenland",
            "voldoet_aan_transparantie",
            "voldoet_verbod_anonieme_giften",
            "voldoet_verbod_giften_niet_ingezetenen",
            "voldoet_meldplicht_giften",
            "voldoet_openbaarmaking_financien",
            "heeft_recht_landelijk",
            "voldoet_zeteleis_landelijk",
            "voldoet_ledeneis",
            "voldoet_zeteleis_decentraal",
            "onderdeel_beschikbaar",
            "aanvraagtermijn_einddatum",
            "beslistermijn_einddatum",
            "voorschotpercentage",
            "rekening_aanvaardbaar",
            "mag_rekening_wijzigen",
            "uitbetaling_aangehouden",
            "betaalopdracht_vereist",
            "betaalopdracht_bedrag",
        ],
    ),
    (
        AWB_ID,
        &[
            "motivering_vereist",
            "bezwaartermijn_weken",
            "bezwaartermijn_startdatum",
            "bezwaartermijn_einddatum",
            "betaaltermijn_einddatum",
            "voldoet_aan_bezwaarschriftvereisten",
            "ontbreekt_naam_en_adres",
            "ontbreekt_dagtekening",
            "ontbreekt_besluitomschrijving",
            "ontbreken_gronden",
            "ontbreekt_ondertekening",
            "herstel_vereist",
            "mag_niet_ontvankelijk_wegens_verzuim",
            "bezwaar_tijdig",
            "mag_afzien_van_horen",
            "beslistermijn_bezwaar_einddatum",
        ],
    ),
    (ATW_ID, &["verlengde_einddatum"]),
    (
        KIESWET_ID,
        &[
            "voldoet_aan_registratie_eisen",
            "voldoet_eis_inschrijving",
            "voldoet_eis_rechtsvorm",
            "voldoet_eis_naam",
        ],
    ),
];

/// Controleer het contract tussen wet en uitvoering tegen de geladen
/// corpus. Aanroepen bij het opstarten: bestaat een output niet (meer),
/// dan weigert de applicatie te starten.
pub fn valideer_contract(corpus: &Arc<LawCorpus>) -> anyhow::Result<()> {
    valideer_contract_met(corpus, CONTRACT)
}

fn valideer_contract_met(
    corpus: &Arc<LawCorpus>,
    contract: &[(&str, &[&str])],
) -> anyhow::Result<()> {
    with_service(corpus, |service| {
        for (law_id, outputs) in contract {
            for output in *outputs {
                if service
                    .resolver()
                    .get_article_by_output(law_id, output, None)
                    .is_none()
                {
                    anyhow::bail!(
                        "contractbreuk wet↔uitvoering: output '{output}' bestaat niet (meer) \
                         in {law_id}"
                    );
                }
            }
        }
        Ok(())
    })
}

/// De beschikking-procedure zoals de wet die definieert: de geordende
/// lifecycle-stages. De orchestratie bewaart per aanvraag de huidige stage
/// (RFC-008: engine stateless, orchestratie persisteert) en valideert elke
/// overgang tegen deze volgorde.
#[derive(Debug, Clone)]
pub struct Procedure {
    stages: Vec<String>,
}

impl Procedure {
    fn index(&self, stage: &str) -> anyhow::Result<usize> {
        self.stages
            .iter()
            .position(|s| s == stage)
            .ok_or_else(|| anyhow::anyhow!("stage '{stage}' bestaat niet in de procedure"))
    }

    /// De stage waarin een zojuist ingediende aanvraag belandt: de stage
    /// volgend op de indieningsstage (de eerste van de procedure).
    pub fn na_indiening(&self) -> anyhow::Result<&str> {
        self.stages
            .get(1)
            .map(String::as_str)
            .ok_or_else(|| anyhow::anyhow!("de procedure heeft geen stage na de indiening"))
    }

    /// Valideer een statusovergang tegen de volgorde van de wet: de
    /// doelstage moet na de huidige stage komen. Tussenliggende momentane
    /// stages (zoals BEKENDMAKING) mogen in één overgang worden doorlopen.
    pub fn transitie(&self, van: &str, naar: &str) -> anyhow::Result<()> {
        if self.index(naar)? <= self.index(van)? {
            anyhow::bail!("overgang van stage '{van}' naar '{naar}' is in strijd met de procedure");
        }
        Ok(())
    }
}

/// Lees de beschikking-procedure uit de wet. De stages waarnaar de
/// orchestratie verwijst moeten bestaan; ontbreekt er een, dan weigert de
/// applicatie te starten.
pub fn beschikking_procedure(wpp_yaml: &str) -> anyhow::Result<Procedure> {
    let mut service = LawExecutionService::new();
    service
        .load_law(wpp_yaml)
        .map_err(|e| anyhow::anyhow!("laden Wpp voor procedure mislukt: {e}"))?;
    let definitie = service
        .resolver()
        .find_procedure("BESCHIKKING", None)
        .ok_or_else(|| anyhow::anyhow!("de wet definieert geen BESCHIKKING-procedure"))?;
    let procedure = Procedure {
        stages: definitie.stages.iter().map(|s| s.name.clone()).collect(),
    };
    for stage in [
        STAGE_AANVRAAG,
        STAGE_BEHANDELING,
        STAGE_BESLUIT,
        STAGE_BEKENDMAKING,
        STAGE_BEZWAAR,
    ] {
        procedure.index(stage)?;
    }
    Ok(procedure)
}

/// Lees de bezwaarprocedure uit de AWB (proceduredefinitie 'bezwaar').
pub fn bezwaar_procedure(awb_yaml: &str) -> anyhow::Result<Procedure> {
    let mut service = LawExecutionService::new();
    service
        .load_law(awb_yaml)
        .map_err(|e| anyhow::anyhow!("laden AWB voor bezwaarprocedure mislukt: {e}"))?;
    let definitie = service
        .resolver()
        .find_procedure("BESCHIKKING", Some("bezwaar"))
        .ok_or_else(|| anyhow::anyhow!("de AWB definieert geen bezwaarprocedure"))?;
    let procedure = Procedure {
        stages: definitie.stages.iter().map(|s| s.name.clone()).collect(),
    };
    for stage in [
        BEZWAAR_STAGE_BEZWAARSCHRIFT,
        BEZWAAR_STAGE_HERSTEL,
        BEZWAAR_STAGE_BEHANDELING,
        BEZWAAR_STAGE_BESLISSING,
    ] {
        procedure.index(stage)?;
    }
    Ok(procedure)
}

/// Uitkomst van een samengestelde jaaraanvraag.
#[derive(Debug, Clone, Serialize)]
pub struct JaaruitkomstUitkomst {
    pub subsidie_toegekend: bool,
    pub subsidiebedrag: i64,
    pub betaalopdracht_vereist: bool,
    pub betaalopdracht_bedrag: i64,
    pub bezwaartermijn_weken: i64,
    pub motivering_vereist: bool,
    pub voldoet_aan_transparantie: bool,
    pub componenten: Vec<ComponentUitkomst>,
    pub motivering: String,
    /// Het bewijs van dit besluit: peildatum, wetversies en per component
    /// de parameters en outputs van de evaluatie. Wordt bij het besluit
    /// gepersisteerd zodat het later herleidbaar is.
    pub bewijs: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct BezwaartermijnUitkomst {
    pub startdatum: String,
    pub einddatum: String,
}

fn build_service(corpus: &LawCorpus) -> anyhow::Result<LawExecutionService> {
    let mut service = LawExecutionService::new();
    service
        .load_law(&corpus.wpp)
        .map_err(|e| anyhow::anyhow!("laden Wpp mislukt: {e}"))?;
    service
        .load_law(&corpus.regeling)
        .map_err(|e| anyhow::anyhow!("laden regeling mislukt: {e}"))?;
    service
        .load_law(&corpus.besluit_decentraal)
        .map_err(|e| anyhow::anyhow!("laden besluit decentrale subsidies mislukt: {e}"))?;
    service
        .load_law(&corpus.awb)
        .map_err(|e| anyhow::anyhow!("laden AWB mislukt: {e}"))?;
    service
        .load_law(&corpus.termijnenwet)
        .map_err(|e| anyhow::anyhow!("laden Algemene termijnenwet mislukt: {e}"))?;
    service
        .load_law(&corpus.kieswet)
        .map_err(|e| anyhow::anyhow!("laden Kieswet mislukt: {e}"))?;
    Ok(service)
}

thread_local! {
    /// Eén geladen service per (blocking-)thread per corpus: de YAML's
    /// worden niet bij elke evaluatie opnieuw geparsed. De sleutel is het
    /// Arc-adres van de corpus; een nieuw geladen corpus krijgt een nieuw
    /// adres en dus een verse service.
    static SERVICE: RefCell<Option<(usize, LawExecutionService)>> = const { RefCell::new(None) };
}

fn with_service<R, F>(corpus: &Arc<LawCorpus>, f: F) -> anyhow::Result<R>
where
    F: FnOnce(&LawExecutionService) -> anyhow::Result<R>,
{
    let key = Arc::as_ptr(corpus) as usize;
    SERVICE.with(|cell| {
        let mut slot = cell.borrow_mut();
        let geldig = matches!(&*slot, Some((k, _)) if *k == key);
        if !geldig {
            *slot = Some((key, build_service(corpus)?));
        }
        // Twee regels hierboven gezet; de Option is hier per constructie Some.
        #[allow(clippy::expect_used)]
        let (_, service) = slot.as_ref().expect("service zojuist gezet");
        f(service)
    })
}

/// Lees een output waarop een besluit wordt gebouwd. Ontbreekt hij of is
/// hij geen boolean (bijvoorbeeld een Untranslatable-taint), dan is dat een
/// contractbreuk tussen wet en uitvoering: hard falen, nooit stil een
/// default kiezen.
fn req_bool(outputs: &BTreeMap<String, Value>, name: &str) -> anyhow::Result<bool> {
    match outputs.get(name) {
        Some(Value::Bool(b)) => Ok(*b),
        Some(other) => anyhow::bail!("output '{name}' is geen boolean maar {other:?}"),
        None => anyhow::bail!("output '{name}' ontbreekt in het engine-resultaat"),
    }
}

/// Rond een niet-geheel engine-getal af op een heel getal.
///
/// De engine kende ooit `Value::Float`; dat is `Value::Decimal` geworden, en
/// dat is geen naamswijziging maar de reden dat dit hier klopt: deze functies
/// lezen bedragen in centen, en een binaire float kan een bedrag als 0,10 niet
/// exact voorstellen. `round()` op een Decimal rondt halve centen weg van nul,
/// zoals bij geld hoort.
fn decimal_naar_i64(d: rust_decimal::Decimal) -> i64 {
    use rust_decimal::prelude::ToPrimitive;
    d.round().to_i64().unwrap_or(0)
}

fn req_int(outputs: &BTreeMap<String, Value>, name: &str) -> anyhow::Result<i64> {
    match outputs.get(name) {
        Some(Value::Int(n)) => Ok(*n),
        Some(Value::Decimal(d)) => Ok(decimal_naar_i64(*d)),
        Some(other) => anyhow::bail!("output '{name}' is geen getal maar {other:?}"),
        None => anyhow::bail!("output '{name}' ontbreekt in het engine-resultaat"),
    }
}

fn req_date(outputs: &BTreeMap<String, Value>, name: &str) -> anyhow::Result<String> {
    match outputs.get(name) {
        Some(Value::String(s)) => Ok(s.clone()),
        Some(other) => anyhow::bail!("output '{name}' is geen datum maar {other:?}"),
        None => anyhow::bail!("output '{name}' ontbreekt in het engine-resultaat"),
    }
}

/// Lenient lezen, uitsluitend voor hook-outputs die er legitiem niet zijn
/// (de betaalopdracht-hook van art. 16 vuurt alleen bij toekenning; de
/// AWB-hooks alleen op hun stage). Voor opgevraagde outputs geldt `req_*`.
fn hook_bool(outputs: &BTreeMap<String, Value>, name: &str) -> bool {
    matches!(outputs.get(name), Some(Value::Bool(true)))
}

fn hook_int(outputs: &BTreeMap<String, Value>, name: &str) -> i64 {
    match outputs.get(name) {
        Some(Value::Int(n)) => *n,
        Some(Value::Decimal(d)) => decimal_naar_i64(*d),
        _ => 0,
    }
}

fn euro(cents: i64) -> String {
    let negative = cents < 0;
    let cents = cents.abs();
    let whole = cents / 100;
    let rest = cents % 100;
    let mut s = whole.to_string();
    let mut grouped = String::new();
    while s.len() > 3 {
        let tail = s.split_off(s.len() - 3);
        grouped = format!(".{tail}{grouped}");
    }
    let sign = if negative { "-" } else { "" };
    format!("{sign}€ {s}{grouped},{rest:02}")
}

/// Engine-parameters voor één component, gecombineerd met de eigen opgaven
/// (ledental, neveninstellingen, transparantieverklaringen) die op
/// rechtspersoonsniveau gelden, plus het door de orchestratie aangeleverde
/// ledentotaal (de noemer van de ledencomponent van art. 14).
fn component_params(
    component: &Component,
    eigen: &serde_json::Map<String, serde_json::Value>,
    totaal_leden: i64,
) -> BTreeMap<String, Value> {
    let mut params: BTreeMap<String, Value> = BTreeMap::new();
    for key in [
        "ontvangt_anonieme_giften",
        "ontvangt_giften_niet_ingezetenen",
        "voldoet_aan_meldplicht_giften",
        "financien_openbaar_op_website",
    ] {
        let waarde = eigen.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
        params.insert(key.to_string(), Value::Bool(waarde));
    }
    let leden = eigen
        .get("aantal_betalende_leden")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let eigen_bool = |key: &str| eigen.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
    let pjo_leden = eigen
        .get("aantal_leden_jongerenorganisatie")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    if component.soort == "LANDELIJK" {
        params.insert("niveau".into(), Value::String("LANDELIJK".into()));
        params.insert("orgaan".into(), Value::String("GEMEENTERAAD".into()));
        params.insert("aantal_kamerzetels".into(), Value::Int(component.zetels));
        params.insert("aantal_betalende_leden".into(), Value::Int(leden));
        params.insert(
            "totaal_aantal_betalende_leden".into(),
            Value::Int(totaal_leden),
        );
        params.insert(
            "heeft_wetenschappelijk_instituut".into(),
            Value::Bool(eigen_bool("heeft_wetenschappelijk_instituut")),
        );
        params.insert(
            "heeft_jongerenorganisatie".into(),
            Value::Bool(eigen_bool("heeft_jongerenorganisatie")),
        );
        params.insert(
            "aantal_leden_jongerenorganisatie".into(),
            Value::Int(pjo_leden),
        );
        params.insert(
            "heeft_instelling_buitenland".into(),
            Value::Bool(eigen_bool("heeft_instelling_buitenland")),
        );
        params.insert("aantal_raadszetels".into(), Value::Int(0));
        params.insert("inwoneraantal_gemeente".into(), Value::Int(0));
    } else {
        params.insert("niveau".into(), Value::String("DECENTRAAL".into()));
        params.insert(
            "orgaan".into(),
            Value::String(
                component
                    .orgaan
                    .clone()
                    .unwrap_or_else(|| "GEMEENTERAAD".into()),
            ),
        );
        params.insert("aantal_raadszetels".into(), Value::Int(component.zetels));
        params.insert(
            "inwoneraantal_gemeente".into(),
            Value::Int(component.inwoneraantal),
        );
        params.insert("aantal_kamerzetels".into(), Value::Int(0));
        params.insert("aantal_betalende_leden".into(), Value::Int(0));
        params.insert("totaal_aantal_betalende_leden".into(), Value::Int(0));
        params.insert(
            "heeft_wetenschappelijk_instituut".into(),
            Value::Bool(false),
        );
        params.insert("heeft_jongerenorganisatie".into(), Value::Bool(false));
        params.insert("aantal_leden_jongerenorganisatie".into(), Value::Int(0));
        params.insert("heeft_instelling_buitenland".into(), Value::Bool(false));
    }
    params
}

fn orgaan_label(component: &Component) -> String {
    match component.soort.as_str() {
        "LANDELIJK" => "landelijk".to_string(),
        _ => match component.orgaan.as_deref() {
            Some("PROVINCIALE_STATEN") => {
                format!(
                    "provinciale staten {}",
                    component.gebied.as_deref().unwrap_or("?")
                )
            }
            Some("WATERSCHAP") => {
                format!("waterschap {}", component.gebied.as_deref().unwrap_or("?"))
            }
            Some("EILANDSRAAD") => {
                format!("eilandsraad {}", component.gebied.as_deref().unwrap_or("?"))
            }
            _ => format!(
                "gemeenteraad {}",
                component.gebied.as_deref().unwrap_or("?")
            ),
        },
    }
}

/// De transparantietoets van art. 5, per voorwaarde zoals de wet die
/// teruggeeft. De motivering benoemt geschonden eisen op basis van deze
/// outputs; de voorwaarden zelf staan uitsluitend in de YAML.
struct TransparantieToets {
    voldoet: bool,
    verbod_anonieme_giften: bool,
    verbod_giften_niet_ingezetenen: bool,
    meldplicht_giften: bool,
    openbaarmaking_financien: bool,
}

/// De motiveringszin voor een afgewezen onderdeel, uit de per-voorwaarde
/// outputs van dezelfde evaluatie als het besluit zelf — geen tweede
/// evaluatie, zodat motivering en bewijs per definitie bij elkaar horen.
/// De drempels (één zetel, duizend leden) staan alleen in de wet; hier
/// wordt uitsluitend geformuleerd.
fn afwijzingsgrond(
    component: &Component,
    outputs: &BTreeMap<String, Value>,
) -> anyhow::Result<String> {
    if component.soort == "LANDELIJK" {
        let reden = if !req_bool(outputs, "voldoet_zeteleis_landelijk")? {
            "de partij geen zetel in de Eerste of Tweede Kamer heeft"
        } else if !req_bool(outputs, "voldoet_ledeneis")? {
            "de partij minder dan duizend betalende leden heeft"
        } else {
            "niet aan de voorwaarden van artikel 6 is voldaan"
        };
        Ok(format!(
            "Het landelijke onderdeel wordt afgewezen omdat {reden} (artikel 6)."
        ))
    } else {
        let reden = if !req_bool(outputs, "voldoet_zeteleis_decentraal")? {
            "daar geen zetel is toegewezen"
        } else {
            "niet aan de voorwaarden van artikel 7 is voldaan"
        };
        Ok(format!(
            "Het onderdeel {} wordt afgewezen omdat {reden} (artikel 7).",
            orgaan_label(component)
        ))
    }
}

fn build_motivering(
    transparantie: &TransparantieToets,
    uitkomsten: &[ComponentUitkomst],
    totaal: i64,
    subsidiejaar: i64,
    voorschot: i64,
) -> String {
    let toegekend: Vec<_> = uitkomsten.iter().filter(|u| u.toegekend).collect();
    let afgewezen: Vec<_> = uitkomsten.iter().filter(|u| !u.toegekend).collect();

    let mut delen: Vec<String> = Vec::new();

    if !transparantie.voldoet {
        let mut redenen = Vec::new();
        if !transparantie.verbod_anonieme_giften {
            redenen.push("de partij ontvangt anonieme giften");
        }
        if !transparantie.verbod_giften_niet_ingezetenen {
            redenen.push("de partij ontvangt giften van niet-ingezetenen");
        }
        if !transparantie.meldplicht_giften {
            redenen
                .push("de partij voldoet niet aan de meldplicht voor giften van € 10.000 of meer");
        }
        if !transparantie.openbaarmaking_financien {
            redenen.push("de partij maakt haar financiën niet openbaar op haar website");
        }
        delen.push(format!(
            "De aanvraag wordt afgewezen omdat niet aan de transparantie-eisen van artikel 5 \
             van de Wet op de politieke partijen is voldaan: {}.",
            redenen.join("; ")
        ));
        return delen.join(" ");
    }

    delen.push(
        "De aanvraag voldoet aan de transparantie-eisen van artikel 5 van de Wet op de \
         politieke partijen."
            .to_string(),
    );

    if !toegekend.is_empty() {
        let n = toegekend.len();
        delen.push(format!(
            "Op grond van de artikelen 6, 7, 12 en 14 wordt voor subsidiejaar {subsidiejaar} \
             een subsidie verleend van {} voor {n} {}, overeenkomstig de specificatie bij \
             dit besluit. Op grond van artikel 17, derde lid, wordt van rechtswege een \
             voorschot verleend van tachtig procent: {}.",
            euro(totaal),
            if n == 1 { "onderdeel" } else { "onderdelen" },
            euro(voorschot)
        ));
    }
    for u in &afgewezen {
        if let Some(grond) = &u.afwijzingsgrond {
            delen.push(grond.clone());
        }
    }
    if toegekend.is_empty() && afgewezen.is_empty() {
        delen.push("De aanvraag bevat geen onderdelen.".to_string());
    }
    delen.join(" ")
}

/// Voer de wet uit voor elke component van een jaaraanvraag en stel het
/// samengestelde verleningsbesluit op. De rekendatum is de peildatum van
/// het subsidiejaar (art. 17: 1 januari); daarmee selecteert de engine ook
/// de wetversies die voor dat jaar gelden. `totaal_leden` is de noemer van
/// de ledencomponent (art. 14, onderdeel a) — aggregatie over aanvragen is
/// bewust orchestratie en geen wet (RFC-012).
pub async fn evaluate_jaaraanvraag(
    corpus: Arc<LawCorpus>,
    componenten: Vec<Component>,
    eigen: serde_json::Map<String, serde_json::Value>,
    peildatum: String,
    subsidiejaar: i64,
    totaal_leden: i64,
) -> anyhow::Result<JaaruitkomstUitkomst> {
    tokio::task::spawn_blocking(move || {
        with_service(&corpus, |service| {
            let mut uitkomsten: Vec<ComponentUitkomst> = Vec::new();
            let mut bewijs_componenten: Vec<serde_json::Value> = Vec::new();
            let mut totaal: i64 = 0;
            let mut betaal_totaal: i64 = 0;
            let mut bezwaartermijn_weken = 0;
            let mut motivering_vereist = false;

            for component in &componenten {
                let params = component_params(component, &eigen, totaal_leden);
                // De eisen-outputs (art. 6/7) worden in dezelfde evaluatie
                // opgevraagd als het besluit: motivering en bewijs komen zo
                // per definitie uit dezelfde berekening.
                let gevraagd: &[&str] = if component.soort == "LANDELIJK" {
                    &[
                        "subsidie_toegekend",
                        "subsidiebedrag",
                        "voldoet_zeteleis_landelijk",
                        "voldoet_ledeneis",
                    ]
                } else {
                    &[
                        "subsidie_toegekend",
                        "subsidiebedrag",
                        "voldoet_zeteleis_decentraal",
                    ]
                };
                let result = service
                    .evaluate_law(WPP_ID, gevraagd, params.clone(), &peildatum)
                    .map_err(|e| {
                        anyhow::anyhow!("berekening onderdeel {} mislukt: {e}", component.key)
                    })?;

                let toegekend = req_bool(&result.outputs, "subsidie_toegekend")?;
                let bedrag = req_int(&result.outputs, "subsidiebedrag")?;
                totaal += bedrag;
                // Hook-outputs: art. 16 vuurt alleen bij toekenning, de
                // AWB-hooks op hun stage — afwezigheid is hier legitiem.
                betaal_totaal += hook_int(&result.outputs, "betaalopdracht_bedrag");
                bezwaartermijn_weken = hook_int(&result.outputs, "bezwaartermijn_weken");
                motivering_vereist |= hook_bool(&result.outputs, "motivering_vereist");

                // De specificatie van de landelijke component: de vier delen
                // van art. 14 (partij + geoormerkte bedragen).
                let mut delen_outputs: Option<serde_json::Value> = None;
                let delen = if component.soort == "LANDELIJK" && toegekend {
                    let delen_result = service
                        .evaluate_law(
                            WPP_ID,
                            &[
                                "subsidie_partij",
                                "subsidie_wetenschappelijk_instituut",
                                "subsidie_jongerenorganisatie",
                                "subsidie_buitenland",
                            ],
                            params.clone(),
                            &peildatum,
                        )
                        .map_err(|e| anyhow::anyhow!("specificatie art. 14 mislukt: {e}"))?;
                    delen_outputs = serde_json::to_value(&delen_result.outputs).ok();
                    Some(LandelijkeDelen {
                        partij: req_int(&delen_result.outputs, "subsidie_partij")?,
                        wetenschappelijk_instituut: req_int(
                            &delen_result.outputs,
                            "subsidie_wetenschappelijk_instituut",
                        )?,
                        jongerenorganisatie: req_int(
                            &delen_result.outputs,
                            "subsidie_jongerenorganisatie",
                        )?,
                        buitenland: req_int(&delen_result.outputs, "subsidie_buitenland")?,
                    })
                } else {
                    None
                };

                // Bij afwijzing levert dezelfde evaluatie de gefaalde
                // voorwaarde; hier wordt alleen de zin samengesteld.
                let grond = if toegekend {
                    None
                } else {
                    Some(afwijzingsgrond(component, &result.outputs)?)
                };

                let mut bewijs_component = json!({
                    "key": component.key,
                    "parameters": serde_json::to_value(&params).unwrap_or_default(),
                    "outputs": serde_json::to_value(&result.outputs).unwrap_or_default(),
                });
                if let Some(d) = delen_outputs {
                    bewijs_component["delen_outputs"] = d;
                }
                bewijs_componenten.push(bewijs_component);

                uitkomsten.push(ComponentUitkomst {
                    component: component.clone(),
                    toegekend,
                    bedrag,
                    delen,
                    afwijzingsgrond: grond,
                });
            }

            // Transparantie geldt op rechtspersoonsniveau; één toets volstaat.
            let (transparantie, transparantie_outputs) = match componenten.first() {
                Some(component) => {
                    let result = service
                        .evaluate_law(
                            WPP_ID,
                            &[
                                "voldoet_aan_transparantie",
                                "voldoet_verbod_anonieme_giften",
                                "voldoet_verbod_giften_niet_ingezetenen",
                                "voldoet_meldplicht_giften",
                                "voldoet_openbaarmaking_financien",
                            ],
                            component_params(component, &eigen, totaal_leden),
                            &peildatum,
                        )
                        .map_err(|e| anyhow::anyhow!("toetsing transparantie mislukt: {e}"))?;
                    let toets = TransparantieToets {
                        voldoet: req_bool(&result.outputs, "voldoet_aan_transparantie")?,
                        verbod_anonieme_giften: req_bool(
                            &result.outputs,
                            "voldoet_verbod_anonieme_giften",
                        )?,
                        verbod_giften_niet_ingezetenen: req_bool(
                            &result.outputs,
                            "voldoet_verbod_giften_niet_ingezetenen",
                        )?,
                        meldplicht_giften: req_bool(&result.outputs, "voldoet_meldplicht_giften")?,
                        openbaarmaking_financien: req_bool(
                            &result.outputs,
                            "voldoet_openbaarmaking_financien",
                        )?,
                    };
                    let outputs = serde_json::to_value(&result.outputs).unwrap_or_default();
                    (toets, outputs)
                }
                None => (
                    TransparantieToets {
                        voldoet: true,
                        verbod_anonieme_giften: true,
                        verbod_giften_niet_ingezetenen: true,
                        meldplicht_giften: true,
                        openbaarmaking_financien: true,
                    },
                    serde_json::Value::Null,
                ),
            };

            let toegekend = uitkomsten.iter().any(|u| u.toegekend);
            let motivering = build_motivering(
                &transparantie,
                &uitkomsten,
                totaal,
                subsidiejaar,
                betaal_totaal,
            );

            // Het bewijs: waarop dit besluit is gebaseerd, herleidbaar
            // vastgelegd (peildatum, wetversies, feiten en oordelen).
            let bewijs = json!({
                "peildatum": peildatum,
                "wetten": corpus.versies,
                "totaal_aantal_betalende_leden": totaal_leden,
                "componenten": bewijs_componenten,
                "transparantie_outputs": transparantie_outputs,
            });

            Ok(JaaruitkomstUitkomst {
                subsidie_toegekend: toegekend,
                subsidiebedrag: totaal,
                betaalopdracht_vereist: betaal_totaal > 0,
                betaalopdracht_bedrag: betaal_totaal,
                bezwaartermijn_weken,
                motivering_vereist,
                voldoet_aan_transparantie: transparantie.voldoet,
                componenten: uitkomsten,
                motivering,
                bewijs,
            })
        })
    })
    .await?
}

/// De noemer van de ledencomponent (art. 14): de som van de opgegeven
/// ledentallen van de partijen waaraan subsidie wordt verstrekt. Of een
/// opgave meetelt bepaalt de wet (art. 6, via heeft_recht_landelijk); de
/// som zelf is orchestratie (RFC-012). De transparantieverklaringen en het
/// ledental komen uit de opgeslagen eigen opgaven van elke aanvraag.
pub async fn ledentotaal(
    corpus: Arc<LawCorpus>,
    opgaven: Vec<crate::db::LandelijkeOpgave>,
    peildatum: String,
) -> anyhow::Result<i64> {
    tokio::task::spawn_blocking(move || {
        with_service(&corpus, |service| {
            let mut totaal: i64 = 0;
            for opgave in &opgaven {
                let leden = opgave
                    .parameters
                    .get("aantal_betalende_leden")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let mut params: BTreeMap<String, Value> = BTreeMap::new();
                params.insert("aantal_kamerzetels".into(), Value::Int(opgave.zetels));
                params.insert("aantal_betalende_leden".into(), Value::Int(leden));
                for key in [
                    "ontvangt_anonieme_giften",
                    "ontvangt_giften_niet_ingezetenen",
                    "voldoet_aan_meldplicht_giften",
                    "financien_openbaar_op_website",
                ] {
                    let waarde = opgave
                        .parameters
                        .get(key)
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    params.insert(key.to_string(), Value::Bool(waarde));
                }
                let result = service
                    .evaluate_law(WPP_ID, &["heeft_recht_landelijk"], params, &peildatum)
                    .map_err(|e| anyhow::anyhow!("toetsing ledenbudget-opgave mislukt: {e}"))?;
                if req_bool(&result.outputs, "heeft_recht_landelijk")? {
                    totaal += leden;
                }
            }
            Ok(totaal)
        })
    })
    .await?
}

/// Termijnen van de jaarcyclus (Wpp art. 17, lex specialis t.o.v. AWB 4:13):
/// de aanvraag moet uiterlijk 1 november voorafgaand aan het subsidiejaar
/// binnen zijn en de Napp besluit voor 1 januari van het subsidiejaar. De
/// beslistermijn is een uiterste datum gekoppeld aan de start van het
/// subsidiejaar; de Algemene termijnenwet wordt hier bewust niet toegepast,
/// omdat verlenging voorbij 1 januari het doel van de termijn (besluit vóór
/// aanvang van het subsidiejaar) zou tenietdoen.
#[derive(Debug, Clone, Serialize)]
pub struct TermijnenUitkomst {
    pub aanvraagtermijn_einddatum: String,
    pub beslistermijn_einddatum: String,
    pub voorschotpercentage: i64,
}

pub async fn evaluate_termijnen(
    corpus: Arc<LawCorpus>,
    subsidiejaar: i64,
    date: String,
) -> anyhow::Result<TermijnenUitkomst> {
    tokio::task::spawn_blocking(move || {
        with_service(&corpus, |service| {
            let mut params = BTreeMap::new();
            params.insert(
                "subsidiejaar_startdatum".to_string(),
                Value::String(format!("{subsidiejaar}-01-01")),
            );
            let result = service
                .evaluate_law(
                    WPP_ID,
                    &[
                        "aanvraagtermijn_einddatum",
                        "beslistermijn_einddatum",
                        "voorschotpercentage",
                    ],
                    params,
                    &date,
                )
                .map_err(|e| anyhow::anyhow!("termijnen art. 17 berekenen mislukt: {e}"))?;
            Ok(TermijnenUitkomst {
                aanvraagtermijn_einddatum: req_date(&result.outputs, "aanvraagtermijn_einddatum")?,
                beslistermijn_einddatum: req_date(&result.outputs, "beslistermijn_einddatum")?,
                voorschotpercentage: req_int(&result.outputs, "voorschotpercentage")?,
            })
        })
    })
    .await?
}

/// Het oordeel van art. 27 over de rekening van de rechtspersoon. De
/// feiten (naam-controle, machtiging, bekendheid) komen uit de
/// orchestratie; de regels staan in de wet.
#[derive(Debug, Clone, Serialize)]
pub struct RekeningToets {
    pub rekening_aanvaardbaar: bool,
    pub mag_rekening_wijzigen: bool,
    pub uitbetaling_aangehouden: bool,
}

pub async fn evaluate_rekening(
    corpus: Arc<LawCorpus>,
    rekening_op_naam_van_rechtspersoon: bool,
    eherkenning_volledige_machtiging: bool,
    rekening_bekend: bool,
    date: String,
) -> anyhow::Result<RekeningToets> {
    tokio::task::spawn_blocking(move || {
        with_service(&corpus, |service| {
            let mut params: BTreeMap<String, Value> = BTreeMap::new();
            params.insert(
                "rekening_op_naam_van_rechtspersoon".into(),
                Value::Bool(rekening_op_naam_van_rechtspersoon),
            );
            params.insert(
                "eherkenning_volledige_machtiging".into(),
                Value::Bool(eherkenning_volledige_machtiging),
            );
            params.insert("rekening_bekend".into(), Value::Bool(rekening_bekend));
            let result = service
                .evaluate_law(
                    WPP_ID,
                    &[
                        "rekening_aanvaardbaar",
                        "mag_rekening_wijzigen",
                        "uitbetaling_aangehouden",
                    ],
                    params,
                    &date,
                )
                .map_err(|e| anyhow::anyhow!("toetsing artikel 27 mislukt: {e}"))?;
            Ok(RekeningToets {
                rekening_aanvaardbaar: req_bool(&result.outputs, "rekening_aanvaardbaar")?,
                mag_rekening_wijzigen: req_bool(&result.outputs, "mag_rekening_wijzigen")?,
                uitbetaling_aangehouden: req_bool(&result.outputs, "uitbetaling_aangehouden")?,
            })
        })
    })
    .await?
}

/// Het oordeel van art. 13 (eenmalige verstrekking per subsidiejaar) per
/// onderdeel. De orchestratie levert per onderdeel de rauwe feiten uit de
/// aanvragentabel (loopt er een aanvraag, is er toegekend, is er
/// afgewezen); de wet beslist wat blokkeert — en dus ook dat een eerdere
/// afwijzing níét blokkeert.
pub async fn beschikbare_onderdelen(
    corpus: Arc<LawCorpus>,
    feiten: Vec<OnderdeelFeiten>,
    date: String,
) -> anyhow::Result<Vec<bool>> {
    tokio::task::spawn_blocking(move || {
        with_service(&corpus, |service| {
            let mut beschikbaar = Vec::with_capacity(feiten.len());
            for feit in &feiten {
                let mut params: BTreeMap<String, Value> = BTreeMap::new();
                params.insert(
                    "onderdeel_in_behandeling".into(),
                    Value::Bool(feit.in_behandeling),
                );
                params.insert(
                    "onderdeel_eerder_toegekend".into(),
                    Value::Bool(feit.eerder_toegekend),
                );
                params.insert(
                    "onderdeel_eerder_afgewezen".into(),
                    Value::Bool(feit.eerder_afgewezen),
                );
                let result = service
                    .evaluate_law(WPP_ID, &["onderdeel_beschikbaar"], params, &date)
                    .map_err(|e| anyhow::anyhow!("toetsing artikel 13 mislukt: {e}"))?;
                beschikbaar.push(req_bool(&result.outputs, "onderdeel_beschikbaar")?);
            }
            Ok(beschikbaar)
        })
    })
    .await?
}

/// Het oordeel van Kieswet G 1 over een Handelsregister-raadpleging, per
/// eis zoals de wet die teruggeeft. Dit is een advies aan de beoordelaar;
/// de bevestiging van een claim blijft een menselijke beslissing.
#[derive(Debug, Clone, Serialize)]
pub struct RegistratieToets {
    pub voldoet_aan_registratie_eisen: bool,
    pub voldoet_eis_inschrijving: bool,
    pub voldoet_eis_rechtsvorm: bool,
    pub voldoet_eis_naam: bool,
}

/// Toets een Handelsregister-raadpleging aan de registratie-eisen van
/// Kieswet G 1. De raadpleging zelf is een databron (orchestratie); welke
/// eisen gelden staat uitsluitend in de wet.
pub async fn evaluate_registratie_eisen(
    corpus: Arc<LawCorpus>,
    toets: crate::handelsregister::HrToets,
    date: String,
) -> anyhow::Result<RegistratieToets> {
    tokio::task::spawn_blocking(move || {
        with_service(&corpus, |service| {
            let mut params: BTreeMap<String, Value> = BTreeMap::new();
            params.insert(
                "ingeschreven_in_handelsregister".into(),
                Value::Bool(toets.gevonden),
            );
            params.insert(
                "rechtsvorm".into(),
                Value::String(toets.rechtsvorm.clone().unwrap_or_default()),
            );
            params.insert("naam_komt_overeen".into(), Value::Bool(toets.naam_match));
            let result = service
                .evaluate_law(
                    KIESWET_ID,
                    &[
                        "voldoet_aan_registratie_eisen",
                        "voldoet_eis_inschrijving",
                        "voldoet_eis_rechtsvorm",
                        "voldoet_eis_naam",
                    ],
                    params,
                    &date,
                )
                .map_err(|e| anyhow::anyhow!("toetsing Kieswet G 1 mislukt: {e}"))?;
            Ok(RegistratieToets {
                voldoet_aan_registratie_eisen: req_bool(
                    &result.outputs,
                    "voldoet_aan_registratie_eisen",
                )?,
                voldoet_eis_inschrijving: req_bool(&result.outputs, "voldoet_eis_inschrijving")?,
                voldoet_eis_rechtsvorm: req_bool(&result.outputs, "voldoet_eis_rechtsvorm")?,
                voldoet_eis_naam: req_bool(&result.outputs, "voldoet_eis_naam")?,
            })
        })
    })
    .await?
}

/// De uiterste betaaldatum van het voorschot (AWB 4:87: betaling binnen
/// zes weken na bekendmaking van de beschikking). De betaalopdracht draagt
/// deze termijn; de uitbetaling zelf is een feitelijke handeling.
pub async fn evaluate_betaaltermijn(
    corpus: Arc<LawCorpus>,
    bekendmaking_datum: String,
) -> anyhow::Result<String> {
    tokio::task::spawn_blocking(move || {
        with_service(&corpus, |service| {
            let mut params = BTreeMap::new();
            params.insert(
                "bekendmaking_datum".to_string(),
                Value::String(bekendmaking_datum.clone()),
            );
            let result = service
                .evaluate_law(
                    AWB_ID,
                    &["betaaltermijn_einddatum"],
                    params,
                    &bekendmaking_datum,
                )
                .map_err(|e| anyhow::anyhow!("betaaltermijn berekenen mislukt: {e}"))?;
            req_date(&result.outputs, "betaaltermijn_einddatum")
        })
    })
    .await?
}

/// Feiten over een bezwaarschrift, aangeleverd door de orchestratie
/// (formuliervelden en datumvergelijkingen); de oordelen komen uit de AWB.
#[derive(Debug, Clone, Default)]
pub struct BezwaarFeiten {
    // AWB 6:5 — vereisten aan het bezwaarschrift.
    pub naam_en_adres_vermeld: bool,
    pub dagtekening_vermeld: bool,
    pub besluit_omschreven: bool,
    pub gronden_vermeld: bool,
    pub ondertekend: bool,
    // AWB 6:6 — verzuimherstel.
    pub herstelgelegenheid_geboden: bool,
    pub binnen_hersteltermijn_aangevuld: bool,
    // AWB 6:9 — tijdigheid; de datumvergelijkingen zijn orchestratie
    // (engine vergelijkt geen datums), de regel staat in de wet.
    pub ontvangen_voor_einde_termijn: bool,
    pub ter_post_bezorgd_voor_einde_termijn: bool,
    pub ontvangen_binnen_week_na_termijn: bool,
}

/// Het AWB-oordeel over een bezwaarschrift (6:5, 6:6 en 6:9), plus alle
/// ruwe outputs voor opslag bij het bezwaar (zelfde bewijs-patroon als het
/// besluit).
#[derive(Debug, Clone, Serialize)]
pub struct BezwaarToets {
    pub voldoet_aan_vereisten: bool,
    pub herstel_vereist: bool,
    pub mag_niet_ontvankelijk_wegens_verzuim: bool,
    pub tijdig: bool,
    pub outputs: serde_json::Value,
}

pub async fn evaluate_bezwaarschrift(
    corpus: Arc<LawCorpus>,
    feiten: BezwaarFeiten,
    date: String,
) -> anyhow::Result<BezwaarToets> {
    tokio::task::spawn_blocking(move || {
        with_service(&corpus, |service| {
            let mut params: BTreeMap<String, Value> = BTreeMap::new();
            for (k, v) in [
                ("naam_en_adres_vermeld", feiten.naam_en_adres_vermeld),
                ("dagtekening_vermeld", feiten.dagtekening_vermeld),
                ("besluit_omschreven", feiten.besluit_omschreven),
                ("gronden_vermeld", feiten.gronden_vermeld),
                ("ondertekend", feiten.ondertekend),
                (
                    "herstelgelegenheid_geboden",
                    feiten.herstelgelegenheid_geboden,
                ),
                (
                    "binnen_hersteltermijn_aangevuld",
                    feiten.binnen_hersteltermijn_aangevuld,
                ),
                (
                    "ontvangen_voor_einde_termijn",
                    feiten.ontvangen_voor_einde_termijn,
                ),
                (
                    "ter_post_bezorgd_voor_einde_termijn",
                    feiten.ter_post_bezorgd_voor_einde_termijn,
                ),
                (
                    "ontvangen_binnen_week_na_termijn",
                    feiten.ontvangen_binnen_week_na_termijn,
                ),
            ] {
                params.insert(k.to_string(), Value::Bool(v));
            }
            let mut outputs: BTreeMap<String, Value> = BTreeMap::new();
            for gevraagd in [
                &[
                    "voldoet_aan_bezwaarschriftvereisten",
                    "ontbreekt_naam_en_adres",
                    "ontbreekt_dagtekening",
                    "ontbreekt_besluitomschrijving",
                    "ontbreken_gronden",
                    "ontbreekt_ondertekening",
                ][..],
                &["herstel_vereist", "mag_niet_ontvankelijk_wegens_verzuim"][..],
                &["bezwaar_tijdig"][..],
            ] {
                let result = service
                    .evaluate_law(AWB_ID, gevraagd, params.clone(), &date)
                    .map_err(|e| anyhow::anyhow!("toetsing bezwaarschrift mislukt: {e}"))?;
                outputs.extend(result.outputs);
            }
            Ok(BezwaarToets {
                voldoet_aan_vereisten: req_bool(&outputs, "voldoet_aan_bezwaarschriftvereisten")?,
                herstel_vereist: req_bool(&outputs, "herstel_vereist")?,
                mag_niet_ontvankelijk_wegens_verzuim: req_bool(
                    &outputs,
                    "mag_niet_ontvankelijk_wegens_verzuim",
                )?,
                tijdig: req_bool(&outputs, "bezwaar_tijdig")?,
                outputs: serde_json::to_value(&outputs).unwrap_or_default(),
            })
        })
    })
    .await?
}

/// AWB 7:3: mag van het horen worden afgezien? Advies aan de beoordelaar.
pub async fn evaluate_afzien_van_horen(
    corpus: Arc<LawCorpus>,
    kennelijk_niet_ontvankelijk: bool,
    kennelijk_ongegrond: bool,
    indiener_ziet_af_van_horen: bool,
    volledig_tegemoetgekomen: bool,
    date: String,
) -> anyhow::Result<bool> {
    tokio::task::spawn_blocking(move || {
        with_service(&corpus, |service| {
            let mut params: BTreeMap<String, Value> = BTreeMap::new();
            params.insert(
                "kennelijk_niet_ontvankelijk".into(),
                Value::Bool(kennelijk_niet_ontvankelijk),
            );
            params.insert(
                "kennelijk_ongegrond".into(),
                Value::Bool(kennelijk_ongegrond),
            );
            params.insert(
                "indiener_ziet_af_van_horen".into(),
                Value::Bool(indiener_ziet_af_van_horen),
            );
            params.insert(
                "volledig_tegemoetgekomen".into(),
                Value::Bool(volledig_tegemoetgekomen),
            );
            let result = service
                .evaluate_law(AWB_ID, &["mag_afzien_van_horen"], params, &date)
                .map_err(|e| anyhow::anyhow!("toetsing artikel 7:3 mislukt: {e}"))?;
            req_bool(&result.outputs, "mag_afzien_van_horen")
        })
    })
    .await?
}

/// AWB 7:10: de beslistermijn op het bezwaar (zes weken na afloop van de
/// bezwaartermijn).
pub async fn evaluate_beslistermijn_bezwaar(
    corpus: Arc<LawCorpus>,
    bezwaartermijn_einddatum: String,
    date: String,
) -> anyhow::Result<String> {
    tokio::task::spawn_blocking(move || {
        with_service(&corpus, |service| {
            let mut params = BTreeMap::new();
            params.insert(
                "bezwaartermijn_verstreken_op".to_string(),
                Value::String(bezwaartermijn_einddatum.clone()),
            );
            let result = service
                .evaluate_law(AWB_ID, &["beslistermijn_bezwaar_einddatum"], params, &date)
                .map_err(|e| anyhow::anyhow!("beslistermijn bezwaar berekenen mislukt: {e}"))?;
            req_date(&result.outputs, "beslistermijn_bezwaar_einddatum")
        })
    })
    .await?
}

/// Compute the bezwaartermijn dates after bekendmaking (AWB 6:8), with the
/// weekend extension from the Algemene termijnenwet (art. 1).
pub async fn evaluate_bezwaartermijn(
    corpus: Arc<LawCorpus>,
    bekendmaking_datum: String,
) -> anyhow::Result<BezwaartermijnUitkomst> {
    tokio::task::spawn_blocking(move || {
        with_service(&corpus, |service| {
            let mut params = BTreeMap::new();
            params.insert(
                "bekendmaking_datum".to_string(),
                Value::String(bekendmaking_datum.clone()),
            );
            let result = service
                .evaluate_law(
                    AWB_ID,
                    &["bezwaartermijn_startdatum", "bezwaartermijn_einddatum"],
                    params,
                    &bekendmaking_datum,
                )
                .map_err(|e| anyhow::anyhow!("bezwaartermijn berekenen mislukt: {e}"))?;
            let einddatum = req_date(&result.outputs, "bezwaartermijn_einddatum")?;

            let mut atw_params = BTreeMap::new();
            atw_params.insert("einddatum".to_string(), Value::String(einddatum.clone()));
            let verlengd = service
                .evaluate_law(
                    ATW_ID,
                    &["verlengde_einddatum"],
                    atw_params,
                    &bekendmaking_datum,
                )
                .map_err(|e| anyhow::anyhow!("termijnverlenging berekenen mislukt: {e}"))?;

            Ok(BezwaartermijnUitkomst {
                startdatum: req_date(&result.outputs, "bezwaartermijn_startdatum")?,
                einddatum: req_date(&verlengd.outputs, "verlengde_einddatum")?,
            })
        })
    })
    .await?
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn procedure() -> Procedure {
        beschikking_procedure(include_str!(
            "../../../corpus-poc/napp/law/wet_op_de_politieke_partijen/2026-01-01.yaml"
        ))
        .expect("procedure uit de wet")
    }

    /// De stages waarnaar de orchestratie verwijst bestaan in de wet en de
    /// indiening mondt uit in de behandelstage.
    #[test]
    fn procedure_komt_uit_de_wet() {
        let p = procedure();
        assert_eq!(
            p.na_indiening().expect("stage na indiening"),
            STAGE_BEHANDELING
        );
    }

    /// Vooruit mag (ook over momentane stages heen), terug of stilstaan niet.
    #[test]
    fn transities_volgen_de_volgorde_van_de_wet() {
        let p = procedure();
        assert!(p.transitie(STAGE_BEHANDELING, STAGE_BESLUIT).is_ok());
        assert!(p.transitie(STAGE_BESLUIT, STAGE_BEKENDMAKING).is_ok());
        assert!(p.transitie(STAGE_BESLUIT, STAGE_BEZWAAR).is_ok());
        assert!(p.transitie(STAGE_BESLUIT, STAGE_BESLUIT).is_err());
        assert!(p.transitie(STAGE_BEZWAAR, STAGE_BESLUIT).is_err());
        assert!(p.transitie("ONBEKEND", STAGE_BESLUIT).is_err());
    }

    /// Elke output waarnaar de orchestratie verwijst bestaat in de echte
    /// wetteksten. Hernoemt iemand een output in de YAML zonder de
    /// uitvoering mee te nemen, dan faalt deze test (en de startup).
    #[test]
    fn contract_houdt_tegen_de_echte_wetteksten() {
        let corpus = Arc::new(LawCorpus::embedded());
        valideer_contract(&corpus).expect("contract wet↔uitvoering");
    }

    /// Een verwijzing naar een niet-bestaande output wordt gemeld met de
    /// naam van de output en de wet.
    #[test]
    fn contractbreuk_wordt_luid_gemeld() {
        let corpus = Arc::new(LawCorpus::embedded());
        let fout = valideer_contract_met(&corpus, &[(WPP_ID, &["bestaat_niet"])])
            .expect_err("onbekende output hoort te falen");
        let melding = fout.to_string();
        assert!(melding.contains("'bestaat_niet'"), "{melding}");
        assert!(melding.contains(WPP_ID), "{melding}");
    }
}
