//! De Awb-levensloop over het echte corpus (RFC-007, RFC-008).
//!
//! De unit-tests bij `execute_stage` bewijzen het raderwerk op synthetische
//! wetten. Deze tests bewijzen dat het ook op de echte Awb klopt: dat een wet
//! die een BESCHIKKING produceert de levensloop binnenkomt zonder er zelf iets
//! van te weten, dat de haken op de juiste fase vuren, en dat een bezwaartermijn
//! als datum uit de keten komt in plaats van als "zes weken".
//!
//! Dat onderscheid doet ertoe: de haken zijn los getest, de wetten zijn los
//! gevalideerd, maar of de Awb in het corpus daadwerkelijk vuurt op een wet die
//! haar niet kent, stond nergens vast. Dat is nu juist de eigenschap waar
//! RFC-007 op rust.

// Allowed crate-wide: test helpers outside a `#[test]` fn may unwrap, expect and
// panic too, because that is how a failing fixture reports itself.
// `allow-*-in-tests` in clippy.toml only reaches `#[test]` fns.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use regelrecht_engine::{ExecutionOutcome, LawExecutionService, StageState, Value};
use std::collections::BTreeMap;
use walkdir::WalkDir;

/// Laad het hele regelingencorpus, zoals de andere integratietests doen.
fn load_corpus() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    let dir = common::regulation_base_path().join("nl");
    assert!(dir.exists(), "corpus niet gevonden: {}", dir.display());

    for entry in WalkDir::new(&dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|ext| ext == "yaml") {
            if let Ok(content) = std::fs::read_to_string(path) {
                let _ = service.load_law(&content);
            }
        }
    }
    service
}

fn date(s: &str) -> Value {
    Value::String(s.to_string())
}

/// Doorloopt de levensloop tot hij klaar is of niet verder kan, en levert elke
/// keer aan wat er gevraagd wordt. Zo leest een test als het verhaal van een
/// besluit en niet als een reeks aanroepen.
fn run_lifecycle(
    service: &LawExecutionService,
    law_id: &str,
    output: &str,
    supply: &BTreeMap<String, Value>,
    calculation_date: &str,
) -> (BTreeMap<String, Value>, Option<StageState>, Vec<String>) {
    let mut state: Option<StageState> = None;
    let mut given: BTreeMap<String, Value> = BTreeMap::new();

    // Ruim boven het aantal fasen van de beschikkingsprocedure: elke ronde
    // levert minstens één ontbrekend gegeven aan, dus dit termineert.
    for _ in 0..16 {
        let outcome = service
            .execute_stage(
                law_id,
                output,
                state.clone(),
                given.clone(),
                calculation_date,
            )
            .expect("levensloop kon niet worden uitgevoerd");

        match outcome {
            ExecutionOutcome::Complete(result) => {
                return (result.outputs.clone(), state, Vec::new());
            }
            ExecutionOutcome::Yielded {
                state: next,
                outputs,
                pending_inputs,
            } => {
                // Alleen aanleveren wat de aanroeper ook echt heeft; wat hij
                // niet heeft, is waar de levensloop op wacht.
                let mut supplied_any = false;
                for name in &pending_inputs {
                    if let Some(v) = supply.get(name) {
                        given.insert(name.clone(), v.clone());
                        supplied_any = true;
                    }
                }
                if !supplied_any {
                    return (outputs, Some(next), pending_inputs);
                }
                state = Some(next);
            }
        }
    }
    panic!("levensloop kwam niet tot een einde");
}

/// Of deze wet in het geladen corpus zit.
///
/// De suite draait over twee corpora: het regelingencorpus en, met
/// `REGULATION_PATH`, het democorpus. De Awb-tests horen bij allebei te
/// slagen — juist dáár kwam een fout in het democorpus aan het licht — maar de
/// Vreemdelingenwet staat alleen in het eerste. Een test over een wet die er
/// niet is, zegt niets; die slaat zichzelf over en zegt dat ook.
fn heeft_wet(service: &LawExecutionService, law_id: &str) -> bool {
    service.list_laws().contains(&law_id)
}

/// Een wet die een BESCHIKKING produceert komt de Awb-levensloop binnen, ook al
/// noemt ze de Awb nergens. Dat is de kern van RFC-007: de verhouding is
/// eenzijdig, de Awb kent de Vreemdelingenwet niet en andersom ook niet.
#[test]
fn een_beschikking_komt_de_awb_levensloop_binnen() {
    let service = load_corpus();
    if !heeft_wet(&service, "vreemdelingenwet_2000") {
        eprintln!("overgeslagen: de Vreemdelingenwet zit niet in dit corpus");
        return;
    }
    let supply = BTreeMap::new();

    let (_outputs, state, pending) = run_lifecycle(
        &service,
        "vreemdelingenwet_2000",
        "minister_is_bevoegd",
        &supply,
        "2026-03-12",
    );

    // Zonder aanvraagdatum komt de levensloop niet voorbij de eerste fase, en
    // zegt hij wat hij nodig heeft in plaats van stilletjes door te rekenen.
    let state = state.expect("de levensloop had moeten wachten op een gegeven");
    assert_eq!(state.procedure_id, "beschikking");
    assert_eq!(state.current_stage, "AANVRAAG");
    assert_eq!(pending, vec!["aanvraag_datum".to_string()]);
}

