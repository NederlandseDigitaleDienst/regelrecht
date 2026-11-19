//! Who may do what: the roles of the server, by name, in one place, and the
//! person a request comes from ([`Principal`]).
//!
//! The roles are Keycloak realm roles, all named with [`ROLE_PREFIX`]
//! because the realm is shared with the editor:
//!
//! - [`CITIZEN`]: a citizen, who submits for themself and reads their own
//!   dossier, identified by a claim of their account ([`Citizen`]);
//! - a ladder for employees ([`Level`]): reader < handler < decider < admin.
//!   Holding a step holds the steps under it, whether or not Keycloak gives
//!   the lower roles too;
//! - per cell a role ([`cell_role`]) that says for which organization (the
//!   municipality, the GGD, the court) an employee works. A level counts only
//!   for the cells whose role the employee holds.
//!
//! The rules on top of them live in the API ([`crate::api`]): which route
//! asks which level, and for which cell.

use std::collections::{BTreeMap, BTreeSet};

use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::request::Parts;
use regelrecht_auth::{
    SESSION_KEY_AUTHENTICATED, SESSION_KEY_CLAIMS, SESSION_KEY_ROLES, SESSION_KEY_SUB,
};
use serde_json::Value;
use tower_sessions::Session;

use crate::api::ApiError;

/// What every role of this server starts with.
pub const ROLE_PREFIX: &str = "wetsproces-";

/// A citizen: submits for themself and reads their own dossier.
pub const CITIZEN: &str = "wetsproces-burger";

/// What the role of a cell starts with; the cell's id follows.
pub const CELL_ROLE_PREFIX: &str = "wetsproces-cel-";

/// The role that says an employee works for the cell `cell`.
pub fn cell_role(cell: &str) -> String {
    format!("{CELL_ROLE_PREFIX}{cell}")
}

/// A step on the ladder of employees, lowest first: each may do what the
/// steps under it may.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Reads the chronicles of their cells.
    Reader,
    /// Records facts and sends copies for their cells.
    Handler,
    /// Takes decisions for their cells.
    Decider,
    /// Sets the clock in test mode, and whatever is added to manage the
    /// server.
    Admin,
}

impl Level {
    /// Every level, lowest first.
    pub const ALL: [Self; 4] = [Self::Reader, Self::Handler, Self::Decider, Self::Admin];

    /// The name of this level: its role without [`ROLE_PREFIX`].
    pub fn name(self) -> &'static str {
        let role = self.role();
        role.strip_prefix(ROLE_PREFIX).unwrap_or(role)
    }

    /// The realm role of this level.
    pub fn role(self) -> &'static str {
        match self {
            Self::Reader => "wetsproces-lezer",
            Self::Handler => "wetsproces-behandelaar",
            Self::Decider => "wetsproces-beslisser",
            Self::Admin => "wetsproces-admin",
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
}

impl Principal {
    /// A principal from what a session holds.
    pub fn new(sub: String, roles: BTreeSet<String>, claims: BTreeMap<String, Value>) -> Self {
        Self { sub, roles, claims }
    }

    /// The account's subject: the OIDC `sub`, which the store keeps beside
    /// every record this principal makes.
    pub fn sub(&self) -> &str {
        &self.sub
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

    /// Whether the principal holds at least `level`, for any cell.
    pub fn has(&self, level: Level) -> bool {
        self.level().is_some_and(|held| held >= level)
    }

    /// Whether the principal holds at least `level` for the cell `cell`.
    pub fn may(&self, level: Level, cell: &str) -> bool {
        self.has(level) && self.roles.contains(&cell_role(cell))
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

    /// Allow when the principal holds `level` for `cell`.
    ///
    /// # Errors
    ///
    /// 403 otherwise.
    pub fn require(&self, level: Level, cell: &str) -> Result<(), ApiError> {
        if self.may(level, cell) {
            Ok(())
        } else {
            tracing::warn!(sub = %self.sub, level = level.role(), cell, "refused");
            Err(forbidden())
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

impl<S: Send + Sync> FromRequestParts<S> for Principal {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|(status, message)| ApiError::new(status, message))?;
        let unauthenticated = || ApiError::new(StatusCode::UNAUTHORIZED, "log in first");
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
            return Err(unauthenticated());
        }
        let sub: Option<String> = session.get(SESSION_KEY_SUB).await.map_err(read_error)?;
        let Some(sub) = sub.filter(|s| !s.is_empty()) else {
            return Err(unauthenticated());
        };
        let roles: Option<BTreeSet<String>> =
            session.get(SESSION_KEY_ROLES).await.map_err(read_error)?;
        let claims: Option<BTreeMap<String, Value>> =
            session.get(SESSION_KEY_CLAIMS).await.map_err(read_error)?;
        Ok(Self::new(
            sub,
            roles.unwrap_or_default(),
            claims.unwrap_or_default(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(roles: &[&str]) -> Principal {
        Principal::new(
            "sub-1".into(),
            roles.iter().map(|r| r.to_string()).collect(),
            BTreeMap::from([("bsn".to_string(), Value::from("999990019"))]),
        )
    }

    #[test]
    fn every_role_has_the_prefix() {
        for role in Level::ALL.map(Level::role).into_iter().chain([CITIZEN]) {
            assert!(role.starts_with(ROLE_PREFIX), "{role}");
        }
        assert!(cell_role("ggd").starts_with(ROLE_PREFIX));
    }

    #[test]
    fn a_level_holds_the_levels_under_it_for_its_own_cells_only() {
        let p = principal(&["wetsproces-beslisser", "wetsproces-cel-gemeente"]);
        assert_eq!(p.level(), Some(Level::Decider));
        assert!(p.may(Level::Reader, "gemeente"));
        assert!(p.may(Level::Handler, "gemeente"));
        assert!(p.may(Level::Decider, "gemeente"));
        assert!(!p.may(Level::Admin, "gemeente"));
        assert!(!p.may(Level::Reader, "ggd"));
        // A cell role without a level gives nothing.
        let p = principal(&["wetsproces-cel-gemeente"]);
        assert!(!p.may(Level::Reader, "gemeente"));
        assert!(!p.is_user());
    }

    #[test]
    fn a_citizen_needs_the_role_and_the_claim() {
        let c = principal(&[CITIZEN]).citizen("bsn").unwrap();
        assert_eq!(c.subject(), "999990019");
        assert_eq!(c.sub(), "sub-1");
        assert!(principal(&[CITIZEN]).citizen("kvk").is_err());
        assert!(principal(&["wetsproces-lezer"]).citizen("bsn").is_err());
    }
}
