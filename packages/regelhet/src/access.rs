//! Who may do what: the roles of the server, by name, in one place, and the
//! person a request comes from ([`Principal`]).
//!
//! The roles are Keycloak realm roles, all named with [`ROLE_PREFIX`]
//! because the realm is shared with the editor:
//!
//! - [`CITIZEN`]: a citizen, who submits for themself and reads their own
//!   dossier, identified by a claim of their account ([`Citizen`]);
//! - a ladder for employees ([`Level`]): reader < decider < admin.
//!   Holding a step holds the steps under it, whether or not Keycloak gives
//!   the lower roles too. A role that is not on the ladder gives no step,
//!   so a token that still carries the former `regelhet-behandelaar`
//!   counts only for the other roles it holds.
//!
//! An employee works for the executor of the case, the one cell the server
//! runs the process for ([`crate::service::Service::executor`]), so a step
//! of the ladder is all an employee needs. The other cells of the case (the
//! GGD that advises, the court that decides on an appeal) have no employees
//! here: what they do, an admin enters in the admin panel on their behalf.
//!
//! The rules on top of them live in the API ([`crate::api`]): which route
//! asks which level, and which records it reaches.
//!
//! A principal comes from one of four channels ([`Channel`]): a browser
//! session after the login through Keycloak or the test login, read here
//! from the session; a session of FSC voor Burgers, which
//! [`crate::fsc4b`] turns into a principal with the citizen's role only; or
//! the front of the organisation the process belongs to, which hands over a
//! browser with a signed statement (a session marked as such, written by
//! [`crate::login`]) or calls the API with one in the header
//! `Authorization: Organisatie <statement>`, for a `GET` only
//! ([`crate::organisatie::statement`]).
//! What follows the principal does not ask which channel it was; the store
//! keeps the channel beside each record ([`Principal::actor`]).
//!
//! A principal holds only the roles its instance admits
//! ([`crate::surface::Surface::admitted_roles`]): on the instance for
//! citizens never an employee's role, on the back office never the
//! citizen's, whatever the token or the test login gave the session. An
//! account with only the citizen's role is then nobody on the back office,
//! and every route there refuses it.

use std::collections::{BTreeMap, BTreeSet};

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::{Method, StatusCode, header};
use regelrecht_auth::{
    SESSION_KEY_AUTHENTICATED, SESSION_KEY_CLAIMS, SESSION_KEY_NAME, SESSION_KEY_ROLES,
    SESSION_KEY_SUB,
};
use serde_json::Value;
use tower_sessions::Session;

use crate::api::ApiError;
use crate::login::{Host, OrganisationMark, SESSION_KEY_ORGANISATION};
use crate::organisatie::statement::Accepted;
use crate::store::{Actor, Channel};
use crate::surface::Surface;

/// The scheme of the header `Authorization` with a statement of the
/// organisation.
pub const ORGANISATION_SCHEME: &str = "Organisatie";

/// What every role of this server starts with.
pub const ROLE_PREFIX: &str = "regelhet-";

/// A citizen: submits for themself and reads their own dossier.
pub const CITIZEN: &str = "regelhet-burger";

/// A step on the ladder of employees, lowest first: each may do what the
/// steps under it may.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Reads the executor's chronicle.
    Reader,
    /// Records facts, sends copies of decisions and takes decisions for the
    /// executor.
    Decider,
    /// Sets the clock in test mode, and enters in the admin panel what an
    /// external party of the case did.
    Admin,
}

impl Level {
    /// Every level, lowest first.
    pub const ALL: [Self; 3] = [Self::Reader, Self::Decider, Self::Admin];

    /// The name of this level: its role without [`ROLE_PREFIX`].
    pub fn name(self) -> &'static str {
        let role = self.role();
        role.strip_prefix(ROLE_PREFIX).unwrap_or(role)
    }

    /// The realm role of this level.
    pub fn role(self) -> &'static str {
        match self {
            Self::Reader => "regelhet-lezer",
            Self::Decider => "regelhet-beslisser",
            Self::Admin => "regelhet-admin",
        }
    }
}

