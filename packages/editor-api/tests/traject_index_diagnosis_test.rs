//! Eén test per oorzaak van een mislukte index-scan van de traject-eigen
//! source: geeft de classificatie het juiste antwoord, en leest de melding
//! die eruit komt als iets waar een gebruiker wat mee kan?
//!
//! GitHub wordt gespeeld door wiremock; de client wordt met `from_base` op
//! die mock gericht. Geen database, geen netwerk naar buiten — de classificatie
//! is bewust een losse functie zodat precies dit mogelijk is.
//!
//! Repo-coördinaten zijn verzonnen placeholders (`example-org/...`): dit is
//! een publieke repo, dus er staat nergens een echt adres in.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use pretty_assertions::assert_eq;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use axum::http::StatusCode;
use regelrecht_editor_api::traject_index_diagnosis::{
    IndexFailureKind, OwnSourceTarget, TokenOrigin, classify_index_failure, index_failure_to_status,
};
use regelrecht_github::GithubClient;

const OWNER: &str = "example-org";
const REPO: &str = "regelrecht-corpus-example";
const BRANCH: &str = "traject/tarieven-1a2b3c4d";
const BASE: &str = "main";
const TOKEN: &str = "gh-token-uit-de-test";

/// De melding die de bibliotheek toont als er niets beters is. Geen enkele
/// classificatie mag hierop terugvallen.
const GENERIEKE_TERUGVAL: &str = "De gegevens konden niet worden opgehaald.";

/// Bovengrens die de bibliotheek hanteert voordat ze een backend-melding
/// als onbruikbaar wegkapt.
const MELDING_MAX: usize = 300;

fn target() -> OwnSourceTarget {
    OwnSourceTarget {
        traject_id: Uuid::nil(),
        source_id: "traject-eigen".to_string(),
        owner: OWNER.to_string(),
        repo: REPO.to_string(),
        branch: BRANCH.to_string(),
        base_branch: BASE.to_string(),
    }
}

fn client_for(server: &MockServer) -> GithubClient {
    GithubClient::from_base(Some(&server.uri())).unwrap()
}

/// `GET /repos/{owner}/{repo}` — repo bestaat en het token mag pushen.
async fn mock_repo_gezond(server: &MockServer) {
    mock_repo(server, true).await;
}

async fn mock_repo(server: &MockServer, push: bool) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWNER}/{REPO}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "default_branch": BASE,
            "private": true,
            "permissions": { "push": push },
        })))
        .mount(server)
        .await;
}

/// `GET /repos/{owner}/{repo}/branches/{base}` — de basisbranch bestaat.
async fn mock_basisbranch_gezond(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWNER}/{REPO}/branches/{BASE}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": BASE,
        })))
        .mount(server)
        .await;
}

/// `GET /repos/{owner}/{repo}/git/ref/heads/{branch}` — de traject-branch.
async fn mock_trajectbranch(server: &MockServer, status: u16) {
    let template = if status == 200 {
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "object": { "sha": "0".repeat(40) },
        }))
    } else {
        ResponseTemplate::new(status)
    };
    Mock::given(method("GET"))
        .and(path(format!(
            "/repos/{OWNER}/{REPO}/git/ref/heads/{BRANCH}"
        )))
        .respond_with(template)
        .mount(server)
        .await;
}

/// `GET /repos/{owner}/{repo}/activity` — het verwijderlogboek.
async fn mock_activity(server: &MockServer, deletions: usize) {
    let body: Vec<serde_json::Value> = (0..deletions)
        .map(|_| {
            serde_json::json!({
                "activity_type": "branch_deletion",
                "ref": format!("refs/heads/{BRANCH}"),
            })
        })
        .collect();
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWNER}/{REPO}/activity")))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

/// Classificeer met een gezonde repo + basisbranch als vertrekpunt.
async fn classificeer(
    server: &MockServer,
    origin: TokenOrigin,
) -> (IndexFailureKind, StatusCode, String) {
    let target = target();
    let kind = classify_index_failure(&client_for(server), &target, Some(TOKEN)).await;
    let (status, melding) = index_failure_to_status(kind, &target, origin);
    (kind, status, melding)
}

/// Situatie 1: vers traject, de branch is nooit gemint. De melder uit het
/// oorspronkelijke bugrapport zat hier — en kreeg toen een kale 502.
#[tokio::test]
async fn verse_traject_meldt_dat_het_nog_niet_geinitialiseerd_is() {
    let server = MockServer::start().await;
    mock_repo_gezond(&server).await;
    mock_basisbranch_gezond(&server).await;
    mock_trajectbranch(&server, 404).await;
    mock_activity(&server, 0).await;

    let (kind, status, melding) = classificeer(&server, TokenOrigin::User).await;

    assert_eq!(kind, IndexFailureKind::TrajectBranchMissing);
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(melding.contains("nog niet geïnitialiseerd"), "{melding}");
    assert!(melding.contains(BRANCH), "{melding}");
}

