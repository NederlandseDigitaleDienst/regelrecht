//! What the app does with the cell: the process steps and the reduction,
//! apart from HTTP.
//!
//! Every step builds a fresh engine service from the regelwerken in the
//! corpus. That costs a few milliseconds and keeps the steps independent:
//! the cell runs its reductions in the service it gets for a decision,
//! and nothing of that may reach the next step.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::NaiveDate;
use regelrecht_cel::{
    decision, reading, shape, Cell, Disk, Error, Files, Gram, Reference, Source, EXECUTOGRAM,
};
use regelrecht_engine::{
    ArticleResult, Authority, EngineError, FilledBy, LawExecutionService, OpenTermFilled,
    ParameterType, PathNode, PathNodeType,
};
use regelrecht_law_model::stored;
use serde_json::{json, Map, Value};

use crate::casus::{afleiden, casussen, Afleiding, Beroep, Casus, Regelwerken};

pub type Result<T> = std::result::Result<T, Error>;

/// A case with what follows for it from the law ([`Afleiding`]): the
/// versions of the regelwerken its cell runs, read once at the start, and the
/// cell's directory, with the files it is read from.
#[derive(Debug, Clone)]
pub struct Corpus {
    pub casus: Casus,
    pub afleiding: Afleiding,
    /// Per regelwerk the cell runs: its `$id`, its name and its directory
    /// under its root ([`crate::casus::Regelwerk::map`]).
    regelwerken: Vec<(String, Option<String>, String)>,
    versies: Vec<String>,
    /// Per `$id` and `valid_from` of a regelwerk the cell runs, that
    /// version as written.
    geschreven: BTreeMap<(String, Option<String>), String>,
    cell_dir: PathBuf,
    /// The directories of the other cells of the sphere, in the order of
    /// the case file.
    andere_dirs: Vec<PathBuf>,
    /// Where the cell directories are read from: the disk, or the bundle a
    /// browser holds.
    files: Arc<dyn Files>,
}

/// The cell of the case and the other cells of its sphere (RFC-052). It
/// reads as the case's cell; a step that decides passes the others along.
#[derive(Debug)]
pub struct Cellen {
    pub eigen: Cell,
    pub andere: Vec<Cell>,
}

impl Cellen {
    /// The other cells, as a decision takes them along.
    pub fn sfeer(&self) -> Vec<&Cell> {
        self.andere.iter().collect()
    }

    /// The bindings of a policy in force on `on` to an answer that one of
    /// the cells gives (RFC-048): of the case's cell or of another cell of
    /// the case. A term such a binding fills is no judgement the form asks.
    pub fn gebonden(&self, service: &LawExecutionService, on: NaiveDate) -> Vec<reading::Binding> {
        reading::bindings(service, on)
            .into_iter()
            .filter(|b| {
                std::iter::once(&self.eigen)
                    .chain(&self.andere)
                    .any(|cell| cell.answers().contains_key(&b.answer))
            })
            .collect()
    }

    /// The case's cell to record in, and the others to ask.
    pub fn splits(&mut self) -> (&mut Cell, Vec<&Cell>) {
        (&mut self.eigen, self.andere.iter().collect())
    }

    /// The chronicle of every cell, the case's cell first, as
    /// [`Corpus::herstellen`] takes them back.
    pub fn kronieken(&self) -> Vec<Vec<Gram>> {
        std::iter::once(&self.eigen)
            .chain(&self.andere)
            .map(|c| c.grams().to_vec())
            .collect()
    }

    /// The other cell `cel` to record in, and as its sphere every other
    /// cell of the case, the case's own among them: the court decides as
    /// the case's cell does, with the cells of the case beside it.
    pub fn splits_op(&mut self, cel: &str) -> Result<(&mut Cell, Vec<&Cell>)> {
        let mut gevonden = None;
        let mut sfeer = vec![&self.eigen];
        for c in &mut self.andere {
            if gevonden.is_none() && c.recording_actor() == cel {
                gevonden = Some(c);
            } else {
                sfeer.push(&*c);
            }
        }
        let cell = gevonden
            .ok_or_else(|| Error::Refused(format!("'{cel}' is no other cell of this case")))?;
        Ok((cell, sfeer))
    }

    /// [`Cellen::splits_op`] to read: the other cell `cel` and its sphere.
    pub fn op(&self, cel: &str) -> Result<(&Cell, Vec<&Cell>)> {
        let cell = self
            .andere
            .iter()
            .find(|c| c.recording_actor() == cel)
            .ok_or_else(|| Error::Refused(format!("'{cel}' is no other cell of this case")))?;
        let sfeer = std::iter::once(&self.eigen)
            .chain(self.andere.iter().filter(|c| c.recording_actor() != cel))
            .collect();
        Ok((cell, sfeer))
    }

    /// Copies of every cell with the chronicle in memory, to try a decision
    /// on ([`Cell::sandbox`]).
    pub fn sandbox(&self) -> Self {
        Self {
            eigen: self.eigen.sandbox(),
            andere: self.andere.iter().map(Cell::sandbox).collect(),
        }
    }
}

impl std::ops::Deref for Cellen {
    type Target = Cell;
    fn deref(&self) -> &Cell {
        &self.eigen
    }
}

impl Corpus {
    /// The case `casus` on the corpus at `roots` (this crate's `corpus/`,
    /// or for a case from a source that source alone) in `files`,
    /// whose regelwerken are `regelwerken`: its cell at `<root>/cells/<cel>`
    /// of the first root that has it, and what follows from the law within
    /// its corpus ([`afleiden`], [`Regelwerken::voor`]).
    pub fn load(
        files: Arc<dyn Files>,
        roots: &[PathBuf],
        regelwerken: &Regelwerken,
        casus: Casus,
    ) -> Result<Self> {
        let cell_dir = cel_dir(&*files, roots, &casus.cel);
        let cell = Cell::in_memory_from(&*files, &casus.cel, &cell_dir)?;
        let andere_dirs: Vec<PathBuf> = casus
            .andere_cellen
            .iter()
            .map(|a| cel_dir(&*files, roots, &a.cel))
            .collect();
        let andere = casus
            .andere_cellen
            .iter()
            .zip(&andere_dirs)
            .map(|(a, dir)| Cell::in_memory_from(&*files, &a.cel, dir))
            .collect::<Result<Vec<_>>>()?;
        let cells: Vec<&Cell> = std::iter::once(&cell).chain(&andere).collect();
        let regelwerken = &regelwerken.voor(&casus)?;
        let afleiding = afleiden(regelwerken, &casus, &cells)?;
        let mut versies = Vec::new();
        let mut geschreven = BTreeMap::new();
        let mut gelezen = Vec::new();
        for id in &afleiding.regelwerken {
            let werk = regelwerken
                .get(id)
                .ok_or_else(|| Error::Setup(format!("'{id}' is not in the corpus")))?;
            versies.extend(werk.versies.iter().map(|(yaml, _)| yaml.clone()));
            for (yaml, law) in &werk.versies {
                geschreven.insert((law.id.clone(), law.valid_from.clone()), yaml.clone());
            }
            let naam = werk.versies.last().and_then(|(_, law)| law.name.clone());
            gelezen.push((id.clone(), naam, werk.map.clone()));
        }
        Ok(Self {
            casus,
            afleiding,
            regelwerken: gelezen,
            versies,
            geschreven,
            cell_dir,
            andere_dirs,
            files,
        })
    }

    /// Every case in `casus_dir` on the corpus at `root`, on disk.
    pub fn alle(root: &Path, casus_dir: &Path) -> Result<Vec<Self>> {
        Self::alle_in(Arc::new(Disk), &[root.to_path_buf()], casus_dir)
    }

    /// Every case in `casus_dir` on the corpus at `roots`, all in `files`.
    pub fn alle_in(
        files: Arc<dyn Files>,
        roots: &[PathBuf],
        casus_dir: &Path,
    ) -> Result<Vec<Self>> {
        let regelwerken = Regelwerken::load(&*files, roots)?;
        casussen(&*files, casus_dir)?
            .into_iter()
            .map(|c| Self::load(files.clone(), roots, &regelwerken, c))
            .collect()
    }

    /// The case's cell and the other cells it takes along ([`Cellen`]), each
    /// with the catalogue in its directory and a chronicle that holds what
    /// it knew before the first scenario (its `begin`), recorded through the
    /// cell as any step: an executogram per fact
    /// (`Cell::record_executogram`).
    /// For vakantieverlof that is where the school is established; with the
    /// school's own holidays in its policy, the cell answers whether leave
    /// falls outside the school holidays and the head is not asked.
    pub fn cell(&self) -> Result<Cellen> {
        let eigen = self.begonnen(&self.casus.cel, &self.cell_dir, self.casus.begin.as_ref())?;
        let mut andere: Vec<Cell> = Vec::new();
        for (a, dir) in self.casus.andere_cellen.iter().zip(&self.andere_dirs) {
            andere.push(self.begonnen(&a.cel, dir, a.begin.as_ref())?);
        }
        Ok(Cellen { eigen, andere })
    }

    /// The case's cell and the other cells of its sphere with the
    /// chronicles `kronieken` held before, in the order of [`Corpus::cell`]
    /// and of [`Cellen::kronieken`]: a game kept across a reload. Each is
    /// checked as a chronicle read from a file ([`Cell::restore`]), and a
    /// count that does not match the case's cells is an error.
    pub fn herstellen(&self, kronieken: Vec<Vec<Gram>>) -> Result<Cellen> {
        let cellen: Vec<(&str, &Path)> =
            std::iter::once((self.casus.cel.as_str(), self.cell_dir.as_path()))
                .chain(
                    self.casus
                        .andere_cellen
                        .iter()
                        .zip(&self.andere_dirs)
                        .map(|(a, dir)| (a.cel.as_str(), dir.as_path())),
                )
                .collect();
        if kronieken.len() != cellen.len() {
            return Err(Error::Setup(format!(
                "case '{}' has {} cells, not {}",
                self.casus.id,
                cellen.len(),
                kronieken.len()
            )));
        }
        let mut cells = cellen
            .into_iter()
            .zip(kronieken)
            .map(|((cel, dir), grams)| Cell::restore(&*self.files, cel, dir, grams))
            .collect::<Result<Vec<_>>>()?
            .into_iter();
        let eigen = cells
            .next()
            .ok_or_else(|| Error::Setup("a case has its own cell".into()))?;
        Ok(Cellen {
            eigen,
            andere: cells.collect(),
        })
    }

    /// The cell `cel` in `dir`, with what `begin` records.
    fn begonnen(&self, cel: &str, dir: &Path, begin: Option<&crate::casus::Begin>) -> Result<Cell> {
        let mut cell = Cell::in_memory_from(&*self.files, cel, dir)?;
        let Some(begin) = begin else {
            return Ok(cell);
        };
        let service = self.service()?;
        for e in &begin.executogrammen {
            cell.record_executogram(&service, &e.feit, &e.velden, e.datum.unwrap_or(begin.datum))?;
        }
        Ok(cell)
    }

    /// The version of the regelwerk `id` the cell runs that is valid from
    /// `valid_from`, as written.
    fn yaml_van(&self, id: &str, valid_from: &Option<String>) -> Option<&str> {
        self.geschreven
            .get(&(id.to_string(), valid_from.clone()))
            .map(String::as_str)
    }

    /// The appeal of the case, or a refusal for a case without a court.
    fn beroep(&self) -> Result<&Beroep> {
        self.afleiding
            .beroep
            .as_ref()
            .ok_or_else(|| Error::Refused(format!("the case '{}' has no court", self.casus.id)))
    }

    /// The paths of the cells the case runs under `cells/` of the corpus: its own,
    /// and the other cells of its sphere (RFC-052), the court among them.
    pub fn cellen(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.casus.cel.as_str())
            .chain(self.casus.andere_cellen.iter().map(|a| a.cel.as_str()))
    }

    /// A service with every regelwerk the cell runs.
    pub fn service(&self) -> Result<LawExecutionService> {
        let mut service = LawExecutionService::new();
        for yaml in &self.versies {
            service.load_law(yaml)?;
        }
        Ok(service)
    }

    fn law(&self) -> &str {
        &self.afleiding.law
    }

    fn article(&self) -> &str {
        &self.afleiding.article
    }

    /// The law and article whose decision the application asks for.
    fn decision_law(&self) -> &str {
        &self.afleiding.decision_law
    }

    fn decision(&self) -> &str {
        &self.afleiding.decision
    }

    fn awb(&self) -> &str {
        &self.afleiding.objection_law
    }

    fn objection(&self) -> &str {
        &self.afleiding.objection
    }

    /// The case as the app shows it: its text and scenarios, and what
    /// follows from the law.
    pub fn beschrijving(&self) -> Value {
        let c = &self.casus;
        let a = &self.afleiding;
        // The court: the other cell that the law makes the one who decides
        // on the appeal ([`Beroep::rechter`]).
        let rechter = a
            .beroep
            .as_ref()
            .and_then(|b| c.andere_cellen.iter().find(|x| x.cel == b.rechter))
            .map(|r| json!({ "cel": r.cel, "cel_naam": r.cel_naam }));
        json!({
            "id": c.id,
            "bron": c.bron,
            "titel": c.titel,
            "cel": c.cel,
            "cel_naam": c.cel_naam,
            "speeldatum": c.speeldatum,
            "uitkomst": c.uitkomst,
            "voorbeeld": c.voorbeeld,
            "begin": c.begin,
            "andere_cellen": c.andere_cellen,
            "scenarios": c.scenarios,
            "besluit": format!("{}#{}", a.decision_law, a.decision),
            "beslissing_op_bezwaar": format!("{}#{}", a.objection_law, a.objection),
            "rechter": rechter,
            "beroep": a.beroep.as_ref().map(|b| format!("{}#{}", b.law, b.article)),
            "ontvangst": a.beroep.as_ref().map(|b| b.ontvangst.clone()),
            "rol": a.rol,
            "regelwerken": self.regelwerken.iter().map(|(id, naam, map)| json!({
                "id": id,
                "naam": naam,
                "map": map,
            })).collect::<Vec<_>>(),
        })
    }

    /// The case in the list a visitor chooses from.
    pub fn kaart(&self) -> Value {
        let c = &self.casus;
        json!({
            "id": c.id,
            "bron": c.bron,
            "titel": c.titel,
            "cel_naam": c.cel_naam,
            "besluit": format!("{}#{}", self.decision_law(), self.decision()),
            "rol": self.afleiding.rol,
        })
    }
}

/// The directory of the cell `cel`: under `cells` of the first of `roots`
/// that has it, or else of the first root, where loading it says it is
/// missing.
fn cel_dir(files: &dyn Files, roots: &[PathBuf], cel: &str) -> PathBuf {
    let onder = |root: &PathBuf| root.join("cells").join(cel);
    roots
        .iter()
        .map(onder)
        .find(|dir| files.is_dir(dir))
        .or_else(|| roots.first().map(onder))
        .unwrap_or_else(|| Path::new("cells").join(cel))
}

/// The object `v` as a map; a value that is no object is an error.
fn object(v: Value) -> Result<Map<String, Value>> {
    match v {
        Value::Object(m) => Ok(m),
        v => Err(Error::Setup(format!("{v} is no object"))),
    }
}

