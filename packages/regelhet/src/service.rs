//! The steps of the process for every user at once: per request the cells
//! are loaded from the [`ChronicleStore`], the step of the kern runs on them
//! on the day the [`Clock`] gives, and what the step recorded goes back to
//! the store in one [`Commit`].
//!
//! The steps are those of the chronolexografie poc (`kern`); this module only
//! carries the state between them. A step that writes ([`Step`]) is
//! idempotent under its key, when it has one, and runs again on fresh
//! chronicles when another request got in first ([`StoreError::Conflict`]):
//! [`MAX_ATTEMPTS`] runs in all. A step that only reads records nothing.
//!
//! Who may run which step is the API's to decide ([`crate::access`]); the
//! service only carries who asked into the store, beside the records
//! ([`Commit::recorded_by`]), and answers the questions the API asks to
//! decide: which cell holds a record ([`Service::holder`]) and which
//! records are about one person ([`Service::dossier`]).
//!
//! The service runs the process for one cell, the executor
//! ([`Service::executor`]): the case's own cell, which the case file names
//! (`cel`). The other cells of the case are external parties
//! ([`crate::external`]); their chronicles run in the same commits, but what
//! they do comes in through the admin panel.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use chrono::NaiveDate;
use regelrecht_cel::Gram;
use regelrecht_poc_chronolexografie::inlogstap;
use regelrecht_poc_chronolexografie::kern::{self, Cellen, Corpus, Indiener};
use regelrecht_poc_chronolexografie::stand::{self, Stand};
use regelrecht_poc_chronolexografie::ui_spec::Systeemvulling;
use regelrecht_poc_chronolexografie::vervolg;
use regelrecht_poc_chronolexografie::verzoek::{self, Verzoek};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::clock::Clock;
use crate::external;
use crate::store::{
    Actor, Appended, CellAppend, Channel, ChronicleStore, Commit, IdempotencyKey, Keyed, Receipt,
    StoreError, StoredChronicle,
};

/// How often a step runs in all, the first run included, before the service
/// gives up after conflicts.
pub const MAX_ATTEMPTS: usize = 3;

/// A submission under `artikel` (the case's own article when absent),
/// referring to the decision `op` if given (`kern::indienen`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Submit {
    #[serde(default, rename = "artikel")]
    pub article: Option<String>,
    #[serde(default, rename = "op")]
    pub refers_to: Option<String>,
    #[serde(default, rename = "velden")]
    pub fields: Map<String, Value>,
    /// Who submits, as the server knows them: set by the server, never read
    /// from a request (a request that names it is refused as an unknown
    /// field). For the case's application, what the UI spec has the system
    /// fill is filled from it when the step runs ([`kern::systeemvelden`]).
    #[serde(
        default,
        skip_deserializing,
        rename = "indiener",
        skip_serializing_if = "Option::is_none"
    )]
    pub submitter: Option<Indiener>,
}

/// A decision on the submission `op`, after the record `na` if given, with
/// the deciding authority's findings (`kern::beslissen`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decide {
    #[serde(rename = "op")]
    pub submission: String,
    #[serde(default, rename = "na")]
    pub after: Option<String>,
    #[serde(default, rename = "bevindingen")]
    pub findings: Map<String, Value>,
}

/// The cell `cel` records the fact `feit` (`<regelwerk>#<article>#<fact>`),
/// about the record `verwijst_naar` if given (`kern::vastleggen_verwijzend`).
/// With `verzoek`, the fact answers the request whose receipt in the
/// chronicle of `cel` that id names (`verzoek::beantwoorden`): the fields the
/// answer shares with the request come from the request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    #[serde(rename = "cel")]
    pub cell: String,
    #[serde(rename = "feit")]
    pub fact: String,
    #[serde(default, rename = "velden")]
    pub fields: Map<String, Value>,
    #[serde(default, rename = "verwijst_naar")]
    pub refers_to: Option<String>,
    #[serde(default, rename = "verzoek", skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
}

/// The cell that holds the decision `besluit` sends a copy of it to the cell
/// whose decision it concerns (`kern::toezenden`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendCopy {
    #[serde(rename = "besluit")]
    pub decision: String,
}

/// The cell `cel` records the fact `feit` as its own server recorded it and
/// sent it over a link (`kern::overnemen`, [`Service::take_over`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TakeOver {
    #[serde(rename = "cel")]
    pub cell: String,
    #[serde(rename = "feit")]
    pub fact: String,
    #[serde(default, rename = "velden")]
    pub fields: Map<String, Value>,
    /// The day that cell's server recorded it on.
    #[serde(rename = "datum")]
    pub on: NaiveDate,
}

/// A step that records. Its JSON form, tagged with `stap`, is the request a
/// [`Receipt`] keeps.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "stap")]
pub enum Step {
    #[serde(rename = "indienen")]
    Submit(Submit),
    #[serde(rename = "beslissen")]
    Decide(Decide),
    #[serde(rename = "vastleggen")]
    Record(Record),
    #[serde(rename = "toezenden")]
    SendCopy(SendCopy),
    #[serde(rename = "overnemen")]
    TakeOver(TakeOver),
    #[serde(rename = "inloggen")]
    LogIn(LogIn),
}

/// A login through the login step of the case (`inloggen.inlogstap`), as
/// with eHerkenning: the user name and the password the applicant gave
/// ([`Service::log_in`]). The password is the one every test account of the
/// simulation shares, and it is never stored: a login commits without a key,
/// so no receipt holds the request.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogIn {
    pub gebruikersnaam: String,
    #[serde(skip_serializing)]
    pub wachtwoord: String,
}

// Without the password: a step in a log line never shows it.
impl std::fmt::Debug for LogIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogIn")
            .field("gebruikersnaam", &self.gebruikersnaam)
            .finish_non_exhaustive()
    }
}

/// What a login through the case's login step came to ([`Service::log_in`]).
#[derive(Debug, Clone, PartialEq)]
pub enum LoginOutcome {
    /// The service asserts these claims, and its cell recorded the
    /// authentication.
    LoggedIn(Map<String, Value>),
    /// The password is not the simulation's; nothing was recorded.
    WrongPassword,
    /// The register holds no authorization of this user for the service;
    /// nothing was recorded.
    NotAuthorized,
}

impl LoginOutcome {
    /// The outcome in `answer`, the step's answer as `inlogstap::Inlog`
    /// serializes it.
    ///
    /// # Errors
    ///
    /// [`ServiceError::Internal`] for an answer of another shape: a change of
    /// `Inlog` is said, not read as a refusal.
    fn from_answer(answer: &Value) -> Result<Self, ServiceError> {
        match (answer["uitkomst"].as_str(), answer["reden"].as_str()) {
            (Some("ingelogd"), _) => answer["claims"]
                .as_object()
                .cloned()
                .map(Self::LoggedIn)
                .ok_or_else(|| ServiceError::Internal("a login without claims".into())),
            (Some("geweigerd"), Some("wachtwoord")) => Ok(Self::WrongPassword),
            (Some("geweigerd"), Some("geen_machtiging")) => Ok(Self::NotAuthorized),
            _ => Err(ServiceError::Internal(format!(
                "the login step answered in an unknown shape: {answer}"
            ))),
        }
    }
}

/// What a step came to in the kern: its answer, and the records the
/// executor's policy made in it besides ([`Commit::by_policy`]).
struct Applied {
    response: Value,
    by_policy: Vec<String>,
}

impl Applied {
    fn only(response: Value) -> Self {
        Self {
            response,
            by_policy: Vec::new(),
        }
    }
}

impl Step {
    /// Run the step of the kern on `cellen`, on `on`, and then every act the
    /// system takes itself that the step brought about, such as ordering a
    /// card once a grant counts (`vervolg::automatisch`): only for the
    /// grants this step changed. Those land in the same commit as the step,
    /// recorded by the article that has the system take them
    /// ([`Commit::by_policy`]).
    fn apply(&self, corpus: &Corpus, cellen: &mut Cellen, on: NaiveDate) -> kern::Result<Applied> {
        let voor = vervolg::standen(corpus, cellen, on)?;
        let mut applied = self.apply_step(corpus, cellen, on)?;
        applied
            .by_policy
            .extend(vervolg::automatisch(corpus, cellen, &voor, on)?);
        Ok(applied)
    }

    /// The step of the kern itself ([`Step::apply`]).
    fn apply_step(
        &self,
        corpus: &Corpus,
        cellen: &mut Cellen,
        on: NaiveDate,
    ) -> kern::Result<Applied> {
        match self {
            Self::Submit(s) => {
                let article = s.article.as_deref().unwrap_or(&corpus.casus.artikel);
                let mut fields = s.fields.clone();
                // What the system fills is filled here, on the day the step
                // runs and on the chronicles it runs on, so the day of the
                // application is the day it is recorded on.
                if let Some(submitter) = &s.submitter {
                    let filled = kern::systeemvelden(
                        corpus,
                        cellen,
                        article,
                        s.refers_to.as_deref(),
                        submitter,
                        &fields,
                        on,
                    )?;
                    fields.extend(filled);
                }
                let (gram, by_policy) = kern::indienen_volgens_beleid(
                    corpus,
                    cellen,
                    article,
                    s.refers_to.as_deref(),
                    &fields,
                    on,
                )?;
                Ok(Applied {
                    response: to_value(&gram)?,
                    by_policy,
                })
            }
            Self::Decide(d) => {
                let besluit = kern::beslissen(
                    corpus,
                    cellen,
                    &d.submission,
                    d.after.as_deref(),
                    &d.findings,
                    on,
                )?;
                to_value(&besluit).map(Applied::only)
            }
            Self::Record(Record {
                cell,
                fact,
                fields,
                refers_to: None,
                request: Some(request),
            }) => {
                let gram = verzoek::beantwoorden(corpus, cellen, cell, request, fact, fields, on)?;
                to_value(&gram).map(Applied::only)
            }
            Self::Record(Record {
                request: Some(_), ..
            }) => Err(regelrecht_cel::Error::Refused(
                "a fact answers a request (`verzoek`) or refers to a record (`verwijst_naar`), \
                 not both"
                    .into(),
            )),
            Self::Record(r) => {
                // An open request this fact would answer is answered from the
                // request, so the answer cannot miss the request it belongs to.
                if let Some(open) = verzoek::open_voor(corpus, cellen, &r.cell, &r.fact, &r.fields)?
                {
                    let id = open.ontvangen.map(|g| g.id).unwrap_or_default();
                    return Err(regelrecht_cel::Error::Refused(format!(
                        "'{}' has an open request this fact answers; record it from that request \
                         (`verzoek`: '{id}')",
                        r.cell
                    )));
                }
                // What the UI spec has the system fill in a fact, such as
                // its day by the clock, is filled here, on the day the step
                // runs; a request that names such a field is refused.
                // Only for the executor's own facts: an external party's
                // fact comes in through the admin panel as that party gave it.
                let filled = if r.cell == corpus.casus.cel {
                    kern::feit_systeemvelden(corpus, &r.fact, on)?
                } else {
                    Map::new()
                };
                if let Some(field) = r.fields.keys().find(|f| filled.contains_key(*f)) {
                    return Err(regelrecht_cel::Error::Refused(format!(
                        "'{field}' is filled by the system; the request may not give it"
                    )));
                }
                let mut fields = r.fields.clone();
                fields.extend(filled);
                let gram = kern::vastleggen_verwijzend(
                    corpus,
                    cellen,
                    &r.cell,
                    &r.fact,
                    &fields,
                    r.refers_to.as_deref(),
                    on,
                )?;
                to_value(&gram).map(Applied::only)
            }
            Self::SendCopy(s) => {
                to_value(&kern::toezenden(corpus, cellen, &s.decision, on)?).map(Applied::only)
            }
            // On the sender's day, not this server's: the record says when
            // its own cell recorded it.
            Self::TakeOver(t) => to_value(&kern::overnemen(
                corpus, cellen, &t.cell, &t.fact, &t.fields, t.on,
            )?)
            .map(Applied::only),
            Self::LogIn(l) => to_value(&inlogstap::inloggen(
                corpus,
                cellen,
                &l.gebruikersnaam,
                &l.wachtwoord,
                None,
                on,
            )?)
            .map(Applied::only),
        }
    }
}

