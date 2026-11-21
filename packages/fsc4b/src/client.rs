//! A reference client of the FSC voor Burgers channel, over HTTP: what an
//! app does, step by step, from the Stelseldocument to a submission. The
//! end-to-end tests run on it, and it shows what a native app has to do:
//!
//! 1. keep a P-256 key pair ([`App`]); on a phone, in its secure hardware;
//! 2. get a credential for that key from an Activatiedienst (here the test
//!    one of the demo Stelsel, [`demo_credential`]);
//! 3. read the Stelseldocument (of the demo: [`demo_stelsel`]), check its
//!    signature, and find the organization's key and discovery
//!    configuration ([`Directory`]);
//! 4. run `CreateSession` ([`create_session`]): ask a challenge, check it
//!    against the organization's key, sign the nonce, encrypt the
//!    presentation to the ephemeral key, and decrypt the session token;
//! 5. call the citizen's service under the session key ([`Session::call`]).
//!
//! Each step is its own function, so a test can do one of them wrong.

use std::sync::Arc;

use reqwest::StatusCode;
use serde_json::{Value, json};

use crate::jose::{self, AesKey, Es256Signer, JoseError, SigningKey, VerifyingKey, sign_jws};
use crate::wire::{
    APP_MANAGER_CERTIFICATE, CHALLENGE_RESPONSE, CHALLENGE_SUB, CITIZEN_SERVICE, DEMO_CREDENTIAL,
    DEMO_STELSELBEHEERDER, DEMO_STELSELDOCUMENT, IDEMPOTENCY_MEMBER, SESSION_SERVICE,
    self_signed_certificate,
};

/// Why a step of the client failed.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("HTTP: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JOSE: {0}")]
    Jose(#[from] JoseError),
    #[error("{0}")]
    Protocol(String),
    #[error("the server answered {status}: {body}")]
    Status { status: StatusCode, body: String },
}

fn protocol(message: impl Into<String>) -> ClientError {
    ClientError::Protocol(message.into())
}

/// The text of `response`, when its status is a success.
async fn ok_text(response: reqwest::Response) -> Result<String, ClientError> {
    let status = response.status();
    let body = response.text().await?;
    if status.is_success() {
        Ok(body)
    } else {
        Err(ClientError::Status { status, body })
    }
}

/// The app: what signs with its key. A real app keeps the private key in
/// the phone's secure hardware and never lets it out; the client only asks
/// it to sign, through [`Es256Signer`].
#[derive(Clone)]
pub struct App {
    key: Arc<dyn Es256Signer>,
}

impl std::fmt::Debug for App {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App")
            .field("thumbprint", &self.key.public_jwk().thumbprint())
            .finish()
    }
}

impl App {
    /// An app with a fresh key pair in this process.
    ///
    /// # Errors
    ///
    /// When the library fails.
    pub fn generate() -> Result<Self, ClientError> {
        Ok(Self::from_key(SigningKey::generate()?))
    }

    /// An app with the key pair `key`, in this process.
    pub fn from_key(key: SigningKey) -> Self {
        Self { key: Arc::new(key) }
    }

    /// An app whose key signs through `signer`, wherever it keeps the key.
    pub fn from_signer(signer: Arc<dyn Es256Signer>) -> Self {
        Self { key: signer }
    }

    pub fn key(&self) -> &dyn Es256Signer {
        self.key.as_ref()
    }

    /// The signature over `nonce` for the verifier with OIN `aud`
    /// (§4.3.4.1).
    ///
    /// # Errors
    ///
    /// When signing fails.
    pub fn sign_nonce(&self, nonce: &str, aud: &str) -> Result<String, ClientError> {
        Ok(sign_jws(
            self.key(),
            &json!({ "sub": CHALLENGE_RESPONSE, "aud": aud, "nonce": nonce }),
            &[],
        )?)
    }
}

/// A credential for `app` and the test BSN `bsn` from the test
/// Activatiedienst of a demo Stelsel at `base_url`.
///
/// # Errors
///
/// When the request fails or is refused.
pub async fn demo_credential(
    http: &reqwest::Client,
    base_url: &str,
    app: &App,
    bsn: &str,
) -> Result<String, ClientError> {
    let body = json!({
        "certificate": self_signed_certificate(app.key())?,
        "bsn": bsn,
    });
    let response = http
        .post(format!("{base_url}{DEMO_CREDENTIAL}"))
        .json(&body)
        .send()
        .await?;
    ok_text(response).await
}