/// A field as the forms show it.
fn veld(field: &shape::Field) -> Value {
    json!({
        "naam": field.name,
        "type": field.type_,
        "mag_leeg": field.nullable,
        "basis": field.first_declared_by(),
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

/// The findings the deciding article of `shape` asks of `wie`, the
/// authority that decides: not those of a policy that fills one of its
/// terms.
fn eigen_oordelen<'s>(
    shape: &'s decision::DecisionShape,
    wie: &'s Authority,
) -> impl Iterator<Item = &'s regelrecht_engine::DecisionTerm> {
    shape
        .terms
        .iter()
        .filter(|t| t.is_finding(wie) && t.law_id == shape.law_id && t.article == shape.article)
}

/// The open term `term` of the article `waar` (`<law>#<article>`) if it is
/// one a policy leaves to the authority for a case: the article fills a
/// term of another (`implements`) and judges this one per case, with
/// nothing filling it (RFC-048 §2), as the engine says
/// ([`LawExecutionService::decision_terms`]). The article need not be one
/// the decision reaches through `implements`: a policy may fill a term of a
/// regulation the decision reads.
fn beleid_laat_oordelen(
    service: &LawExecutionService,
    term: &str,
    waar: &str,
    on: NaiveDate,
) -> Option<regelrecht_engine::DecisionTerm> {
    let (law, article) = waar.split_once('#')?;
    let vult = service
        .resolver()
        .get_law_for_date(law, Some(on))?
        .find_article_by_number(article)?
        .get_implements()
        .is_some_and(|i| !i.is_empty());
    if !vult {
        return None;
    }
    service
        .decision_terms(law, article, &on.to_string(), &Default::default())
        .ok()?
        .into_iter()
        .find(|t| t.id == term && t.declared_by() == waar && t.asks_finding())
}

/// What the law asks on `on`: the fields of the application, the deciding
/// authority's findings on it and the fields of the objection. All of it is
/// derived from the law, for this cell: a term the cell answers from its
/// chronicle is no finding of the authority, so the form does not ask it. An
/// objection does not change the application; a smaller request is a new
/// application. What the authority finds anew on an objection follows from
/// the contested decision ([`heroverweging`]).
pub fn vormen(corpus: &Corpus, cell: &Cellen, on: NaiveDate) -> Result<Value> {
    let service = corpus.service()?;
    let beantwoord = cell.gebonden(&service, on);
    let aanvraag = shape::derive(&service, corpus.law(), corpus.article(), on)?;
    let besluit = decision::derive(&service, corpus.decision_law(), corpus.decision(), on)?;
    let bezwaar = shape::derive(&service, corpus.awb(), corpus.objection(), on)?;
    let op_bezwaar = decision::derive(&service, corpus.awb(), corpus.objection(), on)?;
    let moment = |s: &shape::Shape| {
        s.moment
            .as_ref()
            .map(|m| json!({ "parameter": m.parameter, "basis": m.declared_by }))
    };
    // The form asks the findings of the deciding article itself. One that a
    // policy leaves to the authority where its cell is silent, it asks only
    // when the preview shows the case reaches it ([`voorvertonen`]).
    // What the authority supplies, such as a parameter of a hook on the
    // decision, it gives in the same form: the cell splits the two
    // ([`decide`]).
    let wie = cell.eigen.authority();
    let mut bevindingen: Vec<Value> = eigen_oordelen(&besluit, wie)
        .map(|t| veld(&decision::finding_field(t)))
        .collect();
    // Not one the policy fills, nor one by the name of a finding: the cell
    // refuses those as a value of the authority.
    bevindingen.extend(
        besluit
            .inputs
            .iter()
            .filter(|i| {
                i.source == Source::Supplied
                    && i.field.type_ != ParameterType::Decision
                    && !besluit.filled(&i.field.name)
                    && besluit.finding(&i.field.name, wie).is_none()
            })
            .map(|i| veld(&i.field)),
    );
    let mut herkomst: Vec<Value> = besluit
        .inputs
        .iter()
        .map(|i| {
            json!({
                "naam": i.field.name,
                "bron": match i.source {
                    Source::Application => "aanvraag",
                    Source::Supplied => "bestuursorgaan",
                    Source::Cell => "cel",
                },
            })
        })
        .collect();
    for t in besluit
        .terms
        .iter()
        .filter(|t| t.law_id == besluit.law_id && t.article == besluit.article)
    {
        let bron = if beantwoord.iter().any(|b| b.term.open_term == t.id) {
            "kroniek van de cel"
        } else if !t.filled_by.is_empty() {
            "beleid"
        } else if t.is_finding(wie) {
            "bestuursorgaan"
        } else {
            continue;
        };
        herkomst.push(json!({ "naam": t.id, "bron": bron }));
    }
    let mut vormen = json!({
        "aanvraag": {
            "soort": aanvraag.kind,
            "basis": aanvraag.basis(),
            "velden": in_te_vullen(&aanvraag),
            "moment": moment(&aanvraag),
        },
        "besluit": {
            "soort": besluit.kind,
            "basis": besluit.basis(),
            "bevindingen": bevindingen,
            "invoer": herkomst,
            "uitvoer": besluit.outputs,
        },
        "bezwaar": {
            "soort": bezwaar.kind,
            "basis": bezwaar.basis(),
            "velden": in_te_vullen(&bezwaar),
            "moment": moment(&bezwaar),
        },
        "beslissing_op_bezwaar": {
            "soort": op_bezwaar.kind,
            "basis": op_bezwaar.basis(),
            "uitvoer": op_bezwaar.outputs,
        },
        "andere_cellen": handelingen(&service, corpus, cell, on)?,
    });
    if let Some(b) = &corpus.afleiding.beroep {
        let beroep = shape::derive(&service, &b.law, &b.article, on)?;
        let uitspraak = decision::derive(&service, &b.law, &b.article, on)?;
        let (rechter, _) = cell.op(&b.rechter)?;
        let mut bevindingen =
            oordelen_van_de_rechter(&service, &uitspraak, rechter.authority(), on)?;
        for field in opnieuw_te_beoordelen(corpus, &service, on)? {
            if !bevindingen.iter().any(|v| v["naam"] == field.name) {
                bevindingen.push(veld(&field));
            }
        }
        vormen["beroep"] = json!({
            "soort": beroep.kind,
            "basis": beroep.basis(),
            "velden": in_te_vullen(&beroep),
            "moment": moment(&beroep),
        });
        vormen["uitspraak"] = json!({
            "soort": uitspraak.kind,
            "basis": uitspraak.basis(),
            "bevindingen": bevindingen,
            "uitvoer": uitspraak.outputs,
        });
    }
    Ok(vormen)
}

/// What the court judges when it decides the case itself (Awb 8:72 lid 3
/// onder b): it calls the contested decision on objection anew, and that
/// calls the case's decision anew. Their findings are then the court's,
/// since its judgment takes the place of the decision, also those a policy
/// leaves to the authority where its cell is silent: the court has no such
/// chronicle, so there the policy's cell is silent, and the court
/// supplements the facts itself (8:69 lid 3). So every term that takes a
/// finding counts, whoever answers it in the decision itself.
fn opnieuw_te_beoordelen(
    corpus: &Corpus,
    service: &LawExecutionService,
    on: NaiveDate,
) -> Result<Vec<shape::Field>> {
    let mut uit: Vec<shape::Field> = Vec::new();
    for shape in [
        decision::derive(service, corpus.awb(), corpus.objection(), on)?,
        decision::derive(service, corpus.decision_law(), corpus.decision(), on)?,
    ] {
        for t in shape.terms.iter().filter(|t| t.asks_finding()) {
            if !uit.iter().any(|f| f.name == t.id) {
                uit.push(decision::finding_field(t));
            }
        }
    }
    Ok(uit)
}

/// What the court, `wie`, judges per case for the decision `shape`: the
/// findings of the deciding article, and those of each article it reads an
/// output of (`source.regulation`) that take a finding of the court, as the
/// engine says ([`LawExecutionService::decision_terms`]). Those the deciding
/// article asks only when it reads that article: 8:70 reads 8:72 for a
/// well-founded appeal. Each has its article as `basis`.
fn oordelen_van_de_rechter(
    service: &LawExecutionService,
    shape: &decision::DecisionShape,
    wie: &Authority,
    on: NaiveDate,
) -> Result<Vec<Value>> {
    let mut uit: Vec<Value> = eigen_oordelen(shape, wie)
        .map(|t| veld(&decision::finding_field(t)))
        .collect();
    let resolver = service.resolver();
    let Some(deciding) = resolver
        .get_law_for_date(&shape.law_id, Some(on))
        .and_then(|law| law.find_article_by_number(&shape.article))
    else {
        return Ok(uit);
    };
    let mut gelezen: Vec<(String, String)> = Vec::new();
    for source in deciding
        .get_inputs()
        .iter()
        .filter_map(|i| i.source.as_ref())
    {
        let (Some(law_id), Some(output)) = (&source.regulation, &source.output) else {
            continue;
        };
        if let Some(a) = resolver.get_article_by_output(law_id, output, Some(on)) {
            let waar = (law_id.clone(), a.number.clone());
            if !gelezen.contains(&waar) {
                gelezen.push(waar);
            }
        }
    }
    let dag = on.to_string();
    for (law_id, article) in gelezen {
        for t in service
            .decision_terms(&law_id, &article, &dag, &Default::default())?
            .iter()
            .filter(|t| t.law_id == law_id && t.article == article && t.is_finding(wie))
        {
            uit.push(veld(&decision::finding_field(t)));
        }
    }
    Ok(uit)
}

/// What each other cell of the sphere records by hand (RFC-052): the facts
/// whose executograms its reductions read (`source.facts`), each with the
/// fields the article that requires it lists. For the parking card: the GGD
/// records the medical examination (Regeling art. 3).
fn handelingen(
    service: &LawExecutionService,
    corpus: &Corpus,
    cellen: &Cellen,
    on: NaiveDate,
) -> Result<Vec<Value>> {
    let mut uit = Vec::new();
    for (cell, andere) in cellen.andere.iter().zip(&corpus.casus.andere_cellen) {
        let mut feiten: Vec<String> = Vec::new();
        // Only a reduction reads a chronicle (RFC-048).
        for (_, reduction) in reducties_in(&|id: &str| service.resolver().get_law(id), &[cell]) {
            for article in &reduction.articles {
                for input in article.get_inputs() {
                    if let Some(f) = input.source.as_ref().and_then(|s| s.facts.as_ref()) {
                        if !feiten.contains(f) {
                            feiten.push(f.clone());
                        }
                    }
                }
            }
        }
        let mut acts = Vec::new();
        for feit in feiten {
            let velden: Vec<Value> = shape::fact_fields(service, &feit, on, None)?
                .iter()
                .map(veld)
                .collect();
            acts.push(json!({ "feit": feit, "velden": velden }));
        }
        uit.push(json!({
            "cel": cell.recording_actor(),
            "cel_naam": andere.cel_naam,
            "handelingen": acts,
        }));
    }
    Ok(uit)
}

/// Another cell of the sphere records an executogram of the fact `feit`
/// (`<regulation>#<article>#<fact>`) on `on` (RFC-051, RFC-052), such as the
/// GGD recording that it examined the applicant. The case's own cell
/// records nothing.
pub fn vastleggen(
    corpus: &Corpus,
    cellen: &mut Cellen,
    cel: &str,
    feit: &str,
    velden: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Gram> {
    // Only a fact this cell reads back, as the form offers it.
    let service = corpus.service()?;
    let mag = handelingen(&service, corpus, cellen, on)?
        .into_iter()
        .any(|c| {
            c["cel"] == cel
                && c["handelingen"]
                    .as_array()
                    .is_some_and(|h| h.iter().any(|h| h["feit"] == feit))
        });
    if !mag {
        return Err(Error::Refused(format!(
            "'{cel}' records no fact '{feit}': it is no other cell of this case, or none of its \
             reductions reads that fact"
        )));
    }
    let cell = cellen
        .andere
        .iter_mut()
        .find(|c| c.recording_actor() == cel)
        .ok_or_else(|| Error::Refused(format!("'{cel}' is no other cell of this case")))?;
    cell.record_executogram(&service, feit, velden, on)
}

/// The applicant applies: record the application under the case's article,
/// received on `on`.
pub fn aanvragen(
    corpus: &Corpus,
    cell: &mut Cellen,
    velden: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Gram> {
    let (cell, sfeer) = cell.splits();
    cell.record_submission(
        &mut corpus.service()?,
        &sfeer,
        corpus.law(),
        corpus.article(),
        None,
        velden,
        on,
    )
}

/// Why a decision came out as it did, as the cell explains it
/// ([`Cell::explain`]): the engine's trace, in full and condensed to its
/// steps, every open term with who filled it, the terms the cell answered
/// from its chronicle (one trace node each) and the findings of the authority
/// the decision used ([`Cell::findings`]).
#[derive(Debug, serde::Serialize)]
pub struct Waarom {
    pub spoor: String,
    /// The trace condensed ([`keten`]): the article the decision ran, and
    /// in it what came out, the open terms and who filled them, the
    /// articles it called and the hooks that fired.
    pub keten: Vec<Stap>,
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

/// The nodes of `node` and below where a cell gave an answer, as
/// [`beantwoord`], each with the open term its binding filled: the nearest
/// term above it. The answer has its own name, which need not be the
/// term's (RFC-048).
fn beantwoord_per_begrip(node: &PathNode) -> Vec<(&PathNode, &str)> {
    fn onder<'n>(node: &'n PathNode, begrip: &'n str, uit: &mut Vec<(&'n PathNode, &'n str)>) {
        let begrip = if node.node_type == PathNodeType::OpenTermResolution {
            node.name.as_str()
        } else {
            begrip
        };
        if node.answer.is_some() {
            uit.push((node, begrip));
        }
        for child in &node.children {
            onder(child, begrip, uit);
        }
    }
    let mut uit = Vec::new();
    onder(node, node.name.as_str(), &mut uit);
    uit
}

/// How many times before `node` the decision asked the same cell the same
/// term, in trace order: the index of its explanation among those
/// [`hoe_beantwoord`] gives. `None` when no cell answered at `node`.
fn keer(beantwoord: &[&PathNode], node: &PathNode) -> Option<usize> {
    let i = beantwoord.iter().position(|n| std::ptr::eq(*n, node))?;
    let door = |n: &PathNode| n.answer.as_ref().map(|a| a.answerer.clone());
    Some(
        beantwoord[..i]
            .iter()
            .filter(|n| n.name == node.name && door(n) == door(node))
            .count(),
    )
}

/// The nodes under the open term `node` where a cell gave the answer its
/// binding took (RFC-048): one per answer, under the binding's input. Not
/// those of a term below it, which have their own node.
fn antwoorden_van(node: &PathNode) -> Vec<&PathNode> {
    fn onder<'n>(node: &'n PathNode, uit: &mut Vec<&'n PathNode>) {
        for child in &node.children {
            if child.answer.is_some() {
                uit.push(child);
            } else if child.node_type != PathNodeType::OpenTermResolution {
                onder(child, uit);
            }
        }
    }
    let mut uit = Vec::new();
    onder(node, &mut uit);
    uit
}

/// Why the decision `id` came out as it did ([`Waarom`]).
pub fn waarom(corpus: &Corpus, cell: &Cellen, id: &str) -> Result<Waarom> {
    waarom_in(corpus, cell, &cell.sfeer(), id)
}

/// What follows from the submission `id`, in the chronicle of any cell of
/// the case: the outputs of the article that established it and of the hooks
/// on it, such as the last day of the decision period (Awb 4:13). The
/// chronolexogram holds what was submitted; the cell computes these when
/// asked, in the law in force on the day it was recorded.
pub fn wat_volgt(corpus: &Corpus, cellen: &Cellen, id: &str) -> Result<Value> {
    let mut service = corpus.service()?;
    let houder = std::iter::once(&cellen.eigen)
        .chain(&cellen.andere)
        .find(|c| c.grams().iter().any(|g| g.id == id))
        .ok_or_else(|| Error::Refused(format!("no cell of this case holds '{id}'")))?;
    let uitvoer = if houder.recording_actor() == cellen.eigen.recording_actor() {
        cellen
            .eigen
            .submission_outputs(&mut service, &cellen.sfeer(), id)?
    } else {
        let (cell, sfeer) = cellen.op(houder.recording_actor())?;
        cell.submission_outputs(&mut service, &sfeer, id)?
    };
    Ok(json!(uitvoer))
}

/// [`waarom`] for the decision `id` of `cell`, with the other cells of
/// `sfeer`: the case's cell, or another cell of the case such as the court.
fn waarom_in(corpus: &Corpus, cell: &Cell, sfeer: &[&Cell], id: &str) -> Result<Waarom> {
    let mut service = corpus.service()?;
    let result = cell.explain(&mut service, sfeer, id)?;
    let mut antwoorden = Vec::new();
    if let Some(root) = &result.trace {
        let gevraagd = beantwoord(root);
        for (node, begrip) in beantwoord_per_begrip(root) {
            let Some(answer) = &node.answer else {
                continue;
            };
            let waarde = node.result.as_ref().map(stored::to_stored).transpose()?;
            // The case's cell explains its own answers; another cell's
            // reasoning is its own (RFC-048).
            let keer = keer(&gevraagd, node).filter(|_| answer.answerer == cell.recording_actor());
            antwoorden.push(json!({
                "begrip": begrip,
                "antwoord": answer.answer,
                "keer": keer,
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
        keten: keten(&result, cell.recording_actor())?,
        ingevuld: result.open_terms,
        antwoorden,
        bevindingen: cell.findings(&mut service, sfeer, id)?,
    })
}

/// One step of a decision, condensed from the engine's trace ([`keten`]).
/// A value is in the form the chronicle stores it; a value the engine could
/// not determine is `null`, with the facts it missed in `ontbreekt`.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "soort", rename_all = "snake_case")]
pub enum Stap {
    /// An article runs: the one that decides, or one it calls. A call
    /// under which no article ran (a cached result) names the output it
    /// asked for instead (`uitvoer`), and `artikel` is empty.
    Artikel {
        wet: String,
        artikel: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        uitvoer: Option<String>,
        stappen: Vec<Stap>,
    },
    /// A hook fires: an article that runs at a stage of the decision.
    Hook {
        wet: String,
        artikel: String,
        stappen: Vec<Stap>,
    },
    /// An output of the article that runs, and what it needed.
    Uitkomst {
        naam: String,
        waarde: Value,
        ontbreekt: Vec<String>,
        stappen: Vec<Stap>,
    },
    /// An open term, and who filled it (`ArticleResult::open_terms`). For
    /// the cell's answer, `keer` is how many times the decision asked the
    /// cell this term before, over the whole trace: the index into the
    /// answers of [`hoe_beantwoord`]. `stappen` are those of the regulation
    /// that filled it, only when the cell answered a term in it (the
    /// municipality's policy on the parkeerkaart asks the cell whether the
    /// applicant holds a valid card).
    Begrip {
        naam: String,
        wet: String,
        artikel: String,
        waarde: Value,
        ontbreekt: Vec<String>,
        ingevuld_door: Option<FilledBy>,
        #[serde(skip_serializing_if = "Option::is_none")]
        keer: Option<usize>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        stappen: Vec<Stap>,
    },
}

/// What condensing a trace needs besides the node: who filled each open
/// term, and the nodes where the cell answered one, in trace order.
struct Spoor<'a> {
    ingevuld: &'a [OpenTermFilled],
    beantwoord: Vec<&'a PathNode>,
    /// The case's cell: only its answers have an explanation here.
    eigen: &'a str,
}

/// The decision's trace condensed to the steps a reader follows: the
/// article that ran, per output what came out, and under it the open terms,
/// the articles called and the hooks fired. Resolving a variable and the
/// arithmetic leave no step; what is found under them belongs to the step
/// above. Under an open term nothing more: how a regulation or the cell
/// filled it is their own account, unless the cell answered a term in that
/// regulation; then its steps follow, down to that answer. A hook that determined nothing at the
/// decision leaves no step either; the full trace still has it.
pub fn keten<'a>(result: &'a ArticleResult, eigen: &'a str) -> Result<Vec<Stap>> {
    let Some(root) = &result.trace else {
        return Ok(Vec::new());
    };
    let spoor = Spoor {
        ingevuld: &result.open_terms,
        beantwoord: beantwoord(root),
        eigen,
    };
    Ok(vec![Stap::Artikel {
        wet: result.law_id.clone(),
        artikel: result.article_number.clone(),
        uitvoer: None,
        stappen: stappen_onder(root, &spoor)?,
    }])
}

fn stappen_onder(node: &PathNode, spoor: &Spoor<'_>) -> Result<Vec<Stap>> {
    let mut uit = Vec::new();
    for child in &node.children {
        uit.extend(stappen_van(child, spoor)?);
    }
    Ok(uit)
}

/// What a trace node gives: one step, or the steps under it.
fn stappen_van(node: &PathNode, spoor: &Spoor<'_>) -> Result<Vec<Stap>> {
    let stap = match node.node_type {
        PathNodeType::Article | PathNodeType::CrossLawReference => {
            match waar(node) {
                Some((wet, artikel)) => Stap::Artikel {
                    wet,
                    artikel,
                    uitvoer: None,
                    stappen: stappen_onder(node, spoor)?,
                },
                // A call named `<law>#<output>` under which no article
                // ran: a cached result, or one that failed. A cached
                // result is a node of its own under the call, and leaves no
                // step besides the call's.
                None => match node.name.split_once('#') {
                    Some((wet, uitvoer)) => Stap::Artikel {
                        wet: wet.to_string(),
                        artikel: String::new(),
                        uitvoer: Some(uitvoer.to_string()),
                        stappen: stappen_onder(node, spoor)?,
                    },
                    // An article node that names no call, such as the
                    // marking of a construct the engine cannot translate:
                    // no step of its own.
                    None => return stappen_onder(node, spoor),
                },
            }
        }
        PathNodeType::HookResolution => {
            let (wet, artikel) = waar(node).unwrap_or_else(|| (node.name.clone(), String::new()));
            let stappen = stappen_onder(node, spoor)?;
            // A hook that fires at every stage of a submission also fires at
            // the decision, where the facts it reads (those of the
            // submission) are not passed. It determines nothing there, and
            // is left out: what it gave is in the submission's
            // chronolexogram.
            let niets = stappen
                .iter()
                .all(|s| matches!(s, Stap::Uitkomst { ontbreekt, .. } if !ontbreekt.is_empty()));
            if niets {
                return Ok(Vec::new());
            }
            Stap::Hook {
                wet,
                artikel,
                stappen,
            }
        }
        PathNodeType::Action => {
            let (waarde, ontbreekt) = uitkomst(node)?;
            Stap::Uitkomst {
                naam: node.name.clone(),
                waarde,
                ontbreekt,
                stappen: stappen_onder(node, spoor)?,
            }
        }
        PathNodeType::OpenTermResolution => {
            let (waarde, ontbreekt) = uitkomst(node)?;
            let (wet, artikel) = node
                .anchor
                .as_ref()
                .map(|a| {
                    (
                        a.law_id.clone().unwrap_or_default(),
                        a.article.clone().unwrap_or_default(),
                    )
                })
                .unwrap_or_default();
            let ingevuld_door = spoor
                .ingevuld
                .iter()
                .find(|f| f.term == node.name && f.law_id == wet && f.article == artikel)
                .map(|f| f.filled_by.clone());
            // The policy bound the term to the answer of one or more cells
            // (RFC-048, RFC-052): each answer has a node below this one. The
            // explanation is of the case's own cell, and only where its
            // answer counted (not silent).
            let antwoorden = antwoorden_van(node);
            let eigen_antwoord = antwoorden.iter().copied().find(|c| {
                c.answer.as_ref().is_some_and(|a| a.answerer == spoor.eigen)
                    && c.result.as_ref().is_some_and(|r| !r.is_null())
            });
            let keer = eigen_antwoord.and_then(|n| keer(&spoor.beantwoord, n));
            // The cell answered a term below this one, in the regulation that
            // filled it: the steps there lead to that answer.
            let onder = beantwoord(node).len() > antwoorden.len();
            let stappen = if onder {
                stappen_onder(node, spoor)?
            } else {
                Vec::new()
            };
            Stap::Begrip {
                naam: node.name.clone(),
                wet,
                artikel,
                waarde,
                ontbreekt,
                ingevuld_door,
                keer,
                stappen,
            }
        }
        _ => return stappen_onder(node, spoor),
    };
    Ok(vec![stap])
}

/// The law and article an article step runs: where the engine was at the
/// first output or hook under it. Resolving the parameters of a call
/// happens where the caller is, so those steps do not count, and neither
/// does a call nested in them. `None` when no article ran under it.
fn waar(node: &PathNode) -> Option<(String, String)> {
    fn eerste(node: &PathNode) -> Option<(String, String)> {
        node.children.iter().find_map(|c| match c.node_type {
            PathNodeType::Action | PathNodeType::HookResolution => c
                .anchor
                .as_ref()
                .and_then(|a| Some((a.law_id.clone()?, a.article.clone()?))),
            PathNodeType::Article | PathNodeType::CrossLawReference | PathNodeType::Cached => None,
            _ => eerste(c),
        })
    }
    eerste(node)
}

/// What a step came to, as stored, and the facts it missed when the engine
/// could not determine it.
fn uitkomst(node: &PathNode) -> Result<(Value, Vec<String>)> {
    match &node.result {
        Some(v) if v.is_unknown() => Ok((
            Value::Null,
            v.missing_facts().iter().map(|m| m.name.clone()).collect(),
        )),
        Some(v) => Ok((stored::to_stored(v)?, Vec::new())),
        None => Ok((Value::Null, Vec::new())),
    }
}

/// One action in the log: a chronolexogram as the cell recorded it. Its
/// reason is what it refers to (`refers_to`); `leidde_tot` are the later
/// ones that refer to it, or that it names as taken in turn (a new decision
/// in the place of a revoked one). For a decision, why it came out so
/// ([`waarom`]), from the decision taken again.
#[derive(Debug, serde::Serialize)]
pub struct Handeling {
    pub gram: Gram,
    pub leidde_tot: Vec<String>,
    pub waarom: Option<Waarom>,
}

/// The log of the case, newest first: every chronolexogram of the case's
/// cell as an action ([`Handeling`]), and the executograms of the other
/// cells of the sphere, such as a registration by the Chamber of Commerce
/// or an examination by the GGD (RFC-051, RFC-052). They are facts the
/// decisions read through the answers of those cells. The other cells'
/// decisions stay out: why they came out so, is their own reasoning. Nothing
/// of it is stored apart from the chronicles; a refused step recorded
/// nothing, so it is not in the log. The chronicles are merged by the day
/// each chronolexogram was recorded on, each in its own order.
pub fn logboek(corpus: &Corpus, cell: &Cellen) -> Result<Vec<Handeling>> {
    let service = corpus.service()?;
    let grams = cell.grams();
    // The other cells' facts first, then the case's own steps, and a
    // stable sort by day: on one day a fact of another cell counts as
    // earlier than a step of the case's cell, which may have read it, and
    // each cell keeps the order it recorded in. Newest first, as the log
    // shows it, puts the case's own step of that day above the fact.
    let mut alle: Vec<&Gram> = Vec::new();
    for andere in &cell.andere {
        alle.extend(andere.grams().iter().filter(|g| g.kind == EXECUTOGRAM));
    }
    alle.extend(grams.iter());
    alle.sort_by_key(|g| g.recorded_on);
    let mut uit = Vec::new();
    for gram in alle.into_iter().rev() {
        let leidde_tot = grams
            .iter()
            .filter(|g| {
                g.refers_to.as_deref() == Some(gram.id.as_str())
                    || gram
                        .outputs
                        .values()
                        .any(|v| v.as_str() == Some(g.id.as_str()))
            })
            .map(|g| g.id.clone())
            .collect();
        let waarom = if is_besluit(&service, gram)? {
            Some(waarom(corpus, cell, &gram.id)?)
        } else {
            None
        };
        uit.push(Handeling {
            gram: gram.clone(),
            leidde_tot,
            waarom,
        });
    }
    Ok(uit)
}

/// Whether `gram` is a decision: taken on what it refers to, of the legal
/// character its article gives a decision. An executogram of a fact, such
/// as the receipt of a judgment, refers to a decision and is none.
fn is_besluit(service: &LawExecutionService, gram: &Gram) -> Result<bool> {
    if gram.refers_to.is_none() || gram.kind == EXECUTOGRAM {
        return Ok(false);
    }
    let (law_id, article) = gram
        .basis
        .split_once('#')
        .ok_or_else(|| Error::Setup(format!("'{}' has no basis", gram.id)))?;
    Ok(decision::derive(service, law_id, article, gram.recorded_on)?.kind == gram.kind)
}

/// The reductions the catalogues of `cells` name, each once, as `law`
/// finds them by `$id`: the corpus holds them with their cells (RFC-048).
/// A reduction it does not find is left out.
pub(crate) fn reducties_in<'l>(
    law: &dyn Fn(&str) -> Option<&'l regelrecht_engine::ArticleBasedLaw>,
    cells: &[&Cell],
) -> Vec<(String, &'l regelrecht_engine::ArticleBasedLaw)> {
    let mut uit: Vec<(String, &regelrecht_engine::ArticleBasedLaw)> = Vec::new();
    for id in cells.iter().flat_map(|cell| cell.answers().values()) {
        if uit.iter().any(|(gezien, _)| gezien == id) {
            continue;
        }
        if let Some(found) = law(id) {
            uit.push((id.clone(), found));
        }
    }
    uit
}

/// What `reduction` reads from other rulework (`source.regulation` with
/// `output`): per input the regelwerk and the output.
fn leest_uit(reduction: &regelrecht_engine::ArticleBasedLaw) -> Vec<(String, String)> {
    reduction
        .articles
        .iter()
        .flat_map(|a| a.get_inputs())
        .filter_map(|i| i.source.as_ref())
        .filter_map(|s| Some((s.regulation.clone()?, s.output.clone()?)))
        .collect()
}

/// The articles that `reduction` calls for a norm (RFC-048): per regelwerk of
/// `UITVOERINGSBELEID` or `REDUCTIE` it reads with `source.regulation`, the
/// articles that give the output it reads ([`artikelen_voor`]), with that
/// regelwerk. Any called reduction counts, of the cell or of no cell, such
/// as that of the Awb. Each article once.
pub(crate) fn beleid_van<'s>(
    service: &'s LawExecutionService,
    reduction: &regelrecht_engine::ArticleBasedLaw,
) -> Vec<(
    &'s regelrecht_engine::ArticleBasedLaw,
    &'s regelrecht_engine::Article,
)> {
    let mut uit: Vec<(
        &'s regelrecht_engine::ArticleBasedLaw,
        &'s regelrecht_engine::Article,
    )> = Vec::new();
    for (regelwerk, output) in leest_uit(reduction) {
        let Some(law) = service.resolver().get_law(&regelwerk).filter(|law| {
            matches!(
                law.regulatory_layer,
                regelrecht_engine::RegulatoryLayer::Uitvoeringsbeleid
                    | regelrecht_engine::RegulatoryLayer::Reductie
            )
        }) else {
            continue;
        };
        for article in artikelen_voor(law, &output) {
            if !uit
                .iter()
                .any(|(l, a)| l.id == law.id && a.number == article.number)
            {
                uit.push((law, article));
            }
        }
    }
    uit
}

/// The article of `law` that gives `output`, then the articles of `law` it
/// reads in turn (`source.output` without `regulation`), each once, in the
/// order they are reached.
fn artikelen_voor<'l>(
    law: &'l regelrecht_engine::ArticleBasedLaw,
    output: &str,
) -> Vec<&'l regelrecht_engine::Article> {
    let mut uit: Vec<&regelrecht_engine::Article> = Vec::new();
    let mut te_doen = vec![output.to_string()];
    while let Some(output) = te_doen.pop() {
        let Some(article) = law
            .articles
            .iter()
            .find(|a| a.get_outputs().iter().any(|o| o.name == output))
        else {
            continue;
        };
        if uit.iter().any(|a| a.number == article.number) {
            continue;
        }
        uit.push(article);
        let eigen: Vec<String> = article
            .get_inputs()
            .iter()
            .filter_map(|i| i.source.as_ref())
            .filter(|s| s.regulation.is_none())
            .filter_map(|s| s.output.clone())
            .collect();
        te_doen.extend(eigen.into_iter().rev());
    }
    uit
}

