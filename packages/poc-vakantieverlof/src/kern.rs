//! What the app does with the cell: the process steps and the reduction,
//! apart from HTTP.
//!
//! Every step builds a fresh engine service from the regelwerken in the
//! corpus. That costs a few milliseconds and keeps the steps independent:
//! the cell loads its reductions into the service it gets for a decision,
//! and nothing of that may reach the next step.

use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use regelrecht_cel::{decision, reading, shape, Cell, Error, Gram, Source};
use regelrecht_engine::{ArticleBasedLaw, LawExecutionService, LawLoad, ParameterType, PathNode};
use regelrecht_law_model::stored;
use serde_json::{json, Map, Value};

/// The law and article of the leave, and its decision.
pub const LAW: &str = "leerplichtwet_1969";
pub const ARTICLE: &str = "13a";
/// The Awb, whose art. 7:11 establishes the objection and decides on it.
pub const AWB: &str = "algemene_wet_bestuursrecht";
pub const OBJECTION: &str = "7:11";
/// The fictional school whose cell this is.
pub const SCHOOL: &str = "obs_de_klimop";

/// The regelwerken the cell runs on, as directories under
/// `corpus/regulation/nl`: the law, the Awb and the school's two fictitious
/// policies (the start of its school year, and in which school year a day
/// falls).
const REGELWERKEN: [&str; 4] = [
    "wet/leerplichtwet_1969",
    "wet/algemene_wet_bestuursrecht",
    "uitvoeringsbeleid/fictief_beleid_schooljaar_obs_de_klimop",
    "uitvoeringsbeleid/fictief_beleid_schooljaargrenzen_obs_de_klimop",
];

pub type Result<T> = std::result::Result<T, Error>;

/// The regelwerken (each version as its YAML) and the cell's directory,
/// read once at startup.
#[derive(Debug, Clone)]
pub struct Corpus {
    versies: Vec<String>,
    cell_dir: PathBuf,
}

impl Corpus {
    /// Read the regelwerken under `root/regulation/nl` and find the cell
    /// at `root/cells/<SCHOOL>`. `root` is the repository's `corpus/`.
    pub fn load(root: &Path) -> Result<Self> {
        let mut versies = Vec::new();
        for dir in REGELWERKEN {
            let dir = root.join("regulation/nl").join(dir);
            let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
                .collect();
            paths.sort();
            for path in paths {
                let yaml = std::fs::read_to_string(&path)?;
                // Parsed here, so a broken version names its file at startup.
                ArticleBasedLaw::from_yaml_str(&yaml)
                    .map_err(|e| Error::Setup(format!("{}: {e}", path.display())))?;
                versies.push(yaml);
            }
        }
        Ok(Self {
            versies,
            cell_dir: root.join("cells").join(SCHOOL),
        })
    }

    /// The school's cell, with an empty chronicle and the reductions in its
    /// directory.
    pub fn cell(&self) -> Result<Cell> {
        Cell::in_memory(SCHOOL, &self.cell_dir)
    }

    /// A service with every regelwerk.
    pub fn service(&self) -> Result<LawExecutionService> {
        let mut service = LawExecutionService::new();
        for yaml in &self.versies {
            service.load_law(yaml)?;
        }
        Ok(service)
    }
}

/// A field as the forms show it.
fn veld(field: &shape::Field) -> Value {
    json!({
        "naam": field.name,
        "type": field.type_,
        "mag_leeg": field.nullable,
        "grondslag": field.first_declared_by(),
    })
}

/// The fields of a submission that its submitter fills: not a field of
/// type `decision`, which the cell fills from the decision it refers to.
fn in_te_vullen(shape: &shape::Shape) -> Vec<Value> {
    shape
        .fields
        .iter()
        .filter(|f| f.type_ != ParameterType::Decision)
        .map(veld)
        .collect()
}