/// The Stelseldocument of a demo Stelsel at `base_url`, and the
/// Stelselbeheerder's key it verifies under. A real app has that key built
/// in; the demo serves it, so trusting it is trusting the server.
///
/// # Errors
///
/// When a request fails or is refused, or the key is no P-256 JWK.
pub async fn demo_stelsel(
    http: &reqwest::Client,
    base_url: &str,
) -> Result<(String, VerifyingKey), ClientError> {
    let beheerder = http
        .get(format!("{base_url}{DEMO_STELSELBEHEERDER}"))
        .send()
        .await?;
    let beheerder: Value = serde_json::from_str(&ok_text(beheerder).await?)
        .map_err(|_| protocol("the Stelselbeheerder's key is no JSON"))?;
    let document = http
        .get(format!("{base_url}{DEMO_STELSELDOCUMENT}"))
        .send()
        .await?;
    let document = ok_text(document).await?;
    Ok((document, VerifyingKey::from_value(&beheerder)?))
}

/// What the app needs to know of one organization: its OIN, its key, and
/// where its two services are.
#[derive(Debug)]
pub struct Directory {
    pub oin: String,
    pub organization_key: VerifyingKey,
    pub session_api: String,
    pub citizen_api: String,
}

impl Directory {
    /// The organization with OIN `oin` in `stelseldocument`, checked under
    /// the Stelselbeheerder's key `beheerder`, and its services from its
    /// discovery configuration (§4.2.6).
    ///
    /// # Errors
    ///
    /// When the document does not verify or does not list the
    /// organization, or the discovery configuration does not name both
    /// services in version `v1`.
    pub async fn discover(
        http: &reqwest::Client,
        stelseldocument: &str,
        beheerder: &VerifyingKey,
        oin: &str,
    ) -> Result<Self, ClientError> {
        let document = beheerder.verify(stelseldocument.trim())?;
        let organization = document
            .claims
            .get("organizations")
            .and_then(Value::as_array)
            .and_then(|all| all.iter().find(|o| o["oin"] == oin))
            .ok_or_else(|| protocol(format!("the Stelseldocument does not list {oin}")))?;
        let organization_key = VerifyingKey::from_value(&organization["public_key"])?;
        let discovery_url = organization["discovery_url"]
            .as_str()
            .ok_or_else(|| protocol("no discovery_url"))?;
        let discovery = ok_text(http.get(discovery_url).send().await?).await?;
        let discovery: Value = serde_json::from_str(&discovery)
            .map_err(|_| protocol("the discovery configuration is no JSON"))?;
        let service = |name: &str| {
            discovery[name]["v1"]
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| protocol(format!("the discovery configuration lacks {name} v1")))
        };
        Ok(Self {
            oin: oin.to_string(),
            organization_key,
            session_api: service(SESSION_SERVICE)?,
            citizen_api: service(CITIZEN_SERVICE)?,
        })
    }
}

/// A challenge, checked against the organization's key (steps 10 to 12).
#[derive(Debug, Clone)]
pub struct Challenge {
    pub nonce: String,
    pub ephemeral_der: Vec<u8>,
}

/// A new challenge of the organization in `directory`.
///
/// # Errors
///
/// When the request fails, or the JWS does not verify under the
/// organization's key or lacks a claim.
pub async fn challenge(
    http: &reqwest::Client,
    directory: &Directory,
) -> Result<Challenge, ClientError> {
    let response = http
        .post(format!("{}/v1/challenge", directory.session_api))
        .send()
        .await?;
    let jws = ok_text(response).await?;
    let verified = directory.organization_key.verify(&jws)?;
    if verified.text("sub") != Some(CHALLENGE_SUB) {
        return Err(protocol("the challenge has the wrong `sub`"));
    }
    let nonce = verified
        .text("nonce")
        .ok_or_else(|| protocol("the challenge has no nonce"))?;
    let der = verified
        .text("ephemeral_organization_ec_public_key")
        .ok_or_else(|| protocol("the challenge has no ephemeral key"))?;
    Ok(Challenge {
        nonce: nonce.to_string(),
        ephemeral_der: jose::unb64(der, "the ephemeral key")?,
    })
}

/// The presentation of `credential` for `challenge`, signed for `aud`,
/// with `session_key`, encrypted to the challenge's ephemeral key
/// (steps 13 to 19).
///
/// # Errors
///
/// When signing or encryption fails.
pub fn presentation(
    app: &App,
    challenge: &Challenge,
    credential: &str,
    aud: &str,
    session_key: &AesKey,
) -> Result<String, ClientError> {
    let vp = json!({
        "app_nonce_signature": app.sign_nonce(&challenge.nonce, aud)?,
        "certificate_type": APP_MANAGER_CERTIFICATE,
        "certificate": credential,
        "session_aes_key": jose::b64(session_key.as_bytes()),
    });
    Ok(jose::encrypt_to_der(
        &challenge.ephemeral_der,
        vp.to_string().as_bytes(),
    )?)
}

