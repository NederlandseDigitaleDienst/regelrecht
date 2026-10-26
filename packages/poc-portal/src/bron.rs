//! Sources: a corpus a static PoC loads while it runs, served behind its
//! password at `/<slug>/bron/<name>` (`sources` in `pocs/registry.yaml`).
//!
//! The portal fetches the tarball of the configured branch from the GitHub
//! API with the token in the source's `token_env`, keeps the YAML files under
//! its folder, and answers them as one JSON object, the shape the PoC's
//! bundle already takes: `{ "naam", "versie", "bestanden": { "<path>":
//! "<text>" } }`, each path under the folder. The browser never sees the
//! token. A fetch is kept for [`TTL`]; when GitHub fails, the last good copy
//! is served with a `melding` that says so, and without one the answer is an
//! error that says what went wrong.
//!
//! Locally `POC_BRON_<NAME>` names a folder that takes the branch's place:
//! read on every request, without a token and without the cache, so an edit
//! shows on the next load.

use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::sync::Mutex;

use crate::registry::Source;

/// How long a fetch from GitHub is served before the next request fetches
/// again: long enough to spare GitHub a request per visitor, short enough
/// that a push shows within a minute.
pub const TTL: Duration = Duration::from_secs(60);

/// How long a failed fetch without a good copy is answered from memory:
/// with a bad token or GitHub down, a visitor waits for one request at most,
/// not one per page load.
pub const FOUT_TTL: Duration = Duration::from_secs(10);

/// A source as the portal serves it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Opgehaald {
    pub naam: String,
    /// The short sha of the commit, or `lokaal` for a local folder.
    pub versie: Option<String>,
    pub bestanden: BTreeMap<String, String>,
    /// Set when this is the last good copy because a new fetch failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub melding: Option<String>,
}

/// Where a source comes from in this process. Its `Debug` leaves the token
/// out, so a trace or a failing test never prints it.
#[derive(Clone, PartialEq, Eq)]
pub enum Herkomst {
    /// A local folder, from `POC_BRON_<NAME>`.
    Lokaal(PathBuf),
    /// The branch on GitHub, with the token from `token_env`.
    Github {
        repo: String,
        branch: String,
        path: String,
        token: String,
    },
    /// Neither: why not, for the visitor.
    Geen(String),
}

impl std::fmt::Debug for Herkomst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lokaal(dir) => f.debug_tuple("Lokaal").field(dir).finish(),
            Self::Github {
                repo, branch, path, ..
            } => f
                .debug_struct("Github")
                .field("repo", repo)
                .field("branch", branch)
                .field("path", path)
                .field("token", &"<hidden>")
                .finish(),
            Self::Geen(waarom) => f.debug_tuple("Geen").field(waarom).finish(),
        }
    }
}

impl Herkomst {
    /// Where `source` comes from, given the environment `env`: a local
    /// folder wins over the token, so a developer with both set sees their
    /// own files.
    pub fn uit(source: &Source, env: impl Fn(&str) -> Option<String>) -> Self {
        let gezet = |k: &str| env(k).filter(|v| !v.trim().is_empty());
        if let Some(dir) = gezet(&source.local_env()) {
            return Self::Lokaal(PathBuf::from(dir));
        }
        match gezet(&source.token_env) {
            Some(token) => Self::Github {
                repo: format!("{}/{}", source.github.owner, source.github.repo),
                branch: source.github.branch.clone(),
                path: source.github.path.clone(),
                token,
            },
            None => Self::Geen(format!(
                "{} is niet gezet in deze omgeving, en {} ook niet",
                source.token_env,
                source.local_env()
            )),
        }
    }
}

