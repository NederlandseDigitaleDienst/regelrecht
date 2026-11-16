//! The regelrecht commit this binary was built from, served at
//! [`VERSION_PATH`].
//!
//! A test harness reads it to run its scenarios against exactly the commit
//! that is live. The commit comes in at compile time through the
//! `REGELRECHT_COMMIT` environment variable (the Dockerfile passes it as a
//! build argument) and is checked here, at compile time too: only a full,
//! lowercase 40-character SHA gets through, and anything else becomes
//! [`UNKNOWN`]. The endpoint reads nothing from the request, so what it
//! answers is fixed when the binary is built.

use axum::Json;
use axum::http::{HeaderValue, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// The route. A slug may only contain `a-z`, `0-9` and `-`
/// ([`crate::registry`]), so a path starting with `_` cannot shadow a PoC.
pub const VERSION_PATH: &str = "/_version";

/// What the endpoint reports when the build passed no valid commit, as in a
/// local `cargo build`.
pub const UNKNOWN: &str = "unknown";

/// The commit of this build, checked when the crate is compiled.
pub const BUILD_COMMIT: Commit = Commit::from_build_value(option_env!("REGELRECHT_COMMIT"));

/// A full git commit SHA (40 lowercase hex characters), or [`UNKNOWN`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Commit(&'static str);

impl Commit {
    /// Accept `raw` only when it is a full lowercase SHA; anything else,
    /// including an absent value, becomes [`UNKNOWN`]. A `const fn`, so the
    /// check on [`BUILD_COMMIT`] runs in the compiler and a malformed build
    /// argument never reaches the binary.
    pub const fn from_build_value(raw: Option<&'static str>) -> Self {
        match raw {
            Some(sha) if is_full_sha(sha) => Self(sha),
            _ => Self(UNKNOWN),
        }
    }

    /// The SHA, or [`UNKNOWN`].
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// Whether `s` is exactly 40 characters of `0-9a-f`.
const fn is_full_sha(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 40 {
        return false;
    }
    let mut i = 0;
    while i < bytes.len() {
        if !matches!(bytes[i], b'0'..=b'9' | b'a'..=b'f') {
            return false;
        }
        i += 1;
    }
    true
}

#[derive(Serialize)]
struct VersionBody {
    commit: &'static str,
}

/// `GET /_version`: `{"commit":"<sha>"}` or `{"commit":"unknown"}`, without a
/// password. It takes no extractors, so no part of the request can reach the
/// answer. `no-store`, so a cache in between never reports a commit that has
/// since been replaced.
pub async fn version() -> Response {
    let mut response = Json(VersionBody {
        commit: BUILD_COMMIT.as_str(),
    })
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn a_full_lowercase_sha_is_kept() {
        assert_eq!(Commit::from_build_value(Some(SHA)).as_str(), SHA);
    }

    #[test]
    fn an_absent_value_is_unknown() {
        assert_eq!(Commit::from_build_value(None).as_str(), UNKNOWN);
    }

    #[test]
    fn a_malformed_value_is_unknown() {
        for raw in [
            "",
            "unknown",
            // A short SHA, one character short, one too long.
            "0123456",
            &SHA[..39],
            "0123456789abcdef0123456789abcdef012345678",
            // Uppercase hex is a valid SHA to git but not the form we promise.
            "0123456789ABCDEF0123456789ABCDEF01234567",
            // Forty characters that try to break out of the JSON string or
            // smuggle a header.
            "0123456789abcdef0123456789abcdef012345\"}",
            "0123456789abcdef0123456789abcdef01234\r\nX",
            "0123456789abcdef0123456789abcdef0123456 ",
            "<script>alert(1)</script>00000000000000",
        ] {
            assert_eq!(
                Commit::from_build_value(Some(raw)).as_str(),
                UNKNOWN,
                "{raw:?} should not pass"
            );
        }
    }
}
