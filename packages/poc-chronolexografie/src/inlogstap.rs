//! The simulated login step of a case ([`Inlogstap`]): logging in as with
//! eHerkenning, without a real means of login.
//!
//! The applicant gives a user name and the password every test account
//! shares ([`WACHTWOORD`]). The accredited service's cell answers from its
//! register whether that user may act for the service on the day of the
//! login, through an article of its own policy (`Cell::consult`). When it
//! may, the cell records the authentication as the fact of the case's login,
//! and the claims come back as the assertion. When not, nothing is recorded,
//! as the authorization register answers Deny.
//!
//! Who is logged in is no part of a chronicle. The assertion goes back to
//! the browser, which keeps it as the session, and the form reads it from
//! there. A chronicle records that an authentication took place; it can
//! serve several sessions at once, so nothing here reads who is logged in
//! from it.

use chrono::NaiveDate;
use regelrecht_cel::{EXECUTOGRAM, Error, shape};
use regelrecht_engine::ArticleResult;
use regelrecht_law_model::stored;
use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::casus::{Inloggen, Inlogstap};
use crate::kern::{self, Cellen, Corpus, Result};

/// The password of every test account. The login screen shows it: the step
/// is a simulation, and its accounts guard nothing.
pub const WACHTWOORD: &str = "welkom";

/// The parameter of the access article, and the field of the means of
/// login, that holds the user name.
pub const GEBRUIKERSNAAM: &str = "gebruikersnaam";

/// The parameter of the access article that holds the service.
pub const DIENST: &str = "dienst";

/// What a login comes to.
#[derive(Debug, Serialize)]
#[serde(tag = "uitkomst", rename_all = "snake_case")]
pub enum Inlog {
    /// The service asserts the claims, and the cell recorded the
    /// authentication.
    Ingelogd(Assertie),
    /// The service refused; nothing was recorded.
    Geweigerd {
        gebruikersnaam: String,
        reden: Weigering,
    },
}

/// What the service asserts at a login: the claims, by the names of the
/// outputs of the access article other than access itself, and the record
/// of the authentication.
#[derive(Debug, Serialize)]
pub struct Assertie {
    pub gebruikersnaam: String,
    pub dienst: String,
    pub dag: NaiveDate,
    pub claims: Map<String, Value>,
    pub vastgelegd: String,
}

/// Why the service refused a login.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Weigering {
    /// The password is not [`WACHTWOORD`].
    Wachtwoord,
    /// The register holds no valid authorization of this user for the
    /// service, or no means of login with this user name.
    GeenMachtiging,
}

/// The login of the case and its login step.
///
/// # Errors
///
/// Refused when the case has no login step.
fn van(corpus: &Corpus) -> Result<(&Inloggen, &Inlogstap)> {
    corpus
        .casus
        .inloggen
        .as_ref()
        .and_then(|i| Some((i, i.inlogstap.as_ref()?)))
        .ok_or_else(|| {
            Error::Refused(format!(
                "the case '{}' has no login step (inloggen.inlogstap)",
                corpus.casus.id
            ))
        })
}

/// What the login screen shows on `on`: the cell that checks, its name, the
/// service, the password and the user names of the means of login the cell
/// issued by then (`middel`), in the order it issued them.
///
/// # Errors
///
/// Refused when the case has no login step or its cell is not of the case.
pub fn aanbod(corpus: &Corpus, cellen: &Cellen, on: NaiveDate) -> Result<Value> {
    let (_, stap) = van(corpus)?;
    let (cell, _) = cellen.voor(&stap.cel)?;
    let mut gebruikersnamen: Vec<&Value> = Vec::new();
    if let Some(middel) = &stap.middel {
        let (basis, feit) = shape::split_fact(middel)?;
        for g in cell.grams() {
            let van_middel = g.kind == EXECUTOGRAM
                && g.basis == basis
                && g.fact.as_deref() == Some(feit)
                && g.recorded_on <= on;
            if let Some(naam) = g.fields.get(GEBRUIKERSNAAM).filter(|_| van_middel)
                && !gebruikersnamen.contains(&naam)
            {
                gebruikersnamen.push(naam);
            }
        }
    }
    Ok(json!({
        "cel": stap.cel,
        "cel_naam": corpus.cel_naam(&stap.cel),
        "dienst": stap.dienst,
        "wachtwoord": WACHTWOORD,
        "gebruikersnamen": gebruikersnamen,
    }))
}