/// Whoever a request comes from: the account of its session, with its
/// realm roles and the claims the login kept. Extracting it answers 401
/// when the session holds no login.
#[derive(Debug, Clone)]
pub struct Principal {
    sub: String,
    roles: BTreeSet<String>,
    claims: BTreeMap<String, Value>,
    channel: Channel,
    /// How the account was logged in: the channel itself, except through the
    /// organisation, which says how it logged the account in.
    login: Channel,
    /// The name the login gave, if it gave one.
    name: Option<String>,
}

/// The means a citizen logged in with on `channel`, as an application
/// states it (`login_method` in the UI spec): the simulated DigiD of the
/// test login counts as DigiD, and that login exists in test mode only;
/// Keycloak and FSC voor Burgers go by their own name. An admin entering an
/// external party's act logs in as no citizen and states none, and neither
/// does the server when its policy records.
pub fn login_method(channel: Channel) -> Option<&'static str> {
    match channel {
        Channel::TestLogin => Some("DigiD"),
        Channel::Keycloak => Some("Keycloak"),
        Channel::Fsc4b => Some("FSC voor Burgers"),
        Channel::AdminPanel | Channel::Policy | Channel::Link | Channel::Organisation => None,
    }
}

impl Principal {
    /// A principal from what a session of `channel` holds.
    pub fn new(
        sub: String,
        roles: BTreeSet<String>,
        claims: BTreeMap<String, Value>,
        channel: Channel,
    ) -> Self {
        Self {
            sub,
            roles,
            claims,
            channel,
            login: channel,
            name: None,
        }
    }

    /// The principal of a statement of the organisation, on an instance of
    /// `surface`: the account `<organisation id>:<sub>`, with only the roles
    /// both the side the statement is for and the instance admit. One role
    /// ladder holds for the whole organisation, so a decider at the front is
    /// a decider here.
    pub fn of_organisation(accepted: Accepted, surface: Surface) -> Self {
        let Accepted {
            statement,
            side,
            issuer,
        } = accepted;
        let roles = surface.admitted_roles(side.surface().admitted_roles(statement.roles));
        let mut principal = Self::new(
            format!("{issuer}:{}", statement.sub),
            roles,
            statement.claims,
            Channel::Organisation,
        )
        .with_name(statement.name);
        principal.login = statement.login;
        principal
    }

    /// How the account was logged in, for the means an application states
    /// ([`login_method`]).
    pub fn login_channel(&self) -> Channel {
        self.login
    }

    /// The realm roles the principal holds here.
    pub fn roles(&self) -> &BTreeSet<String> {
        &self.roles
    }

    /// The claims the login kept.
    pub fn claims(&self) -> &BTreeMap<String, Value> {
        &self.claims
    }

    /// The principal with the name the login gave.
    pub fn with_name(mut self, name: Option<String>) -> Self {
        self.name = name.filter(|n| !n.trim().is_empty());
        self
    }

    /// The name the login gave, if it gave one.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// A citizen of the FSC voor Burgers channel: the account `sub`, with the
    /// citizen's role and no other, identified by `subject` under the claim
    /// `claim`. A session of that channel never holds an employee's role.
    pub fn fsc4b_citizen(sub: String, claim: &str, subject: &str) -> Self {
        Self::new(
            sub,
            BTreeSet::from([CITIZEN.to_string()]),
            BTreeMap::from([(claim.to_string(), Value::from(subject))]),
            Channel::Fsc4b,
        )
    }

    /// The account's subject: the OIDC `sub`, which the store keeps beside
    /// every record this principal makes.
    pub fn sub(&self) -> &str {
        &self.sub
    }

    /// The channel the principal came in by.
    pub fn channel(&self) -> Channel {
        self.channel
    }

    /// The principal as the store keeps it beside a record.
    pub fn actor(&self) -> Actor {
        Actor {
            account: self.sub.clone(),
            channel: self.channel,
        }
    }

    /// Whether the principal holds a role of this server: a citizen or an
    /// employee.
    pub fn is_user(&self) -> bool {
        self.is_citizen() || self.level().is_some()
    }

    /// Whether the principal holds the citizen's role.
    pub fn is_citizen(&self) -> bool {
        self.roles.contains(CITIZEN)
    }

    /// The highest step of the ladder the principal holds, if any.
    pub fn level(&self) -> Option<Level> {
        Level::ALL
            .into_iter()
            .rev()
            .find(|l| self.roles.contains(l.role()))
    }

