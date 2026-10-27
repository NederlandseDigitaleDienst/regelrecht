//! The chain of a case (tab Keten): from the article that decides it, the
//! open terms it leaves and the articles that fill them, what each article
//! reads from other regelwerken, the cell that answers a policy and the
//! reduction it answers from, down to what a reduction reads from the
//! chronicle of that cell.
//!
//! The chain adds no rule of its own about what a case runs. It walks the
//! regelwerken the case loads ([`crate::casus::afleiden`], in
//! [`Corpus::service`]) with the lookups the engine executes with: the
//! version in force on a day, the article that gives an output
//! (`get_article_by_output`) and the articles that fill an open term
//! (`find_implementations`, winner first). Which cell answers a policy is
//! [`Cell::bound_terms`], the routing the engine applies, and what a
//! reduction reads from a chronicle is [`kern::bron_van`], as the tab
//! Reductie shows it. Each step carries its article as written, so the
//! visitor reads the YAML the case runs.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::NaiveDate;
use regelrecht_cel::{reading, Cell, Term};
use regelrecht_engine::{Article, ArticleBasedLaw, LawExecutionService, RegulatoryLayer};
use serde::Serialize;

use crate::fragment::{self, Keuze};
use crate::kern::{self, Bron, Cellen, Corpus, Result};

/// One step of the chain.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "soort", rename_all = "snake_case")]
pub enum Schakel {
    /// An article, as the case runs it on the day of the chain.
    Artikel(Artikel),
    /// An open term of the article above it, with the articles that fill it.
    Begrip {
        /// The law and article that declare it.
        wet: String,
        artikel: String,
        id: String,
        beschrijving: Option<String>,
        /// Who judges it per case, in the words of the article
        /// (`decided_per_case_by`).
        ingevuld_door: Option<String>,
        /// Its entry under `open_terms`, as written.
        fragment: Option<String>,
        kinderen: Vec<Schakel>,
    },
    /// The cell that gives the answer `antwoord` a policy takes
    /// (`source.reduction`), from its reduction `reductie` (`cel.yaml`,
    /// `answers`). Without a cell of the case that answers, `cel` is none:
    /// the engine then leaves an input that may be absent empty, and refuses
    /// the decision for one that may not.
    Cel {
        cel: Option<String>,
        antwoord: String,
        reductie: Option<String>,
        /// The line of `answers` in the cell's `cel.yaml` that names the
        /// reduction, as written.
        fragment: Option<String>,
        kinderen: Vec<Schakel>,
    },
    /// What a reduction reads from the chronicle of the cell `cel`: the
    /// decisions under an article, or the executograms of a fact.
    Kroniek {
        besluiten_onder: Option<String>,
        executogrammen_van: Option<String>,
        cel: String,
    },
    /// An article the chain already showed above, with the same parameters
    /// and on the chronicle of the same cell.
    Eerder { wet: String, artikel: String },
}

/// An article in the chain.
#[derive(Debug, Clone, Serialize)]
pub struct Artikel {
    pub wet: String,
    pub artikel: String,
    /// The name the regelwerk carries.
    pub naam: Option<String>,
    /// Its `regulatory_layer`.
    pub laag: String,
    /// The file of the version in force, under its root.
    pub bestand: Option<String>,
    pub url: Option<String>,
    pub tekst: Option<String>,
    /// The article's `machine_readable` as written in that file.
    pub yaml: Option<String>,
    /// Invented for the PoC: a reduction, a regelwerk whose `$id` or name
    /// says it is fictional, or an article whose text says so.
    pub fictief: bool,
    /// The blocks of `yaml` that make the link to the steps around it
    /// ([`fragment::van_artikel`]): its `implements`, the inputs that take
    /// an answer of a cell, read the chronicle or pass a literal parameter,
    /// and the actions that set what the step above reads or the open term
    /// it fills.
    pub fragment: Option<String>,
    /// The outputs the step above reads from it; for the root, the outcome
    /// of the case.
    pub leest: Vec<String>,
    /// The open term of the step above that it fills.
    pub vult: Option<String>,
    /// The answers of a cell it takes (`source.reduction`).
    pub vraagt: Vec<String>,
    /// The literal parameters the step above passes, such as `basis`.
    pub parameters: BTreeMap<String, String>,
    pub kinderen: Vec<Schakel>,
}

