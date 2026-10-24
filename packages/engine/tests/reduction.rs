//! A reduction reads the decisions taken under an article
//! (`source.decisions`, schema v0.8.2, RFC-048), named in the law or by a
//! parameter (schema v0.8.11); only a reduction does, and only a data
//! source or the chronicle of the cell that runs it answers that input,
//! also for a reduction it calls. For its norms it calls
//! policy like any rulework (`source.regulation`), passing what it read as
//! parameters. The school's reduction of its decisions on leave is such a
//! reduction. It fills no open term: the head's policy binds the term to the
//! answer of the cell (`source.reduction`, schema v0.8.9), and the engine
//! asks the caller's answerer for it, at any depth, also where the article
//! is called anew (RFC-050), and records who answered and through which
//! binding.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

use regelrecht_engine::{
    Answer, AnswerProvenance, AnswerRequest, ArticleBasedLaw, Authority, Call, ChronicleRead,
    DataSource, EngineError, FilledBy, LawExecutionService, LawLoad, OpenTermAnswerer, Value,
};
use serde_json::json;

const REDUCTION: &str = "reductie_verlof_al_verleend_in_schooljaar";
const ONCE: &str = "verlof_al_verleend_in_schooljaar";
/// The head's own finding on it, which the binding takes where the cell
/// gives no answer (art. 2 of the policy, RFC-048 §2).
const ONCE_HEAD: &str = "verlof_al_verleend_in_schooljaar_volgens_hoofd";
/// The head's policy: the norm the reduction applies (art. 1) and the
/// binding of the term of art. 13a to the cell's answer (art. 2).
const POLICY: &str = "fictief_beleid_vakantieverlof_obs_de_klimop";
/// The policy the head's policy calls for the school year.
const SCHOOL_YEAR: &str = "fictief_beleid_schooljaargrenzen_obs_de_klimop";

fn corpus(path: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../corpus")
        .join(path);
    std::fs::read_to_string(path).unwrap()
}

/// The reduction of the fictional school that reads its decisions, from
/// the corpus.
fn reduction_yaml() -> String {
    corpus(&format!(
        "cells/obs_de_klimop/reductions/{REDUCTION}/2026-08-01.yaml"
    ))
}

/// The head's policy, from the corpus.
fn policy_yaml() -> String {
    corpus(&format!(
        "regulation/nl/uitvoeringsbeleid/{POLICY}/2026-08-01.yaml"
    ))
}

/// The reduction, the Awb reduction and the school's policies it calls.
fn service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(&reduction_yaml()).unwrap();
    service.load_law(&awb_reduction_yaml()).unwrap();
    service.load_law(&policy_yaml()).unwrap();
    service
        .load_law(&corpus(&format!(
            "regulation/nl/uitvoeringsbeleid/{SCHOOL_YEAR}/2026-08-01.yaml"
        )))
        .unwrap();
    service
}

/// A decision under art. 13a, as the cell gives it: the fields and the
/// outputs of the decision under their own names, its article and its id.
fn decision(bsn: &str, first: &str, granted: bool) -> serde_json::Value {
    json!({
        "id": format!("{bsn}-{first}-{granted}"),
        "basis": "leerplichtwet_1969#13a",
        "bsn_jongere": bsn,
        "eerste_verlofdag": first,
        "laatste_verlofdag": first,
        "verlof_verleend": granted,
    })
}

/// A decision on an objection under Awb 7:11 against the decision
/// `contested` (its id), that revoked it or not (RFC-050).
fn on_objection(contested: &serde_json::Value, revoked: bool) -> serde_json::Value {
    json!({
        "id": format!("bezwaar-{}", contested["id"].as_str().unwrap()),
        "bestreden_besluit": contested["id"],
        "bezwaar_ontvankelijk": true,
        "bestreden_besluit_herroepen": revoked,
    })
}

/// [`on_objection`], which revoked `contested` and took `instead` in its
/// place, or upheld it (`instead` is None), under the id `id`.
fn revoked_for(
    id: &str,
    contested: &serde_json::Value,
    instead: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut decision = on_objection(contested, instead.is_some());
    decision["id"] = json!(id);
    decision["nieuw_besluit"] = json!(instead.map(|d| d["id"].clone()));
    decision
}

fn evaluate(
    service: &LawExecutionService,
    bsn: &str,
    first: &str,
) -> regelrecht_engine::Result<regelrecht_engine::ArticleResult> {
    let parameters = BTreeMap::from([
        ("bsn_jongere".to_string(), Value::String(bsn.into())),
        ("eerste_verlofdag".to_string(), Value::String(first.into())),
    ]);
    service.evaluate_law(REDUCTION, &[ONCE], parameters, "2027-03-01")
}

/// Whether the head already granted `bsn` leave in the school year of
/// `first`.
fn already(service: &LawExecutionService, bsn: &str, first: &str) -> bool {
    let result = evaluate(service, bsn, first).unwrap();
    result.outputs[ONCE] == Value::Bool(true)
}

/// The decisions under art. 13a and those under Awb 7:11, each as one list,
/// whatever the criteria, for `law` only: what a cell would give the
/// reduction of the Awb, which reads them for the school's reduction.
struct Decisions {
    law: String,
    rows: BTreeMap<&'static str, Value>,
}

impl DataSource for Decisions {
    fn name(&self) -> &str {
        "kroniek"
    }
    fn priority(&self) -> i32 {
        10
    }
    fn source_type(&self) -> &str {
        "chronicle"
    }
    fn has_field(&self, field: &str) -> bool {
        self.rows.contains_key(field)
    }
    fn get(&self, field: &str, _criteria: &BTreeMap<String, Value>) -> Option<Value> {
        self.rows.get(field).cloned()
    }
    fn fields(&self) -> Vec<&str> {
        self.rows.keys().copied().collect()
    }
    fn law_scope(&self) -> Option<&str> {
        Some(&self.law)
    }
    fn key_fields(&self) -> Option<&[String]> {
        Some(&[])
    }
}

/// Answer `besluiten` with `decisions` and `beslissingen_op_bezwaar` with
/// `on_objection`, for `law` only.
fn supply_for(
    service: &mut LawExecutionService,
    law: &str,
    decisions: serde_json::Value,
    on_objection: serde_json::Value,
) {
    service.add_data_source(Box::new(Decisions {
        law: law.to_string(),
        rows: BTreeMap::from([
            ("besluiten", Value::from(decisions)),
            ("beslissingen_op_bezwaar", Value::from(on_objection)),
            // The judgments the school received (RFC-053): none here.
            ("uitspraken", Value::from(json!([]))),
        ]),
    }));
}

/// Answer the decisions under art. 13a, with no decision on an objection.
fn supply(service: &mut LawExecutionService, decisions: serde_json::Value) {
    supply_for(service, AWB_REDUCTION, decisions, json!([]));
}

#[test]
fn leave_granted_to_the_child_in_the_school_year_counts() {
    let mut service = service();
    supply(
        &mut service,
        json!([decision("999990019", "2026-10-05", true)]),
    );
    assert!(already(&service, "999990019", "2027-05-10"));
}