/// The fields of a record that name the identity beside a login key, in
/// order: the case's own `inloggen.naam` is tried first
/// ([`Service::begin_identities`] takes these when it has none).
pub const NAME_FIELDS: [&str; 3] = ["handelsnaam", "statutaire_naam", "naam"];

/// A login key as text: a string as it is, a number in digits.
fn identity_text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.trim().is_empty() => Some(s.trim().to_owned()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn to_value(value: &impl Serialize) -> kern::Result<Value> {
    serde_json::to_value(value)
        .map_err(|e| regelrecht_cel::Error::Setup(format!("the answer does not serialize: {e}")))
}

/// Why a step gives no answer.
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    /// The cell or the engine refused the request: the caller's to correct.
    #[error("{0}")]
    Refused(String),
    #[error("there is no cell '{0}' in this process")]
    UnknownCell(String),
    /// The key was used before, for another request.
    #[error("the idempotency key '{0}' was used for another request")]
    KeyReused(String),
    /// Other requests kept getting in first.
    #[error("the chronicles changed {MAX_ATTEMPTS} times while the step ran; try again")]
    Busy,
    #[error(transparent)]
    Store(StoreError),
    /// What the service cannot blame on the request.
    #[error("{0}")]
    Internal(String),
    /// The case names no single field that the login fills, so the server
    /// cannot tell whose application a submission is.
    #[error(
        "the case names {0} fields the login fills (inloggen.veld, or a UI spec field with \
         `fill: {{by: system}}` and no source); the server needs exactly one"
    )]
    LoginField(usize),
    /// The UI spec prefills fields the applicant may change (`fill: {from}`
    /// without `by: system`). This server computes no prefill, so the form
    /// would show those fields empty without saying why.
    #[error(
        "the UI spec prefills {0:?} with `fill: {{from: ...}}`, which this server does not \
         compute; fill such a field by the system (`by: system`) or leave it to the applicant"
    )]
    Prefill(Vec<String>),
    /// The UI spec fills a field from a claim of the login that no login of
    /// this server keeps ([`KEPT_CLAIMS`]), so the field would stay empty on
    /// every channel.
    #[error(
        "the UI spec fills a field from the login claim(s) {0:?}; a login of this server keeps \
         only the identifier (the login field) and `naam`"
    )]
    LoginClaim(Vec<String>),
}

/// The claims of a login that a field of the UI spec can take with
/// `login: <claim>`: every login of this server (Keycloak, the test login,
/// FSC voor Burgers) keeps the identifier, which fills the login field, and
/// the browser logins the name. Nothing else.
pub const KEPT_CLAIMS: [&str; 1] = [NAME_CLAIM];

/// The claim of the login that holds the name of who logged in.
pub const NAME_CLAIM: &str = "naam";

impl From<regelrecht_cel::Error> for ServiceError {
    fn from(e: regelrecht_cel::Error) -> Self {
        use regelrecht_cel::Error;
        match e {
            Error::Refused(_) | Error::Stored(_) => Self::Refused(e.to_string()),
            // Mostly a value the request left out, which is the caller's to
            // give; it may also be a fault in a regelwerk, so it is logged.
            Error::Engine(_) => {
                tracing::warn!(error = %e, "the engine refused a step");
                Self::Refused(e.to_string())
            }
            Error::Setup(_) | Error::Io(_) => Self::Internal(e.to_string()),
        }
    }
}

impl From<StoreError> for ServiceError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}

/// The cells of the case as one step read them: per cell, in the order of
/// the case, its records.
struct Read {
    chronicles: Vec<StoredChronicle>,
}

impl Read {
    /// Per record the server recorded by an article of the executor's own
    /// policy ([`Channel::Policy`]), that article.
    fn by_policy(&self) -> BTreeMap<String, String> {
        self.chronicles
            .iter()
            .flat_map(|c| &c.grams)
            .filter(|g| g.recorded_via == Some(Channel::Policy))
            .filter_map(|g| Some((g.gram.id.clone(), g.recorded_by.clone()?)))
            .collect()
    }

    /// The generation the chronicles were read in; one read is all one.
    fn generation(&self) -> u64 {
        self.chronicles.first().map_or(0, |c| c.generation)
    }
}

/// What a step that records came to: its answer, the appends to commit,
/// and which of their records the executor's policy made ([`Commit::by_policy`]).
struct Ran {
    response: Value,
    appends: Vec<CellAppend>,
    by_policy: BTreeSet<String>,
    /// The generation the step read in.
    generation: u64,
}

/// The process of one case, on a store and a clock.
#[derive(Debug)]
pub struct Service {
    corpus: Arc<Corpus>,
    /// The cells of the case by their id, the case's own first, in the order
    /// `Corpus::herstellen` takes their chronicles.
    cells: Vec<String>,
    /// The field of the application that identifies the applicant, filled
    /// by the login ([`login_field`]).
    login_field: String,
    store: Arc<dyn ChronicleStore>,
    clock: Arc<dyn Clock>,
    link: LinkRole,
}

/// What a link with the servers of other cells makes of this one
/// ([`Service::with_link`]).
#[derive(Debug, Clone, Default)]
struct LinkRole {
    /// The external party this server works for, when it is not the
    /// executor's: its staff log in here and record its acts themselves.
    party: Option<String>,
    /// The cells whose own servers this one exchanges records with.
    linked: BTreeSet<String>,
}

/// A record of the cell this server works for that the server of the cell
/// `to` has not received yet ([`Service::outbound`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Outbound {
    pub to: String,
    pub gram: Gram,
}

/// The field of the case's application that the login fills: the case's
/// own login field (`inloggen.veld`), or else the one field its UI spec
/// has the system fill from the login (`fill: {by: system}` without a
/// source).
///
/// # Errors
///
/// [`ServiceError::LoginField`] when the UI spec names none or several.
fn login_field(corpus: &Corpus) -> Result<String, ServiceError> {
    if let Some(login) = &corpus.casus.inloggen {
        return Ok(login.veld.clone());
    }
    let mut fields = corpus
        .ui_spec
        .systeem
        .iter()
        .filter(|(_, fill)| matches!(fill, Systeemvulling::Inloggen))
        .map(|(field, _)| field);
    match (fields.next(), fields.next()) {
        (Some(field), None) => Ok(field.clone()),
        (None, _) => Err(ServiceError::LoginField(0)),
        (Some(_), Some(_)) => Err(ServiceError::LoginField(2 + fields.count())),
    }
}

/// An external party as the admin panel shows it ([`Service::external_parties`]).
#[derive(Debug, Clone, PartialEq)]
pub struct ExternalParty {
    /// The cell, by its recording actor.
    pub cell: String,
    /// The name a screen shows for it.
    pub name: String,
    /// The submissions in its chronicle that wait for its decision, each
    /// with the record to decide after when an earlier decision was revoked.
    pub open: Vec<Waiting>,
    /// Its decisions of which no other cell received a copy yet.
    pub unsent: Vec<Gram>,
    /// The requests of the executor it received and did not answer yet,
    /// each by its receipt in the party's chronicle.
    pub requests: Vec<OpenRequest>,
    /// The requests it answered, each with its answer, latest first: only on
    /// the party's own server ([`Service::party`]), where its staff look
    /// back at their own work. The executor's admin panel never gets them,
    /// since it shows of a party only what an input needs.
    pub answered: Vec<AnsweredRequest>,
}

/// A request an external party answered, as its own server shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct AnsweredRequest {
    /// Its receipt in the party's chronicle.
    pub received: Gram,
    /// The party's answer, in its own chronicle.
    pub answer: Gram,
}

/// A request of the executor that an external party received and did not
/// answer yet, as the admin panel shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct OpenRequest {
    /// Its receipt in the party's chronicle: what the request holds.
    pub received: Gram,
    /// The fact by which the party answers it, `<regelwerk>#<article>#<fact>`.
    pub answer: String,
}

/// A submission that waits for a decision of the cell whose chronicle holds
/// it: one not decided on yet, or one whose last decision a later record
/// revoked with nothing in its place (`stand::Stand::open`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Waiting {
    /// The submission.
    #[serde(rename = "stuk")]
    pub submission: Gram,
    /// The record the decision is to be taken after (`Decide::after`), when
    /// an earlier decision on it was revoked.
    #[serde(rename = "na", skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

