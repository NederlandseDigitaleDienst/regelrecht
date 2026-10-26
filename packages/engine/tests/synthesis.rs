//! Synthesis (RFC-052) at the consumer (RFC-048): a value that another
//! actor establishes is read with `source.regulation` from the article that
//! establishes it, and the policy of the authority that decides binds its
//! open term to a named answer (`source.reduction`). Where the term names
//! its own `competent_authority`, the engine asks every answerer of the
//! call whose authority the term names for that answer, and none other, and
//! makes one value of their answers with the term's `combine`, or else the
//! binding's. Where the term names no one, it asks the policy's holder. An
//! answerer without an answer of that name is silent. The article's
//! `competent_authority` says who decides (RFC-002) and routes nothing.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::cell::RefCell;
use std::collections::BTreeMap;

use regelrecht_engine::{
    Answer, AnswerProvenance, AnswerRequest, ArticleBasedLaw, Authority, Call, FilledBy,
    LawExecutionService, LawLoad, OpenTermAnswerer, PathNode, Value,
};

const DATE: &str = "2026-10-20";

/// The law that establishes the facts, with the competence on each term:
/// who issued a card before (art. 1, a college or the fictitious national
/// issuer), a fact without competence (art. 2), one of an article whose
/// decider is a reference (art. 3), a mark one college gives (art. 4), a
/// fact one college gives without `combine` (art. 5), and one the GGD
/// establishes for an article the college decides (art. 7).
const ESTABLISHING: &str = r#"
$id: wet_verstrekking
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het college of de landelijke uitgever verstrekt de kaart.
    machine_readable:
      open_terms:
        - id: eerder_verstrekt
          competent_authority:
            - name: college van burgemeester en wethouders
              type: CATEGORY
            - name: Landelijke uitgever
          type: boolean
          required: true
          decided_per_case_by: het college
          combine: OR
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        output:
          - name: eerder_verstrekt
            type: boolean
        actions:
          - output: eerder_verstrekt
            value: $eerder_verstrekt
  - number: '2'
    text: Een feit zonder bevoegde.
    machine_readable:
      open_terms:
        - id: ander_feit
          type: boolean
          required: true
          decided_per_case_by: het college
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        output:
          - name: ander_feit
            type: boolean
        actions:
          - output: ander_feit
            value: $ander_feit
  - number: '3'
    text: Een feit van een bevoegde die een uitvoer aanwijst.
    machine_readable:
      competent_authority: '#uitgever'
      open_terms:
        - id: verwezen_feit
          type: boolean
          required: true
          decided_per_case_by: het college
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        output:
          - name: verwezen_feit
            type: boolean
        actions:
          - output: verwezen_feit
            value: $verwezen_feit
  - number: '4'
    text: Het kenmerk dat één college geeft.
    machine_readable:
      open_terms:
        - id: kenmerk
          competent_authority:
            name: college van burgemeester en wethouders
            type: CATEGORY
          type: string
          required: false
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        output:
          - name: kenmerk
            type: string
            nullable: true
        actions:
          - output: kenmerk
            value: $kenmerk
  - number: '5'
    text: Een feit dat één college geeft.
    machine_readable:
      open_terms:
        - id: enig_feit
          competent_authority:
            name: college van burgemeester en wethouders
            type: CATEGORY
          type: boolean
          required: true
          decided_per_case_by: het college
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        output:
          - name: enig_feit
            type: boolean
        actions:
          - output: enig_feit
            value: $enig_feit
  - number: '6'
    text: Een feit dat een ander artikel van deze wet geeft.
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: zevende
            type: boolean
            source:
              output: zevende_feit
        output:
          - name: zesde_feit
            type: boolean
        actions:
          - output: zesde_feit
            value: $zevende
  - number: '7'
    text: Het college beslist op een feit dat de GGD vaststelt.
    machine_readable:
      competent_authority:
        name: college van burgemeester en wethouders
        type: CATEGORY
      open_terms:
        - id: zevende_feit
          competent_authority:
            name: Gemeentelijke Gezondheidsdienst
            type: CATEGORY
          type: boolean
          required: true
          decided_per_case_by: het college
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        output:
          - name: zevende_feit
            type: boolean
        actions:
          - output: zevende_feit
            value: $zevende_feit
  - number: '8'
    text: Het aantal kaarten dat elk college telt.
    machine_readable:
      open_terms:
        - id: aantal_kaarten
          competent_authority:
            - name: college van burgemeester en wethouders
              type: CATEGORY
          type: number
          required: false
          combine: ADD
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        output:
          - name: aantal_kaarten
            type: number
            nullable: true
        actions:
          - output: aantal_kaarten
            value: $aantal_kaarten
"#;

/// The law that reads those facts, each through `source.regulation`.
const READING: &str = r#"
$id: wet_lezer
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Wie eerder een kaart kreeg.
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: eerder
            type: boolean
            source:
              regulation: wet_verstrekking
              output: eerder_verstrekt
              parameters:
                bsn: $bsn
          - name: ander
            type: boolean
            source:
              regulation: wet_verstrekking
              output: ander_feit
              parameters:
                bsn: $bsn
          - name: verwezen
            type: boolean
            source:
              regulation: wet_verstrekking
              output: verwezen_feit
              parameters:
                bsn: $bsn
          - name: gelezen_kenmerk
            type: string
            nullable: true
            source:
              regulation: wet_verstrekking
              output: kenmerk
              parameters:
                bsn: $bsn
          - name: enig
            type: boolean
            source:
              regulation: wet_verstrekking
              output: enig_feit
              parameters:
                bsn: $bsn
          - name: zesde
            type: boolean
            source:
              regulation: wet_verstrekking
              output: zesde_feit
              parameters:
                bsn: $bsn
        output:
          - name: eerder_gelezen
            type: boolean
          - name: ander_gelezen
            type: boolean
          - name: verwezen_gelezen
            type: boolean
          - name: kenmerk_gelezen
            type: string
            nullable: true
          - name: enig_gelezen
            type: boolean
          - name: zesde_gelezen
            type: boolean
        actions:
          - output: eerder_gelezen
            value: $eerder
          - output: ander_gelezen
            value: $ander
          - output: verwezen_gelezen
            value: $verwezen
          - output: kenmerk_gelezen
            value: $gelezen_kenmerk
          - output: enig_gelezen
            value: $enig
          - output: zesde_gelezen
            value: $zesde