/// Een wet zonder afwijkende termijn krijgt de zes weken van Awb 6:7, en een
/// einddatum die daarbij hoort. De wet zelf zegt niets over bezwaar; ze
/// produceert een BESCHIKKING en dat is genoeg.
///
/// Een eigen wetje en geen wet uit het corpus: elke BESCHIKKING daar vraagt om
/// persoonsgegevens (een BSN en wat daaraan hangt), en dan zou deze test over
/// het aanleveren van die gegevens gaan in plaats van over de levensloop. De
/// twee andere tests draaien wel op het echte corpus.
const WET_ZONDER_AFWIJKING: &str = r#"
$id: test_besluit_zonder_afwijking
regulatory_layer: WET
publication_date: '2026-01-01'
valid_from: '2020-01-01'
name: Testwet die een beschikking geeft
articles:
  - number: '1'
    text: Het bestuursorgaan kent de voorziening toe.
    machine_readable:
      execution:
        produces:
          PRODUCES
        output:
          - name: toegekend
            type: boolean
        actions:
          - output: toegekend
            value: true
"#;

/// De haken van de Awb vuren op de fase waar ze horen: 3:46 en 6:7 bij het
/// besluit, 6:8 pas na de bekendmaking. Zonder fasen zouden ze alle drie
/// tegelijk vuren, en dan rekent 6:8 met een bekendmakingsdatum die er nog niet
/// is (RFC-008, "wat er misgaat zonder fasen").
#[test]
fn de_termijn_is_een_datum_en_niet_zes_weken() {
    let mut service = load_corpus();
    // Een Awb die soorten vestigt (schema v0.9.0) grijpt in op een
    // beschikking van die soort; een Awb zonder soorten, zoals die van het
    // democorpus, op het rechtskarakter BESCHIKKING.
    let produces = if service
        .resolver()
        .kind("algemene_wet_bestuursrecht/beschikking")
        .is_some()
    {
        "- type: algemene_wet_bestuursrecht/beschikking"
    } else {
        "legal_character: BESCHIKKING"
    };
    service
        .load_law(&WET_ZONDER_AFWIJKING.replace("PRODUCES", produces))
        .expect("testwet kon niet worden geladen");

    let mut supply = BTreeMap::new();
    supply.insert("aanvraag_datum".to_string(), date("2026-01-05"));
    supply.insert("beslistermijn_start".to_string(), date("2026-01-06"));
    supply.insert("besluit_datum".to_string(), date("2026-03-12"));
    supply.insert("bekendmaking_datum".to_string(), date("2026-03-12"));

    let (outputs, _state, pending) = run_lifecycle(
        &service,
        "test_besluit_zonder_afwijking",
        "toegekend",
        &supply,
        "2026-03-12",
    );

    assert!(
        pending.is_empty(),
        "de levensloop bleef wachten op {pending:?}"
    );

    // Awb 6:7 als haak op BESLUIT: de termijn zelf.
    assert_eq!(
        outputs.get("termijn_weken"),
        Some(&Value::Int(6)),
        "outputs: {outputs:?}"
    );
    // Awb 3:46 als haak op BESLUIT: de motiveringsplicht.
    assert_eq!(
        outputs.get("motivering_vereist"),
        Some(&Value::Bool(true)),
        "outputs: {outputs:?}"
    );
    // Awb 6:8 als haak op BEKENDMAKING: de dag ná de bekendmaking, en de
    // einddatum zes weken ná de bekendmaking (de startdag telt mee).
    assert_eq!(
        outputs.get("termijn_startdatum"),
        Some(&date("2026-03-13")),
        "outputs: {outputs:?}"
    );
    assert_eq!(
        outputs.get("termijn_einddatum"),
        Some(&date("2026-04-23")),
        "outputs: {outputs:?}"
    );
}

/// Lex specialis: de Vreemdelingenwet zegt "in afwijking van artikel 6:7 ...
/// vier weken", en dat werkt door in de datum die 6:8 eruit rekent, zonder dat
/// 6:8 van de Vreemdelingenwet weet (RFC-007, "de override werkt door de
/// verwijzingsketen heen").
#[test]
fn een_afwijkende_termijn_werkt_door_in_de_einddatum() {
    let service = load_corpus();
    if !heeft_wet(&service, "vreemdelingenwet_2000") {
        eprintln!("overgeslagen: de Vreemdelingenwet zit niet in dit corpus");
        return;
    }
    let mut supply = BTreeMap::new();
    supply.insert("aanvraag_datum".to_string(), date("2026-01-05"));
    supply.insert("beslistermijn_start".to_string(), date("2026-01-06"));
    supply.insert("besluit_datum".to_string(), date("2026-03-12"));
    supply.insert("bekendmaking_datum".to_string(), date("2026-03-12"));

    let (outputs, _state, pending) = run_lifecycle(
        &service,
        "vreemdelingenwet_2000",
        "minister_is_bevoegd",
        &supply,
        "2026-03-12",
    );

    assert!(
        pending.is_empty(),
        "de levensloop bleef wachten op {pending:?}"
    );
    assert_eq!(
        outputs.get("termijn_weken"),
        Some(&Value::Int(4)),
        "outputs: {outputs:?}"
    );
    // Vier weken na de bekendmaking, niet zes.
    assert_eq!(
        outputs.get("termijn_einddatum"),
        Some(&date("2026-04-09")),
        "outputs: {outputs:?}"
    );
}