/// Situatie 2: de branch heeft bestaan en is verwijderd. Zelfde 404 op de
/// Refs API als situatie 1 — het activiteitenlogboek maakt het verschil, en
/// alleen hier mag er over mogelijk werkverlies gepraat worden.
#[tokio::test]
async fn verwijderde_trajectbranch_waarschuwt_voor_werkverlies() {
    let server = MockServer::start().await;
    mock_repo_gezond(&server).await;
    mock_basisbranch_gezond(&server).await;
    mock_trajectbranch(&server, 404).await;
    mock_activity(&server, 1).await;

    let (kind, status, melding) = classificeer(&server, TokenOrigin::User).await;

    assert_eq!(kind, IndexFailureKind::TrajectBranchGone);
    assert_eq!(status, StatusCode::GONE);
    assert!(melding.contains("verwijderd"), "{melding}");
    assert!(melding.contains("mogelijk verloren"), "{melding}");
}

/// Situatie 3: de repo staat niet meer op het vastgelegde adres.
#[tokio::test]
async fn verplaatste_repo_meldt_het_adres_uit_het_traject() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWNER}/{REPO}")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let (kind, status, melding) = classificeer(&server, TokenOrigin::User).await;

    assert_eq!(kind, IndexFailureKind::RepoUnavailable);
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(melding.contains(&format!("{OWNER}/{REPO}")), "{melding}");
    assert!(melding.contains("hernoemd"), "{melding}");
}

/// Situatie 4: de basisbranch waar het traject vanaf takt bestaat niet meer.
#[tokio::test]
async fn verdwenen_basisbranch_noemt_de_basisbranch() {
    let server = MockServer::start().await;
    mock_repo_gezond(&server).await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWNER}/{REPO}/branches/{BASE}")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let (kind, status, melding) = classificeer(&server, TokenOrigin::User).await;

    assert_eq!(kind, IndexFailureKind::BaseBranchMissing);
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(melding.contains("basisbranch"), "{melding}");
    assert!(melding.contains(BASE), "{melding}");
}

/// Situatie 5: GitHub weigert het token. Een ingetrokken token wordt op
/// élke call geweigerd — vandaar de 401 op de hele mock. Met het eigen
/// token van de gebruiker is de koppel-flow (428) het antwoord; dat is de
/// enige situatie op dit pad die 428 mag geven. Hetzelfde oordeel over het
/// token van de beheerder stuurt de gebruiker níet die flow in: daar valt
/// voor hem niets te koppelen.
#[tokio::test]
async fn geweigerd_token_stuurt_alleen_de_eigen_koppeling_naar_de_koppel_flow() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let (kind, status, melding) = classificeer(&server, TokenOrigin::User).await;
    assert_eq!(kind, IndexFailureKind::LinkRevoked);
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
    assert!(
        melding.contains("Koppel je GitHub-account opnieuw"),
        "{melding}"
    );
    // Het token zelf hoort nergens in de melding te staan, ook niet een stuk.
    assert!(!melding.contains(TOKEN), "{melding}");
    assert!(!melding.contains(&TOKEN[..6]), "{melding}");

    let (status, melding) = index_failure_to_status(kind, &target(), TokenOrigin::Server);
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(melding.contains("beheerder"), "{melding}");
}

/// Situatie 5, andere smaak: het token is geldig maar de toegang is te
/// smal. Opnieuw koppelen lost dat niet op, dus juist géén 428 — dat zou
/// de gebruiker in een rondje sturen.
#[tokio::test]
async fn te_smalle_toegang_meldt_rechten_en_vermijdt_de_koppel_flow() {
    let server = MockServer::start().await;
    mock_repo(&server, false).await;

    let (kind, status, melding) = classificeer(&server, TokenOrigin::User).await;

    assert_eq!(kind, IndexFailureKind::InsufficientScope);
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_ne!(status, StatusCode::PRECONDITION_REQUIRED);
    assert!(melding.contains("niet toereikend"), "{melding}");

    // Is het het token van de beheerder dat te smal is, dan valt er voor het
    // lid niets te koppelen — een leesbron met een eigen servertoken kijkt
    // sowieso nooit naar een persoonlijke koppeling.
    let (status, melding) = index_failure_to_status(kind, &target(), TokenOrigin::Server);
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(melding.contains("beheerder"), "{melding}");
    assert!(
        !melding.contains("Koppel je GitHub-account"),
        "een servertoken-probleem mag niet naar de koppel-flow wijzen: {melding}"
    );
}