"#;

/// The policy of the college of Wilgendaal that binds each term of the
/// establishing law to the answer of that name (RFC-048). Where a term
/// names who is competent, the engine asks those; where it names no one,
/// the policy's holder.
const BINDING: &str = r#"
$id: beleid_verstrekking
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
url: https://example.org/beleid
competent_authority:
  name: wilgendaal
articles:
  - number: '1'
    text: Wat het beleid ontleent aan het antwoord eerder_verstrekt.
    machine_readable:
      open_terms:
        - id: eerder_verstrekt_volgens_college
          type: boolean
          required: true
          decided_per_case_by: het college
      implements:
        - {law: wet_verstrekking, article: '1', open_term: eerder_verstrekt}
      execution:
        parameters:
          - {name: bsn, type: string}
        input:
          - name: antwoord_eerder_verstrekt
            type: boolean
            nullable: true
            source:
              reduction: eerder_verstrekt
              parameters: {bsn: $bsn}
        output:
          - {name: eerder_verstrekt, type: boolean}
        actions:
          - output: eerder_verstrekt
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $antwoord_eerder_verstrekt, value: null}
                  then: $eerder_verstrekt_volgens_college
              default: $antwoord_eerder_verstrekt
  - number: '2'
    text: Wat het beleid ontleent aan het antwoord ander_feit.
    machine_readable:
      open_terms:
        - id: ander_feit_volgens_college
          type: boolean
          required: true
          decided_per_case_by: het college
      implements:
        - {law: wet_verstrekking, article: '2', open_term: ander_feit}
      execution:
        parameters:
          - {name: bsn, type: string}
        input:
          - name: antwoord_ander_feit
            type: boolean
            nullable: true
            source:
              reduction: ander_feit
              parameters: {bsn: $bsn}
        output:
          - {name: ander_feit, type: boolean}
        actions:
          - output: ander_feit
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $antwoord_ander_feit, value: null}
                  then: $ander_feit_volgens_college
              default: $antwoord_ander_feit
  - number: '3'
    text: Wat het beleid ontleent aan het antwoord verwezen_feit.
    machine_readable:
      open_terms:
        - id: verwezen_feit_volgens_college
          type: boolean
          required: true
          decided_per_case_by: het college
      implements:
        - {law: wet_verstrekking, article: '3', open_term: verwezen_feit}
      execution:
        parameters:
          - {name: bsn, type: string}
        input:
          - name: antwoord_verwezen_feit
            type: boolean
            nullable: true
            source:
              reduction: verwezen_feit
              parameters: {bsn: $bsn}
        output:
          - {name: verwezen_feit, type: boolean}
        actions:
          - output: verwezen_feit
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $antwoord_verwezen_feit, value: null}
                  then: $verwezen_feit_volgens_college
              default: $antwoord_verwezen_feit
  - number: '4'
    text: Wat het beleid ontleent aan het antwoord kenmerk.
    machine_readable:
      implements:
        - {law: wet_verstrekking, article: '4', open_term: kenmerk}
      execution:
        parameters:
          - {name: bsn, type: string}
        input:
          - name: antwoord_kenmerk
            type: string
            nullable: true
            source:
              reduction: kenmerk
              parameters: {bsn: $bsn}
        output:
          - {name: kenmerk, type: string, nullable: true}
        actions:
          - {output: kenmerk, value: $antwoord_kenmerk}
  - number: '5'
    text: Wat het beleid ontleent aan het antwoord enig_feit.
    machine_readable:
      open_terms:
        - id: enig_feit_volgens_college
          type: boolean
          required: true
          decided_per_case_by: het college
      implements:
        - {law: wet_verstrekking, article: '5', open_term: enig_feit}
      execution:
        parameters:
          - {name: bsn, type: string}
        input:
          - name: antwoord_enig_feit
            type: boolean
            nullable: true
            source:
              reduction: enig_feit
              parameters: {bsn: $bsn}
        output:
          - {name: enig_feit, type: boolean}
        actions:
          - output: enig_feit
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $antwoord_enig_feit, value: null}
                  then: $enig_feit_volgens_college
              default: $antwoord_enig_feit
  - number: '7'
    text: Wat het beleid ontleent aan het antwoord zevende_feit.
    machine_readable:
      open_terms:
        - id: zevende_feit_volgens_college
          type: boolean
          required: true
          decided_per_case_by: het college
      implements:
        - {law: wet_verstrekking, article: '7', open_term: zevende_feit}
      execution:
        parameters:
          - {name: bsn, type: string}
        input:
          - name: antwoord_zevende_feit
            type: boolean
            nullable: true
            source:
              reduction: zevende_feit
              parameters: {bsn: $bsn}
        output:
          - {name: zevende_feit, type: boolean}
        actions:
          - output: zevende_feit
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $antwoord_zevende_feit, value: null}
                  then: $zevende_feit_volgens_college
              default: $antwoord_zevende_feit
  - number: '8'
    text: Wat het beleid ontleent aan het antwoord aantal_kaarten.
    machine_readable:
      implements:
        - {law: wet_verstrekking, article: '8', open_term: aantal_kaarten}
      execution:
        parameters:
          - {name: bsn, type: string}
        input:
          - name: antwoord_aantal_kaarten
            type: number
            nullable: true
            source:
              reduction: aantal_kaarten
              parameters: {bsn: $bsn}
        output:
          - {name: aantal_kaarten, type: number, nullable: true}
        actions:
          - {output: aantal_kaarten, value: $antwoord_aantal_kaarten}
