//! Values of other actors by competence (RFC-051, RFC-048 §4): a value that
//! another actor establishes is read with `source.regulation` from the
//! article that establishes it, and the policy of the authority that decides
//! binds its open term to a named answer (`source.reduction`). Where the term
//! names its own `competent_authority`, the engine asks the answerer of the
//! call whose authority the term names for that answer, and none other.
//! Where the term names no one, it asks the policy's holder. One question
//! goes to one cell: two cells of the call that the term names are an error,
//! and the engine merges no answers. An answerer without an answer of that
//! name is silent. The article's `competent_authority` says who decides
//! (RFC-002) and routes nothing.
//!
//! The cell that runs the execution needs a ground to use the answer of
//! another cell (RFC-056): the binding is in its own policy or in that of the
//! authority it decides for, or the term names the policy's holder as
//! competent and the holder's cell gives the answer. Most tests here run
//! the cell of wilgendaal, the policy's holder.
//!
//! Synthesis is an article that takes the answers of different cells, to
//! different questions, and makes one outcome of them
//! (`synthesis_makes_one_outcome_of_answers_of_different_cells`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::cell::RefCell;
use std::collections::BTreeMap;

use regelrecht_engine::{
    Answer, AnswerProvenance, AnswerRequest, ArticleBasedLaw, Authority, Call, FilledBy,
    LawExecutionService, LawLoad, OpenTermAnswerer, PathNode, PathNodeType, Unanswered, Value,
};

const DATE: &str = "2026-10-20";

/// The law that establishes the facts, with the competence on each term:
/// who issued a card before (art. 1, a college or the fictitious national
/// issuer), a fact without competence (art. 2), one of an article whose
/// decider is a reference (art. 3), a mark one college gives (art. 4), a
/// fact one college gives (art. 5), and one the GGD establishes for an
/// article the college decides (art. 7), read through art. 6.
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
  - number: '2'
    text: Een kaart zonder nieuw onderzoek, als eerder een kaart is verstrekt en de GGD het feit vaststelt.
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
          - name: zesde
            type: boolean
            source:
              regulation: wet_verstrekking
              output: zesde_feit
              parameters:
                bsn: $bsn
        output:
          - name: kaart_zonder_onderzoek
            type: boolean
        actions:
          - output: kaart_zonder_onderzoek
            value:
              operation: AND
              conditions: [$eerder, $zesde]
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
            trace: None,
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
        decides_for: None,
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
        unanswered: Vec::new(),
    }
}

/// [`bound`], where the input that takes `answer` got none, for the reason
/// `message`: the report lists it beside the answers.
fn unanswered(article: &str, answer: &str, answerers: &[&str], message: &str) -> FilledBy {
    FilledBy::Answer {
        law_id: "beleid_verstrekking".to_string(),
        article: article.to_string(),
        answers: answerers.iter().map(|a| provenance(a, answer)).collect(),
        unanswered: vec![Unanswered {
            answer: answer.to_string(),
            input: format!("antwoord_{answer}"),
            message: message.to_string(),
        }],
    }
}

/// Why no cell gave `eerder_verstrekt`: no cell of the call is one the term
/// names.
const NO_CELL_FOR_EERDER: &str = "No cell of this execution gives the answer 'eerder_verstrekt' \
     for 'college van burgemeester en wethouders' or 'Landelijke uitgever'";

#[test]
fn only_the_answerer_the_term_names_is_asked() {
    // The policy is of the running cell; the term names a category it does
    // not belong to.
    let running = Fake::new("wilgendaal", None).answering("eerder_verstrekt", Value::Bool(false));
    let other = Fake::college("oostmeer").answering("eerder_verstrekt", Value::Bool(true));
    let result = read("eerder_gelezen", &running, vec![&other], BTreeMap::new()).unwrap();
    assert_eq!(result.outputs["eerder_gelezen"], Value::Bool(true));
    // CATEGORY by category; the running cell is not asked.
    assert_eq!(other.asked(), ["eerder_verstrekt"]);
    assert!(running.asked().is_empty());
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
    assert!(message.contains("given by oostmeer"), "{message}");
}

