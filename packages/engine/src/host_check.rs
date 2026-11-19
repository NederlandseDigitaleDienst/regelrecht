//! Warnings for a `host` that cannot be right (RFC-058).
//!
//! A filling of a hook's open term that names its host (`implements.host`,
//! schema v0.8.24) applies only while the hook fires on that article, and a
//! hook that names a host (`applies_to.host`, schema v0.8.25) fires only on
//! that law's or article's decisions. A host that names the wrong thing does
//! not fail: the filling never applies, or the hook never fires, and nothing
//! in the outcome says so. Behavior stays that way, since the host may sit in
//! a regulation that is not loaded. This module catches the mistakes that
//! can be seen in the regulation itself, as warnings and never as load
//! errors:
//!
//! - the host law is known, but has no article with that number, or that
//!   article gives no decision (and establishes no submission) a hook fires
//!   on;
//! - a filling asks for a required parameter that the host article has
//!   neither as a parameter nor as an input, nor as an output where the hook
//!   fires after the actions.
//!
//! A host law that `lookup` cannot find gives no warning: it may come from
//! another corpus. `just validate` prints these as `WARN`, and the loader
//! logs them.
//!
//! The check is narrower than the engine. It reads the version `lookup`
//! gives (the newest), so an article that only an older version in force
//! has is reported as missing. It does not count the outputs of the
//! `pre_actions` hooks, which the scope of a `post_actions` hook also holds.
//! And a host that names a law without an article is not checked.

use std::collections::BTreeSet;
use std::fmt;

use crate::article::{Article, ArticleBasedLaw};
use regelrecht_law_model::{HookPoint, ImplementsDeclaration};

/// One host that cannot be right, in article `article` of law `law_id`,
/// naming law `host_law`. For a filling, `hook_law` is the law of the hook
/// article whose term it fills, which decides whether outputs count.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HostWarning {
    pub law_id: String,
    pub article: String,
    pub host_law: String,
    pub hook_law: Option<String>,
    pub message: String,
}

impl HostWarning {
    /// Whether the warning rests on what law `law_id` contains, so that
    /// loading that law can bring it about.
    pub(crate) fn rests_on(&self, law_id: &str) -> bool {
        self.host_law == law_id || self.hook_law.as_deref() == Some(law_id)
    }
}

impl fmt::Display for HostWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} article {}: {}",
            self.law_id, self.article, self.message
        )
    }
}

/// Check every `host` in `law`. `lookup` finds the laws a host may name (the
/// other laws of a set being validated, or the laws already loaded); a law it
/// cannot find is skipped.
pub fn check_law<'l>(
    law: &'l ArticleBasedLaw,
    lookup: &dyn Fn(&str) -> Option<&'l ArticleBasedLaw>,
) -> Vec<HostWarning> {
    let mut warnings = Vec::new();
    for article in &law.articles {
        let mut warn = |host_law: &str, hook_law: Option<&str>, message: String| {
            warnings.push(HostWarning {
                law_id: law.id.clone(),
                article: article.number.clone(),
                host_law: host_law.to_string(),
                hook_law: hook_law.map(str::to_string),
                message,
            });
        };
        for filling in article.get_implements().into_iter().flatten() {
            if let (Some(host), Some((message, reads_hook))) =
                (&filling.host, check_filling(article, filling, lookup))
            {
                let hook_law = reads_hook.then_some(filling.law.as_str());
                warn(&host.law, hook_law, message);
            }
        }
        for hook in article.get_hooks().into_iter().flatten() {
            let filter = &hook.applies_to;
            let (Some(host), Some(legal_character)) = (&filter.host, &filter.legal_character)
            else {
                continue;
            };
            let (Some(number), Some(host_law)) = (&host.article, lookup(&host.law)) else {
                continue;
            };
            let decision_type = filter.decision_type.as_deref();
            let gives = |a: &Article| {
                a.get_decision().is_some_and(|d| {
                    d.legal_character() == Some(legal_character.as_str())
                        && decision_type.is_none_or(|dt| d.decision_type() == Some(dt))
                })
            };
            let kind = decision_type.map_or(legal_character.clone(), |dt| {
                format!("{legal_character} {dt}")
            });
            match host_law.find_article_by_number(number) {
                None => warn(
                    &host.law,
                    None,
                    format!(
                        "a hook names host article {number} of '{}', which that law does not have; \
                     the hook never fires",
                        host.law
                    ),
                ),
                Some(a) if !gives(a) => warn(
                    &host.law,
                    None,
                    format!(
                        "a hook names host article {number} of '{}', which gives no {kind}; \
                     the hook never fires",
                        host.law
                    ),
                ),
                Some(_) => {}
            }
        }
    }
    warnings
}

