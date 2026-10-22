use std::collections::HashSet;

use regelrecht_law_model::RegulatoryLayer;

use crate::models::{Scope, Source};
use crate::source_map::SourceMap;

/// A validation warning for a law outside its source's scope.
#[derive(Debug, Clone)]
pub struct ScopeWarning {
    pub law_id: String,
    pub source_id: String,
    pub source_name: String,
    pub expected_scopes: Vec<String>,
    pub actual_scope_code: Option<String>,
    pub message: String,
}

/// Validate that sources only provide laws within their declared scopes.
///
/// Returns warnings for laws that appear to be outside their source's
/// jurisdictional scope. A source with empty scopes is unrestricted.
///
/// Every version is checked, not only the one that wins on priority: a scoped
/// source that loses on priority still supplies the versions for dates the
/// winning source lacks, and the engine loads those.
///
/// The checks read the regulation body. A version whose body has not been
/// fetched yet (a GitHub source enumerated by path only) cannot be checked and
/// is skipped.
pub fn validate_scopes(source_map: &SourceMap, sources: &[Source]) -> Vec<ScopeWarning> {
    let mut warnings = Vec::new();
    let mut seen: HashSet<(String, String, String)> = HashSet::new();

    for law in source_map.all_versions() {
        let source = match sources.iter().find(|s| s.id == law.source_id) {
            Some(s) => s,
            None => continue,
        };

        // Unrestricted sources don't need scope validation, and an unfetched
        // body has nothing to check.
        if source.scopes.is_empty() || !law.is_loaded() {
            continue;
        }

        let expected_scopes = || -> Vec<String> {
            source
                .scopes
                .iter()
                .map(|s| format!("{}:{}", s.scope_type, s.value))
                .collect()
        };
        let mut push = |warning: ScopeWarning| {
            let key = (
                warning.law_id.clone(),
                warning.source_id.clone(),
                warning.message.clone(),
            );
            if seen.insert(key) {
                warnings.push(warning);
            }
        };

        // Extract scope codes from the YAML content
        let gemeente_code = extract_gemeente_code(&law.yaml_content);
        let waterschap_code = extract_waterschap_code(&law.yaml_content);

        // A scoped source publishes one jurisdiction's regulations. A national
        // regulation coming from it is either a copy or an attempt to replace
        // the national text for everyone, and a municipal or water board
        // source can do neither: a decentral ordinance can execute, fill in or
        // supplement a national law, but not replace it (RFC-010, review #1099).
        match extract_regulatory_layer(&law.yaml_content) {
            Some(LayerField::Known(layer)) if is_national_layer(layer) => push(ScopeWarning {
                law_id: law.law_id.clone(),
                source_id: law.source_id.clone(),
                source_name: law.source_name.clone(),
                expected_scopes: expected_scopes(),
                actual_scope_code: None,
                message: format!(
                    "Law '{}' from scoped source '{}' is a national regulation ({}); a decentral source cannot provide or replace national law",
                    law.law_id,
                    source.id,
                    layer.as_str()
                ),
            }),
            Some(LayerField::Unknown(raw)) => push(ScopeWarning {
                law_id: law.law_id.clone(),
                source_id: law.source_id.clone(),
                source_name: law.source_name.clone(),
                expected_scopes: expected_scopes(),
                actual_scope_code: None,
                message: format!(
                    "Law '{}' from scoped source '{}' has an unknown regulatory_layer '{}'; cannot tell whether it is national",
                    law.law_id, source.id, raw
                ),
            }),
            _ => {}
        }

        if let Some(code) = &gemeente_code {
            if !scope_matches(&source.scopes, "gemeente_code", code) {
                push(ScopeWarning {
                    law_id: law.law_id.clone(),
                    source_id: law.source_id.clone(),
                    source_name: law.source_name.clone(),
                    expected_scopes: expected_scopes(),
                    actual_scope_code: gemeente_code.clone(),
                    message: format!(
                        "Law '{}' from source '{}' has gemeente_code '{}' which is outside declared scopes {:?}",
                        law.law_id,
                        source.id,
                        code,
                        source.scopes.iter().map(|s| &s.value).collect::<Vec<_>>()
                    ),
                });
            }
        }

        if let Some(code) = &waterschap_code {
            if !scope_matches(&source.scopes, "waterschap_code", code) {
                push(ScopeWarning {
                    law_id: law.law_id.clone(),
                    source_id: law.source_id.clone(),
                    source_name: law.source_name.clone(),
                    expected_scopes: expected_scopes(),
                    actual_scope_code: waterschap_code.clone(),
                    message: format!(
                        "Law '{}' from source '{}' has waterschap_code '{}' which is outside declared scopes {:?}",
                        law.law_id,
                        source.id,
                        code,
                        source.scopes.iter().map(|s| &s.value).collect::<Vec<_>>()
                    ),
                });
            }
        }
    }

    warnings
}