    /// Whether the principal holds at least `level`.
    pub fn has(&self, level: Level) -> bool {
        self.level().is_some_and(|held| held >= level)
    }

    /// The principal as a citizen identified by the claim `claim`.
    ///
    /// # Errors
    ///
    /// 403 when the principal is no citizen, or the account has no such
    /// claim, or holds it as anything but non-empty text.
    pub fn citizen(&self, claim: &str) -> Result<Citizen, ApiError> {
        if !self.is_citizen() {
            return Err(forbidden());
        }
        match self.claims.get(claim) {
            Some(Value::String(subject)) if !subject.trim().is_empty() => Ok(Citizen {
                sub: self.sub.clone(),
                subject: subject.clone(),
            }),
            _ => {
                tracing::warn!(sub = %self.sub, claim, "a citizen without an identifying claim");
                Err(ApiError::new(
                    StatusCode::FORBIDDEN,
                    "your account carries no identity to act as a citizen with",
                ))
            }
        }
    }

    /// Allow when the principal holds at least `level`.
    ///
    /// # Errors
    ///
    /// 403 otherwise.
    pub fn require(&self, level: Level) -> Result<(), ApiError> {
        if self.has(level) {
            Ok(())
        } else {
            tracing::warn!(sub = %self.sub, level = level.role(), "refused");
            Err(forbidden())
        }
    }

    /// The principal as the store keeps it beside a record it enters in the
    /// admin panel for an external party: its own account, through the
    /// panel's channel.
    pub fn admin_panel_actor(&self) -> Actor {
        Actor {
            account: self.sub.clone(),
            channel: Channel::AdminPanel,
        }
    }
}

/// A citizen: the account and the identifier its claim holds, which fills
/// the login field of an application and picks out its dossier.
#[derive(Debug, Clone)]
pub struct Citizen {
    sub: String,
    subject: String,
}

impl Citizen {
    pub fn sub(&self) -> &str {
        &self.sub
    }

    /// The identifier of the citizen, from the claim of their account.
    pub fn subject(&self) -> &str {
        &self.subject
    }
}

/// The answer to a principal who may not do what they asked.
pub fn forbidden() -> ApiError {
    ApiError::new(StatusCode::FORBIDDEN, "you may not do this")
}

impl<S: Host> FromRequestParts<S> for Principal {
    type Rejection = ApiError;

    /// The principal of the request's browser session, with the roles the
    /// instance's surface admits; the channel is the server's login, or the
    /// organisation for a session it handed over. Without a session, the
    /// statement of the organisation in the header, for a `GET`.
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|(status, message)| ApiError::new(status, message))?;
        if let Some(principal) = of_session(&session, state).await? {
            return Ok(principal);
        }
        let Some(token) = statement_header(parts) else {
            return Err(unauthenticated());
        };
        let Some(acceptor) = state.acceptor() else {
            return Err(unauthenticated());
        };
        if parts.method != Method::GET {
            tracing::warn!(method = %parts.method, "a statement of the organisation that acts");
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "a statement of the organisation only reads",
            ));
        }
        match acceptor.accept(token, crate::fsc4b::now_unix()).await {
            Ok(accepted) => Ok(Self::of_organisation(accepted, state.surface())),
            Err(e) => {
                tracing::warn!(error = %e, "a statement of the organisation refused");
                Err(unauthenticated())
            }
        }
    }
}

fn unauthenticated() -> ApiError {
    ApiError::new(StatusCode::UNAUTHORIZED, "log in first")
}

/// The statement in `Authorization: Organisatie <statement>`, if any.
fn statement_header(parts: &Parts) -> Option<&str> {
    let value = parts.headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    (scheme == ORGANISATION_SCHEME).then(|| token.trim())
}