/// The chain of the case of `corpus` on `on`, with the cells of `cellen`
/// as the ones that answer: its root is the article that decides the case.
pub fn van_de_casus(corpus: &Corpus, cellen: &Cellen, on: NaiveDate) -> Result<Schakel> {
    let service = corpus.service()?;
    let mut keten = Keten {
        corpus,
        service: &service,
        cellen: std::iter::once(&cellen.eigen)
            .chain(&cellen.andere)
            .collect(),
        on,
        gezien: BTreeSet::new(),
    };
    let a = &corpus.afleiding;
    keten
        .artikel(
            &a.law,
            &a.article,
            &BTreeMap::new(),
            &corpus.casus.cel,
            vec![corpus.casus.uitkomst.clone()],
            None,
        )?
        .ok_or_else(|| {
            regelrecht_cel::Error::Setup(format!(
                "case '{}': no version of {}#{} in force on {on} has that article",
                corpus.casus.id, a.law, a.article
            ))
        })
}

struct Keten<'a> {
    corpus: &'a Corpus,
    service: &'a LawExecutionService,
    cellen: Vec<&'a Cell>,
    on: NaiveDate,
    /// Per article shown: its law, number, parameters and the cell whose
    /// chronicle it reads.
    gezien: BTreeSet<(String, String, BTreeMap<String, String>, String)>,
}

/// Where an input of an article leads, before it becomes a step: the steps
/// of one article it reads, with the outputs, are joined.
enum Doel {
    Artikel {
        wet: String,
        artikel: String,
        leest: Vec<String>,
        parameters: BTreeMap<String, String>,
    },
    Schakel(Box<Schakel>),
}