"#;

fn service() -> LawExecutionService {
    let mut service = LawExecutionService::new();
    service.load_law(ESTABLISHING).unwrap();
    service.load_law(READING).unwrap();
    service.load_law(BINDING).unwrap();
    service
}

/// An answerer with a name, maybe a category, and an answer per name: a
/// value, `null`, or no answer at all (an answer it does not give). It
/// keeps which answers it was asked, with the parameters.
struct Fake {
    authority: Authority,
    answers: BTreeMap<&'static str, Value>,
    asked: RefCell<Vec<(String, BTreeMap<String, Value>)>>,
}

impl Fake {
    fn college(name: &str) -> Self {
        Self::new(name, Some("college van burgemeester en wethouders"))
    }

    fn new(name: &str, category: Option<&str>) -> Self {
        Self {
            authority: Authority {
                name: name.to_string(),
                category: category.map(str::to_string),
            },
            answers: BTreeMap::new(),
            asked: RefCell::new(Vec::new()),
        }
    }

    fn answering(mut self, name: &'static str, value: Value) -> Self {
        self.answers.insert(name, value);
        self
    }

    fn asked(&self) -> Vec<String> {
        self.asked
            .borrow()
            .iter()
            .map(|(name, _)| name.clone())
            .collect()
    }
}

impl OpenTermAnswerer for Fake {
    fn authority(&self) -> &Authority {
        &self.authority
    }

    fn answer(
        &self,
        _service: &LawExecutionService,
        request: &AnswerRequest<'_>,
    ) -> regelrecht_engine::Result<Option<Answer>> {
        assert_eq!(request.calculation_date, DATE);
        self.asked
            .borrow_mut()
            .push((request.name.to_string(), request.parameters.clone()));
        Ok(self.answers.get(request.name).map(|value| Answer {
            value: value.clone(),
            provenance: provenance(&self.authority.name, request.name),
        }))
    }
}

fn provenance(answerer: &str, answer: &str) -> AnswerProvenance {
    AnswerProvenance {
        answerer: answerer.to_string(),
        answer: answer.to_string(),
        law_id: format!("reductie_{answerer}"),
        article: "1".to_string(),
        valid_from: "2026-01-01".to_string(),
        records_read: 1,
        last_record: None,
    }
}

fn parameters() -> BTreeMap<String, Value> {
    BTreeMap::from([("bsn".to_string(), Value::String("999990019".into()))])
}

/// `output` of the reading law, with `answerer` running and `sphere` as
/// the other answerers, traced.
fn read(
    output: &str,
    answerer: &dyn OpenTermAnswerer,
    sphere: Vec<&dyn OpenTermAnswerer>,
    findings: BTreeMap<String, Value>,
) -> regelrecht_engine::Result<regelrecht_engine::ArticleResult> {
    let call = Call {
        findings,
        answerer: Some(answerer),
        chronicle: None,
        sphere,
    };
    service().evaluate_law_with_trace_in("wet_lezer", &[output], parameters(), call, DATE)
}

/// The node of the input of the binding that takes the answer `answer`:
/// the innermost of that name, where the engine asks the cells.
fn node<'n>(node: &'n PathNode, answer: &str) -> Option<&'n PathNode> {
    let below = node
        .children
        .iter()
        .find_map(|child| self::node(child, answer));
    below.or_else(|| (node.name == format!("antwoord_{answer}")).then_some(node))
}

/// The answerers under the node of the input that takes `answer`, in the
/// order of the trace: one node each, named after the answer.
fn answer_nodes(result: &regelrecht_engine::ArticleResult, answer: &str) -> Vec<String> {
    let trace = result.trace.as_ref().expect("traced");
    let input = node(trace, answer).expect("a node for the input");
    input
        .children
        .iter()
        .filter(|child| child.name == answer)
        .filter_map(|child| child.answer.as_ref().map(|a| a.answerer.clone()))
        .collect()
}

/// The message on the node of the input that takes `answer`.
fn input_message(result: &regelrecht_engine::ArticleResult, answer: &str) -> String {
    node(result.trace.as_ref().unwrap(), answer)
        .unwrap()
        .message
        .clone()
        .unwrap()
}

fn filled_by(result: &regelrecht_engine::ArticleResult, term: &str) -> FilledBy {
    result
        .open_terms
        .iter()
        .find(|f| f.term == term)
        .map(|f| f.filled_by.clone())
        .expect("the term is filled")
}

/// What the report says when the binding of `article` filled a term with
/// the answers of `answerers`.
fn bound(article: &str, answer: &str, answerers: &[&str]) -> FilledBy {
    FilledBy::Answer {
        law_id: "beleid_verstrekking".to_string(),
        article: article.to_string(),
        answers: answerers.iter().map(|a| provenance(a, answer)).collect(),
    }
}