impl From<stand::Open<'_, '_>> for Waiting {
    fn from(o: stand::Open<'_, '_>) -> Self {
        Self {
            submission: o.stuk.clone(),
            after: o.na.map(str::to_string),
        }
    }
}

/// The work queue of the executor ([`Service::work_queue`]).
#[derive(Debug, Clone, PartialEq)]
pub struct WorkQueue {
    /// The submissions that wait for its decision, in chronicle order.
    pub open: Vec<Waiting>,
}

/// The records of every cell that are about one person, as
/// [`Service::dossier`] gives them.
#[derive(Debug, Clone, PartialEq)]
pub struct Dossier {
    /// Per cell in the order of the case, its records about the person.
    pub grams: Vec<Gram>,
}

impl Dossier {
    /// Whether the record `id` is in it.
    pub fn holds(&self, id: &str) -> bool {
        self.grams.iter().any(|g| g.id == id)
    }
}

impl Service {
    /// Empty the test setup: every chronicle, the kept answers and the test
    /// clock, and record what the case begins with, as at a fresh start, in
    /// the same go ([`ChronicleStore::reset`]). The caller sees to it that
    /// this runs only in test mode.
    ///
    /// # Errors
    ///
    /// [`ServiceError::Store`] when the store fails; nothing is emptied then.
    pub async fn reset(&self) -> Result<(), ServiceError> {
        let begin = self.beginning().await?;
        Ok(self.store.reset(begin).await?)
    }