#[test]
fn a_refusal_another_child_or_another_school_year_does_not_count() {
    let mut service = service();
    supply(
        &mut service,
        json!([
            decision("999990019", "2026-10-05", false),
            decision("999990020", "2026-10-05", true),
            decision("999990019", "2026-07-27", true),
            decision("999990019", "2027-08-02", true),
        ]),
    );
    assert!(!already(&service, "999990019", "2027-05-10"));
}

#[test]
fn a_grant_revoked_on_objection_does_not_count() {
    let mut service = service();
    let revoked = decision("999990019", "2026-10-05", true);
    supply_for(
        &mut service,
        AWB_REDUCTION,
        json!([revoked]),
        json!([on_objection(&revoked, true)]),
    );
    assert!(!already(&service, "999990019", "2027-05-10"));
}

#[test]
fn a_grant_upheld_on_objection_counts() {
    let mut service = service();
    // Both in the school year 2026-2027 of the query, so only the
    // objection tells them apart.
    let upheld = decision("999990019", "2026-10-05", true);
    let other = decision("999990019", "2027-02-01", true);
    supply_for(
        &mut service,
        AWB_REDUCTION,
        json!([upheld, other]),
        json!([on_objection(&upheld, false), on_objection(&other, true)]),
    );
    assert!(already(&service, "999990019", "2027-05-10"));
}

#[test]
fn the_reduction_asks_the_policy_for_the_school_year_of_the_first_day() {
    let mut service = service();
    supply(
        &mut service,
        json!([decision("999990019", "2027-07-31", true)]),
    );
    // 31 July is the last day of the school year 2026-2027 and 1 August the
    // first of the next, as the policy says.
    assert!(already(&service, "999990019", "2026-08-01"));
    assert!(!already(&service, "999990019", "2027-08-01"));
    let result = evaluate(&service, "999990019", "2027-08-01").unwrap();
    assert_eq!(
        result.resolved_inputs["begin_schooljaar"],
        Value::String("2027-08-01".into())
    );
    // Without the policy the school year is not known, nor the answer.
    let mut alone = LawExecutionService::new();
    alone.load_law(&reduction_yaml()).unwrap();
    alone.load_law(&awb_reduction_yaml()).unwrap();
    alone.load_law(&policy_yaml()).unwrap();
    supply(
        &mut alone,
        json!([decision("999990019", "2027-07-31", true)]),
    );
    let answer = evaluate(&alone, "999990019", "2027-08-01").map(|r| r.outputs[ONCE].clone());
    assert!(!matches!(answer, Ok(Value::Bool(_))), "{answer:?}");
}

#[test]
fn no_decisions_is_no_leave_granted() {
    let mut service = service();
    supply(&mut service, json!([]));
    assert!(!already(&service, "999990019", "2027-05-10"));
}

#[test]
fn decisions_nobody_supplies_are_an_error_not_an_empty_list() {
    // The judgments are supplied (RFC-053), the decisions are not.
    let mut service = service();
    service.add_data_source(Box::new(Decisions {
        law: AWB_REDUCTION.to_string(),
        rows: BTreeMap::from([("uitspraken", Value::from(json!([])))]),
    }));
    let e = evaluate(&service, "999990019", "2027-05-10").unwrap_err();
    let message = e.to_string();
    assert!(matches!(e, EngineError::DataSourceError(_)), "{message}");
    assert!(
        message.contains("reads the decisions taken under")
            && message.contains("no data source supplies them"),
        "{message}"
    );
}

#[test]
fn judgments_nobody_supplies_are_an_error_not_an_empty_list() {
    // The decisions are supplied, the judgments (RFC-053), which are
    // executograms, are not.
    let mut service = service();
    service.add_data_source(Box::new(Decisions {
        law: AWB_REDUCTION.to_string(),
        rows: BTreeMap::from([
            (
                "besluiten",
                Value::from(json!([decision("999990019", "2026-10-05", true)])),
            ),
            ("beslissingen_op_bezwaar", Value::from(json!([]))),
        ]),
    }));
    let e = evaluate(&service, "999990019", "2027-05-10").unwrap_err();
    let message = e.to_string();
    assert!(matches!(e, EngineError::DataSourceError(_)), "{message}");
    assert!(
        message.contains(
            "reads the recorded facts algemene_wet_bestuursrecht#8:79#afschrift_van_de_uitspraak"
        ) && message.contains("no data source supplies them"),
        "{message}"
    );
}

#[test]
fn a_source_for_another_law_does_not_answer_the_decisions() {
    let mut service = service();
    supply_for(&mut service, "een_andere_wet", json!([]), json!([]));
    let e = evaluate(&service, "999990019", "2027-05-10").unwrap_err();
    let message = e.to_string();
    // The same failure as without any source: the decisions stay unanswered.
    assert!(matches!(e, EngineError::DataSourceError(_)), "{message}");
    assert!(
        message.contains("no data source supplies them"),
        "{message}"
    );
}

/// `yaml` with `from` replaced by `to`.
fn changed(yaml: &str, from: &str, to: &str) -> String {
    assert!(yaml.contains(from), "{from}");
    yaml.replace(from, to)
}

/// The reduction with its layer or its input changed.
fn variant(from: &str, to: &str) -> String {
    changed(&reduction_yaml(), from, to)
}

/// The reduction of the Awb with its layer or its input changed.
fn awb_variant(from: &str, to: &str) -> String {
    changed(&awb_reduction_yaml(), from, to)
}

fn refused(yaml: &str, says: &str) {
    let e = ArticleBasedLaw::from_yaml_str(yaml)
        .unwrap_err()
        .to_string();
    assert!(e.contains(says), "{e}");
}

#[test]
fn only_a_reduction_reads_decisions() {
    for layer in ["UITVOERINGSBELEID", "BELEIDSREGEL"] {
        refused(
            &awb_variant(
                "regulatory_layer: REDUCTIE",
                &format!("regulatory_layer: {layer}"),
            ),
            "only a REDUCTIE reads decisions or facts",
        );
    }
}

#[test]
fn decisions_stand_alone_and_name_an_article() {
    // Each narrowing key on its own is refused, not only in company.
    for narrowing in [
        "regulation: leerplichtwet_1969",
        "output: verlof_verleend",
        "parameters:\n                bsn_jongere: $bsn_jongere",
        "endpoint: verlof",
    ] {
        refused(
            &awb_variant(
                "decisions: algemene_wet_bestuursrecht#7:11",
                &format!("decisions: algemene_wet_bestuursrecht#7:11\n              {narrowing}"),
            ),
            "no regulation, output, parameters or endpoint",
        );
    }
    refused(
        &awb_variant(
            "decisions: algemene_wet_bestuursrecht#7:11",
            "decisions: algemene_wet_bestuursrecht",
        ),
        "is neither <regulation>#<article> without a paragraph nor a $-reference",
    );
}

#[test]
fn decisions_named_by_a_reference_name_a_parameter_of_the_article() {
    // The reduction of the Awb names them by its parameter basis.
    ArticleBasedLaw::from_yaml_str(&awb_reduction_yaml()).unwrap();
    for not_a_parameter in ["$besluiten_in_stand", "$basis.wet"] {
        refused(
            &awb_variant(
                "decisions: $basis",
                &format!("decisions: {not_a_parameter}"),
            ),
            "is no parameter of the article",
        );
    }
}

