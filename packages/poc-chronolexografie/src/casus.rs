//! The cases the app plays (casussen), and what follows for each from the
//! law.
//!
//! A case names two things: the cell that plays it and the article that
//! establishes the application. Everything else is derived from the law
//! ([`Afleiding`]): which regelwerken the cell runs, which article
//! establishes the objection, and what the deciding role is called. What the
//! law does not say is in the case file with the reason (`uitkomst`), next to
//! the text a visitor reads: a description, example values for the forms and
//! the scenarios.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use regelrecht_cel::{identity, Cell, Error, Files};
use regelrecht_engine::LawLoad;
use regelrecht_law_model::{
    Article, ArticleBasedLaw, AuthorityType, CompetentAuthority, HookFilter, NamedAuthority,
    ProcedureDefinition, RegulatoryLayer,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::kern::Result;

/// One case, as its file under `casussen/` writes it. The file name is its
/// id.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Casus {
    /// From the file name, never from the file.
    #[serde(skip_deserializing)]
    pub id: String,
    /// The source the case file came from (`pocs/registry.yaml`, `sources`
    /// of chronolexografie), or none for a case of this crate. From where the
    /// file is, never from the file.
    #[serde(skip_deserializing)]
    pub bron: Option<String>,
    /// The cell that plays the case: its directory under `corpus/cells`, and
    /// the actor it records as.
    pub cel: String,
    /// The article that establishes the application, `<law>#<article>`.
    pub artikel: String,
    /// The output of the decision that says whether it grants what was
    /// asked. The law does not mark it: `decision_type: TOEKENNING` says what
    /// kind of decision it is, not which of its outputs is the grant, and
    /// both decisions have more than one boolean output.
    pub uitkomst: String,
    pub titel: String,
    pub omschrijving: String,
    /// The cell's name as the visitor reads it.
    pub cel_naam: String,
    /// The first day of every scenario.
    pub speeldatum: NaiveDate,
    pub voorbeeld: Voorbeeld,
    /// What the cell knows before the first scenario, recorded at every
    /// start and reset as the cell records anything ([`Begin`]).
    #[serde(default)]
    pub begin: Option<Begin>,
    /// The other cells of the sphere (RFC-052): those the engine may ask
    /// for a value another actor establishes, where the law makes them
    /// competent. Each records what it knows before the first scenario.
    /// The court is one of them, for a case that plays the appeal
    /// ([`Beroep::rechter`]); without one the case ends at the objection.
    #[serde(default)]
    pub andere_cellen: Vec<AndereCel>,
    pub scenarios: Vec<Scenario>,
}

/// Another cell of the sphere: its directory under `corpus/cells`, its name
/// as the visitor reads it, and what it records before the first scenario.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AndereCel {
    pub cel: String,
    pub cel_naam: String,
    #[serde(default)]
    pub begin: Option<Begin>,
}

/// What the cell records before the first scenario, on `datum`:
/// executograms (RFC-051), such as where a school is established. The law
/// asks for these, but no step of the process records them, so the case
/// file gives them. The cell checks them as any other
/// step.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Begin {
    pub datum: NaiveDate,
    #[serde(default)]
    pub executogrammen: Vec<BeginExecutogram>,
    /// Applications under the case's article, each decided on its day,
    /// after the executograms: a card the cell issued before (RFC-052).
    #[serde(default)]
    pub aanvragen: Vec<BeginAanvraag>,
}

/// An application with its values `velden`, received and decided on
/// `datum` with the deciding authority's `bevindingen`. The sphere is the
/// case's own cell and the other cells begun before this one: they answer
/// what the policy binds, as in any decision, and a finding is only the
/// cell's own.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BeginAanvraag {
    pub datum: NaiveDate,
    pub velden: Map<String, Value>,
    #[serde(default)]
    pub bevindingen: Map<String, Value>,
}

/// An executogram of the fact `feit` (`<regulation>#<article>#<fact>`, a
/// fact the article requires), with the values `velden`, recorded on
/// `datum`, or else on the day of its [`Begin`].
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BeginExecutogram {
    pub feit: String,
    pub velden: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub datum: Option<NaiveDate>,
}