    /// The service for `corpus` on `store` and `clock`. When no cell of the
    /// case has a record yet, what the case begins with (its `begin`) is
    /// committed first, once, whichever instance gets there first.
    ///
    /// # Errors
    ///
    /// When the UI spec asks what this server does not do: no single login
    /// field ([`ServiceError::LoginField`]), a prefill the applicant may
    /// change ([`ServiceError::Prefill`]), or a claim of the login it does not
    /// keep ([`ServiceError::LoginClaim`]); when the store fails, or the
    /// case's beginning does not record.
    pub async fn start(
        corpus: Corpus,
        store: Arc<dyn ChronicleStore>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, ServiceError> {
        let cells = corpus.cellen().map(str::to_string).collect();
        let login_field = login_field(&corpus)?;
        if !corpus.ui_spec.prefill.is_empty() {
            return Err(ServiceError::Prefill(
                corpus.ui_spec.prefill.keys().cloned().collect(),
            ));
        }
        let unknown: Vec<String> = corpus
            .ui_spec
            .systeem
            .values()
            .filter_map(|fill| match fill {
                Systeemvulling::Inlogclaim(claim) if !KEPT_CLAIMS.contains(&claim.as_str()) => {
                    Some(claim.clone())
                }
                _ => None,
            })
            .collect();
        if !unknown.is_empty() {
            return Err(ServiceError::LoginClaim(unknown));
        }
        let service = Self {
            corpus: Arc::new(corpus),
            cells,
            login_field,
            store,
            clock,
            link: LinkRole::default(),
        };
        service.begin().await?;
        Ok(service)
    }

    async fn begin(&self) -> Result<(), ServiceError> {
        let read = self.read().await?;
        if read.chronicles.iter().any(|c| c.head() > 0) {
            return Ok(());
        }
        let appends = self.beginning().await?;
        if appends.is_empty() {
            return Ok(());
        }
        // No key: the expected heads of 0 already make sure the beginning is
        // recorded once, whichever instance gets there first.
        let commit = Commit {
            appends,
            keyed: None,
            recorded_by: None,
            by_policy: BTreeSet::new(),
            generation: read.generation(),
        };
        match self.store.append(commit).await {
            // Another instance began first, or a reset recorded the
            // beginning itself: what is there counts.
            Ok(_) | Err(StoreError::Conflict { .. } | StoreError::Reset { .. }) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// What the case begins with (its `begin`), per cell after a head of 0;
    /// nothing when it begins with no record at all.
    async fn beginning(&self) -> Result<Vec<CellAppend>, ServiceError> {
        let corpus = Arc::clone(&self.corpus);
        let begun = blocking(move || Ok(corpus.cell()?.kronieken())).await?;
        if begun.iter().all(Vec::is_empty) {
            return Ok(Vec::new());
        }
        Ok(self
            .cells
            .iter()
            .zip(begun)
            .map(|(cell, grams)| CellAppend {
                cell: cell.clone(),
                expected_head: 0,
                grams,
            })
            .collect())
    }

    /// The field of the case's application that identifies the applicant.
    pub fn login_field(&self) -> &str {
        &self.login_field
    }

    /// The field of the case's application that holds the applicant's name:
    /// the one the UI spec fills from the login's name ([`NAME_CLAIM`]).
    /// `None` when the spec names none.
    pub fn name_field(&self) -> Option<&str> {
        self.corpus
            .ui_spec
            .systeem
            .iter()
            .find(|(_, fill)| matches!(fill, Systeemvulling::Inlogclaim(c) if c == NAME_CLAIM))
            .map(|(field, _)| field.as_str())
    }

    /// The case's own title (`titel` in the case file), which the screens
    /// carry in their bar when the deployment names none.
    pub fn case_title(&self) -> &str {
        &self.corpus.casus.titel
    }

    /// The article of the case's application, which the login field
    /// belongs to.
    pub fn application_article(&self) -> &str {
        &self.corpus.casus.artikel
    }

    /// The fields of the case's application that the UI spec has the system
    /// fill, the login field among them: the server fills them, and a
    /// request may not.
    pub fn system_fields(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.login_field.as_str()).chain(
            self.corpus
                .ui_spec
                .systeem
                .keys()
                .map(String::as_str)
                .filter(|f| *f != self.login_field),
        )
    }

    /// What the system would fill in a submission under `article` of
    /// `submitter` today ([`kern::systeemvelden`]), for a form to show before
    /// it is submitted. Nothing is recorded; the submission fills it anew.
    ///
    /// # Errors
    ///
    /// When the store fails, or a source the UI spec names gives nothing.
    pub async fn system_preview(
        &self,
        article: String,
        submitter: Indiener,
    ) -> Result<Map<String, Value>, ServiceError> {
        let read = self.read().await?;
        let corpus = Arc::clone(&self.corpus);
        let today = self.today().await?;
        blocking(move || {
            let cellen = restore(&corpus, read)?;
            Ok(kern::systeemvelden(
                &corpus,
                &cellen,
                &article,
                None,
                &submitter,
                &Map::new(),
                today,
            )?)
        })
        .await
    }

    /// Today, by the clock of this service.
    ///
    /// # Errors
    ///
    /// When the clock keeps its day in the store and the store fails.
    pub async fn today(&self) -> Result<NaiveDate, ServiceError> {
        Ok(self.clock.today().await?)
    }

    /// The case as a screen shows it: its text and what follows from the
    /// law (`Corpus::beschrijving`), and today.
    ///
    /// # Errors
    ///
    /// As [`Service::today`].
    pub async fn case(&self) -> Result<Value, ServiceError> {
        let mut case = self.corpus.beschrijving();
        if let Some(map) = case.as_object_mut() {
            map.insert("vandaag".into(), json!(self.today().await?));
        }
        Ok(case)
    }

    /// The forms of the process today (`kern::vormen`): per submission its
    /// fields, with what the UI spec of the source says of them, and per
    /// decision what the authority finds.
    ///
    /// # Errors
    ///
    /// When the store fails or the cell cannot derive the forms.
    pub async fn forms(&self) -> Result<Value, ServiceError> {
        let read = self.read().await?;
        let corpus = Arc::clone(&self.corpus);
        let today = self.today().await?;
        blocking(move || {
            let cellen = restore(&corpus, read)?;
            Ok(kern::vormen(&corpus, &cellen, today)?)
        })
        .await
    }

    /// What the login screen of the case's login step shows today: the cell
    /// that checks, the service, the password of the simulation and the user
    /// names of the means of login that cell issued (`inlogstap::aanbod`).
    /// `None` for a case without a login step.
    ///
    /// # Errors
    ///
    /// When the store fails or the cell of the step is not of the case.
    pub async fn login_offer(&self) -> Result<Option<Value>, ServiceError> {
        if self.login_step().is_none() {
            return Ok(None);
        }
        let read = self.read().await?;
        let corpus = Arc::clone(&self.corpus);
        let today = self.today().await?;
        blocking(move || {
            let cellen = restore(&corpus, read)?;
            Ok(Some(inlogstap::aanbod(&corpus, &cellen, today)?))
        })
        .await
    }

    /// The login step of the case (`inloggen.inlogstap`), if it has one.
    pub fn login_step(&self) -> Option<&regelrecht_poc_chronolexografie::casus::Inlogstap> {
        self.corpus.casus.inloggen.as_ref()?.inlogstap.as_ref()
    }

    /// The login of the case (`inloggen`), if it has one.
    pub fn login(&self) -> Option<&regelrecht_poc_chronolexografie::casus::Inloggen> {
        self.corpus.casus.inloggen.as_ref()
    }

    /// The identities the case's begin gives for the login key `key`, such
    /// as the businesses of a Chamber of Commerce cell for `kvk_nummer`:
    /// each value of `key` in what any cell records before the first step,
    /// in the order of the case, with the first name a record beside it
    /// gives ([`NAME_FIELDS`]).
    pub fn begin_identities(&self, key: &str) -> Vec<(String, Option<String>)> {
        let casus = &self.corpus.casus;
        let begins = std::iter::once(casus.begin.as_ref())
            .chain(casus.andere_cellen.iter().map(|a| a.begin.as_ref()))
            .flatten();
        let mut out: Vec<(String, Option<String>)> = Vec::new();
        for record in begins.flat_map(|b| &b.executogrammen) {
            let Some(value) = record.velden.get(key).and_then(identity_text) else {
                continue;
            };
            let own = self.login().and_then(|l| l.naam.as_deref());
            let name = own
                .into_iter()
                .chain(NAME_FIELDS)
                .find_map(|f| record.velden.get(f).and_then(Value::as_str))
                .map(str::to_owned);
            match out.iter_mut().find(|(v, _)| *v == value) {
                Some((_, known)) => {
                    if known.is_none() {
                        *known = name;
                    }
                }
                None => out.push((value, name)),
            }
        }
        out
    }

    /// Log in through the case's login step as `gebruikersnaam` with
    /// `wachtwoord`, on today's date: the cell of the step answers whether
    /// that user may act for the service, and when it may, records the fact
    /// of the case's login, in a commit of its own
    /// (`inlogstap::inloggen`).
    ///
    /// # Errors
    ///
    /// What [`Service::perform`] refuses, such as a case without a login
    /// step.
    pub async fn log_in(
        &self,
        gebruikersnaam: &str,
        wachtwoord: &str,
    ) -> Result<LoginOutcome, ServiceError> {
        let actor = Actor {
            account: format!("inlogstap:{gebruikersnaam}"),
            channel: Channel::TestLogin,
        };
        let step = Step::LogIn(LogIn {
            gebruikersnaam: gebruikersnaam.to_owned(),
            wachtwoord: wachtwoord.to_owned(),
        });
        LoginOutcome::from_answer(&self.perform(None, step, &actor).await?)
    }

    /// The cell this server runs the process for, whose employees log in to
    /// the back office: the case's own cell (`cel` in the case file).
    pub fn executor(&self) -> &str {
        &self.corpus.casus.cel
    }

    /// The name a screen shows for the executor.
    pub fn executor_name(&self) -> &str {
        self.corpus.cel_naam(self.executor())
    }

    /// Whether `cell` is a cell of the case other than the executor: a
    /// party whose acts an admin enters in the admin panel.
    pub fn is_external(&self, cell: &str) -> bool {
        cell != self.executor() && self.cells.iter().any(|c| c == cell)
    }

    /// This service linked with the servers of the cells `linked`, working
    /// for the external party `party` when given and else for the executor.
    ///
    /// # Errors
    ///
    /// [`ServiceError::UnknownCell`] when `party` is no external party of
    /// the case, or a linked cell is no cell of the case or is the cell
    /// this server works for itself.
    pub fn with_link(
        mut self,
        party: Option<&str>,
        linked: impl IntoIterator<Item = String>,
    ) -> Result<Self, ServiceError> {
        if let Some(party) = party.filter(|p| !self.is_external(p)) {
            return Err(ServiceError::UnknownCell(party.to_string()));
        }
        let own = party.unwrap_or(self.executor()).to_string();
        let linked: BTreeSet<String> = linked.into_iter().collect();
        if let Some(cell) = linked
            .iter()
            .find(|c| **c == own || !self.cells.contains(*c))
        {
            return Err(ServiceError::UnknownCell(cell.clone()));
        }
        self.link = LinkRole {
            party: party.map(str::to_string),
            linked,
        };
        Ok(self)
    }

    /// The external party this server works for, when it is not the
    /// executor's own server ([`Service::with_link`]).
    pub fn party(&self) -> Option<&str> {
        self.link.party.as_deref()
    }

    /// The cell whose records this server makes itself and sends over its
    /// links: the party it works for, or else the executor.
    pub fn own_cell(&self) -> &str {
        self.party().unwrap_or(self.executor())
    }

    /// Whether the cell `cell` has a server of its own this one is linked
    /// with.
    pub fn is_linked(&self, cell: &str) -> bool {
        self.link.linked.contains(cell)
    }

    /// The external parties the executor asks something of by its own
    /// policy (`verzoek::gevraagde_cellen`), today: the parties that do work
    /// of their own in the process, which a deployment may give a server of
    /// their own.
    ///
    /// # Errors
    ///
    /// When the clock fails or the kern cannot read an article of the policy.
    pub async fn asked_parties(&self) -> Result<Vec<String>, ServiceError> {
        let corpus = Arc::clone(&self.corpus);
        let today = self.today().await?;
        blocking(move || {
            let cellen = corpus.cell()?;
            Ok(verzoek::gevraagde_cellen(&corpus, &cellen, today)?)
        })
        .await
    }

    /// The cells this server is linked with, in their own order.
    pub fn linked_cells(&self) -> impl Iterator<Item = &str> {
        self.link.linked.iter().map(String::as_str)
    }

    /// Whether a person on this server enters what the external party
    /// `cell` did: on the party's own server only that party, and on the
    /// executor's every party without a server of its own.
    pub fn enters_for(&self, cell: &str) -> bool {
        match self.party() {
            Some(party) => party == cell,
            None => self.is_external(cell) && !self.is_linked(cell),
        }
    }

    /// The records of this server's own cell that a linked cell records a
    /// receipt of (`kern::ontvangers`) and whose server has not confirmed
    /// them yet, in the order of the chronicle. The chronicle is the
    /// outbox: nothing else is kept of a record still to send.
    ///
    /// A record that came about over a link is never sent: the receipt this
    /// cell made of a linked cell's fact is the other half of that same
    /// message, and sending it would bring the fact back to its sender.
    ///
    /// # Errors
    ///
    /// When the store fails or the kern cannot read a record's article.
    pub async fn outbound(&self) -> Result<Vec<Outbound>, ServiceError> {
        if self.link.linked.is_empty() {
            return Ok(Vec::new());
        }
        let read = self.read().await?;
        let corpus = Arc::clone(&self.corpus);
        let own = self.own_cell().to_string();
        let linked = self.link.linked.clone();
        let over_a_link: BTreeSet<String> = read
            .chronicles
            .iter()
            .flat_map(|c| &c.grams)
            .filter(|g| g.recorded_via == Some(crate::store::Channel::Link))
            .map(|g| g.gram.id.clone())
            .collect();
        let candidates = blocking(move || {
            let cellen = restore(&corpus, read)?;
            let engine = corpus.service()?;
            let (cell, _) = cellen.voor(&own)?;
            let mut out = Vec::new();
            let sent_from_here = cell
                .grams()
                .iter()
                .filter(|g| g.recording_actor == own && !over_a_link.contains(&g.id));
            for gram in sent_from_here {
                for to in kern::ontvangers(&engine, &cellen, gram)? {
                    if linked.contains(&to) {
                        out.push(Outbound {
                            to,
                            gram: gram.clone(),
                        });
                    }
                }
            }
            Ok(out)
        })
        .await?;
        let mut delivered: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for to in &self.link.linked {
            delivered.insert(to.clone(), self.store.delivered(to).await?);
        }
        Ok(candidates
            .into_iter()
            .filter(|o| !delivered.get(&o.to).is_some_and(|d| d.contains(&o.gram.id)))
            .collect())
    }

    /// Keep that the server of the cell `to` confirmed the record `gram`.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn confirm_delivery(&self, gram: &str, to: &str) -> Result<(), ServiceError> {
        Ok(self.store.mark_delivered(gram, to).await?)
    }

    /// Take over the record `id` that the server of the linked cell `from`
    /// made and sent: that cell records the fact here too, on the day it
    /// recorded it there, and what the fact sends this server's own cell
    /// arrives in the same commit. A record taken over before is answered
    /// as it was then and recorded once, under a key made of the cell and
    /// the record's id.
    ///
    /// With `follow_clock`, in test mode, the test clock then moves on to
    /// the record's day when the sender's clock is ahead: each server of a
    /// test setup has a clock of its own, and the next step here must not
    /// fall before the record just taken over.
    ///
    /// # Errors
    ///
    /// [`ServiceError::UnknownCell`] when `from` is no linked cell, and
    /// what [`Service::perform`] refuses.
    pub async fn take_over(
        &self,
        from: &str,
        id: &str,
        step: TakeOver,
        follow_clock: bool,
    ) -> Result<Value, ServiceError> {
        if !self.is_linked(from) || step.cell != from {
            return Err(ServiceError::UnknownCell(from.to_string()));
        }
        let key: IdempotencyKey = format!("koppeling:{from}:{id}")
            .parse()
            .map_err(ServiceError::Refused)?;
        let on = step.on;
        let actor = Actor {
            account: from.to_string(),
            channel: crate::store::Channel::Link,
        };
        let answer = self
            .perform(Some(key), Step::TakeOver(step), &actor)
            .await?;
        if follow_clock && on > self.today().await? {
            self.store.set_test_day(on).await?;
        }
        Ok(answer)
    }

    /// Per external party, in the order of the case, what the admin panel
    /// needs to enter its acts: the submissions that wait for its decision
    /// and its decisions of which no copy was received yet. Not its
    /// chronicle: what else the party recorded stays out of view.
    ///
    /// # Errors
    ///
    /// When the store fails, or the kern cannot read a record's article.
    pub async fn external_parties(&self) -> Result<Vec<ExternalParty>, ServiceError> {
        let requests = self.requests().await?;
        let (chronicles, stands) = self.stands().await?;
        let afleiding = &self.corpus.afleiding;
        let chronicles: Vec<(&str, Vec<Gram>, Stand)> = self
            .cells
            .iter()
            .map(String::as_str)
            .zip(chronicles)
            .zip(stands)
            .map(|((cell, grams), stand)| (cell, grams, stand))
            .collect();
        let parties = chronicles
            .iter()
            .filter(|(cell, _, _)| self.enters_for(cell))
            .map(|(cell, grams, stand)| {
                let elsewhere: Vec<&Gram> = chronicles
                    .iter()
                    .filter(|(other, _, _)| other != cell)
                    .flat_map(|(_, g, _)| g)
                    .collect();
                ExternalParty {
                    cell: (*cell).to_string(),
                    name: self.corpus.cel_naam(cell).to_string(),
                    open: stand.open(grams).into_iter().map(Waiting::from).collect(),
                    unsent: external::unsent(grams, &elsewhere, stand, afleiding)
                        .into_iter()
                        .cloned()
                        .collect(),
                    requests: requests
                        .iter()
                        .filter(|r| r.aan == *cell && r.open())
                        .filter_map(|r| {
                            Some(OpenRequest {
                                received: r.ontvangen.clone()?,
                                answer: r.antwoordfeit.clone()?,
                            })
                        })
                        .collect(),
                    answered: requests
                        .iter()
                        .rev()
                        .filter(|r| r.aan == *cell && self.party() == Some(*cell))
                        .filter_map(|r| {
                            Some(AnsweredRequest {
                                received: r.ontvangen.clone()?,
                                answer: r.antwoord.clone()?,
                            })
                        })
                        .collect(),
                }
            })
            .collect();
        Ok(parties)
    }

    /// The requests the executor made by its policy (`verzoek::verzoeken`),
    /// each with its receipt, the answer and the receipt of the answer, as
    /// the chronicles hold them now. The caller decides what of another
    /// cell's records it shows.
    ///
    /// # Errors
    ///
    /// When the store fails or the corpus does not load.
    pub async fn requests(&self) -> Result<Vec<Verzoek>, ServiceError> {
        let read = self.read().await?;
        let corpus = Arc::clone(&self.corpus);
        blocking(move || {
            let cellen = restore(&corpus, read)?;
            Ok(verzoek::verzoeken(&corpus, &cellen)?)
        })
        .await
    }

    /// What waits for a decision of the executor: its submissions not
    /// decided on yet, and those whose last decision a later record revoked
    /// with nothing in its place, such as an objection whose decision a
    /// court annulled; that one is to be decided again after the receipt of
    /// the judgment (`na`).
    ///
    /// # Errors
    ///
    /// When the store fails, or the kern cannot read a record's article.
    pub async fn work_queue(&self) -> Result<WorkQueue, ServiceError> {
        let (chronicles, stands) = self.stands().await?;
        let position = self
            .cells
            .iter()
            .position(|c| c == self.executor())
            .ok_or_else(|| ServiceError::Internal("the executor is no cell of the case".into()))?;
        let (Some(grams), Some(stand)) = (chronicles.get(position), stands.get(position)) else {
            return Err(ServiceError::Internal(
                "the store gave no chronicle for the executor".into(),
            ));
        };
        let open = stand.open(grams).into_iter().map(Waiting::from).collect();
        Ok(WorkQueue { open })
    }

    /// The timeline of the application the record `id` belongs to, from one
    /// read of every chronicle ([`crate::timeline::derive`]), not yet kept
    /// to what a reader may see: the caller does that
    /// ([`crate::timeline::Timeline::within`]).
    ///
    /// `None` when no cell holds `id`.
    ///
    /// # Errors
    ///
    /// When the store fails or the kern cannot read the chronicles.
    pub async fn timeline(
        &self,
        id: &str,
    ) -> Result<Option<crate::timeline::Timeline>, ServiceError> {
        let read = self.read().await?;
        let by_policy = read.by_policy();
        let corpus = Arc::clone(&self.corpus);
        let cells = self.cells.clone();
        let today = self.today().await?;
        let id = id.to_string();
        blocking(move || {
            let cellen = restore(&corpus, read)?;
            let chronicles = cellen.kronieken();
            let stands = stands_of(&corpus, &chronicles)?;
            let requests = verzoek::verzoeken(&corpus, &cellen)?;
            let grants = vervolg::standen(&corpus, &cellen, today)?;
            let deadlines = kern::termijnvelden(&corpus, today)?;
            let per_cell: Vec<(String, Vec<Gram>, Stand)> = cells
                .into_iter()
                .zip(chronicles)
                .zip(stands)
                .map(|((c, g), s)| (c, g, s))
                .collect();
            let sources = crate::timeline::Sources {
                cells: &per_cell,
                executor: &corpus.casus.cel,
                requests: &requests,
                grants: &grants,
                after_grant: corpus.casus.na_toekenning.as_ref(),
                afleiding: &corpus.afleiding,
                deadlines: &deadlines,
                by_policy: &by_policy,
                today,
            };
            Ok(crate::timeline::derive(&sources, &id))
        })
        .await
    }

    /// Every cell's chronicle, in the order of the case, with what the kern
    /// says of it ([`stand::stand`]).
    async fn stands(&self) -> Result<(Vec<Vec<Gram>>, Vec<Stand>), ServiceError> {
        let read = self.read().await?;
        let corpus = Arc::clone(&self.corpus);
        blocking(move || {
            let chronicles: Vec<Vec<Gram>> = read
                .chronicles
                .into_iter()
                .map(StoredChronicle::into_grams)
                .collect();
            let stands = stands_of(&corpus, &chronicles)?;
            Ok((chronicles, stands))
        })
        .await
    }

    /// The facts by which an external party answers a request of the
    /// executor, such as the GGD's advice: those the case file names as the
    /// answer to a request (`bij_aanvraag`). With a request open, the party
    /// records such a fact from that request.
    pub fn answer_facts(&self) -> Vec<&str> {
        self.corpus
            .casus
            .bij_aanvraag
            .iter()
            .filter_map(|o| o.antwoord.as_ref())
            .map(|a| a.feit.as_str())
            .collect()
    }

    /// The name a screen shows for the cell `cell` of the case.
    pub fn cell_name<'a>(&'a self, cell: &'a str) -> &'a str {
        self.corpus.cel_naam(cell)
    }

    /// Why the decision `id` came out as it did (`kern::waarom`): who filled
    /// each open term, and where each answer of a cell came from. Derived
    /// from the chronicles as they are now; nothing is recorded.
    ///
    /// # Errors
    ///
    /// [`ServiceError::Refused`] when no cell holds `id` or it is no
    /// decision, and when the store fails.
    pub async fn explain(&self, id: &str) -> Result<Value, ServiceError> {
        let read = self.read().await?;
        let corpus = Arc::clone(&self.corpus);
        let id = id.to_string();
        blocking(move || {
            let cellen = restore(&corpus, read)?;
            let waarom = kern::waarom(&corpus, &cellen, &id)?;
            Ok(to_value(&waarom)?)
        })
        .await
    }

    /// What the decision on the submission `d` would come to today, before
    /// it is taken (`kern::voorvertoning`): the outcome or the cell's
    /// refusal, and the findings a policy leaves to the authority for this
    /// case, and what the system would then do by itself (`automatisch`,
    /// [`vervolg::zou_automatisch`]). Taken on a copy of the cells; nothing
    /// is recorded.
    ///
    /// # Errors
    ///
    /// When the store fails, and what the kern fails on besides a refusal of
    /// the cell, which the preview carries as `geweigerd`.
    pub async fn preview(&self, d: Decide) -> Result<Value, ServiceError> {
        let read = self.read().await?;
        let corpus = Arc::clone(&self.corpus);
        let today = self.today().await?;
        blocking(move || {
            let cellen = restore(&corpus, read)?;
            let preview = kern::voorvertoning(
                &corpus,
                &cellen,
                &d.submission,
                d.after.as_deref(),
                &d.findings,
                today,
            )?;
            let mut preview = to_value(&preview)?;
            // What the system would do by itself once this decision is taken,
            // such as ordering the card it grants: on a copy, nothing lands.
            // A preview that cannot say so still shows the outcome.
            let mut proef = cellen.sandbox();
            let automatisch = vervolg::standen(&corpus, &cellen, today)
                .and_then(|voor| {
                    kern::beslissen(
                        &corpus,
                        &mut proef,
                        &d.submission,
                        d.after.as_deref(),
                        &d.findings,
                        today,
                    )?;
                    vervolg::zou_automatisch(&corpus, &proef, &voor, today)
                })
                .unwrap_or_else(|e| {
                    tracing::debug!(error = %e, "no automatic acts to preview");
                    Vec::new()
                });
            if let Some(map) = preview.as_object_mut() {
                map.insert("automatisch".into(), Value::Array(automatisch));
            }
            Ok(preview)
        })
        .await
    }

    /// The chronicle of the cell `cell`: its records, its head, and what the
    /// kern says of them ([`stand::stand`]).
    ///
    /// # Errors
    ///
    /// When the case has no such cell, the store fails, or the kern cannot
    /// read a record's article.
    pub async fn chronicle(&self, cell: &str) -> Result<Value, ServiceError> {
        if !self.cells.iter().any(|c| c == cell) {
            return Err(ServiceError::UnknownCell(cell.to_string()));
        }
        let stored = self.store.load(cell).await?;
        let head = stored.head();
        let grams = stored.into_grams();
        let corpus = Arc::clone(&self.corpus);
        let (grams, stand) = blocking(move || {
            let stand = stand::stand(&corpus.service()?, &grams)?;
            Ok((grams, stand))
        })
        .await?;
        Ok(json!({
            "cel": cell,
            "cel_naam": self.corpus.cel_naam(cell),
            "kop": head,
            "grammen": grams,
            "stand": stand,
        }))
    }

    /// Per record the server recorded by an article of the executor's own
    /// policy, without a person ([`Channel::Policy`]), that article: the
    /// request to the GGD an application brings about, the order of a card
    /// once a grant counts. A screen says of such a record that the system
    /// made it, and by which article.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn by_policy(&self) -> Result<BTreeMap<String, String>, ServiceError> {
        Ok(self.read().await?.by_policy())
    }