/// The reduction with `key` before its execution.
fn acting(key: &str) -> String {
    variant(
        "      execution:\n        # Wat het antwoord",
        &format!("{key}      execution:\n        # Wat het antwoord"),
    )
}

#[test]
fn a_reduction_does_not_act_on_law() {
    refused(
        &acting(
            "      hooks:\n        - hook_point: pre_actions\n          \
             applies_to:\n            legal_character: BESCHIKKING\n",
        ),
        "does not act on law (hooks)",
    );
    refused(
        &variant(
            "      execution:\n        # Wat het antwoord",
            "      execution:\n        produces:\n          legal_character: BESCHIKKING\n          \
             decision_type: TOEKENNING\n        # Wat het antwoord",
        ),
        "does not act on law (produces)",
    );
    refused(
        &acting(
            "      open_terms:\n        - id: verlof_grens\n          \
             description: De grens.\n          type: number\n          \
             delegated_to: het hoofd\n          delegation_type: UITVOERINGSBELEID\n",
        ),
        "does not act on law (open_terms)",
    );
}

#[test]
fn a_reduction_fills_no_open_term() {
    // Its cell gives it as a named answer, which a policy binds (RFC-048).
    let implements = "      implements:\n        - law: leerplichtwet_1969\n          \
                      article: 13a\n          open_term: verlof_al_verleend_in_schooljaar\n";
    refused(
        &acting(implements),
        "a REDUCTIE fills no open term (implements)",
    );
    // Schema v0.8.5 to v0.8.8 allowed it, and a file that names one of them
    // loads; the engine gives the declaration no meaning.
    for allowed in ["v0.8.5", "v0.8.8"] {
        ArticleBasedLaw::from_yaml_str(&acting(implements).replace("v0.8.12", allowed)).unwrap();
    }
    refused(
        &acting(implements).replace("v0.8.12", "v0.8.4"),
        "a REDUCTIE fills no open term (implements)",
    );
    // Without `$schema` the file is held to the current version.
    let unnamed: String = acting(implements)
        .lines()
        .filter(|line| !line.starts_with("$schema:"))
        .collect::<Vec<_>>()
        .join("\n");
    refused(&unnamed, "a REDUCTIE fills no open term (implements)");
}

/// The head's policy with `from` replaced by `to` in its binding of art. 2.
fn binding(from: &str, to: &str) -> String {
    changed(&policy_yaml(), from, to)
}

/// The source of the binding of art. 2.
const BINDS: &str = "              reduction: verlof_al_verleend_in_schooljaar\n";

#[test]
fn an_answer_stands_alone() {
    ArticleBasedLaw::from_yaml_str(&policy_yaml()).unwrap();
    for (key, line) in [
        ("regulation", "regulation: leerplichtwet_1969"),
        ("output", "output: verlof_verleend"),
        ("endpoint", "endpoint: verlof"),
        ("decisions", "decisions: leerplichtwet_1969#13a"),
        ("facts", "facts: leerplichtwet_1969#13a#feit"),
        ("article", "article: leerplichtwet_1969#13a"),
    ] {
        refused(
            &binding(BINDS, &format!("{BINDS}              {line}\n")),
            &format!(
                "takes the answer 'verlof_al_verleend_in_schooljaar' of a cell, which no {key} \
              can stand beside"
            ),
        );
    }
}

#[test]
fn only_the_policy_of_the_deciding_authority_binds_a_term_to_an_answer() {
    for layer in ["WET", "BELEIDSREGEL"] {
        refused(
            &binding(
                "regulatory_layer: UITVOERINGSBELEID",
                &format!("regulatory_layer: {layer}"),
            ),
            "only the UITVOERINGSBELEID of the authority that decides binds a term",
        );
    }
    // A reduction fills no term at all, so it binds none either.
    refused(
        &binding(
            "regulatory_layer: UITVOERINGSBELEID",
            "regulatory_layer: REDUCTIE",
        ),
        "a REDUCTIE fills no open term",
    );
}

#[test]
fn a_binding_fills_a_term_in_a_policy_that_names_its_holder() {
    let yaml = policy_yaml();
    let start = yaml.find("  - number: '3'").unwrap();
    let implements = &yaml[start..][..yaml[start..].find("      execution:").unwrap()];
    let without = yaml.replacen(
        implements,
        &implements[..implements.find("      implements:").unwrap()],
        1,
    );
    refused(&without, "but the article fills no open term (implements)");
    for holder in [
        "",
        "competent_authority: '#hoofd'\n",
        "competent_authority:\n  - name: Het hoofd van OBS De Klimop\n",
    ] {
        refused(
            &binding(
                "competent_authority:\n  name: Het hoofd van OBS De Klimop\n",
                holder,
            ),
            "names no holder by name (competent_authority, one authority)",
        );
    }
}

#[test]
fn a_binding_takes_an_answer_only_from_schema_v0_8_9() {
    for older in ["v0.8.2", "v0.8.8"] {
        refused(
            &policy_yaml().replace("v0.8.9", older),
            "which a policy does only from schema v0.8.9",
        );
    }
    // Without `$schema` the file is held to the current version.
    let unnamed: String = policy_yaml()
        .lines()
        .filter(|line| !line.starts_with("$schema:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(unnamed, policy_yaml());
    ArticleBasedLaw::from_yaml_str(&unnamed).unwrap();
}

#[test]
fn only_an_answer_takes_a_combine_that_fits_its_type() {
    let combined = |op: &str| binding(BINDS, &format!("{BINDS}              combine: {op}\n"));
    ArticleBasedLaw::from_yaml_str(&combined("OR")).unwrap();
    refused(&combined("ADD"), "which does not fit its type boolean");
    refused(&combined("ONLY"), "which is no rule for answers");
    // Beside another way to the value, combine has nothing to combine.
    let schooljaar = "              output: begin_schooljaar\n";
    refused(
        &binding(
            schooljaar,
            &format!("{schooljaar}              combine: OR\n"),
        ),
        "has combine OR without the answer of a cell",
    );
}

// ---------------------------------------------------------------------------
// The head's policy binds the open term of art. 13a to the cell's answer
// (source.reduction, schema v0.8.9)
// ---------------------------------------------------------------------------

const AWB: &str = "algemene_wet_bestuursrecht";

/// The Leerplichtwet, the Awb and the school's policies from the corpus,
/// the head's binding among them, and the reduction if `with_reduction`.
fn corpus_service(with_reduction: bool) -> LawExecutionService {
    let regulation = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/regulation/nl");
    let mut service = LawExecutionService::new();
    for law in [
        "wet/leerplichtwet_1969",
        "wet/algemene_wet_bestuursrecht",
        "uitvoeringsbeleid/fictief_beleid_schooljaar_obs_de_klimop",
        "uitvoeringsbeleid/fictief_beleid_schooljaargrenzen_obs_de_klimop",
        "uitvoeringsbeleid/fictief_beleid_vakantieverlof_obs_de_klimop",
    ] {
        for entry in std::fs::read_dir(regulation.join(law)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|x| x == "yaml") {
                service
                    .load_law(&std::fs::read_to_string(&path).unwrap())
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            }
        }
    }
    if with_reduction {
        service.load_law(&reduction_yaml()).unwrap();
        service.load_law(&awb_reduction_yaml()).unwrap();
    }
    service
}

/// The application of the child for 10 to 14 May 2027, and the head's
/// judgements under art. 11 onder f.
fn application() -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("bsn_jongere".to_string(), Value::String("999990019".into())),
        (
            "eerste_verlofdag".to_string(),
            Value::String("2027-05-10".into()),
        ),
        (
            "laatste_verlofdag".to_string(),
            Value::String("2027-05-14".into()),
        ),
        (
            "specifieke_aard_beroep_ouder".to_string(),
            Value::Bool(true),
        ),
        (
            "verlof_valt_buiten_schoolvakanties_volgens_hoofd".to_string(),
            Value::Bool(true),
        ),
    ])
}

