//! Cache-Control policy for the editor's static files.
//!
//! The editor image has no nginx in front of it: `editor-api` is the web
//! server for `frontend/dist` as well as the API, so caching policy has to
//! be decided here or not at all. Until now it was not decided at all —
//! every response left without a `Cache-Control`, which makes a returning
//! visitor re-fetch the whole bundle or, at best, revalidate every file
//! one by one.
//!
//! Two classes, and the split is the whole point:
//!
//! * **Content-hashed bundle files** (`/assets/index-DkX9a2Bc.js`). The
//!   hash is derived from the contents, so the URL can never denote
//!   different bytes. These get a year plus `immutable`: no request at
//!   all on a repeat visit, not even a conditional one.
//! * **Everything else** — `index.html`, the SPA fallback for deep
//!   links, `favorites.json`, `regelrecht-icon.svg`. These live at fixed
//!   names and their contents do change, so they get `no-cache`:
//!   *revalidate before use*, not "never store". The browser keeps the
//!   body and a `304` confirms it, which is the cheap answer we want.
//!
//! Getting the split wrong in the immutable direction is expensive and
//! unfixable from the server side: a visitor holding a stale `immutable`
//! entry will not ask again for a year, and no deploy reaches them. So
//! the predicate below errs toward `no-cache` whenever a name does not
//! unambiguously carry a hash.

use axum::http::header::{CACHE_CONTROL, HeaderValue};
use axum::http::{Response, StatusCode, Uri};

/// One year, the maximum `max-age` RFC 9111 recommends anyone bother
/// with, plus `immutable` so browsers skip the revalidation they would
/// otherwise still send on a reload.
pub const IMMUTABLE: HeaderValue = HeaderValue::from_static("public, max-age=31536000, immutable");

/// Store it, but check with us before using it. Not `no-store`: we want
/// the body kept so revalidation can answer `304` off the ETag that
/// `ServeDir` already sets.
pub const REVALIDATE: HeaderValue = HeaderValue::from_static("no-cache");

/// Directory Vite writes its hashed output to. Nothing else is emitted
/// here: `public/` is copied to the root of `dist`, and the build has no
/// `assetFileNames` override, so every file under this prefix carries a
/// content hash.
const HASHED_DIR: &str = "/assets/";

/// Vite's default hash is 8 base64url characters. Shorter runs are
/// ordinary words in a kebab-case filename.
const MIN_HASH_LEN: usize = 8;

/// Set the cache policy on a static-file response.
///
/// Takes the *request* URI rather than reading the response, because the
/// decision is about the URL's stability, not about the bytes. The
/// status is consulted for one reason: a request for a missing
/// `/assets/…` file falls through to the SPA index, and that HTML must
/// never be cached for a year under an asset URL. Only a response that
/// actually delivered (or confirmed) the asset may be marked immutable.
pub fn apply<B>(uri: &Uri, response: &mut Response<B>) {
    let immutable = response_is_authoritative(response.status()) && is_hashed_asset(uri.path());
    let value = if immutable { IMMUTABLE } else { REVALIDATE };
    response.headers_mut().insert(CACHE_CONTROL, value);
}

/// `200`, `304` and `206` mean the URL resolved to the file it names.
/// Anything else (notably the `404` the SPA fallback carries) is some
/// other resource wearing this URL and must stay revalidated.
fn response_is_authoritative(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::OK | StatusCode::NOT_MODIFIED | StatusCode::PARTIAL_CONTENT
    )
}

/// Whether a request path denotes a content-hashed bundle file.
///
/// Two gates. The path must sit under [`HASHED_DIR`], and the filename
/// must carry a hash: a `-` followed by at least [`MIN_HASH_LEN`]
/// base64url characters, up to the first `.`, containing a digit or an
/// uppercase letter.
///
/// That last condition is what separates `index-DkX9a2Bc.js` from a
/// hand-placed `some-long-name.css`, whose tail is equally long and
/// equally base64url but reads as words. It costs us the ~1-in-1000 hash
/// that happens to be all-lowercase — that file merely revalidates,
/// which is the direction to be wrong in.
pub fn is_hashed_asset(path: &str) -> bool {
    let Some(rest) = path.strip_prefix(HASHED_DIR) else {
        return false;
    };
    // Nested chunk directories are still Vite output; take the leaf.
    let name = rest.rsplit('/').next().unwrap_or("");
    let stem = strip_extensions(name);

    stem.char_indices()
        .filter(|(_, c)| *c == '-')
        .any(|(i, _)| looks_like_hash(&stem[i + 1..]))
}