/// The YAML files under `path` in a GitHub tarball, by their path under it,
/// and the short sha of the commit (seven characters from the end of the
/// archive's top-level directory, `{owner}-{repo}-{sha}`). No file under `path` is an error: the
/// folder is not on the branch.
pub fn uit_tarball(
    bytes: &[u8],
    path: &str,
) -> Result<(Option<String>, BTreeMap<String, String>), String> {
    let prefix = format!("{}/", path.trim_matches('/'));
    let mut bestanden = BTreeMap::new();
    let top = regelrecht_corpus::github::each_yaml_in_tar_gz(bytes, |rel, text| {
        if let Some(onder) = rel.strip_prefix(&prefix) {
            bestanden.insert(onder.to_string(), text);
        }
    })
    .map_err(|e| format!("the tarball does not read: {e}"))?;
    if bestanden.is_empty() {
        return Err(format!("the branch has no YAML file under '{path}'"));
    }
    let versie = top.and_then(|t| {
        t.rsplit('-')
            .next()
            .map(|sha| sha.chars().take(7).collect())
    });
    Ok((versie, bestanden))
}

/// The YAML files under the folder `dir`, by their path under it.
pub fn uit_map(dir: &Path) -> Result<BTreeMap<String, String>, String> {
    fn loop_door(
        dir: &Path,
        rel: &Path,
        uit: &mut BTreeMap<String, String>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir.join(rel))? {
            let entry = entry?;
            let rel = rel.join(entry.file_name());
            if entry.file_type()?.is_dir() {
                loop_door(dir, &rel, uit)?;
            } else if rel.extension().is_some_and(|x| x == "yaml" || x == "yml") {
                let text = std::fs::read_to_string(dir.join(&rel))?;
                uit.insert(rel.to_string_lossy().replace('\\', "/"), text);
            }
        }
        Ok(())
    }
    let mut uit = BTreeMap::new();
    loop_door(dir, Path::new(""), &mut uit).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(uit)
}

/// The last fetch of one source, served for [`TTL`], and the last failure
/// while there is no good copy, served for [`FOUT_TTL`].
#[derive(Debug)]
pub struct Cache {
    ttl: Duration,
    fout_ttl: Duration,
    laatste: Mutex<Staat>,
}

#[derive(Debug, Default)]
struct Staat {
    goed: Option<(Instant, Arc<Opgehaald>)>,
    fout: Option<(Instant, String)>,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new(TTL)
    }
}

impl Cache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            fout_ttl: FOUT_TTL.min(ttl),
            laatste: Mutex::new(Staat::default()),
        }
    }

    /// The source at `nu`: the last fetch if it is younger than the TTL,
    /// else what `haal` fetches. When `haal` fails, the last good copy with
    /// a `melding`, kept as the fetch of `nu` so a broken GitHub is not asked
    /// again on every request; without a copy, the error, kept for
    /// [`FOUT_TTL`]. The lock is held across the fetch, so concurrent
    /// visitors wait for one request.
    pub async fn geef<F, Fut>(&self, nu: Instant, haal: F) -> Result<Arc<Opgehaald>, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Opgehaald, String>>,
    {
        let mut staat = self.laatste.lock().await;
        if let Some((op, o)) = staat.goed.as_ref() {
            if nu.saturating_duration_since(*op) < self.ttl {
                return Ok(o.clone());
            }
        }
        if let (None, Some((op, e))) = (&staat.goed, &staat.fout) {
            if nu.saturating_duration_since(*op) < self.fout_ttl {
                return Err(e.clone());
            }
        }
        match haal().await {
            Ok(o) => {
                let o = Arc::new(o);
                staat.goed = Some((nu, o.clone()));
                staat.fout = None;
                Ok(o)
            }
            Err(e) => {
                let Some((_, oud)) = staat.goed.as_ref() else {
                    staat.fout = Some((nu, e.clone()));
                    return Err(e);
                };
                tracing::warn!(source = %oud.naam, error = %e, "source fetch failed; serving the last good copy");
                let mut kopie = (**oud).clone();
                kopie.melding = Some(format!(
                    "De bron kon niet opnieuw worden opgehaald ({e}); dit is de laatste goede kopie."
                ));
                let kopie = Arc::new(kopie);
                staat.goed = Some((nu, kopie.clone()));
                Ok(kopie)
            }
        }
    }
}

/// One source of one PoC, as the portal runs it.
#[derive(Debug)]
pub struct Bron {
    pub naam: String,
    pub herkomst: Herkomst,
    pub cache: Cache,
}