/// Fictional values the forms start with, so a visitor clicks through
/// quickly. Which fields there are, the law says; these only fill them.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Voorbeeld {
    #[serde(default)]
    pub aanvraag: Map<String, Value>,
    #[serde(default)]
    pub bevindingen: Map<String, Value>,
    #[serde(default)]
    pub bezwaar: Map<String, Value>,
    /// The beroepschrift (Awb 6:5).
    #[serde(default)]
    pub beroep: Map<String, Value>,
    /// The court's findings the form starts with.
    #[serde(default)]
    pub uitspraak: Map<String, Value>,
}

/// A scenario as instruction text: the steps in order, and what the visitor
/// sees at the end.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    pub naam: String,
    pub stappen: Vec<String>,
    pub eind: String,
}

/// The cases in `dir`, one per `<id>.yaml`, in the order of their ids, read
/// through `files`.
pub fn casussen(files: &dyn Files, dir: &Path) -> Result<Vec<Casus>> {
    let mut paths: Vec<PathBuf> = files
        .entries(dir)?
        .into_iter()
        .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
        .collect();
    paths.sort();
    let mut uit = Vec::new();
    for path in paths {
        let mut casus: Casus = serde_yaml_ng::from_str(&files.read(&path)?)
            .map_err(|e| Error::Setup(format!("{}: {e}", path.display())))?;
        casus.id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        uit.push(casus);
    }
    if uit.is_empty() {
        return Err(Error::Setup(format!("no case in {}", dir.display())));
    }
    Ok(uit)
}

/// One regelwerk in the corpus: where it is and its versions, each as its
/// YAML and parsed.
#[derive(Debug, Clone)]
pub struct Regelwerk {
    /// Its directory, relative to its root: under `regulation/nl`, such as
    /// `regulation/nl/wet/leerplichtwet_1969`, or for a reduction with its
    /// cell (`cells/<cel>/reductions/<$id>`) or with none
    /// (`cells/reductions/<$id>`).
    pub map: String,
    /// The root it was read from: the repository's `corpus`, or a source.
    pub wortel: PathBuf,
    /// For a reduction with its cell: that cell, by the name of its
    /// directory. Where a reduction sits says whose it is.
    pub houder: Option<String>,
    pub versies: Vec<(String, ArticleBasedLaw)>,
}

impl Regelwerk {
    pub fn id(&self) -> &str {
        &self.versies[0].1.id
    }

    /// Every article of every version.
    fn artikelen(&self) -> impl Iterator<Item = &Article> {
        self.versies.iter().flat_map(|(_, law)| &law.articles)
    }

    /// The newest version.
    fn nieuwste(&self) -> &ArticleBasedLaw {
        &self.versies[self.versies.len() - 1].1
    }

    fn is_reductie(&self) -> bool {
        self.nieuwste().regulatory_layer == RegulatoryLayer::Reductie
    }

    /// Where it is, for a message: `<root>/<map>`.
    pub fn pad(&self) -> String {
        format!("{}/{}", self.wortel.display(), self.map)
    }
}

/// Where in a root a directory of regelwerken is: regulation, or the
/// reductions of a cell or of none.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Plek {
    Regelgeving,
    Reducties(Option<String>),
}

/// Every regelwerk of each root, read once at the start: the regulation
/// under `regulation/nl`, and the reductions with their cell
/// (`cells/<cel>/reductions`) or with none (`cells/reductions`). The
/// derivation looks across all of them for what acts on a case's
/// regelwerken, so the whole corpus has to parse, and a new regelwerk that
/// implements an open term of the Awb, or hooks on every decision, joins
/// every case, and the tests on the derivation and on the image fail.
#[derive(Debug, Clone, Default)]
pub struct Regelwerken {
    werken: Vec<Regelwerk>,
    /// What loading set aside: a `$id` of a later root that an earlier one
    /// already holds.
    pub meldingen: Vec<String>,
}

