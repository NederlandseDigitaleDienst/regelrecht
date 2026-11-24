//! The configuration of the two roles around an organisation, from the
//! environment ([`crate::config`] reads the rest).
//!
//! The front of an organisation (`REGELHET_ROL=organisatie`,
//! [`FrontSettings`]):
//!
//! - `REGELHET_ORGANISATIE_ID`: `<namespace>/<name>` of the object, the
//!   `iss` of every statement the front signs; required;
//! - `REGELHET_ORGANISATIE_NAAM`: the name it shows, such as `Gemeente
//!   Wilgendaal`; required;
//! - `REGELHET_ORGANISATIE_LOGO`: an `https` URL of its logo; optional;
//! - `REGELHET_PROCESSEN`: the file that lists the processes of the
//!   organisation ([`crate::organisatie::processes`]); required;
//! - `REGELHET_ORGANISATIE_SLEUTEL`: the file with the private key that
//!   signs the statements, PKCS#8 PEM on P-256. Both instances of a front
//!   share it, so it is required, except on `REGELHET_SURFACE=all` in test
//!   mode, where one instance makes a key at start.
//!
//! The front runs in test mode, keeps no chronicles and loads no manifest;
//! the store in Postgres and FSC voor Burgers are refused.
//!
//! A process that belongs to an organisation ([`Membership`]), beside what
//! it has anyway:
//!
//! - `REGELHET_PROCES_ID`: `<namespace>/<name>` of the process, the `aud`
//!   it accepts (with `#burger` or `#backoffice`); setting it turns the
//!   membership on;
//! - `REGELHET_ORGANISATIE_ID`: the `iss` it accepts; required then;
//! - `REGELHET_ORGANISATIE_NAAM`: the name its screens show; by default
//!   the id;
//! - `REGELHET_ORGANISATIE_URL`: the address of the organisation's front on
//!   the side this instance serves, for the link back; optional;
//! - `REGELHET_ORGANISATIE_SLEUTEL_URL`: where the front's public key is
//!   fetched (`/organisatie/v1/sleutel`), test mode only; or
//! - `REGELHET_ORGANISATIE_SLEUTEL_JWK`: that key itself, pinned, as a JWK.
//!
//! One of the two keys is required. A variable of the membership set without
//! `REGELHET_PROCES_ID` is a configuration half done, and does not start.

use std::path::PathBuf;

use crate::config::{ConfigError, Mode};
use crate::surface::Surface;

/// What the server is.
#[derive(Debug, Clone)]
pub enum Role {
    /// It runs one legal process; `Some` when the process belongs to an
    /// organisation.
    Process(Option<Membership>),
    /// It is the front of an organisation, before the processes that belong
    /// to it.
    Organisation(FrontSettings),
}

impl Role {
    /// Its name, as `REGELHET_ROL` takes it.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Process(_) => "proces",
            Self::Organisation(_) => "organisatie",
        }
    }
}

/// The front of an organisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontSettings {
    /// `<namespace>/<name>`: the `iss` of its statements.
    pub id: String,
    /// The name it shows.
    pub name: String,
    /// An `https` URL of its logo.
    pub logo: Option<String>,
    /// The file that lists its processes.
    pub processes: PathBuf,
    /// The file with its private key; none where it makes one at start.
    pub key: Option<PathBuf>,
}

/// Where a process finds the public key of its organisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrganisationKey {
    /// Fetched from the front, test mode only.
    Url(String),
    /// Pinned in the configuration, a JWK as JSON text.
    Jwk(String),
}

/// A process's membership of an organisation: whose statements it accepts,
/// and for whom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membership {
    /// `<namespace>/<name>` of this process: the `aud` it accepts.
    pub process_id: String,
    /// The organisation's id: the `iss` it accepts.
    pub organisation_id: String,
    /// The organisation's name, for the screens.
    pub name: String,
    /// The organisation's front on this instance's side, for the link back.
    pub url: Option<String>,
    pub key: OrganisationKey,
}

/// The variables of a membership besides `REGELHET_PROCES_ID`.
const MEMBERSHIP_VARIABLES: [&str; 5] = [
    "REGELHET_ORGANISATIE_NAAM",
    "REGELHET_ORGANISATIE_URL",
    "REGELHET_ORGANISATIE_SLEUTEL_URL",
    "REGELHET_ORGANISATIE_SLEUTEL_JWK",
    "REGELHET_ORGANISATIE_ID",
];

/// The variables only a front reads.
const FRONT_VARIABLES: [&str; 3] = [
    "REGELHET_ORGANISATIE_LOGO",
    "REGELHET_PROCESSEN",
    "REGELHET_ORGANISATIE_SLEUTEL",
];