/// What the law asks on `on`: the fields of the application, the head's
/// findings on it and the fields of the objection. All of it is derived
/// from the law, for this cell: a term the cell answers from its chronicle
/// is no finding of the head, so the form does not ask it. An objection
/// does not change the application; a smaller request is a new
/// application. What the head finds anew on an objection follows from the
/// contested decision ([`heroverweging`]).
pub fn vormen(corpus: &Corpus, cell: &Cell, on: NaiveDate) -> Result<Value> {
    let service = corpus.service()?;
    let beantwoord = reading::terms_on(cell.reductions(), on);
    let aanvraag = shape::derive(&service, LAW, ARTICLE, on)?;
    let besluit = decision::derive_with(&service, LAW, ARTICLE, on, &beantwoord)?;
    let bezwaar = shape::derive(&service, AWB, OBJECTION, on)?;
    let op_bezwaar = decision::derive(&service, AWB, OBJECTION, on)?;
    let moment = |s: &shape::Shape| {
        s.moment
            .as_ref()
            .map(|m| json!({ "parameter": m.parameter, "grondslag": m.declared_by }))
    };
    let bevindingen: Vec<Value> = besluit
        .inputs
        .iter()
        .filter(|i| besluit.judgements.contains(&i.field.name))
        .map(|i| veld(&i.field))
        .collect();
    let herkomst: Vec<Value> = besluit
        .inputs
        .iter()
        .map(|i| {
            let door_cel = beantwoord.iter().any(|t| t.open_term == i.field.name);
            json!({
                "naam": i.field.name,
                "bron": match i.source {
                    _ if door_cel => "kroniek van de school",
                    Source::Application => "aanvraag",
                    Source::Caller => "hoofd",
                    Source::Cell => "cel",
                    Source::Policy => "beleid",
                },
            })
        })
        .collect();
    Ok(json!({
        "aanvraag": {
            "soort": aanvraag.kind,
            "grondslag": aanvraag.grondslag(),
            "velden": in_te_vullen(&aanvraag),
            "moment": moment(&aanvraag),
        },
        "besluit": {
            "soort": besluit.kind,
            "grondslag": besluit.grondslag(),
            "bevindingen": bevindingen,
            "invoer": herkomst,
            "uitvoer": besluit.outputs,
        },
        "bezwaar": {
            "soort": bezwaar.kind,
            "grondslag": bezwaar.grondslag(),
            "velden": in_te_vullen(&bezwaar),
            "moment": moment(&bezwaar),
        },
        "beslissing_op_bezwaar": {
            "soort": op_bezwaar.kind,
            "grondslag": op_bezwaar.grondslag(),
            "uitvoer": op_bezwaar.outputs,
        },
    }))
}