/// `parameters` as the call takes them: the head's findings on the open
/// terms of the policy (`..._volgens_hoofd`) apart, as findings.
fn split(
    parameters: BTreeMap<String, Value>,
) -> (BTreeMap<String, Value>, BTreeMap<String, Value>) {
    parameters
        .into_iter()
        .partition(|(name, _)| !name.ends_with("_volgens_hoofd"))
}

fn granted_13a(
    service: &LawExecutionService,
    parameters: BTreeMap<String, Value>,
) -> regelrecht_engine::Result<regelrecht_engine::ArticleResult> {
    let (parameters, findings) = split(parameters);
    service.evaluate_law_with_trace_in(
        "leerplichtwet_1969",
        &["verlof_verleend"],
        parameters,
        Call::with_findings(findings),
        "2026-10-27",
    )
}

/// An answerer that gives the answer the head's policy binds the term of
/// art. 13a to, as a cell would: `value`, or nothing, and keeps what the
/// engine asked.
struct Answerer {
    value: Option<Value>,
    asked: RefCell<Vec<(String, BTreeMap<String, Value>)>>,
    authority: Authority,
}

impl Answerer {
    fn giving(value: Option<bool>) -> Self {
        Self::giving_value(value.map(Value::Bool))
    }

    fn giving_value(value: Option<Value>) -> Self {
        Self {
            value,
            asked: RefCell::new(Vec::new()),
            authority: cell_authority(),
        }
    }

    /// The same answerer, as another authority.
    fn of(self, name: &str) -> Self {
        Self {
            authority: Authority {
                name: name.to_string(),
                category: None,
            },
            ..self
        }
    }
}

/// Who the answerer of these tests is: the holder of the head's policy, so
/// its binding asks it.
fn cell_authority() -> Authority {
    Authority {
        name: "Het hoofd van OBS De Klimop".to_string(),
        category: None,
    }
}

fn provenance() -> AnswerProvenance {
    AnswerProvenance {
        answerer: "een_cel".to_string(),
        answer: ONCE.to_string(),
        law_id: REDUCTION.to_string(),
        article: "1".to_string(),
        valid_from: "2026-08-01".to_string(),
        records_read: 3,
        last_record: Some("gram-3".to_string()),
    }
}

impl OpenTermAnswerer for Answerer {
    fn authority(&self) -> &Authority {
        &self.authority
    }

    fn answer(
        &self,
        _service: &LawExecutionService,
        request: &AnswerRequest<'_>,
    ) -> regelrecht_engine::Result<Option<Answer>> {
        if request.name != ONCE {
            return Ok(None);
        }
        self.asked
            .borrow_mut()
            .push((request.name.to_string(), request.parameters.clone()));
        Ok(self.value.clone().map(|value| Answer {
            value,
            provenance: provenance(),
        }))
    }
}

fn granted_13a_with(
    service: &LawExecutionService,
    parameters: BTreeMap<String, Value>,
    answerer: &Answerer,
) -> regelrecht_engine::Result<regelrecht_engine::ArticleResult> {
    let (parameters, findings) = split(parameters);
    let call = Call {
        findings,
        answerer: Some(answerer),
        chronicle: None,
        sphere: Vec::new(),
    };
    service.evaluate_law_with_trace_in(
        "leerplichtwet_1969",
        &["verlof_verleend"],
        parameters,
        call,
        "2026-10-27",
    )
}

/// What the report says filled the term: the head's binding, with the
/// answer of the cell.
fn bound() -> FilledBy {
    FilledBy::Answer {
        law_id: POLICY.to_string(),
        article: "2".to_string(),
        answers: vec![provenance()],
    }
}

/// The nodes of `node` and below where an answerer gave an answer.
fn answered(node: &regelrecht_engine::PathNode) -> Vec<&regelrecht_engine::PathNode> {
    let mut found: Vec<_> = node.children.iter().flat_map(answered).collect();
    if node.answer.is_some() {
        found.insert(0, node);
    }
    found
}

#[test]
fn the_answerer_fills_the_term_of_13a() {
    // The reduction is loaded, but the engine does not run it: the answer
    // is the answerer's, which the head's binding takes.
    let service = corpus_service(true);
    let mut parameters = application();
    // The head's own finding is asked only where the cell is silent.
    parameters.insert(ONCE_HEAD.to_string(), Value::Bool(false));
    let answerer = Answerer::giving(Some(true));
    let result = granted_13a_with(&service, parameters, &answerer).unwrap();
    assert_eq!(result.outputs["verlof_verleend"], Value::Bool(false));
    // Asked by name, with the parameters the binding passes.
    let asked = answerer.asked.borrow().clone();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].0, ONCE);
    assert_eq!(
        asked[0].1.keys().collect::<Vec<_>>(),
        ["bsn_jongere", "eerste_verlofdag"]
    );
    assert_eq!(
        asked[0].1["eerste_verlofdag"],
        Value::String("2027-05-10".into())
    );
    let filled = result
        .open_terms
        .iter()
        .find(|f| f.term == ONCE)
        .expect("the term is reported");
    assert_eq!(filled.filled_by, bound());
    let root = result.trace.unwrap();
    let nodes = answered(&root);
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].answer, Some(provenance()));
    assert!(nodes[0].children.is_empty());
    let trace = root.render_box_drawing();
    assert!(
        trace.contains(&format!(
            "Answered by answer '{ONCE}' of een_cel with {REDUCTION} article 1 (valid from \
             2026-08-01), over 3 records up to 'gram-3'"
        )),
        "{trace}"
    );
    assert!(
        trace.contains(&format!(
            "implemented by {POLICY} article 2, with the answer of een_cel"
        )),
        "{trace}"
    );
    assert!(!trace.contains(&format!("'{ONCE_HEAD}'")), "{trace}");
    assert!(!trace.contains("FOREACH"), "{trace}");
}

#[test]
fn a_finding_on_the_term_the_policy_fills_is_asked_by_none() {
    // No judgement of the head steps in on a term his policy fills: the
    // policy says itself where he judges (RFC-048 §2).
    let service = corpus_service(false);
    let mut parameters = application();
    parameters.insert(ONCE.to_string(), Value::Bool(false));
    let answerer = Answerer::giving(Some(true));
    let e = granted_13a_with(&service, parameters, &answerer).unwrap_err();
    assert!(e.to_string().contains("is asked by none"), "{e}");
}

