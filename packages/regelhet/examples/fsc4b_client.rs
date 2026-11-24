//! The reference client of FSC voor Burgers against a running server in
//! test mode with the demo Stelsel (`REGELHET_FSC4B_DEMO=true`): it makes
//! an app key, gets a credential from the test Activatiedienst, reads and
//! checks the Stelseldocument, runs CreateSession, asks the source data the
//! executor holds about the citizen and checks their signature, submits an
//! application and reads its own dossier.
//!
//! ```text
//! cargo run -p regelrecht-regelhet --features test-utils --example fsc4b_client -- \
//!     http://localhost:8000 999990019
//! ```

use std::process::ExitCode;

use regelrecht_regelhet::fsc4b::client::{self, App, Directory};
use regelrecht_regelhet::fsc4b::demo::{DEMO_ORGANIZATION_OIN, STELSELBEHEERDER, STELSELDOCUMENT};
use regelrecht_regelhet::fsc4b::jose::VerifyingKey;
use serde_json::{Value, json};

#[tokio::main]
async fn main() -> ExitCode {
    // reqwest is built without a TLS provider of its own; the server's.
    regelrecht_auth::install_crypto_provider();
    let mut args = std::env::args().skip(1);
    let base = args
        .next()
        .unwrap_or_else(|| "http://localhost:8000".into());
    let bsn = args.next().unwrap_or_else(|| "999990019".into());
    match run(base.trim_end_matches('/'), &bsn).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("fsc4b_client: {e}");
            ExitCode::FAILURE
        }
    }
}

async fn run(base: &str, bsn: &str) -> Result<(), Box<dyn std::error::Error>> {
    let http = reqwest::Client::new();
    let app = App::generate()?;
    let credential = client::demo_credential(&http, base, &app, bsn).await?;
    println!("credential for {bsn} from the test Activatiedienst");
    // A real app has the Stelselbeheerder's key built in; the demo serves it.
    let beheerder: Value = http
        .get(format!("{base}{STELSELBEHEERDER}"))
        .send()
        .await?
        .json()
        .await?;
    let beheerder = VerifyingKey::from_value(&beheerder)?;
    let document = http
        .get(format!("{base}{STELSELDOCUMENT}"))
        .send()
        .await?
        .text()
        .await?;
    let directory =
        Directory::discover(&http, &document, &beheerder, DEMO_ORGANIZATION_OIN).await?;
    println!("session service at {}", directory.session_api);
    let session = client::create_session(&http, &directory, &app, &credential).await?;
    println!("session created");
    let (status, answer) = session.call(&http, "gegevens", &json!({}), None).await?;
    println!("source data: {status}");
    // The answer carries the data twice: signed, and as the payload of that
    // signature for reading. An app keeps the signed form, which shows later
    // where a value came from.
    let signed = answer["ondertekend"]
        .as_str()
        .ok_or("the source data are not signed")?;
    let verified = directory.organization_key.verify(signed)?;
    if answer["gegevens"].as_object() != Some(&verified.claims) {
        return Err("the source data differ from what the organization signed".into());
    }
    println!(
        "signed by the organization, for {}:\n{:#}",
        verified.claims["sub"], verified.claims["antwoorden"]
    );
    let application = json!({ "velden": {
        "kaart_als_bestuurder": true,
        "kaart_als_passagier": false,
        "adres_aanvrager": "Voorbeeldlaan 1",
    }});
    let (status, answer) = session
        .call(&http, "indieningen", &application, None)
        .await?;
    println!("submission: {status}\n{answer:#}");
    let (status, dossier) = session.call(&http, "dossier", &json!({}), None).await?;
    println!("dossier: {status}\n{dossier:#}");
    Ok(())
}
