//! NAPP backend — orchestratielaag rond de regelrecht-engine.
//!
//! Axum API met sessie-gebaseerde auth: echte SSO Rijk (OIDC) voor
//! beoordelaars wanneer geconfigureerd, gemockte eHerkenning voor aanvragers.

mod beheer;
mod bezwaar;
mod claim;
mod db;
mod engine;
mod handelsregister;
mod handlers;
mod machtiging;
mod register;
mod rekening;
mod state;

use std::sync::Arc;

use axum::Router;
use axum::extract::Request;
use axum::routing::{get, post, put};
use regelrecht_auth::OidcAppState;
use sqlx::sqlite::SqlitePoolOptions;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_memory_store::MemoryStore;

use state::{AppState, LawCorpus};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Eén crypto-backend voor het hele proces. sqlx en reqwest laten de keuze
    // aan rustls, en rustls kiest alleen zelf als er precies één backend
    // meegecompileerd is. Zonder deze regel valt de eerste HTTPS-aanroep om
    // met "No provider set" — zie packages/auth.
    regelrecht_auth::install_crypto_provider();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // regelrecht_auth logt de werkelijke oorzaak van OIDC-fouten
                // (token exchange, ID-token-verificatie); zonder die target
                // is een 500 op /auth/callback niet te herleiden.
                "napp_backend=info,tower_http=info,regelrecht_auth=debug".into()
            }),
        )
        .init();

    let corpus = Arc::new(LawCorpus::load()?);
    // Fail-loud contract: elke output waarnaar de orchestratie verwijst
    // moet in de geladen corpus bestaan, anders start de applicatie niet.
    engine::valideer_contract(&corpus)?;
    tracing::info!("wetscorpus geladen en contract wet↔uitvoering gevalideerd");

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:napp.db?mode=rwc".to_string());
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;
    db::init(&pool).await?;
    register::seed_if_empty(&pool).await?;
    tracing::info!(database = %database_url, "database gereed");

    // OIDC (SSO Rijk) — alleen actief wanneer de OIDC_* variabelen gezet zijn.
    let mut oidc_config = regelrecht_auth::parse_oidc_from_env()
        .map_err(|e| anyhow::anyhow!("OIDC-configuratie ongeldig: {e}"))?;
    // ZAD provisiont per app een eigen Keycloak-realm; wie tot het realm is
    // toegelaten, mag de beoordelingsomgeving in. Zonder expliciete
    // OIDC_REQUIRED_ROLE eist de auth-crate de rol `allowed-user`, die in
    // ZAD-realms niet bestaat — val dan terug op de composietrol die elke
    // realm-gebruiker draagt.
    let role_overridden = std::env::var("OIDC_REQUIRED_ROLE")
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    if !role_overridden
        && let Some(config) = oidc_config.as_mut()
        && let Some(role) = realm_membership_role(&config.issuer_url)
    {
        tracing::info!(
            role,
            "OIDC_REQUIRED_ROLE niet gezet; realm-lidmaatschap volstaat als toegangseis"
        );
        config.required_role = role;
    }
    let (oidc_client, end_session_url) = if let Some(ref config) = oidc_config {
        match regelrecht_auth::discover_client(config).await {
            Ok(result) => {
                tracing::info!("SSO Rijk (OIDC) actief");
                (Some(Arc::new(result.client)), result.end_session_url)
            }
            Err(e) => {
                tracing::error!(error = %e, "OIDC-discovery mislukt");
                return Err(anyhow::anyhow!("OIDC-discovery mislukt: {e}"));
            }
        }
    } else {
        tracing::warn!("OIDC niet geconfigureerd — mock-SSO-login actief (alleen voor demo)");
        (None, None)
    };

    let procedure = Arc::new(engine::beschikking_procedure(&corpus.wpp)?);
    let bezwaar_procedure = Arc::new(engine::bezwaar_procedure(&corpus.awb)?);

    let app_state = AppState {
        pool,
        corpus,
        procedure,
        bezwaar_procedure,
        oidc_client,
        oidc_config,
        end_session_url,
        base_url: std::env::var("BASE_URL").ok(),
        // Redirects volgen opent de token-exchange voor SSRF; de oauth2-crate
        // schrijft Policy::none voor (zelfde opzet als regelrecht editor-api).
        http_client: reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(10))
            .build()?,
    };

    let auth_routes = regelrecht_auth::auth_routes::<AppState>();

    let mut api = Router::new()
        .route("/api/me", get(handlers::me))
        .route("/api/eherkenning/login", post(handlers::eherkenning_login))
        .route(
            "/api/eherkenning/logout",
            post(handlers::eherkenning_logout),
        )
        .route(
            "/api/eherkenning/machtigingen",
            get(machtiging::machtigingen),
        )
        .route("/api/mijn-registratie", get(handlers::mijn_registratie))
        .route(
            "/api/mijn-rekening",
            get(rekening::get_mijn_rekening).put(rekening::put_mijn_rekening),
        )
        // Claim-flow: een rechtspersoon koppelt zichzelf aan een
        // ONGEKOPPELDE aanduiding uit de uitslag (zie claim.rs).
        .route("/api/claim/aanduidingen", get(claim::list_aanduidingen))
        .route("/api/claim", post(claim::create_claim))
        .route("/api/mijn-claim", get(claim::mijn_claim))
        .route("/api/register/demo", get(handlers::register_demo))
        .route("/api/aanvragen", post(handlers::create_aanvraag))
        .route("/api/aanvragen/proef", post(handlers::proef_aanspraken))
        .route("/api/aanvragen", get(handlers::list_aanvragen))
        .route("/api/mijn-aanvragen", get(handlers::list_mijn_aanvragen))
        .route("/api/mijn-aanvragen/{id}", get(handlers::get_mijn_aanvraag))
        .route("/api/aanvragen/{id}", get(handlers::get_aanvraag))
        .route(
            "/api/aanvragen/{id}/proefberekening",
            post(handlers::proefberekening),
        )
        .route(
            "/api/aanvragen/{id}/besluit",
            post(handlers::stel_besluit_vast),
        )
        .route(
            "/api/aanvragen/{id}/bekendmaking",
            post(handlers::bekendmaking),
        )
        .route(
            "/api/betaalopdrachten",
            get(handlers::list_betaalopdrachten),
        )
        .route(
            "/api/betaalopdrachten/{id}/uitbetalen",
            post(handlers::betaal_uit),
        )
        // Bezwaar (AWB hoofdstuk 6/7, zie bezwaar.rs).
        .route(
            "/api/besluiten/{id}/bezwaar",
            post(bezwaar::dien_bezwaar_in),
        )
        .route("/api/bezwaren/{id}/herstel", put(bezwaar::herstel_bezwaar))
        .route("/api/bezwaren", get(bezwaar::list_bezwaren))
        .route("/api/bezwaren/{id}/horen", post(bezwaar::registreer_horen))
        .route(
            "/api/bezwaren/{id}/beslissen",
            post(bezwaar::beslis_bezwaar),
        )
        .route("/api/register", get(handlers::register))
        .route("/api/register/statistieken", get(handlers::statistieken))
        // Partijregister-beheer (beoordelaar-only, zie beheer.rs). De
        // uitslagen zijn referentiedata (Kiesraad/CBS) en kennen bewust
        // geen mutatie-endpoints; koppelingen ontstaan via de claim-flow.
        .route("/api/beheer/partijen", get(beheer::list_partijen))
        .route(
            "/api/beheer/partijen/{kvk}",
            get(beheer::get_partij).put(beheer::update_partij),
        )
        .route("/api/beheer/claims", get(claim::beheer_list_claims))
        .route(
            "/api/beheer/claims/{id}/bevestig",
            post(claim::bevestig_claim),
        )
        .route(
            "/api/beheer/claims/{id}/afwijzen",
            post(claim::wijs_claim_af),
        );

    // Demo-login blijft ook naast echte OIDC beschikbaar: dit is een PoC met
    // fictieve data, en bezoekers zonder Rijksaccount moeten de
    // beoordelaarsflow kunnen demonstreren. Uitzetten kan met
    // NAPP_MOCK_SSO=0 (bijvoorbeeld zodra de omgeving niet meer publiek
    // gedemonstreerd wordt).
    let mock_sso = std::env::var("NAPP_MOCK_SSO")
        .map(|v| v != "0")
        .unwrap_or(true);
    if mock_sso {
        if app_state.is_auth_enabled() {
            tracing::warn!(
                "mock-SSO-login actief naast echte OIDC (demo); zet NAPP_MOCK_SSO=0 om dit uit te schakelen"
            );
        }
        api = api
            .route("/api/sso/mock-login", post(handlers::sso_mock_login))
            // Demo-gereedschap voor het seedscript: dossiers wissen en het
            // register herseeden. Bewust geen UI-knop (zie beheer::demo_reset).
            .route("/api/beheer/demo/reset", post(beheer::demo_reset));
    }

    // Secure cookies wanneer OIDC actief is (de omgeving draait dan achter
    // TLS); lokaal zonder OIDC werkt de dev-opstelling over http.
    let session_layer = SessionManagerLayer::new(MemoryStore::default())
        .with_expiry(Expiry::OnInactivity(time::Duration::hours(8)))
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_http_only(true)
        .with_secure(app_state.is_auth_enabled());

    let static_dir = std::env::var("NAPP_STATIC_DIR").unwrap_or_else(|_| "frontend/dist".into());
    let index_file = format!("{static_dir}/index.html");

    // De wortel expliciet, want onder `nest` bereikt "/" de fallback van de
    // genestelde router niet: `/napp/` gaf 404 terwijl `/napp/index.html` het
    // deed. Dat is de startpagina van de poc — het adres dat je doorstuurt.
    let root_index = ServeFile::new(&index_file);
    let inner = Router::new()
        .route_service("/", root_index)
        .route("/health", get(handlers::health))
        .merge(auth_routes)
        .merge(api)
        .with_state(app_state)
        .fallback_service(ServeDir::new(&static_dir).not_found_service(ServeFile::new(&index_file)))
        .layer(session_layer)
        // De drie portalen naar hun eigen entry. Dit mág een layer zijn: het
        // herschrijft binnen dezelfde router, vóór de fallback die het opvangt.
        .layer(axum::middleware::map_request(rewrite_portal_path));

    // Het voorvoegsel hoort in de router, niet in een layer. `Router::layer`
    // wikkelt de routes die er al zijn, maar het matchen van het pad gebeurt
    // dáárvoor: een URI die een layer herschrijft komt te laat om nog een
    // andere route te kiezen. `/napp/api/me` gaf daardoor 404 terwijl
    // `/api/me` werkte, en `/napp/` leek te werken omdat de statische fallback
    // hem opving — precies het soort "werkt half" dat je pas ziet als je een
    // API-pad probeert.
    let base = base_path();
    let app = if base == "/" {
        inner
    } else {
        // `nest` vangt `/napp` (→ de "/"-route hierboven) en `/napp/<iets>`,
        // maar `/napp/` zelf wordt een lege rest die axum niet routeert: die
        // gaf 404 terwijl `/napp` en `/napp/index.html` het deden. Daarom
        // eerst een redirect van de slash-vorm naar de kale prefix, in de
        // buitenste router waar het pad nog heel is.
        let prefix = base.trim_end_matches('/').to_string();
        let naar = prefix.clone();
        Router::new()
            .route(
                &base,
                get(move || {
                    let naar = naar.clone();
                    async move { axum::response::Redirect::permanent(&naar) }
                }),
            )
            .nest(&prefix, inner)
    }
    .layer(TraceLayer::new_for_http());

    let port: u16 = std::env::var("NAPP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8400);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("NAPP-backend luistert op http://localhost:{port}");
    axum::serve(listener, app).await?;
    Ok(())
}