#[test]
fn a_loaded_reduction_fills_nothing_by_itself() {
    // Without a cell the binding gets no answer; the engine does not run the
    // reduction, also with its decisions at hand.
    let mut service = corpus_service(true);
    supply(
        &mut service,
        json!([decision("999990019", "2027-02-01", true)]),
    );
    let e = granted_13a(&service, application()).unwrap_err();
    assert!(e.to_string().contains(ONCE), "{e}");
    let mut parameters = application();
    parameters.insert(ONCE_HEAD.to_string(), Value::Bool(false));
    let result = granted_13a(&service, parameters).unwrap();
    assert_eq!(result.outputs["verlof_verleend"], Value::Bool(true));
    assert!(result
        .open_terms
        .iter()
        .any(|f| f.term == ONCE_HEAD && f.filled_by == FilledBy::Finding));
}

#[test]
fn an_answerer_that_gives_no_answer_leaves_it_to_the_policy() {
    // The binding still fills the term, with what the policy says holds
    // without an answer: the head's own finding.
    let service = corpus_service(false);
    let mut parameters = application();
    parameters.insert(ONCE_HEAD.to_string(), Value::Bool(true));
    let answerer = Answerer::giving(None);
    let result = granted_13a_with(&service, parameters, &answerer).unwrap();
    assert_eq!(result.outputs["verlof_verleend"], Value::Bool(false));
    assert_eq!(answerer.asked.borrow().len(), 1);
    assert!(result
        .open_terms
        .iter()
        .any(|f| f.term == ONCE_HEAD && f.filled_by == FilledBy::Finding));
    // Without a finding either, the policy's required term has no value.
    let e = granted_13a_with(&service, application(), &answerer).unwrap_err();
    assert!(e.to_string().contains(ONCE), "{e}");
}

#[test]
fn a_silent_answerer_leaves_it_to_the_policy() {
    // A null answer is no value of the binding's input; the policy says
    // what then holds: the head's own finding.
    let service = corpus_service(false);
    let mut parameters = application();
    parameters.insert(ONCE_HEAD.to_string(), Value::Bool(true));
    let answerer = Answerer::giving_value(Some(Value::Null));
    let result = granted_13a_with(&service, parameters, &answerer).unwrap();
    assert_eq!(result.outputs["verlof_verleend"], Value::Bool(false));
    assert!(result
        .open_terms
        .iter()
        .any(|f| f.term == ONCE_HEAD && f.filled_by == FilledBy::Finding));
    let trace = result.trace.unwrap().render_box_drawing();
    assert!(trace.contains("is silent (null)"), "{trace}");
    assert!(!trace.contains("is silent for this case"), "{trace}");
}

#[test]
fn an_answer_needs_no_finding() {
    let service = corpus_service(false);
    let answerer = Answerer::giving(Some(false));
    let result = granted_13a_with(&service, application(), &answerer).unwrap();
    assert_eq!(result.outputs["verlof_verleend"], Value::Bool(true));
    let trace = result.trace.unwrap().render_box_drawing();
    assert!(trace.contains("with the answer of een_cel"), "{trace}");
}

/// An answerer whose own work fails.
struct Failing(Authority);

impl OpenTermAnswerer for Failing {
    fn authority(&self) -> &Authority {
        &self.0
    }

    fn answer(
        &self,
        _service: &LawExecutionService,
        _request: &AnswerRequest<'_>,
    ) -> regelrecht_engine::Result<Option<Answer>> {
        Err(EngineError::ResolutionError(
            "the chronicle is damaged".to_string(),
        ))
    }
}

#[test]
fn an_answerer_that_fails_ends_the_execution() {
    let service = corpus_service(false);
    let (parameters, findings) = split(application());
    let call = Call {
        findings,
        answerer: Some(&Failing(cell_authority())),
        chronicle: None,
        sphere: Vec::new(),
    };
    let e = service
        .evaluate_law_in(
            "leerplichtwet_1969",
            &["verlof_verleend"],
            parameters,
            call,
            "2026-10-27",
        )
        .unwrap_err();
    assert!(e.to_string().contains("the chronicle is damaged"), "{e}");
}

#[test]
fn an_answerer_the_policy_does_not_name_is_not_asked() {
    // The binding asks the cell of the policy's holder (RFC-048); another
    // cell of the call is not asked, also when it has the answer.
    let service = corpus_service(false);
    let mut parameters = application();
    parameters.insert(ONCE_HEAD.to_string(), Value::Bool(true));
    let answerer = Answerer::giving(Some(false)).of("Het hoofd van OBS Het Anker");
    let result = granted_13a_with(&service, parameters, &answerer).unwrap();
    assert!(answerer.asked.borrow().is_empty());
    assert_eq!(result.outputs["verlof_verleend"], Value::Bool(false));
    let trace = result.trace.unwrap().render_box_drawing();
    assert!(
        trace.contains(&format!(
            "No cell of this execution gives the answer '{ONCE}' for 'Het hoofd van OBS De Klimop'"
        )),
        "{trace}"
    );
}

#[test]
fn without_an_answerer_the_term_is_the_heads_finding_or_an_error() {
    let service = corpus_service(false);
    let mut parameters = application();
    parameters.insert(ONCE_HEAD.to_string(), Value::Bool(true));
    let result = granted_13a(&service, parameters).unwrap();
    assert_eq!(result.outputs["verlof_verleend"], Value::Bool(false));
    assert!(result
        .open_terms
        .iter()
        .any(|f| f.term == ONCE_HEAD && matches!(f.filled_by, FilledBy::Finding)));
    // No default: without the head's finding there is no answer, and no
    // second leave granted by silence.
    let e = granted_13a(&service, application()).unwrap_err();
    assert!(e.to_string().contains(ONCE), "{e}");
}

/// The refused decision B (leave in May), as a decision value from the
/// chronicle: the reduction's answer is no finding of the head, so it is
/// not in it.
fn refusal_b() -> serde_json::Value {
    json!({
        "basis": "leerplichtwet_1969#13a",
        "taken_on": "leerplichtwet_1969#13a",
        "fields": {
            "bsn_jongere": "999990019",
            "eerste_verlofdag": "2027-05-10",
            "laatste_verlofdag": "2027-05-14",
        },
        "findings": {
            "specifieke_aard_beroep_ouder": true,
            "verlof_valt_buiten_schoolvakanties_volgens_hoofd": true,
        },
        "outputs": {
            "verlofdagen": 5,
            "verlof_raakt_eerste_twee_lesweken": false,
            "verlof_verleend": false,
        },
    })
}

fn revoked_on_objection_with(
    service: &LawExecutionService,
    answerer: &Answerer,
) -> regelrecht_engine::Result<regelrecht_engine::ArticleResult> {
    let parameters = BTreeMap::from([
        ("bestreden_besluit".to_string(), Value::from(refusal_b())),
        (
            "bezwaar_of_beroepschrift_tijdig_ingediend".to_string(),
            Value::Bool(true),
        ),
    ]);
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(answerer),
        chronicle: None,
        sphere: Vec::new(),
    };
    service.evaluate_law_with_trace_in(
        AWB,
        &["bestreden_besluit_herroepen"],
        parameters,
        call,
        "2026-12-01",
    )
}