/// Check if a scope code matches any of the source's scopes for the given type.
///
/// Supports `gemeente_code` and `waterschap_code` scope types.
fn scope_matches(scopes: &[Scope], scope_type: &str, code: &str) -> bool {
    let matching_scopes: Vec<_> = scopes
        .iter()
        .filter(|s| s.scope_type == scope_type)
        .collect();

    // If the source has no scopes of this type, we cannot validate —
    // treat as matching (no warning).
    if matching_scopes.is_empty() {
        return true;
    }

    matching_scopes.iter().any(|scope| scope.value == code)
}

/// Extract top-level gemeente_code from YAML content using line-based parsing.
///
/// Only matches `gemeente_code:` at the start of a line (no leading whitespace)
/// to avoid matching nested fields, consistent with `extract_law_id`.
fn extract_gemeente_code(yaml: &str) -> Option<String> {
    for line in yaml.lines() {
        if let Some(rest) = line.strip_prefix("gemeente_code:") {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// The top-level `regulatory_layer` of a regulation, as far as it can be read.
#[derive(Debug, PartialEq, Eq)]
enum LayerField {
    Known(RegulatoryLayer),
    Unknown(String),
}

/// Extract the top-level regulatory_layer from YAML content.
///
/// Uses the shared tolerant header scan, and drops a trailing `# comment`
/// that the scan leaves in the value.
fn extract_regulatory_layer(yaml: &str) -> Option<LayerField> {
    let raw = regelrecht_law_model::parse_law_header(yaml).regulatory_layer?;
    let value = raw
        .split(" #")
        .next()
        .unwrap_or_default()
        .trim()
        .trim_matches('"')
        .trim_matches('\'');
    if value.is_empty() {
        return None;
    }
    Some(match RegulatoryLayer::from_yaml_str(value) {
        Some(layer) => LayerField::Known(layer),
        None => LayerField::Unknown(value.to_string()),
    })
}

/// Layers whose regulations apply nationally. Policy rules and implementation
/// policy are left out: a decentral body can issue those too. The match is
/// exhaustive, so a new layer has to be classified here before it compiles.
fn is_national_layer(layer: RegulatoryLayer) -> bool {
    match layer {
        RegulatoryLayer::Verdrag
        | RegulatoryLayer::EuVerordening
        | RegulatoryLayer::EuRichtlijn
        | RegulatoryLayer::Grondwet
        | RegulatoryLayer::Wet
        | RegulatoryLayer::KoninklijkBesluit
        | RegulatoryLayer::Amvb
        | RegulatoryLayer::MinisterieleRegeling => true,
        RegulatoryLayer::Beleidsregel
        | RegulatoryLayer::Uitvoeringsbeleid
        | RegulatoryLayer::GemeentelijkeVerordening
        | RegulatoryLayer::ProvincialeVerordening
        | RegulatoryLayer::WaterschapsVerordening => false,
        // No regulation: a reduction of one cell over its own chronicle.
        RegulatoryLayer::Reductie => false,
    }
}

/// Extract top-level waterschap_code from YAML content using line-based parsing.
///
/// Only matches `waterschap_code:` at the start of a line (no leading whitespace)
/// to avoid matching nested fields, consistent with `extract_gemeente_code`.
fn extract_waterschap_code(yaml: &str) -> Option<String> {
    for line in yaml.lines() {
        if let Some(rest) = line.strip_prefix("waterschap_code:") {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::{LocalSource, SourceType};
    use tempfile::TempDir;

    fn make_scoped_source(
        id: &str,
        path: &std::path::Path,
        scopes: Vec<Scope>,
        priority: u32,
    ) -> Source {
        Source {
            id: id.to_string(),
            name: format!("Source {}", id),
            source_type: SourceType::Local {
                local: LocalSource {
                    path: path.to_path_buf(),
                },
            },
            scopes,
            priority,
            auth_ref: None,
            strict_auth: false,
            enriched: None,
            load_all: false,
        }
    }

    fn write_law(dir: &std::path::Path, name: &str, id: &str, gemeente_code: Option<&str>) {
        let path = dir.join(format!("{}.yaml", name));
        let gc = gemeente_code
            .map(|c| format!("\ngemeente_code: '{}'", c))
            .unwrap_or_default();
        std::fs::write(
            &path,
            format!("$id: {id}\nregulatory_layer: GEMEENTELIJKE_VERORDENING\npublication_date: '2025-01-01'{gc}\narticles: []\n"),
        ).unwrap();
    }

    #[test]
    fn test_extract_gemeente_code() {
        assert_eq!(
            extract_gemeente_code("gemeente_code: GM0363\nfoo: bar"),
            Some("GM0363".to_string())
        );
        assert_eq!(
            extract_gemeente_code("gemeente_code: 'GM0518'\nfoo: bar"),
            Some("GM0518".to_string())
        );
        assert_eq!(extract_gemeente_code("foo: bar\nbaz: qux"), None);
    }

    #[test]
    fn test_extract_waterschap_code() {
        assert_eq!(
            extract_waterschap_code("waterschap_code: WS0653\nfoo: bar"),
            Some("WS0653".to_string())
        );
        assert_eq!(
            extract_waterschap_code("waterschap_code: 'WS0653'\nfoo: bar"),
            Some("WS0653".to_string())
        );
        assert_eq!(extract_waterschap_code("foo: bar\nbaz: qux"), None);
    }

    #[test]
    fn test_scope_valid_no_warnings() {
        let dir = TempDir::new().unwrap();
        write_law(dir.path(), "verordening", "test_v", Some("GM0363"));

        let source = make_scoped_source(
            "amsterdam",
            dir.path(),
            vec![Scope {
                scope_type: "gemeente_code".to_string(),
                value: "GM0363".to_string(),
            }],
            10,
        );

        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();

        let warnings = validate_scopes(&map, &[source]);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_scope_violation_warning() {
        let dir = TempDir::new().unwrap();
        // Source declares GM0363, but law has GM0518
        write_law(dir.path(), "verordening", "wrong_v", Some("GM0518"));

        let source = make_scoped_source(
            "amsterdam",
            dir.path(),
            vec![Scope {
                scope_type: "gemeente_code".to_string(),
                value: "GM0363".to_string(),
            }],
            10,
        );

        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();

        let warnings = validate_scopes(&map, &[source]);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].law_id, "wrong_v");
        assert_eq!(warnings[0].actual_scope_code, Some("GM0518".to_string()));
    }

    #[test]
    fn test_unrestricted_scope_no_warnings() {
        let dir = TempDir::new().unwrap();
        write_law(dir.path(), "verordening", "any_v", Some("GM0518"));

        // Empty scopes = unrestricted
        let source = make_scoped_source("central", dir.path(), vec![], 1);

        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();

        let warnings = validate_scopes(&map, &[source]);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_law_without_gemeente_code_no_warning() {
        let dir = TempDir::new().unwrap();
        write_law(dir.path(), "wet", "national_wet", None);

        let source = make_scoped_source(
            "amsterdam",
            dir.path(),
            vec![Scope {
                scope_type: "gemeente_code".to_string(),
                value: "GM0363".to_string(),
            }],
            10,
        );

        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();

        let warnings = validate_scopes(&map, &[source]);
        assert!(warnings.is_empty());
    }

    fn write_national_law(dir: &std::path::Path, name: &str, id: &str) {
        let path = dir.join(format!("{}.yaml", name));
        std::fs::write(
            &path,
            format!(
                "$id: {id}\nregulatory_layer: WET\npublication_date: '2025-01-01'\narticles: []\n"
            ),
        )
        .unwrap();
    }

    #[test]
    fn test_scoped_source_with_national_law_warns() {
        let dir = TempDir::new().unwrap();
        write_national_law(dir.path(), "wet", "participatiewet");

        let source = make_scoped_source(
            "amsterdam",
            dir.path(),
            vec![Scope {
                scope_type: "gemeente_code".to_string(),
                value: "GM0363".to_string(),
            }],
            0,
        );

        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();

        let warnings = validate_scopes(&map, &[source]);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].law_id, "participatiewet");
        assert!(warnings[0].message.contains("national regulation (WET)"));
    }

    #[test]
    fn test_unrestricted_source_with_national_law_no_warning() {
        let dir = TempDir::new().unwrap();
        write_national_law(dir.path(), "wet", "participatiewet");

        let source = make_scoped_source("central", dir.path(), vec![], 1);

        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();

        let warnings = validate_scopes(&map, &[source]);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_is_national_layer() {
        for layer in RegulatoryLayer::ALL {
            let expected = matches!(
                layer.as_str(),
                "VERDRAG"
                    | "EU_VERORDENING"
                    | "EU_RICHTLIJN"
                    | "GRONDWET"
                    | "WET"
                    | "KONINKLIJK_BESLUIT"
                    | "AMVB"
                    | "MINISTERIELE_REGELING"
            );
            assert_eq!(is_national_layer(*layer), expected, "{}", layer.as_str());
        }
    }

    #[test]
    fn test_extract_regulatory_layer() {
        assert_eq!(
            extract_regulatory_layer("regulatory_layer: WET # formele wet\n"),
            Some(LayerField::Known(RegulatoryLayer::Wet))
        );
        assert_eq!(
            extract_regulatory_layer("regulatory_layer: 'AMVB'\n"),
            Some(LayerField::Known(RegulatoryLayer::Amvb))
        );
        assert_eq!(
            extract_regulatory_layer("regulatory_layer: WETT\n"),
            Some(LayerField::Unknown("WETT".to_string()))
        );
        assert_eq!(extract_regulatory_layer("foo: bar\n"), None);
    }

    #[test]
    fn test_scoped_source_with_unknown_layer_warns() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("x.yaml"),
            "$id: onbekend\nregulatory_layer: WETT\npublication_date: '2025-01-01'\narticles: []\n",
        )
        .unwrap();
        let source = make_scoped_source(
            "amsterdam",
            dir.path(),
            vec![Scope {
                scope_type: "gemeente_code".to_string(),
                value: "GM0363".to_string(),
            }],
            10,
        );
        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();

        let warnings = validate_scopes(&map, &[source]);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0]
            .message
            .contains("unknown regulatory_layer 'WETT'"));
    }

    /// A scoped source that loses on priority still supplies a version for a
    /// date the central source lacks, and the engine loads that version.
    #[test]
    fn test_losing_scoped_source_with_own_national_version_warns() {
        let central_dir = TempDir::new().unwrap();
        let local_dir = TempDir::new().unwrap();
        for (dir, date) in [(&central_dir, "2022-03-15"), (&local_dir, "2030-01-01")] {
            let law_dir = dir.path().join("wet").join("participatiewet");
            std::fs::create_dir_all(&law_dir).unwrap();
            std::fs::write(
                law_dir.join(format!("{date}.yaml")),
                format!(
                    "$id: participatiewet\nregulatory_layer: WET\nvalid_from: '{date}'\narticles: []\n"
                ),
            )
            .unwrap();
        }
        let central = make_scoped_source("central", central_dir.path(), vec![], 1);
        let amsterdam = make_scoped_source(
            "amsterdam",
            local_dir.path(),
            vec![Scope {
                scope_type: "gemeente_code".to_string(),
                value: "GM0363".to_string(),
            }],
            2,
        );
        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&central).unwrap();
        map.load_source(&amsterdam).unwrap();
        assert_eq!(map.get_law("participatiewet").unwrap().source_id, "central");

        let warnings = validate_scopes(&map, &[central, amsterdam]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(warnings[0].source_id, "amsterdam");
        assert!(warnings[0].message.contains("national regulation (WET)"));
    }

    /// Several versions of one out-of-scope regulation give one warning.
    #[test]
    fn test_warnings_deduplicated_across_versions() {
        let dir = TempDir::new().unwrap();
        let law_dir = dir.path().join("wet").join("participatiewet");
        std::fs::create_dir_all(&law_dir).unwrap();
        for date in ["2022-03-15", "2024-01-01"] {
            std::fs::write(
                law_dir.join(format!("{date}.yaml")),
                format!(
                    "$id: participatiewet\nregulatory_layer: WET\nvalid_from: '{date}'\narticles: []\n"
                ),
            )
            .unwrap();
        }
        let source = make_scoped_source(
            "amsterdam",
            dir.path(),
            vec![Scope {
                scope_type: "gemeente_code".to_string(),
                value: "GM0363".to_string(),
            }],
            2,
        );
        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();

        assert_eq!(validate_scopes(&map, &[source]).len(), 1);
    }

    #[test]
    fn test_waterschap_scope_valid_no_warnings() {
        let dir = TempDir::new().unwrap();
        write_waterschap_law(dir.path(), "keur", "keur_valid", "WS0653");

        let source = make_scoped_source(
            "waterschap_hhnk",
            dir.path(),
            vec![Scope {
                scope_type: "waterschap_code".to_string(),
                value: "WS0653".to_string(),
            }],
            10,
        );

        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();

        let warnings = validate_scopes(&map, &[source]);
        assert!(warnings.is_empty());
    }

    fn write_waterschap_law(dir: &std::path::Path, name: &str, id: &str, code: &str) {
        let path = dir.join(format!("{}.yaml", name));
        std::fs::write(
            &path,
            format!("$id: {id}\nregulatory_layer: WATERSCHAPS_VERORDENING\npublication_date: '2025-01-01'\nwaterschap_code: '{code}'\narticles: []\n"),
        ).unwrap();
    }

    #[test]
    fn test_waterschap_scope_violation_warning() {
        let dir = TempDir::new().unwrap();
        // Source declares WS0653, but law has WS0999
        write_waterschap_law(dir.path(), "keur", "keur_test", "WS0999");

        let source = make_scoped_source(
            "waterschap_test",
            dir.path(),
            vec![Scope {
                scope_type: "waterschap_code".to_string(),
                value: "WS0653".to_string(),
            }],
            10,
        );

        let mut map = SourceMap::new("2026-06-01");
        map.load_source(&source).unwrap();

        let warnings = validate_scopes(&map, &[source]);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].law_id, "keur_test");
        assert_eq!(warnings[0].actual_scope_code, Some("WS0999".to_string()));
    }
}
