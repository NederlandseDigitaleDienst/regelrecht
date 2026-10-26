//! The PoC register: `pocs/registry.yaml`, parsed once at startup.
//!
//! Everything that differs per PoC is described here and nowhere else — the
//! cards on the index page, which paths are served or proxied, and the name of
//! the environment variable holding each PoC's password. Adding a PoC is
//! therefore a register entry plus a directory, not an edit to this crate.

use std::collections::HashSet;

use serde::Deserialize;

/// How a PoC reaches the visitor.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Soort {
    /// Built into this image, served from `{static_root}/{slug}`.
    Statisch,
    /// Another ZAD component, reached in-cluster and proxied through.
    Proxy,
}

/// How finished a PoC's model of the law is.
///
/// A computed amount looks equally confident whether the rules behind it were
/// walked through with a lawyer or sketched in an afternoon, so the difference
/// has to be stated rather than left to the visitor to guess. There is no
/// "production" here on purpose: a PoC never becomes one, it gets replaced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Verkenning,
    InOntwikkeling,
    Gevalideerd,
}

impl Status {
    /// Short label, for the strip inside a PoC.
    pub fn label(self) -> &'static str {
        match self {
            Status::Verkenning => "Verkenning",
            Status::InOntwikkeling => "In ontwikkeling",
            Status::Gevalideerd => "Gevalideerd",
        }
    }

    /// What this status means, in one sentence.
    ///
    /// Lives in the strip's tooltip, alongside the voorbehoud. It used to be on
    /// the login screen, which is public; the status of a named dossier is
    /// itself something not to hand out, so it moved behind the password with
    /// the rest of the case.
    pub fn uitleg(self) -> &'static str {
        match self {
            Status::Verkenning => {
                "Een eerste uitwerking, niet nagelopen door iemand van het beleidsterrein. \
                 Neem geen enkele uitkomst voor waar aan."
            }
            Status::InOntwikkeling => {
                "Gedeeltelijk nagelopen: sommige onderdelen kloppen, andere nog niet, en \
                 welke dat zijn ligt niet vast."
            }
            Status::Gevalideerd => {
                "Doorgelopen met uitvoerings- of beleidsexperts. Nog steeds een demonstratie \
                 op fictieve gegevens, geen besluit over een echt geval."
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Poc {
    pub slug: String,
    /// Public name. The index and the login screen are both readable without a
    /// password (the latter by guessing a slug), so this says what kind of
    /// exploration this is and not which dossier it is about.
    pub titel: String,
    /// Public summary, under the same rule as `titel`.
    pub samenvatting: String,
    /// The real subject, shown once the visitor is past the password: the
    /// portal puts it in the strip it injects into the PoC's own pages. Falls
    /// back to the public title when absent, so a PoC with nothing to hide
    /// needs only the one field.
    #[serde(default)]
    pub titel_intern: Option<String>,
    /// The full description, for the register's own readers. The portal renders
    /// no summary behind the gate — a PoC's own pages say what it is — so this
    /// keeps the real text next to the public one instead of only in git
    /// history, and the leak test asserts it never reaches a public page.
    #[serde(default)]
    pub samenvatting_intern: Option<String>,
    pub soort: Soort,
    /// How finished this uitwerking is. No default: leaving it out would make
    /// every new PoC look as trustworthy as the most-checked one.
    pub status: Status,
    /// Why this does not (yet) hold, in this PoC's own words. Required for the
    /// same reason — and free text, because the house sentence is the one
    /// every reader skips.
    pub voorbehoud: String,
    /// Whether the portal puts its strip with the status and the voorbehoud
    /// into this PoC's own pages ([`crate::app`]). On unless the register
    /// turns it off, for a PoC that is shown in front of an audience and says
    /// what it is itself; the index and the login screen still carry both.
    #[serde(default = "strook_default")]
    pub strook: bool,
    /// npm workspace producing the `dist` — `statisch` only.
    #[serde(default)]
    pub bron: Option<String>,
    /// ZAD component name — `proxy` only.
    #[serde(default)]
    pub upstream: Option<String>,
    #[serde(default)]
    pub corpus: Vec<String>,
    #[serde(default)]
    pub assistent: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Corpora the portal fetches for this PoC while it runs, behind its
    /// password ([`crate::bron`]): `/<slug>/bron/<name>`.
    #[serde(default)]
    pub sources: Vec<Source>,
}

fn strook_default() -> bool {
    true
}

/// A corpus a static PoC loads while it runs: a folder on a branch of a
/// GitHub repository, fetched with the token in `token_env`. The browser
/// never sees the token; the portal serves the files at
/// `/<slug>/bron/<name>`. Locally `POC_BRON_<NAME>` names a folder that
/// takes its place, without a token.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub name: String,
    pub github: GithubSource,
    /// The environment variable holding the token; its name, never its value.
    pub token_env: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GithubSource {
    pub owner: String,
    pub repo: String,
    pub branch: String,
    /// The folder of the branch that holds the source; its files are served
    /// by their path under it.
    pub path: String,
}

impl Source {
    /// The environment variable naming a local folder that replaces the
    /// GitHub fetch: `napp` -> `POC_BRON_NAPP`.
    pub fn local_env(&self) -> String {
        format!("POC_BRON_{}", self.name.to_uppercase().replace('-', "_"))
    }
}

impl Poc {
    /// What this PoC is called behind the password.
    ///
    /// Named rather than read from the field directly, so that a page renders
    /// the internal text only where it says so out loud. Getting this backwards
    /// on a public page is the failure this split exists to prevent, and it is
    /// invisible in review if both are plain fields.
    pub fn titel_achter_de_poort(&self) -> &str {
        self.titel_intern.as_deref().unwrap_or(&self.titel)
    }

    /// Name of the environment variable carrying this PoC's password.
    ///
    /// `terugbetaalregimes` → `POC_PW_TERUGBETAALREGIMES`, and a hyphenated
    /// slug becomes an underscore: `mijn-poc` → `POC_PW_MIJN_POC`. Derived
    /// rather than configured, so a new PoC needs no code here.
    pub fn wachtwoord_env(&self) -> String {
        format!("POC_PW_{}", self.slug.to_uppercase().replace('-', "_"))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Registry {
    pub pocs: Vec<Poc>,
}

/// Why a register was refused. Every one of these is a startup failure: a
/// half-understood register would decide who gets in.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error("registry is empty — a portal with no PoCs is a mistake, not a state")]
    Empty,
    #[error("slug {0:?} is used more than once")]
    DuplicateSlug(String),
    #[error("slug {0:?} may only contain a-z, 0-9 and '-', and must start with a letter or digit")]
    InvalidSlug(String),
    #[error("PoC {0:?} is `statisch` but names no `bron`")]
    StaticWithoutBron(String),
    #[error("PoC {0:?} is `proxy` but names no `upstream`")]
    ProxyWithoutUpstream(String),
    #[error(
        "PoC {0:?} is `proxy`; the assistant runs in this image and cannot serve a proxied PoC"
    )]
    ProxyWithAssistent(String),
    #[error(
        "PoC {0:?} has no usable `voorbehoud`. Write what does not hold in this PoC, in your own \
         words — a visitor cannot tell a sketch from a checked model by looking at it."
    )]
    VoorbehoudTeKort(String),
    #[error(
        "PoC {0:?} names source {1:?} twice, with a name outside a-z, 0-9 and '-', or without a \
         folder (`github.path`)"
    )]
    BadSource(String, String),
    #[error("PoC {0:?} is `proxy`; a source is served to a static PoC only")]
    ProxyWithSource(String),
}