/// The principal a browser session holds, or none when it holds no login.
async fn of_session<S: Host>(session: &Session, state: &S) -> Result<Option<Principal>, ApiError> {
    let read_error = |e: tower_sessions::session::Error| {
        tracing::error!(error = %e, "reading the session");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "the server failed on this request; the cause is in its log",
        )
    };
    let authenticated: Option<bool> = session
        .get(SESSION_KEY_AUTHENTICATED)
        .await
        .map_err(read_error)?;
    if authenticated != Some(true) {
        return Ok(None);
    }
    let sub: Option<String> = session.get(SESSION_KEY_SUB).await.map_err(read_error)?;
    let Some(sub) = sub.filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let roles: Option<BTreeSet<String>> =
        session.get(SESSION_KEY_ROLES).await.map_err(read_error)?;
    let claims: Option<BTreeMap<String, Value>> =
        session.get(SESSION_KEY_CLAIMS).await.map_err(read_error)?;
    // The name only fills a field the system shows; a session that does
    // not give it leaves that field empty, as `GET /api/ik` does.
    let name: Option<String> = session.get(SESSION_KEY_NAME).await.unwrap_or_else(|e| {
        tracing::warn!(error = %e, "reading the name from the session");
        None
    });
    // A session the organisation handed over carries its mark, for the
    // account it handed over; a later login on the same session writes
    // another account, and the mark no longer counts.
    let mark: Option<OrganisationMark> = session
        .get(SESSION_KEY_ORGANISATION)
        .await
        .map_err(read_error)?;
    let mark = mark.filter(|m| m.sub == sub);
    let mut principal = Principal::new(
        sub,
        state.surface().admitted_roles(roles.unwrap_or_default()),
        claims.unwrap_or_default(),
        if mark.is_some() {
            Channel::Organisation
        } else {
            state.session_channel()
        },
    )
    .with_name(name);
    if let Some(mark) = mark {
        principal.login = mark.login();
    }
    Ok(Some(principal))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(roles: &[&str]) -> Principal {
        Principal::new(
            "sub-1".into(),
            roles.iter().map(|r| r.to_string()).collect(),
            BTreeMap::from([("bsn".to_string(), Value::from("999990019"))]),
            Channel::TestLogin,
        )
    }

    #[test]
    fn every_role_has_the_prefix() {
        for role in Level::ALL.map(Level::role).into_iter().chain([CITIZEN]) {
            assert!(role.starts_with(ROLE_PREFIX), "{role}");
        }
    }

    #[test]
    fn a_level_holds_the_levels_under_it_and_no_other_role_gives_one() {
        let p = principal(&["regelhet-beslisser"]);
        assert_eq!(p.level(), Some(Level::Decider));
        assert!(p.has(Level::Reader));
        assert!(p.has(Level::Decider));
        assert!(!p.has(Level::Admin));
        assert!(p.require(Level::Decider).is_ok());
        assert_eq!(
            p.require(Level::Admin).unwrap_err().status(),
            StatusCode::FORBIDDEN
        );
        // A role of the old cell model gives nothing.
        let p = principal(&["regelhet-cel-gemeente_wilgendaal"]);
        assert_eq!(p.level(), None);
        assert!(!p.is_user());
        // Nor does the former handler's role, beside the roles it came with.
        assert_eq!(principal(&["regelhet-behandelaar"]).level(), None);
        let p = principal(&["regelhet-behandelaar", "regelhet-lezer"]);
        assert_eq!(p.level(), Some(Level::Reader));
        assert!(!p.has(Level::Decider));
    }

    #[test]
    fn what_an_admin_enters_in_the_panel_is_kept_as_through_the_panel() {
        let p = principal(&["regelhet-admin"]);
        assert_eq!(p.actor().channel, Channel::TestLogin);
        let via_panel = p.admin_panel_actor();
        assert_eq!(via_panel.account, "sub-1");
        assert_eq!(via_panel.channel, Channel::AdminPanel);
    }

    #[test]
    fn a_citizen_needs_the_role_and_the_claim() {
        let c = principal(&[CITIZEN]).citizen("bsn").unwrap();
        assert_eq!(c.subject(), "999990019");
        assert_eq!(c.sub(), "sub-1");
        assert!(principal(&[CITIZEN]).citizen("kvk").is_err());
        assert!(principal(&["regelhet-lezer"]).citizen("bsn").is_err());
    }

    #[test]
    fn a_principal_of_fsc_voor_burgers_is_a_citizen_and_nothing_else() {
        let p = Principal::fsc4b_citizen("fsc4b:x".into(), "bsn", "999990019");
        assert!(p.is_citizen());
        assert_eq!(p.level(), None);
        assert!(!p.has(Level::Reader));
        assert_eq!(p.citizen("bsn").unwrap().subject(), "999990019");
        assert_eq!(p.actor().channel, Channel::Fsc4b);
    }
}