/// The role the environment `set` names, for `mode` on `surface`.
///
/// # Errors
///
/// When `REGELHET_ROL` names no role, a front lacks a required variable or
/// does not run in test mode, or a membership is half configured.
pub fn role(
    mode: Mode,
    surface: Surface,
    set: &impl Fn(&str) -> Option<String>,
) -> Result<Role, ConfigError> {
    match set("REGELHET_ROL").as_deref().map(str::trim) {
        None | Some("proces") => {
            if let Some(name) = FRONT_VARIABLES.into_iter().find(|n| set(n).is_some()) {
                return Err(ConfigError::FrontVariableOnProcess(name));
            }
            membership(mode, set).map(Role::Process)
        }
        Some("organisatie") => front(mode, surface, set).map(Role::Organisation),
        Some(other) => Err(ConfigError::UnknownRole(other.to_string())),
    }
}

fn front(
    mode: Mode,
    surface: Surface,
    set: &impl Fn(&str) -> Option<String>,
) -> Result<FrontSettings, ConfigError> {
    if mode == Mode::Stable {
        return Err(ConfigError::StableFront);
    }
    if set("REGELHET_PROCES_ID").is_some() {
        return Err(ConfigError::ProcessIdOnFront);
    }
    let need = |name: &'static str| {
        set(name)
            .map(|v| v.trim().to_string())
            .ok_or(ConfigError::FrontNeeds(name))
    };
    let logo = set("REGELHET_ORGANISATIE_LOGO").map(|l| l.trim().to_string());
    if let Some(logo) = &logo
        && !logo.starts_with("https://")
    {
        return Err(ConfigError::BadLogo(logo.clone()));
    }
    let key = set("REGELHET_ORGANISATIE_SLEUTEL").map(|p| PathBuf::from(p.trim()));
    if key.is_none() && surface != Surface::All {
        return Err(ConfigError::FrontNeeds("REGELHET_ORGANISATIE_SLEUTEL"));
    }
    Ok(FrontSettings {
        id: need("REGELHET_ORGANISATIE_ID")?,
        name: need("REGELHET_ORGANISATIE_NAAM")?,
        logo,
        processes: PathBuf::from(need("REGELHET_PROCESSEN")?),
        key,
    })
}

fn membership(
    mode: Mode,
    set: &impl Fn(&str) -> Option<String>,
) -> Result<Option<Membership>, ConfigError> {
    let Some(process_id) = set("REGELHET_PROCES_ID").map(|v| v.trim().to_string()) else {
        return match MEMBERSHIP_VARIABLES.into_iter().find(|n| set(n).is_some()) {
            Some(name) => Err(ConfigError::PartialMembership(name)),
            None => Ok(None),
        };
    };
    let organisation_id = set("REGELHET_ORGANISATIE_ID")
        .map(|v| v.trim().to_string())
        .ok_or(ConfigError::MembershipNeeds("REGELHET_ORGANISATIE_ID"))?;
    let key = match (
        set("REGELHET_ORGANISATIE_SLEUTEL_URL"),
        set("REGELHET_ORGANISATIE_SLEUTEL_JWK"),
    ) {
        (_, Some(jwk)) => OrganisationKey::Jwk(jwk),
        (Some(_), None) if mode == Mode::Stable => return Err(ConfigError::StableFetchesNoKey),
        (Some(url), None) => OrganisationKey::Url(http_url(&url)?),
        (None, None) => {
            return Err(ConfigError::MembershipNeeds(
                "REGELHET_ORGANISATIE_SLEUTEL_URL or REGELHET_ORGANISATIE_SLEUTEL_JWK",
            ));
        }
    };
    let url = set("REGELHET_ORGANISATIE_URL")
        .map(|u| http_url(&u))
        .transpose()?;
    Ok(Some(Membership {
        name: set("REGELHET_ORGANISATIE_NAAM")
            .map_or_else(|| organisation_id.clone(), |n| n.trim().to_string()),
        process_id,
        organisation_id,
        url,
        key,
    }))
}