#[test]
fn an_instance_is_asked_by_name() {
    let running = Fake::new("wilgendaal", None);
    let national =
        Fake::new("Landelijke uitgever", None).answering("eerder_verstrekt", Value::Bool(true));
    let result = read("eerder_gelezen", &running, vec![&national], BTreeMap::new()).unwrap();
    assert_eq!(result.outputs["eerder_gelezen"], Value::Bool(true));
    assert_eq!(national.asked(), ["eerder_verstrekt"]);
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
        unanswered("1", "eerder_verstrekt", &[], NO_CELL_FOR_EERDER)
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
    let running = Fake::new("wilgendaal", None).answering("eerder_verstrekt", Value::Bool(true));
    let other = Fake::college("oostmeer").answering("eerder_verstrekt", Value::Bool(false));
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(&running),
        decides_for: None,
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
    // it is, and no other. The running cell decides for wilgendaal, which
    // gives it the ground to use the answer (RFC-056).
    let running = Fake::new("rechtbank", None).answering("ander_feit", Value::Bool(false));
    let holder = Fake::college("wilgendaal").answering("ander_feit", Value::Bool(true));
    let other = Fake::college("oostmeer").answering("ander_feit", Value::Bool(false));
    let result = read_with(
        BINDING,
        "ander_gelezen",
        &running,
        Some(holder.authority()),
        vec![&other, &holder],
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
fn two_cells_the_term_names_are_an_error_and_none_is_asked() {
    // One question goes to one cell. Which of two colleges answers is open,
    // so the engine asks neither and merges nothing.
    let running = Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Bool(true));
    let other = Fake::college("oostmeer").answering("eerder_verstrekt", Value::Bool(true));
    let national =
        Fake::new("Landelijke uitgever", None).answering("eerder_verstrekt", Value::Bool(false));
    let e = read(
        "eerder_gelezen",
        &running,
        vec![&other, &national],
        BTreeMap::new(),
    )
    .unwrap_err();
    let message = e.to_string();
    assert!(
        message.contains(
            "3 cells of the call are named by 'college van burgemeester en wethouders' or \
             'Landelijke uitgever': Landelijke uitgever, oostmeer, wilgendaal"
        ),
        "{message}"
    );
    assert!(message.contains("which one is open"), "{message}");
    assert!(running.asked().is_empty());
    assert!(other.asked().is_empty());
    assert!(national.asked().is_empty());
}
#[test]
fn when_the_cell_is_silent_the_policy_takes_its_own_finding() {
    // The binding fills the term; where the cell is silent, the policy says
    // what holds: the finding on its own open term (RFC-048 §2).
    let running = Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Null);
    let mut findings = BTreeMap::new();
    findings.insert(
        "eerder_verstrekt_volgens_college".to_string(),
        Value::Bool(false),
    );
    let result = read("eerder_gelezen", &running, vec![], findings).unwrap();
    assert_eq!(result.outputs["eerder_gelezen"], Value::Bool(false));
    assert_eq!(
        filled_by(&result, "eerder_verstrekt"),
        unanswered(
            "1",
            "eerder_verstrekt",
            &["wilgendaal"],
            "No cell gives the answer 'eerder_verstrekt' for 'college van burgemeester en \
             wethouders' or 'Landelijke uitgever'; silent: wilgendaal"
        )
    );
    assert_eq!(
        filled_by(&result, "eerder_verstrekt_volgens_college"),
        FilledBy::Finding
    );
    // The silent cell has its node too.
    assert_eq!(answer_nodes(&result, "eerder_verstrekt"), ["wilgendaal"]);
    let message = input_message(&result, "eerder_verstrekt");
    assert!(message.contains("silent: wilgendaal"), "{message}");
    // A cell without the answer is silent without a node.
    let running = Fake::college("wilgendaal");
    let mut findings = BTreeMap::new();
    findings.insert(
        "eerder_verstrekt_volgens_college".to_string(),
        Value::Bool(true),
    );
    let result = read("eerder_gelezen", &running, vec![], findings).unwrap();
    assert_eq!(result.outputs["eerder_gelezen"], Value::Bool(true));
    let message = input_message(&result, "eerder_verstrekt");
    assert!(
        message.contains("silent: wilgendaal (no answer 'eerder_verstrekt')"),
        "{message}"
    );
}

#[test]
fn synthesis_makes_one_outcome_of_answers_of_different_cells() {
    // Article 2 of the reading law asks two questions, each of the cell the
    // law makes competent for it: whether a card was issued before (a
    // college) and the fact the GGD establishes. Each question goes to one
    // cell; the article makes one outcome of the two answers.
    let running = Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Bool(true));
    let ggd = Fake::new("ggd", Some("Gemeentelijke Gezondheidsdienst"))
        .answering("zevende_feit", Value::Bool(true));
    let result = read(
        "kaart_zonder_onderzoek",
        &running,
        vec![&ggd],
        BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(result.outputs["kaart_zonder_onderzoek"], Value::Bool(true));
    assert_eq!(running.asked(), ["eerder_verstrekt"]);
    assert_eq!(ggd.asked(), ["zevende_feit"]);
    assert_eq!(
        filled_by(&result, "eerder_verstrekt"),
        bound("1", "eerder_verstrekt", &["wilgendaal"])
    );
    assert_eq!(
        filled_by(&result, "zevende_feit"),
        bound("7", "zevende_feit", &["ggd"])
    );
    // One answer that is false makes the outcome false.
    let ggd = Fake::new("ggd", Some("Gemeentelijke Gezondheidsdienst"))
        .answering("zevende_feit", Value::Bool(false));
    let result = read(
        "kaart_zonder_onderzoek",
        &running,
        vec![&ggd],
        BTreeMap::new(),
    )
    .unwrap();
    assert_eq!(result.outputs["kaart_zonder_onderzoek"], Value::Bool(false));
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
    assert!(e.to_string().contains("wilgendaal, wilgendaal"), "{e}");
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
        decides_for: None,
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
        unanswered("1", "eerder_verstrekt", &[], NO_CELL_FOR_EERDER)
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
    let silent = "No cell gives the answer 'kenmerk' for 'college van burgemeester en \
                  wethouders'; silent: wilgendaal";
    assert_eq!(
        filled_by(&result, "kenmerk"),
        unanswered("4", "kenmerk", &["wilgendaal"], silent)
    );
    // A cell without the answer leaves the report with no answer at all,
    // and says which one it missed.
    let running = Fake::college("wilgendaal");
    let result = read("kenmerk_gelezen", &running, vec![], BTreeMap::new()).unwrap();
    assert_eq!(
        filled_by(&result, "kenmerk"),
        unanswered(
            "4",
            "kenmerk",
            &[],
            &format!("{silent} (no answer 'kenmerk')")
        )
    );
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
        decides_for: None,
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
            decides_for: None,
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
/// The decision does not read it, so it belongs to no item (RFC-045 §1).
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
        decides_for: None,
        chronicle: None,
        sphere,
    };
    service
        .evaluate_law_with_trace_in("wet_lezer_besluit", &["gelezen"], parameters(), call, DATE)
        .unwrap()
}