/// Drop the file extension(s), leaving the part the hash lives in.
///
/// Not "everything before the first dot": Vite happily emits dots inside
/// the name it derives from a source module, as in
/// `runtime-core.esm-bundler-yi8_EWx1.js`. Instead, strip from the right
/// while the trailing segment reads as an extension — short and purely
/// alphanumeric. Twice at most, which covers `.js.map` and stops well
/// short of eating a dotted name.
fn strip_extensions(name: &str) -> &str {
    let mut stem = name;
    for _ in 0..2 {
        let Some((head, ext)) = stem.rsplit_once('.') else {
            break;
        };
        let is_extension = (1..=5).contains(&ext.len())
            && !ext.is_empty()
            && ext.chars().all(|c| c.is_ascii_alphanumeric());
        if !is_extension {
            break;
        }
        stem = head;
    }
    stem
}

fn looks_like_hash(tail: &str) -> bool {
    tail.len() >= MIN_HASH_LEN
        && tail
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && tail.chars().any(|c| c.is_ascii_digit() || c.is_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hashed(path: &str) -> bool {
        is_hashed_asset(path)
    }

    /// The shapes Vite actually emits, including a hash that contains the
    /// base64url specials — those must not be mistaken for a word break.
    #[test]
    fn vite_output_is_hashed() {
        assert!(hashed("/assets/index-DkX9a2Bc.js"));
        assert!(hashed("/assets/index-DkX9a2Bc.css"));
        assert!(hashed("/assets/index-DkX9a2Bc.js.map"));
        assert!(hashed("/assets/vendor-router-B7f_2a-x.js"));
        assert!(hashed("/assets/RegelrechtSans-a1B2c3D4.woff2"));
        assert!(hashed("/assets/chunks/deep-Qq8XmZ01.js"));
        // Vite derives chunk names from the source module, dots and all —
        // these two are verbatim from a real `frontend/dist`.
        assert!(hashed("/assets/runtime-core.esm-bundler-yi8_EWx1.js"));
        assert!(hashed("/assets/JetBrainsMono-Italic_wght_-Ljgv2psh.woff2"));
    }

    /// Fixed names are the whole risk. None of these may ever be handed a
    /// year, wherever they sit.
    #[test]
    fn fixed_names_are_not_hashed() {
        assert!(!hashed("/index.html"));
        assert!(!hashed("/favorites.json"));
        assert!(!hashed("/regelrecht-icon.svg"));
        assert!(!hashed("/favicon.ico"));
        assert!(!hashed("/manifest.webmanifest"));
        assert!(!hashed("/assets/style.css"));
        assert!(!hashed("/assets/logo.svg"));
        assert!(!hashed("/assets/some-long-name.css"));
        assert!(!hashed("/assets/fonts/regelrecht-sans.woff2"));
    }

    /// A hash-shaped name outside `/assets/` is still a fixed URL as far
    /// as we know; only the build directory carries the guarantee.
    #[test]
    fn the_prefix_is_required() {
        assert!(!hashed("/index-DkX9a2Bc.js"));
        assert!(!hashed("/static/assets/index-DkX9a2Bc.js"));
    }

    /// Too short to be a Vite hash — `-min`, `-v2`, `-nl` and friends.
    #[test]
    fn short_suffixes_are_not_hashes() {
        assert!(!hashed("/assets/app-v2.js"));
        assert!(!hashed("/assets/bundle-min.js"));
    }

    fn apply_to(path: &str, status: StatusCode) -> HeaderValue {
        let mut response = Response::builder()
            .status(status)
            .body(())
            .expect("response");
        apply(&path.parse::<Uri>().expect("uri"), &mut response);
        response.headers()[CACHE_CONTROL].clone()
    }

    #[test]
    fn hashed_assets_get_a_year() {
        assert_eq!(
            apply_to("/assets/index-DkX9a2Bc.js", StatusCode::OK),
            IMMUTABLE
        );
        assert_eq!(
            apply_to("/assets/index-DkX9a2Bc.js", StatusCode::NOT_MODIFIED),
            IMMUTABLE
        );
    }

    #[test]
    fn the_index_revalidates() {
        assert_eq!(apply_to("/index.html", StatusCode::OK), REVALIDATE);
        assert_eq!(apply_to("/", StatusCode::OK), REVALIDATE);
    }

    /// A missing asset is answered with the SPA index. Caching that HTML
    /// for a year under a `.js` URL would break the app for exactly as
    /// long, so the status gate has to hold.
    #[test]
    fn a_missing_asset_never_gets_a_year() {
        assert_eq!(
            apply_to("/assets/gone-DkX9a2Bc.js", StatusCode::NOT_FOUND),
            REVALIDATE
        );
    }

    /// Whatever a previous layer put there, ours is the policy that ships.
    #[test]
    fn an_existing_header_is_replaced() {
        let mut response = Response::builder()
            .status(StatusCode::OK)
            .header(CACHE_CONTROL, "private")
            .body(())
            .expect("response");
        apply(
            &"/assets/index-DkX9a2Bc.js".parse::<Uri>().expect("uri"),
            &mut response,
        );
        assert_eq!(
            response.headers().get_all(CACHE_CONTROL).iter().count(),
            1,
            "the old value must be replaced, not appended"
        );
        assert_eq!(response.headers()[CACHE_CONTROL], IMMUTABLE);
    }
}