/// De Keycloak-composietrol die elk lid van een realm draagt
/// (`default-roles-<realm>`), afgeleid uit de issuer-URL. Geeft `None` voor
/// issuers zonder `/realms/`-pad (niet-Keycloak), zodat de strikte default
/// van de auth-crate dan blijft gelden.
fn realm_membership_role(issuer_url: &str) -> Option<String> {
    let realm = issuer_url
        .split("/realms/")
        .nth(1)?
        .trim_end_matches('/')
        .split('/')
        .next()?;
    if realm.is_empty() {
        return None;
    }
    Some(format!("default-roles-{realm}"))
}

/// Dezelfde portal-rewrites als de vite-dev-server: extensieloze paden onder
/// /aanvrager en /beoordelaar wijzen naar de entry-html van dat portaal,
/// zodat ServeDir niet terugvalt op index.html (het publieke portaal).
/// Onder welk pad napp zichzelf serveert. Los is dat `/`; achter het
/// poc-portaal `/napp/`, want de frontend wordt met die `base` gebouwd en
/// verwijst dus met dat voorvoegsel naar zijn eigen assets. Het portaal stuurt
/// het pad ongewijzigd door — de prefix afknippen zou napp's eigen HTML naar
/// bestanden laten wijzen die het portaal dan niet vindt.
pub fn base_path() -> String {
    let ruw = std::env::var("NAPP_BASE_PATH").unwrap_or_else(|_| "/".to_string());
    let met_slash = if ruw.starts_with('/') {
        ruw
    } else {
        format!("/{ruw}")
    };
    if met_slash.ends_with('/') {
        met_slash
    } else {
        format!("{met_slash}/")
    }
}