/// The version of the reduction `id` in `service` valid from `valid_from`,
/// or its newest without one.
fn versies_van<'s>(
    service: &'s LawExecutionService,
    id: &str,
    valid_from: Option<&str>,
) -> Vec<&'s regelrecht_engine::ArticleBasedLaw> {
    let day = valid_from.and_then(|v| NaiveDate::parse_from_str(v, "%Y-%m-%d").ok());
    let law = match day {
        Some(day) => service.resolver().get_law_for_date(id, Some(day)),
        None => service.resolver().get_law(id),
    };
    law.into_iter()
        .filter(|l| valid_from.is_none_or(|v| l.valid_from.as_deref() == Some(v)))
        .collect()
}

/// The output names of the version of the reduction `id` of the cell that
/// is valid from `valid_from`, as written, and those of the articles of
/// policy it calls for a norm (RFC-048).
fn uitvoer_van(service: &LawExecutionService, id: &str, valid_from: Option<&str>) -> Vec<String> {
    let mut namen = Vec::new();
    for reduction in versies_van(service, id, valid_from) {
        let beleid = beleid_van(service, reduction);
        for article in reduction
            .articles
            .iter()
            .chain(beleid.iter().map(|(_, a)| *a))
        {
            for output in article.get_outputs() {
                if !namen.contains(&output.name) {
                    namen.push(output.name.clone());
                }
            }
        }
    }
    namen
}

/// The inputs of the version of the reduction `id` valid from `valid_from`
/// and of the articles of policy it calls for a norm, that come from other
/// rulework (`source.regulation`): per input its name, the regelwerk and the
/// output it reads. The reduction's own reading of the policy is left out:
/// the policy is named apart ([`beleid_van`]).
fn uit_regelgeving(
    service: &LawExecutionService,
    id: &str,
    valid_from: Option<&str>,
) -> Vec<(String, String, String)> {
    let mut uit = Vec::new();
    for reduction in versies_van(service, id, valid_from) {
        let beleid = beleid_van(service, reduction);
        let articles = reduction
            .articles
            .iter()
            .chain(beleid.iter().map(|(_, a)| *a));
        for input in articles.flat_map(|a| a.get_inputs()) {
            let Some(source) = input.source.as_ref() else {
                continue;
            };
            if let (Some(regelwerk), Some(output)) = (&source.regulation, &source.output) {
                if beleid.iter().any(|(b, _)| &b.id == regelwerk) {
                    continue;
                }
                uit.push((input.name.clone(), regelwerk.clone(), output.clone()));
            }
        }
    }
    uit
}

/// The value of the input `naam` in the trace under `node`: where the
/// engine resolved it by calling `aanroep` (`<regelwerk>#<output>`), the
/// first value `output` came to under that call. Two inputs may read one
/// output with other parameters (this summer and the one before), so the
/// call is found under the input's own resolution.
fn invoerwaarde<'a>(
    node: &'a PathNode,
    naam: &str,
    aanroep: &str,
    output: &str,
) -> Option<&'a regelrecht_engine::Value> {
    fn onder<'a>(node: &'a PathNode, pred: &dyn Fn(&PathNode) -> bool) -> Option<&'a PathNode> {
        if pred(node) {
            return Some(node);
        }
        node.children.iter().find_map(|c| onder(c, pred))
    }
    let resolutie = onder(node, &|n| {
        n.node_type == PathNodeType::Resolve
            && n.name.eq_ignore_ascii_case(naam)
            && onder(n, &|c| c.name == aanroep).is_some()
    })?;
    let call = onder(resolutie, &|c| c.name == aanroep)?;
    // A call the engine had made before with the same parameters has a
    // cached node, named as the call, in place of the article's output.
    onder(call, &|c| {
        c.result.is_some()
            && (c.name == output || (c.node_type == PathNodeType::Cached && c.name == aanroep))
    })?
    .result
    .as_ref()
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

/// How the cell came to its answer `antwoord` (its name in the catalogue,
/// RFC-048) in the decision `id` ([`Cell::explain_answer`]): per time the
/// decision asked it, the reduction and its version, what its outputs came to (the
/// answer, and an output it computes on the way, such as the revoked
/// decisions), what it read from other rulework (such as the region of the
/// school from the regeling), the answers the cell gave within it (such
/// as where the school is established), and its own trace.
pub fn hoe_beantwoord(corpus: &Corpus, cell: &Cellen, id: &str, antwoord: &str) -> Result<Value> {
    let mut service = corpus.service()?;
    let mut uit = Vec::new();
    // Per nested answer, how many times the explanations before this one
    // asked it. `keer` indexes the cell's explanations of that answer, which
    // count every time the decision asked it; this assumes it is asked only
    // inside the reductions for `antwoord`, as the answers on where the
    // school is established are now.
    let mut eerder: BTreeMap<String, usize> = Default::default();
    for result in cell.explain_answer(&mut service, &cell.sfeer(), id, antwoord)? {
        let mut uitvoer = Map::new();
        let mut gelezen = Vec::new();
        if let Some(root) = &result.trace {
            let versie = result.regulation_valid_from.as_deref();
            let namen = uitvoer_van(&service, &result.law_id, versie);
            eerste_waarden(root, &namen, &mut uitvoer)?;
            // What it read from other rulework, such as the region of the
            // school from the regeling: per input the value its call gave.
            for (naam, regelwerk, output) in uit_regelgeving(&service, &result.law_id, versie) {
                let aanroep = format!("{regelwerk}#{output}");
                let waarde = invoerwaarde(root, &naam, &aanroep, &output)
                    .map(stored::to_stored)
                    .transpose()?;
                gelezen.push(json!({
                    "naam": naam,
                    "regelwerk": regelwerk,
                    "waarde": waarde,
                }));
            }
        }
        for (naam, waarde) in &result.outputs {
            uitvoer.insert(naam.clone(), stored::to_stored(waarde)?);
        }
        // Where the reduction called rulework whose open term a policy binds
        // to an answer of the cell, such as the regeling asking where the
        // school is established: per answer the term, the answer, its value
        // and the reduction, and `keer`, the index into the explanations of
        // that answer.
        let mut genest = Vec::new();
        if let Some(root) = &result.trace {
            let gevraagd = beantwoord(root);
            for (node, begrip_daar) in beantwoord_per_begrip(root) {
                let Some(answer) = &node.answer else {
                    continue;
                };
                let vooraf = eerder.get(&node.name).copied().unwrap_or(0);
                genest.push(json!({
                    "begrip": begrip_daar,
                    "antwoord": answer.answer,
                    "keer": keer(&gevraagd, node).map(|k| k + vooraf),
                    "waarde": node.result.as_ref().map(stored::to_stored).transpose()?,
                    "reductie": answer.law_id,
                    "geldig_vanaf": answer.valid_from,
                }));
            }
            for node in gevraagd {
                *eerder.entry(node.name.clone()).or_default() += 1;
            }
        }
        uit.push(json!({
            "reductie": result.law_id,
            "geldig_vanaf": result.regulation_valid_from,
            "uitvoer": uitvoer,
            "uit_regelgeving": gelezen,
            "antwoorden": genest,
            "spoor": result
                .trace
                .as_ref()
                .map(PathNode::render_box_drawing)
                .unwrap_or_default(),
        }));
    }
    Ok(json!({ "besluit": id, "antwoord": antwoord, "antwoorden": uit }))
}

/// A decision on an application, and why it came out so.
#[derive(Debug, serde::Serialize)]
pub struct Besluit {
    pub besluit: Gram,
    pub waarom: Waarom,
}