/// Shortest `voorbehoud` that can say anything. Not a quality bar — it only
/// catches the placeholder that was never filled in.
const VOORBEHOUD_MINIMUM: usize = 30;

/// A slug ends up in a URL path, a cookie name and an environment variable
/// name. Restricting it to this alphabet means none of those three ever needs
/// escaping, and no slug can climb out of its path prefix.
fn slug_is_valid(slug: &str) -> bool {
    !slug.is_empty()
        && slug
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

impl Registry {
    /// Parse and validate a register.
    pub fn from_yaml(yaml: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let registry: Registry = serde_yaml_ng::from_str(yaml)?;
        registry.validate()?;
        Ok(registry)
    }

    fn validate(&self) -> Result<(), RegistryError> {
        if self.pocs.is_empty() {
            return Err(RegistryError::Empty);
        }

        let mut seen = HashSet::new();
        for poc in &self.pocs {
            if !slug_is_valid(&poc.slug) {
                return Err(RegistryError::InvalidSlug(poc.slug.clone()));
            }
            if !seen.insert(&poc.slug) {
                return Err(RegistryError::DuplicateSlug(poc.slug.clone()));
            }
            if poc.voorbehoud.trim().len() < VOORBEHOUD_MINIMUM {
                return Err(RegistryError::VoorbehoudTeKort(poc.slug.clone()));
            }
            match poc.soort {
                Soort::Statisch if poc.bron.is_none() => {
                    return Err(RegistryError::StaticWithoutBron(poc.slug.clone()))
                }
                Soort::Proxy if poc.upstream.is_none() => {
                    return Err(RegistryError::ProxyWithoutUpstream(poc.slug.clone()))
                }
                // The assistant spawns the Claude CLI inside *this* container
                // and rewrites this image's own corpus copy. A proxied PoC runs
                // somewhere else, so switching it on there would silently do
                // nothing.
                Soort::Proxy if poc.assistent => {
                    return Err(RegistryError::ProxyWithAssistent(poc.slug.clone()))
                }
                _ => {}
            }
            if poc.soort == Soort::Proxy && !poc.sources.is_empty() {
                return Err(RegistryError::ProxyWithSource(poc.slug.clone()));
            }
            let mut namen = HashSet::new();
            for source in &poc.sources {
                let geen_map = source.github.path.trim_matches('/').is_empty();
                if !slug_is_valid(&source.name) || !namen.insert(&source.name) || geen_map {
                    return Err(RegistryError::BadSource(
                        poc.slug.clone(),
                        source.name.clone(),
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn get(&self, slug: &str) -> Option<&Poc> {
        self.pocs.iter().find(|p| p.slug == slug)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    const VOORBEHOUD: &str = "Dit is een demonstratie en geen geldend recht.";

    fn statisch(slug: &str) -> String {
        format!(
            "pocs:\n  - slug: {slug}\n    titel: T\n    samenvatting: S\n    soort: statisch\n    \
             bron: b\n    status: verkenning\n    voorbehoud: {VOORBEHOUD}\n"
        )
    }

    fn registry_echt() -> Registry {
        Registry::from_yaml(include_str!("../../../pocs/registry.yaml")).expect("registry.yaml")
    }

    #[test]
    fn parses_the_real_register() {
        // The register that ships is the one the portal will run on; a typo in
        // it should fail here rather than at startup in the cluster.
        let yaml = include_str!("../../../pocs/registry.yaml");
        let registry = Registry::from_yaml(yaml).expect("registry.yaml is valid");
        assert!(registry.get("napp").is_some());
        assert_eq!(registry.get("napp").map(|p| &p.soort), Some(&Soort::Proxy));
    }

    #[test]
    fn env_var_name_is_derived_from_the_slug() {
        let poc = Registry::from_yaml(&statisch("mijn-poc"))
            .expect("valid")
            .pocs[0]
            .clone();
        assert_eq!(poc.wachtwoord_env(), "POC_PW_MIJN_POC");
    }

    #[test]
    fn duplicate_slugs_are_refused() {
        let yaml = format!("{}{}", statisch("a"), statisch("a").replace("pocs:\n", ""));
        let err = Registry::from_yaml(&yaml).expect_err("duplicate slug must fail");
        assert!(err.to_string().contains("more than once"), "{err}");
    }

    #[test]
    fn a_slug_cannot_escape_its_path_prefix() {
        for bad in ["../etc", "Foo", "a/b", "", "-leading"] {
            let yaml = statisch(bad);
            assert!(
                Registry::from_yaml(&yaml).is_err(),
                "slug {bad:?} should be refused"
            );
        }
    }

    #[test]
    fn a_static_poc_must_name_its_source() {
        let yaml = &format!(
            "pocs:\n  - slug: a\n    titel: T\n    samenvatting: S\n    soort: statisch\n    \
             status: verkenning\n    voorbehoud: {VOORBEHOUD}\n"
        );
        let err = Registry::from_yaml(yaml).expect_err("must fail");
        assert!(err.to_string().contains("bron"), "{err}");
    }

    #[test]
    fn a_proxy_poc_must_name_its_upstream() {
        let yaml = &format!(
            "pocs:\n  - slug: a\n    titel: T\n    samenvatting: S\n    soort: proxy\n    \
             status: verkenning\n    voorbehoud: {VOORBEHOUD}\n"
        );
        let err = Registry::from_yaml(yaml).expect_err("must fail");
        assert!(err.to_string().contains("upstream"), "{err}");
    }

    #[test]
    fn the_assistant_cannot_be_switched_on_for_a_proxied_poc() {
        let yaml = &format!(
            "pocs:\n  - slug: a\n    titel: T\n    samenvatting: S\n    soort: proxy\n    \
             upstream: u\n    assistent: true\n    status: verkenning\n    \
             voorbehoud: {VOORBEHOUD}\n"
        );
        let err = Registry::from_yaml(yaml).expect_err("must fail");
        assert!(err.to_string().contains("assistant"), "{err}");
    }

    #[test]
    fn every_poc_in_the_real_register_states_what_does_not_hold() {
        // The point of the whole field: a visitor who lands on a PoC through a
        // forwarded link must be able to see how finished it is.
        for poc in &registry_echt().pocs {
            assert!(
                poc.voorbehoud.trim().len() >= VOORBEHOUD_MINIMUM,
                "{} has no real voorbehoud",
                poc.slug
            );
        }
    }

    #[test]
    fn a_missing_status_is_refused_rather_than_assumed() {
        // No `#[serde(default)]` on purpose: a default would silently make a
        // brand-new sketch look as checked as the most-reviewed PoC.
        let yaml = format!(
            "pocs:\n  - slug: a\n    titel: T\n    samenvatting: S\n    soort: statisch\n    \
             bron: b\n    voorbehoud: {VOORBEHOUD}\n"
        );
        assert!(Registry::from_yaml(&yaml).is_err());
    }

    #[test]
    fn a_placeholder_voorbehoud_is_refused() {
        let yaml =
            "pocs:\n  - slug: a\n    titel: T\n    samenvatting: S\n    soort: statisch\n    \
                    bron: b\n    status: verkenning\n    voorbehoud: tbd\n";
        let err = Registry::from_yaml(yaml).expect_err("must fail");
        assert!(err.to_string().contains("voorbehoud"), "{err}");
    }

    #[test]
    fn an_unknown_status_is_refused() {
        let yaml = format!(
            "pocs:\n  - slug: a\n    titel: T\n    samenvatting: S\n    soort: statisch\n    \
             bron: b\n    status: productie\n    voorbehoud: {VOORBEHOUD}\n"
        );
        assert!(
            Registry::from_yaml(&yaml).is_err(),
            "a PoC is never 'productie'"
        );
    }

    #[test]
    fn a_source_is_read_with_its_branch_and_token_name() {
        let yaml = format!(
            "{}    sources:\n      - name: voorbeeld\n        github:\n          owner: example-org\n          \
             repo: regelrecht-corpus-example\n          branch: poc/x\n          path: x\n        \
             token_env: EXAMPLE_TOKEN\n",
            statisch("a")
        );
        let registry = Registry::from_yaml(&yaml).expect("valid");
        let source = &registry.pocs[0].sources[0];
        assert_eq!(source.github.branch, "poc/x");
        assert_eq!(source.local_env(), "POC_BRON_VOORBEELD");
        let twice = yaml.replace(
            "    sources:\n",
            "    sources:\n      - name: voorbeeld\n        github: {owner: o, repo: r, branch: b, path: p}\n        token_env: T\n",
        );
        assert!(Registry::from_yaml(&twice).is_err());
        let zonder_map = yaml.replace("path: x", "path: /");
        assert!(Registry::from_yaml(&zonder_map).is_err());
    }

    #[test]
    fn an_empty_register_is_refused() {
        assert_eq!(
            Registry::from_yaml("pocs: []")
                .expect_err("must fail")
                .to_string(),
            RegistryError::Empty.to_string()
        );
    }
}