    /// Every grant of the executor today, with how far the acts that follow
    /// it are ([`vervolg::standen`]). Empty when the case names no such
    /// acts.
    ///
    /// # Errors
    ///
    /// When the store fails or the aggregate does not play.
    pub async fn standen(&self) -> Result<Vec<vervolg::Stand>, ServiceError> {
        let read = self.read().await?;
        let corpus = Arc::clone(&self.corpus);
        let today = self.today().await?;
        blocking(move || {
            let cellen = restore(&corpus, read)?;
            Ok(vervolg::standen(&corpus, &cellen, today)?)
        })
        .await
    }

    /// The records of every cell about the person `subject` identifies in
    /// the login field ([`crate::dossier`]), derived from the chronicles as
    /// they are now.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn dossier(&self, subject: &str) -> Result<Dossier, ServiceError> {
        let read = self.read().await?;
        let all: Vec<&Gram> = read
            .chronicles
            .iter()
            .flat_map(|c| c.grams.iter().map(|g| &g.gram))
            .collect();
        let ids = crate::dossier::ids(&all, &self.login_field, subject);
        let grams = all
            .into_iter()
            .filter(|g| ids.contains(g.id.as_str()))
            .cloned()
            .collect();
        Ok(Dossier { grams })
    }

    /// [`Service::dossier`], with what the kern says of its records
    /// ([`Stand`]): what each is, which are revoked, and what answers each
    /// submission now. Read from the whole chronicles, kept to the records
    /// of the dossier ([`Stand::binnen`]): an entry that would name a record
    /// outside it is left out. For the citizen's screen; an access check needs only
    /// [`Service::dossier`].
    ///
    /// # Errors
    ///
    /// When the store fails, or the kern cannot read a record's article.
    pub async fn dossier_with_stand(
        &self,
        subject: &str,
    ) -> Result<(Dossier, Stand), ServiceError> {
        let (chronicles, stands) = self.stands().await?;
        let all: Vec<&Gram> = chronicles.iter().flatten().collect();
        let ids = crate::dossier::ids(&all, &self.login_field, subject);
        let grams = all
            .into_iter()
            .filter(|g| ids.contains(g.id.as_str()))
            .cloned()
            .collect();
        // Of each chronicle's standing, what is about the dossier and names
        // nothing outside it: no id of another person's record, or of a
        // record of an external party that did not reach the dossier.
        let mut stand = Stand::default();
        for s in stands {
            let s = s.binnen(|id| ids.contains(id));
            stand.rollen.extend(s.rollen);
            stand.herroepen.extend(s.herroepen);
            stand.antwoorden.extend(s.antwoorden);
        }
        Ok((Dossier { grams }, stand))
    }

    /// The ids of the records an employee of the executor may see: those
    /// of its chronicle, and those of another cell that reached it, as what
    /// a record of its received (`received`).
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn seen_by_executor(
        &self,
    ) -> Result<std::collections::BTreeSet<String>, ServiceError> {
        let stored = self.store.load(self.executor()).await?;
        Ok(stored
            .into_grams()
            .into_iter()
            .flat_map(|g| std::iter::once(g.id).chain(g.received.map(|r| r.id)))
            .collect())
    }

    /// The ids of every record in the cells for which `include` holds.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn record_ids(
        &self,
        include: impl Fn(&str) -> bool,
    ) -> Result<std::collections::BTreeSet<String>, ServiceError> {
        let read = self.read().await?;
        Ok(self
            .cells
            .iter()
            .zip(read.chronicles)
            .filter(|(cell, _)| include(cell))
            .flat_map(|(_, c)| c.grams.into_iter().map(|g| g.gram.id))
            .collect())
    }

    /// The cell whose chronicle holds the record `id`, if one does.
    ///
    /// # Errors
    ///
    /// When the store fails.
    pub async fn holder(&self, id: &str) -> Result<Option<String>, ServiceError> {
        let read = self.read().await?;
        Ok(self
            .cells
            .iter()
            .zip(&read.chronicles)
            .find(|(_, c)| c.grams.iter().any(|g| g.gram.id == id))
            .map(|(cell, _)| cell.clone()))
    }

    /// Run `step` for `actor` (an account and the channel it came in by) and
    /// commit what it recorded, with `actor` beside each record. Under a `key` the same
    /// account used before for the same request it answers as it did then
    /// and records nothing; under a key another account used, or used for
    /// another request, it refuses. Without a key every call records.
    ///
    /// # Errors
    ///
    /// [`ServiceError::Refused`] when the cell refuses the step,
    /// [`ServiceError::KeyReused`] when the key answered another request,
    /// [`ServiceError::Busy`] after [`MAX_ATTEMPTS`] conflicts, and the
    /// store's and the service's own failures.
    pub async fn perform(
        &self,
        key: Option<IdempotencyKey>,
        step: Step,
        actor: &Actor,
    ) -> Result<Value, ServiceError> {
        // Who asked is part of the request a receipt keeps, so a key replays
        // only for the account that used it.
        let step_value = serde_json::to_value(&step)
            .map_err(|e| ServiceError::Internal(format!("the request does not serialize: {e}")))?;
        let request = json!({ "door": actor.account, "verzoek": step_value });
        if holds_nul(&request) {
            // PostgreSQL's jsonb cannot hold U+0000; refused for every store
            // alike, so the store in memory does not accept what Postgres
            // would fail on.
            return Err(ServiceError::Refused(
                "the request holds a NUL character (U+0000), which is not stored".into(),
            ));
        }
        if let Some(key) = &key
            && let Some(receipt) = self.store.receipt(key).await?
        {
            return replay(key, &request, receipt);
        }
        let step = Arc::new(step);
        for _ in 0..MAX_ATTEMPTS {
            let ran = self.run(Arc::clone(&step)).await?;
            let commit = Commit {
                appends: ran.appends,
                keyed: key.clone().map(|key| Keyed {
                    key,
                    receipt: Receipt {
                        request: request.clone(),
                        response: ran.response.clone(),
                    },
                }),
                recorded_by: Some(actor.clone()),
                by_policy: ran.by_policy,
                generation: ran.generation,
            };
            match self.store.append(commit).await {
                Ok(Appended::Committed) => return Ok(ran.response),
                Ok(Appended::Replayed(receipt)) => {
                    // Only a keyed commit is replayed.
                    let key = key.as_ref().ok_or_else(|| {
                        ServiceError::Internal("a commit without a key was replayed".into())
                    })?;
                    return replay(key, &request, receipt);
                }
                Err(StoreError::Conflict { cell, .. }) => {
                    tracing::debug!(%cell, "conflict; running the step again");
                }
                // The chronicles it read are gone: run it on what a reset
                // left, as on a fresh start.
                Err(StoreError::Reset { .. }) => {
                    tracing::debug!("reset since the read; running the step again");
                }
                Err(e) => return Err(e.into()),
            }
        }
        Err(ServiceError::Busy)
    }

    /// Run `step` on the chronicles as they are now, and say what it
    /// recorded in each cell.
    async fn run(&self, step: Arc<Step>) -> Result<Ran, ServiceError> {
        let read = self.read().await?;
        let generation = read.generation();
        let corpus = Arc::clone(&self.corpus);
        let cells = self.cells.clone();
        let today = self.today().await?;
        blocking(move || {
            let before: Vec<Vec<String>> = read
                .chronicles
                .iter()
                .map(|c| c.grams.iter().map(|g| g.gram.id.clone()).collect())
                .collect();
            let mut cellen = restore(&corpus, read)?;
            let applied = step.apply(&corpus, &mut cellen, today)?;
            let appends = appended(cells, before, cellen.kronieken())?;
            Ok(Ran {
                response: applied.response,
                appends,
                by_policy: applied.by_policy.into_iter().collect(),
                generation,
            })
        })
        .await
    }

    /// Every cell's chronicle, in the order of the case, at one moment.
    async fn read(&self) -> Result<Read, ServiceError> {
        let cells: Vec<&str> = self.cells.iter().map(String::as_str).collect();
        let chronicles = self.store.load_all(&cells).await?;
        if chronicles.len() != cells.len() {
            return Err(ServiceError::Internal(format!(
                "the store gave {} chronicles for {} cells",
                chronicles.len(),
                cells.len()
            )));
        }
        if chronicles
            .windows(2)
            .any(|pair| pair[0].generation != pair[1].generation)
        {
            return Err(ServiceError::Internal(
                "the store gave chronicles of more than one generation".into(),
            ));
        }
        Ok(Read { chronicles })
    }
}

