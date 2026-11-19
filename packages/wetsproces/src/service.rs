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

use std::sync::Arc;

use chrono::NaiveDate;
use regelrecht_cel::Gram;
use regelrecht_poc_chronolexografie::kern::{self, Cellen, Corpus};
use regelrecht_poc_chronolexografie::ui_spec::Systeemvulling;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::clock::Clock;
use crate::store::{
    Actor, Appended, CellAppend, ChronicleStore, Commit, IdempotencyKey, Keyed, Receipt,
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
}

/// The cell that holds the decision `besluit` sends a copy of it to the cell
/// whose decision it concerns (`kern::toezenden`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SendCopy {
    #[serde(rename = "besluit")]
    pub decision: String,
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
}

impl Step {
    /// Run the step of the kern on `cellen`, on `on`.
    fn apply(&self, corpus: &Corpus, cellen: &mut Cellen, on: NaiveDate) -> kern::Result<Value> {
        match self {
            Self::Submit(s) => {
                let article = s.article.as_deref().unwrap_or(&corpus.casus.artikel);
                let gram = kern::indienen(
                    corpus,
                    cellen,
                    article,
                    s.refers_to.as_deref(),
                    &s.fields,
                    on,
                )?;
                to_value(&gram)
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
                to_value(&besluit)
            }
            Self::Record(r) => {
                let gram = kern::vastleggen_verwijzend(
                    corpus,
                    cellen,
                    &r.cell,
                    &r.fact,
                    &r.fields,
                    r.refers_to.as_deref(),
                    on,
                )?;
                to_value(&gram)
            }
            Self::SendCopy(s) => to_value(&kern::toezenden(corpus, cellen, &s.decision, on)?),
        }
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
}

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

/// What a step that records came to: its answer and the appends to commit.
struct Ran {
    response: Value,
    appends: Vec<CellAppend>,
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
    /// The service for `corpus` on `store` and `clock`. When no cell of the
    /// case has a record yet, what the case begins with (its `begin`) is
    /// committed first, once, whichever instance gets there first.
    ///
    /// # Errors
    ///
    /// When the store fails, or the case's beginning does not record.
    pub async fn start(
        corpus: Corpus,
        store: Arc<dyn ChronicleStore>,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, ServiceError> {
        let cells = corpus.cellen().map(str::to_string).collect();
        let login_field = login_field(&corpus)?;
        let service = Self {
            corpus: Arc::new(corpus),
            cells,
            login_field,
            store,
            clock,
        };
        service.begin().await?;
        Ok(service)
    }

    async fn begin(&self) -> Result<(), ServiceError> {
        let read = self.read().await?;
        if read.chronicles.iter().any(|c| c.head() > 0) {
            return Ok(());
        }
        let corpus = Arc::clone(&self.corpus);
        let begun = blocking(move || Ok(corpus.cell()?.kronieken())).await?;
        if begun.iter().all(Vec::is_empty) {
            return Ok(());
        }
        let appends = self
            .cells
            .iter()
            .zip(begun)
            .map(|(cell, grams)| CellAppend {
                cell: cell.clone(),
                expected_head: 0,
                grams,
            })
            .collect();
        // No key: the expected heads of 0 already make sure the beginning is
        // recorded once, whichever instance gets there first.
        let commit = Commit {
            appends,
            keyed: None,
            recorded_by: None,
        };
        match self.store.append(commit).await {
            // Another instance began first: what it recorded counts.
            Ok(_) | Err(StoreError::Conflict { .. }) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// The field of the case's application that identifies the applicant.
    pub fn login_field(&self) -> &str {
        &self.login_field
    }

    /// The article of the case's application, which the login field
    /// belongs to.
    pub fn application_article(&self) -> &str {
        &self.corpus.casus.artikel
    }

    /// Today, by the clock of this service.
    pub fn today(&self) -> NaiveDate {
        self.clock.today()
    }

    /// The case as a screen shows it: its text and what follows from the
    /// law (`Corpus::beschrijving`), and today.
    pub fn case(&self) -> Value {
        let mut case = self.corpus.beschrijving();
        if let Some(map) = case.as_object_mut() {
            map.insert("vandaag".into(), json!(self.today()));
        }
        case
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
        let today = self.today();
        blocking(move || {
            let cellen = restore(&corpus, read)?;
            Ok(kern::vormen(&corpus, &cellen, today)?)
        })
        .await
    }

    /// The cells of the case, its own first, each with the name a screen
    /// shows for it.
    pub fn cells(&self) -> impl Iterator<Item = (&str, &str)> {
        self.cells
            .iter()
            .map(|c| (c.as_str(), self.corpus.cel_naam(c)))
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
    /// case. Taken on a copy of the cells; nothing is recorded.
    ///
    /// # Errors
    ///
    /// When the store fails, and what the kern fails on besides a refusal of
    /// the cell, which the preview carries as `geweigerd`.
    pub async fn preview(&self, d: Decide) -> Result<Value, ServiceError> {
        let read = self.read().await?;
        let corpus = Arc::clone(&self.corpus);
        let today = self.today();
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
            Ok(to_value(&preview)?)
        })
        .await
    }

    /// The chronicle of the cell `cell`: its records and its head.
    ///
    /// # Errors
    ///
    /// When the case has no such cell, or the store fails.
    pub async fn chronicle(&self, cell: &str) -> Result<Value, ServiceError> {
        if !self.cells.iter().any(|c| c == cell) {
            return Err(ServiceError::UnknownCell(cell.to_string()));
        }
        let stored = self.store.load(cell).await?;
        Ok(json!({
            "cel": cell,
            "cel_naam": self.corpus.cel_naam(cell),
            "kop": stored.head(),
            "grammen": stored.into_grams(),
        }))
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
                Err(e) => return Err(e.into()),
            }
        }
        Err(ServiceError::Busy)
    }