/// `url` without a trailing slash, when it is an `http(s)` URL.
fn http_url(url: &str) -> Result<String, ConfigError> {
    let url = url.trim().trim_end_matches('/');
    if url.starts_with("https://") || url.starts_with("http://") {
        Ok(url.to_string())
    } else {
        Err(ConfigError::BadOrganisationUrl(url.to_string()))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |k| {
            pairs
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        }
    }

    const FRONT: &[(&str, &str)] = &[
        ("REGELHET_ROL", "organisatie"),
        ("REGELHET_ORGANISATIE_ID", "tnt-voorbeeld/wilgendaal"),
        ("REGELHET_ORGANISATIE_NAAM", "Gemeente Wilgendaal"),
        ("REGELHET_PROCESSEN", "/etc/organisatie/processen.yaml"),
        (
            "REGELHET_ORGANISATIE_SLEUTEL",
            "/var/run/organisatie/sleutel.pem",
        ),
    ];

    #[test]
    fn without_a_role_the_server_runs_a_process_on_its_own() {
        let role = role(Mode::Test, Surface::All, &env(&[])).unwrap();
        assert!(matches!(role, Role::Process(None)));
        assert_eq!(role.name(), "proces");
        let e = role_error(&[("REGELHET_ROL", "voorkant")]);
        assert_eq!(e, ConfigError::UnknownRole("voorkant".into()));
    }

    fn role_error(pairs: &'static [(&'static str, &'static str)]) -> ConfigError {
        role(Mode::Test, Surface::Backoffice, &env(pairs)).unwrap_err()
    }

    #[test]
    fn a_front_reads_its_settings() {
        let Role::Organisation(front) = role(Mode::Test, Surface::Backoffice, &env(FRONT)).unwrap()
        else {
            unreachable!("the front's role");
        };
        assert_eq!(front.id, "tnt-voorbeeld/wilgendaal");
        assert_eq!(front.name, "Gemeente Wilgendaal");
        assert_eq!(front.logo, None);
        assert_eq!(
            front.key,
            Some(PathBuf::from("/var/run/organisatie/sleutel.pem"))
        );
    }

    #[test]
    fn a_front_runs_in_test_mode_and_shares_its_key_between_instances() {
        assert_eq!(
            role(Mode::Stable, Surface::Backoffice, &env(FRONT)).unwrap_err(),
            ConfigError::StableFront
        );
        const NO_KEY: &[(&str, &str)] = &[
            ("REGELHET_ROL", "organisatie"),
            ("REGELHET_ORGANISATIE_ID", "tnt-voorbeeld/wilgendaal"),
            ("REGELHET_ORGANISATIE_NAAM", "Gemeente Wilgendaal"),
            ("REGELHET_PROCESSEN", "/etc/organisatie/processen.yaml"),
        ];
        assert_eq!(
            role_error(NO_KEY),
            ConfigError::FrontNeeds("REGELHET_ORGANISATIE_SLEUTEL")
        );
        let Role::Organisation(front) = role(Mode::Test, Surface::All, &env(NO_KEY)).unwrap()
        else {
            unreachable!("the front's role");
        };
        assert_eq!(front.key, None);
        assert_eq!(
            role_error(&[
                ("REGELHET_ROL", "organisatie"),
                ("REGELHET_ORGANISATIE_LOGO", "http://example.org/logo.svg"),
            ]),
            ConfigError::BadLogo("http://example.org/logo.svg".into())
        );
    }

    #[test]
    fn a_membership_needs_the_organisation_and_a_key() {
        let Role::Process(Some(m)) = role(
            Mode::Test,
            Surface::Citizen,
            &env(&[
                ("REGELHET_PROCES_ID", "tnt-voorbeeld/parkeerkaart"),
                ("REGELHET_ORGANISATIE_ID", "tnt-voorbeeld/wilgendaal"),
                (
                    "REGELHET_ORGANISATIE_SLEUTEL_URL",
                    "http://wilgendaal-backoffice:8000/organisatie/v1/sleutel",
                ),
                (
                    "REGELHET_ORGANISATIE_URL",
                    "https://wilgendaal.example.org/",
                ),
            ]),
        )
        .unwrap() else {
            unreachable!("a membership");
        };
        assert_eq!(m.name, "tnt-voorbeeld/wilgendaal");
        assert_eq!(m.url.as_deref(), Some("https://wilgendaal.example.org"));
        assert!(matches!(m.key, OrganisationKey::Url(_)));
        assert_eq!(
            role_error(&[("REGELHET_ORGANISATIE_NAAM", "Gemeente Wilgendaal")]),
            ConfigError::PartialMembership("REGELHET_ORGANISATIE_NAAM")
        );
        assert_eq!(
            role_error(&[
                ("REGELHET_PROCES_ID", "tnt-voorbeeld/parkeerkaart"),
                ("REGELHET_ORGANISATIE_ID", "tnt-voorbeeld/wilgendaal"),
            ]),
            ConfigError::MembershipNeeds(
                "REGELHET_ORGANISATIE_SLEUTEL_URL or REGELHET_ORGANISATIE_SLEUTEL_JWK"
            )
        );
        assert_eq!(
            role(
                Mode::Stable,
                Surface::Citizen,
                &env(&[
                    ("REGELHET_PROCES_ID", "tnt-voorbeeld/parkeerkaart"),
                    ("REGELHET_ORGANISATIE_ID", "tnt-voorbeeld/wilgendaal"),
                    (
                        "REGELHET_ORGANISATIE_SLEUTEL_URL",
                        "http://wilgendaal:8000/k"
                    ),
                ])
            )
            .unwrap_err(),
            ConfigError::StableFetchesNoKey
        );
        assert_eq!(
            role_error(&[("REGELHET_PROCESSEN", "/etc/processen.yaml")]),
            ConfigError::FrontVariableOnProcess("REGELHET_PROCESSEN")
        );
    }
}