impl Regelwerken {
    /// Read every directory of regelwerken of each of `roots` that holds
    /// versions (`<date>.yaml`), through `files`; its `scenarios` are no
    /// versions. A root without them adds nothing. Two directories of one
    /// root with the same `$id` are an error: neither is the one the corpus
    /// means. Across roots the first wins, such as the regelrecht corpus
    /// over a source, and [`Regelwerken::meldingen`] says which it set
    /// aside.
    ///
    /// A `REDUCTIE` sits in a directory of reductions, named by its `$id`,
    /// and nothing else does; a reduction under `regulation/nl` is an
    /// error. A reduction of one cell that reads one of another cell is an
    /// error too: the reduction it calls would read the chronicle of the
    /// cell that runs it, not of its own.
    pub fn load(files: &dyn Files, roots: &[PathBuf]) -> Result<Self> {
        let mut uit = Self::default();
        for root in roots {
            let mut eigen: Vec<Regelwerk> = Vec::new();
            for werk in Self::uit_wortel(files, root)? {
                if let Some(ander) = eigen.iter().find(|w| w.id() == werk.id()) {
                    return Err(Error::Setup(format!(
                        "'{}' is in both {} and {}",
                        werk.id(),
                        ander.pad(),
                        werk.pad()
                    )));
                }
                eigen.push(werk);
            }
            for werk in eigen {
                if let Some(ander) = uit.get(werk.id()) {
                    uit.meldingen.push(format!(
                        "'{}' in {} is also in {}; the app runs the latter",
                        werk.id(),
                        werk.pad(),
                        ander.pad()
                    ));
                    continue;
                }
                uit.werken.push(werk);
            }
        }
        uit.geen_reductie_van_een_andere_cel()?;
        Ok(uit)
    }

    /// The directories of regelwerken of `root`, each with its place.
    fn mappen(files: &dyn Files, root: &Path) -> Result<Vec<(PathBuf, Plek)>> {
        let mut uit = Vec::new();
        let regulation = root.join("regulation/nl");
        if files.is_dir(&regulation) {
            let mut mappen = Vec::new();
            verzamel(files, &regulation, &mut mappen)?;
            uit.extend(mappen.into_iter().map(|m| (m, Plek::Regelgeving)));
        }
        let cells = root.join("cells");
        if files.is_dir(&cells) {
            for cel in files.entries(&cells)? {
                let (dir, houder) = if identity::is_shared_reductions(&cel) {
                    (cel.clone(), None)
                } else {
                    let naam = cel.file_name().map(|n| n.to_string_lossy().into_owned());
                    (cel.join(identity::REDUCTIONS), naam)
                };
                for id in identity::reductions(files, &dir)? {
                    uit.push((dir.join(id), Plek::Reducties(houder.clone())));
                }
            }
        }
        uit.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(uit)
    }

    fn uit_wortel(files: &dyn Files, root: &Path) -> Result<Vec<Regelwerk>> {
        let mut uit = Vec::new();
        for (map, plek) in Self::mappen(files, root)? {
            let mut paths: Vec<PathBuf> = files
                .entries(&map)?
                .into_iter()
                .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
                .collect();
            if paths.is_empty() {
                continue;
            }
            paths.sort();
            let mut versies = Vec::new();
            for path in paths {
                let yaml = files.read(&path)?;
                // Parsed here, so a broken version names its file at the start.
                let law = ArticleBasedLaw::from_yaml_str(&yaml)
                    .map_err(|e| Error::Setup(format!("{}: {e}", path.display())))?;
                let reductie = law.regulatory_layer == RegulatoryLayer::Reductie;
                let waar = match (&plek, reductie) {
                    (Plek::Regelgeving, true) => Some(
                        "a REDUCTIE is no regulation: it sits with its cell \
                         (cells/<cel>/reductions/) or with none (cells/reductions/)",
                    ),
                    (Plek::Reducties(_), false) => {
                        Some("a directory of reductions holds only REDUCTIE")
                    }
                    _ => None,
                };
                if let Some(waarom) = waar {
                    return Err(Error::Setup(format!("{}: {waarom}", path.display())));
                }
                if matches!(plek, Plek::Reducties(_))
                    && map.file_name().is_none_or(|n| n != law.id.as_str())
                {
                    return Err(Error::Setup(format!(
                        "{}: the reduction '{}' sits in a directory named by its $id",
                        path.display(),
                        law.id
                    )));
                }
                versies.push((yaml, law));
            }
            let houder = match &plek {
                Plek::Reducties(houder) => houder.clone(),
                Plek::Regelgeving => None,
            };
            let map = map
                .strip_prefix(root)
                .map_err(|e| Error::Setup(e.to_string()))?
                .to_string_lossy()
                .replace('\\', "/");
            uit.push(Regelwerk {
                map,
                wortel: root.to_path_buf(),
                houder,
                versies,
            });
        }
        Ok(uit)
    }