    /// Run `step` on the chronicles as they are now, and say what it
    /// recorded in each cell.
    async fn run(&self, step: Arc<Step>) -> Result<Ran, ServiceError> {
        let read = self.read().await?;
        let corpus = Arc::clone(&self.corpus);
        let cells = self.cells.clone();
        let today = self.today();
        blocking(move || {
            let before: Vec<Vec<String>> = read
                .chronicles
                .iter()
                .map(|c| c.grams.iter().map(|g| g.gram.id.clone()).collect())
                .collect();
            let mut cellen = restore(&corpus, read)?;
            let response = step.apply(&corpus, &mut cellen, today)?;
            let appends = appended(cells, before, cellen.kronieken())?;
            Ok(Ran { response, appends })
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
        Ok(Read { chronicles })
    }
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;

    use super::*;
    use crate::clock::ControllableClock;
    use crate::store::memory::InMemoryStore;

    /// The store in memory, refusing the first `conflicts` commits as if
    /// another request got in first.
    #[derive(Debug)]
    struct Contested {
        inner: InMemoryStore,
        conflicts: usize,
        appends: AtomicUsize,
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
        async fn append(&self, commit: Commit) -> Result<Appended, StoreError> {
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
            inner: InMemoryStore::new(),
            conflicts,
            appends: AtomicUsize::new(0),
        });
        let clock = Arc::new(ControllableClock::new(corpus.casus.speeldatum));
        let s = Service::start(corpus, Arc::clone(&store) as _, clock)
            .await
            .unwrap();
        (s, store)
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

    pub(crate) fn submission() -> Step {
        Step::Submit(Submit {
            article: None,
            refers_to: None,
            fields: serde_json::from_value(json!({
                "bsn_aanvrager": "999990020",
                "kaart_als_bestuurder": true,
                "kaart_als_passagier": false,
                "aanvraag_ondertekend": true,
                "naam_aanvrager": "A. de Vries",
                "adres_aanvrager": "Voorbeeldlaan 2",
                "dagtekening": "2026-10-05",
                "gevraagde_beschikking": "gehandicaptenparkeerkaart",
            }))
            .unwrap(),
        })
    }

    #[tokio::test]
    async fn a_conflict_runs_the_step_again() {
        let (s, store) = service(2).await;
        let gram = s.perform(None, submission(), &actor()).await.unwrap();
        assert_eq!(gram["kind"], json!("AANVRAAG"));
        assert_eq!(store.appends.load(Ordering::SeqCst), 3);
        assert_eq!(s.store.load("gemeente_wilgendaal").await.unwrap().head(), 1);
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
                .insert("naam_aanvrager".into(), json!("A.\u{0}de Vries"));
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
        assert_eq!(s.store.load("gemeente_wilgendaal").await.unwrap().head(), 1);
    }

    #[tokio::test]
    async fn the_dossier_and_the_holder_of_a_record_come_from_the_chronicles() {
        let (s, _) = service(0).await;
        assert_eq!(s.login_field(), "bsn_aanvrager");
        let gram = s.perform(None, submission(), &actor()).await.unwrap();
        let id = gram["id"].as_str().unwrap();
        let mine = s.dossier("999990020").await.unwrap();
        assert!(mine.holds(id));
        assert_eq!(mine.grams.len(), 1);
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