/// GitHub gebruikt 403 óók voor een uitgeputte rate limit, een organisatie
/// die SSO afdwingt en een IP-allowlist die de call blokkeert. Geen van die
/// drie is een ontbrekende of verlopen koppeling, en opnieuw koppelen lost
/// er geen één op — een 428 zou de gebruiker via `apiAuthGuard.js` telkens
/// opnieuw de koppel-flow in sturen, met exact dezelfde 403 als resultaat.
#[tokio::test]
async fn geweigerde_repo_met_403_stuurt_de_gebruiker_niet_de_koppel_flow_in() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWNER}/{REPO}")))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    // De bevestigingslezing krijgt hetzelfde antwoord: het token wordt niet
    // geweigerd, deze repo blijft dicht.
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWNER}/{REPO}/git/ref/heads/{BASE}")))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;

    let (kind, status, melding) = classificeer(&server, TokenOrigin::User).await;

    assert_eq!(kind, IndexFailureKind::InsufficientScope);
    assert_ne!(
        status,
        StatusCode::PRECONDITION_REQUIRED,
        "een 403 van GitHub mag de koppel-flow niet triggeren, got: {melding}"
    );
    assert_eq!(status, StatusCode::FORBIDDEN);
}

/// Een opgebruikte rate limit komt als 403 binnen en is noch een rechten-
/// noch een koppelprobleem: hij lost zichzelf op. De melding moet dus
/// "probeer het straks nog eens" zijn, niet "vraag om toegang".
#[tokio::test]
async fn opgebruikte_rate_limit_meldt_probeer_het_later() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWNER}/{REPO}")))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWNER}/{REPO}/git/ref/heads/{BASE}")))
        .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
            "message": "API rate limit exceeded for user ID 1.",
        })))
        .mount(&server)
        .await;

    let (kind, status, melding) = classificeer(&server, TokenOrigin::User).await;

    assert_eq!(kind, IndexFailureKind::GithubUnreachable);
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(melding.contains("paar minuten"), "{melding}");
}

/// Dezelfde rate limit kan ook één call later toeslaan: de repo-lookup gaat
/// nog goed, de lezing van de basisbranch krijgt de 403. De preflight noemt
/// dat "geen schrijftoegang" — als dat blindelings zou worden overgenomen,
/// kreeg het lid "vraag toegang tot de repo" voor iets wat vanzelf overgaat.
#[tokio::test]
async fn rate_limit_op_de_basisbranch_meldt_ook_probeer_het_later() {
    let server = MockServer::start().await;
    mock_repo_gezond(&server).await;
    let throttled = ResponseTemplate::new(403).set_body_json(serde_json::json!({
        "message": "You have exceeded a secondary rate limit.",
    }));
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWNER}/{REPO}/branches/{BASE}")))
        .respond_with(throttled.clone())
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/repos/{OWNER}/{REPO}/git/ref/heads/{BASE}")))
        .respond_with(throttled)
        .mount(&server)
        .await;

    let (kind, status, melding) = classificeer(&server, TokenOrigin::User).await;

    assert_eq!(kind, IndexFailureKind::GithubUnreachable);
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(melding.contains("paar minuten"), "{melding}");
}

/// Restcategorie: repo, basisbranch én traject-branch zijn alle drie in
/// orde, dus de scan viel om op iets wat deze classificatie niet dekt. Dat
/// wordt expliciet "onbekend" genoemd — met een eigen melding, niet met de
/// generieke terugval.
#[tokio::test]
async fn gezonde_repo_met_gevallen_scan_wordt_expliciet_onbekend() {
    let server = MockServer::start().await;
    mock_repo_gezond(&server).await;
    mock_basisbranch_gezond(&server).await;
    mock_trajectbranch(&server, 200).await;

    let (kind, status, melding) = classificeer(&server, TokenOrigin::User).await;

    assert_eq!(kind, IndexFailureKind::Unknown);
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_ne!(melding, GENERIEKE_TERUGVAL);
    assert!(melding.contains("niet vast te stellen"), "{melding}");
}

/// Zonder token valt er niets te vragen — en er wordt dan ook niets
/// gevraagd: anoniem proberen zou een 404 opleveren en "repo weg" melden,
/// wat pertinent onwaar is voor een privé-repo.
#[tokio::test]
async fn zonder_token_wordt_github_niet_bevraagd() {
    let server = MockServer::start().await;
    let target = target();

    let kind = classify_index_failure(&client_for(&server), &target, None).await;

    assert_eq!(kind, IndexFailureKind::NoCredential);
    assert!(
        server.received_requests().await.unwrap().is_empty(),
        "de classificatie mag zonder token geen enkele GitHub-call doen"
    );
    let (status, melding) = index_failure_to_status(kind, &target, TokenOrigin::Absent);
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(melding.contains("geen GitHub-toegang"), "{melding}");
}