#[test]
fn only_the_answerers_the_term_names_are_asked() {
    let running = Fake::college("wilgendaal");
    let other = Fake::college("oostmeer").answering("eerder_verstrekt", Value::Bool(true));
    let national = Fake::new("Landelijke uitgever", None);
    let stranger = Fake::new("ggd", Some("Gemeentelijke Gezondheidsdienst"))
        .answering("eerder_verstrekt", Value::Bool(false));
    let result = read(
        "eerder_gelezen",
        &running,
        vec![&other, &national, &stranger],
        BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(result.outputs["eerder_gelezen"], Value::Bool(true));
    // CATEGORY by category, INSTANCE by name; the GGD is not asked. A cell
    // without the answer is silent and has no node.
    assert_eq!(running.asked(), ["eerder_verstrekt"]);
    assert_eq!(other.asked(), ["eerder_verstrekt"]);
    assert_eq!(national.asked(), ["eerder_verstrekt"]);
    assert!(stranger.asked().is_empty());
    assert_eq!(
        filled_by(&result, "eerder_verstrekt"),
        bound("1", "eerder_verstrekt", &["oostmeer"])
    );
    assert_eq!(answer_nodes(&result, "eerder_verstrekt"), ["oostmeer"]);
    // The binding passes its parameters under the names it gives them.
    assert_eq!(
        other.asked.borrow()[0].1["bsn"],
        Value::String("999990019".into())
    );
    let message = input_message(&result, "eerder_verstrekt");
    assert!(
        message.contains("given by oostmeer")
            && message.contains(
                "silent: Landelijke uitgever (no answer 'eerder_verstrekt'), wilgendaal (no \
                 answer 'eerder_verstrekt')"
            ),
        "{message}"
    );
}

#[test]
fn a_running_answerer_the_term_does_not_name_is_not_asked() {
    let running = Fake::new("ggd", Some("Gemeentelijke Gezondheidsdienst"))
        .answering("eerder_verstrekt", Value::Bool(true));
    let mut findings = BTreeMap::new();
    findings.insert(
        "eerder_verstrekt_volgens_college".to_string(),
        Value::Bool(false),
    );
    let result = read("eerder_gelezen", &running, vec![], findings).unwrap();
    assert!(running.asked().is_empty());
    assert_eq!(result.outputs["eerder_gelezen"], Value::Bool(false));
    assert_eq!(
        filled_by(&result, "eerder_verstrekt"),
        bound("1", "eerder_verstrekt", &[])
    );
    assert_eq!(
        filled_by(&result, "eerder_verstrekt_volgens_college"),
        FilledBy::Finding
    );
    let message = input_message(&result, "eerder_verstrekt");
    assert!(
        message.contains(
            "No cell of this execution gives the answer 'eerder_verstrekt' for 'college van \
             burgemeester en wethouders' or 'Landelijke uitgever'"
        ),
        "{message}"
    );
}

#[test]
fn the_binding_routes_wherever_it_runs() {
    // Also where the article is evaluated at the top: the binding asks the
    // cells the term names, and the running one only where it is named.
    let running = Fake::new("ggd", None).answering("eerder_verstrekt", Value::Bool(true));
    let other = Fake::college("oostmeer").answering("eerder_verstrekt", Value::Bool(false));
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(&running),
        chronicle: None,
        sphere: vec![&other],
    };
    let result = service()
        .evaluate_law_in(
            "wet_verstrekking",
            &["eerder_verstrekt"],
            parameters(),
            call,
            DATE,
        )
        .unwrap();
    assert_eq!(result.outputs["eerder_verstrekt"], Value::Bool(false));
    assert!(running.asked().is_empty());
    assert_eq!(other.asked(), ["eerder_verstrekt"]);
}

#[test]
fn a_term_without_competent_authority_asks_the_holder_of_the_policy() {
    // The policy is of wilgendaal: its cell is asked, wherever in the call
    // it is, and no other.
    let running = Fake::new("ggd", None).answering("ander_feit", Value::Bool(false));
    let holder = Fake::college("wilgendaal").answering("ander_feit", Value::Bool(true));
    let other = Fake::college("oostmeer").answering("ander_feit", Value::Bool(false));
    let result = read(
        "ander_gelezen",
        &running,
        vec![&other, &holder],
        BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(result.outputs["ander_gelezen"], Value::Bool(true));
    assert!(running.asked().is_empty());
    assert!(other.asked().is_empty());
    assert_eq!(
        filled_by(&result, "ander_feit"),
        bound("2", "ander_feit", &["wilgendaal"])
    );
}

#[test]
fn two_answers_combine_with_or_and_each_has_its_node() {
    let running = Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Bool(true));
    let other = Fake::college("oostmeer").answering("eerder_verstrekt", Value::Bool(true));
    let result = read("eerder_gelezen", &running, vec![&other], BTreeMap::new()).unwrap();
    assert_eq!(result.outputs["eerder_gelezen"], Value::Bool(true));
    assert_eq!(
        filled_by(&result, "eerder_verstrekt"),
        bound("1", "eerder_verstrekt", &["oostmeer", "wilgendaal"])
    );
    assert_eq!(
        answer_nodes(&result, "eerder_verstrekt"),
        ["oostmeer", "wilgendaal"]
    );
    let message = input_message(&result, "eerder_verstrekt");
    assert!(message.contains("combined with OR"), "{message}");
}

#[test]
fn true_and_false_combine_with_or_to_true() {
    let running = Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Bool(false));
    let other = Fake::college("oostmeer").answering("eerder_verstrekt", Value::Bool(true));
    let result = read("eerder_gelezen", &running, vec![&other], BTreeMap::new()).unwrap();
    assert_eq!(result.outputs["eerder_gelezen"], Value::Bool(true));
}

#[test]
fn the_order_of_the_sphere_changes_neither_outcome_nor_trace() {
    let a = Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Bool(false));
    let b = Fake::college("oostmeer").answering("eerder_verstrekt", Value::Bool(true));
    let c = Fake::new("Landelijke uitgever", None).answering("eerder_verstrekt", Value::Null);
    let one = read("eerder_gelezen", &a, vec![&b, &c], BTreeMap::new()).unwrap();
    let two = read("eerder_gelezen", &a, vec![&c, &b], BTreeMap::new()).unwrap();
    assert_eq!(one.outputs, two.outputs);
    assert_eq!(one.open_terms, two.open_terms);
    assert_eq!(
        answer_nodes(&one, "eerder_verstrekt"),
        answer_nodes(&two, "eerder_verstrekt")
    );
    assert_eq!(
        answer_nodes(&one, "eerder_verstrekt"),
        ["Landelijke uitgever", "oostmeer", "wilgendaal"]
    );
}