/// What the kern says of each of `chronicles` ([`stand::stand`]), in their
/// order.
fn stands_of(corpus: &Corpus, chronicles: &[Vec<Gram>]) -> Result<Vec<Stand>, ServiceError> {
    let service = corpus.service()?;
    chronicles
        .iter()
        .map(|grams| Ok(stand::stand(&service, grams)?))
        .collect()
}

/// The cells of the case with the chronicles `read` holds.
fn restore(corpus: &Corpus, read: Read) -> Result<Cellen, ServiceError> {
    Ok(corpus.herstellen(
        read.chronicles
            .into_iter()
            .map(StoredChronicle::into_grams)
            .collect(),
    )?)
}

/// Per cell what the step appended after the records `before` held, as a
/// commit takes it. A chronicle is append-only: one that lost or changed a
/// record it was read with is a bug, not a request to store.
fn appended(
    cells: Vec<String>,
    before: Vec<Vec<String>>,
    after: Vec<Vec<Gram>>,
) -> Result<Vec<CellAppend>, ServiceError> {
    if before.len() != cells.len() || after.len() != cells.len() {
        return Err(ServiceError::Internal(format!(
            "{} cells, read as {} chronicles and left as {}",
            cells.len(),
            before.len(),
            after.len()
        )));
    }
    cells
        .into_iter()
        .zip(before)
        .zip(after)
        .map(|((cell, before), mut after)| {
            let kept = after.len() >= before.len()
                && after.iter().zip(&before).all(|(gram, id)| &gram.id == id);
            if !kept {
                return Err(ServiceError::Internal(format!(
                    "the chronicle of '{cell}' changed a record it was read with"
                )));
            }
            let grams = after.split_off(before.len());
            Ok(CellAppend {
                cell,
                expected_head: before.len() as u64,
                grams,
            })
        })
        .collect()
}

/// The answer to a request under a key used before: the same answer when it
/// is the same request.
fn replay(key: &IdempotencyKey, request: &Value, receipt: Receipt) -> Result<Value, ServiceError> {
    // Two JSON values compare by content: serde_json's maps are ordered by
    // key (no `preserve_order` in this workspace), so the order a client
    // sent its fields in does not count.
    if &receipt.request == request {
        Ok(receipt.response)
    } else {
        Err(ServiceError::KeyReused(key.as_str().to_string()))
    }
}