/// Welk van de drie portalen dit pad bedient, als het er een is.
///
/// `base` is het voorvoegsel waaronder napp draait; alles daarvoor hoort niet
/// bij ons. Zonder die stap zoekt `/napp/aanvrager` niets en valt de
/// SPA-fallback terug op het publieke portaal — een diepe link naar het
/// subsidieportaal landt dan stil op de verkeerde applicatie.
fn portal_rewrite_target_met_base(path: &str, base: &str) -> Option<&'static str> {
    if path.contains('.') {
        return None;
    }
    // `/napp/` → rest `/aanvrager`; `/napp` zelf → rest `/`.
    let rest = match path.strip_prefix(base.trim_end_matches('/')) {
        Some("") => "/",
        Some(r) if r.starts_with('/') => r,
        Some(_) | None => return None,
    };
    if rest == "/aanvrager" || rest.starts_with("/aanvrager/") {
        Some("/aanvrager.html")
    } else if rest == "/beoordelaar" || rest.starts_with("/beoordelaar/") {
        Some("/beoordelaar.html")
    } else {
        None
    }
}

/// Het pad zoals de router het kent: zonder het voorvoegsel waaronder napp
/// draait, en met de drie portalen naar hun eigen entry herschreven.
///
/// Het voorvoegsel eraf halen gebeurt hier en niet in elke route: de app kent
/// honderd `/api/...`-paden, en die allemaal van een variabele prefix voorzien
/// zou elke route van zijn leesbaarheid ontdoen om één env-var te dienen. De
/// frontend wordt met dezelfde prefix gebouwd, dus binnen dit proces is alles
/// weer gewoon `/api/...`.
fn intern_pad(path: &str, base: &str) -> Option<String> {
    if let Some(target) = portal_rewrite_target_met_base(path, base) {
        return Some(target.to_string());
    }
    // Onder `nest` komt de wortel van de poc binnen als "" of "/", en axum
    // stuurt dat niet naar de fallback van de genestelde router. Zonder deze
    // regel geeft `/napp/` een 404 terwijl `/napp/index.html` werkt — de
    // startpagina van de poc, precies het adres dat je doorstuurt.
    if path.is_empty() || path == "/" {
        return Some("/index.html".to_string());
    }
    if base == "/" {
        return None;
    }
    match path.strip_prefix(base.trim_end_matches('/')) {
        Some("") => Some("/".to_string()),
        Some(rest) if rest.starts_with('/') => Some(rest.to_string()),
        _ => None,
    }
}