    /// Refuse a reduction that reads a reduction of another cell: a
    /// reduction is applied to the chronicle of the cell that runs it.
    fn geen_reductie_van_een_andere_cel(&self) -> Result<()> {
        for werk in self.werken.iter().filter(|w| w.is_reductie()) {
            for id in leest(werk.artikelen()) {
                let Some(ander) = self.get(&id).filter(|a| a.is_reductie()) else {
                    continue;
                };
                if ander.houder.is_some() && ander.houder != werk.houder {
                    return Err(Error::Setup(format!(
                        "{} reads the reduction '{id}' of the cell '{}' ({}); a reduction reads \
                         only those of its own cell and those of none",
                        werk.pad(),
                        ander.houder.as_deref().unwrap_or_default(),
                        ander.pad()
                    )));
                }
            }
        }
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&Regelwerk> {
        self.werken.iter().find(|r| r.id() == id)
    }
}

fn verzamel(files: &dyn Files, dir: &Path, uit: &mut Vec<PathBuf>) -> Result<()> {
    for path in files.entries(dir)? {
        if files.is_dir(&path) && path.file_name().is_none_or(|n| n != "scenarios") {
            uit.push(path.clone());
            verzamel(files, &path, uit)?;
        }
    }
    Ok(())
}

/// What follows from the law for a case.
#[derive(Debug, Clone, Serialize)]
pub struct Afleiding {
    /// The law and article of the application, and of its decision.
    pub law: String,
    pub article: String,
    /// The article that establishes the objection (`type: BEZWAAR`), and
    /// decides on it: Awb 7:11 for both cases.
    pub objection_law: String,
    pub objection: String,
    /// For a case with a court: the article that establishes the appeal
    /// (`type: BEROEP`) and decides on it, Awb 8:70, and the fact in which
    /// the cell records the receipt of a judgment ([`Beroep`]).
    pub beroep: Option<Beroep>,
    /// Who decides: the competent authority of the article, or else the
    /// holder of its rulework (RFC-002), as the app names it ("het hoofd",
    /// "het college van burgemeester en wethouders").
    pub rol: String,
    /// The regelwerken the cell runs, by `$id`, the article's law first.
    pub regelwerken: Vec<String>,
}

/// What follows from the law and the cell for an appeal.
#[derive(Debug, Clone, Serialize)]
pub struct Beroep {
    pub law: String,
    pub article: String,
    /// The fact the law requires whose parameters are all outputs of the
    /// judgment, `<law>#<article>#<fact>`: what the cell records when it
    /// receives a copy of one (`Cell::record_receipt`).
    pub ontvangst: String,
    /// The court: the other cell of the case that the competent authority of
    /// the article that decides on the appeal names ("de bestuursrechter",
    /// on Awb 8:70).
    pub rechter: String,
}

/// Derive from the law what `casus` needs, with its `cells`: its own and the
/// other cells of its sphere, whose reductions read regelwerken of their
/// own.
///
/// The regelwerken are those the article's law reaches, and the reductions
/// the catalogues of the cells name (`cel.yaml`, `answers`, RFC-048), with
/// what they reach. A regelwerk reaches another when it reads from it
/// (`source.regulation`, `source.article`, `source.decisions`), and when the
/// other is in the corpus to act on it: it implements one of its open terms,
/// such as the policy that binds a term to an answer of a cell, or has a
/// hook or a procedure that applies to what it produces (the Awb on every
/// application and decision).
pub fn afleiden(regelwerken: &Regelwerken, casus: &Casus, cells: &[&Cell]) -> Result<Afleiding> {
    let (law, article) = casus.artikel.split_once('#').ok_or_else(|| {
        Error::Setup(format!(
            "case '{}': '{}' names no article",
            casus.id, casus.artikel
        ))
    })?;

    let mut gevonden: Vec<String> = vec![law.to_string()];
    for id in cells.iter().flat_map(|cell| cell.answers().values()) {
        if !gevonden.contains(id) {
            gevonden.push(id.clone());
        }
    }

    let mut i = 0;
    while i < gevonden.len() {
        let werk = regelwerken.get(&gevonden[i]).ok_or_else(|| {
            Error::Setup(format!(
                "case '{}' needs regelwerk '{}', which is not in the corpus",
                casus.id, gevonden[i]
            ))
        })?;
        let mut erbij: Vec<String> = leest(werk.artikelen()).into_iter().collect();
        for ander in &regelwerken.werken {
            if handelt_op(ander, werk) {
                erbij.push(ander.id().to_string());
            }
        }
        for id in erbij {
            if !gevonden.contains(&id) {
                gevonden.push(id);
            }
        }
        i += 1;
    }

    let (objection_law, objection) = vestigt(regelwerken, &casus.id, &gevonden, "BEZWAAR")?;
    let beroep = beroep(
        regelwerken,
        &casus.id,
        &gevonden,
        cells.get(1..).unwrap_or_default(),
    )?;

    let werk = regelwerken
        .get(law)
        .ok_or_else(|| Error::Setup(format!("'{law}' is not in the corpus")))?;
    let rol = rol(werk.nieuwste(), article).ok_or_else(|| {
        Error::Setup(format!(
            "case '{}': {law}#{article} names no competent authority",
            casus.id
        ))
    })?;

    Ok(Afleiding {
        law: law.to_string(),
        article: article.to_string(),
        objection_law,
        objection,
        beroep,
        rol,
        regelwerken: gevonden,
    })
}

/// The appeal, for a case with a court among its `andere` cells: the
/// article that establishes it, the fact of its receipt, and the court, the
/// one cell the competent authority of that article names. A case
/// plays no appeal without other cells, without an article among its
/// regelwerken that establishes an appeal (who the court is follows from
/// that article), or when none of the other cells is the court.
fn beroep(
    regelwerken: &Regelwerken,
    casus: &str,
    gevonden: &[String],
    andere: &[&Cell],
) -> Result<Option<Beroep>> {
    if andere.is_empty() {
        return Ok(None);
    }
    // Without an article that establishes the appeal there is no appeal to
    // play, whatever the other cells are.
    let Some((law, article)) = vestigend(regelwerken, casus, gevonden, "BEROEP")? else {
        return Ok(None);
    };
    let werk = regelwerken
        .get(&law)
        .ok_or_else(|| Error::Setup(format!("'{law}' is not in the corpus")))?;
    let bevoegd = bevoegd(werk.nieuwste(), &article).ok_or_else(|| {
        Error::Setup(format!(
            "case '{casus}': {law}#{article} names no one who decides on the appeal"
        ))
    })?;
    let wie = &bevoegd.name;
    let mut rechters = andere
        .iter()
        .filter(|c| c.authority().is_named_by(std::slice::from_ref(&bevoegd)));
    let rechter = match (rechters.next(), rechters.next()) {
        (None, _) => return Ok(None),
        (Some(r), None) => r.recording_actor().to_string(),
        (Some(a), Some(b)) => {
            return Err(Error::Setup(format!(
                "case '{casus}': both '{}' and '{}' are {wie}",
                a.recording_actor(),
                b.recording_actor()
            )))
        }
    };
    let ontvangst = ontvangst(regelwerken, casus, gevonden, &law, &article)?;
    Ok(Some(Beroep {
        law,
        article,
        ontvangst,
        rechter,
    }))
}

/// The one article among the regelwerken `gevonden` whose `produces` lists a
/// submission of the kind `soort`: the objection (`BEZWAAR`) or the appeal
/// (`BEROEP`), each established by the article that decides on it.
fn vestigt(
    regelwerken: &Regelwerken,
    casus: &str,
    gevonden: &[String],
    soort: &str,
) -> Result<(String, String)> {
    vestigend(regelwerken, casus, gevonden, soort)?.ok_or_else(|| {
        Error::Setup(format!(
            "case '{casus}': no regelwerk it runs establishes a submission {soort}"
        ))
    })
}

/// The one article among the regelwerken in `gevonden` that establishes a
/// submission of the kind `soort`, if any; two are an error.
fn vestigend(
    regelwerken: &Regelwerken,
    casus: &str,
    gevonden: &[String],
    soort: &str,
) -> Result<Option<(String, String)>> {
    let mut artikelen = BTreeSet::new();
    for id in gevonden {
        if let Some(werk) = regelwerken.get(id) {
            for a in werk.artikelen() {
                if a.get_produced()
                    .into_iter()
                    .any(|p| p.submission_kind() == Some(soort))
                {
                    artikelen.insert((id.clone(), a.number.clone()));
                }
            }
        }
    }
    let mut artikelen = artikelen.into_iter();
    match (artikelen.next(), artikelen.next()) {
        (Some(b), None) => Ok(Some(b)),
        (None, _) => Ok(None),
        (Some(a), Some(b)) => Err(Error::Setup(format!(
            "case '{casus}': both {}#{} and {}#{} establish a submission {soort}",
            a.0, a.1, b.0, b.1
        ))),
    }
}

/// The fact in which a cell records the receipt of a judgment under
/// `law#article`, as `<law>#<article>#<fact>`: the one fact an article of
/// the regelwerken `gevonden` requires whose parameters are all outputs of
/// that decision, so a copy of the judgment holds each by name (Awb 8:79,
/// `afschrift_van_de_uitspraak`).
fn ontvangst(
    regelwerken: &Regelwerken,
    casus: &str,
    gevonden: &[String],
    law: &str,
    article: &str,
) -> Result<String> {
    let uitvoer: BTreeSet<&str> = regelwerken
        .get(law)
        .into_iter()
        .flat_map(|w| w.artikelen())
        .filter(|a| a.number == article)
        .flat_map(|a| {
            a.get_decision()
                .map(|d| a.outputs_of(d))
                .unwrap_or_default()
        })
        .collect();
    let mut feiten = BTreeSet::new();
    for id in gevonden {
        for a in regelwerken.get(id).into_iter().flat_map(|w| w.artikelen()) {
            for r in a.get_requires() {
                let (Some(feit), Some(parameters)) = (&r.fact, &r.parameters) else {
                    continue;
                };
                if !parameters.is_empty() && parameters.iter().all(|p| uitvoer.contains(p.as_str()))
                {
                    feiten.insert(format!("{id}#{}#{feit}", a.number));
                }
            }
        }
    }
    let mut feiten = feiten.into_iter();
    match (feiten.next(), feiten.next()) {
        (Some(f), None) => Ok(f),
        (None, _) => Err(Error::Setup(format!(
            "case '{casus}': no regelwerk it runs requires a fact that holds the judgment of {law}#{article}"
        ))),
        (Some(a), Some(b)) => Err(Error::Setup(format!(
            "case '{casus}': both {a} and {b} hold the judgment of {law}#{article}"
        ))),
    }
}

/// The regelwerken `artikelen` read from: `source.regulation`, and the law of
/// `source.article`, `source.decisions` and `source.facts` (not a
/// `$`-reference, which names the article only when it runs).
fn leest<'a>(artikelen: impl Iterator<Item = &'a Article>) -> BTreeSet<String> {
    let mut uit = BTreeSet::new();
    for a in artikelen {
        for source in a.get_inputs().iter().filter_map(|i| i.source.as_ref()) {
            if let Some(id) = &source.regulation {
                uit.insert(id.clone());
            }
            for verwijzing in [&source.article, &source.decisions, &source.facts]
                .into_iter()
                .flatten()
            {
                if let Some((id, _)) = verwijzing.split_once('#') {
                    if !id.starts_with('$') {
                        uit.insert(id.to_string());
                    }
                }
            }
        }
    }
    uit
}