#[test]
fn an_output_beside_the_decision_is_asked_only_when_read() {
    let running = Fake::new("wilgendaal", None);
    let other = Fake::college("oostmeer").answering("eerder_verstrekt", Value::Bool(true));
    // The decision reads nothing of it: no cell is asked.
    let result = read_deciding("kaart_verstrekt", &running, vec![&other]);
    assert_eq!(result.outputs["gelezen"], Value::Bool(true));
    assert!(running.asked().is_empty());
    assert!(other.asked().is_empty());
    // Read itself, the term is asked of the competent cell.
    let result = read_deciding("eerder_verstrekt", &running, vec![&other]);
    assert_eq!(result.outputs["gelezen"], Value::Bool(true));
    assert!(running.asked().is_empty());
    assert_eq!(other.asked(), ["eerder_verstrekt"]);
}

/// A law with an open term the policy below binds to an answer, for
/// `a_binding_passes_another_input_of_its_article_to_the_cell`.
const PEIL: &str = r#"
$id: wet_peil
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Of er in het jaar van de dag al eerder iets is verstrekt.
    machine_readable:
      open_terms:
        - id: al_eerder
          type: boolean
          required: true
          decided_per_case_by: het college
      execution:
        parameters:
          - {name: datum, type: date}
        output:
          - {name: al_eerder_verstrekt, type: boolean}
        actions:
          - output: al_eerder_verstrekt
            value: $al_eerder
"#;

/// The policy that binds the term: it asks the cell with the year of the
/// day, which another of its own articles computes.
const PEIL_BELEID: &str = r#"
$id: beleid_peil
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
url: https://example.org/beleid_peil
competent_authority:
  name: wilgendaal
articles:
  - number: '1'
    text: Wat het beleid ontleent aan het antwoord al_eerder.
    machine_readable:
      implements:
        - {law: wet_peil, article: '1', open_term: al_eerder}
      execution:
        parameters:
          - {name: datum, type: date}
        input:
          - name: jaar
            type: number
            source:
              regulation: beleid_peil
              output: jaar_van
              parameters: {datum: $datum}
          - name: antwoord
            type: boolean
            nullable: true
            source:
              reduction: al_eerder
              parameters: {jaar: $jaar}
        output:
          - {name: al_eerder, type: boolean}
        actions:
          - output: al_eerder
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $antwoord, value: null}
                  then: false
              default: $antwoord
  - number: '2'
    text: Het jaar van een dag.
    machine_readable:
      execution:
        parameters:
          - {name: datum, type: date}
        output:
          - {name: jaar_van, type: number}
        actions:
          - output: jaar_van
            value: {operation: DATE_PART, date: $datum, in: year}
"#;

#[test]
fn a_binding_passes_another_input_of_its_article_to_the_cell() {
    // The parameter of the answer is another input of the binding, which
    // the engine resolves before it asks the cell, as for a call to a law.
    let mut service = LawExecutionService::new();
    service.load_law(PEIL).unwrap();
    service.load_law(PEIL_BELEID).unwrap();
    let running = Fake::college("wilgendaal").answering("al_eerder", Value::Bool(true));
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(&running),
        decides_for: None,
        chronicle: None,
        sphere: vec![],
    };
    let result = service
        .evaluate_law_in(
            "wet_peil",
            &["al_eerder_verstrekt"],
            BTreeMap::from([("datum".to_string(), Value::String(DATE.into()))]),
            call,
            DATE,
        )
        .unwrap();
    assert_eq!(result.outputs["al_eerder_verstrekt"], Value::Bool(true));
    let asked = running.asked.borrow();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].1["jaar"], Value::Int(2026));
}

/// An answerer that says how it came to its answer, as a trace of its own.
struct Explaining(Fake);

impl OpenTermAnswerer for Explaining {
    fn authority(&self) -> &Authority {
        self.0.authority()
    }

    fn answer(
        &self,
        service: &LawExecutionService,
        request: &AnswerRequest<'_>,
    ) -> regelrecht_engine::Result<Option<Answer>> {
        let how = PathNode::new(PathNodeType::Article, "reductie_oostmeer")
            .with_node_id("n")
            .with_child(PathNode::new(PathNodeType::Operation, "PROJECTION").with_node_id("n0"));
        // Only a traced execution asks how the answer came about.
        assert!(request.trace);
        Ok(self.0.answer(service, request)?.map(|a| Answer {
            trace: Some(how),
            ..a
        }))
    }
}

#[test]
fn a_traced_execution_nests_how_the_answerer_came_to_its_answer() {
    let cell =
        Explaining(Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Bool(true)));
    let result = read("eerder_gelezen", &cell, vec![], BTreeMap::new()).unwrap();
    let input = node(result.trace.as_ref().unwrap(), "eerder_verstrekt").unwrap();
    let answer = input
        .children
        .iter()
        .find(|c| c.answer.is_some())
        .expect("the node of the answer");
    let [how] = &answer.children[..] else {
        panic!("one trace under the answer: {answer:?}");
    };
    assert_eq!(how.name, "reductie_oostmeer");
    // The ids follow the place in this trace, not the one it was built in.
    let id = answer.node_id.clone().unwrap();
    assert_eq!(how.node_id, Some(format!("{id}.0")));
    assert_eq!(how.children[0].name, "PROJECTION");
    assert_eq!(how.children[0].node_id, Some(format!("{id}.0.0")));
}