/// Whether a string or key anywhere in `value` holds U+0000.
fn holds_nul(value: &Value) -> bool {
    match value {
        Value::String(s) => s.contains('\0'),
        Value::Array(items) => items.iter().any(holds_nul),
        Value::Object(map) => map.iter().any(|(k, v)| k.contains('\0') || holds_nul(v)),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

/// Run `work` off the async runtime: a step builds an engine service and
/// executes regelwerken, which is CPU work.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ServiceError> + Send + 'static,
) -> Result<T, ServiceError> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| ServiceError::Internal(format!("the step did not finish: {e}")))?
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod tests {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use async_trait::async_trait;

    use super::*;
    use crate::clock::ControllableClock;
    use crate::store::memory::InMemoryStore;

    /// The store in memory, refusing the first `conflicts` commits as if
    /// another request got in first. With `reset_first` the next commit
    /// finds a reset just before it, as when an admin empties the test
    /// setup while a step runs. With `intrude` a record lands right after
    /// every reset, as a submission that comes in at that moment.
    #[derive(Debug, Default)]
    struct Contested {
        inner: InMemoryStore,
        conflicts: usize,
        appends: AtomicUsize,
        reset_first: AtomicBool,
        intrude: bool,
    }

    #[async_trait]
    impl ChronicleStore for Contested {
        async fn load(&self, cell: &str) -> Result<StoredChronicle, StoreError> {
            self.inner.load(cell).await
        }
        async fn load_all(&self, cells: &[&str]) -> Result<Vec<StoredChronicle>, StoreError> {
            self.inner.load_all(cells).await
        }
        async fn receipt(&self, key: &IdempotencyKey) -> Result<Option<Receipt>, StoreError> {
            self.inner.receipt(key).await
        }
        async fn test_day(&self) -> Result<Option<NaiveDate>, StoreError> {
            self.inner.test_day().await
        }
        async fn set_test_day(&self, day: NaiveDate) -> Result<(), StoreError> {
            self.inner.set_test_day(day).await
        }
        async fn delivered(&self, to: &str) -> Result<BTreeSet<String>, StoreError> {
            self.inner.delivered(to).await
        }
        async fn mark_delivered(&self, gram: &str, to: &str) -> Result<(), StoreError> {
            self.inner.mark_delivered(gram, to).await
        }
        async fn reset(&self, begin: Vec<CellAppend>) -> Result<(), StoreError> {
            let cell = begin
                .iter()
                .find(|a| !a.grams.is_empty())
                .map(|a| a.cell.clone());
            self.inner.reset(begin).await?;
            if let Some(cell) = cell.filter(|_| self.intrude) {
                let now = self.inner.load(&cell).await?;
                let mut gram = now.grams[0].gram.clone();
                gram.id = "binnengekomen-na-de-reset".into();
                let commit = Commit {
                    appends: vec![CellAppend {
                        cell,
                        expected_head: now.head(),
                        grams: vec![gram],
                    }],
                    keyed: None,
                    recorded_by: None,
                    by_policy: BTreeSet::new(),
                    generation: now.generation,
                };
                self.inner.append(commit).await?;
            }
            Ok(())
        }
        async fn append(&self, commit: Commit) -> Result<Appended, StoreError> {
            if self.reset_first.swap(false, Ordering::SeqCst) {
                self.inner.reset(Vec::new()).await?;
            }
            if self.appends.fetch_add(1, Ordering::SeqCst) < self.conflicts {
                return Err(StoreError::Conflict {
                    cell: "x".into(),
                    expected: 0,
                    actual: 1,
                });
            }
            self.inner.append(commit).await
        }
    }

    async fn service(conflicts: usize) -> (Service, Arc<Contested>) {
        let (corpus, _) =
            crate::case::load(std::path::Path::new(crate::config::DEFAULT_MANIFEST)).unwrap();
        let store = Arc::new(Contested {
            conflicts,
            ..Contested::default()
        });
        let clock = Arc::new(ControllableClock::new(
            Arc::clone(&store) as _,
            corpus.casus.speeldatum,
        ));
        let s = Service::start(corpus, Arc::clone(&store) as _, clock)
            .await
            .unwrap();
        (s, store)
    }

    const GEMEENTE: &str = "gemeente_wilgendaal";
    const GGD: &str = "ggd_wilgendaal";
    const ADVICE: &str =
        "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_uitgebracht";

    /// A service on a store of its own, linked as [`Service::with_link`]
    /// takes it.
    async fn linked(party: Option<&str>, with: &str) -> Service {
        let (s, _) = service(0).await;
        s.with_link(party, [with.to_string()]).unwrap()
    }

    /// Carry every record `from` still has to send over to `to`, as the
    /// link does, and confirm each.
    async fn carry(from: &Service, to: &Service) -> usize {
        let outbound = from.outbound().await.unwrap();
        for o in &outbound {
            let step = TakeOver {
                cell: from.own_cell().into(),
                fact: format!("{}#{}", o.gram.basis, o.gram.fact.as_deref().unwrap()),
                fields: o.gram.fields.clone().into_iter().collect(),
                on: o.gram.recorded_on,
            };
            to.take_over(from.own_cell(), &o.gram.id, step, true)
                .await
                .unwrap();
            from.confirm_delivery(&o.gram.id, &o.to).await.unwrap();
        }
        outbound.len()
    }

    #[tokio::test]
    async fn a_request_and_its_answer_cross_between_two_servers() {
        let gemeente = linked(None, GGD).await;
        let ggd = linked(Some(GGD), GEMEENTE).await;
        // The application at the municipality brings the request about; its
        // receipt at the GGD lands in the municipality's own copy of that
        // cell, and the request waits to be sent.
        gemeente
            .perform(None, submission(), &actor())
            .await
            .unwrap();
        assert!(gemeente.requests().await.unwrap()[0].open());
        assert!(ggd.requests().await.unwrap().is_empty());
        assert_eq!(carry(&gemeente, &ggd).await, 1);
        assert_eq!(
            carry(&gemeente, &ggd).await,
            0,
            "a confirmed record is sent once"
        );

        // The GGD's own server now holds the request, and only the GGD is
        // entered for there.
        let parties = ggd.external_parties().await.unwrap();
        assert_eq!(parties.len(), 1);
        assert_eq!(parties[0].cell, GGD);
        let received = parties[0].requests[0].received.clone();
        assert!(
            gemeente
                .external_parties()
                .await
                .unwrap()
                .iter()
                .all(|p| p.cell != GGD),
            "the municipality's admin enters nothing for a linked party"
        );

        // The GGD answers on its own server; the advice crosses back and
        // closes the request at the municipality.
        let advice = Step::Record(Record {
            cell: GGD.into(),
            fact: ADVICE.into(),
            fields: serde_json::from_value(json!({
                "datum_onderzoek": "2026-10-16",
                "langdurige_loopbeperking": true,
                "continu_afhankelijk_van_hulp_bestuurder": false,
                "permanent_rolstoelgebonden": false,
                "ernstige_beperkingen_andere_dan_loopbeperkingen": false,
            }))
            .unwrap(),
            refers_to: None,
            request: Some(received.id),
        });
        ggd.store
            .set_test_day("2026-10-16".parse().unwrap())
            .await
            .unwrap();
        ggd.perform(None, advice, &actor()).await.unwrap();
        assert!(gemeente.requests().await.unwrap()[0].open());
        assert_eq!(carry(&ggd, &gemeente).await, 1);
        let request = &gemeente.requests().await.unwrap()[0];
        assert!(!request.open());
        assert!(request.antwoord_ontvangen.is_some());
        // The advice stands at the municipality on the day the GGD recorded
        // it, though the municipality's own clock was still behind.
        let advice = gemeente.store.load(GGD).await.unwrap();
        assert_eq!(
            advice.grams.last().unwrap().gram.recorded_on,
            "2026-10-16".parse::<NaiveDate>().unwrap()
        );
        // The municipality's clock followed the GGD's.
        assert_eq!(
            gemeente.today().await.unwrap(),
            "2026-10-16".parse().unwrap()
        );
        // What came over the link says so beside the record.
        let chronicle = gemeente.store.load(GGD).await.unwrap();
        let last = chronicle.grams.last().unwrap();
        assert_eq!(last.recorded_via, Some(crate::store::Channel::Link));
        assert_eq!(last.recorded_by.as_deref(), Some(GGD));
    }

    #[tokio::test]
    async fn a_record_taken_over_twice_is_recorded_once() {
        let gemeente = linked(None, GGD).await;
        let ggd = linked(Some(GGD), GEMEENTE).await;
        gemeente
            .perform(None, submission(), &actor())
            .await
            .unwrap();
        let o = gemeente.outbound().await.unwrap().remove(0);
        let step = TakeOver {
            cell: GEMEENTE.into(),
            fact: format!("{}#{}", o.gram.basis, o.gram.fact.as_deref().unwrap()),
            fields: o.gram.fields.clone().into_iter().collect(),
            on: o.gram.recorded_on,
        };
        ggd.take_over(GEMEENTE, &o.gram.id, step.clone(), true)
            .await
            .unwrap();
        let head = ggd.store.load(GGD).await.unwrap().head();
        ggd.take_over(GEMEENTE, &o.gram.id, step.clone(), true)
            .await
            .unwrap();
        assert_eq!(ggd.store.load(GGD).await.unwrap().head(), head);
        // A cell this server is not linked with sends nothing here.
        let stranger = TakeOver {
            cell: "rechtbank_wilgendaal".into(),
            ..step
        };
        assert!(matches!(
            ggd.take_over("rechtbank_wilgendaal", "x", stranger, true)
                .await,
            Err(ServiceError::UnknownCell(_))
        ));
    }

    #[tokio::test]
    async fn a_reset_records_what_the_case_begins_with_again() {
        // A case of the poc's tests whose register begins with three means.
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../poc-chronolexografie/tests/bron-inlogstap/bron.yaml");
        let (corpus, _) = crate::case::load(&manifest).unwrap();
        let store = Arc::new(Contested {
            intrude: true,
            ..Contested::default()
        });
        let clock = Arc::new(ControllableClock::new(
            Arc::clone(&store) as _,
            corpus.casus.speeldatum,
        ));
        let s = Service::start(corpus, Arc::clone(&store) as _, clock)
            .await
            .unwrap();
        let ids = || async {
            let cells: Vec<&str> = s.cells.iter().map(String::as_str).collect();
            store
                .load_all(&cells)
                .await
                .unwrap()
                .into_iter()
                .flat_map(|c| c.into_grams().into_iter().map(|g| g.id))
                .collect::<Vec<_>>()
        };
        let heads = || async {
            let cells: Vec<&str> = s.cells.iter().map(String::as_str).collect();
            store
                .load_all(&cells)
                .await
                .unwrap()
                .iter()
                .map(StoredChronicle::head)
                .collect::<Vec<_>>()
        };
        let begun = heads().await;
        assert!(begun.iter().any(|h| *h > 0), "{begun:?}");
        // A record comes in right after the reset. The beginning is
        // recorded with the reset itself, so it is there all the same, and
        // the record that came in lands after it.
        s.reset().await.unwrap();
        let after = heads().await;
        let grown: Vec<u64> = begun
            .iter()
            .zip(&after)
            .map(|(b, a)| a.checked_sub(*b).unwrap())
            .collect();
        assert_eq!(grown.iter().sum::<u64>(), 1, "{begun:?} -> {after:?}");
        assert!(
            ids()
                .await
                .iter()
                .any(|id| id == "binnengekomen-na-de-reset")
        );
    }

    #[tokio::test]
    async fn a_step_that_a_reset_overtook_runs_again_on_what_the_reset_left() {
        // The default case begins with nothing, so the reset puts every head
        // back where the step read it: only the generation tells.
        let (s, store) = service(0).await;
        store.reset_first.store(true, Ordering::SeqCst);
        let gram = s.perform(None, submission(), &actor()).await.unwrap();
        assert_eq!(store.appends.load(Ordering::SeqCst), 2);
        let stored = store.inner.load("gemeente_wilgendaal").await.unwrap();
        assert_eq!(stored.generation, 1);
        assert_eq!(stored.into_grams()[0].id, gram["id"].as_str().unwrap());
    }

    /// The made-up `sub` of the account the tests act as.
    pub(crate) const ACTOR: &str = "00000000-0000-4000-8000-0000000000ac";

    /// [`ACTOR`] after the test login.
    pub(crate) fn actor() -> Actor {
        Actor {
            account: ACTOR.into(),
            channel: crate::store::Channel::TestLogin,
        }
    }

    /// The citizen of the tests, logged in with `method`, as the API makes
    /// them a submitter.
    pub(crate) fn submitter(method: &str) -> Indiener {
        Indiener {
            inlog: serde_json::from_value(json!({
                "bsn_aanvrager": "999990020",
                "naam": "A. de Vries",
            }))
            .unwrap(),
            inlogmiddel: Some(method.into()),
        }
    }

    pub(crate) fn submission() -> Step {
        Step::Submit(Submit {
            article: None,
            refers_to: None,
            fields: serde_json::from_value(json!({
                "kaart_als_bestuurder": true,
                "kaart_als_passagier": false,
                "adres_aanvrager": "Voorbeeldlaan 2",
            }))
            .unwrap(),
            submitter: Some(submitter("DigiD")),
        })
    }

    /// A service on the PoC's test source with a login step, whose
    /// accredited service knows the users bakker, kapper and bloemist.
    async fn login_step_service() -> Service {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../poc-chronolexografie/tests/bron-inlogstap/bron.yaml");
        let (corpus, _) = crate::case::load(&manifest).unwrap();
        let store = Arc::new(InMemoryStore::new());
        let clock = Arc::new(ControllableClock::new(
            Arc::clone(&store) as _,
            corpus.casus.speeldatum,
        ));
        Service::start(corpus, store, clock).await.unwrap()
    }

    #[tokio::test]
    async fn a_login_through_the_step_records_the_authentication_once_granted() {
        let s = login_step_service().await;
        let cell = "voorbeeld_erkende_dienst";
        let head = || async { s.chronicle(cell).await.unwrap()["kop"].as_u64().unwrap() };
        let before = head().await;

        let refused = s.log_in("kapper", "welkom").await.unwrap();
        assert_eq!(refused, LoginOutcome::NotAuthorized);
        let wrong = s.log_in("bakker", "verkeerd").await.unwrap();
        assert_eq!(wrong, LoginOutcome::WrongPassword);
        assert_eq!(head().await, before, "a refused login records nothing");

        let LoginOutcome::LoggedIn(claims) = s.log_in("bakker", "welkom").await.unwrap() else {
            panic!("bakker may log in");
        };
        assert_eq!(claims["kvk_nummer"], "99970011");
        assert_eq!(head().await, before + 1);
        let step = Step::LogIn(LogIn {
            gebruikersnaam: "bakker".into(),
            wachtwoord: "welkom".into(),
        });
        assert!(!serde_json::to_string(&step).unwrap().contains("welkom"));
        assert!(!format!("{step:?}").contains("welkom"));
    }

    #[tokio::test]
    async fn the_begin_names_the_numbers_of_the_login_key() {
        let s = login_step_service().await;
        let numbers: Vec<String> = s
            .begin_identities("kvk_nummer")
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(numbers, ["99970011", "99970022", "99970033"]);
        assert!(s.login_offer().await.unwrap().is_some());
    }

    #[tokio::test]
    async fn a_ui_spec_that_prefills_a_field_stops_the_server() {
        let (mut corpus, _) =
            crate::case::load(std::path::Path::new(crate::config::DEFAULT_MANIFEST)).unwrap();
        corpus
            .ui_spec
            .prefill
            .insert("adres_aanvrager".into(), "r#5#adres".into());
        let store = Arc::new(InMemoryStore::new());
        let clock = Arc::new(ControllableClock::new(
            Arc::clone(&store) as _,
            corpus.casus.speeldatum,
        ));
        let e = Service::start(corpus, store, clock).await.unwrap_err();
        assert!(
            matches!(e, ServiceError::Prefill(ref f) if f == &["adres_aanvrager"]),
            "{e}"
        );
    }

    /// A field the UI spec fills from a claim no login of this server keeps
    /// would stay empty on every channel: the server does not start.
    #[tokio::test]
    async fn a_field_filled_from_a_claim_no_login_keeps_stops_the_server() {
        use regelrecht_poc_chronolexografie::ui_spec::Systeemvulling;
        let (mut corpus, _) =
            crate::case::load(std::path::Path::new(crate::config::DEFAULT_MANIFEST)).unwrap();
        corpus.ui_spec.systeem.insert(
            "adres_aanvrager".into(),
            Systeemvulling::Inlogclaim("adres".into()),
        );
        let store = Arc::new(InMemoryStore::new());
        let clock = Arc::new(ControllableClock::new(
            Arc::clone(&store) as _,
            corpus.casus.speeldatum,
        ));
        let e = Service::start(corpus, store, clock).await.unwrap_err();
        assert!(
            matches!(e, ServiceError::LoginClaim(ref c) if c == &["adres"]),
            "{e}"
        );
    }

    #[tokio::test]
    async fn a_conflict_runs_the_step_again() {
        let (s, store) = service(2).await;
        let gram = s.perform(None, submission(), &actor()).await.unwrap();
        assert_eq!(gram["kind"], json!("AANVRAAG"));
        assert_eq!(store.appends.load(Ordering::SeqCst), 3);
        // The application and the request for an examination it brought
        // about, once.
        assert_eq!(s.store.load("gemeente_wilgendaal").await.unwrap().head(), 2);
        assert_eq!(s.store.load("ggd_wilgendaal").await.unwrap().head(), 1);
    }

    #[tokio::test]
    async fn conflicts_without_end_give_up_and_record_nothing() {
        let (s, _) = service(MAX_ATTEMPTS).await;
        let e = s.perform(None, submission(), &actor()).await.unwrap_err();
        assert!(matches!(e, ServiceError::Busy), "{e}");
        assert_eq!(s.store.load("gemeente_wilgendaal").await.unwrap().head(), 0);
    }

    #[tokio::test]
    async fn a_request_with_a_nul_character_is_refused_and_records_nothing() {
        let (s, _) = service(0).await;
        let mut step = submission();
        if let Step::Submit(submit) = &mut step {
            submit
                .fields
                .insert("adres_aanvrager".into(), json!("Voorbeeld\u{0}laan 2"));
        }
        let e = s.perform(None, step, &actor()).await.unwrap_err();
        assert!(matches!(e, ServiceError::Refused(_)), "{e}");
        assert_eq!(s.store.load("gemeente_wilgendaal").await.unwrap().head(), 0);
        assert!(holds_nul(&json!({ "a\u{0}": 1 })));
        assert!(!holds_nul(&json!({ "a": ["b", 1, null] })));
    }

    #[tokio::test]
    async fn a_step_is_recorded_as_made_by_its_account_and_a_key_replays_only_for_it() {
        let (s, store) = service(0).await;
        let key: IdempotencyKey = "k-1".parse().unwrap();
        let first = s
            .perform(Some(key.clone()), submission(), &actor())
            .await
            .unwrap();
        let stored = store.inner.load("gemeente_wilgendaal").await.unwrap();
        assert_eq!(stored.grams[0].recorded_by.as_deref(), Some(ACTOR));
        assert_eq!(
            stored.grams[0].recorded_via,
            Some(crate::store::Channel::TestLogin)
        );
        // The request the municipality's policy made with it, and its
        // receipt at the GGD, are the server's, by the article that made
        // them.
        let ggd = store.inner.load("ggd_wilgendaal").await.unwrap();
        for policy in [&stored.grams[1], &ggd.grams[0]] {
            assert_eq!(
                policy.recorded_by.as_deref(),
                Some("beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#9")
            );
            assert_eq!(policy.recorded_via, Some(crate::store::Channel::Policy));
        }
        let again = s
            .perform(Some(key.clone()), submission(), &actor())
            .await
            .unwrap();
        assert_eq!(again, first);
        let e = s
            .perform(
                Some(key),
                submission(),
                &Actor {
                    account: "someone-else".into(),
                    channel: crate::store::Channel::TestLogin,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(e, ServiceError::KeyReused(_)), "{e}");
        // The replay recorded no second application and no second request.
        assert_eq!(s.store.load("gemeente_wilgendaal").await.unwrap().head(), 2);
        assert_eq!(s.store.load("ggd_wilgendaal").await.unwrap().head(), 1);
    }

    #[tokio::test]
    async fn the_dossier_and_the_holder_of_a_record_come_from_the_chronicles() {
        let (s, _) = service(0).await;
        assert_eq!(s.login_field(), "bsn_aanvrager");
        let gram = s.perform(None, submission(), &actor()).await.unwrap();
        let id = gram["id"].as_str().unwrap();
        let mine = s.dossier("999990020").await.unwrap();
        assert!(mine.holds(id));
        // The application, the municipality's request for an examination of
        // the applicant, and its receipt at the GGD: each names the applicant.
        assert_eq!(mine.grams.len(), 3);
        assert!(s.dossier("999990021").await.unwrap().grams.is_empty());
        assert_eq!(
            s.holder(id).await.unwrap().as_deref(),
            Some("gemeente_wilgendaal")
        );
        assert_eq!(s.holder("no-such-record").await.unwrap(), None);
    }

    #[test]
    fn a_chronicle_that_lost_a_record_is_a_bug() {
        let e = appended(vec!["a".into()], vec![vec!["1".into()]], vec![vec![]]).unwrap_err();
        assert!(matches!(e, ServiceError::Internal(_)), "{e}");
        let e = appended(vec!["a".into()], vec![vec![]], vec![]).unwrap_err();
        assert!(matches!(e, ServiceError::Internal(_)), "{e}");
    }
}