#[test]
fn a_silent_answer_does_not_count() {
    let running = Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Null);
    let other = Fake::college("oostmeer").answering("eerder_verstrekt", Value::Bool(true));
    let result = read("eerder_gelezen", &running, vec![&other], BTreeMap::new()).unwrap();
    assert_eq!(result.outputs["eerder_gelezen"], Value::Bool(true));
    assert_eq!(
        filled_by(&result, "eerder_verstrekt"),
        bound("1", "eerder_verstrekt", &["oostmeer"])
    );
    // The silent one has its node too.
    assert_eq!(
        answer_nodes(&result, "eerder_verstrekt"),
        ["oostmeer", "wilgendaal"]
    );
}

#[test]
fn when_all_are_silent_the_policy_takes_its_own_finding() {
    // The binding fills the term; where every cell is silent, the policy
    // says what holds: the finding on its own open term (RFC-048 §2).
    let running = Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Null);
    let other = Fake::college("oostmeer");
    let mut findings = BTreeMap::new();
    findings.insert(
        "eerder_verstrekt_volgens_college".to_string(),
        Value::Bool(false),
    );
    let result = read("eerder_gelezen", &running, vec![&other], findings).unwrap();
    assert_eq!(result.outputs["eerder_gelezen"], Value::Bool(false));
    assert_eq!(
        filled_by(&result, "eerder_verstrekt"),
        bound("1", "eerder_verstrekt", &["wilgendaal"])
    );
    assert_eq!(
        filled_by(&result, "eerder_verstrekt_volgens_college"),
        FilledBy::Finding
    );
    let message = input_message(&result, "eerder_verstrekt");
    assert!(
        message.contains("silent: oostmeer (no answer 'eerder_verstrekt'), wilgendaal"),
        "{message}"
    );
}

#[test]
fn two_answers_without_combine_are_an_error() {
    let running = Fake::college("wilgendaal").answering("enig_feit", Value::Bool(true));
    let other = Fake::college("oostmeer").answering("enig_feit", Value::Bool(true));
    let e = read("enig_gelezen", &running, vec![&other], BTreeMap::new()).unwrap_err();
    let message = e.to_string();
    assert!(
        message.contains("has 2 answers 'enig_feit', from oostmeer, wilgendaal"),
        "{message}"
    );
    assert!(message.contains("no combine"), "{message}");
}

#[test]
fn the_combine_of_the_binding_serves_where_the_law_has_none() {
    let with_or = BINDING.replace(
        "              reduction: enig_feit\n",
        "              reduction: enig_feit\n              combine: AND\n",
    );
    assert_ne!(with_or, BINDING);
    let mut service = LawExecutionService::new();
    for law in [ESTABLISHING, READING, with_or.as_str()] {
        service.load_law(law).unwrap();
    }
    let running = Fake::college("wilgendaal").answering("enig_feit", Value::Bool(true));
    let other = Fake::college("oostmeer").answering("enig_feit", Value::Bool(false));
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(&running),
        chronicle: None,
        sphere: vec![&other],
    };
    let result = service
        .evaluate_law_with_trace_in("wet_lezer", &["enig_gelezen"], parameters(), call, DATE)
        .unwrap();
    assert_eq!(result.outputs["enig_gelezen"], Value::Bool(false));
    assert!(input_message(&result, "enig_feit").contains("combined with AND"));
    // The law's rule wins over the binding's.
    let overruled = BINDING.replace(
        "              reduction: eerder_verstrekt\n",
        "              reduction: eerder_verstrekt\n              combine: AND\n",
    );
    let mut service = LawExecutionService::new();
    for law in [ESTABLISHING, READING, overruled.as_str()] {
        service.load_law(law).unwrap();
    }
    let running = Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Bool(true));
    let other = Fake::college("oostmeer").answering("eerder_verstrekt", Value::Bool(false));
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(&running),
        chronicle: None,
        sphere: vec![&other],
    };
    let result = service
        .evaluate_law_in("wet_lezer", &["eerder_gelezen"], parameters(), call, DATE)
        .unwrap();
    assert_eq!(result.outputs["eerder_gelezen"], Value::Bool(true));
}

#[test]
fn combining_that_fails_is_an_error_that_names_the_answers() {
    let a = Fake::college("wilgendaal").answering("aantal_kaarten", Value::String("x".into()));
    let b = Fake::college("oostmeer").answering("aantal_kaarten", Value::Int(2));
    let e = try_aantal(&a, vec![&b]).unwrap_err();
    assert!(
        e.to_string()
            .contains("combining the answers 'aantal_kaarten' of oostmeer, wilgendaal with ADD"),
        "{e}"
    );
}

#[test]
fn one_answer_beside_a_silent_one_needs_no_combine() {
    let running = Fake::college("wilgendaal").answering("kenmerk", Value::String("A".into()));
    let other = Fake::college("oostmeer").answering("kenmerk", Value::Null);
    let result = read("kenmerk_gelezen", &running, vec![&other], BTreeMap::new()).unwrap();
    assert_eq!(result.outputs["kenmerk_gelezen"], Value::String("A".into()));
}

#[test]
fn the_competence_of_the_article_routes_nothing() {
    // Art. 3 names who decides by a reference; its term names no one, so
    // the holder of the policy is asked, and its answer counts.
    let running = Fake::college("wilgendaal").answering("verwezen_feit", Value::Bool(true));
    let other = Fake::college("oostmeer").answering("verwezen_feit", Value::Bool(false));
    let result = read("verwezen_gelezen", &running, vec![&other], BTreeMap::new()).unwrap();
    assert_eq!(result.outputs["verwezen_gelezen"], Value::Bool(true));
    assert!(other.asked().is_empty());
}

#[test]
fn two_answerers_with_one_name_are_an_error() {
    let running = Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Bool(true));
    let twin = Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Bool(true));
    let e = read("eerder_gelezen", &running, vec![&twin], BTreeMap::new()).unwrap_err();
    assert!(e.to_string().contains("named 'wilgendaal'"), "{e}");
}

