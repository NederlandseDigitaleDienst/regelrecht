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

use std::sync::Arc;

use chrono::NaiveDate;
use regelrecht_cel::Gram;
use regelrecht_poc_chronolexografie::kern::{self, Cellen, Corpus};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::clock::Clock;
use crate::store::{
    Appended, CellAppend, ChronicleStore, Commit, IdempotencyKey, Keyed, Receipt, StoreError,
    StoredChronicle,
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
    store: Arc<dyn ChronicleStore>,
    clock: Arc<dyn Clock>,
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
        let service = Self {
            corpus: Arc::new(corpus),
            cells,
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
        };
        match self.store.append(commit).await {
            // Another instance began first: what it recorded counts.
            Ok(_) | Err(StoreError::Conflict { .. }) => Ok(()),
            Err(e) => Err(e.into()),
        }
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
            "grammen": stored.grams,
        }))
    }

    /// Run `step` and commit what it recorded. Under a `key` used before for
    /// the same request it answers as it did then and records nothing;
    /// without a key every call records.
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
    ) -> Result<Value, ServiceError> {
        let request = serde_json::to_value(&step)
            .map_err(|e| ServiceError::Internal(format!("the request does not serialize: {e}")))?;
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
                .map(|c| c.grams.iter().map(|g| g.id.clone()).collect())
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
    Ok(corpus.herstellen(read.chronicles.into_iter().map(|c| c.grams).collect())?)
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
mod tests {
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

    fn submission() -> Step {
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
        let gram = s.perform(None, submission()).await.unwrap();
        assert_eq!(gram["kind"], json!("AANVRAAG"));
        assert_eq!(store.appends.load(Ordering::SeqCst), 3);
        assert_eq!(s.store.load("gemeente_wilgendaal").await.unwrap().head(), 1);
    }

    #[tokio::test]
    async fn conflicts_without_end_give_up_and_record_nothing() {
        let (s, _) = service(MAX_ATTEMPTS).await;
        let e = s.perform(None, submission()).await.unwrap_err();
        assert!(matches!(e, ServiceError::Busy), "{e}");
        assert_eq!(s.store.load("gemeente_wilgendaal").await.unwrap().head(), 0);
    }

    #[test]
    fn a_chronicle_that_lost_a_record_is_a_bug() {
        let e = appended(vec!["a".into()], vec![vec!["1".into()]], vec![vec![]]).unwrap_err();
        assert!(matches!(e, ServiceError::Internal(_)), "{e}");
        let e = appended(vec!["a".into()], vec![vec![]], vec![]).unwrap_err();
        assert!(matches!(e, ServiceError::Internal(_)), "{e}");
    }
}
