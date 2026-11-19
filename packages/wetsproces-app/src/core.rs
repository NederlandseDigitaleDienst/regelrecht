//! The commands the webview calls, and the session behind them.
//!
//! [`AppCore`] holds what proves who the citizen is: the app key (through
//! its [`KeyStore`]), the credential, and the session of `CreateSession`
//! with its token and AES key. A command takes plain JSON and gives plain
//! JSON; nothing it returns carries any of those.
//!
//! A session lasts at most fifteen minutes on the server. When the service
//! answers `401`, the core starts a new session with the same credential
//! and sends the request once more, so a form the citizen filled in is sent
//! as it was. A submission keeps its idempotency key inside the encrypted
//! body, so sending it again records it once.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use regelrecht_fsc4b::client::{self, App, ClientError, Directory, Session};
use regelrecht_fsc4b::wire::is_test_bsn;
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::config::{AppConfig, ConfigError};
use crate::credential::{Credential, Holder};
use crate::error::CommandError;
use crate::keystore::{KeyStorage, KeyStore};

/// The made-up citizen numbers the app offers in the demo: the ones the
/// test login of the server offers, and the example of the docs. All are
/// from the test range.
pub const TEST_BSNS: [&str; 3] = ["999990019", "999990020", "999990032"];

/// How long before its end a credential counts as lapsed: a session started
/// later would end before it is used.
pub const CREDENTIAL_MARGIN_SECS: i64 = 60;

/// How long a request to the server may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// What the screens may know of the app: never a key, a token or the
/// credential itself.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Status {
    /// The server's address, when the app is configured.
    pub server: Option<String>,
    /// Why the app is not configured, when it is not.
    pub config_problem: Option<String>,
    /// The citizen the credential names, when there is a valid one.
    pub holder: Option<Holder>,
    /// When the credential lapses, as a Unix time.
    pub credential_expires_at: Option<i64>,
    /// Whether a credential is kept that has lapsed.
    pub credential_expired: bool,
    /// Where the app key lives.
    pub key_storage: KeyStorage,
    /// Whether secure hardware keeps the key.
    pub hardware_backed: bool,
    /// The numbers the demo offers to activate with.
    pub test_bsns: Vec<&'static str>,
}

#[derive(Default)]
struct Link {
    directory: Option<Arc<Directory>>,
    session: Option<Session>,
}

/// The core of the app: its configuration, its key store, and the session.
pub struct AppCore {
    config: Result<AppConfig, ConfigError>,
    store: Arc<dyn KeyStore>,
    http: reqwest::Client,
    link: Mutex<Link>,
}

impl std::fmt::Debug for AppCore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppCore")
            .field("config", &self.config)
            .field("storage", &self.store.storage())
            .finish_non_exhaustive()
    }
}

/// An error of a step against the demo Stelsel, where a `404` means that the
/// server offers no demo.
fn demo_step(e: ClientError) -> CommandError {
    match CommandError::from(e) {
        CommandError::Service { status: 404, body } => CommandError::NoDemo {
            detail: body.to_string(),
        },
        other => other,
    }
}