/// `output` of the reading law with `policy` as the binding, `answerer`
/// running, deciding for `decides_for`, and `sphere` as the other cells.
fn read_with(
    policy: &str,
    output: &str,
    answerer: &dyn OpenTermAnswerer,
    decides_for: Option<&Authority>,
    sphere: Vec<&dyn OpenTermAnswerer>,
) -> regelrecht_engine::Result<regelrecht_engine::ArticleResult> {
    let mut service = LawExecutionService::new();
    for law in [ESTABLISHING, READING, policy] {
        service.load_law(law).unwrap();
    }
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(answerer),
        decides_for,
        chronicle: None,
        sphere,
    };
    service.evaluate_law_with_trace_in("wet_lezer", &[output], parameters(), call, DATE)
}

#[test]
fn the_policy_of_another_authority_is_no_ground_to_use_an_answer() {
    // The RDW runs the execution; the binding is in the policy of
    // wilgendaal, and the term names no one. The RDW has no ground in a law
    // or in a policy of its own to use what the cell of wilgendaal holds
    // (RFC-056): the engine refuses before it asks that cell.
    let running = Fake::new("RDW", None);
    let holder = Fake::college("wilgendaal").answering("ander_feit", Value::Bool(true));
    let e = read_with(BINDING, "ander_gelezen", &running, None, vec![&holder]).unwrap_err();
    let message = e.to_string();
    assert!(
        message.contains(
            "beleid_verstrekking is the policy of 'wilgendaal', not of RDW, and the term names \
             no competent authority"
        ),
        "{message}"
    );
    assert!(message.contains("RFC-056"), "{message}");
    assert!(holder.asked().is_empty());
}

#[test]
fn a_law_that_gives_the_term_to_the_holder_is_the_ground() {
    // The term names the national issuer by name, and the binding is in the
    // national issuer's own policy: the law gives it the term, so the RDW
    // applies the law when it uses the answer.
    let policy = BINDING.replacen("  name: wilgendaal\n", "  name: Landelijke uitgever\n", 1);
    assert_ne!(policy, BINDING);
    let running = Fake::new("RDW", None);
    let national =
        Fake::new("Landelijke uitgever", None).answering("eerder_verstrekt", Value::Bool(true));
    let result = read_with(&policy, "eerder_gelezen", &running, None, vec![&national]).unwrap();
    assert_eq!(result.outputs["eerder_gelezen"], Value::Bool(true));
    assert_eq!(national.asked(), ["eerder_verstrekt"]);
}

#[test]
fn a_law_that_gives_the_term_to_the_holder_does_not_cover_another_competent_cell() {
    // The term names the national issuer and every college. The policy is
    // the national issuer's, but the cell the term routes to is a college:
    // the law gives the national issuer no say over that college's answer.
    let policy = BINDING.replacen("  name: wilgendaal\n", "  name: Landelijke uitgever\n", 1);
    let running = Fake::new("RDW", None);
    let college = Fake::college("oostmeer").answering("eerder_verstrekt", Value::Bool(true));
    let e = read_with(&policy, "eerder_gelezen", &running, None, vec![&college]).unwrap_err();
    let message = e.to_string();
    assert!(
        message.contains("is the policy of 'Landelijke uitgever', not of RDW"),
        "{message}"
    );
    assert!(
        message.contains("the term routes the question to oostmeer, not to the holder"),
        "{message}"
    );
    assert!(message.contains("use the answer of oostmeer"), "{message}");
    assert!(college.asked().is_empty());
}

#[test]
fn a_law_that_gives_the_term_to_the_category_of_the_holder_is_the_ground() {
    // The term names a category; the holder belongs to it, as its cell in
    // the call says.
    let running = Fake::new("RDW", None);
    let holder = Fake::college("wilgendaal").answering("eerder_verstrekt", Value::Bool(true));
    let result = read_with(BINDING, "eerder_gelezen", &running, None, vec![&holder]).unwrap();
    assert_eq!(result.outputs["eerder_gelezen"], Value::Bool(true));
    assert_eq!(holder.asked(), ["eerder_verstrekt"]);
    // Without a cell to ask, no answer reaches the RDW and nothing is
    // refused: the policy's own term then has no finding.
    let e = read_with(BINDING, "eerder_gelezen", &running, None, vec![]).unwrap_err();
    assert!(!e.to_string().contains("RFC-056"), "{e}");
    assert!(
        e.to_string().contains("eerder_verstrekt_volgens_college"),
        "{e}"
    );
}

#[test]
fn deciding_in_the_place_of_the_holder_is_the_ground() {
    // A court that decides the case itself (Awb 8:72 lid 3 onder b) takes
    // the decision of wilgendaal, with the policy of wilgendaal: it decides
    // for that college, and has the ground the college has.
    let court = Fake::new("rechtbank", Some("de bestuursrechter"));
    let college = Fake::college("wilgendaal").answering("ander_feit", Value::Bool(true));
    let result = read_with(
        BINDING,
        "ander_gelezen",
        &court,
        Some(college.authority()),
        vec![&college],
    )
    .unwrap();
    assert_eq!(result.outputs["ander_gelezen"], Value::Bool(true));
    assert_eq!(college.asked(), ["ander_feit"]);
    // Deciding for another authority gives no ground.
    let other = Authority {
        name: "oostmeer".to_string(),
        category: None,
    };
    let college = Fake::college("wilgendaal").answering("ander_feit", Value::Bool(true));
    let e = read_with(
        BINDING,
        "ander_gelezen",
        &court,
        Some(&other),
        vec![&college],
    )
    .unwrap_err();
    assert!(e.to_string().contains("RFC-056"), "{e}");
    assert!(college.asked().is_empty());
}

