use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::ApiState;

const SRU_BASE: &str = "https://zoekservice.overheid.nl/sru/Search";
const MAX_RESULTS: u32 = 20;

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct BwbSearchResult {
    pub bwb_id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub law_type: String,
}

/// GET /harvest/search?q=...
///
/// Search wetten.overheid.nl via the SRU API for laws matching the query.
pub async fn search_bwb(
    State(state): State<ApiState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<BwbSearchResult>>, (StatusCode, String)> {
    match search_bwb_by_name(&state.http_client, params.q.trim()).await {
        Ok(results) => Ok(Json(results)),
        Err(e) => Err((StatusCode::BAD_GATEWAY, e)),
    }
}

/// Search wetten.overheid.nl via the SRU API for laws matching `q`.
///
/// The client-taking core shared by the axum handler and the enrich worker's
/// related-legislation resolution. Queries shorter than 3 characters (after the
/// same sanitize as the handler) return an empty list rather than an error.
pub async fn search_bwb_by_name(
    client: &reqwest::Client,
    q: &str,
) -> Result<Vec<BwbSearchResult>, String> {
    search_bwb_at(SRU_BASE, client, q).await
}

/// `search_bwb_by_name` against an explicit SRU base URL, so a test can point it
/// at a local server.
async fn search_bwb_at(
    base: &str,
    client: &reqwest::Client,
    q: &str,
) -> Result<Vec<BwbSearchResult>, String> {
    let q = q.trim();
    if q.len() < 3 {
        return Ok(vec![]);
    }

    let sanitized: String = q
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '.')
        .collect();
    let cql = format!("overheidbwb.titel any \"{sanitized}\"");

    let url = url::Url::parse_with_params(
        base,
        &[
            ("operation", "searchRetrieve"),
            ("version", "1.2"),
            ("x-connection", "BWB"),
            ("query", &cql),
            ("maximumRecords", &MAX_RESULTS.to_string()),
        ],
    )
    .map_err(|e| format!("URL build error: {e}"))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("BWB search failed: {e}"))?;

    // Zonder deze poort leest elk niet-XML antwoord — een 429 bij throttling,
    // een 503 bij onderhoud — als een lege trefferlijst, en dus als "die wet
    // bestaat niet". De enrich-worker zet die uitkomst om in Unresolved en gaat
    // door: stil onvolledige verrijking.
    let status = response.status();
    if !status.is_success() {
        return Err(format!("BWB search returned HTTP {status}"));
    }

    let xml_text = response
        .text()
        .await
        .map_err(|e| format!("BWB response read failed: {e}"))?;

    parse_sru_response(&xml_text).map_err(|e| format!("XML parse error: {e}"))
}

/// Parse SRU XML response and extract unique laws (deduplicated by BWBR ID).
fn parse_sru_response(xml: &str) -> Result<Vec<BwbSearchResult>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;

    // SRU returns results in relevance order — preserve that ordering by
    // deduplicating via a HashSet of seen IDs while pushing into a Vec.
    let mut seen: HashSet<String> = HashSet::new();
    let mut results: Vec<BwbSearchResult> = Vec::new();

    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }
        if node.tag_name().name() != "owmskern" {
            continue;
        }

        let mut identifier = None;
        let mut title = None;
        let mut law_type = None;

        for child in node.children().filter(|n| n.is_element()) {
            match child.tag_name().name() {
                "identifier" => identifier = child.text().map(|s| s.trim().to_string()),
                "title" => title = child.text().map(|s| s.trim().to_string()),
                "type" => law_type = child.text().map(|s| s.trim().to_string()),
                _ => {}
            }
        }

        if let (Some(bwb_id), Some(title)) = (identifier, title) {
            if !bwb_id.starts_with("BWBR") {
                continue;
            }
            if seen.insert(bwb_id.clone()) {
                results.push(BwbSearchResult {
                    bwb_id,
                    title,
                    law_type: law_type.unwrap_or_default(),
                });
            }
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn search_against(status: u16, body: &str) -> Result<Vec<BwbSearchResult>, String> {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(status).set_body_string(body))
            .mount(&server)
            .await;

        search_bwb_at(&server.uri(), &reqwest::Client::new(), "zorgtoeslag").await
    }

    const ONE_HIT: &str = r#"<srw:searchRetrieveResponse xmlns:srw="http://www.loc.gov/zing/srw/">
        <owmskern><identifier>BWBR0018451</identifier><title>Wet op de zorgtoeslag</title>
        <type>wet</type></owmskern></srw:searchRetrieveResponse>"#;

    #[tokio::test]
    async fn a_hit_is_returned() {
        let results = search_against(200, ONE_HIT).await.expect("200 with XML");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].bwb_id, "BWBR0018451");
    }

    #[tokio::test]
    async fn throttling_is_an_error_and_not_an_empty_result() {
        let err = search_against(429, "rate limited")
            .await
            .expect_err("429 must not read as 'no such law'");
        assert!(err.contains("429"), "message loses the status: {err}");
    }

    #[tokio::test]
    async fn a_server_error_is_an_error() {
        let err = search_against(503, "service unavailable")
            .await
            .expect_err("503 must not read as 'no such law'");
        assert!(err.contains("503"), "message loses the status: {err}");
    }

    #[tokio::test]
    async fn an_empty_result_set_stays_empty() {
        let empty = r#"<srw:searchRetrieveResponse xmlns:srw="http://www.loc.gov/zing/srw/"/>"#;
        let results = search_against(200, empty).await.expect("200 with XML");
        assert!(results.is_empty());
    }
}