#[test]
fn in_the_call_anew_the_engine_asks_the_answerer() {
    let service = corpus_service(false);
    // A grant still counts: art. 13a called anew refuses again, no cause.
    let answerer = Answerer::giving(Some(true));
    let result = revoked_on_objection_with(&service, &answerer).unwrap();
    assert_eq!(
        result.outputs["bestreden_besluit_herroepen"],
        Value::Bool(false)
    );
    // The binding of 13a within 7:11 asks it, with the parameters of the
    // decision called anew.
    let asked = answerer.asked.borrow();
    assert_eq!(asked[0].0, ONCE);
    assert_eq!(asked[0].1["bsn_jongere"], Value::String("999990019".into()));
    drop(asked);

    // No grant counts any more: called anew, art. 13a grants B.
    let answerer = Answerer::giving(Some(false));
    let result = revoked_on_objection_with(&service, &answerer).unwrap();
    assert_eq!(
        result.outputs["bestreden_besluit_herroepen"],
        Value::Bool(true)
    );
    assert!(result
        .open_terms
        .iter()
        .any(|f| f.term == ONCE && f.filled_by == bound()));
    let root = result.trace.unwrap();
    assert_eq!(answered(&root).len(), 1, "{}", root.render_box_drawing());
}

#[test]
fn in_the_call_anew_without_an_answerer_the_finding_is_missing() {
    // Outside a cell nothing fills the term, and the decision value holds
    // no finding on it: the call anew fails on it, it does not guess.
    let service = corpus_service(true);
    let parameters = BTreeMap::from([
        ("bestreden_besluit".to_string(), Value::from(refusal_b())),
        (
            "bezwaar_of_beroepschrift_tijdig_ingediend".to_string(),
            Value::Bool(true),
        ),
    ]);
    let e = service
        .evaluate_law_output(AWB, "bestreden_besluit_herroepen", parameters, "2026-12-01")
        .unwrap_err();
    assert!(e.to_string().contains(ONCE), "{e}");
}

// ---------------------------------------------------------------------------
// The Awb reduction: which decisions still stand after objection and appeal
// (Awb 7:11, 8:72), and the decisions the court set in place (8:72 lid 3
// onder b). It is of no cell, so it is no answer of a cell; another
// reduction calls it and passes the rows it read.
// ---------------------------------------------------------------------------

const AWB_REDUCTION: &str = "reductie_algemene_wet_bestuursrecht";
/// The article of the decisions on leave.
const LEAVE: &str = "leerplichtwet_1969#13a";

/// The Awb reduction from the corpus.
fn awb_reduction_yaml() -> String {
    corpus(&format!("cells/reductions/{AWB_REDUCTION}/2026-01-01.yaml"))
}

fn awb_service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(&awb_reduction_yaml()).unwrap();
    service
}

/// The receipt of a judgment about the decision `about`, that annulled it
/// with its legal effects, and set `in_place` in its place.
fn judgment(about: &str, in_place: serde_json::Value) -> serde_json::Value {
    judgment_keeping(about, false, in_place)
}

/// [`judgment`], where the court kept the legal effects of the annulled
/// decision or not (Awb 8:72 lid 3 onder a). The decision set in place is
/// a row with the id of the receipt, as is a decision it holds in turn.
fn judgment_keeping(about: &str, kept: bool, in_place: serde_json::Value) -> serde_json::Value {
    let id = format!("uitspraak-{about}");
    json!({
        "id": id,
        "basis": "algemene_wet_bestuursrecht#8:79",
        "refers_to": about,
        "bestreden_besluit_vernietigd": true,
        "rechtsgevolgen_vernietigd": !kept,
        "uitspraak_in_de_plaats": with_id(in_place, &id),
    })
}

/// `row`, and a row it holds, with the id `id`.
fn with_id(mut row: serde_json::Value, id: &str) -> serde_json::Value {
    if let Some(object) = row.as_object_mut() {
        object.insert("id".into(), json!(id));
        for value in object.values_mut() {
            if value.get("basis").is_some() {
                *value = with_id(value.clone(), id);
            }
        }
    }
    row
}

/// The row of a decision value under `basis`, as a judgment holds it:
/// the fields and the outputs flat, and the id of the receipt that holds it
/// ([`judgment_keeping`]).
fn value_under(basis: &str, outputs: serde_json::Value) -> serde_json::Value {
    let mut row = json!({
        "basis": basis,
        "bsn_jongere": "999990019",
    });
    for (name, value) in outputs.as_object().unwrap() {
        row[name] = value.clone();
    }
    row
}

/// The output `output` of article 1 of the Awb reduction on these rows.
fn awb_output(
    decisions: serde_json::Value,
    on_objection: serde_json::Value,
    judgments: serde_json::Value,
    output: &str,
) -> serde_json::Value {
    let parameters = BTreeMap::from([
        ("basis".to_string(), Value::String(LEAVE.into())),
        ("besluiten".to_string(), Value::from(decisions)),
        (
            "beslissingen_op_bezwaar".to_string(),
            Value::from(on_objection),
        ),
        ("uitspraken".to_string(), Value::from(judgments)),
    ]);
    let result = awb_service()
        .evaluate_law_output(AWB_REDUCTION, output, parameters, "2027-03-01")
        .unwrap();
    serde_json::to_value(&result.outputs[output]).unwrap()
}