#[test]
fn an_answer_from_the_receivers_own_cell_needs_no_ground() {
    // Oostmeer decides under the policy of wilgendaal; the term names every
    // college, so Oostmeer's own cell is asked. Nothing crosses to another
    // cell, so no ground is needed (RFC-056).
    let running = Fake::college("oostmeer").answering("eerder_verstrekt", Value::Bool(true));
    let result = read_with(BINDING, "eerder_gelezen", &running, None, vec![]).unwrap();
    assert_eq!(result.outputs["eerder_gelezen"], Value::Bool(true));
    assert_eq!(running.asked(), ["eerder_verstrekt"]);
}

#[test]
fn a_policy_of_a_category_is_no_ground_by_law_for_one_of_its_members() {
    // The policy is held by every college at once. The term routes the
    // question to Oostmeer, a college, but that makes Oostmeer's answer no
    // holder's own: the RDW has no ground by law to use it.
    let policy = BINDING.replacen(
        "competent_authority:\n  name: wilgendaal\n",
        "competent_authority:\n  name: college van burgemeester en wethouders\n  type: CATEGORY\n",
        1,
    );
    assert_ne!(policy, BINDING);
    let running = Fake::new("RDW", None);
    let college = Fake::college("oostmeer").answering("eerder_verstrekt", Value::Bool(true));
    let e = read_with(&policy, "eerder_gelezen", &running, None, vec![&college]).unwrap_err();
    assert!(
        e.to_string()
            .contains("a policy held by a category gives none of its members the term by law"),
        "{e}"
    );
    assert!(college.asked().is_empty());
}

/// A law whose article 2 reads article 1 without passing the number that
/// article 1 takes, for
/// `an_answer_whose_parameter_is_unknown_asks_no_cell_and_is_unknown`.
const SLEUTEL: &str = r#"
$id: wet_sleutel
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Of het nummer is bevestigd.
    machine_readable:
      open_terms:
        - id: bevestigd
          type: boolean
          required: false
          decided_per_case_by: het college
      execution:
        parameters:
          - {name: nummer, type: string, required: false, nullable: true}
        output:
          - {name: bevestigd_uit, type: boolean, nullable: true}
        actions:
          - output: bevestigd_uit
            value: $bevestigd
  - number: '2'
    text: Wat artikel 1 zegt, zonder het nummer door te geven.
    machine_readable:
      execution:
        input:
          - name: gelezen
            type: boolean
            nullable: true
            source:
              output: bevestigd_uit
        output:
          - {name: gelezen_uit, type: boolean, nullable: true}
        actions:
          - output: gelezen_uit
            value: $gelezen
"#;

/// The policy that binds the term to the answer `bevestigd`, asked with the
/// number.
const SLEUTEL_BELEID: &str = r#"
$id: beleid_sleutel
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
url: https://example.org/beleid_sleutel
competent_authority:
  name: wilgendaal
articles:
  - number: '1'
    text: Wat het beleid ontleent aan het antwoord bevestigd.
    machine_readable:
      implements:
        - {law: wet_sleutel, article: '1', open_term: bevestigd}
      execution:
        parameters:
          - {name: nummer, type: string, required: false, nullable: true}
        input:
          - name: antwoord
            type: boolean
            nullable: true
            source:
              reduction: bevestigd
              parameters: {nummer: $nummer}
        output:
          - {name: bevestigd, type: boolean, nullable: true}
        actions:
          - output: bevestigd
            value: $antwoord
"#;

fn sleutel(
    output: &str,
    parameters: BTreeMap<String, Value>,
    running: &Fake,
) -> regelrecht_engine::ArticleResult {
    let mut service = LawExecutionService::new();
    service.load_law(SLEUTEL).unwrap();
    service.load_law(SLEUTEL_BELEID).unwrap();
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(running),
        decides_for: None,
        chronicle: None,
        sphere: vec![],
    };
    service
        .evaluate_law_in("wet_sleutel", &[output], parameters, call, DATE)
        .unwrap()
}

#[test]
fn an_answer_whose_parameter_is_unknown_asks_no_cell_and_is_unknown() {
    // Article 2 reads article 1 without the number, so the binding would ask
    // the cell about nobody. A cell's reduction refuses that and fails the
    // whole execution, also where this read decides nothing. As for a call
    // to another law (RFC-036), the cell is not asked, and the input is
    // unknown for the missing number.
    let running = Fake::college("wilgendaal").answering("bevestigd", Value::Bool(true));
    let result = sleutel("gelezen_uit", BTreeMap::new(), &running);
    assert!(running.asked().is_empty(), "{:?}", running.asked());
    let gelezen = &result.outputs["gelezen_uit"];
    assert!(gelezen.is_unknown(), "{gelezen:?}");
    assert!(
        gelezen
            .missing_facts()
            .iter()
            .any(|fact| fact.name == "nummer"),
        "{gelezen:?}"
    );

    // With the number, the cell is asked and its answer counts.
    let running = Fake::college("wilgendaal").answering("bevestigd", Value::Bool(true));
    let result = sleutel(
        "bevestigd_uit",
        BTreeMap::from([("nummer".to_string(), Value::String("12345678".into()))]),
        &running,
    );
    assert_eq!(result.outputs["bevestigd_uit"], Value::Bool(true));
    assert_eq!(running.asked(), ["bevestigd"]);
}