#[test]
fn an_article_the_target_reads_in_its_own_law_is_routed_by_its_binding() {
    // Art. 6 reads art. 7 within its own law; the binding of art. 7's term
    // asks the GGD the term names, not the running college.
    let running = Fake::college("wilgendaal").answering("zevende_feit", Value::Bool(true));
    let ggd = Fake::new("ggd", Some("Gemeentelijke Gezondheidsdienst"))
        .answering("zevende_feit", Value::Bool(false));
    let result = read("zesde_gelezen", &running, vec![&ggd], BTreeMap::new()).unwrap();
    assert_eq!(result.outputs["zesde_gelezen"], Value::Bool(false));
    assert!(running.asked().is_empty());
    assert_eq!(ggd.asked(), ["zevende_feit"]);
}

#[test]
fn a_term_is_filled_only_by_the_answers_of_its_own_binding() {
    // Art. 2 of the policy takes no answer itself but reads the term of
    // art. 1, which its binding (art. 1 of the same policy) fills with the
    // answer of the cell. That answer is art. 1's: art. 2 filled its term
    // as regulation, not with an answer.
    let policy = BINDING.replacen(
        "reduction: ander_feit\n",
        "regulation: wet_verstrekking\n              output: eerder_verstrekt\n",
        1,
    );
    assert_ne!(policy, BINDING);
    let mut service = LawExecutionService::new();
    service.load_law(ESTABLISHING).unwrap();
    service.load_law(READING).unwrap();
    service.load_law(&policy).unwrap();
    let running = Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Bool(true));
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(&running),
        chronicle: None,
        sphere: Vec::new(),
    };
    let result = service
        .evaluate_law_with_trace_in("wet_lezer", &["ander_gelezen"], parameters(), call, DATE)
        .unwrap();
    assert_eq!(result.outputs["ander_gelezen"], Value::Bool(true));
    assert_eq!(
        filled_by(&result, "eerder_verstrekt"),
        bound("1", "eerder_verstrekt", &["wilgendaal"])
    );
    assert_eq!(
        filled_by(&result, "ander_feit"),
        FilledBy::Regulation {
            law_id: "beleid_verstrekking".to_string(),
            article: "2".to_string(),
        }
    );
    let trace = result.trace.unwrap().render_box_drawing();
    let art2 = "Open term 'ander_feit' implemented by beleid_verstrekking article 2";
    assert!(trace.contains(art2), "{trace}");
    assert!(!trace.contains(&format!("{art2}, with")), "{trace}");
}

#[test]
fn without_answerers_the_binding_gives_nothing_and_the_policy_says_what_holds() {
    let result = service()
        .evaluate_law_with_trace_in(
            "wet_lezer",
            &["eerder_gelezen"],
            parameters(),
            Call::with_findings(BTreeMap::from([(
                "eerder_verstrekt_volgens_college".to_string(),
                Value::Bool(true),
            )])),
            DATE,
        )
        .unwrap();
    assert_eq!(result.outputs["eerder_gelezen"], Value::Bool(true));
    assert_eq!(
        filled_by(&result, "eerder_verstrekt"),
        bound("1", "eerder_verstrekt", &[])
    );
    // Without the finding the policy's own required term has no value.
    let e = service()
        .evaluate_law("wet_lezer", &["eerder_gelezen"], parameters(), DATE)
        .unwrap_err();
    assert!(
        e.to_string().contains("eerder_verstrekt_volgens_college"),
        "{e}"
    );
}

#[test]
fn a_finding_on_a_term_the_policy_fills_is_asked_by_none() {
    // No judgement of the authority steps in where a policy fills the term:
    // the policy's value is the term's, also null (RFC-048 §2).
    let running = Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Bool(true));
    let mut findings = BTreeMap::new();
    findings.insert("eerder_verstrekt".to_string(), Value::Bool(false));
    let e = read("eerder_gelezen", &running, vec![], findings).unwrap_err();
    assert!(e.to_string().contains("is asked by none"), "{e}");
}

#[test]
fn a_null_answer_without_a_fallback_is_the_value() {
    // `kenmerk` has no default and nobody decides it per case: null is the
    // answer, and the report says who gave it.
    let running = Fake::college("wilgendaal").answering("kenmerk", Value::Null);
    let result = read("kenmerk_gelezen", &running, vec![], BTreeMap::new()).unwrap();
    assert_eq!(result.outputs["kenmerk_gelezen"], Value::Null);
    assert_eq!(
        filled_by(&result, "kenmerk"),
        bound("4", "kenmerk", &["wilgendaal"])
    );
    // A cell without the answer leaves the report with no answer at all.
    let running = Fake::college("wilgendaal");
    let result = read("kenmerk_gelezen", &running, vec![], BTreeMap::new()).unwrap();
    assert_eq!(filled_by(&result, "kenmerk"), bound("4", "kenmerk", &[]));
}

#[test]
fn a_binding_that_passes_a_value_it_does_not_have_fails() {
    // The binding names a parameter it does not declare: with a cell to ask,
    // that is an error, not an answer without it.
    let typo = BINDING.replacen(
        "parameters: {bsn: $bsn}",
        "parameters: {bsn: $bsn_aanvrager}",
        1,
    );
    assert_ne!(typo, BINDING);
    let mut service = LawExecutionService::new();
    for law in [ESTABLISHING, READING, typo.as_str()] {
        service.load_law(law).unwrap();
    }
    let running = Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Bool(true));
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(&running),
        chronicle: None,
        sphere: vec![],
    };
    let e = service
        .evaluate_law_in("wet_lezer", &["eerder_gelezen"], parameters(), call, DATE)
        .unwrap_err();
    assert!(e.to_string().contains("bsn_aanvrager"), "{e}");
    assert!(running.asked().is_empty());
}