/// The warning for one filling of `article` that names a host, if any, and
/// whether it rests on the hook article as well as on the host.
fn check_filling<'l>(
    article: &Article,
    filling: &ImplementsDeclaration,
    lookup: &dyn Fn(&str) -> Option<&'l ArticleBasedLaw>,
) -> Option<(String, bool)> {
    let host = filling.host.as_ref()?;
    let host_law = lookup(&host.law)?;
    let names = format!(
        "fills '{}' of {} article {} for host article {} of '{}'",
        filling.open_term, filling.law, filling.article, host.article, host.law
    );
    let Some(host_article) = host_law.find_article_by_number(&host.article) else {
        return Some((
            format!("{names}, which that law does not have; the filling never applies"),
            false,
        ));
    };
    if host_article.get_produced().is_empty() {
        return Some((
            format!(
                "{names}, which gives no decision and establishes no submission, so no hook \
                 fires on it; the filling never applies"
            ),
            false,
        ));
    }
    // The host's outputs are in the scope when the hook fires after the
    // actions. Where the hook article is not loaded, or declares no hooks
    // (and so fires nowhere), the check cannot tell, and counts them.
    let after_actions = lookup(&filling.law)
        .and_then(|l| l.find_article_by_number(&filling.article))
        .and_then(Article::get_hooks)
        .is_none_or(|hooks| hooks.iter().any(|h| h.hook_point == HookPoint::PostActions));
    let scope = host_scope(host_article, after_actions);
    let missing: Vec<&str> = article
        .get_parameters()
        .iter()
        .filter(|p| p.required != Some(false) && !scope.contains(p.name.as_str()))
        .map(|p| p.name.as_str())
        .collect();
    if missing.is_empty() {
        return None;
    }
    let message = format!(
        "{names}, and asks for required parameter(s) '{}', which that article does not hand \
         its hooks; under the hook the filling fails with VariableNotFound",
        missing.join("', '")
    );
    Some((message, true))
}

/// The names in the scope the host article hands its hooks: its parameters
/// and inputs, and with `after_actions` its outputs.
fn host_scope(host: &Article, after_actions: bool) -> BTreeSet<&str> {
    let parameters = host.get_parameters().iter().map(|p| p.name.as_str());
    let inputs = host.get_inputs().iter().map(|i| i.name.as_str());
    let outputs = host
        .get_outputs()
        .iter()
        .map(|o| o.name.as_str())
        .filter(|_| after_actions);
    parameters.chain(inputs).chain(outputs).collect()
}

/// Whether loading `law_id` can change what [`check_law`] says about `law`:
/// `law` names it as a host, or fills with a host a term of it.
pub(crate) fn depends_on(law: &ArticleBasedLaw, law_id: &str) -> bool {
    law.articles.iter().any(|article| {
        article.get_implements().into_iter().flatten().any(|i| {
            i.host
                .as_ref()
                .is_some_and(|h| h.law == law_id || i.law == law_id)
        }) || article
            .get_hooks()
            .into_iter()
            .flatten()
            .any(|h| h.applies_to.host.as_ref().is_some_and(|h| h.law == law_id))
    })
}