/// A law whose article leaves three terms to three authorities: the
/// register says `leden`, the office says `ontvangen`, and `korting` names
/// no one, so the holder of the policy that fills it says it.
const THREE_TERMS: &str = r#"
$id: wet_drie_termen
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het bedrag hangt af van wat het register, het bureau en de houder zeggen.
    machine_readable:
      open_terms:
        - id: leden
          competent_authority: {name: register, type: CATEGORY}
          type: number
          required: true
        - id: ontvangen
          competent_authority: {name: bureau, type: CATEGORY}
          type: number
          required: true
        - id: korting
          type: number
          required: true
      execution:
        parameters:
          - {name: bsn, type: string, required: true}
        output:
          - {name: bedrag, type: number}
        actions:
          - output: bedrag
            value:
              operation: SUBTRACT
              values: [$leden, $ontvangen, $korting]
"#;

/// The policy of wilgendaal: one article fills all three terms, each from
/// an answer of its own, and counts 0 where the cell is silent. `ontvangen`
/// reads its answer through a helper output, and the helper output
/// `opmerking` feeds no term. The input of `opmerking` passes the discount's
/// answer on as a parameter: that input reads the discount's input, and no
/// term reads it, so the discount's input feeds only the discount.
const ONE_BINDING: &str = r#"
$id: beleid_drie_termen
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
url: https://example.org/beleid
competent_authority:
  name: wilgendaal
articles:
  - number: '1'
    text: Wat het beleid ontleent aan de drie antwoorden.
    machine_readable:
      implements:
        - {law: wet_drie_termen, article: '1', open_term: leden}
        - {law: wet_drie_termen, article: '1', open_term: ontvangen}
        - {law: wet_drie_termen, article: '1', open_term: korting}
      execution:
        parameters:
          - {name: bsn, type: string}
        input:
          - name: antwoord_leden
            type: number
            nullable: true
            source: {reduction: leden, parameters: {bsn: $bsn}}
          - name: antwoord_ontvangen
            type: number
            nullable: true
            source: {reduction: ontvangen, parameters: {bsn: $bsn}}
          - name: antwoord_korting
            type: number
            nullable: true
            source: {reduction: korting, parameters: {bsn: $bsn}}
          - name: antwoord_opmerking
            type: string
            nullable: true
            source: {reduction: opmerking, parameters: {bsn: $bsn, korting: $antwoord_korting}}
        output:
          - {name: leden, type: number}
          - {name: tussenstand, type: number, nullable: true}
          - {name: ontvangen, type: number}
          - {name: korting, type: number}
          - {name: opmerking, type: string, nullable: true}
        actions:
          - output: leden
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $antwoord_leden, value: null}
                  then: 0
              default: $antwoord_leden
          - {output: tussenstand, value: $antwoord_ontvangen}
          - output: ontvangen
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $tussenstand, value: null}
                  then: 0
              default: $tussenstand
          - output: korting
            value:
              operation: IF
              cases:
                - when: {operation: EQUALS, subject: $antwoord_korting, value: null}
                  then: 0
              default: $antwoord_korting
          - {output: opmerking, value: $antwoord_opmerking}
"#;

/// `output` of `law` with `policy` loaded beside [`THREE_TERMS`], run by
/// `answerer` with `sphere` along, traced.
fn read_three(
    policy: &str,
    law: &str,
    output: &str,
    answerer: &dyn OpenTermAnswerer,
    sphere: Vec<&dyn OpenTermAnswerer>,
) -> regelrecht_engine::Result<regelrecht_engine::ArticleResult> {
    let mut service = LawExecutionService::new();
    service.load_law(THREE_TERMS).unwrap();
    service.load_law(policy).unwrap();
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(answerer),
        decides_for: None,
        chronicle: None,
        sphere,
    };
    service.evaluate_law_with_trace_in(law, &[output], parameters(), call, DATE)
}

fn three_cells() -> (Fake, Fake, Fake) {
    (
        Fake::new("wilgendaal", None).answering("korting", Value::Int(1)),
        Fake::new("oostmeer", Some("register")).answering("leden", Value::Int(10)),
        Fake::new("zuidpolder", Some("bureau")).answering("ontvangen", Value::Int(3)),
    )
}

#[test]
fn one_binding_asks_each_term_of_the_cell_that_term_names() {
    // One article fills three terms. Each input goes to the cell of the term
    // it feeds: the register, the office, and for the term that names no
    // one, the holder. `ontvangen` reads its answer through a helper output.
    let (running, register, office) = three_cells();
    let result = read_three(
        ONE_BINDING,
        "wet_drie_termen",
        "bedrag",
        &running,
        vec![&register, &office],
    )
    .unwrap();
    assert_eq!(result.outputs["bedrag"], Value::Int(6));
    assert_eq!(register.asked(), ["leden"]);
    assert_eq!(office.asked(), ["ontvangen"]);
    assert_eq!(running.asked(), ["korting"]);
    for (term, answerer) in [
        ("leden", "oostmeer"),
        ("ontvangen", "zuidpolder"),
        ("korting", "wilgendaal"),
    ] {
        let FilledBy::Answer { answers, .. } = filled_by(&result, term) else {
            panic!("{term} is filled by an answer");
        };
        assert_eq!(answers, [provenance(answerer, term)], "{term}");
    }
}