impl<'a> Keten<'a> {
    fn law(&self, id: &str) -> Option<&'a ArticleBasedLaw> {
        self.service.resolver().get_law_for_date(id, Some(self.on))
    }

    /// The step of `wet#artikel`, reached with the literal parameters
    /// `given`, on the chronicle of `cel`, read for `leest` or filling the
    /// open term `vult`; none for a
    /// regelwerk without a version in force on the day, or a version without
    /// that article.
    fn artikel(
        &mut self,
        wet: &str,
        artikel: &str,
        given: &BTreeMap<String, String>,
        cel: &str,
        leest: Vec<String>,
        vult: Option<&str>,
    ) -> Result<Option<Schakel>> {
        let Some(law) = self.law(wet) else {
            return Ok(None);
        };
        let Some(article) = law.find_article_by_number(artikel) else {
            return Ok(None);
        };
        let sleutel = (
            wet.to_string(),
            artikel.to_string(),
            given.clone(),
            cel.to_string(),
        );
        if !self.gezien.insert(sleutel) {
            return Ok(Some(Schakel::Eerder {
                wet: wet.to_string(),
                artikel: artikel.to_string(),
            }));
        }
        let mut kinderen = self.begrippen(law, article, cel)?;
        kinderen.extend(self.bronnen(law, article, given, cel)?);
        let geschreven = self.corpus.yaml_van(&law.id, &law.valid_from);
        let yaml = geschreven.and_then(|y| fragment::machine_readable(y, artikel));
        let invoer = schakelende_invoer(article);
        let uitvoer: Vec<String> = leest
            .iter()
            .cloned()
            .chain(vult.map(str::to_string))
            .collect();
        let keuze = Keuze {
            implements: vult.is_some(),
            invoer: &invoer,
            begrip: None,
            uitvoer: &uitvoer,
        };
        let fragment = geschreven.and_then(|y| fragment::van_artikel(y, artikel, &keuze));
        let vraagt = article
            .get_inputs()
            .iter()
            .filter_map(|i| i.source.as_ref()?.reduction.clone())
            .collect();
        let bestand = self
            .corpus
            .bestand_van(&law.id, &law.valid_from)
            .map(str::to_string);
        Ok(Some(Schakel::Artikel(Artikel {
            wet: law.id.clone(),
            artikel: artikel.to_string(),
            naam: law.name.clone(),
            laag: law.regulatory_layer.as_str().to_string(),
            bestand,
            url: article.url.clone().or_else(|| law.url.clone()),
            tekst: Some(article.text.clone()).filter(|t| !t.trim().is_empty()),
            yaml,
            fragment,
            fictief: fictief(law, article),
            leest,
            vult: vult.map(str::to_string),
            vraagt,
            parameters: given.clone(),
            kinderen,
        })))
    }

    /// The open terms of `article`, each with the articles that fill it on
    /// the day, the engine's winner first.
    fn begrippen(
        &mut self,
        law: &ArticleBasedLaw,
        article: &Article,
        cel: &str,
    ) -> Result<Vec<Schakel>> {
        let mut uit = Vec::new();
        for term in article.get_open_terms().into_iter().flatten() {
            let vullers: Vec<(String, String)> = self
                .service
                .resolver()
                .find_implementations(
                    &law.id,
                    &article.number,
                    term,
                    Some(self.on),
                    &HashMap::new(),
                )
                .map(|lookup| {
                    lookup
                        .implementations
                        .iter()
                        .map(|(l, a)| (l.id.clone(), a.number.clone()))
                        .collect()
                })?;
            let mut kinderen = Vec::new();
            for (w, a) in &vullers {
                kinderen.extend(self.artikel(
                    w,
                    a,
                    &BTreeMap::new(),
                    cel,
                    Vec::new(),
                    Some(&term.id),
                )?);
            }
            let fragment = self
                .corpus
                .yaml_van(&law.id, &law.valid_from)
                .and_then(|y| {
                    let keuze = Keuze {
                        begrip: Some(&term.id),
                        ..Keuze::default()
                    };
                    fragment::van_artikel(y, &article.number, &keuze)
                });
            uit.push(Schakel::Begrip {
                wet: law.id.clone(),
                artikel: article.number.clone(),
                id: term.id.clone(),
                beschrijving: term.description.clone(),
                ingevuld_door: term.decided_per_case_by.clone(),
                fragment,
                kinderen,
            });
        }
        Ok(uit)
    }

    /// What the inputs of `article` read, in their order: another article
    /// (one step per article, with every output read from it), the answer
    /// of a cell, or the chronicle.
    fn bronnen(
        &mut self,
        law: &ArticleBasedLaw,
        article: &Article,
        given: &BTreeMap<String, String>,
        cel: &str,
    ) -> Result<Vec<Schakel>> {
        let mut doelen: Vec<Doel> = Vec::new();
        for source in article
            .get_inputs()
            .iter()
            .filter_map(|i| i.source.as_ref())
        {
            if let Some(bron) = kern::bron_van(source, given) {
                let (besluiten_onder, executogrammen_van) = match bron {
                    Bron::Besluiten(b) => (Some(b), None),
                    Bron::Feiten(f) => (None, Some(f)),
                };
                doelen.push(Doel::Schakel(Box::new(Schakel::Kroniek {
                    besluiten_onder,
                    executogrammen_van,
                    cel: cel.to_string(),
                })));
                continue;
            }
            if let Some(antwoord) = &source.reduction {
                let parameters = kern::doorgegeven(source, given);
                doelen.extend(
                    self.cellen_voor(law, article, antwoord, &parameters)?
                        .into_iter()
                        .map(|s| Doel::Schakel(Box::new(s))),
                );
                continue;
            }
            let doel = match (&source.regulation, &source.output, &source.article) {
                // Another regelwerk, or (without one) the article's own.
                (regelwerk, Some(output), None) => {
                    let wet = regelwerk.as_deref().unwrap_or(&law.id);
                    let parameters = match regelwerk {
                        Some(_) => kern::doorgegeven(source, given),
                        None => given.clone(),
                    };
                    self.service
                        .resolver()
                        .get_article_by_output(wet, output, Some(self.on))
                        .map(|a| {
                            (
                                wet.to_string(),
                                a.number.clone(),
                                Some(output.clone()),
                                parameters,
                            )
                        })
                }
                // The decision under an article (RFC-050), as written or
                // in a literal parameter.
                (_, output, Some(verwijzing)) => {
                    let verwijzing = match verwijzing.strip_prefix('$') {
                        None => Some(verwijzing.clone()),
                        Some(naam) => given.get(naam).cloned(),
                    };
                    verwijzing.and_then(|v| {
                        let (wet, nummer) = v.split_once('#')?;
                        Some((
                            wet.to_string(),
                            nummer.to_string(),
                            output.clone(),
                            kern::doorgegeven(source, given),
                        ))
                    })
                }
                _ => None,
            };
            let Some((wet, nummer, output, parameters)) = doel else {
                continue;
            };
            if wet == law.id && nummer == article.number {
                continue;
            }
            let zelfde = doelen.iter_mut().find_map(|d| match d {
                Doel::Artikel {
                    wet: w,
                    artikel: a,
                    leest,
                    parameters: p,
                } if *w == wet && *a == nummer && *p == parameters => Some(leest),
                _ => None,
            });
            match zelfde {
                Some(leest) => leest.extend(output.filter(|o| !leest.contains(o))),
                None => doelen.push(Doel::Artikel {
                    wet,
                    artikel: nummer,
                    leest: output.into_iter().collect(),
                    parameters,
                }),
            }
        }
        let mut uit = Vec::new();
        for d in doelen {
            match d {
                Doel::Artikel {
                    wet,
                    artikel,
                    leest,
                    parameters,
                } => uit.extend(self.artikel(&wet, &artikel, &parameters, cel, leest, None)?),
                Doel::Schakel(s) => uit.push(*s),
            }
        }
        Ok(uit)
    }

    /// The cells that give `antwoord` to the policy `law#article`: those whose
    /// catalogue gives it and to which the engine routes a term that this
    /// article binds to this answer ([`reading::bindings`],
    /// [`Cell::bound_terms`]), each with the article of its reduction that
    /// gives the answer.
    fn cellen_voor(
        &mut self,
        law: &ArticleBasedLaw,
        article: &Article,
        antwoord: &str,
        parameters: &BTreeMap<String, String>,
    ) -> Result<Vec<Schakel>> {
        let vult: Vec<Term> = reading::bindings(self.service, self.on)
            .into_iter()
            .filter(|b| b.law_id == law.id && b.article == article.number && b.answer == antwoord)
            .map(|b| b.term)
            .collect();
        let mut uit = Vec::new();
        for cell in self.cellen.clone() {
            let Some(reductie) = cell.answers().get(antwoord) else {
                continue;
            };
            if !cell
                .bound_terms(self.service, self.on)
                .iter()
                .any(|t| vult.contains(t))
            {
                continue;
            }
            let actor = cell.recording_actor().to_string();
            let nummer = self
                .service
                .resolver()
                .get_article_by_output(reductie, antwoord, Some(self.on))
                .map(|a| a.number.clone());
            let mut kinderen = Vec::new();
            if let Some(nummer) = nummer {
                kinderen.extend(self.artikel(
                    reductie,
                    &nummer,
                    parameters,
                    &actor,
                    vec![antwoord.to_string()],
                    None,
                )?);
            }
            let fragment = self
                .corpus
                .cel_yaml(&actor)
                .and_then(|y| fragment::van_catalogus(&y, antwoord));
            uit.push(Schakel::Cel {
                cel: Some(actor),
                antwoord: antwoord.to_string(),
                reductie: Some(reductie.clone()),
                fragment,
                kinderen,
            });
        }
        if uit.is_empty() {
            uit.push(Schakel::Cel {
                cel: None,
                antwoord: antwoord.to_string(),
                reductie: None,
                fragment: None,
                kinderen: Vec::new(),
            });
        }
        Ok(uit)
    }
}

/// Whether the PoC invented `article` of `law`: every reduction, a
/// regelwerk whose `$id` or name says it is fictional, or an article whose
/// text says so ("FICTIEF").
fn fictief(law: &ArticleBasedLaw, article: &Article) -> bool {
    law.regulatory_layer == RegulatoryLayer::Reductie
        || law.id.starts_with("fictie")
        || law
            .name
            .as_deref()
            .is_some_and(|n| n.to_lowercase().contains("fictie"))
        || article.text.contains("FICTIEF")
}

/// The inputs of `article` that link it to a step below in a way its
/// outputs do not show: those that take an answer of a cell, read the
/// chronicle, or pass a literal parameter such as `basis`.
fn schakelende_invoer(article: &Article) -> Vec<String> {
    article
        .get_inputs()
        .iter()
        .filter(|i| {
            i.source.as_ref().is_some_and(|s| {
                s.reduction.is_some()
                    || s.decisions.is_some()
                    || s.facts.is_some()
                    || s.parameters
                        .iter()
                        .flatten()
                        .any(|(_, v)| !v.starts_with('$'))
            })
        })
        .map(|i| i.name.clone())
        .collect()
}