/// Whether `ander` acts on `werk`: it implements one of its open terms, or
/// has a hook or procedure that applies to what `werk` produces.
fn handelt_op(ander: &Regelwerk, werk: &Regelwerk) -> bool {
    let id = werk.id();
    // An override from another law is no edge: it applies only within the
    // execution that law starts (RFC-007, "contextual law"), so it matters
    // only when that law is in the case for another reason. The
    // Vreemdelingenwet's term on Awb 6:7 is no part of a parking card.
    let vult = ander.artikelen().any(|a| {
        a.get_implements()
            .into_iter()
            .flatten()
            .any(|i| i.law == id)
    });
    let haakt = ander
        .artikelen()
        .flat_map(|a| a.get_hooks().into_iter().flatten())
        .any(|h| werk.artikelen().any(|a| past(&h.applies_to, id, a)));
    let procedure = ander
        .versies
        .iter()
        .flat_map(|(_, law)| law.procedure.iter().flatten())
        .any(|p: &ProcedureDefinition| {
            werk.artikelen().any(|a| {
                a.get_produced()
                    .into_iter()
                    .any(|x| x.legal_character() == Some(p.applies_to.legal_character.as_str()))
            })
        });
    vult || haakt || procedure
}

/// Whether the hook filter `filter` applies to what the article `a` of the
/// law `id` produces.
fn past(filter: &HookFilter, id: &str, a: &Article) -> bool {
    a.get_produced().into_iter().any(|p| {
        let besluit = filter.legal_character.as_deref().is_some_and(|lc| {
            p.legal_character() == Some(lc)
                && filter
                    .decision_type
                    .as_deref()
                    .is_none_or(|t| p.decision_type() == Some(t))
        });
        let indiening = filter.submission.as_deref().is_some_and(|k| {
            p.submission_kind() == Some(k)
                && filter
                    .established_by
                    .as_deref()
                    .is_none_or(|e| e == format!("{id}#{}", a.number))
        });
        besluit || indiening
    })
}