/// The parent applies: record the application of art. 13a, received on `on`.
pub fn aanvragen(
    corpus: &Corpus,
    cell: &mut Cell,
    velden: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Gram> {
    cell.record_submission(&mut corpus.service()?, LAW, ARTICLE, None, velden, on)
}

/// Why a decision came out as it did, as the cell explains it
/// ([`Cell::explain`]): the engine's trace, every open term with who filled
/// it, the terms the cell answered from its chronicle (one trace node
/// each) and the findings of the head the decision used
/// ([`Cell::findings`]).
#[derive(Debug, serde::Serialize)]
pub struct Waarom {
    pub spoor: String,
    /// The engine's report per open term (`ArticleResult::open_terms`):
    /// a finding, a regulation, or the cell's answer.
    pub ingevuld: Vec<regelrecht_engine::OpenTermFilled>,
    pub antwoorden: Vec<Value>,
    pub bevindingen: Vec<String>,
}

/// The nodes of `node` and below where the cell answered an open term.
fn beantwoord(node: &PathNode) -> Vec<&PathNode> {
    let mut gevonden: Vec<_> = node.children.iter().flat_map(beantwoord).collect();
    if node.answer.is_some() {
        gevonden.insert(0, node);
    }
    gevonden
}

/// Why the decision `id` came out as it did ([`Waarom`]).
pub fn waarom(corpus: &Corpus, cell: &Cell, id: &str) -> Result<Waarom> {
    let mut service = corpus.service()?;
    let result = cell.explain(&mut service, id)?;
    let mut antwoorden = Vec::new();
    if let Some(root) = &result.trace {
        for node in beantwoord(root) {
            let Some(answer) = &node.answer else {
                continue;
            };
            let waarde = node.result.as_ref().map(stored::to_stored).transpose()?;
            antwoorden.push(json!({
                "begrip": node.name,
                "waarde": waarde,
                "door": answer.answerer,
                "reductie": answer.law_id,
                "geldig_vanaf": answer.valid_from,
                "gelezen": answer.records_read,
                "tot_en_met": answer.last_record,
            }));
        }
    }
    Ok(Waarom {
        spoor: result
            .trace
            .as_ref()
            .map(PathNode::render_box_drawing)
            .unwrap_or_default(),
        ingevuld: result.open_terms,
        antwoorden,
        bevindingen: cell.findings(&mut service, id)?,
    })
}

/// The output names of the version of the reduction `id` of the cell that
/// is valid from `valid_from`, as written.
fn uitvoer_van(cell: &Cell, id: &str, valid_from: Option<&str>) -> Vec<String> {
    let mut namen = Vec::new();
    let versie = cell
        .reductions()
        .iter()
        .filter(|r| r.id() == id)
        .filter(|r| valid_from.is_none_or(|v| r.valid_from().to_string() == v));
    for reduction in versie {
        for article in &reduction.law().articles {
            let outputs = article
                .get_execution_spec()
                .and_then(|e| e.output.as_ref())
                .into_iter()
                .flatten();
            for output in outputs {
                if !namen.contains(&output.name) {
                    namen.push(output.name.clone());
                }
            }
        }
    }
    namen
}

/// The first value the trace under `node` gives each of `namen`.
fn eerste_waarden(node: &PathNode, namen: &[String], uit: &mut Map<String, Value>) -> Result<()> {
    if let Some(waarde) = &node.result {
        if namen.contains(&node.name) && !uit.contains_key(&node.name) {
            uit.insert(node.name.clone(), stored::to_stored(waarde)?);
        }
    }
    for child in &node.children {
        eerste_waarden(child, namen, uit)?;
    }
    Ok(())
}

/// How the cell came to its answer on the open term `begrip` in the
/// decision `id` ([`Cell::explain_answer`]): per time the decision asked
/// it, the reduction and its version, what its outputs came to (the
/// answer, and an output it computes on the way, such as the revoked
/// decisions), and its own trace.
pub fn hoe_beantwoord(corpus: &Corpus, cell: &Cell, id: &str, begrip: &str) -> Result<Value> {
    let mut service = corpus.service()?;
    let mut uit = Vec::new();
    for result in cell.explain_answer(&mut service, id, begrip)? {
        let mut uitvoer = Map::new();
        if let Some(root) = &result.trace {
            let namen = uitvoer_van(
                cell,
                &result.law_id,
                result.regulation_valid_from.as_deref(),
            );
            eerste_waarden(root, &namen, &mut uitvoer)?;
        }
        for (naam, waarde) in &result.outputs {
            uitvoer.insert(naam.clone(), stored::to_stored(waarde)?);
        }
        uit.push(json!({
            "reductie": result.law_id,
            "geldig_vanaf": result.regulation_valid_from,
            "uitvoer": uitvoer,
            "spoor": result
                .trace
                .as_ref()
                .map(PathNode::render_box_drawing)
                .unwrap_or_default(),
        }));
    }
    Ok(json!({ "besluit": id, "begrip": begrip, "antwoorden": uit }))
}

/// A decision under art. 13a, and why it came out so.
#[derive(Debug, serde::Serialize)]
pub struct Besluit {
    pub besluit: Gram,
    pub waarom: Waarom,
}

/// Decide on the submission `op` under `law_id#article` with the head's
/// `bevindingen` (RFC-050): the decision on it first, then a decision it
/// takes in turn.
fn beslissen(
    corpus: &Corpus,
    cell: &mut Cell,
    law_id: &str,
    article: &str,
    op: &str,
    bevindingen: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Vec<Gram>> {
    let mut service = corpus.service()?;
    cell.decide(
        &mut service,
        law_id,
        article,
        op,
        &Map::new(),
        bevindingen,
        on,
    )
}

/// The head decides on the application `aanvraag` with its findings.
pub fn besluiten(
    corpus: &Corpus,
    cell: &mut Cell,
    aanvraag: &str,
    bevindingen: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Besluit> {
    let besluit = beslissen(corpus, cell, LAW, ARTICLE, aanvraag, bevindingen, on)?.remove(0);
    let waarom = waarom(corpus, cell, &besluit.id)?;
    Ok(Besluit { besluit, waarom })
}

/// The parent objects to the decision `besluit`: a submission under Awb
/// 7:11 that refers to it (RFC-050).
pub fn bezwaren(
    corpus: &Corpus,
    cell: &mut Cell,
    besluit: &str,
    velden: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Gram> {
    cell.record_submission(
        &mut corpus.service()?,
        AWB,
        OBJECTION,
        Some(besluit),
        velden,
        on,
    )
}

/// Before the head decides on the objection `bezwaar` on `on`: whether it
/// is admissible, as Awb 7:11 decides it (on time per 6:9, and not against
/// a decision taken on objection), and, if so, what the head can find anew
/// at the heroverweging. Those are the findings the contested decision
/// used ([`Cell::findings`]), with what the head found then. A term the
/// cell answered is none of them.
///
/// The admissibility is that of the decision without new findings, taken
/// on a copy of the cell: the law decides it, not the app. Nothing is kept.
pub fn heroverweging(corpus: &Corpus, cell: &Cell, bezwaar: &str, on: NaiveDate) -> Result<Value> {
    let toets = beslissen(
        corpus,
        &mut cell.sandbox(),
        AWB,
        OBJECTION,
        bezwaar,
        &Map::new(),
        on,
    )?
    .remove(0);
    let ontvankelijk = toets.outputs.get("bezwaar_ontvankelijk") == Some(&json!(true));
    let ingediend = cell
        .grams()
        .iter()
        .find(|g| g.id == bezwaar)
        .ok_or_else(|| Error::Refused(format!("there is no objection '{bezwaar}'")))?;
    let bestreden = ingediend
        .refers_to
        .as_deref()
        .and_then(|id| cell.grams().iter().find(|g| g.id == id))
        .ok_or_else(|| Error::Refused(format!("'{bezwaar}' contests no decision")))?;
    let mut bevindingen = Vec::new();
    if ontvankelijk {
        let (law_id, article) = bestreden
            .grondslag
            .split_once('#')
            .ok_or_else(|| Error::Setup(format!("'{}' has no grondslag", bestreden.id)))?;
        let mut service = corpus.service()?;
        let shape = decision::derive(&service, law_id, article, bestreden.recorded_on)?;
        for naam in cell.findings(&mut service, &bestreden.id)? {
            let Some(input) = shape.input(&naam) else {
                continue;
            };
            let mut v = veld(&input.field);
            v["eerder"] = bestreden.fields.get(&naam).cloned().unwrap_or(Value::Null);
            bevindingen.push(v);
        }
    }
    Ok(json!({
        "bezwaar": bezwaar,
        "bestreden_besluit": bestreden.id,
        "ontvankelijk": ontvankelijk,
        "tijdig": ingediend.outputs.get("bezwaarschrift_tijdig_ingediend"),
        "uitvoer": toets.outputs,
        "bevindingen": bevindingen,
    }))
}

/// The school decides on the objection `bezwaar` under Awb 7:11, as any
/// decision; `bevindingen` are what the head finds anew at the
/// heroverweging. The decision comes with why ([`waarom`]): art. 13a
/// called anew is in its trace, a new decision in place of the revoked one
/// is its outcome.
pub fn beslissen_op_bezwaar(
    corpus: &Corpus,
    cell: &mut Cell,
    bezwaar: &str,
    bevindingen: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Value> {
    let mut genomen = beslissen(corpus, cell, AWB, OBJECTION, bezwaar, bevindingen, on)?;
    let nieuw = (genomen.len() > 1).then(|| genomen.remove(1));
    let beslissing = genomen.remove(0);
    let waarom = waarom(corpus, cell, &beslissing.id)?;
    Ok(json!({
        "beslissing": beslissing,
        "nieuw_besluit": nieuw,
        "waarom": waarom,
    }))
}

/// The open terms the cell answers from its chronicle ([`Cell::answers`]),
/// its reductions as written, and per reduction the decisions it reads
/// from the chronicle (`source.decisions`), as rows.
pub fn reducties(corpus: &Corpus, cell: &Cell, on: NaiveDate) -> Result<Value> {
    let service = corpus.service()?;
    let mut uit = Vec::new();
    for reduction in cell.reductions() {
        let mut invoer = Vec::new();
        for input in reduction.law().articles.iter().flat_map(|a| a.get_inputs()) {
            let Some(grondslag) = input.source.as_ref().and_then(|s| s.decisions.clone()) else {
                continue;
            };
            let (law_id, article) = grondslag
                .split_once('#')
                .ok_or_else(|| Error::Setup(format!("'{grondslag}' names no article")))?;
            let kind = decision::derive(&service, law_id, article, on)?.kind;
            let rijen = reading::rows(cell.grams(), &grondslag, &kind)?;
            invoer.push(json!({
                "naam": input.name,
                "besluiten_onder": grondslag,
                "rijen": serde_json::to_value(&rijen).map_err(|e| Error::Setup(e.to_string()))?,
            }));
        }
        uit.push(json!({
            "id": reduction.id(),
            "geldig_vanaf": reduction.valid_from(),
            "yaml": reduction.yaml(),
            "invoer": invoer,
        }));
    }
    let antwoorden =
        serde_json::to_value(cell.answers()).map_err(|e| Error::Setup(e.to_string()))?;
    Ok(json!({ "antwoorden": antwoorden, "reducties": uit }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn corpus() -> Corpus {
        Corpus::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus")).unwrap()
    }

    fn day(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn map(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    fn aanvraag(laatste: &str) -> Map<String, Value> {
        map(json!({
            "bsn_jongere": "999990019",
            "eerste_verlofdag": "2027-02-01",
            "laatste_verlofdag": laatste,
            "aanvraag_ondertekend": true,
            "naam_aanvrager": "J. Jansen",
            "adres_aanvrager": "Voorbeeldstraat 1",
            "dagtekening": "2026-10-12",
            "gevraagde_beschikking": "vakantieverlof",
        }))
    }

    fn bevindingen() -> Map<String, Value> {
        map(json!({
            "specifieke_aard_beroep_ouder": true,
            "verlof_valt_buiten_schoolvakanties": true,
        }))
    }

    fn bezwaar_velden(dagtekening: &str) -> Map<String, Value> {
        map(json!({
            "bezwaarschrift_ondertekend": true,
            "naam_indiener": "J. Jansen",
            "adres_indiener": "Voorbeeldstraat 1",
            "dagtekening_bezwaarschrift": dagtekening,
            "omschrijving_bestreden_besluit": "weigering vakantieverlof",
            "gronden_bezwaar": "verklaring van de werkgever: geen verlof in de schoolvakanties",
        }))
    }

    fn beroep(oordeel: bool) -> Map<String, Value> {
        map(json!({ "specifieke_aard_beroep_ouder": oordeel }))
    }

    /// Five school days, applied for on 13 October and refused on 20
    /// October: the head judged the profession of a parent not of the kind
    /// art. 11 onder f asks.
    fn geweigerd(corpus: &Corpus, cell: &mut Cell) -> Besluit {
        let app = aanvragen(corpus, cell, &aanvraag("2027-02-05"), day("2026-10-13")).unwrap();
        let geweigerd = map(json!({
            "specifieke_aard_beroep_ouder": false,
            "verlof_valt_buiten_schoolvakanties": true,
        }));
        besluiten(corpus, cell, &app.id, &geweigerd, day("2026-10-20")).unwrap()
    }

    #[test]
    fn the_forms_come_from_the_law() {
        let corpus = corpus();
        let vormen = vormen(&corpus, &corpus.cell().unwrap(), day("2026-10-14")).unwrap();
        let namen = |pad: &str| -> Vec<String> {
            vormen
                .pointer(pad)
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v["naam"].as_str().unwrap().to_string())
                .collect()
        };
        assert!(namen("/aanvraag/velden").contains(&"laatste_verlofdag".to_string()));
        // The cell answers verlof_al_verleend_in_schooljaar from its
        // chronicle, so the head is not asked it.
        assert_eq!(
            namen("/besluit/bevindingen"),
            [
                "specifieke_aard_beroep_ouder",
                "verlof_valt_buiten_schoolvakanties"
            ]
        );
        let invoer = vormen["besluit"]["invoer"].as_array().unwrap();
        assert!(invoer.contains(
            &json!({ "naam": "verlof_al_verleend_in_schooljaar", "bron": "kroniek van de school" })
        ));
        let bezwaar = namen("/bezwaar/velden");
        assert!(bezwaar.contains(&"gronden_bezwaar".to_string()));
        // The contested decision the cell fills from `refers_to`.
        assert!(!bezwaar.contains(&"bestreden_besluit".to_string()));
        assert!(vormen["beslissing_op_bezwaar"]["uitvoer"]
            .as_array()
            .unwrap()
            .contains(&json!("bestreden_besluit_herroepen")));
    }

    #[test]
    fn the_whole_process_plays_through() {
        let corpus = corpus();
        let mut cell = corpus.cell().unwrap();
        let besluit = geweigerd(&corpus, &mut cell);
        assert_eq!(besluit.besluit.outputs["verlof_verleend"], json!(false));
        let uitleg = &besluit.waarom;
        assert!(uitleg.spoor.contains("verlof_verleend"), "{}", uitleg.spoor);
        // The profession already refuses the leave, so art. 13a never
        // reads the term the cell answers; the head's findings are what the
        // decision used.
        assert!(uitleg.antwoorden.is_empty(), "{:?}", uitleg.antwoorden);
        assert_eq!(
            uitleg.bevindingen,
            [
                "specifieke_aard_beroep_ouder",
                "verlof_valt_buiten_schoolvakanties"
            ]
        );

        // An objection does not change the application.
        let mut gewijzigd = bezwaar_velden("2026-11-01");
        gewijzigd.insert("laatste_verlofdag".into(), json!("2027-02-03"));
        let e = bezwaren(
            &corpus,
            &mut cell,
            &besluit.besluit.id,
            &gewijzigd,
            day("2026-11-01"),
        )
        .unwrap_err();
        assert!(matches!(e, Error::Refused(_)), "{e}");

        // The objection with its grounds is admissible; at the
        // heroverweging the head can find anew what it found the first time.
        let bezwaar = bezwaren(
            &corpus,
            &mut cell,
            &besluit.besluit.id,
            &bezwaar_velden("2026-11-01"),
            day("2026-11-01"),
        )
        .unwrap();
        assert_eq!(
            bezwaar.refers_to.as_deref(),
            Some(besluit.besluit.id.as_str())
        );
        let toets = heroverweging(&corpus, &cell, &bezwaar.id, day("2026-11-15")).unwrap();
        assert_eq!(toets["ontvankelijk"], json!(true));
        let gevraagd: Vec<_> = toets["bevindingen"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| (b["naam"].as_str().unwrap(), b["eerder"].clone()))
            .collect();
        assert_eq!(
            gevraagd,
            [
                ("specifieke_aard_beroep_ouder", json!(false)),
                ("verlof_valt_buiten_schoolvakanties", json!(true)),
            ]
        );
        assert_eq!(cell.grams().len(), 3);

        // The head finds the profession anew: gegrond, and a new decision.
        let beslist = beslissen_op_bezwaar(
            &corpus,
            &mut cell,
            &bezwaar.id,
            &beroep(true),
            day("2026-11-15"),
        )
        .unwrap();
        assert_eq!(
            beslist["beslissing"]["outputs"]["bestreden_besluit_herroepen"],
            json!(true)
        );
        let nieuw = &beslist["nieuw_besluit"];
        assert_eq!(nieuw["outputs"]["verlof_verleend"], json!(true));
        assert_eq!(
            beslist["beslissing"]["outputs"]["nieuw_besluit"],
            nieuw["id"]
        );
        let gebruikt = waarom(&corpus, &cell, nieuw["id"].as_str().unwrap()).unwrap();
        assert!(gebruikt
            .bevindingen
            .contains(&"specifieke_aard_beroep_ouder".to_string()));
        let spoor = beslist["waarom"]["spoor"].as_str().unwrap();
        assert!(spoor.contains("verlofdagen"), "{spoor}");

        // The cell answers one term from its chronicle.
        let stand = reducties(&corpus, &cell, day("2027-03-01")).unwrap();
        assert_eq!(
            stand["antwoorden"],
            json!([{
                "term": {
                    "law": "leerplichtwet_1969",
                    "article": "13a",
                    "open_term": "verlof_al_verleend_in_schooljaar",
                },
                "reduction": "reductie_verlof_al_verleend_in_schooljaar",
                "valid_from": "2026-08-01",
            }])
        );
        // The reduction reads two inputs: the decisions under 13a and those under 7:11.
        let invoer = stand["reducties"][0]["invoer"].as_array().unwrap();
        let rijen = |onder: &str| {
            invoer
                .iter()
                .find(|i| i["besluiten_onder"] == json!(onder))
                .unwrap()["rijen"]
                .as_array()
                .unwrap()
                .len()
        };
        assert_eq!(rijen("leerplichtwet_1969#13a"), 2);
        assert_eq!(rijen("algemene_wet_bestuursrecht#7:11"), 1);
        assert!(stand["reducties"][0]["yaml"]
            .as_str()
            .unwrap()
            .contains("REDUCTIE"));
        assert_eq!(cell.grams().len(), 5);
    }

    #[test]
    fn a_late_objection_is_not_admissible_and_takes_no_findings() {
        let corpus = corpus();
        let mut cell = corpus.cell().unwrap();
        let besluit = geweigerd(&corpus, &mut cell);
        // The period ends on 1 December.
        let laat = bezwaren(
            &corpus,
            &mut cell,
            &besluit.besluit.id,
            &bezwaar_velden("2026-12-02"),
            day("2026-12-02"),
        )
        .unwrap();
        let toets = heroverweging(&corpus, &cell, &laat.id, day("2026-12-10")).unwrap();
        assert_eq!(toets["ontvankelijk"], json!(false));
        assert_eq!(toets["tijdig"], json!(false));
        assert_eq!(
            toets["uitvoer"]["heroverweging_geeft_aanleiding"],
            Value::Null
        );
        assert!(toets["bevindingen"].as_array().unwrap().is_empty());
        // A finding anew is asked by no article, so refused.
        let e = beslissen_op_bezwaar(
            &corpus,
            &mut cell,
            &laat.id,
            &beroep(true),
            day("2026-12-10"),
        )
        .unwrap_err();
        assert!(
            e.to_string().contains("specifieke_aard_beroep_ouder"),
            "{e}"
        );
        let beslist =
            beslissen_op_bezwaar(&corpus, &mut cell, &laat.id, &Map::new(), day("2026-12-10"))
                .unwrap();
        assert_eq!(
            beslist["beslissing"]["outputs"]["bezwaar_ontvankelijk"],
            json!(false)
        );
        assert!(beslist["nieuw_besluit"].is_null());
    }

    #[test]
    fn a_second_objection_against_a_replaced_decision_is_refused() {
        // Two objections against one grant. On the first, the head finds the
        // profession anew and a refusal takes the grant's place. On the
        // second, the same finding would replace the grant once more: the
        // cell refuses that, and records nothing.
        let corpus = corpus();
        let mut cell = corpus.cell().unwrap();
        let app = aanvragen(
            &corpus,
            &mut cell,
            &aanvraag("2027-02-05"),
            day("2026-10-13"),
        )
        .unwrap();
        let verleend = besluiten(
            &corpus,
            &mut cell,
            &app.id,
            &bevindingen(),
            day("2026-10-20"),
        )
        .unwrap()
        .besluit;
        assert_eq!(verleend.outputs["verlof_verleend"], json!(true));
        let velden = bezwaar_velden("2026-11-01");
        let eerste =
            bezwaren(&corpus, &mut cell, &verleend.id, &velden, day("2026-11-01")).unwrap();
        let tweede =
            bezwaren(&corpus, &mut cell, &verleend.id, &velden, day("2026-11-02")).unwrap();
        let beslist = beslissen_op_bezwaar(
            &corpus,
            &mut cell,
            &eerste.id,
            &beroep(false),
            day("2026-11-15"),
        )
        .unwrap();
        assert_eq!(
            beslist["nieuw_besluit"]["outputs"]["verlof_verleend"],
            json!(false)
        );
        let vastgelegd = cell.grams().len();
        // The second is admissible; the finding anew is what the cell refuses.
        let toets = heroverweging(&corpus, &cell, &tweede.id, day("2026-11-16")).unwrap();
        assert_eq!(toets["ontvankelijk"], json!(true));
        let e = beslissen_op_bezwaar(
            &corpus,
            &mut cell,
            &tweede.id,
            &beroep(false),
            day("2026-11-16"),
        )
        .unwrap_err();
        assert!(
            matches!(&e, Error::Refused(m) if m.contains("already replaces")),
            "{e}"
        );
        assert_eq!(cell.grams().len(), vastgelegd);
    }

    #[test]
    fn a_second_leave_in_the_school_year_stays_refused_on_objection() {
        // Ongegrond: the second leave this school year is refused (art. 13a,
        // once per school year). The heroverweging gives the same outcome,
        // so the refusal stands.
        let corpus = corpus();
        let mut cell = corpus.cell().unwrap();
        let eerste = aanvragen(
            &corpus,
            &mut cell,
            &aanvraag("2027-02-05"),
            day("2026-10-13"),
        )
        .unwrap();
        besluiten(
            &corpus,
            &mut cell,
            &eerste.id,
            &bevindingen(),
            day("2026-10-20"),
        )
        .unwrap();
        let mut later = aanvraag("2027-05-14");
        later.insert("eerste_verlofdag".into(), json!("2027-05-10"));
        let tweede = aanvragen(&corpus, &mut cell, &later, day("2026-10-21")).unwrap();
        let geweigerd = besluiten(
            &corpus,
            &mut cell,
            &tweede.id,
            &bevindingen(),
            day("2026-10-22"),
        )
        .unwrap()
        .besluit;
        assert_eq!(geweigerd.outputs["verlof_verleend"], json!(false));
        let bezwaar = bezwaren(
            &corpus,
            &mut cell,
            &geweigerd.id,
            &bezwaar_velden("2026-11-01"),
            day("2026-11-01"),
        )
        .unwrap();
        let toets = heroverweging(&corpus, &cell, &bezwaar.id, day("2026-11-15")).unwrap();
        assert_eq!(toets["ontvankelijk"], json!(true));
        let beslist = beslissen_op_bezwaar(
            &corpus,
            &mut cell,
            &bezwaar.id,
            &Map::new(),
            day("2026-11-15"),
        )
        .unwrap();
        let uit = &beslist["beslissing"]["outputs"];
        assert_eq!(uit["heroverweging_geeft_aanleiding"], json!(false));
        assert_eq!(uit["bestreden_besluit_herroepen"], json!(false));
        assert!(beslist["nieuw_besluit"].is_null());
    }

    #[test]
    fn a_grant_revoked_on_objection_no_longer_counts_on_the_objection_against_the_refusal() {
        // Scenario 6, the owner's case: leave A is granted, leave B in the
        // same school year is refused because A counts. On an objection
        // against A the head judges the profession anew, so A is revoked.
        // On the objection against B art. 13a is applied anew; A no longer
        // counts, and B is granted. The cell answered the term itself.
        let corpus = corpus();
        let mut cell = corpus.cell().unwrap();
        let eerste = aanvragen(
            &corpus,
            &mut cell,
            &aanvraag("2027-02-05"),
            day("2026-10-14"),
        )
        .unwrap();
        let a = besluiten(
            &corpus,
            &mut cell,
            &eerste.id,
            &bevindingen(),
            day("2026-10-14"),
        )
        .unwrap()
        .besluit;
        assert_eq!(a.outputs["verlof_verleend"], json!(true));
        let mut later = aanvraag("2027-05-14");
        later.insert("eerste_verlofdag".into(), json!("2027-05-10"));
        let tweede = aanvragen(&corpus, &mut cell, &later, day("2026-10-14")).unwrap();
        let b = besluiten(
            &corpus,
            &mut cell,
            &tweede.id,
            &bevindingen(),
            day("2026-10-14"),
        )
        .unwrap();
        assert_eq!(b.besluit.outputs["verlof_verleend"], json!(false));
        assert_eq!(b.waarom.antwoorden[0]["waarde"], json!(true));

        // Against A: the head finds the profession anew, A is revoked.
        let tegen_a = bezwaren(
            &corpus,
            &mut cell,
            &a.id,
            &bezwaar_velden("2026-11-01"),
            day("2026-11-01"),
        )
        .unwrap();
        let op_a = beslissen_op_bezwaar(
            &corpus,
            &mut cell,
            &tegen_a.id,
            &beroep(false),
            day("2026-11-15"),
        )
        .unwrap();
        assert_eq!(
            op_a["beslissing"]["outputs"]["bestreden_besluit_herroepen"],
            json!(true)
        );
        assert_eq!(
            op_a["nieuw_besluit"]["outputs"]["verlof_verleend"],
            json!(false)
        );

        // Against B: admissible, and the head is not asked the term the cell
        // answers.
        let tegen_b = bezwaren(
            &corpus,
            &mut cell,
            &b.besluit.id,
            &bezwaar_velden("2026-11-20"),
            day("2026-11-20"),
        )
        .unwrap();
        let toets = heroverweging(&corpus, &cell, &tegen_b.id, day("2026-12-01")).unwrap();
        assert_eq!(toets["ontvankelijk"], json!(true));
        let gevraagd: Vec<&str> = toets["bevindingen"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["naam"].as_str().unwrap())
            .collect();
        assert!(!gevraagd.contains(&"verlof_al_verleend_in_schooljaar"));
        let op_b = beslissen_op_bezwaar(
            &corpus,
            &mut cell,
            &tegen_b.id,
            &Map::new(),
            day("2026-12-01"),
        )
        .unwrap();
        assert_eq!(
            op_b["beslissing"]["outputs"]["bestreden_besluit_herroepen"],
            json!(true)
        );
        assert_eq!(
            op_b["nieuw_besluit"]["outputs"]["verlof_verleend"],
            json!(true)
        );

        // Why: one node for the term, the cell's answer "nee", read up to
        // the objection against B.
        let beslissing = op_b["beslissing"]["id"].as_str().unwrap();
        let antwoorden = op_b["waarom"]["antwoorden"].as_array().unwrap();
        assert_eq!(antwoorden.len(), 1, "{antwoorden:?}");
        let antwoord = &antwoorden[0];
        assert_eq!(
            antwoord["begrip"],
            json!("verlof_al_verleend_in_schooljaar")
        );
        assert_eq!(antwoord["waarde"], json!(false));
        assert_eq!(antwoord["door"], json!(SCHOOL));
        assert_eq!(
            antwoord["reductie"],
            json!("reductie_verlof_al_verleend_in_schooljaar")
        );
        assert_eq!(antwoord["geldig_vanaf"], json!("2026-08-01"));
        assert_eq!(antwoord["tot_en_met"], json!(tegen_b.id));
        let spoor = op_b["waarom"]["spoor"].as_str().unwrap();
        assert!(!spoor.contains("FOREACH"), "{spoor}");
        assert!(op_b["waarom"]["ingevuld"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["term"] == json!("verlof_al_verleend_in_schooljaar")
                && f["filled_by"]["answer"]["law_id"]
                    == json!("reductie_verlof_al_verleend_in_schooljaar")));

        // How the school answered: A is among the revoked decisions, and
        // both rows under 13a that are not B (A, and the refusal in its
        // place) are skipped.
        let hoe = hoe_beantwoord(
            &corpus,
            &cell,
            beslissing,
            "verlof_al_verleend_in_schooljaar",
        )
        .unwrap();
        let eerst = &hoe["antwoorden"][0];
        assert_eq!(
            eerst["uitvoer"]["verlof_al_verleend_in_schooljaar"],
            json!(false)
        );
        assert_eq!(eerst["uitvoer"]["herroepen_besluiten"], json!([a.id]));
        let spoor = eerst["spoor"].as_str().unwrap();
        assert!(
            spoor.contains(&format!("herroepen_besluiten = ['{}']", a.id)),
            "{spoor}"
        );
        assert!(
            spoor.contains("FOREACH over 0 element(s), 2 skipped by filter = False"),
            "{spoor}"
        );
    }
}