impl Bron {
    pub fn new(source: &Source, env: impl Fn(&str) -> Option<String>) -> Self {
        let herkomst = Herkomst::uit(source, env);
        match &herkomst {
            Herkomst::Lokaal(dir) => {
                tracing::info!(source = %source.name, dir = %dir.display(), "source from a local folder")
            }
            Herkomst::Github { repo, branch, .. } => {
                tracing::info!(source = %source.name, %repo, %branch, "source from GitHub")
            }
            Herkomst::Geen(waarom) => {
                tracing::info!(source = %source.name, %waarom, "source not available")
            }
        }
        Self {
            naam: source.name.clone(),
            herkomst,
            cache: Cache::default(),
        }
    }

    /// The source now, or why not. `github` is needed only for a source on
    /// GitHub; without a client such a source answers as a failed fetch.
    pub async fn geef(
        &self,
        github: Option<&regelrecht_github::GithubClient>,
    ) -> Result<Arc<Opgehaald>, BronFout> {
        match &self.herkomst {
            Herkomst::Geen(waarom) => Err(BronFout::NietBeschikbaar(waarom.clone())),
            Herkomst::Lokaal(dir) => {
                let dir = dir.clone();
                let bestanden = tokio::task::spawn_blocking(move || uit_map(&dir))
                    .await
                    .map_err(|e| BronFout::Ophalen(e.to_string()))?
                    .map_err(BronFout::Ophalen)?;
                Ok(Arc::new(Opgehaald {
                    naam: self.naam.clone(),
                    versie: Some("lokaal".into()),
                    bestanden,
                    melding: None,
                }))
            }
            Herkomst::Github {
                repo,
                branch,
                path,
                token,
            } => self
                .cache
                .geef(Instant::now(), || async {
                    let github = github.ok_or("no GitHub client in this process")?;
                    let bytes = github
                        .fetch_tarball(repo, branch, Some(token))
                        .await
                        .map_err(|e| format!("GitHub: {e}"))?;
                    let path = path.clone();
                    let (versie, bestanden) =
                        tokio::task::spawn_blocking(move || uit_tarball(&bytes, &path))
                            .await
                            .map_err(|e| e.to_string())??;
                    Ok(Opgehaald {
                        naam: self.naam.clone(),
                        versie,
                        bestanden,
                        melding: None,
                    })
                })
                .await
                .map_err(BronFout::Ophalen),
        }
    }
}