/// Who decides under `article` of `law`: the authority the article names,
/// else the holder of the rulework (RFC-002). A reference (`#...`) names no
/// one by itself; on an article the loader refuses a list (RFC-052).
fn bevoegd(law: &ArticleBasedLaw, article: &str) -> Option<NamedAuthority> {
    let a = law.articles.iter().find(|a| a.number == article)?;
    match a
        .machine_readable
        .as_ref()
        .and_then(|mr| mr.competent_authority.as_ref())
        .or(law.competent_authority.as_ref())?
    {
        CompetentAuthority::Structured(one) => Some(one.clone()),
        CompetentAuthority::String(s) if !s.starts_with('#') => Some(NamedAuthority {
            name: s.clone(),
            authority_type: None,
        }),
        CompetentAuthority::String(_) | CompetentAuthority::List(_) => None,
    }
}

/// Who decides under `article` of `law` ([`bevoegd`]), as the app names it:
/// a category is a kind of body, "het college van burgemeester en
/// wethouders", unless its name already says so ("het hoofd").
fn rol(law: &ArticleBasedLaw, article: &str) -> Option<String> {
    let one = bevoegd(law, article)?;
    let lidwoord = ["de ", "het ", "een "]
        .iter()
        .any(|l| one.name.to_lowercase().starts_with(l));
    Some(if one.kind() == AuthorityType::Category && !lidwoord {
        format!("het {}", one.name)
    } else {
        one.name
    })
}