impl AppCore {
    /// A core for `config` (or the reason there is none) that keeps its key
    /// in `store`.
    ///
    /// # Errors
    ///
    /// When the HTTP client cannot be built.
    pub fn build(
        config: Result<AppConfig, ConfigError>,
        store: Arc<dyn KeyStore>,
    ) -> Result<Self, CommandError> {
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|e| CommandError::Protocol {
                detail: format!("the HTTP client: {e}"),
            })?;
        Ok(Self {
            config,
            store,
            http,
            link: Mutex::default(),
        })
    }

    fn config(&self) -> Result<&AppConfig, CommandError> {
        self.config.as_ref().map_err(|e| e.clone().into())
    }

    fn app(&self) -> Result<App, CommandError> {
        Ok(App::from_signer(self.store.signer()?))
    }

    /// The kept credential, read for `app`, valid at least
    /// [`CREDENTIAL_MARGIN_SECS`] longer.
    fn valid_credential(&self, app: &App) -> Result<Credential, CommandError> {
        let token = self.store.credential()?.ok_or(CommandError::NotActivated)?;
        let credential = Credential::read(&token, app.key().public_jwk())?;
        if credential.lapsed(now_unix(), CREDENTIAL_MARGIN_SECS) {
            return Err(CommandError::CredentialExpired);
        }
        Ok(credential)
    }

    /// What the screens may know of the app.
    pub async fn status(&self) -> Status {
        let storage = self.store.storage();
        let credential = match self.store.credential() {
            Ok(Some(token)) => self
                .app()
                .ok()
                .and_then(|app| Credential::read(&token, app.key().public_jwk()).ok()),
            _ => None,
        };
        let expired = credential
            .as_ref()
            .is_some_and(|c| c.lapsed(now_unix(), CREDENTIAL_MARGIN_SECS));
        let valid = credential.filter(|_| !expired);
        Status {
            server: self.config.as_ref().ok().map(|c| c.server().to_string()),
            config_problem: self.config.as_ref().err().map(ToString::to_string),
            holder: valid.as_ref().map(|c| c.holder().clone()),
            credential_expires_at: valid.as_ref().map(Credential::expires_at),
            credential_expired: expired,
            hardware_backed: storage.hardware_backed(),
            key_storage: storage,
            test_bsns: TEST_BSNS.to_vec(),
        }
    }

    /// The organization's directory, read from the demo Stelsel and kept.
    async fn directory(&self, link: &mut Link) -> Result<Arc<Directory>, CommandError> {
        if let Some(directory) = &link.directory {
            return Ok(Arc::clone(directory));
        }
        let config = self.config()?;
        let (document, beheerder) = client::demo_stelsel(&self.http, config.server())
            .await
            .map_err(demo_step)?;
        let directory = Directory::discover(&self.http, &document, &beheerder, config.oin())
            .await
            .map_err(demo_step)?;
        let directory = Arc::new(directory);
        link.directory = Some(Arc::clone(&directory));
        Ok(directory)
    }

    /// A new session for `app` with `credential`, kept in `link`.
    async fn new_session(
        &self,
        link: &mut Link,
        app: &App,
        credential: &Credential,
    ) -> Result<Session, CommandError> {
        let directory = self.directory(link).await?;
        let session = client::create_session(&self.http, &directory, app, credential.token())
            .await
            .map_err(|e| match e {
                ClientError::Status { status, .. } if status == StatusCode::UNAUTHORIZED => {
                    CommandError::CredentialRefused
                }
                other => other.into(),
            })?;
        link.session = Some(session.clone());
        Ok(session)
    }

    /// Activate the app for the test citizen `bsn`: a new key, a
    /// credential from the test Activatiedienst, the Stelseldocument
    /// checked, and a first session.
    ///
    /// # Errors
    ///
    /// [`CommandError::Input`] for a number the demo does not offer,
    /// [`CommandError::NoDemo`] when the server has no demo Stelsel, and
    /// any error of the steps.
    pub async fn activate(&self, bsn: &str) -> Result<Status, CommandError> {
        if !TEST_BSNS.contains(&bsn) || !is_test_bsn(bsn) {
            return Err(CommandError::input(format!(
                "the demo activates with one of {TEST_BSNS:?} only"
            )));
        }
        let config = self.config()?;
        let mut link = self.link.lock().await;
        *link = Link::default();
        self.store.reset()?;
        let app = self.app()?;
        // The Stelsel first: a server without a demo is told before a key
        // is asked a credential.
        self.directory(&mut link).await?;
        let token = client::demo_credential(&self.http, config.server(), &app, bsn)
            .await
            .map_err(demo_step)?;
        let credential = Credential::read(&token, app.key().public_jwk())?;
        self.store.store_credential(credential.token())?;
        self.new_session(&mut link, &app, &credential).await?;
        drop(link);
        Ok(self.status().await)
    }

    /// Forget the key, the credential and the session.
    ///
    /// # Errors
    ///
    /// When the key store cannot be cleared.
    pub async fn deactivate(&self) -> Result<Status, CommandError> {
        let mut link = self.link.lock().await;
        *link = Link::default();
        self.store.reset()?;
        drop(link);
        Ok(self.status().await)
    }

    /// The live session, or a new one when there is none or when the one
    /// under the token `stale` was refused.
    async fn session(&self, app: &App, stale: Option<&str>) -> Result<Session, CommandError> {
        // Held through `CreateSession`: commands that find no live session at
        // the same moment wait for one and share it.
        let mut link = self.link.lock().await;
        if let Some(session) = &link.session
            && stale.is_none_or(|token| token != session.token)
        {
            return Ok(session.clone());
        }
        let credential = self.valid_credential(app)?;
        self.new_session(&mut link, app, &credential).await
    }

    /// `payload` to the citizen's route `op`, with `idempotency_key` inside
    /// it; the answer, after one new session when the first was refused.
    async fn call(
        &self,
        op: &str,
        payload: &Value,
        idempotency_key: Option<&str>,
    ) -> Result<Value, CommandError> {
        self.config()?;
        let app = self.app()?;
        self.valid_credential(&app)?;
        let session = self.session(&app, None).await?;
        let mut answer = session
            .call(&self.http, op, payload, idempotency_key)
            .await?;
        if answer.0 == StatusCode::UNAUTHORIZED {
            let renewed = self.session(&app, Some(&session.token)).await?;
            answer = renewed
                .call(&self.http, op, payload, idempotency_key)
                .await?;
        }
        match answer {
            (status, _) if status == StatusCode::UNAUTHORIZED => {
                Err(CommandError::CredentialRefused)
            }
            (status, body) if status.is_success() => Ok(body),
            (status, body) => Err(CommandError::Service {
                status: status.as_u16(),
                body,
            }),
        }
    }

    /// The case and today's date.
    ///
    /// # Errors
    ///
    /// As every command: not configured or activated, a lapsed or refused
    /// credential, the network, or the service's own refusal.
    pub async fn case(&self) -> Result<Value, CommandError> {
        self.call("casus", &json!({}), None).await
    }

    /// The forms.
    ///
    /// # Errors
    ///
    /// As [`AppCore::case`].
    pub async fn forms(&self) -> Result<Value, CommandError> {
        self.call("formulier", &json!({}), None).await
    }

    /// The citizen's own dossier.
    ///
    /// # Errors
    ///
    /// As [`AppCore::case`].
    pub async fn dossier(&self) -> Result<Value, CommandError> {
        self.call("dossier", &json!({}), None).await
    }

    /// Why the decision `decision` of the citizen's dossier came out so.
    ///
    /// # Errors
    ///
    /// As [`AppCore::case`].
    pub async fn explain(&self, decision: &str) -> Result<Value, CommandError> {
        self.call("waarom", &json!({ "id": decision }), None).await
    }

    /// The submission `submission`, under `idempotency_key`: sent again
    /// with the same key, it is answered as the first time.
    ///
    /// # Errors
    ///
    /// [`CommandError::Input`] when `submission` is no JSON object or the
    /// key is empty; otherwise as [`AppCore::case`].
    pub async fn submit(
        &self,
        submission: &Value,
        idempotency_key: &str,
    ) -> Result<Value, CommandError> {
        if !submission.is_object() {
            return Err(CommandError::input("a submission is a JSON object"));
        }
        if idempotency_key.trim().is_empty() {
            return Err(CommandError::input("a submission needs an idempotency key"));
        }
        self.call("indieningen", submission, Some(idempotency_key))
            .await
    }

    /// Spoil the token of the live session, as if the server had let it
    /// lapse.
    #[cfg(feature = "test-utils")]
    pub async fn spoil_session(&self) {
        if let Some(session) = self.link.lock().await.session.as_mut() {
            session.token = "spoiled".into();
        }
    }
}