/// Send the encrypted presentation `jwe` for `nonce` (step 20), and the
/// answer as it came.
///
/// # Errors
///
/// When the request fails.
pub async fn complete(
    http: &reqwest::Client,
    directory: &Directory,
    nonce: &str,
    jwe: &str,
) -> Result<reqwest::Response, ClientError> {
    Ok(http
        .post(format!("{}/v1/complete", directory.session_api))
        .json(&json!({ "nonce": nonce, "encrypted_challenge_result": jwe }))
        .send()
        .await?)
}

/// A session: its token, its key, and where the citizen's service is.
#[derive(Clone)]
pub struct Session {
    pub token: String,
    pub key: AesKey,
    pub citizen_api: String,
}

/// `CreateSession` from start to end for `app` with `credential`.
///
/// # Errors
///
/// When any step fails or the server refuses.
pub async fn create_session(
    http: &reqwest::Client,
    directory: &Directory,
    app: &App,
    credential: &str,
) -> Result<Session, ClientError> {
    let challenge = challenge(http, directory).await?;
    let key = AesKey::generate()?;
    let jwe = presentation(app, &challenge, credential, &directory.oin, &key)?;
    let response = complete(http, directory, &challenge.nonce, &jwe).await?;
    let encrypted = ok_text(response).await?;
    let token = String::from_utf8(key.decrypt(&encrypted)?)
        .map_err(|_| protocol("the session token is no text"))?;
    Ok(Session {
        token,
        key,
        citizen_api: directory.citizen_api.clone(),
    })
}

impl std::fmt::Debug for Session {
    // The token is a secret as long as the session lasts.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("citizen_api", &self.citizen_api)
            .finish_non_exhaustive()
    }
}

impl Session {
    /// `payload` to the citizen's route `/v1/{op}` (`casus`, `formulier`,
    /// `dossier`, `indieningen`), encrypted under the session key, with
    /// `idempotency_key` inside it; the status and the decrypted answer.
    ///
    /// # Errors
    ///
    /// When the request fails, or an answer does not decrypt under the
    /// session key. Only a `401` and a `500` are read as plain JSON.
    pub async fn call(
        &self,
        http: &reqwest::Client,
        op: &str,
        payload: &Value,
        idempotency_key: Option<&str>,
    ) -> Result<(StatusCode, Value), ClientError> {
        let mut payload = payload.clone();
        if let Some(key) = idempotency_key {
            payload
                .as_object_mut()
                .ok_or_else(|| protocol("an idempotency key goes into a JSON object"))?
                .insert(IDEMPOTENCY_MEMBER.into(), Value::from(key));
        }
        let body = self.key.encrypt(payload.to_string().as_bytes())?;
        self.send(http, op, body).await
    }

    /// The encrypted `body` to the citizen's route `/v1/{op}`, as it is: for
    /// a client that keeps a request to send it again; the status and the
    /// decrypted answer.
    ///
    /// # Errors
    ///
    /// As [`Session::call`].
    pub async fn send(
        &self,
        http: &reqwest::Client,
        op: &str,
        body: String,
    ) -> Result<(StatusCode, Value), ClientError> {
        let response = http
            .post(format!("{}/v1/{op}", self.citizen_api))
            .header(reqwest::header::AUTHORIZATION, &self.token)
            .body(body)
            .send()
            .await?;
        let status = response.status();
        let text = response.text().await?;
        // Only a refusal of the session (401) and a failure to read it (500)
        // come unencrypted; every other answer must decrypt, or it did not
        // come from the server that holds the session.
        // A 500 after the session was read is encrypted like any answer.
        let plain = match self.key.decrypt(&text) {
            Ok(plain) => plain,
            Err(_)
                if status == StatusCode::UNAUTHORIZED
                    || status == StatusCode::INTERNAL_SERVER_ERROR =>
            {
                let value = serde_json::from_str(&text).unwrap_or(Value::String(text));
                return Ok((status, value));
            }
            Err(e) => return Err(e.into()),
        };
        let value = serde_json::from_slice(&plain)
            .map_err(|_| protocol("the answer decrypts to no JSON"))?;
        Ok((status, value))
    }
}