/// [`ONE_BINDING`] where the member count is computed in a helper output,
/// `ledenhulp`, that the term `leden` takes over.
fn with_member_helper() -> String {
    let policy = ONE_BINDING
        .replace(
            "          - output: leden\n            value:\n              operation: IF\n",
            "          - {output: leden, value: $ledenhulp}\n          - output: ledenhulp\n            value:\n              operation: IF\n",
        )
        .replace(
            "          - {name: leden, type: number}\n",
            "          - {name: leden, type: number}\n          - {name: ledenhulp, type: number}\n",
        );
    assert_eq!(policy.matches("ledenhulp").count(), 3, "{policy}");
    policy
}

/// Run [`read_three`] for `bedrag` with `policy` and [`three_cells`], and
/// return the error and whether the register was asked.
fn bedrag_error(policy: &str) -> (String, Vec<String>) {
    let (running, register, office) = three_cells();
    let e = read_three(
        policy,
        "wet_drie_termen",
        "bedrag",
        &running,
        vec![&register, &office],
    )
    .unwrap_err();
    (e.to_string(), register.asked())
}

#[test]
fn an_input_that_feeds_terms_of_different_authorities_is_an_error() {
    // The discount also reads the helper that holds the member count, so the
    // register's input feeds a term of the register and, through the helper,
    // a term of the holder. One input asks one cell, so the engine asks none
    // and says why.
    let policy = with_member_helper().replace(
        "              default: $antwoord_korting\n",
        "              default: {operation: ADD, values: [$antwoord_korting, $ledenhulp]}\n",
    );
    let (message, asked) = bedrag_error(&policy);
    assert!(
        message.contains("input 'antwoord_leden' of beleid_drie_termen takes the answer 'leden'"),
        "{message}"
    );
    assert!(
        message.contains("'leden' to 'register', 'korting' to 'wilgendaal'"),
        "{message}"
    );
    assert!(asked.is_empty(), "{asked:?}");
}

#[test]
fn an_input_passed_as_a_parameter_feeds_the_term_of_the_other_input() {
    // The office's answer now takes the member count as a parameter, so the
    // register's input also feeds the office's term. That too is one input
    // for two authorities.
    let policy = ONE_BINDING.replace(
        "source: {reduction: ontvangen, parameters: {bsn: $bsn}}",
        "source: {reduction: ontvangen, parameters: {bsn: $bsn, leden: $antwoord_leden}}",
    );
    assert_ne!(policy, ONE_BINDING);
    let (message, asked) = bedrag_error(&policy);
    assert!(
        message.contains("'leden' to 'register', 'ontvangen' to 'bureau'"),
        "{message}"
    );
    assert!(asked.is_empty(), "{asked:?}");
}

#[test]
fn a_term_that_builds_on_another_term_takes_its_value_not_its_input() {
    // The discount adds the member count, the output of the term `leden`.
    // That term was answered by the register; the discount takes its value,
    // so the register's input still feeds only `leden` and goes to the
    // register.
    let policy = ONE_BINDING.replace(
        "              default: $antwoord_korting\n",
        "              default: {operation: ADD, values: [$antwoord_korting, $leden]}\n",
    );
    assert_ne!(policy, ONE_BINDING);
    let (running, register, office) = three_cells();
    let result = read_three(
        &policy,
        "wet_drie_termen",
        "bedrag",
        &running,
        vec![&register, &office],
    )
    .unwrap();
    // 10 - 3 - (1 + 10)
    assert_eq!(result.outputs["bedrag"], Value::Int(-4));
    // The binding computes `leden` again for the discount, so the register
    // may be asked once per term; it is only ever asked for `leden`.
    let asked = register.asked();
    assert!(
        !asked.is_empty() && asked.iter().all(|a| a == "leden"),
        "{asked:?}"
    );
    assert_eq!(running.asked(), ["korting"]);
}

#[test]
fn an_input_that_feeds_no_term_asks_the_holder() {
    // `opmerking` is an output of the binding that fills no term. Its input
    // asks the holder of the policy, as for a term that names no one; the
    // register and the office are not asked. The holder is asked for the
    // discount first, which the input passes on.
    let (running, register, office) = three_cells();
    let running = running.answering("opmerking", Value::String("geen".into()));
    let result = read_three(
        ONE_BINDING,
        "beleid_drie_termen",
        "opmerking",
        &running,
        vec![&register, &office],
    )
    .unwrap();
    assert_eq!(result.outputs["opmerking"], Value::String("geen".into()));
    assert_eq!(running.asked(), ["korting", "opmerking"]);
    assert!(register.asked().is_empty() && office.asked().is_empty());
}

#[test]
fn an_answer_no_cell_gives_is_reported_with_the_term() {
    // Without the holder's answer the discount falls back to 0, as the
    // policy says. The report of the term says which answer was missing.
    let (_, register, office) = three_cells();
    let running = Fake::new("wilgendaal", None);
    let result = read_three(
        ONE_BINDING,
        "wet_drie_termen",
        "bedrag",
        &running,
        vec![&register, &office],
    )
    .unwrap();
    assert_eq!(result.outputs["bedrag"], Value::Int(7));
    let FilledBy::Answer {
        answers,
        unanswered,
        ..
    } = filled_by(&result, "korting")
    else {
        panic!("korting is filled by an answer");
    };
    assert!(answers.is_empty(), "{answers:?}");
    let [missing] = unanswered.as_slice() else {
        panic!("one answer is missing: {unanswered:?}");
    };
    assert_eq!(
        (missing.answer.as_str(), missing.input.as_str()),
        ("korting", "antwoord_korting")
    );
    assert!(
        missing.message.contains("silent: wilgendaal"),
        "{}",
        missing.message
    );
    // The terms that were answered report no missing answer.
    let FilledBy::Answer { unanswered, .. } = filled_by(&result, "leden") else {
        panic!("leden is filled by an answer");
    };
    assert!(unanswered.is_empty(), "{unanswered:?}");
}