#[test]
fn an_answer_a_binding_declares_never_absent_must_be_given() {
    let never_absent = BINDING
        .replacen(
            "          - name: antwoord_eerder_verstrekt\n            type: boolean\n            nullable: true\n",
            "          - name: antwoord_eerder_verstrekt\n            type: boolean\n",
            1,
        )
        .replacen(
            "          - output: eerder_verstrekt\n            value:\n              operation: IF\n              cases:\n                - when: {operation: EQUALS, subject: $antwoord_eerder_verstrekt, value: null}\n                  then: $eerder_verstrekt_volgens_college\n              default: $antwoord_eerder_verstrekt\n",
            "          - {output: eerder_verstrekt, value: $antwoord_eerder_verstrekt}\n",
            1,
        );
    assert_ne!(never_absent, BINDING);
    let mut service = LawExecutionService::new();
    for law in [ESTABLISHING, READING, never_absent.as_str()] {
        service.load_law(law).unwrap();
    }
    for running in [
        Fake::college("wilgendaal"),
        Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Null),
    ] {
        let call = Call {
            findings: BTreeMap::from([(
                "eerder_verstrekt_volgens_college".to_string(),
                Value::Bool(true),
            )]),
            answerer: Some(&running),
            chronicle: None,
            sphere: vec![],
        };
        let e = service
            .evaluate_law_in("wet_lezer", &["eerder_gelezen"], parameters(), call, DATE)
            .unwrap_err();
        assert!(e.to_string().contains("declared never absent"), "{e}");
    }
    // Without any cell it fails the same way.
    let e = service
        .evaluate_law_with_findings(
            "wet_lezer",
            &["eerder_gelezen"],
            parameters(),
            BTreeMap::from([(
                "eerder_verstrekt_volgens_college".to_string(),
                Value::Bool(true),
            )]),
            DATE,
        )
        .unwrap_err();
    assert!(e.to_string().contains("No cell of this execution"), "{e}");
}

/// Art. 8 read through `source.regulation`, with `answerer` and `sphere`.
fn aantal(
    answerer: &dyn OpenTermAnswerer,
    sphere: Vec<&dyn OpenTermAnswerer>,
) -> regelrecht_engine::ArticleResult {
    try_aantal(answerer, sphere).unwrap()
}

/// [`aantal`], or the error.
fn try_aantal(
    answerer: &dyn OpenTermAnswerer,
    sphere: Vec<&dyn OpenTermAnswerer>,
) -> regelrecht_engine::Result<regelrecht_engine::ArticleResult> {
    let reader = r#"
$id: wet_teller
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het aantal kaarten.
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: aantal
            type: number
            nullable: true
            source:
              regulation: wet_verstrekking
              output: aantal_kaarten
              parameters:
                bsn: $bsn
        output:
          - name: aantal_gelezen
            type: number
            nullable: true
        actions:
          - output: aantal_gelezen
            value: $aantal
"#;
    let mut service = service();
    service.load_law(reader).unwrap();
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(answerer),
        chronicle: None,
        sphere,
    };
    service.evaluate_law_with_trace_in("wet_teller", &["aantal_gelezen"], parameters(), call, DATE)
}

#[test]
fn two_answers_add_up_and_the_trace_names_the_rule() {
    let a = Fake::college("wilgendaal").answering("aantal_kaarten", Value::Int(1));
    let b = Fake::college("oostmeer").answering("aantal_kaarten", Value::Int(2));
    let result = aantal(&a, vec![&b]);
    assert_eq!(result.outputs["aantal_gelezen"], Value::Int(3));
    let message = input_message(&result, "aantal_kaarten");
    assert!(message.contains("combined with ADD"), "{message}");
}

#[test]
fn several_silent_answers_without_a_fallback_are_null_and_combine_nothing() {
    let a = Fake::college("wilgendaal").answering("aantal_kaarten", Value::Null);
    let b = Fake::college("oostmeer").answering("aantal_kaarten", Value::Null);
    let result = aantal(&a, vec![&b]);
    assert_eq!(result.outputs["aantal_gelezen"], Value::Null);
    let message = input_message(&result, "aantal_kaarten");
    assert!(
        message.contains("silent: oostmeer, wilgendaal"),
        "{message}"
    );
    assert!(!message.contains("combined"), "{message}");
}

#[test]
fn a_combine_that_does_not_fit_the_type_is_refused_at_load() {
    let yaml = ESTABLISHING.replacen("combine: OR", "combine: ADD", 1);
    let e = ArticleBasedLaw::from_yaml_str(&yaml).unwrap_err();
    assert!(
        e.to_string().contains(
            "combines its answers with ADD (combine), which does not fit its type boolean"
        ),
        "{e}"
    );
    // ONLY is no rule for answers: one needs none, two without one fail.
    let yaml = ESTABLISHING.replacen("combine: OR", "combine: ONLY", 1);
    let e = ArticleBasedLaw::from_yaml_str(&yaml).unwrap_err();
    assert!(
        e.to_string().contains("which is no rule for answers"),
        "{e}"
    );
    // Without competent authorities named, only one answerer is asked.
    let yaml = ESTABLISHING.replacen(
        "          decided_per_case_by: het college\n      execution:\n        parameters:\n          - name: bsn\n            type: string\n            required: true\n        output:\n          - name: ander_feit",
        "          decided_per_case_by: het college\n          combine: OR\n      execution:\n        parameters:\n          - name: bsn\n            type: string\n            required: true\n        output:\n          - name: ander_feit",
        1,
    );
    assert_ne!(yaml, ESTABLISHING);
    let e = ArticleBasedLaw::from_yaml_str(&yaml).unwrap_err();
    assert!(
        e.to_string().contains("names no competent authorities"),
        "{e}"
    );
}