/// GitHub helemaal niet te bereiken: geen diagnose, wel een eerlijk
/// "probeer het straks nog eens" in plaats van een verwijt aan de repo.
#[tokio::test]
async fn onbereikbaar_github_meldt_probeer_het_later() {
    // Poort 1 op loopback: niets luistert, de verbinding wordt geweigerd.
    let client = GithubClient::new()
        .unwrap()
        .with_base_url("http://127.0.0.1:1");
    let target = target();

    let kind = classify_index_failure(&client, &target, Some(TOKEN)).await;
    let (status, melding) = index_failure_to_status(kind, &target, TokenOrigin::User);

    assert_eq!(kind, IndexFailureKind::GithubUnreachable);
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(melding.contains("niet bereikbaar"), "{melding}");
}

/// De melding moet ongeschonden bij de gebruiker aankomen: de bibliotheek
/// kapt alles boven de 300 tekens weg en negeert alles wat op HTML of JSON
/// lijkt. Geldt voor élke classificatie, en geen enkele valt terug op de
/// generieke tekst of lijkt op een andere.
const ALLE_SITUATIES: [IndexFailureKind; 9] = [
    IndexFailureKind::TrajectBranchMissing,
    IndexFailureKind::TrajectBranchGone,
    IndexFailureKind::RepoUnavailable,
    IndexFailureKind::BaseBranchMissing,
    IndexFailureKind::LinkRevoked,
    IndexFailureKind::InsufficientScope,
    IndexFailureKind::GithubUnreachable,
    IndexFailureKind::NoCredential,
    IndexFailureKind::Unknown,
];

#[test]
fn elke_melding_overleeft_de_grens_van_de_bibliotheek() {
    let target = target();
    let alle = ALLE_SITUATIES;

    let mut meldingen = Vec::new();
    let mut namen = Vec::new();
    for kind in alle {
        for origin in [TokenOrigin::User, TokenOrigin::Server, TokenOrigin::Absent] {
            let (status, melding) = index_failure_to_status(kind, &target, origin);
            assert!(
                melding.chars().count() <= MELDING_MAX,
                "{kind} is {} tekens: {melding}",
                melding.chars().count()
            );
            assert!(!melding.starts_with('<'), "{kind}: {melding}");
            assert!(!melding.starts_with('{'), "{kind}: {melding}");
            assert_ne!(
                melding, GENERIEKE_TERUGVAL,
                "{kind} valt terug op de generieke tekst"
            );
            // 428 blijft van de koppel-flow: alleen een geweigerde eigen
            // koppeling mag hem geven.
            if status == StatusCode::PRECONDITION_REQUIRED {
                assert_eq!(kind, IndexFailureKind::LinkRevoked);
                assert_eq!(origin, TokenOrigin::User);
            }
        }
        meldingen.push(index_failure_to_status(kind, &target, TokenOrigin::User).1);
        namen.push(kind.as_str());
    }

    let unieke_meldingen: std::collections::HashSet<_> = meldingen.iter().collect();
    assert_eq!(
        unieke_meldingen.len(),
        meldingen.len(),
        "twee situaties delen een melding"
    );
    let unieke_namen: std::collections::HashSet<_> = namen.iter().collect();
    assert_eq!(
        unieke_namen.len(),
        namen.len(),
        "twee situaties delen een logwaarde"
    );
}

/// Dezelfde grens, maar met de langste namen die GitHub toestaat: een
/// eigenaar van 39 tekens, een repo van 100 en een basisbranch die (anders
/// dan de traject-branch) door een mens is ingetypt. Een lange repo-naam mag
/// de melding niet over de 300 duwen — dan kapt de bibliotheek hem niet af
/// maar vervangt hem integraal door precies de generieke tekst die deze
/// classificatie moest wegnemen.
#[test]
fn zelfs_de_langst_mogelijke_repo_naam_kapt_de_melding_niet_weg() {
    let target = OwnSourceTarget {
        traject_id: Uuid::nil(),
        source_id: "traject-eigen".to_string(),
        owner: "o".repeat(39),
        repo: "r".repeat(100),
        // De traject-branch is afgeleid (slug van 32 + korte id), de
        // basisbranch komt uit een invoerveld; beide ruim genomen.
        branch: format!("traject/{}-1a2b3c4d", "s".repeat(32)),
        base_branch: "b".repeat(120),
    };

    for kind in ALLE_SITUATIES {
        for origin in [TokenOrigin::User, TokenOrigin::Server, TokenOrigin::Absent] {
            let (_, melding) = index_failure_to_status(kind, &target, origin);
            assert!(
                melding.chars().count() <= MELDING_MAX,
                "{kind} is {} tekens en valt daarmee terug op '{GENERIEKE_TERUGVAL}': {melding}",
                melding.chars().count()
            );
        }
    }
}