async fn rewrite_portal_path(mut req: Request) -> Request {
    // Binnen de genestelde router is het voorvoegsel er al af, dus hier geldt
    // altijd basis '/'.
    if let Some(doel) = intern_pad(req.uri().path(), "/") {
        let query = req
            .uri()
            .query()
            .map(|q| format!("?{q}"))
            .unwrap_or_default();
        let mut parts = req.uri().clone().into_parts();
        if let Ok(pad) = format!("{doel}{query}").parse() {
            parts.path_and_query = Some(pad);
            if let Ok(uri) = axum::http::Uri::from_parts(parts) {
                *req.uri_mut() = uri;
            }
        }
    }
    req
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::realm_membership_role;

    /// De oude wrapper is weg: binnen de genestelde router is de basis altijd
    /// "/", en dat is precies wat deze tests aannemen.
    fn portal_rewrite_target(path: &str) -> Option<&'static str> {
        super::portal_rewrite_target_met_base(path, "/")
    }

    #[test]
    fn realm_rol_uit_keycloak_issuer() {
        assert_eq!(
            realm_membership_role("https://keycloak.rijksapp.nl/realms/napp-avm-odcn-production")
                .as_deref(),
            Some("default-roles-napp-avm-odcn-production")
        );
        assert_eq!(
            realm_membership_role("https://keycloak.example.com/realms/demo/").as_deref(),
            Some("default-roles-demo")
        );
    }

    #[test]
    fn geen_realm_rol_voor_niet_keycloak_issuer() {
        assert_eq!(realm_membership_role("https://idp.example.com"), None);
        assert_eq!(
            realm_membership_role("https://keycloak.example.com/realms/"),
            None
        );
    }

    #[test]
    fn portal_paden_wijzen_naar_eigen_entry() {
        assert_eq!(portal_rewrite_target("/aanvrager"), Some("/aanvrager.html"));
        assert_eq!(
            portal_rewrite_target("/aanvrager/dossier/42"),
            Some("/aanvrager.html")
        );
        assert_eq!(
            portal_rewrite_target("/beoordelaar/"),
            Some("/beoordelaar.html")
        );
    }

    /// Achter het poc-portaal draait napp onder /napp/.
    ///
    /// Zonder het afknippen van dat voorvoegsel matcht `/napp/aanvrager` niets
    /// en valt de SPA-fallback terug op het publieke portaal: een diepe link
    /// naar het subsidieportaal landt dan stil op de verkeerde applicatie.
    #[test]
    fn portal_paden_werken_ook_onder_een_voorvoegsel() {
        use super::portal_rewrite_target_met_base as doel;
        assert_eq!(doel("/napp/aanvrager", "/napp/"), Some("/aanvrager.html"));
        assert_eq!(
            doel("/napp/aanvrager/dossier/42", "/napp/"),
            Some("/aanvrager.html")
        );
        assert_eq!(
            doel("/napp/beoordelaar/", "/napp/"),
            Some("/beoordelaar.html")
        );
        // De wortel van de poc zelf is het publieke portaal, geen van beide.
        assert_eq!(doel("/napp/", "/napp/"), None);
        assert_eq!(doel("/napp", "/napp/"), None);
        // Buiten het voorvoegsel is niets van ons — ook niet als het pad er
        // toevallig op lijkt.
        assert_eq!(doel("/aanvrager", "/napp/"), None);
        assert_eq!(doel("/nappx/aanvrager", "/napp/"), None);
    }

    /// Achter het portaal komt elk pad met /napp/ ervoor binnen; intern moet
    /// het weer gewoon /api/... zijn, anders matcht geen enkele route.
    #[test]
    fn het_voorvoegsel_wordt_afgeknipt() {
        use super::intern_pad;
        assert_eq!(
            intern_pad("/napp/api/me", "/napp/").as_deref(),
            Some("/api/me")
        );
        assert_eq!(intern_pad("/napp/", "/napp/").as_deref(), Some("/"));
        assert_eq!(intern_pad("/napp", "/napp/").as_deref(), Some("/"));
        assert_eq!(
            intern_pad("/napp/aanvrager/x", "/napp/").as_deref(),
            Some("/aanvrager.html")
        );
        // Los draaiend verandert er niets aan het pad.
        assert_eq!(intern_pad("/api/me", "/"), None);
        // Buiten het voorvoegsel blijft alles ongemoeid.
        assert_eq!(intern_pad("/api/me", "/napp/"), None);
    }

    #[test]
    fn overige_paden_blijven_ongemoeid() {
        assert_eq!(portal_rewrite_target("/"), None);
        assert_eq!(portal_rewrite_target("/register"), None);
        assert_eq!(portal_rewrite_target("/aanvrager.html"), None);
        assert_eq!(portal_rewrite_target("/aanvragers"), None);
        assert_eq!(
            portal_rewrite_target("/wasm/pkg/regelrecht_engine.js"),
            None
        );
        assert_eq!(portal_rewrite_target("/api/aanvragen"), None);
    }
}