#[test]
fn competence_in_the_wrong_place_is_refused_at_load() {
    let refused = |from: &str, to: &str, says: &str| {
        let yaml = ESTABLISHING.replacen(from, to, 1);
        assert_ne!(yaml, ESTABLISHING, "{from}");
        let e = ArticleBasedLaw::from_yaml_str(&yaml).unwrap_err();
        assert!(e.to_string().contains(says), "{e}");
    };
    let term_list = "          competent_authority:\n            - name: college van burgemeester en wethouders\n              type: CATEGORY\n            - name: Landelijke uitgever\n";
    // An empty list would route to nobody without a word.
    refused(
        term_list,
        "          competent_authority: []\n",
        "empty competent_authority list",
    );
    // A term names its authorities, not an output that points at one.
    refused(
        term_list,
        "          competent_authority: '#uitgever'\n",
        "name the authorities themselves",
    );
    // On an article the field says who decides: one authority.
    refused(
        "      competent_authority: '#uitgever'\n",
        "      competent_authority:\n        - name: Landelijke uitgever\n",
        "a list of those who may answer belongs on an open term or a fact",
    );
}

#[test]
fn the_competence_of_a_fact_is_read_and_checked() {
    let with_fact = |entry: &str| {
        format!(
            r#"
$id: wet_feit
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Een onderzoek heeft plaatsgehad.
    machine_readable:
      requires:
{entry}
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        output:
          - name: klaar
            type: boolean
        actions:
          - output: klaar
            value: true
"#
        )
    };
    let law = ArticleBasedLaw::from_yaml_str(&with_fact(
        "        - fact: onderzoek\n          parameters: [bsn]\n          competent_authority:\n            - name: Gemeentelijke Gezondheidsdienst\n              type: CATEGORY\n",
    ))
    .unwrap();
    let requirement = &law.articles[0]
        .machine_readable
        .as_ref()
        .unwrap()
        .requires
        .as_ref()
        .unwrap()[0];
    let named = requirement
        .competent_authority
        .as_ref()
        .unwrap()
        .named()
        .unwrap();
    assert_eq!(named[0].name, "Gemeentelijke Gezondheidsdienst");
    let e = ArticleBasedLaw::from_yaml_str(&with_fact(
        "        - fact: onderzoek\n          parameters: [bsn]\n          competent_authority: []\n",
    ))
    .unwrap_err();
    assert!(
        e.to_string().contains("fact 'onderzoek' has an empty"),
        "{e}"
    );
    let e = ArticleBasedLaw::from_yaml_str(&with_fact(
        "        - article: '2'\n          competent_authority:\n            name: Gemeentelijke Gezondheidsdienst\n",
    ))
    .unwrap_err();
    assert!(
        e.to_string()
            .contains("has a competent_authority but no fact"),
        "{e}"
    );
}

/// A deciding article with one more output beside its decision: whether a
/// card was issued before, for the college or the national issuer to say.
/// The decision does not read it, so it belongs to no item (RFC-052).
const DECIDING: &str = r#"
$id: wet_beslissing
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het college verstrekt de kaart.
    machine_readable:
      competent_authority:
        name: college van burgemeester en wethouders
        type: CATEGORY
      open_terms:
        - id: eerder_verstrekt
          competent_authority:
            - name: college van burgemeester en wethouders
              type: CATEGORY
            - name: Landelijke uitgever
          type: boolean
          required: true
          decided_per_case_by: het college
          combine: OR
      execution:
        produces:
          - type: BESCHIKKING
            outputs: [kaart_verstrekt]
        parameters:
          - name: bsn
            type: string
            required: true
        output:
          - name: kaart_verstrekt
            type: boolean
          - name: eerder_verstrekt
            type: boolean
        actions:
          - output: kaart_verstrekt
            value: true
          - output: eerder_verstrekt
            value: $eerder_verstrekt
"#;

/// `output` of the deciding article, read through `source.regulation`.
fn read_deciding(
    output: &str,
    answerer: &dyn OpenTermAnswerer,
    sphere: Vec<&dyn OpenTermAnswerer>,
) -> regelrecht_engine::ArticleResult {
    let reader = format!(
        r#"
$id: wet_lezer_besluit
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Wat de lezer leest.
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
            required: true
        input:
          - name: gelezen
            type: boolean
            source:
              regulation: wet_beslissing
              output: {output}
              parameters:
                bsn: $bsn
        output:
          - name: gelezen
            type: boolean
        actions:
          - output: gelezen
            value: $gelezen
"#
    );
    let mut service = LawExecutionService::new();
    service.load_law(DECIDING).unwrap();
    service.load_law(&reader).unwrap();
    service
        .load_law(&BINDING.replace(
            "        - {law: wet_verstrekking, article: '1', open_term: eerder_verstrekt}",
            "        - {law: wet_beslissing, article: '1', open_term: eerder_verstrekt}",
        ))
        .unwrap();
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(answerer),
        chronicle: None,
        sphere,
    };
    service
        .evaluate_law_with_trace_in("wet_lezer_besluit", &["gelezen"], parameters(), call, DATE)
        .unwrap()
}

#[test]
fn an_output_beside_the_decision_is_asked_only_when_read() {
    let running = Fake::college("wilgendaal");
    let other = Fake::college("oostmeer").answering("eerder_verstrekt", Value::Bool(true));
    // The decision reads nothing of it: no cell is asked.
    let result = read_deciding("kaart_verstrekt", &running, vec![&other]);
    assert_eq!(result.outputs["gelezen"], Value::Bool(true));
    assert!(running.asked().is_empty());
    assert!(other.asked().is_empty());
    // Read itself, the term is asked of every competent cell.
    let result = read_deciding("eerder_verstrekt", &running, vec![&other]);
    assert_eq!(result.outputs["gelezen"], Value::Bool(true));
    assert_eq!(running.asked(), ["eerder_verstrekt"]);
    assert_eq!(other.asked(), ["eerder_verstrekt"]);
}