#[test]
fn the_ground_for_an_answer_follows_the_term_its_input_feeds() {
    // The register's term now names the holder, wilgendaal, by name, and
    // zuidmeer runs the execution. The law gives that term to the holder,
    // so zuidmeer may use wilgendaal's answer (RFC-056). The office's term
    // routes to the office, which the law does not give the holder: that
    // input has no ground, and the execution fails on it.
    let law = THREE_TERMS.replacen("{name: register, type: CATEGORY}", "{name: wilgendaal}", 1);
    assert_ne!(law, THREE_TERMS);
    let mut service = LawExecutionService::new();
    service.load_law(&law).unwrap();
    service.load_law(ONE_BINDING).unwrap();
    let running = Fake::new("zuidmeer", None);
    let holder = Fake::new("wilgendaal", None)
        .answering("leden", Value::Int(10))
        .answering("korting", Value::Int(1));
    let office = Fake::new("zuidpolder", Some("bureau")).answering("ontvangen", Value::Int(3));
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(&running),
        decides_for: None,
        chronicle: None,
        sphere: vec![&holder, &office],
    };
    let leden = service
        .evaluate_law_in("beleid_drie_termen", &["leden"], parameters(), call, DATE)
        .unwrap();
    assert_eq!(leden.outputs["leden"], Value::Int(10));
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(&running),
        decides_for: None,
        chronicle: None,
        sphere: vec![&holder, &office],
    };
    let e = service
        .evaluate_law_in(
            "beleid_drie_termen",
            &["ontvangen"],
            parameters(),
            call,
            DATE,
        )
        .unwrap_err();
    let message = e.to_string();
    assert!(
        message.contains("the term routes the question to zuidpolder, not to the holder"),
        "{message}"
    );
    assert!(office.asked().is_empty());
}

#[test]
fn a_term_that_names_no_one_lends_an_input_no_ground() {
    // The register's term names the holder, wilgendaal, by name; the
    // discount names no one and reads the same helper. Both route to
    // wilgendaal's cell, but only one of the two terms is given to the holder
    // by law. Zuidmeer runs the execution and has no policy of its own here,
    // so the input has no ground (RFC-056) and wilgendaal is not asked.
    let law = THREE_TERMS.replacen("{name: register, type: CATEGORY}", "{name: wilgendaal}", 1);
    assert_ne!(law, THREE_TERMS);
    let policy = with_member_helper().replace(
        "              default: $antwoord_korting\n",
        "              default: {operation: ADD, values: [$antwoord_korting, $ledenhulp]}\n",
    );
    let mut service = LawExecutionService::new();
    service.load_law(&law).unwrap();
    service.load_law(&policy).unwrap();
    let running = Fake::new("zuidmeer", None);
    let holder = Fake::new("wilgendaal", None).answering("leden", Value::Int(10));
    let call = Call {
        findings: BTreeMap::new(),
        answerer: Some(&running),
        decides_for: None,
        chronicle: None,
        sphere: vec![&holder],
    };
    let e = service
        .evaluate_law_in("beleid_drie_termen", &["leden"], parameters(), call, DATE)
        .unwrap_err();
    let message = e.to_string();
    assert!(
        message.contains("input 'antwoord_leden'") && message.contains("RFC-056"),
        "{message}"
    );
    assert!(holder.asked().is_empty(), "{:?}", holder.asked());
}

#[test]
fn an_input_named_like_its_term_feeds_only_that_term() {
    // The register's input carries the name of its term (`leden: $leden`),
    // and the discount builds on that term. The discount takes the term's
    // value, so the input still feeds only `leden` and goes to the register.
    // The input is declared never absent and passed through as is
    // (`leden: $leden`), so the type check reads `$leden` as a number.
    let policy = ONE_BINDING
        .replace("antwoord_leden", "leden")
        .replace(
            "          - name: leden\n            type: number\n            nullable: true\n",
            "          - name: leden\n            type: number\n",
        )
        .replace(
            "          - output: leden\n            value:\n              operation: IF\n              cases:\n                - when: {operation: EQUALS, subject: $leden, value: null}\n                  then: 0\n              default: $leden\n",
            "          - {output: leden, value: $leden}\n",
        )
        .replace(
            "              default: $antwoord_korting\n",
            "              default: {operation: ADD, values: [$antwoord_korting, $leden]}\n",
        );
    assert!(
        policy.contains("{output: leden, value: $leden}")
            && policy.contains("[$antwoord_korting, $leden]")
    );
    let (running, register, office) = three_cells();
    let result = read_three(
        &policy,
        "wet_drie_termen",
        "bedrag",
        &running,
        vec![&register, &office],
    )
    .unwrap();
    // 10 - 3 - (1 + 10)
    assert_eq!(result.outputs["bedrag"], Value::Int(-4));
    assert!(!register.asked().is_empty());
    assert_eq!(running.asked(), ["korting"]);
}
