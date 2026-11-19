//! The names on the wire that a server and an app must spell alike: the
//! fixed values inside the tokens, the service names of the discovery
//! configuration, and the routes of the demo Stelsel. The server's modules
//! re-export each one where it is used, so a change here changes both
//! sides at once.

use serde_json::json;

use crate::jose::{Es256Signer, JoseError, sign_jws};

/// The `certificate_type` of a credential from an Activatiedienst.
pub const APP_MANAGER_CERTIFICATE: &str = "app_manager_jwt_certificate";

/// The `certificate_type` of a self-signed key (§4.3.4.1).
pub const SELF_SIGNED_CERTIFICATE: &str = "certificate_type_self_signed";

/// The fixed `sub` of the app's signature over the nonce.
pub const CHALLENGE_RESPONSE: &str = "challenge_response";

/// The fixed `sub` of a challenge.
pub const CHALLENGE_SUB: &str = "challenge";

/// The name of the session service in the discovery configuration, as in
/// the example of §4.2.5.
pub const SESSION_SERVICE: &str = "sessionApi";

/// The name of the citizen's service in the discovery configuration.
pub const CITIZEN_SERVICE: &str = "wetsprocesBurgerApi";

/// The member of a submission's payload that holds its idempotency key.
pub const IDEMPOTENCY_MEMBER: &str = "idempotency_key";

/// `GET`: the demo Stelseldocument.
pub const DEMO_STELSELDOCUMENT: &str = "/fsc4b/demo/stelseldocument";
/// `GET`: the demo Stelselbeheerder's public key.
pub const DEMO_STELSELBEHEERDER: &str = "/fsc4b/demo/stelselbeheerder";
/// `POST`: a credential of the test Activatiedienst.
pub const DEMO_CREDENTIAL: &str = "/fsc4b/demo/activatiedienst/v1/credential";
/// The made-up OIN of the organization in the demo.
pub const DEMO_ORGANIZATION_OIN: &str = "00000099000000000001";
/// What a test BSN starts with: the range of numbers made for testing,
/// which no person holds.
pub const TEST_BSN_PREFIX: &str = "99999";

/// The made-up citizens of the test login and the demo app, from the test
/// range.
pub const TEST_CITIZENS: [&str; 2] = ["999990032", "999990020"];

/// Whether `bsn` is a number of the test range: nine digits after
/// [`TEST_BSN_PREFIX`].
pub fn is_test_bsn(bsn: &str) -> bool {
    bsn.len() == 9 && bsn.starts_with(TEST_BSN_PREFIX) && bsn.bytes().all(|b| b.is_ascii_digit())
}

/// The self-signed certificate (§4.3.4.1) of the key of `signer`: what the
/// test Activatiedienst takes as proof that the app holds that key.
///
/// # Errors
///
/// When signing fails.
pub fn self_signed_certificate(signer: &dyn Es256Signer) -> Result<String, JoseError> {
    sign_jws(
        signer,
        &json!({ "sub": SELF_SIGNED_CERTIFICATE, "cnf": { "jwk": signer.public_jwk() } }),
        &[],
    )
}