/// Decide on the submission `op` under `law_id#article` with the deciding
/// authority's `bevindingen` (RFC-050): the decision on it first, then a
/// decision it takes in turn.
fn beslissen(
    corpus: &Corpus,
    cell: &mut Cellen,
    law_id: &str,
    article: &str,
    op: &str,
    bevindingen: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Vec<Gram>> {
    let (cell, sfeer) = cell.splits();
    decide(
        cell,
        &mut corpus.service()?,
        &sfeer,
        law_id,
        article,
        op,
        None,
        bevindingen,
        on,
    )
}

/// Decide on the submission `op` under `law_id#article`, after `after` when
/// it is given ([`Cell::decide_after`]), with what the deciding authority
/// gives: the cell splits it into what it supplies and its findings, as the
/// decision asks them ([`decision::DecisionShape::split`]).
#[allow(clippy::too_many_arguments)]
fn decide(
    cell: &mut Cell,
    service: &mut LawExecutionService,
    sfeer: &[&Cell],
    law_id: &str,
    article: &str,
    op: &str,
    after: Option<&str>,
    gegeven: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Vec<Gram>> {
    let (supplied, findings) = decision::derive(service, law_id, article, on)?.split(gegeven);
    Ok(match after {
        None => cell.decide(
            service, sfeer, law_id, article, op, &supplied, &findings, on,
        )?,
        Some(after) => cell.decide_after(
            service, sfeer, law_id, article, op, after, &supplied, &findings, on,
        )?,
    })
}

/// The deciding authority decides on the application `aanvraag` with its findings.
pub fn besluiten(
    corpus: &Corpus,
    cell: &mut Cellen,
    aanvraag: &str,
    bevindingen: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Besluit> {
    let besluit = beslissen(
        corpus,
        cell,
        corpus.decision_law(),
        corpus.decision(),
        aanvraag,
        bevindingen,
        on,
    )?
    .remove(0);
    let waarom = waarom(corpus, cell, &besluit.id)?;
    Ok(Besluit { besluit, waarom })
}

/// The applicant objects to the decision `besluit`: a submission under Awb
/// 7:11 that refers to it (RFC-050).
pub fn bezwaren(
    corpus: &Corpus,
    cell: &mut Cellen,
    besluit: &str,
    velden: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Gram> {
    let (cell, sfeer) = cell.splits();
    cell.record_submission(
        &mut corpus.service()?,
        &sfeer,
        corpus.awb(),
        corpus.objection(),
        Some(Reference::Local(besluit)),
        velden,
        on,
    )
}

/// Before the authority decides on the objection `bezwaar` on `on`: whether it
/// is admissible, as Awb 7:11 decides it (on time per 6:9, and not against
/// a decision taken on objection), and, if so, what the authority can find anew
/// at the heroverweging. Those are the findings the contested decision
/// used ([`Cell::findings`]), with what the authority found then. A term the
/// cell answered is none of them.
///
/// The admissibility is that of the decision without new findings, taken
/// on a copy of the cell: the law decides it, not the app. Nothing is kept.
pub fn heroverweging(
    corpus: &Corpus,
    cell: &Cellen,
    bezwaar: &str,
    on: NaiveDate,
) -> Result<Value> {
    let toets = beslissen(
        corpus,
        &mut cell.sandbox(),
        corpus.awb(),
        corpus.objection(),
        bezwaar,
        &Map::new(),
        on,
    )?
    .remove(0);
    let ontvankelijk = toets.outputs.get("bezwaar_ontvankelijk") == Some(&json!(true));
    let bestreden = bestreden(cell, bezwaar)?;
    let bevindingen = if ontvankelijk {
        opnieuw_te_vinden(corpus, cell, bestreden)?
    } else {
        Vec::new()
    };
    Ok(json!({
        "bezwaar": bezwaar,
        "bestreden_besluit": bestreden.id,
        "ontvankelijk": ontvankelijk,
        "tijdig": toets.fields.get("bezwaar_of_beroepschrift_tijdig_ingediend"),
        "uitvoer": toets.outputs,
        "bevindingen": bevindingen,
    }))
}

/// The decision the objection `bezwaar` contests.
fn bestreden<'a>(cell: &'a Cellen, bezwaar: &str) -> Result<&'a Gram> {
    let ingediend = cell
        .grams()
        .iter()
        .find(|g| g.id == bezwaar)
        .ok_or_else(|| Error::Refused(format!("there is no objection '{bezwaar}'")))?;
    ingediend
        .refers_to
        .as_deref()
        .and_then(|id| cell.grams().iter().find(|g| g.id == id))
        .ok_or_else(|| Error::Refused(format!("'{bezwaar}' contests no decision")))
}

/// The names of the findings the authority can give anew on an objection
/// against the decision `bestreden` finds ([`opnieuw_te_vinden`]), for a
/// preview with `bevindingen`. Without any finding there is nothing to pass
/// on, and the contested decision is not looked up. The form gives none on
/// an objection that is not admissible ([`heroverweging`]).
fn opnieuw_gevraagd<'a>(
    corpus: &Corpus,
    cell: &'a Cellen,
    bevindingen: &Map<String, Value>,
    bestreden: impl FnOnce() -> Result<&'a Gram>,
) -> Result<Vec<String>> {
    if bevindingen.is_empty() {
        return Ok(Vec::new());
    }
    Ok(opnieuw_te_vinden(corpus, cell, bestreden()?)?
        .iter()
        .filter_map(|v| v["naam"].as_str().map(str::to_string))
        .collect())
}

/// What the authority can find anew on an objection against `bestreden`: the
/// findings that decision used ([`Cell::findings`]), each with what the
/// authority found then (`eerder`).
fn opnieuw_te_vinden(corpus: &Corpus, cell: &Cellen, bestreden: &Gram) -> Result<Vec<Value>> {
    let (law_id, article) = bestreden
        .basis
        .split_once('#')
        .ok_or_else(|| Error::Setup(format!("'{}' has no basis", bestreden.id)))?;
    let mut service = corpus.service()?;
    let shape = decision::derive(&service, law_id, article, bestreden.recorded_on)?;
    let mut bevindingen = Vec::new();
    for naam in cell.findings(&mut service, &cell.sfeer(), &bestreden.id)? {
        let Some(term) = shape.finding(&naam, cell.authority()) else {
            continue;
        };
        let mut v = veld(&decision::finding_field(term));
        v["eerder"] = bestreden.fields.get(&naam).cloned().unwrap_or(Value::Null);
        bevindingen.push(v);
    }
    Ok(bevindingen)
}

/// The authority decides on the objection `bezwaar` under Awb 7:11, as any
/// decision; `bevindingen` are what the authority finds anew at the
/// heroverweging. The decision comes with why ([`waarom`]): the case's
/// article called anew is in its trace, a new decision in place of the
/// revoked one is its outcome.
pub fn beslissen_op_bezwaar(
    corpus: &Corpus,
    cell: &mut Cellen,
    bezwaar: &str,
    bevindingen: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Value> {
    let genomen = beslissen(
        corpus,
        cell,
        corpus.awb(),
        corpus.objection(),
        bezwaar,
        bevindingen,
        on,
    )?;
    op_bezwaar(corpus, cell, genomen)
}

/// The answer to a decision on objection that `genomen` took: the decision,
/// the new decision it took in turn if any, and why.
fn op_bezwaar(corpus: &Corpus, cell: &Cellen, mut genomen: Vec<Gram>) -> Result<Value> {
    let nieuw = (genomen.len() > 1).then(|| genomen.remove(1));
    let beslissing = genomen.remove(0);
    let waarom = waarom(corpus, cell, &beslissing.id)?;
    Ok(json!({
        "beslissing": beslissing,
        "nieuw_besluit": nieuw,
        "waarom": waarom,
    }))
}

/// A preview of a decision ([`voorvertoning_besluit`]): what it would come
/// to, or the cell's refusal, and the terms the authority must judge
/// because a policy leaves them to it for this case.
#[derive(Debug, serde::Serialize)]
pub struct Voorvertoning<T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voorlopig: Option<T>,
    /// What the cell refuses, in its own words: a finding not given yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geweigerd: Option<String>,
    /// The findings a policy leaves to the authority for this case, with an
    /// open term of its own (RFC-048 §2), as fields of the form: the
    /// authority judges them. Such a field says so (`beleid`).
    pub stil: Vec<Value>,
}

/// The open term that the engine says is required and has no judgement, in
/// its own message: `Required open term '<term>' on <law>#<article> ...`.
fn zonder_oordeel(e: &Error) -> Option<(String, String)> {
    let Error::Engine(regelrecht_engine::EngineError::ResolutionError(m)) = e else {
        return None;
    };
    let rest = m.strip_prefix("Required open term '")?;
    let (term, rest) = rest.split_once("' on ")?;
    let (waar, _) = rest.split_once(' ')?;
    m.contains("no judgement was given")
        .then(|| (term.to_string(), waar.to_string()))
}

/// Take a decision on a copy of the cell ([`Cell::sandbox`]) with
/// `bevindingen`, and find which findings a policy leaves to the authority
/// for this case. The form asks the findings of the deciding article
/// itself; a policy that fills a term may leave a case to the authority with
/// an open term of its own (RFC-048 §2): the school's holiday rooster does
/// not reach that school year, and without an advice of the GGD the parking
/// card's policy leaves the four criteria to the college. Whether a case
/// reaches such a term shows only by asking: the decision is taken without a
/// finding on any of them, and each the engine then misses is asked. With
/// the authority's finding on it, the next one may surface; without, that
/// is where it stops. Nothing is kept.
///
/// `gevraagd` are the findings the form already asks, whether or not a
/// policy declares them: on an objection those of the contested decision
/// ([`opnieuw_te_vinden`]). The decision on objection takes the earlier
/// finding on such a term when none is given anew, so the engine never
/// misses it; a finding the authority gives on it goes in as given.
fn voorvertonen<T>(
    corpus: &Corpus,
    cell: &Cellen,
    bevindingen: &Map<String, Value>,
    gevraagd: &[String],
    on: NaiveDate,
    neem: impl Fn(&mut Cellen, &Map<String, Value>) -> Result<T>,
) -> Result<Voorvertoning<T>> {
    let service = corpus.service()?;
    // A term a policy binds to the answer of a cell of the case takes no
    // finding.
    let beantwoord = cell.gebonden(&service, on);
    let van_cel = |naam: &str| beantwoord.iter().any(|b| b.term.open_term == naam);
    // The decision the application asks for must derive: its error is the
    // step's error.
    let eigen_shape = decision::derive(&service, corpus.decision_law(), corpus.decision(), on)?;
    // A term the policy fills takes no finding; one a policy leaves to the
    // authority is given once the case reaches it.
    let wie = cell.eigen.authority();
    let van_beleid = |naam: &str| {
        eigen_shape.filled(naam)
            || eigen_shape.terms.iter().any(|t| {
                t.id == naam && t.is_finding(wie) && t.declared_by() != eigen_shape.basis()
            })
    };
    let mut gegeven: Map<String, Value> = Map::new();
    for (naam, waarde) in bevindingen {
        if gevraagd.contains(naam) || (!van_cel(naam) && !van_beleid(naam)) {
            gegeven.insert(naam.clone(), waarde.clone());
        }
    }
    // Per term asked its name, the article it is open on, and whether a
    // policy leaves it to the authority.
    let mut stil: Vec<(String, String, bool)> = Vec::new();
    let uitkomst = loop {
        let e = match neem(&mut cell.sandbox(), &gegeven) {
            Ok(v) => break Ok(v),
            Err(e) => e,
        };
        let Some((term, waar)) = zonder_oordeel(&e) else {
            break Err(e);
        };
        let beleid = beleid_laat_oordelen(&service, &term, &waar, on).is_some();
        // A judgement of the authority that shares its name with a term
        // of the cell elsewhere: give it back, as the real decision would.
        if !gegeven.contains_key(&term) && bevindingen.contains_key(&term) && !beleid {
            gegeven.insert(term.clone(), bevindingen[&term].clone());
            continue;
        }
        if !beleid || stil.iter().any(|(t, _, _)| *t == term) {
            break Err(e);
        }
        stil.push((term.clone(), waar, beleid));
        match bevindingen.get(&term) {
            Some(oordeel) => {
                gegeven.insert(term, oordeel.clone());
            }
            None => break Err(e),
        }
    };
    let (voorlopig, geweigerd) = match uitkomst {
        Ok(v) => (Some(v), None),
        Err(e) if toont(&e, &stil, bevindingen) => (None, Some(geweigerd_om(&e, &stil))),
        Err(e) => return Err(e),
    };
    let stil = stil
        .iter()
        .map(|(term, waar, beleid)| {
            let mut veld = stil_veld(&service, term, waar, on);
            if *beleid {
                veld["beleid"] = json!(true);
            }
            veld
        })
        .collect();
    Ok(Voorvertoning {
        voorlopig,
        geweigerd,
        stil,
    })
}

/// The field for the silent term `term`, open on `waar` (`<law>#<article>`),
/// as a decision takes the finding on it ([`decision::finding_field`]), and
/// otherwise with only its name and where it is open, so the form still
/// asks it.
fn stil_veld(service: &LawExecutionService, term: &str, waar: &str, on: NaiveDate) -> Value {
    beleid_laat_oordelen(service, term, waar, on)
        .map(|t| veld(&decision::finding_field(&t)))
        .unwrap_or_else(|| json!({ "naam": term, "type": null, "mag_leeg": false, "basis": waar }))
}

/// The refusal as the preview shows it. A judgement missing on a term a
/// policy leaves to the authority for this case says so first, so the app
/// does not take a missing judgement of another kind for it.
fn geweigerd_om(e: &Error, stil: &[(String, String, bool)]) -> String {
    match zonder_oordeel(e) {
        Some((term, _)) if stil.iter().any(|(t, _, _)| *t == term) => {
            format!("the policy leaves '{term}' to the authority for this case: {e}")
        }
        _ => e.to_string(),
    }
}

/// Whether the preview shows `e` as a refusal: the cell's own, or the
/// engine missing the judgement on a term a policy leaves to the authority,
/// which it has not given yet.
fn toont(e: &Error, stil: &[(String, String, bool)], bevindingen: &Map<String, Value>) -> bool {
    match e {
        Error::Refused(_) => true,
        e => zonder_oordeel(e).is_some_and(|(term, _)| {
            stil.iter().any(|(t, _, _)| *t == term) && !bevindingen.contains_key(&term)
        }),
    }
}

/// What the decision on the application `aanvraag` would be with these
/// findings on `on`, before the authority takes it: [`besluiten`] on a copy
/// of the cell with the chronicle in memory ([`voorvertonen`]), so the
/// outcome and the chain are those of the real decision. Nothing is kept.
pub fn voorvertoning_besluit(
    corpus: &Corpus,
    cell: &Cellen,
    aanvraag: &str,
    bevindingen: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Voorvertoning<Besluit>> {
    voorvertonen(corpus, cell, bevindingen, &[], on, |c, b| {
        besluiten(corpus, c, aanvraag, b, on)
    })
}

/// What the decision on the objection `bezwaar` would be with these
/// findings anew, and the new decision that would follow: as
/// [`voorvertoning_besluit`], with [`beslissen_op_bezwaar`]. Nothing is
/// kept.
pub fn voorvertoning_op_bezwaar(
    corpus: &Corpus,
    cell: &Cellen,
    bezwaar: &str,
    bevindingen: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Voorvertoning<Value>> {
    let gevraagd = opnieuw_gevraagd(corpus, cell, bevindingen, || bestreden(cell, bezwaar))?;
    voorvertonen(corpus, cell, bevindingen, &gevraagd, on, |c, b| {
        beslissen_op_bezwaar(corpus, c, bezwaar, b, on)
    })
}

/// What an input reads from the chronicle: the decisions under an article,
/// or the executograms of a fact.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Bron {
    Besluiten(String),
    Feiten(String),
}

/// One input of a reduction that reads the chronicle: its name, the
/// reduction it belongs to, and what it reads.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Lezing {
    naam: String,
    reductie: String,
    bron: Bron,
}

/// The inputs of `reduction` that read the chronicle, and those of the
/// reductions it calls (`source.regulation` naming a `REDUCTIE`), which
/// read the same chronicle: that of the cell that runs it (RFC-048). Of a
/// called reduction only the articles that give the output it reads count
/// ([`artikelen_voor`]). A `decisions: $<parameter>` names the article the
/// calling input passes under that name (`source.parameters`), as written;
/// a reference the PoC cannot resolve that way is left out. Each read
/// once, in the order of the articles.
fn kroniek_lezingen(
    service: &LawExecutionService,
    reduction: &regelrecht_engine::ArticleBasedLaw,
    on: NaiveDate,
) -> Vec<Lezing> {
    let mut uit = Vec::new();
    let articles: Vec<&regelrecht_engine::Article> = reduction.articles.iter().collect();
    lezingen_van(
        service,
        reduction,
        &articles,
        &BTreeMap::new(),
        &mut vec![reduction.id.clone()],
        on,
        &mut uit,
    );
    uit
}

/// [`kroniek_lezingen`] for `articles` of `reduction`, with `given` as the
/// literal parameters of its caller. `bezig` holds the reductions being
/// followed, so a reduction that calls back is not followed again.
fn lezingen_van(
    service: &LawExecutionService,
    reduction: &regelrecht_engine::ArticleBasedLaw,
    articles: &[&regelrecht_engine::Article],
    given: &BTreeMap<String, String>,
    bezig: &mut Vec<String>,
    on: NaiveDate,
    uit: &mut Vec<Lezing>,
) {
    for input in articles.iter().flat_map(|a| a.get_inputs()) {
        let Some(source) = input.source.as_ref() else {
            continue;
        };
        let bron = match (&source.decisions, &source.facts) {
            (Some(basis), _) => match basis.strip_prefix('$') {
                None => Some(Bron::Besluiten(basis.clone())),
                Some(naam) => given.get(naam).cloned().map(Bron::Besluiten),
            },
            (None, Some(feit)) => Some(Bron::Feiten(feit.clone())),
            (None, None) => None,
        };
        if let Some(bron) = bron {
            let lezing = Lezing {
                naam: input.name.clone(),
                reductie: reduction.id.clone(),
                bron,
            };
            if !uit.contains(&lezing) {
                uit.push(lezing);
            }
            continue;
        }
        let (Some(regelwerk), Some(output)) = (&source.regulation, &source.output) else {
            continue;
        };
        if bezig.contains(regelwerk) {
            continue;
        }
        let Some(called) = service
            .resolver()
            .get_law_for_date(regelwerk, Some(on))
            .filter(|law| law.regulatory_layer == regelrecht_engine::RegulatoryLayer::Reductie)
        else {
            continue;
        };
        let letterlijk: BTreeMap<String, String> = source
            .parameters
            .iter()
            .flatten()
            .filter(|(_, waarde)| !waarde.starts_with('$'))
            .map(|(naam, waarde)| (naam.clone(), waarde.clone()))
            .collect();
        bezig.push(called.id.clone());
        lezingen_van(
            service,
            called,
            &artikelen_voor(called, output),
            &letterlijk,
            bezig,
            on,
            uit,
        );
        bezig.pop();
    }
}

/// The bindings of policy in force on `on` that take an answer of the cell
/// (`antwoorden`, RFC-048), and the reductions its catalogue names
/// ([`Cell::answers`]): per reduction its text and as written, the answers
/// the cell gives from it, the policy it calls for a norm, and what it
/// reads from the chronicle, as rows: the decisions under an article
/// (`source.decisions`) or the executograms of a fact an article requires
/// (`source.facts`).
pub fn reducties(corpus: &Corpus, cell: &Cellen, on: NaiveDate) -> Result<Value> {
    let service = corpus.service()?;
    let mut uit = Vec::new();
    for (id, reduction) in reducties_in(
        &|id: &str| service.resolver().get_law_for_date(id, Some(on)),
        &[&cell.eigen],
    ) {
        let mut invoer = Vec::new();
        // Only a reduction reads a chronicle (RFC-048): this one, and the
        // reductions it calls, on the same chronicle.
        for lezing in kroniek_lezingen(&service, reduction, on) {
            let (bron, rijen) = match &lezing.bron {
                Bron::Besluiten(basis) => {
                    let kind = reading::decision_kind(&service, basis, on)?;
                    (
                        json!({ "besluiten_onder": basis }),
                        reading::rows(&service, cell.grams(), basis, &kind)?,
                    )
                }
                Bron::Feiten(feit) => (
                    json!({ "executogrammen_van": feit }),
                    reading::executogram_rows(&service, cell.grams(), feit)?,
                ),
            };
            let mut rij = object(bron)?;
            rij.insert("naam".into(), json!(lezing.naam));
            rij.insert("reductie".into(), json!(lezing.reductie));
            rij.insert(
                "rijen".into(),
                serde_json::to_value(&rijen).map_err(|e| Error::Setup(e.to_string()))?,
            );
            invoer.push(Value::Object(rij));
        }
        // The articles of policy or of a reduction it calls for a norm
        // (RFC-048), each once, with the layer of their regelwerk.
        let mut normen: Vec<Value> = Vec::new();
        for (regelwerk, article) in beleid_van(&service, reduction) {
            let norm = json!({
                "regelwerk": regelwerk.id,
                "laag": regelwerk.regulatory_layer.as_str(),
                "artikel": article.number,
            });
            if !normen.contains(&norm) {
                normen.push(norm);
            }
        }
        uit.push(json!({
            "id": id,
            "geldig_vanaf": reduction.valid_from,
            "yaml": corpus.yaml_van(&id, &reduction.valid_from),
            "tekst": reduction.articles.iter().map(|a| json!({
                "artikel": a.number,
                "tekst": a.text,
            })).collect::<Vec<_>>(),
            // The answers the cell's catalogue gives from it (`cel.yaml`).
            "antwoorden": cell
                .answers()
                .iter()
                .filter(|(_, r)| **r == id)
                .map(|(antwoord, _)| antwoord)
                .collect::<Vec<_>>(),
            "normen": normen,
            "invoer": invoer,
        }));
    }
    // Per open term a policy binds to an answer of the cell: the term, the
    // answer and its reduction, and the binding (RFC-048).
    let mut antwoorden = Vec::new();
    for binding in reading::bindings(&service, on) {
        let Some(reductie) = cell.answers().get(&binding.answer) else {
            continue;
        };
        let geldig_vanaf = service
            .resolver()
            .get_law_for_date(reductie, Some(on))
            .and_then(|law| law.valid_from.clone());
        antwoorden.push(json!({
            "term": binding.term,
            "antwoord": binding.answer,
            "reduction": reductie,
            "valid_from": geldig_vanaf,
            "beleid": binding.law_id,
            "artikel": binding.article,
        }));
    }
    Ok(json!({ "antwoorden": antwoorden, "reducties": uit }))
}

/// The applicant appeals to the court against the decision `besluit` of the
/// case's cell (RFC-053): the cell makes a copy of it ([`Cell::afschrift`]),
/// and the court records the appeal with that copy, received on `on`
/// (`Reference::Received`). Whether the appeal is admissible, the judgment
/// says (Awb 8:70), not this step. Each cell takes the others along as its
/// sphere.
pub fn beroep_instellen(
    corpus: &Corpus,
    cellen: &mut Cellen,
    besluit: &str,
    velden: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Gram> {
    let beroep = corpus.beroep()?;
    let afschrift = cellen.afschrift(&mut corpus.service()?, &cellen.sfeer(), besluit)?;
    let (rechter, sfeer) = cellen.splits_op(&beroep.rechter)?;
    rechter.record_submission(
        &mut corpus.service()?,
        &sfeer,
        &beroep.law,
        &beroep.article,
        Some(Reference::Received(&afschrift)),
        velden,
        on,
    )
}

/// The court gives judgment on the appeal `beroep` with its `bevindingen`,
/// as any decision. A finding on an article the judgment reads, such as
/// Awb 8:72, or on a decision it calls anew (8:72 lid 3 onder b), is asked
/// only when it reads that article: a finding the engine reports as asked
/// by none is not given, and the answer names it under `niet_gevraagd`.
/// The judgment comes with why ([`waarom`]).
pub fn uitspraak_doen(
    corpus: &Corpus,
    cellen: &mut Cellen,
    beroep: &str,
    bevindingen: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Value> {
    let b = corpus.beroep()?;
    let neem = |cellen: &mut Cellen, gegeven: &Map<String, Value>| -> Result<Vec<Gram>> {
        let (rechter, sfeer) = cellen.splits_op(&b.rechter)?;
        decide(
            rechter,
            &mut corpus.service()?,
            &sfeer,
            &b.law,
            &b.article,
            beroep,
            None,
            gegeven,
            on,
        )
    };
    // Leave out what the engine names as asked by none, until the judgment
    // asks every finding it is given. Leaving out a finding nothing reads
    // changes nothing else; any other refusal is the court's to see.
    let mut gegeven = bevindingen.clone();
    while let Err(Error::Engine(EngineError::FindingsNotAsked { findings })) =
        neem(&mut cellen.sandbox(), &gegeven)
    {
        let voor = gegeven.len();
        for k in &findings {
            gegeven.remove(k);
        }
        if gegeven.len() == voor {
            break;
        }
    }
    let gegeven = &gegeven;
    let uitspraak = neem(cellen, gegeven)?.remove(0);
    let niet_gevraagd: Vec<&String> = bevindingen
        .keys()
        .filter(|k| !gegeven.contains_key(*k))
        .collect();
    let waarom = waarom_uitspraak(corpus, cellen, &uitspraak.id)?;
    Ok(json!({
        "uitspraak": uitspraak,
        "niet_gevraagd": niet_gevraagd,
        "waarom": waarom,
    }))
}

/// Why the court's judgment `id` came out as it did, as [`waarom`] in the
/// court's cell, with the other cells of the case as its sphere.
pub fn waarom_uitspraak(corpus: &Corpus, cellen: &Cellen, id: &str) -> Result<Waarom> {
    let b = corpus.beroep()?;
    let (rechter, sfeer) = cellen.op(&b.rechter)?;
    waarom_in(corpus, rechter, &sfeer, id)
}

/// What the judgment on the appeal `beroep` would be with these findings:
/// [`uitspraak_doen`] on a copy of the cells ([`voorvertonen`]). Nothing is
/// kept.
pub fn voorvertoning_uitspraak(
    corpus: &Corpus,
    cellen: &Cellen,
    beroep: &str,
    bevindingen: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Voorvertoning<Value>> {
    voorvertonen(corpus, cellen, bevindingen, &[], on, |c, b| {
        uitspraak_doen(corpus, c, beroep, b, on)
    })
}

/// The court sends a copy of its judgment `uitspraak` to the case's cell,
/// which records the receipt on `on` as the fact the law requires for it
/// ([`Cell::record_receipt`]). The receipt refers to the decision of the
/// cell the judgment concerns: what the copy is `about`.
pub fn toezenden(
    corpus: &Corpus,
    cellen: &mut Cellen,
    uitspraak: &str,
    on: NaiveDate,
) -> Result<Gram> {
    let b = corpus.beroep()?;
    let mut service = corpus.service()?;
    let afschrift = {
        let (rechter, sfeer) = cellen.op(&b.rechter)?;
        rechter.afschrift(&mut service, &sfeer, uitspraak)?
    };
    let cell = &mut cellen.eigen;
    let besluit = afschrift
        .about
        .as_ref()
        .filter(|a| a.from == cell.recording_actor())
        .map(|a| a.id.clone())
        .ok_or_else(|| {
            Error::Refused(format!(
                "the judgment '{uitspraak}' concerns no decision of '{}'",
                cell.recording_actor()
            ))
        })?;
    cell.record_receipt(&service, &b.ontvangst, &afschrift, &besluit, on)
}

/// The receipt `ontvangst` of a judgment on a decision on objection, and
/// what it leads back to in the chronicle: that decision, the objection,
/// and the decision the objection contested.
fn na_uitspraak<'c>(corpus: &Corpus, cell: &'c Cell, ontvangst: &str) -> Result<[&'c Gram; 4]> {
    let b = corpus.beroep()?;
    let gram = |id: Option<&str>, wat: &str| {
        id.and_then(|id| cell.grams().iter().find(|g| g.id == id))
            .ok_or_else(|| Error::Refused(format!("'{ontvangst}' leads to no {wat}")))
    };
    let receipt = gram(Some(ontvangst), "receipt of a judgment")?;
    let (basis, feit) = shape::split_fact(&b.ontvangst)?;
    if receipt.kind != EXECUTOGRAM
        || receipt.basis != basis
        || receipt.fact.as_deref() != Some(feit)
    {
        return Err(Error::Refused(format!(
            "'{ontvangst}' is no receipt of the fact {}",
            b.ontvangst
        )));
    }
    let beslissing = gram(receipt.refers_to.as_deref(), "decision")?;
    let op_bezwaar = format!("{}#{}", corpus.awb(), corpus.objection());
    if beslissing.basis != op_bezwaar {
        return Err(Error::Refused(format!(
            "the judgment received in '{ontvangst}' concerns no decision under {op_bezwaar}"
        )));
    }
    let bezwaar = gram(beslissing.refers_to.as_deref(), "objection")?;
    let bestreden = gram(bezwaar.refers_to.as_deref(), "contested decision")?;
    Ok([receipt, beslissing, bezwaar, bestreden])
}

/// Before the authority decides on the objection again after the judgment
/// received in `ontvangst` (Awb 8:72 lid 4): the objection, what the
/// judgment holds, and what the authority can find anew. Those are the
/// findings of the decision the objection contested, as at the first
/// heroverweging ([`heroverweging`]).
pub fn heroverweging_na_uitspraak(
    corpus: &Corpus,
    cell: &Cellen,
    ontvangst: &str,
) -> Result<Value> {
    let [receipt, beslissing, bezwaar, bestreden] = na_uitspraak(corpus, cell, ontvangst)?;
    Ok(json!({
        "ontvangst": receipt.id,
        "uitspraak": receipt.fields,
        "beslissing": beslissing.id,
        "bezwaar": bezwaar.id,
        "bestreden_besluit": bestreden.id,
        "bevindingen": opnieuw_te_vinden(corpus, cell, bestreden)?,
    }))
}

/// The authority decides on the objection again after the judgment received
/// in `ontvangst` ([`Cell::decide_after`]), with what it finds anew. The
/// answer is that of [`beslissen_op_bezwaar`].
pub fn opnieuw_beslissen_op_bezwaar(
    corpus: &Corpus,
    cellen: &mut Cellen,
    ontvangst: &str,
    bevindingen: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Value> {
    let bezwaar = na_uitspraak(corpus, cellen, ontvangst)?[2].id.clone();
    let (cell, sfeer) = cellen.splits();
    let genomen = decide(
        cell,
        &mut corpus.service()?,
        &sfeer,
        corpus.awb(),
        corpus.objection(),
        &bezwaar,
        Some(ontvangst),
        bevindingen,
        on,
    )?;
    op_bezwaar(corpus, cellen, genomen)
}

/// What deciding on the objection again after the judgment received in
/// `ontvangst` would come to: as [`voorvertoning_op_bezwaar`], with
/// [`opnieuw_beslissen_op_bezwaar`]. Nothing is kept.
pub fn voorvertoning_na_uitspraak(
    corpus: &Corpus,
    cell: &Cellen,
    ontvangst: &str,
    bevindingen: &Map<String, Value>,
    on: NaiveDate,
) -> Result<Voorvertoning<Value>> {
    let gevraagd = opnieuw_gevraagd(corpus, cell, bevindingen, || {
        Ok(na_uitspraak(corpus, cell, ontvangst)?[3])
    })?;
    voorvertonen(corpus, cell, bevindingen, &gevraagd, on, |c, b| {
        opnieuw_beslissen_op_bezwaar(corpus, c, ontvangst, b, on)
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const LAW: &str = "leerplichtwet_1969";
    const ARTICLE: &str = "13a";
    const SCHOOL: &str = "obs_de_klimop";
    const ROOSTER: &str = "fictief_beleid_vakantierooster_obs_de_klimop";

    /// The case `id` from `corpus/casussen/`, on the corpus of this crate.
    fn casus(id: &str) -> Corpus {
        static ALLE: std::sync::OnceLock<Vec<Corpus>> = std::sync::OnceLock::new();
        let alle = ALLE.get_or_init(|| {
            let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
            Corpus::alle(&dir.join("corpus"), &dir.join("corpus/casussen")).unwrap()
        });
        alle.iter().find(|c| c.casus.id == id).unwrap().clone()
    }

    fn corpus() -> Corpus {
        casus("vakantieverlof")
    }

    /// The court's cell among the other cells of the case.
    fn rechtbank(cell: &Cellen) -> &Cell {
        cell.op("rechtbank_wilgendaal").unwrap().0
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

    /// What the head finds: only the profession of a parent. Whether the
    /// leave falls outside the school holidays the cell answers.
    fn bevindingen() -> Map<String, Value> {
        beroep(true)
    }

    fn bezwaar_velden(dagtekening: &str) -> Map<String, Value> {
        map(json!({
            "bezwaar_of_beroepschrift_ondertekend": true,
            "naam_indiener": "J. Jansen",
            "adres_indiener": "Voorbeeldstraat 1",
            "dagtekening_bezwaar_of_beroepschrift": dagtekening,
            "omschrijving_bestreden_besluit": "weigering vakantieverlof",
            "gronden_bezwaar_of_beroep": "verklaring van de werkgever: geen verlof in de schoolvakanties",
        }))
    }

    fn beroep(oordeel: bool) -> Map<String, Value> {
        map(json!({ "specifieke_aard_beroep_ouder": oordeel }))
    }

    /// Five school days, applied for on 13 October and refused on 20
    /// October: the head judged the profession of a parent not of the kind
    /// art. 11 onder f asks.
    fn geweigerd(corpus: &Corpus, cell: &mut Cellen) -> Besluit {
        let app = aanvragen(corpus, cell, &aanvraag("2027-02-05"), day("2026-10-13")).unwrap();
        besluiten(corpus, cell, &app.id, &beroep(false), day("2026-10-20")).unwrap()
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
        // The cell answers verlof_al_verleend_in_schooljaar and
        // verlof_valt_buiten_schoolvakanties from its chronicle, so the head
        // is not asked them.
        assert_eq!(
            namen("/besluit/bevindingen"),
            ["specifieke_aard_beroep_ouder"]
        );
        let invoer = vormen["besluit"]["invoer"].as_array().unwrap();
        for begrip in [
            "verlof_al_verleend_in_schooljaar",
            "verlof_valt_buiten_schoolvakanties",
        ] {
            assert!(invoer.contains(&json!({ "naam": begrip, "bron": "kroniek van de cel" })));
        }
        let bezwaar = namen("/bezwaar/velden");
        assert!(bezwaar.contains(&"gronden_bezwaar_of_beroep".to_string()));
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
        // reads the terms the cell answers; the head's finding is what the
        // decision used.
        assert!(uitleg.antwoorden.is_empty(), "{:?}", uitleg.antwoorden);
        assert_eq!(uitleg.bevindingen, ["specifieke_aard_beroep_ouder"]);

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
        assert_eq!(gevraagd, [("specifieke_aard_beroep_ouder", json!(false))]);
        // The school's vestiging, the application, the refusal and the
        // objection.
        assert_eq!(cell.grams().len(), 4);

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

        // The cell answers two terms of art. 13a from its chronicle, and
        // the three of the regeling on where the school is established.
        let stand = reducties(&corpus, &cell, day("2027-03-01")).unwrap();
        let beantwoord: Vec<(&str, &str)> = stand["antwoorden"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                (
                    a["term"]["open_term"].as_str().unwrap(),
                    a["reduction"].as_str().unwrap(),
                )
            })
            .collect();
        // In the order of the bindings in the head's policy (RFC-048).
        assert_eq!(
            beantwoord,
            [
                (
                    "verlof_al_verleend_in_schooljaar",
                    "reductie_verlof_al_verleend_in_schooljaar"
                ),
                (
                    "verlof_valt_buiten_schoolvakanties",
                    "reductie_verlof_valt_buiten_schoolvakanties"
                ),
                ("gemeente_van_vestiging", "reductie_plaats_van_vestiging"),
                ("provincie_van_vestiging", "reductie_plaats_van_vestiging"),
                (
                    "deel_van_gemeente_van_vestiging",
                    "reductie_plaats_van_vestiging"
                ),
            ]
        );
        let reductie = |id: &str| {
            stand["reducties"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["id"] == json!(id))
                .unwrap()
        };
        let rijen = |id: &str, sleutel: &str, bron: &str| {
            reductie(id)["invoer"]
                .as_array()
                .unwrap()
                .iter()
                .find(|i| i[sleutel] == json!(bron))
                .unwrap()["rijen"]
                .as_array()
                .unwrap()
                .len()
        };
        // Lid 2 reads the decisions under 13a and those under 7:11, through
        // the reduction of the Awb it calls, which names 13a by its
        // parameter basis.
        let lid_2 = "reductie_verlof_al_verleend_in_schooljaar";
        assert_eq!(rijen(lid_2, "besluiten_onder", "leerplichtwet_1969#13a"), 2);
        assert!(reductie(lid_2)["invoer"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["reductie"] == json!("reductie_algemene_wet_bestuursrecht")));
        assert_eq!(
            rijen(lid_2, "besluiten_onder", "algemene_wet_bestuursrecht#7:11"),
            1
        );
        // The vestiging is read from its executograms. The holidays come
        // from the regeling and the school's policy, not from the chronicle.
        assert_eq!(
            rijen(
                "reductie_plaats_van_vestiging",
                "executogrammen_van",
                "fictief_beleid_vestiging_obs_de_klimop#1#vestiging"
            ),
            1
        );
        assert!(
            reductie("reductie_verlof_valt_buiten_schoolvakanties")["invoer"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        // The YAML is the version whose `valid_from` is `geldig_vanaf`.
        let yaml = reductie(lid_2)["yaml"].as_str().unwrap().to_string();
        assert!(yaml.contains("REDUCTIE"));
        assert!(yaml.contains(&format!(
            "valid_from: '{}'",
            reductie(lid_2)["geldig_vanaf"].as_str().unwrap()
        )));
        // The answers the catalogue gives from a reduction, and the norms
        // it calls in the head's policy.
        assert_eq!(
            reductie("reductie_plaats_van_vestiging")["antwoorden"],
            json!([
                "deel_van_gemeente_van_vestiging",
                "gemeente_van_vestiging",
                "provincie_van_vestiging"
            ])
        );
        let beleid = "fictief_beleid_vakantieverlof_obs_de_klimop";
        let awb = "reductie_algemene_wet_bestuursrecht";
        assert_eq!(
            reductie(lid_2)["normen"],
            json!([
                { "regelwerk": awb, "laag": "REDUCTIE", "artikel": "2" },
                { "regelwerk": awb, "laag": "REDUCTIE", "artikel": "1" },
                { "regelwerk": beleid, "laag": "UITVOERINGSBELEID", "artikel": "1" }
            ])
        );
        assert!(reductie(lid_2)["tekst"][0]["tekst"]
            .as_str()
            .unwrap()
            .contains("chronolexografische reductie"));
        assert_eq!(cell.grams().len(), 6);
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
        let antwoord = |w: &Value, begrip: &str| {
            w["antwoorden"]
                .as_array()
                .unwrap()
                .iter()
                .find(|a| a["begrip"] == json!(begrip))
                .cloned()
                .unwrap_or_else(|| panic!("{begrip}: {w}"))
        };
        let waarom_b = serde_json::to_value(&b.waarom).unwrap();
        assert_eq!(
            antwoord(&waarom_b, "verlof_al_verleend_in_schooljaar")["waarde"],
            json!(true)
        );

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

        // Why: one node per term the cell answers. May falls outside the
        // school holidays; the earlier leave, read up to the objection
        // against B, is "nee".
        let beslissing = op_b["beslissing"]["id"].as_str().unwrap();
        let antwoorden = op_b["waarom"]["antwoorden"].as_array().unwrap();
        assert_eq!(antwoorden.len(), 2, "{antwoorden:?}");
        assert_eq!(
            antwoord(&op_b["waarom"], "verlof_valt_buiten_schoolvakanties")["waarde"],
            json!(true)
        );
        let antwoord = &antwoord(&op_b["waarom"], "verlof_al_verleend_in_schooljaar");
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
                && f["filled_by"]["answer"]["answers"][0]["law_id"]
                    == json!("reductie_verlof_al_verleend_in_schooljaar")));

        // How the school answered: the reduction of the Awb counts A among
        // the revoked decisions, and of the rows under 13a that are not B
        // only the refusal in its place stands, which the filter skips.
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
            !eerst["uitvoer"]["besluiten_in_stand"]
                .as_array()
                .unwrap()
                .iter()
                .any(|rij| rij["id"] == json!(a.id)),
            "{spoor}"
        );
        assert!(
            spoor.contains("FOREACH over 0 element(s), 1 skipped by filter = False"),
            "{spoor}"
        );

        // The log: one action per chronolexogram, newest first.
        let log = serde_json::to_value(logboek(&corpus, &cell).unwrap()).unwrap();
        let log = log.as_array().unwrap();
        assert_eq!(log.len(), cell.grams().len());
        let nieuw = op_b["nieuw_besluit"]["id"].as_str().unwrap();
        let laatste = &log[0];
        assert_eq!(laatste["gram"]["id"], json!(nieuw));
        // Its reason is the objection against B, which led to the decision
        // on it and to the new decision; the decision on it names the new one.
        assert_eq!(laatste["gram"]["refers_to"], json!(tegen_b.id));
        let handeling = |id: &str| log.iter().find(|h| h["gram"]["id"] == json!(id)).unwrap();
        assert_eq!(
            handeling(&tegen_b.id)["leidde_tot"],
            json!([beslissing, nieuw])
        );
        assert_eq!(handeling(beslissing)["leidde_tot"], json!([nieuw]));
        // An application is no decision, and has no chain.
        assert!(handeling(&eerste.id)["waarom"].is_null());

        // The chain: Awb 7:11 runs, and under the heroverweging it calls
        // art. 13a, where the cell answers "nee", read up to the objection.
        let keten = &laatste["waarom"]["keten"];
        assert_eq!(keten[0]["soort"], json!("artikel"));
        assert_eq!(keten[0]["artikel"], json!("7:11"));
        let heroverweging = vind(keten, &|s| {
            s["naam"] == json!("heroverweging_geeft_aanleiding")
        })
        .unwrap();
        assert_eq!(heroverweging["waarde"], json!(true));
        let art13a = vind(&heroverweging["stappen"], &|s| {
            s["soort"] == json!("artikel") && s["artikel"] == json!("13a")
        })
        .unwrap();
        assert_eq!(art13a["wet"], json!(LAW));
        let antwoord = vind(&art13a["stappen"], &|s| {
            s["naam"] == json!("verlof_al_verleend_in_schooljaar")
        })
        .unwrap();
        assert_eq!(antwoord["soort"], json!("begrip"));
        assert_eq!(antwoord["waarde"], json!(false));
        // The head's binding (art. 2 of his policy), with the school's
        // answer (RFC-048).
        let binding = &antwoord["ingevuld_door"]["answer"];
        assert_eq!(
            (&binding["law_id"], &binding["article"]),
            (
                &json!("fictief_beleid_vakantieverlof_obs_de_klimop"),
                &json!("2")
            )
        );
        let door = &binding["answers"][0];
        assert_eq!(door["answerer"], json!(SCHOOL));
        assert_eq!(door["answer"], json!("verlof_al_verleend_in_schooljaar"));
        assert_eq!(
            door["law_id"],
            json!("reductie_verlof_al_verleend_in_schooljaar")
        );
        assert_eq!(door["last_record"], json!(tegen_b.id));
        // The first time the decision asked the cell this term.
        assert_eq!(antwoord["keer"], json!(0));
        // The head's finding, and a hook with its effect.
        let beroep = vind(&art13a["stappen"], &|s| {
            s["naam"] == json!("specifieke_aard_beroep_ouder")
        })
        .unwrap();
        assert_eq!(beroep["ingevuld_door"], json!("finding"));
        let hook = vind(keten, &|s| {
            s["soort"] == json!("hook") && s["artikel"] == json!("6:7")
        })
        .unwrap();
        assert_eq!(hook["stappen"][0]["naam"], json!("termijn_weken"));
        // Hook 6:9 reads the objection's own facts, which the decision is
        // not passed: it determines nothing there and leaves no step.
        assert!(vind(keten, &|s| s["artikel"] == json!("6:9")).is_none());
        // Arithmetic and variable resolution leave no step.
        let tekst = keten.to_string();
        assert!(
            !tekst.contains("DATE_DIFF") && !tekst.contains("\"resolve\""),
            "{tekst}"
        );
    }

    #[test]
    fn leave_in_the_christmas_holiday_is_refused_without_a_judgement_of_the_head() {
        // Scenario 7: leave from 21 to 23 December 2026, in the
        // kerstvakantie of region midden. The head finds the profession of a
        // parent and nothing else; the cell answers that the leave does not
        // fall outside the school holidays, from the school's vestiging in
        // Utrecht and the regeling.
        let corpus = corpus();
        let mut cell = corpus.cell().unwrap();
        let mut kerst = aanvraag("2026-12-23");
        kerst.insert("eerste_verlofdag".into(), json!("2026-12-21"));
        let app = aanvragen(&corpus, &mut cell, &kerst, day("2026-10-14")).unwrap();
        let besluit = besluiten(
            &corpus,
            &mut cell,
            &app.id,
            &bevindingen(),
            day("2026-10-14"),
        )
        .unwrap();
        assert_eq!(besluit.besluit.outputs["verlof_verleend"], json!(false));
        assert_eq!(besluit.waarom.bevindingen, ["specifieke_aard_beroep_ouder"]);
        let id = besluit.besluit.id.as_str();

        // Why: the cell's answer "nee", read up to the application.
        let antwoord = besluit
            .waarom
            .antwoorden
            .iter()
            .find(|a| a["begrip"] == json!("verlof_valt_buiten_schoolvakanties"))
            .unwrap();
        assert_eq!(antwoord["waarde"], json!(false));
        assert_eq!(antwoord["door"], json!(SCHOOL));
        assert_eq!(
            antwoord["reductie"],
            json!("reductie_verlof_valt_buiten_schoolvakanties")
        );
        assert_eq!(antwoord["tot_en_met"], json!(app.id));

        // In the log's chain: art. 13a runs, and the term is one step,
        // filled in by the school.
        let log = serde_json::to_value(logboek(&corpus, &cell).unwrap()).unwrap();
        let keten = &log[0]["waarom"]["keten"];
        assert_eq!(keten[0]["artikel"], json!(ARTICLE));
        let stap = vind(keten, &|s| {
            s["naam"] == json!("verlof_valt_buiten_schoolvakanties")
        })
        .unwrap();
        assert_eq!(stap["soort"], json!("begrip"));
        assert_eq!(stap["waarde"], json!(false));
        // Filled by the head's binding, with the school's answer (RFC-048).
        assert_eq!(
            stap["ingevuld_door"]["answer"]["law_id"],
            json!("fictief_beleid_vakantieverlof_obs_de_klimop")
        );
        assert_eq!(
            stap["ingevuld_door"]["answer"]["answers"][0]["answerer"],
            json!(SCHOOL)
        );
        assert_eq!(stap["keer"], json!(0));
        // The school's own executogram is the oldest in the log, without a
        // chain.
        let n = log.as_array().unwrap().len();
        assert_eq!(log[n - 1]["gram"]["kind"], json!("EXECUTOGRAM"));
        assert!(log[n - 1]["waarom"].is_null());

        // How the school answered: its reduction read the kerstvakantie of
        // the regeling for region midden, and in it the cell answered where
        // the school is established.
        let hoe = hoe_beantwoord(&corpus, &cell, id, "verlof_valt_buiten_schoolvakanties").unwrap();
        let eerst = &hoe["antwoorden"][0];
        assert_eq!(
            eerst["uitvoer"]["verlof_valt_buiten_schoolvakanties"],
            json!(false)
        );
        let gelezen = |naam: &str| {
            eerst["uit_regelgeving"]
                .as_array()
                .unwrap()
                .iter()
                .find(|g| g["naam"] == json!(naam))
                .unwrap_or_else(|| panic!("{naam}: {}", eerst["uit_regelgeving"]))
                .clone()
        };
        let regio = gelezen("regio");
        assert_eq!(regio["waarde"], json!("Midden"));
        assert_eq!(
            regio["regelwerk"],
            json!("regeling_vaststelling_schoolvakanties_20252030")
        );
        assert_eq!(
            gelezen("kerstvakantie_eerste_dag")["waarde"],
            json!("2026-12-19")
        );
        let genest = eerst["antwoorden"].as_array().unwrap();
        let gemeente = genest
            .iter()
            .find(|a| a["begrip"] == json!("gemeente_van_vestiging"))
            .unwrap_or_else(|| panic!("{genest:?}"));
        assert_eq!(gemeente["waarde"], json!("Utrecht"));
        assert_eq!(gemeente["reductie"], json!("reductie_plaats_van_vestiging"));
        // And how it answered that: from the executogram of its vestiging.
        let plaats = hoe_beantwoord(&corpus, &cell, id, "gemeente_van_vestiging").unwrap();
        let k = gemeente["keer"].as_u64().unwrap() as usize;
        let vestiging = &plaats["antwoorden"][k];
        assert_eq!(
            vestiging["reductie"],
            json!("reductie_plaats_van_vestiging")
        );
        assert_eq!(
            vestiging["uitvoer"]["gemeente_van_vestiging"],
            json!("Utrecht")
        );
    }

    #[test]
    fn the_chain_counts_each_answer_and_names_a_cached_call_by_its_output() {
        use regelrecht_engine::{AnswerProvenance, Value as V};
        let anker = |artikel: &str| regelrecht_engine::trace::LegalAnchor {
            law_id: Some(LAW.into()),
            article: Some(artikel.into()),
            ..Default::default()
        };
        // The term, filled by a binding whose input took the answer of the
        // school: one node per answer, under the input (RFC-048).
        let antwoord = || {
            let mut n = PathNode::new(PathNodeType::Resolve, "begrip")
                .with_result(V::Bool(false))
                .with_anchor(anker(ARTICLE));
            n.answer = Some(AnswerProvenance {
                answerer: SCHOOL.into(),
                answer: "begrip".into(),
                law_id: "reductie".into(),
                article: "1".into(),
                valid_from: "2026-08-01".into(),
                records_read: 1,
                last_record: None,
            });
            PathNode::new(PathNodeType::OpenTermResolution, "begrip")
                .with_result(V::Bool(false))
                .with_anchor(anker(ARTICLE))
                .with_child(PathNode::new(PathNodeType::Resolve, "antwoord_begrip").with_child(n))
        };
        let actie = |naam: &str| {
            PathNode::new(PathNodeType::Action, naam)
                .with_result(V::Bool(true))
                .with_anchor(anker(ARTICLE))
        };
        let root = PathNode::new(PathNodeType::Article, LAW)
            .with_child(
                actie("a").with_child(antwoord()).with_child(
                    // A call whose result was cached: named by its output.
                    PathNode::new(PathNodeType::CrossLawReference, "andere_wet#uitvoer")
                        .with_child(PathNode::new(PathNodeType::Cached, "andere_wet#uitvoer")),
                ),
            )
            // A marking of a construct the engine cannot translate.
            .with_child(
                PathNode::new(PathNodeType::Article, "untranslatable:x").with_child(antwoord()),
            );
        let spoor = Spoor {
            ingevuld: &[],
            beantwoord: beantwoord(&root),
            eigen: SCHOOL,
        };
        let stappen = serde_json::to_value(stappen_onder(&root, &spoor).unwrap()).unwrap();
        // The second time the term is asked counts on from the first.
        assert_eq!(stappen[0]["stappen"][0]["keer"], json!(0));
        assert_eq!(stappen[1]["keer"], json!(1));
        let aanroep = &stappen[0]["stappen"][1];
        assert_eq!(aanroep["soort"], json!("artikel"));
        assert_eq!(aanroep["wet"], json!("andere_wet"));
        assert_eq!(aanroep["artikel"], json!(""));
        assert_eq!(aanroep["uitvoer"], json!("uitvoer"));
        assert_eq!(aanroep["stappen"], json!([]));
        // The marking leaves no step: what is under it follows directly.
        assert_eq!(stappen[1]["soort"], json!("begrip"));
    }

    #[test]
    fn what_a_case_needs_follows_from_the_law() {
        // Vakantieverlof: the leave article, the policy that fills its open
        // term, the Awb with its hooks, the reductions the school's
        // catalogue names (RFC-048), what those read: the head's policy with
        // its norm and its bindings, the reduction of the Awb, the regeling
        // and the school's policy on its rooster, and the policy that
        // requires the fact of its vestiging. The Wet op het primair
        // onderwijs is only the regeling's legal basis.
        let verlof = corpus();
        let a = &verlof.afleiding;
        assert_eq!((a.law.as_str(), a.article.as_str()), (LAW, ARTICLE));
        assert_eq!(
            (a.objection_law.as_str(), a.objection.as_str()),
            ("algemene_wet_bestuursrecht", "7:11")
        );
        assert_eq!(a.rol, "het hoofd");
        // The appeal, from the Awb, and the fact for receiving a judgment:
        // the one the law requires whose parameters are all outputs of the
        // judgment.
        let beroep = a.beroep.as_ref().unwrap();
        assert_eq!(
            (beroep.law.as_str(), beroep.article.as_str()),
            ("algemene_wet_bestuursrecht", "8:70")
        );
        assert_eq!(
            beroep.ontvangst,
            "algemene_wet_bestuursrecht#8:79#afschrift_van_de_uitspraak"
        );
        let mut ids = a.regelwerken.clone();
        assert_eq!(ids[0], LAW);
        ids.sort();
        assert_eq!(
            ids,
            [
                "algemene_wet_bestuursrecht",
                "fictief_beleid_schooljaar_obs_de_klimop",
                "fictief_beleid_schooljaargrenzen_obs_de_klimop",
                ROOSTER,
                "fictief_beleid_vakantieverlof_obs_de_klimop",
                "fictief_beleid_vestiging_obs_de_klimop",
                LAW,
                "reductie_algemene_wet_bestuursrecht",
                "reductie_plaats_van_vestiging",
                "reductie_verlof_al_verleend_in_schooljaar",
                "reductie_verlof_valt_buiten_schoolvakanties",
                "regeling_vaststelling_schoolvakanties_20252030",
            ]
        );

        // The parkeerkaart: art. 49 reads the Regeling, the municipality's
        // policy fills its open term and binds the terms to the answers of
        // the cells, and the Awb; the reductions of the cells' catalogues.
        // The catalogue of the municipality also names the reduction of the
        // bedrijfsparkeervergunning: a cell's answers are the cell's, not a
        // case's. That reduction reads the decisions under article 2 of the
        // ordinance (the `basis` it passes to the reduction of the Awb), so
        // the ordinance and what acts on it are in this case too.
        let kaart = casus("gehandicaptenparkeerkaart");
        let a = &kaart.afleiding;
        assert_eq!(a.objection, "7:11");
        assert_eq!(
            a.beroep.as_ref().unwrap().ontvangst,
            "algemene_wet_bestuursrecht#8:79#afschrift_van_de_uitspraak"
        );
        // The authority BABW art. 49 names as the one who decides (RFC-002).
        assert_eq!(a.rol, "het college van burgemeester en wethouders");
        let mut ids = a.regelwerken.clone();
        ids.sort();
        assert_eq!(
            ids,
            [
                "algemene_wet_bestuursrecht",
                "beleid_bedrijfsparkeervergunning_gemeente_wilgendaal",
                "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal",
                "beleid_geneeskundig_advies_ggd_wilgendaal",
                "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw",
                "parkeerverordening_gemeente_wilgendaal",
                "reductie_algemene_wet_bestuursrecht",
                "reductie_geldige_bedrijfsparkeervergunningen",
                "reductie_geldige_gehandicaptenparkeerkaart",
                "reductie_geneeskundig_advies",
                "reductie_geneeskundig_onderzoek",
                "regeling_gehandicaptenparkeerkaart",
                "uitvoeringsbesluit_parkeren_gemeente_wilgendaal",
            ]
        );

        // The bedrijfsparkeervergunning: art. 2 of the fictitious ordinance,
        // its implementing decision that fills its open terms, the policy
        // that binds those to the answers of the cells, and the Awb; the
        // reductions of the catalogues of the municipality and of the Chamber
        // of Commerce, and the policy that requires the Chamber's
        // registrations. The municipality's reduction for the parking card
        // reads the decisions under BABW art. 49, so that article and what
        // acts on it are in this case too.
        let vergunning = casus("bedrijfsparkeervergunning");
        let a = &vergunning.afleiding;
        assert_eq!(a.objection, "7:11");
        assert_eq!(a.rol, "het college van burgemeester en wethouders");
        let mut ids = a.regelwerken.clone();
        ids.sort();
        assert_eq!(
            ids,
            [
                "algemene_wet_bestuursrecht",
                "beleid_bedrijfsparkeervergunning_gemeente_wilgendaal",
                "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal",
                "beleid_handelsregister_kamer_van_koophandel",
                "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw",
                "parkeerverordening_gemeente_wilgendaal",
                "reductie_algemene_wet_bestuursrecht",
                "reductie_geldige_bedrijfsparkeervergunningen",
                "reductie_geldige_gehandicaptenparkeerkaart",
                "reductie_vestiging_in_handelsregister",
                "regeling_gehandicaptenparkeerkaart",
                "uitvoeringsbesluit_parkeren_gemeente_wilgendaal",
            ]
        );
    }

    #[test]
    fn zorgtoeslag_the_application_asks_for_the_advance_and_the_policy_its_amount() {
        // Awir art. 15 establishes the application and asks for the
        // decision of art. 16, the advance, which the Dienst Toeslagen takes
        // as the article names it. Its open term, the amount the allowance
        // is expected to come to, is filled by the Dienst's policy, which
        // reads the Wet op de zorgtoeslag; that law reads the standard
        // premium from its regeling, and the Awir, the Zvw and the Wet IB
        // 2001 are read by the articles of these laws.
        let zorg = casus("zorgtoeslag");
        let a = &zorg.afleiding;
        assert_eq!(
            (a.decision_law.as_str(), a.decision.as_str()),
            ("algemene_wet_inkomensafhankelijke_regelingen", "16")
        );
        assert_eq!(a.objection, "7:11");
        assert!(a.beroep.is_none(), "no court among the cells of this case");
        assert_eq!(a.rol, "Dienst Toeslagen");
        let mut ids = a.regelwerken.clone();
        ids.sort();
        assert_eq!(
            ids,
            [
                "algemene_wet_bestuursrecht",
                "algemene_wet_inkomensafhankelijke_regelingen",
                "algemene_wet_inzake_rijksbelastingen",
                "beleid_zorgtoeslag_dienst_toeslagen",
                "regeling_standaardpremie",
                "wet_inkomstenbelasting_2001",
                "wet_op_de_zorgtoeslag",
                "zorgverzekeringswet",
            ]
        );
    }

    #[test]
    fn zorgtoeslag_the_preview_asks_each_estimate_of_the_dienst() {
        // The deciding article asks no finding of its own: its open term is
        // filled by the policy, which leaves each estimate to the Dienst.
        // The preview asks them one at a time, as the case reaches them.
        let corpus = casus("zorgtoeslag");
        let mut cell = corpus.cell().unwrap();
        let op = day("2025-01-06");
        let vormen = vormen(&corpus, &cell, op).unwrap();
        assert_eq!(vormen["besluit"]["bevindingen"], json!([]));
        let velden = map(json!({
            "bsn": "999993653",
            "berekeningsjaar": 2025,
            "aanvraag_ondertekend": true,
            "naam_aanvrager": "S. de Vries",
            "adres_aanvrager": "Voorbeeldlaan 7",
            "dagtekening": "2025-01-03",
            "gevraagde_beschikking": "zorgtoeslag over 2025",
        }));
        let app = aanvragen(&corpus, &mut cell, &velden, op).unwrap();

        let leeg = voorvertoning_besluit(&corpus, &cell, &app.id, &Map::new(), op).unwrap();
        let eerste = namen(&leeg.stil);
        assert_eq!(eerste.len(), 1);
        assert!(eerste[0].starts_with("vermoedelijk_"), "{eerste:?}");
        assert_eq!(
            leeg.stil[0]["basis"],
            json!("beleid_zorgtoeslag_dienst_toeslagen#1")
        );
        assert!(leeg.voorlopig.is_none());

        let oordelen = map(json!({
            "vermoedelijk_toetsingsinkomen": 2500000,
            "vermoedelijk_heeft_toeslagpartner": false,
            "vermoedelijk_toetsingsinkomen_toeslagpartner": 0,
            "vermoedelijk_verzekerd": true,
            "vermoedelijk_vermogen_onder_grens": true,
        }));
        let alle = voorvertoning_besluit(&corpus, &cell, &app.id, &oordelen, op).unwrap();
        assert_eq!(alle.stil.len(), 5);
        let besluit = alle.voorlopig.unwrap().besluit;
        assert_eq!(besluit.outputs["voorschot_verleend"], json!(true));
        assert_eq!(besluit.outputs["voorschotbedrag"], json!(157700));
        assert!(cell.kronieken()[0].iter().all(|g| g.kind != "BESCHIKKING"));
    }

    #[test]
    fn the_preview_of_a_decision_is_the_decision_and_records_nothing() {
        let corpus = corpus();
        let mut cell = corpus.cell().unwrap();
        let app = aanvragen(
            &corpus,
            &mut cell,
            &aanvraag("2027-02-05"),
            day("2026-10-13"),
        )
        .unwrap();
        let vastgelegd = cell.grams().len();
        let op = day("2026-10-20");
        let voorlopig = |b: &Map<String, Value>| {
            voorvertoning_besluit(&corpus, &cell, &app.id, b, op)
                .unwrap()
                .voorlopig
                .unwrap()
        };
        let verleend = voorlopig(&beroep(true));
        assert_eq!(verleend.besluit.outputs["verlof_verleend"], json!(true));
        let voorlopig = voorlopig(&beroep(false));
        // A finding not given yet: refused, as the decision itself would be.
        let zonder = voorvertoning_besluit(&corpus, &cell, &app.id, &Map::new(), op).unwrap();
        let e = zonder.geweigerd.unwrap();
        assert!(e.contains("specifieke_aard_beroep_ouder"), "{e}");
        // The cell answers both of its terms for February 2027.
        assert!(zonder.stil.is_empty());
        assert_eq!(cell.grams().len(), vastgelegd);

        let genomen = besluiten(&corpus, &mut cell, &app.id, &beroep(false), op).unwrap();
        assert_eq!(voorlopig.besluit.outputs, genomen.besluit.outputs);
        assert_eq!(
            serde_json::to_value(&voorlopig.waarom.keten).unwrap(),
            serde_json::to_value(&genomen.waarom.keten).unwrap()
        );
        assert_eq!(cell.grams().len(), vastgelegd + 1);
    }

    #[test]
    fn the_preview_of_a_decision_on_objection_is_that_decision_and_records_nothing() {
        let corpus = corpus();
        let mut cell = corpus.cell().unwrap();
        let besluit = geweigerd(&corpus, &mut cell);
        let bezwaar = bezwaren(
            &corpus,
            &mut cell,
            &besluit.besluit.id,
            &bezwaar_velden("2026-11-01"),
            day("2026-11-01"),
        )
        .unwrap();
        let vastgelegd = cell.grams().len();
        let op = day("2026-11-15");
        let voorlopig = |b: &Map<String, Value>| {
            voorvertoning_op_bezwaar(&corpus, &cell, &bezwaar.id, b, op)
                .unwrap()
                .voorlopig
                .unwrap()
        };
        let ongegrond = voorlopig(&Map::new());
        assert_eq!(
            ongegrond["beslissing"]["outputs"]["bestreden_besluit_herroepen"],
            json!(false)
        );
        assert!(ongegrond["nieuw_besluit"].is_null());
        let voorlopig = voorlopig(&beroep(true));
        assert_eq!(cell.grams().len(), vastgelegd);

        let genomen =
            beslissen_op_bezwaar(&corpus, &mut cell, &bezwaar.id, &beroep(true), op).unwrap();
        assert_eq!(
            voorlopig["beslissing"]["outputs"]["heroverweging_geeft_aanleiding"],
            json!(true)
        );
        for pad in ["/nieuw_besluit/outputs/verlof_verleend", "/waarom/keten"] {
            assert_eq!(voorlopig.pointer(pad), genomen.pointer(pad), "{pad}");
        }
        let herroepen = "/beslissing/outputs/bestreden_besluit_herroepen";
        assert_eq!(voorlopig.pointer(herroepen), Some(&json!(true)));
        assert_eq!(voorlopig.pointer(herroepen), genomen.pointer(herroepen));
        assert_eq!(cell.grams().len(), vastgelegd + 2);
    }

    /// Leave from 20 to 22 December 2027, applied for on 14 October 2026:
    /// school year 2027–2028, which the school's holiday rooster does not
    /// reach yet.
    fn kerst_2027(corpus: &Corpus, cell: &mut Cellen) -> Gram {
        let mut velden = aanvraag("2027-12-22");
        velden.insert("eerste_verlofdag".into(), json!("2027-12-20"));
        aanvragen(corpus, cell, &velden, day("2026-10-14")).unwrap()
    }

    fn namen(velden: &[Value]) -> Vec<&str> {
        velden.iter().map(|v| v["naam"].as_str().unwrap()).collect()
    }

    #[test]
    fn a_term_the_cell_leaves_silent_is_asked_of_the_head() {
        // The bug from production: the cell answers
        // verlof_valt_buiten_schoolvakanties, so the form hid it, but for
        // 2027–2028 its reduction answers null. The head's policy then
        // leaves the case to him (art. 4, RFC-048 §2), and the preview asks
        // that judgement.
        let corpus = corpus();
        let mut cell = corpus.cell().unwrap();
        let app = kerst_2027(&corpus, &mut cell);
        let op = day("2026-10-14");
        let term = "verlof_valt_buiten_schoolvakanties_volgens_hoofd";

        let zonder = voorvertoning_besluit(&corpus, &cell, &app.id, &beroep(true), op).unwrap();
        assert_eq!(namen(&zonder.stil), [term]);
        assert_eq!(
            zonder.stil[0]["basis"],
            json!("fictief_beleid_vakantieverlof_obs_de_klimop#3")
        );
        assert!(zonder.voorlopig.is_none());
        assert!(zonder.geweigerd.unwrap().contains(term));

        // With the head's judgement the preview decides, and the term stays
        // on the form.
        let mut oordeel = beroep(true);
        oordeel.insert(term.into(), json!(false));
        let met = voorvertoning_besluit(&corpus, &cell, &app.id, &oordeel, op).unwrap();
        assert_eq!(namen(&met.stil), [term]);
        let voorlopig = met.voorlopig.unwrap();
        assert_eq!(voorlopig.besluit.outputs["verlof_verleend"], json!(false));

        // The real decision takes the judgement too, and comes out the same.
        let genomen = besluiten(&corpus, &mut cell, &app.id, &oordeel, op).unwrap();
        assert_eq!(genomen.besluit.outputs, voorlopig.besluit.outputs);
        assert!(genomen.waarom.bevindingen.contains(&term.to_string()));
    }

    /// A home care business in the city centre (the case's third
    /// registration) with two valid permits applies for a third and is
    /// refused on the maximum: the college found the hardship clause (art. 6
    /// of the parking ordinance) not to apply. The business objects. The
    /// cells as they stand then, and the objection.
    fn hardheid_bezwaar(corpus: &Corpus) -> (Cellen, String) {
        let mut cell = corpus.cell().unwrap();
        let voorbeeld = &corpus.casus.voorbeeld;
        let hardheid = "toepassing_leidt_tot_onaanvaardbare_gevolgen";
        let mut besluit = None;
        for (kenteken, dag) in [
            ("ZZ-001-Z", "2026-10-20"),
            ("ZZ-002-Z", "2026-10-22"),
            ("ZZ-003-Z", "2026-10-27"),
        ] {
            let mut velden = voorbeeld.aanvraag.clone();
            velden.insert("kvk_nummer".into(), json!("99990044"));
            velden.insert("naam_aanvrager".into(), json!("Thuiszorg De Linde"));
            velden.insert("kenteken".into(), json!(kenteken));
            let app = aanvragen(corpus, &mut cell, &velden, day(dag)).unwrap();
            let bevindingen = map(json!({ hardheid: false }));
            besluit = Some(besluiten(corpus, &mut cell, &app.id, &bevindingen, day(dag)).unwrap());
        }
        let besluit = besluit.unwrap().besluit;
        assert_eq!(
            besluit.outputs["bedrijfsparkeervergunning_verleend"],
            json!(false)
        );
        let mut velden = voorbeeld.bezwaar.clone();
        velden.extend(map(json!({
            "naam_indiener": "Thuiszorg De Linde",
            "gronden_bezwaar_of_beroep": "de wijkverpleging rijdt dagelijks met drie auto's naar cliënten",
            "dagtekening_bezwaar_of_beroepschrift": "2026-11-01",
        })));
        let bezwaar = bezwaren(corpus, &mut cell, &besluit.id, &velden, day("2026-11-01")).unwrap();
        (cell, bezwaar.id)
    }

    #[test]
    fn the_preview_on_objection_takes_a_finding_a_policy_declares() {
        // The hardship clause is an open term of art. 6, which fills a term
        // of the deciding art. 2. The preview took it for one a policy
        // leaves to the college once the case reaches it, and dropped it;
        // the decision on objection then took the earlier finding, so the
        // preview stayed unfounded whatever the college found anew.
        let corpus = casus("bedrijfsparkeervergunning");
        let op = day("2026-11-20");
        let herroepen = "/beslissing/outputs/bestreden_besluit_herroepen";
        for oordeel in [true, false] {
            let bevindingen =
                map(json!({ "toepassing_leidt_tot_onaanvaardbare_gevolgen": oordeel }));
            let (mut cell, bezwaar) = hardheid_bezwaar(&corpus);
            let voorlopig = voorvertoning_op_bezwaar(&corpus, &cell, &bezwaar, &bevindingen, op)
                .unwrap()
                .voorlopig
                .unwrap();
            let genomen =
                beslissen_op_bezwaar(&corpus, &mut cell, &bezwaar, &bevindingen, op).unwrap();
            assert_eq!(
                voorlopig.pointer(herroepen),
                Some(&json!(oordeel)),
                "{oordeel}"
            );
            for pad in [herroepen, "/nieuw_besluit/outputs", "/waarom/keten"] {
                assert_eq!(
                    voorlopig.pointer(pad),
                    genomen.pointer(pad),
                    "{oordeel}: {pad}"
                );
            }
        }
    }

    #[test]
    fn the_preview_after_a_judgment_takes_a_finding_a_policy_declares() {
        // As above, deciding on the objection again after a judgment: the
        // objection unfounded, the court annuls and orders a new decision,
        // and the college finds the hardship clause anew.
        let corpus = casus("bedrijfsparkeervergunning");
        let (mut cell, bezwaar) = hardheid_bezwaar(&corpus);
        let ongegrond =
            beslissen_op_bezwaar(&corpus, &mut cell, &bezwaar, &Map::new(), day("2026-11-20"))
                .unwrap();
        let op_bezwaar = ongegrond["beslissing"]["id"].as_str().unwrap().to_string();
        let mut velden = corpus.casus.voorbeeld.beroep.clone();
        velden.insert(
            "dagtekening_bezwaar_of_beroepschrift".into(),
            json!("2026-12-01"),
        );
        let ingesteld =
            beroep_instellen(&corpus, &mut cell, &op_bezwaar, &velden, day("2026-12-01")).unwrap();
        let mut gegrond = uitspraak_bevindingen(&corpus, &[]);
        gegrond.extend(map(json!({
            "rechtsregel_geschonden": true,
            "geschonden_rechtsregel": "artikel 3:46 van de Algemene wet bestuursrecht",
            "rechtsgevolgen_in_stand": false,
            "uitspraak_treedt_in_de_plaats": false,
            "termijn_nieuw_besluit": "2027-02-25",
        })));
        let gedaan = uitspraak_doen(
            &corpus,
            &mut cell,
            &ingesteld.id,
            &gegrond,
            day("2027-01-11"),
        )
        .unwrap();
        let ontvangst = toezenden(
            &corpus,
            &mut cell,
            gedaan["uitspraak"]["id"].as_str().unwrap(),
            day("2027-01-11"),
        )
        .unwrap();
        assert_eq!(ontvangst.fields["opdracht_nieuw_besluit"], json!(true));

        let op = day("2027-01-18");
        let bevindingen = map(json!({ "toepassing_leidt_tot_onaanvaardbare_gevolgen": true }));
        let voorlopig = voorvertoning_na_uitspraak(&corpus, &cell, &ontvangst.id, &bevindingen, op)
            .unwrap()
            .voorlopig
            .unwrap();
        let genomen =
            opnieuw_beslissen_op_bezwaar(&corpus, &mut cell, &ontvangst.id, &bevindingen, op)
                .unwrap();
        let herroepen = "/beslissing/outputs/bestreden_besluit_herroepen";
        assert_eq!(voorlopig.pointer(herroepen), Some(&json!(true)));
        for pad in [herroepen, "/nieuw_besluit/outputs", "/waarom/keten"] {
            assert_eq!(voorlopig.pointer(pad), genomen.pointer(pad), "{pad}");
        }
    }

    #[test]
    fn a_silent_term_is_asked_on_objection_too() {
        // Refused on the profession, so the decision never asked the cell. At
        // the heroverweging the head finds the profession anew; then art. 13a
        // asks the term the cell is silent on, and the policy asks the head.
        let corpus = corpus();
        let mut cell = corpus.cell().unwrap();
        let app = kerst_2027(&corpus, &mut cell);
        let besluit = besluiten(
            &corpus,
            &mut cell,
            &app.id,
            &beroep(false),
            day("2026-10-14"),
        )
        .unwrap()
        .besluit;
        let bezwaar = bezwaren(
            &corpus,
            &mut cell,
            &besluit.id,
            &bezwaar_velden("2026-11-01"),
            day("2026-11-01"),
        )
        .unwrap();
        let op = day("2026-11-15");
        let term = "verlof_valt_buiten_schoolvakanties_volgens_hoofd";
        let zonder =
            voorvertoning_op_bezwaar(&corpus, &cell, &bezwaar.id, &beroep(true), op).unwrap();
        assert_eq!(namen(&zonder.stil), [term]);
        assert!(zonder.voorlopig.is_none());

        let mut oordeel = beroep(true);
        oordeel.insert(term.into(), json!(true));
        let met = voorvertoning_op_bezwaar(&corpus, &cell, &bezwaar.id, &oordeel, op)
            .unwrap()
            .voorlopig
            .unwrap();
        let genomen = beslissen_op_bezwaar(&corpus, &mut cell, &bezwaar.id, &oordeel, op).unwrap();
        let verleend = "/nieuw_besluit/outputs/verlof_verleend";
        assert_eq!(met.pointer(verleend), Some(&json!(true)));
        assert_eq!(met.pointer(verleend), genomen.pointer(verleend));
    }

    #[test]
    fn a_silent_term_is_asked_when_deciding_again_after_a_judgment() {
        // Leave in 2027–2028, which the school's holiday rooster does not
        // reach. Refused on the profession and the objection unfounded, so
        // neither decision asked the cell. The court annuls and orders a new
        // decision; deciding again, the head finds the profession anew, and
        // then art. 13a asks the term the cell is silent on.
        let corpus = corpus();
        let mut cell = corpus.cell().unwrap();
        let app = kerst_2027(&corpus, &mut cell);
        let besluit = besluiten(
            &corpus,
            &mut cell,
            &app.id,
            &beroep(false),
            day("2026-10-14"),
        )
        .unwrap()
        .besluit;
        let bezwaar = bezwaren(
            &corpus,
            &mut cell,
            &besluit.id,
            &bezwaar_velden("2026-11-01"),
            day("2026-11-01"),
        )
        .unwrap();
        let ongegrond = beslissen_op_bezwaar(
            &corpus,
            &mut cell,
            &bezwaar.id,
            &Map::new(),
            day("2026-11-15"),
        )
        .unwrap();
        let op_bezwaar = ongegrond["beslissing"]["id"].as_str().unwrap().to_string();
        let mut velden = corpus.casus.voorbeeld.beroep.clone();
        velden.insert(
            "dagtekening_bezwaar_of_beroepschrift".into(),
            json!("2026-12-01"),
        );
        let ingesteld =
            beroep_instellen(&corpus, &mut cell, &op_bezwaar, &velden, day("2026-12-01")).unwrap();
        let gedaan = uitspraak_doen(
            &corpus,
            &mut cell,
            &ingesteld.id,
            &uitspraak_bevindingen(&corpus, &[]),
            day("2027-01-11"),
        )
        .unwrap();
        let ontvangst = toezenden(
            &corpus,
            &mut cell,
            gedaan["uitspraak"]["id"].as_str().unwrap(),
            day("2027-01-11"),
        )
        .unwrap();

        let op = day("2027-01-18");
        let term = "verlof_valt_buiten_schoolvakanties_volgens_hoofd";
        let zonder =
            voorvertoning_na_uitspraak(&corpus, &cell, &ontvangst.id, &beroep(true), op).unwrap();
        assert_eq!(namen(&zonder.stil), [term]);
        assert_eq!(
            zonder.stil[0]["basis"],
            json!("fictief_beleid_vakantieverlof_obs_de_klimop#3")
        );
        assert!(zonder.voorlopig.is_none());
        assert!(zonder.geweigerd.unwrap().contains(term));

        let mut oordeel = beroep(true);
        oordeel.insert(term.into(), json!(true));
        let met = voorvertoning_na_uitspraak(&corpus, &cell, &ontvangst.id, &oordeel, op)
            .unwrap()
            .voorlopig
            .unwrap();
        let genomen =
            opnieuw_beslissen_op_bezwaar(&corpus, &mut cell, &ontvangst.id, &oordeel, op).unwrap();
        let verleend = "/nieuw_besluit/outputs/verlof_verleend";
        assert_eq!(met.pointer(verleend), Some(&json!(true)));
        assert_eq!(met.pointer(verleend), genomen.pointer(verleend));
    }

    /// The case `gehandicaptenparkeerkaart`, with its example values.
    fn kaart() -> (Corpus, Cellen) {
        let corpus = casus("gehandicaptenparkeerkaart");
        let cell = corpus.cell().unwrap();
        (corpus, cell)
    }

    /// Apply with the case's example values on `aangevraagd` and decide on
    /// `besloten` with its example findings, `loopbeperking` changed.
    fn kaart_besluit(
        corpus: &Corpus,
        cell: &mut Cellen,
        aangevraagd: &str,
        besloten: &str,
        loopbeperking: bool,
    ) -> Besluit {
        let voorbeeld = &corpus.casus.voorbeeld;
        let app = aanvragen(corpus, cell, &voorbeeld.aanvraag, day(aangevraagd)).unwrap();
        let mut bevindingen = voorbeeld.bevindingen.clone();
        bevindingen.insert(
            "langdurige_loopbeperking_volgens_college".into(),
            json!(loopbeperking),
        );
        besluiten(corpus, cell, &app.id, &bevindingen, day(besloten)).unwrap()
    }

    fn kaart_bezwaar(corpus: &Corpus, cell: &mut Cellen, besluit: &str, op: &str) -> Gram {
        let mut velden = corpus.casus.voorbeeld.bezwaar.clone();
        velden.insert("dagtekening_bezwaar_of_beroepschrift".into(), json!(op));
        bezwaren(corpus, cell, besluit, &velden, day(op)).unwrap()
    }

    /// The fact of the advice (the municipality's fictitious policy, art.
    /// 5), and the applicant of the case whose advice finds no criterion.
    const ADVIES: &str =
        "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_uitgebracht";
    const ZONDER_CRITERIUM: &str = "999990068";

    /// An advice on `bsn` of an examination on `onderzocht`: only a
    /// long-term walking limitation, or none.
    fn advies(bsn: &str, onderzocht: &str, loopbeperking: bool) -> Map<String, Value> {
        map(json!({
            "bsn_aanvrager": bsn,
            "datum_onderzoek": onderzocht,
            "langdurige_loopbeperking": loopbeperking,
            "continu_afhankelijk_van_hulp_bestuurder": false,
            "permanent_rolstoelgebonden": false,
            "ernstige_beperkingen_andere_dan_loopbeperkingen": false,
        }))
    }

    #[test]
    fn parkeerkaart_the_advice_of_the_ggd_decides_the_criteria() {
        // Scenarios 1 and 2 of the case: the policy fills the four criteria
        // from the GGD's advice, and the college's finding gives way.
        let (corpus, mut cell) = kaart();
        // The GGD offers the advice as an act of its own.
        let vormen = vormen(&corpus, &cell, day("2026-10-13")).unwrap();
        let ggd = vormen["andere_cellen"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["cel"] == json!("ggd_wilgendaal"))
            .unwrap();
        assert!(
            ggd["handelingen"]
                .as_array()
                .unwrap()
                .iter()
                .any(|h| h["feit"] == json!(ADVIES)),
            "{ggd}"
        );

        // 1: the example applicant; the advice finds the limitation, even
        // where the college would not.
        let kaart = kaart_besluit(&corpus, &mut cell, "2026-10-13", "2026-10-20", false);
        assert_eq!(
            kaart.besluit.outputs["gehandicaptenparkeerkaart_verstrekt"],
            json!(true)
        );
        assert_eq!(
            kaart.besluit.fields["langdurige_loopbeperking"],
            json!(true)
        );
        let door = kaart
            .waarom
            .ingevuld
            .iter()
            .find(|f| f.term == "langdurige_loopbeperking")
            .map(|f| f.filled_by.clone())
            .unwrap();
        assert_eq!(
            door,
            FilledBy::Regulation {
                law_id: "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal".into(),
                article: "6".into()
            }
        );
        let antwoord = kaart
            .waarom
            .antwoorden
            .iter()
            .find(|a| a["antwoord"] == json!("advies_langdurige_loopbeperking"))
            .unwrap_or_else(|| panic!("{:#?}", kaart.waarom.antwoorden));
        assert_eq!(antwoord["reductie"], json!("reductie_geneeskundig_advies"));
        assert_eq!(antwoord["door"], json!("ggd_wilgendaal"));
        assert_eq!(antwoord["waarde"], json!(true));
        // The preview asks the college nothing on the criteria.
        let mut aanvraag = corpus.casus.voorbeeld.aanvraag.clone();
        aanvraag.insert("bsn_aanvrager".into(), json!(ZONDER_CRITERIUM));
        let app = aanvragen(&corpus, &mut cell, &aanvraag, day("2026-10-21")).unwrap();
        let voor =
            voorvertoning_besluit(&corpus, &cell, &app.id, &Map::new(), day("2026-10-21")).unwrap();
        assert!(voor.stil.is_empty(), "{:?}", voor.stil);

        // 2: the advice finds no criterion, and the card is refused.
        let geweigerd = besluiten(
            &corpus,
            &mut cell,
            &app.id,
            &corpus.casus.voorbeeld.bevindingen,
            day("2026-10-21"),
        )
        .unwrap();
        assert_eq!(
            geweigerd.besluit.outputs["gehandicaptenparkeerkaart_verstrekt"],
            json!(false)
        );
    }

    #[test]
    fn parkeerkaart_synthesis_answers_of_two_cells_make_one_decision() {
        // Synthesis (RFC-048): the decision takes the answers of different
        // cells to different questions and makes one outcome of them.
        let (corpus, mut cell) = kaart();
        // The GGD and the court (RFC-053).
        assert_eq!(cell.andere.len(), 2);
        let voorbeeld = &corpus.casus.voorbeeld;

        // The example applicant: the GGD answers that it examined him and
        // what its advice says, the municipality's own cell that he holds no
        // valid card; the card follows.
        let kaart = kaart_besluit(&corpus, &mut cell, "2026-10-13", "2026-10-20", true);
        assert_eq!(
            kaart.besluit.outputs["gehandicaptenparkeerkaart_verstrekt"],
            json!(true)
        );
        let antwoord = |begrip: &str| {
            kaart
                .waarom
                .antwoorden
                .iter()
                .find(|a| a["begrip"] == json!(begrip))
                .cloned()
                .unwrap_or_else(|| panic!("{begrip}: {:#?}", kaart.waarom.antwoorden))
        };
        let ggd = antwoord("geneeskundig_onderzoek_heeft_plaatsgehad");
        assert_eq!(ggd["door"], json!("ggd_wilgendaal"));
        // Another cell's answer has no explanation in this cell.
        assert_eq!(ggd["keer"], Value::Null);
        assert_eq!(
            antwoord("aanvrager_heeft_geldige_gehandicaptenparkeerkaart")["door"],
            json!("gemeente_wilgendaal")
        );

        // Five years on the card has expired; the college leaves the
        // examination out under lid 2 onder a, since its own cell answers
        // that it issued a card before.
        let mut later = voorbeeld.aanvraag.clone();
        later.insert("dagtekening".into(), json!("2031-10-20"));
        let app = aanvragen(&corpus, &mut cell, &later, day("2031-10-20")).unwrap();
        let mut bevindingen = voorbeeld.bevindingen.clone();
        bevindingen.insert(
            "geneeskundig_onderzoek_achterwege_gelaten".into(),
            json!(true),
        );
        bevindingen.insert(
            "bekend_dat_aanvrager_nog_steeds_voldoet".into(),
            json!(true),
        );
        let tweede =
            besluiten(&corpus, &mut cell, &app.id, &bevindingen, day("2031-10-20")).unwrap();
        assert_eq!(
            tweede.besluit.outputs["gehandicaptenparkeerkaart_verstrekt"],
            json!(true)
        );
        let FilledBy::Answer { answers, .. } = tweede
            .waarom
            .ingevuld
            .iter()
            .find(|f| f.term == "aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt")
            .map(|f| f.filled_by.clone())
            .unwrap()
        else {
            panic!("one answer")
        };
        let [by] = answers.as_slice() else {
            panic!("one answer: {answers:?}")
        };
        assert_eq!(by.answerer, "gemeente_wilgendaal");
    }

    #[test]
    fn parkeerkaart_a_second_card_next_to_a_valid_one_the_cell_answers() {
        // Scenario 3 of the case: a card is granted, and a second application
        // while it is valid is refused, because the municipality's policy
        // gives no second card. Whether there is a valid one, the cell
        // answers from its chronicle.
        let (corpus, mut cell) = kaart();
        let eerste = kaart_besluit(&corpus, &mut cell, "2026-10-13", "2026-10-20", true).besluit;
        assert_eq!(eerste.outputs[corpus.casus.uitkomst.as_str()], json!(true));
        assert_eq!(eerste.outputs["kaart_geldig_tot"], json!("2031-10-19"));

        // The college is asked the three of art. 2 lid 2 of the Regeling,
        // not the valid card, the earlier card or the examination. Nor the
        // four criteria: the policy fills them from the GGD's advice, and
        // asks the college only where there is none (the preview).
        let vormen = vormen(&corpus, &cell, day("2027-03-01")).unwrap();
        let gevraagd: Vec<&str> = vormen["besluit"]["bevindingen"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| b["naam"].as_str().unwrap())
            .collect();
        assert_eq!(gevraagd.len(), 3, "{gevraagd:?}");
        assert!(gevraagd.contains(&"geneeskundig_onderzoek_achterwege_gelaten"));

        let tweede = kaart_besluit(&corpus, &mut cell, "2027-03-01", "2027-03-01", true);
        assert_eq!(
            tweede.besluit.outputs["gehandicaptenparkeerkaart_verstrekt"],
            json!(false)
        );
        let begrip = "aanvrager_heeft_geldige_gehandicaptenparkeerkaart";
        let antwoord = tweede
            .waarom
            .antwoorden
            .iter()
            .find(|a| a["begrip"] == json!(begrip))
            .unwrap();
        assert_eq!(antwoord["begrip"], json!(begrip));
        assert_eq!(antwoord["waarde"], json!(true));
        assert_eq!(antwoord["door"], json!("gemeente_wilgendaal"));

        // How the cell answered: its reduction's trace counts the first card.
        let hoe = hoe_beantwoord(&corpus, &cell, &tweede.besluit.id, begrip).unwrap();
        let spoor = hoe["antwoorden"][0]["spoor"].as_str().unwrap();
        assert!(spoor.contains(&eerste.id), "{spoor}");

        // The log, newest first: art. 49 runs, the policy fills its term,
        // and in it the cell answers. The GGD's examinations and advice from
        // the case file are in the GGD's own chronicle; the log shows them
        // too, as the facts they are: four steps of the municipality and
        // four executograms of the GGD.
        let log = serde_json::to_value(logboek(&corpus, &cell).unwrap()).unwrap();
        let stappen = log.as_array().unwrap();
        assert_eq!(stappen.len(), 8);
        let ggd = stappen
            .iter()
            .filter(|h| h["gram"]["recording_actor"] == json!("ggd_wilgendaal"))
            .count();
        assert_eq!(ggd, 4);
        assert_eq!(log[0]["gram"]["id"], json!(tweede.besluit.id));
        let keten = &log[0]["waarom"]["keten"];
        assert_eq!(keten[0]["artikel"], json!("49"));
        let cel = vind(keten, &|s| s["naam"] == json!(begrip)).unwrap();
        assert_eq!(cel["soort"], json!("begrip"));
        assert_eq!(
            cel["ingevuld_door"]["answer"]["answers"][0]["law_id"],
            json!("reductie_geldige_gehandicaptenparkeerkaart")
        );
    }

    #[test]
    fn parkeerkaart_without_a_report_no_card_and_lid_2_without_a_new_one() {
        // Scenario 6 of the case: an applicant of whom the municipality has
        // no report from the GGD gets no decision; the chronicle stays as it
        // was (Regeling art. 2 lid 1).
        let (corpus, mut cell) = kaart();
        let mut aanvraag = corpus.casus.voorbeeld.aanvraag.clone();
        aanvraag.insert("bsn_aanvrager".into(), json!("999990032"));
        let app = aanvragen(&corpus, &mut cell, &aanvraag, day("2026-10-13")).unwrap();
        let voor = cell.grams().len();
        let e = besluiten(
            &corpus,
            &mut cell,
            &app.id,
            &corpus.casus.voorbeeld.bevindingen,
            day("2026-10-20"),
        )
        .unwrap_err();
        assert!(
            e.to_string()
                .contains("geneeskundig_onderzoek_heeft_plaatsgehad"),
            "{e}"
        );
        assert_eq!(cell.grams().len(), voor);
        // The form asks the college the four criteria, since the GGD gave
        // no advice and the policy then leaves them to the college, and
        // about the examination, since the GGD, which the law makes
        // competent, is silent for this application too, and the policy
        // leaves that to the college as well.
        let voor = voorvertoning_besluit(
            &corpus,
            &cell,
            &app.id,
            &corpus.casus.voorbeeld.bevindingen,
            day("2026-10-20"),
        )
        .unwrap();
        let stil: Vec<(&Value, &Value)> = voor
            .stil
            .iter()
            .map(|v| (&v["naam"], &v["beleid"]))
            .collect();
        // Art. 49 passes all four criteria to art. 1 of the Regeling, so
        // all four are asked, one by one, before the examination.
        let beleid = json!(true);
        assert_eq!(
            stil,
            [
                (
                    &json!("continu_afhankelijk_van_hulp_bestuurder_volgens_college"),
                    &beleid
                ),
                (
                    &json!("ernstige_beperkingen_andere_dan_loopbeperkingen_volgens_college"),
                    &beleid
                ),
                (&json!("langdurige_loopbeperking_volgens_college"), &beleid),
                (
                    &json!("permanent_rolstoelgebonden_volgens_college"),
                    &beleid
                ),
                (
                    &json!("geneeskundig_onderzoek_heeft_plaatsgehad_volgens_college"),
                    &beleid
                )
            ]
        );

        // Scenario 7: after the card of 2026 expired, the college leaves the
        // examination out for an applicant it knows still qualifies (lid 2
        // onder a); the cell answers that a card was issued before.
        let (corpus, mut cell) = kaart();
        kaart_besluit(&corpus, &mut cell, "2026-10-13", "2026-10-20", true);
        let mut aanvraag = corpus.casus.voorbeeld.aanvraag.clone();
        aanvraag.insert("dagtekening".into(), json!("2031-10-20"));
        let app = aanvragen(&corpus, &mut cell, &aanvraag, day("2031-10-20")).unwrap();
        let mut bevindingen = corpus.casus.voorbeeld.bevindingen.clone();
        bevindingen.insert(
            "geneeskundig_onderzoek_achterwege_gelaten".into(),
            json!(true),
        );
        bevindingen.insert(
            "bekend_dat_aanvrager_nog_steeds_voldoet".into(),
            json!(true),
        );
        let besluit =
            besluiten(&corpus, &mut cell, &app.id, &bevindingen, day("2031-10-20")).unwrap();
        assert_eq!(
            besluit.besluit.outputs["gehandicaptenparkeerkaart_verstrekt"],
            json!(true)
        );
        let begrippen: Vec<&Value> = besluit
            .waarom
            .antwoorden
            .iter()
            .map(|a| &a["begrip"])
            .collect();
        assert!(
            begrippen.contains(&&json!(
                "aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt"
            )),
            "{begrippen:?}"
        );
        assert!(
            !begrippen.contains(&&json!("geneeskundig_onderzoek_heeft_plaatsgehad")),
            "{begrippen:?}"
        );
    }

    #[test]
    fn parkeerkaart_objections_ongegrond_keeps_the_validity_gegrond_grants() {
        // Scenario 4: an unfounded objection against a grant leaves it
        // standing with its own validity.
        let (corpus, mut cell) = kaart();
        let verstrekt = kaart_besluit(&corpus, &mut cell, "2026-10-13", "2026-10-20", true).besluit;
        let bezwaar = kaart_bezwaar(&corpus, &mut cell, &verstrekt.id, "2026-11-01");
        let toets = heroverweging(&corpus, &cell, &bezwaar.id, day("2026-11-20")).unwrap();
        assert_eq!(toets["ontvankelijk"], json!(true));
        // The three of art. 2 lid 2 of the Regeling; the four criteria came
        // from the advice, so they were no findings of the college.
        assert_eq!(toets["bevindingen"].as_array().unwrap().len(), 3);
        let beslist = beslissen_op_bezwaar(
            &corpus,
            &mut cell,
            &bezwaar.id,
            &Map::new(),
            day("2026-11-20"),
        )
        .unwrap();
        assert_eq!(
            beslist["beslissing"]["outputs"]["bestreden_besluit_herroepen"],
            json!(false)
        );
        assert!(beslist["nieuw_besluit"].is_null());
        let bewaard = cell.grams().iter().find(|g| g.id == verstrekt.id).unwrap();
        assert_eq!(bewaard.outputs["kaart_geldig_tot"], json!("2031-10-19"));

        // Scenario 5: the advice of the GGD finds no criterion, and the card
        // is refused. On objection the GGD examines again and finds the
        // walking limitation; at the heroverweging the new advice counts,
        // the refusal is revoked and a card valid from the new decision
        // takes its place.
        let (corpus, mut cell) = kaart();
        let mut aanvraag = corpus.casus.voorbeeld.aanvraag.clone();
        aanvraag.insert("bsn_aanvrager".into(), json!(ZONDER_CRITERIUM));
        let app = aanvragen(&corpus, &mut cell, &aanvraag, day("2026-10-13")).unwrap();
        let geweigerd = besluiten(
            &corpus,
            &mut cell,
            &app.id,
            &corpus.casus.voorbeeld.bevindingen,
            day("2026-10-20"),
        )
        .unwrap()
        .besluit;
        assert_eq!(
            geweigerd.outputs["gehandicaptenparkeerkaart_verstrekt"],
            json!(false)
        );
        let bezwaar = kaart_bezwaar(&corpus, &mut cell, &geweigerd.id, "2026-11-01");
        vastleggen(
            &corpus,
            &mut cell,
            "ggd_wilgendaal",
            ADVIES,
            &advies(ZONDER_CRITERIUM, "2026-11-10", true),
            day("2026-11-10"),
        )
        .unwrap();
        let beslist = beslissen_op_bezwaar(
            &corpus,
            &mut cell,
            &bezwaar.id,
            &Map::new(),
            day("2026-11-20"),
        )
        .unwrap();
        assert_eq!(
            beslist["beslissing"]["outputs"]["bestreden_besluit_herroepen"],
            json!(true)
        );
        let nieuw = &beslist["nieuw_besluit"]["outputs"];
        assert_eq!(nieuw["gehandicaptenparkeerkaart_verstrekt"], json!(true));
        assert_eq!(nieuw["kaart_geldig_tot"], json!("2031-11-19"));
    }

    #[test]
    fn an_input_read_from_a_cached_call_has_its_value() {
        use regelrecht_engine::Value as V;
        let aanroep = "regeling#regio_van_school";
        let resolutie = |naam: &str, kind: PathNode| {
            PathNode::new(PathNodeType::Resolve, naam).with_child(
                PathNode::new(PathNodeType::CrossLawReference, aanroep).with_child(kind),
            )
        };
        let root = PathNode::new(PathNodeType::Article, "reductie")
            .with_child(resolutie(
                "regio",
                PathNode::new(PathNodeType::Action, "regio_van_school")
                    .with_result(V::String("Midden".into())),
            ))
            .with_child(resolutie(
                "regio_weer",
                PathNode::new(PathNodeType::Cached, aanroep)
                    .with_result(V::String("Midden".into())),
            ));
        for naam in ["regio", "regio_weer"] {
            let waarde = invoerwaarde(&root, naam, aanroep, "regio_van_school");
            assert_eq!(waarde, Some(&V::String("Midden".into())), "{naam}");
        }
    }

    /// The court's findings from the case's example values, with `zonder`
    /// left out.
    fn uitspraak_bevindingen(corpus: &Corpus, zonder: &[&str]) -> Map<String, Value> {
        let mut b = corpus.casus.voorbeeld.uitspraak.clone();
        for naam in zonder {
            b.remove(*naam);
        }
        b
    }

    #[test]
    fn vakantieverlof_objection_unfounded_appeal_well_founded_decided_again() {
        // The scenario of the case file: refused on 14 October, objection
        // unfounded on 15 November, appeal received on 1 December, judgment
        // on 11 January 2027 (Awb 7:12 lid 1 breached, annulled, a new
        // decision by 25 January), received by the school, and on 18
        // January the head decides on the objection again: well founded.
        let corpus = corpus();
        let mut cell = corpus.cell().unwrap();
        let voorbeeld = &corpus.casus.voorbeeld;
        let app = aanvragen(&corpus, &mut cell, &voorbeeld.aanvraag, day("2026-10-14")).unwrap();
        let geweigerd = besluiten(
            &corpus,
            &mut cell,
            &app.id,
            &beroep(false),
            day("2026-10-14"),
        )
        .unwrap()
        .besluit;
        let bezwaar = kaart_bezwaar(&corpus, &mut cell, &geweigerd.id, "2026-11-01");
        let ongegrond = beslissen_op_bezwaar(
            &corpus,
            &mut cell,
            &bezwaar.id,
            &Map::new(),
            day("2026-11-15"),
        )
        .unwrap();
        let op_bezwaar = ongegrond["beslissing"]["id"].as_str().unwrap().to_string();
        assert_eq!(
            ongegrond["beslissing"]["outputs"]["bestreden_besluit_herroepen"],
            json!(false)
        );

        // The forms: the beroepschrift without the contested decision, and
        // the court's findings on 8:70 and 8:72.
        let vormen = vormen(&corpus, &cell, day("2026-12-01")).unwrap();
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
        assert_eq!(
            vormen["beroep"]["basis"],
            json!("algemene_wet_bestuursrecht#8:70")
        );
        assert!(!namen("/beroep/velden").contains(&"bestreden_besluit".to_string()));
        assert_eq!(
            namen("/uitspraak/bevindingen"),
            [
                "indiener_is_belanghebbende",
                "rechtsregel_geschonden",
                "geschonden_rechtsregel",
                "rechtsgevolgen_in_stand",
                "uitspraak_treedt_in_de_plaats",
                "termijn_nieuw_besluit",
                "specifieke_aard_beroep_ouder",
                "verlof_valt_buiten_schoolvakanties_volgens_hoofd",
                "verlof_al_verleend_in_schooljaar_volgens_hoofd",
            ]
        );

        let mut velden = voorbeeld.beroep.clone();
        velden.insert(
            "dagtekening_bezwaar_of_beroepschrift".into(),
            json!("2026-12-01"),
        );
        let ingesteld =
            beroep_instellen(&corpus, &mut cell, &op_bezwaar, &velden, day("2026-12-01")).unwrap();
        assert_eq!(ingesteld.kind, "BEROEP");
        assert_eq!(ingesteld.received.as_ref().unwrap().id, op_bezwaar);
        assert_eq!(
            wat_volgt(&corpus, &cell, &ingesteld.id).unwrap()
                ["bezwaar_of_beroepschrift_tijdig_ingediend"],
            json!(true)
        );

        // Had the court decided the case itself (8:72 lid 3 onder b), the
        // form's findings on 13a would have been asked: the court grants
        // the leave in place of the school.
        let mut zelf = uitspraak_bevindingen(&corpus, &["termijn_nieuw_besluit"]);
        zelf.insert("uitspraak_treedt_in_de_plaats".into(), json!(true));
        zelf.insert("specifieke_aard_beroep_ouder".into(), json!(true));
        zelf.insert(
            "verlof_valt_buiten_schoolvakanties_volgens_hoofd".into(),
            json!(true),
        );
        zelf.insert(
            "verlof_al_verleend_in_schooljaar_volgens_hoofd".into(),
            json!(false),
        );
        let in_de_plaats = uitspraak_doen(
            &corpus,
            &mut cell.sandbox(),
            &ingesteld.id,
            &zelf,
            day("2027-01-11"),
        )
        .unwrap();
        assert_eq!(in_de_plaats["niet_gevraagd"], json!([]));
        assert_eq!(
            in_de_plaats["uitspraak"]["outputs"]["opdracht_nieuw_besluit"],
            json!(false)
        );
        assert_eq!(
            in_de_plaats["uitspraak"]["outputs"]["uitspraak_in_de_plaats"]["outputs"]
                ["nieuw_besluit"]["outputs"]["verlof_verleend"],
            json!(true)
        );

        // Filled in, the findings on 13a are not asked when the court does
        // not decide the case itself.
        let mut niet_zelf = uitspraak_bevindingen(&corpus, &[]);
        niet_zelf.insert("specifieke_aard_beroep_ouder".into(), json!(true));
        let niet_in_de_plaats = uitspraak_doen(
            &corpus,
            &mut cell.sandbox(),
            &ingesteld.id,
            &niet_zelf,
            day("2027-01-11"),
        )
        .unwrap();
        assert_eq!(
            niet_in_de_plaats["niet_gevraagd"],
            json!(["specifieke_aard_beroep_ouder"])
        );

        let gedaan = uitspraak_doen(
            &corpus,
            &mut cell,
            &ingesteld.id,
            &uitspraak_bevindingen(&corpus, &[]),
            day("2027-01-11"),
        )
        .unwrap();
        let uitspraak = &gedaan["uitspraak"];
        assert_eq!(
            uitspraak["outputs"]["uitspraak_strekt_tot"],
            json!("gegrondverklaring van het beroep")
        );
        assert_eq!(
            uitspraak["outputs"]["termijn_nieuw_besluit_einddatum"],
            json!("2027-01-25")
        );
        assert_eq!(gedaan["niet_gevraagd"], json!([]));
        assert!(gedaan["waarom"]["bevindingen"]
            .as_array()
            .unwrap()
            .contains(&json!("rechtsgevolgen_in_stand")));

        let ontvangst = toezenden(
            &corpus,
            &mut cell,
            uitspraak["id"].as_str().unwrap(),
            day("2027-01-11"),
        )
        .unwrap();
        assert_eq!(
            ontvangst.fact.as_deref(),
            Some("afschrift_van_de_uitspraak")
        );
        assert_eq!(ontvangst.refers_to.as_deref(), Some(op_bezwaar.as_str()));
        // A receipt is no decision: the log has no chain for it.
        let log = serde_json::to_value(logboek(&corpus, &cell).unwrap()).unwrap();
        assert_eq!(log[0]["gram"]["id"], json!(ontvangst.id));
        assert!(log[0]["waarom"].is_null());

        let toets = heroverweging_na_uitspraak(&corpus, &cell, &ontvangst.id).unwrap();
        assert_eq!(toets["bezwaar"], json!(bezwaar.id));
        assert_eq!(toets["bestreden_besluit"], json!(geweigerd.id));
        assert_eq!(
            toets["uitspraak"]["geschonden_geoordeelde_rechtsregel"],
            json!("artikel 7:12, eerste lid, van de Algemene wet bestuursrecht")
        );
        let gevraagd: Vec<_> = toets["bevindingen"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| (b["naam"].as_str().unwrap(), b["eerder"].clone()))
            .collect();
        assert_eq!(gevraagd, [("specifieke_aard_beroep_ouder", json!(false))]);

        let opnieuw = opnieuw_beslissen_op_bezwaar(
            &corpus,
            &mut cell,
            &ontvangst.id,
            &beroep(true),
            day("2027-01-18"),
        )
        .unwrap();
        assert_eq!(opnieuw["beslissing"]["after"], json!(ontvangst.id));
        assert_eq!(opnieuw["beslissing"]["refers_to"], json!(bezwaar.id));
        assert_eq!(
            opnieuw["beslissing"]["outputs"]["bestreden_besluit_herroepen"],
            json!(true)
        );
        let nieuw = &opnieuw["nieuw_besluit"];
        assert_eq!(nieuw["outputs"]["verlof_verleend"], json!(true));
        assert_eq!(nieuw["fields"]["eerste_verlofdag"], json!("2027-02-01"));
        // The school: its vestiging, the application, the refusal, the
        // objection, the decision on it, the receipt, and the two decisions
        // taken again. The court: the appeal and the judgment.
        assert_eq!(cell.grams().len(), 8);
        assert_eq!(rechtbank(&cell).grams().len(), 2);
        // The school's reduction for lid 2 reads, through the head's
        // policy, the judgment the school received.
        let stand = reducties(&corpus, &cell, day("2027-01-18")).unwrap();
        let lid_2 = stand["reducties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == json!("reductie_verlof_al_verleend_in_schooljaar"))
            .unwrap();
        let uitspraken = lid_2["invoer"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| {
                i["executogrammen_van"]
                    == json!("algemene_wet_bestuursrecht#8:79#afschrift_van_de_uitspraak")
            })
            .unwrap();
        assert_eq!(uitspraken["rijen"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn parkeerkaart_an_unfounded_appeal_is_received_by_the_municipality() {
        // The scenario "Beroep ongegrond": the court asks nothing of 8:72,
        // and a finding given for it anyway is named as not asked.
        let (corpus, mut cell) = kaart();
        let geweigerd =
            kaart_besluit(&corpus, &mut cell, "2026-10-13", "2026-10-20", false).besluit;
        let bezwaar = kaart_bezwaar(&corpus, &mut cell, &geweigerd.id, "2026-11-01");
        let ongegrond = beslissen_op_bezwaar(
            &corpus,
            &mut cell,
            &bezwaar.id,
            &Map::new(),
            day("2026-11-20"),
        )
        .unwrap();
        let op_bezwaar = ongegrond["beslissing"]["id"].as_str().unwrap();
        let mut velden = corpus.casus.voorbeeld.beroep.clone();
        velden.insert(
            "dagtekening_bezwaar_of_beroepschrift".into(),
            json!("2026-12-01"),
        );
        let ingesteld =
            beroep_instellen(&corpus, &mut cell, op_bezwaar, &velden, day("2026-12-01")).unwrap();
        let mut bevindingen = uitspraak_bevindingen(&corpus, &[]);
        bevindingen.insert("rechtsgevolgen_in_stand".into(), json!(false));
        let gedaan = uitspraak_doen(
            &corpus,
            &mut cell,
            &ingesteld.id,
            &bevindingen,
            day("2027-01-11"),
        )
        .unwrap();
        assert_eq!(
            gedaan["uitspraak"]["outputs"]["uitspraak_strekt_tot"],
            json!("ongegrondverklaring van het beroep")
        );
        assert_eq!(gedaan["niet_gevraagd"], json!(["rechtsgevolgen_in_stand"]));
        let ontvangst = toezenden(
            &corpus,
            &mut cell,
            gedaan["uitspraak"]["id"].as_str().unwrap(),
            day("2027-01-12"),
        )
        .unwrap();
        assert_eq!(ontvangst.recording_actor, "gemeente_wilgendaal");
        assert_eq!(
            ontvangst.fields["bestreden_besluit_vernietigd"],
            json!(false)
        );
        assert_eq!(ontvangst.fields["opdracht_nieuw_besluit"], json!(false));
        // Toezending once more records a second receipt; the cell does not
        // refuse that, but a receipt before the judgment it does.
        let e = toezenden(
            &corpus,
            &mut cell,
            gedaan["uitspraak"]["id"].as_str().unwrap(),
            day("2027-01-10"),
        )
        .unwrap_err();
        assert!(matches!(e, Error::Refused(_)), "{e}");
    }

    #[test]
    fn a_well_founded_appeal_without_the_findings_of_8_72_is_an_error() {
        let corpus = corpus();
        let mut cell = corpus.cell().unwrap();
        let geweigerd = geweigerd(&corpus, &mut cell);
        let bezwaar = bezwaren(
            &corpus,
            &mut cell,
            &geweigerd.besluit.id,
            &bezwaar_velden("2026-11-01"),
            day("2026-11-01"),
        )
        .unwrap();
        let ongegrond = beslissen_op_bezwaar(
            &corpus,
            &mut cell,
            &bezwaar.id,
            &Map::new(),
            day("2026-11-15"),
        )
        .unwrap();
        let mut velden = corpus.casus.voorbeeld.beroep.clone();
        velden.insert(
            "dagtekening_bezwaar_of_beroepschrift".into(),
            json!("2026-12-01"),
        );
        let ingesteld = beroep_instellen(
            &corpus,
            &mut cell,
            ongegrond["beslissing"]["id"].as_str().unwrap(),
            &velden,
            day("2026-12-01"),
        )
        .unwrap();
        let zonder = uitspraak_bevindingen(
            &corpus,
            &["rechtsgevolgen_in_stand", "termijn_nieuw_besluit"],
        );
        let e = uitspraak_doen(
            &corpus,
            &mut cell,
            &ingesteld.id,
            &zonder,
            day("2027-01-11"),
        )
        .unwrap_err();
        assert!(e.to_string().contains("rechtsgevolgen_in_stand"), "{e}");
        assert_eq!(rechtbank(&cell).grams().len(), 1);
        // A case without a receipt cannot be decided on again from one.
        let e = heroverweging_na_uitspraak(&corpus, &cell, &geweigerd.besluit.id).unwrap_err();
        assert!(e.to_string().contains("no receipt"), "{e}");
    }

    /// The first step in `stappen`, or under one, for which `past` holds.
    fn vind<'a>(stappen: &'a Value, past: &dyn Fn(&Value) -> bool) -> Option<&'a Value> {
        stappen.as_array()?.iter().find_map(|s| {
            if past(s) {
                Some(s)
            } else {
                vind(&s["stappen"], past)
            }
        })
    }
}