/// The ids of the decisions that still stand.
fn standing(
    decisions: serde_json::Value,
    on_objection: serde_json::Value,
    judgments: serde_json::Value,
) -> Vec<String> {
    awb_output(decisions, on_objection, judgments, "besluiten_in_stand")
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn the_awb_reduction_drops_a_revoked_decision_and_keeps_the_one_in_its_place() {
    let a = decision("999990019", "2026-10-05", false);
    let b = decision("999990019", "2026-10-05", true);
    let rows = standing(
        json!([a, b]),
        json!([revoked_for("x", &a, Some(&b))]),
        json!([]),
    );
    assert_eq!(rows, [b["id"].as_str().unwrap()]);
    // The row comes back as it was given.
    let full = awb_output(json!([b]), json!([]), json!([]), "besluiten_in_stand");
    assert_eq!(full, json!([b]));
}

#[test]
fn an_annulled_decision_on_objection_revives_the_contested_decision() {
    let a = decision("999990019", "2026-10-05", false);
    let b = decision("999990019", "2026-10-05", true);
    let on_objection = json!([revoked_for("x", &a, Some(&b))]);
    // Annulled with its legal effects: its revocation no longer holds, nor
    // the decision it took in place.
    let rows = standing(
        json!([a, b]),
        on_objection.clone(),
        json!([judgment("x", json!(null))]),
    );
    assert_eq!(rows, [a["id"].as_str().unwrap()]);
    // Annulled with the legal effects left in place (8:72 lid 3 onder a):
    // the revocation holds.
    let rows = standing(
        json!([a, b]),
        on_objection,
        json!([judgment_keeping("x", true, json!(null))]),
    );
    assert_eq!(rows, [b["id"].as_str().unwrap()]);
}

#[test]
fn an_annulled_decision_drops_out() {
    let a = decision("999990019", "2026-10-05", true);
    let rows = standing(
        json!([a]),
        json!([]),
        json!([judgment(a["id"].as_str().unwrap(), json!(null))]),
    );
    assert!(rows.is_empty(), "{rows:?}");
}

#[test]
fn a_revocation_in_the_decision_the_court_set_in_place_holds() {
    let a = decision("999990019", "2026-10-05", false);
    let grant = value_under("leerplichtwet_1969#13a", json!({ "verlof_verleend": true }));
    let in_place = value_under(
        "algemene_wet_bestuursrecht#7:11",
        json!({ "bestreden_besluit_herroepen": true, "nieuw_besluit": grant }),
    );
    let rows = json!([a]);
    let on_objection = json!([revoked_for("x", &a, None)]);
    let judgments = json!([judgment("x", in_place)]);
    assert_eq!(
        awb_output(
            rows.clone(),
            on_objection.clone(),
            judgments.clone(),
            "beslissingen_herroepen_bij_uitspraak"
        ),
        json!(["x"])
    );
    assert_eq!(
        awb_output(
            rows.clone(),
            on_objection.clone(),
            judgments.clone(),
            "herroepen_besluiten"
        ),
        json!([a["id"]])
    );
    assert!(standing(rows, on_objection, judgments).is_empty());
}

#[test]
fn a_decision_on_objection_the_court_set_in_place_that_upholds_revokes_nothing() {
    // The decision on objection revoked `a` and took `b` in its place; the
    // court annulled it and set in place one that upholds `a`.
    let a = decision("999990019", "2026-10-05", false);
    let b = decision("999990019", "2026-10-05", true);
    let upheld = value_under(
        "algemene_wet_bestuursrecht#7:11",
        json!({ "bestreden_besluit_herroepen": false, "nieuw_besluit": null }),
    );
    let rows = json!([a, b]);
    let on_objection = json!([revoked_for("x", &a, Some(&b))]);
    let judgments = json!([judgment("x", upheld)]);
    assert_eq!(
        awb_output(
            rows.clone(),
            on_objection.clone(),
            judgments.clone(),
            "beslissingen_herroepen_bij_uitspraak"
        ),
        json!([])
    );
    assert_eq!(
        awb_output(
            rows.clone(),
            on_objection.clone(),
            judgments.clone(),
            "herroepen_besluiten"
        ),
        json!([])
    );
    assert_eq!(
        standing(rows, on_objection, judgments),
        [a["id"].as_str().unwrap()]
    );
}

/// Article 2 of the Awb reduction on `judgments`, for `basis`.
fn in_place(judgments: serde_json::Value, basis: &str) -> serde_json::Value {
    let parameters = BTreeMap::from([
        ("uitspraken".to_string(), Value::from(judgments)),
        ("basis".to_string(), Value::String(basis.into())),
    ]);
    let result = awb_service()
        .evaluate_law_output(
            AWB_REDUCTION,
            "besluiten_in_de_plaats",
            parameters,
            "2027-03-01",
        )
        .unwrap();
    serde_json::to_value(&result.outputs["besluiten_in_de_plaats"]).unwrap()
}

#[test]
fn the_decisions_the_court_set_in_place_come_from_either_path() {
    let direct = value_under(LEAVE, json!({ "verlof_verleend": true }));
    let anew = value_under(LEAVE, json!({ "verlof_verleend": false }));
    let on_objection = value_under(
        "algemene_wet_bestuursrecht#7:11",
        json!({ "bestreden_besluit_herroepen": true, "nieuw_besluit": anew }),
    );
    let upheld = value_under(
        "algemene_wet_bestuursrecht#7:11",
        json!({ "bestreden_besluit_herroepen": false, "nieuw_besluit": null }),
    );
    let other = value_under("een_andere_wet#1", json!({ "toegekend": true }));
    let judgments = json!([
        judgment("a", direct.clone()),
        judgment("b", on_objection),
        judgment("c", upheld),
        judgment("d", other.clone()),
        judgment_keeping("e", true, json!(null)),
    ]);
    assert_eq!(
        in_place(judgments.clone(), LEAVE),
        json!([with_id(direct, "uitspraak-a"), with_id(anew, "uitspraak-b")])
    );
    assert_eq!(
        in_place(judgments.clone(), "een_andere_wet#1"),
        json!([with_id(other, "uitspraak-d")])
    );
    assert_eq!(in_place(judgments, "nog_een_wet#1"), json!([]));
}

#[test]
fn a_decision_the_court_set_in_place_under_another_article_revokes_nothing() {
    // The court decided on the contested decision itself, not on a decision
    // on objection: that value has no bestreden_besluit_herroepen, and the
    // reduction must not read it.
    let a = decision("999990019", "2026-10-05", true);
    let id = a["id"].as_str().unwrap();
    let in_place = value_under(
        "leerplichtwet_1969#13a",
        json!({ "verlof_verleend": false }),
    );
    let judgments = json!([judgment(id, in_place)]);
    assert_eq!(
        awb_output(
            json!([a]),
            json!([]),
            judgments.clone(),
            "beslissingen_herroepen_bij_uitspraak"
        ),
        json!([])
    );
    // The decision itself is annulled, so it no longer stands.
    assert!(standing(json!([a]), json!([]), judgments).is_empty());
}

// ---------------------------------------------------------------------------
// The chronicle of the call (RFC-048): the cell that runs a reduction gives
// the rows every reduction in the execution reads, also one it calls, such
// as that of the Awb, which is of no cell and reads from the chronicle of
// the executing cell.
// ---------------------------------------------------------------------------

/// A chronicle that gives fixed rows per read, and keeps what it was asked.
struct Kroniek {
    decisions: BTreeMap<&'static str, serde_json::Value>,
    judgments: serde_json::Value,
    asked: RefCell<Vec<(String, String)>>,
}

impl Kroniek {
    fn with(decisions: serde_json::Value, on_objection: serde_json::Value) -> Self {
        Self {
            decisions: BTreeMap::from([
                (LEAVE, decisions),
                ("algemene_wet_bestuursrecht#7:11", on_objection),
            ]),
            judgments: json!([]),
            asked: RefCell::new(Vec::new()),
        }
    }
}

impl regelrecht_engine::Chronicle for Kroniek {
    fn keeper(&self) -> &str {
        "obs_de_klimop"
    }

    fn rows(
        &self,
        _service: &LawExecutionService,
        law: &ArticleBasedLaw,
        read: ChronicleRead<'_>,
        _calculation_date: &str,
    ) -> regelrecht_engine::Result<Option<Value>> {
        self.asked
            .borrow_mut()
            .push((law.id.clone(), read.to_string()));
        Ok(match read {
            ChronicleRead::Decisions(basis) => self.decisions.get(basis).cloned(),
            ChronicleRead::Facts(_) => Some(self.judgments.clone()),
        }
        .map(Value::from))
    }
}

/// Whether the head already granted `bsn` leave in the school year of
/// `first`, with `chronicle` as the chronicle of the call.
fn already_in(chronicle: &Kroniek, bsn: &str, first: &str) -> regelrecht_engine::Result<Value> {
    let parameters = BTreeMap::from([
        ("bsn_jongere".to_string(), Value::String(bsn.into())),
        ("eerste_verlofdag".to_string(), Value::String(first.into())),
    ]);
    let call = Call {
        chronicle: Some(chronicle),
        ..Call::default()
    };
    service()
        .evaluate_law_with_trace_in(REDUCTION, &[ONCE], parameters, call, "2027-03-01")
        .map(|r| r.outputs[ONCE].clone())
}

#[test]
fn a_called_reduction_reads_its_rows_from_the_call_s_chronicle() {
    let revoked = decision("999990019", "2026-10-05", true);
    let chronicle = Kroniek::with(json!([revoked]), json!([on_objection(&revoked, true)]));
    assert_eq!(
        already_in(&chronicle, "999990019", "2027-05-10").unwrap(),
        Value::Bool(false)
    );
    // The school's reduction reads nothing itself; the reduction of the Awb
    // it calls reads the decisions, the decisions on objection and the
    // judgments (both its articles read those).
    let asked = chronicle.asked.borrow();
    assert!(
        asked.iter().all(|(law, _)| law == AWB_REDUCTION),
        "{asked:?}"
    );
    let reads: std::collections::BTreeSet<&str> = asked.iter().map(|(_, r)| r.as_str()).collect();
    assert_eq!(reads.len(), 3, "{asked:?}");
}

#[test]
fn decisions_named_by_a_parameter_are_read_under_its_value() {
    let granted = decision("999990019", "2026-10-05", true);
    let chronicle = Kroniek::with(json!([granted]), json!([]));
    assert_eq!(
        already_in(&chronicle, "999990019", "2027-05-10").unwrap(),
        Value::Bool(true)
    );
    // `decisions: $basis` is read under the article the school passes.
    assert!(chronicle.asked.borrow().contains(&(
        AWB_REDUCTION.to_string(),
        format!("the decisions taken under {LEAVE}")
    )));
}

#[test]
fn leave_the_court_granted_in_place_counts() {
    // The court annulled a refusal and granted leave itself (Awb 8:72 lid
    // 3 onder b): the grant it set in place is a row like any other.
    let refusal = decision("999990019", "2026-10-05", false);
    let mut chronicle = Kroniek::with(json!([refusal]), json!([]));
    let mut grant = value_under(LEAVE, json!({ "verlof_verleend": true }));
    grant["eerste_verlofdag"] = json!("2026-10-05");
    chronicle.judgments = json!([judgment(refusal["id"].as_str().unwrap(), grant)]);
    assert_eq!(
        already_in(&chronicle, "999990019", "2027-05-10").unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn without_a_chronicle_a_called_reduction_errs_not_empty() {
    // No data source and no chronicle: the reduction of the Awb does not
    // read its decisions as none.
    let e = evaluate(&service(), "999990019", "2027-05-10").unwrap_err();
    let message = e.to_string();
    assert!(matches!(e, EngineError::DataSourceError(_)), "{message}");
    assert!(message.contains(AWB_REDUCTION), "{message}");
    // A chronicle that does not answer a read is no empty list either.
    let silent = Kroniek {
        decisions: BTreeMap::new(),
        judgments: json!([]),
        asked: RefCell::new(Vec::new()),
    };
    let e = already_in(&silent, "999990019", "2027-05-10").unwrap_err();
    let message = e.to_string();
    assert!(
        message.contains("the decisions taken under")
            && message.contains("no data source supplies them"),
        "{message}"
    );
}

#[test]
fn only_a_reductie_calls_a_reductie() {
    // The head's policy calls the reduction of the Awb for its decisions:
    // refused, since a reduction reads the chronicle of the cell that runs
    // it, and a policy is run by no cell.
    let policy = r#"
$id: beleid_dat_een_reductie_aanroept
regulatory_layer: UITVOERINGSBELEID
publication_date: '2026-01-01'
valid_from: '2026-01-01'
url: https://example.org/beleid
articles:
  - number: '1'
    text: Fictief.
    url: https://example.org/beleid#1
    machine_readable:
      execution:
        input:
          - name: geldende_besluiten
            type: array
            source:
              regulation: reductie_algemene_wet_bestuursrecht
              output: geldende_besluiten
              parameters:
                basis: leerplichtwet_1969#13a
        output:
          - name: besluiten
            type: array
        actions:
          - output: besluiten
            value: $geldende_besluiten
"#;
    let mut service = service();
    service.load_law(policy).unwrap();
    let chronicle = Kroniek::with(json!([]), json!([]));
    let call = Call {
        chronicle: Some(&chronicle),
        ..Call::default()
    };
    let e = service
        .evaluate_law_in(
            "beleid_dat_een_reductie_aanroept",
            &["besluiten"],
            BTreeMap::new(),
            call,
            "2027-03-01",
        )
        .unwrap_err();
    assert!(
        e.to_string().contains("only a REDUCTIE calls a REDUCTIE"),
        "{e}"
    );
    assert!(chronicle.asked.borrow().is_empty());
}

#[test]
fn the_decisions_that_hold_are_those_in_force_and_those_set_in_place() {
    let a = decision("999990019", "2026-10-05", true);
    let b = decision("999990019", "2026-11-05", false);
    let grant = value_under(LEAVE, json!({ "verlof_verleend": true }));
    let judgments = json!([judgment(b["id"].as_str().unwrap(), grant.clone())]);
    let parameters = BTreeMap::from([
        ("basis".to_string(), Value::String(LEAVE.into())),
        ("besluiten".to_string(), Value::from(json!([a, b]))),
        (
            "beslissingen_op_bezwaar".to_string(),
            Value::from(json!([])),
        ),
        ("uitspraken".to_string(), Value::from(judgments)),
    ]);
    let result = awb_service()
        .evaluate_law_output(
            AWB_REDUCTION,
            "geldende_besluiten",
            parameters,
            "2027-03-01",
        )
        .unwrap();
    let id = format!("uitspraak-{}", b["id"].as_str().unwrap());
    assert_eq!(
        serde_json::to_value(&result.outputs["geldende_besluiten"]).unwrap(),
        json!([a, with_id(grant, &id)])
    );
}

#[test]
fn a_parameter_that_names_no_article_is_an_error_not_a_read() {
    // `decisions: $basis` reads only under an article reference.
    let chronicle = Kroniek::with(json!([]), json!([]));
    for not_an_article in [json!("leerplichtwet_1969"), json!(7)] {
        let parameters = BTreeMap::from([
            ("basis".to_string(), Value::from(not_an_article.clone())),
            (
                "beslissingen_op_bezwaar".to_string(),
                Value::from(json!([])),
            ),
            ("uitspraken".to_string(), Value::from(json!([]))),
        ]);
        let call = Call {
            chronicle: Some(&chronicle),
            ..Call::default()
        };
        let e = awb_service()
            .evaluate_law_in(
                AWB_REDUCTION,
                &["besluiten_in_stand"],
                parameters,
                call,
                "2027-03-01",
            )
            .unwrap_err();
        assert!(
            matches!(&e, EngineError::TypeMismatch { expected, .. }
                if expected.contains("an article reference in $basis")),
            "{not_an_article}: {e}"
        );
    }
    assert!(chronicle
        .asked
        .borrow()
        .iter()
        .all(|(_, read)| !read.contains("leerplichtwet_1969")));
}