/// Log in on `on` as `gebruikersnaam` with `wachtwoord`, for `dienst` or
/// else the service of the case's login step. A wrong password is refused
/// without asking the cell. Otherwise the cell answers the access article
/// of its login step; when that grants access, it records the fact of the
/// case's login with the claims, and the claims come back.
///
/// # Errors
///
/// Refused when the case has no login step; when the cell refuses to answer
/// ([`regelrecht_cel::Cell::consult`]); when the claims miss a field of the
/// fact of the login; and whatever the cell refuses when it records it
/// ([`kern::vastleggen`]).
pub fn inloggen(
    corpus: &Corpus,
    cellen: &mut Cellen,
    gebruikersnaam: &str,
    wachtwoord: &str,
    dienst: Option<&str>,
    on: NaiveDate,
) -> Result<Inlog> {
    let (login, stap) = van(corpus)?;
    let geweigerd = |reden| Inlog::Geweigerd {
        gebruikersnaam: gebruikersnaam.to_string(),
        reden,
    };
    if wachtwoord != WACHTWOORD {
        return Ok(geweigerd(Weigering::Wachtwoord));
    }
    let dienst = dienst.unwrap_or(&stap.dienst);
    let Some(claims) = claims(corpus, cellen, stap, gebruikersnaam, dienst, on)? else {
        return Ok(geweigerd(Weigering::GeenMachtiging));
    };
    let service = corpus.service()?;
    let mut velden = Map::new();
    for veld in shape::fact_fields(&service, &login.feit, on, None)? {
        let waarde = claims.get(&veld.name).ok_or_else(|| {
            Error::Refused(format!(
                "the login step of the case asserts no '{}', a field of the fact {}",
                veld.name, login.feit
            ))
        })?;
        velden.insert(veld.name, waarde.clone());
    }
    let gram = kern::vastleggen(corpus, cellen, &stap.cel, &login.feit, &velden, on)?;
    Ok(Inlog::Ingelogd(Assertie {
        gebruikersnaam: gebruikersnaam.to_string(),
        dienst: dienst.to_string(),
        dag: on,
        claims,
        vastgelegd: gram.id,
    }))
}

/// The claims the cell of `stap` asserts for `gebruikersnaam` and `dienst`
/// on `on`: every output of its access article but access itself, by name.
/// `None` when the article does not grant access.
fn claims(
    corpus: &Corpus,
    cellen: &Cellen,
    stap: &Inlogstap,
    gebruikersnaam: &str,
    dienst: &str,
    on: NaiveDate,
) -> Result<Option<Map<String, Value>>> {
    let service = corpus.service()?;
    let (cell, sfeer) = cellen.voor(&stap.cel)?;
    let given = Map::from_iter([
        (GEBRUIKERSNAAM.to_string(), json!(gebruikersnaam)),
        (DIENST.to_string(), json!(dienst)),
    ]);
    let result = cell.consult(&service, &sfeer, &stap.toegang, &given, on)?;
    let (basis, toegang) = stap
        .toegang
        .rsplit_once('#')
        .ok_or_else(|| Error::Refused(format!("'{}' names no output", stap.toegang)))?;
    if uitvoer(&result, toegang)? != Value::Bool(true) {
        return Ok(None);
    }
    // `consult` found the article, so it is in force on `on`.
    let (law, article) = basis.split_once('#').unwrap_or((basis, ""));
    let namen: Vec<String> = service
        .resolver()
        .get_law_for_date(law, Some(on))
        .and_then(|l| l.find_article_by_number(article))
        .map(|a| a.get_output_names().into_iter().map(String::from).collect())
        .unwrap_or_default();
    let mut claims = Map::new();
    for naam in namen.into_iter().filter(|n| n != toegang) {
        // The engine computes the outputs it was asked for; another is
        // asked on its own.
        let waarde = if result.outputs.contains_key(&naam) {
            uitvoer(&result, &naam)?
        } else {
            let bron = format!("{basis}#{naam}");
            uitvoer(&cell.consult(&service, &sfeer, &bron, &given, on)?, &naam)?
        };
        claims.insert(naam, waarde);
    }
    Ok(Some(claims))
}

/// The output `naam` of `result` as the chronicle stores a value, `null`
/// when it has none.
fn uitvoer(result: &ArticleResult, naam: &str) -> Result<Value> {
    Ok(result
        .outputs
        .get(naam)
        .map(stored::to_stored)
        .transpose()?
        .unwrap_or(Value::Null))
}