/// Why a source is not served.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BronFout {
    /// Not configured here (no token, no local folder): 503.
    #[error("bron niet beschikbaar: {0}")]
    NietBeschikbaar(String),
    /// Configured, but the fetch failed and there is no copy: 502.
    #[error("bron kon niet worden opgehaald: {0}")]
    Ophalen(String),
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::registry::GithubSource;

    /// A gzipped tar laid out as GitHub's tarball endpoint lays it out.
    fn tarball(top: &str, files: &[(&str, &str)]) -> Vec<u8> {
        let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        let mut tar = tar::Builder::new(gz);
        // GitHub's archive opens with a pax global header, before the
        // top-level directory.
        let comment = b"52 comment=0123456789abcdef0123456789abcdef01234567\n";
        let mut pax = tar::Header::new_ustar();
        pax.set_entry_type(tar::EntryType::XGlobalHeader);
        pax.set_size(comment.len() as u64);
        pax.set_mode(0o644);
        tar.append_data(&mut pax, "pax_global_header", &comment[..])
            .unwrap();
        let mut dir = tar::Header::new_gnu();
        dir.set_entry_type(tar::EntryType::Directory);
        dir.set_size(0);
        dir.set_mode(0o755);
        tar.append_data(&mut dir, format!("{top}/"), std::io::empty())
            .unwrap();
        for (path, text) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(text.len() as u64);
            header.set_mode(0o644);
            tar.append_data(&mut header, format!("{top}/{path}"), text.as_bytes())
                .unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap()
    }

    fn source() -> Source {
        Source {
            name: "voorbeeld".into(),
            github: GithubSource {
                owner: "example-org".into(),
                repo: "regelrecht-corpus-example".into(),
                branch: "poc/x".into(),
                path: "x".into(),
            },
            token_env: "EXAMPLE_TOKEN".into(),
        }
    }

    fn opgehaald(versie: &str) -> Opgehaald {
        Opgehaald {
            naam: "voorbeeld".into(),
            versie: Some(versie.into()),
            bestanden: BTreeMap::from([("a.yaml".into(), "a: 1".into())]),
            melding: None,
        }
    }

    #[test]
    fn a_tarball_becomes_the_yaml_files_under_the_folder() {
        let bytes = tarball(
            "example-org-regelrecht-corpus-example-abc1234",
            &[
                ("x/casussen/c.yaml", "cel: c"),
                ("x/regulation/nl/w/2026-01-01.yaml", "$id: w"),
                ("x/README.md", "not yaml"),
                ("y/elders.yaml", "not under x"),
                ("x-other/z.yaml", "a sibling, not under x"),
            ],
        );
        let (versie, bestanden) = uit_tarball(&bytes, "x").unwrap();
        assert_eq!(versie.as_deref(), Some("abc1234"));
        assert_eq!(
            bestanden,
            BTreeMap::from([
                ("casussen/c.yaml".to_string(), "cel: c".to_string()),
                (
                    "regulation/nl/w/2026-01-01.yaml".to_string(),
                    "$id: w".to_string()
                ),
            ])
        );
    }

    #[test]
    fn a_tarball_without_the_folder_is_an_error() {
        let bytes = tarball("o-r-abc", &[("y/a.yaml", "a: 1")]);
        let e = uit_tarball(&bytes, "x").unwrap_err();
        assert!(e.contains("'x'"), "{e}");
        assert!(uit_tarball(b"no tarball", "x").is_err());
    }

    #[test]
    fn a_local_folder_becomes_its_yaml_files() {
        let dir = std::env::temp_dir().join(format!("poc-bron-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("casussen")).unwrap();
        std::fs::write(dir.join("casussen/c.yaml"), "cel: c").unwrap();
        std::fs::write(dir.join("notes.txt"), "no").unwrap();
        let bestanden = uit_map(&dir).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            bestanden,
            BTreeMap::from([("casussen/c.yaml".to_string(), "cel: c".to_string())])
        );
        assert!(uit_map(Path::new("/nonexistent/poc-bron")).is_err());
    }

    #[test]
    fn a_local_folder_wins_over_the_token_and_without_both_there_is_none() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |k: &str| {
                pairs
                    .iter()
                    .find(|(n, _)| *n == k)
                    .map(|(_, v)| v.to_string())
            }
        };
        let s = source();
        assert_eq!(
            Herkomst::uit(
                &s,
                env(&[("POC_BRON_VOORBEELD", "/d"), ("EXAMPLE_TOKEN", "t")])
            ),
            Herkomst::Lokaal("/d".into())
        );
        assert_eq!(
            Herkomst::uit(&s, env(&[("EXAMPLE_TOKEN", "t")])),
            Herkomst::Github {
                repo: "example-org/regelrecht-corpus-example".into(),
                branch: "poc/x".into(),
                path: "x".into(),
                token: "t".into(),
            }
        );
        // An empty variable is no token.
        let Herkomst::Geen(waarom) = Herkomst::uit(&s, env(&[("EXAMPLE_TOKEN", " ")])) else {
            panic!("no source without a token");
        };
        assert!(waarom.contains("EXAMPLE_TOKEN"), "{waarom}");
    }

    #[tokio::test]
    async fn a_fetch_is_served_until_the_ttl_has_passed() {
        let cache = Cache::new(Duration::from_secs(60));
        let haal = AtomicUsize::new(0);
        let t0 = Instant::now();
        let fetch = |v: &'static str| {
            haal.fetch_add(1, Ordering::SeqCst);
            async move { Ok(opgehaald(v)) }
        };
        assert_eq!(
            cache
                .geef(t0, || fetch("a"))
                .await
                .unwrap()
                .versie
                .as_deref(),
            Some("a")
        );
        let binnen = t0 + Duration::from_secs(59);
        assert_eq!(
            cache
                .geef(binnen, || fetch("b"))
                .await
                .unwrap()
                .versie
                .as_deref(),
            Some("a")
        );
        assert_eq!(haal.load(Ordering::SeqCst), 1);
        let na = t0 + Duration::from_secs(60);
        assert_eq!(
            cache
                .geef(na, || fetch("b"))
                .await
                .unwrap()
                .versie
                .as_deref(),
            Some("b")
        );
        assert_eq!(haal.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_failed_fetch_serves_the_last_good_copy_and_says_so() {
        let cache = Cache::new(Duration::from_secs(60));
        let t0 = Instant::now();
        cache
            .geef(t0, || async { Ok(opgehaald("a")) })
            .await
            .unwrap();
        let later = t0 + Duration::from_secs(61);
        let oud = cache
            .geef(later, || async { Err("GitHub: 502".to_string()) })
            .await
            .unwrap();
        assert_eq!(oud.versie.as_deref(), Some("a"));
        assert!(oud.melding.as_deref().unwrap().contains("GitHub: 502"));
        // GitHub is not asked again within the TTL of that failure.
        let nog = cache
            .geef(later + Duration::from_secs(1), || async {
                panic!("fetched again within the TTL")
            })
            .await
            .unwrap();
        assert_eq!(nog.versie.as_deref(), Some("a"));
        // A fetch that succeeds again drops the message.
        let weer = cache
            .geef(later + Duration::from_secs(61), || async {
                Ok(opgehaald("b"))
            })
            .await
            .unwrap();
        assert_eq!(weer.melding, None);
    }

    #[tokio::test]
    async fn a_failed_fetch_without_a_copy_is_an_error_kept_briefly() {
        let cache = Cache::default();
        let t0 = Instant::now();
        let e = cache
            .geef(t0, || async { Err("GitHub: 404".to_string()) })
            .await
            .unwrap_err();
        assert_eq!(e, "GitHub: 404");
        // Within FOUT_TTL the next visitor gets the error without a request.
        let e = cache
            .geef(t0 + Duration::from_secs(9), || async {
                panic!("fetched again within FOUT_TTL")
            })
            .await
            .unwrap_err();
        assert_eq!(e, "GitHub: 404");
        let o = cache
            .geef(t0 + FOUT_TTL, || async { Ok(opgehaald("a")) })
            .await
            .unwrap();
        assert_eq!(o.versie.as_deref(), Some("a"));
    }

    #[test]
    fn the_debug_form_of_a_source_hides_its_token() {
        let h = Herkomst::uit(&source(), |k| {
            (k == "EXAMPLE_TOKEN").then(|| "geheim".into())
        });
        let debug = format!("{h:?}");
        assert!(!debug.contains("geheim"), "{debug}");
        assert!(debug.contains("<hidden>"), "{debug}");
    }

    #[tokio::test]
    async fn a_source_without_a_token_is_not_available() {
        let bron = Bron::new(&source(), |_| None);
        let github = regelrecht_github::GithubClient::new().unwrap();
        let e = bron.geef(Some(&github)).await.unwrap_err();
        assert!(matches!(e, BronFout::NietBeschikbaar(_)), "{e}");
        assert!(e.to_string().starts_with("bron niet beschikbaar"), "{e}");
    }

    #[tokio::test]
    async fn a_source_is_fetched_from_github_with_the_token() {
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(
                "/repos/example-org/regelrecht-corpus-example/tarball/poc/x",
            ))
            .and(header("authorization", "Bearer geheim"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball(
                "example-org-regelrecht-corpus-example-def5678901234567890123456789012345678901",
                &[("x/casussen/c.yaml", "cel: c")],
            )))
            .expect(1)
            .mount(&server)
            .await;
        let github = regelrecht_github::GithubClient::new()
            .unwrap()
            .with_base_url(server.uri());
        let bron = Bron::new(&source(), |k| {
            (k == "EXAMPLE_TOKEN").then(|| "geheim".into())
        });
        let o = bron.geef(Some(&github)).await.unwrap();
        assert_eq!(o.versie.as_deref(), Some("def5678"));
        assert_eq!(o.bestanden["casussen/c.yaml"], "cel: c");
        // Within the TTL the second visitor gets the same copy (`expect(1)`).
        bron.geef(Some(&github)).await.unwrap();
    }
}
