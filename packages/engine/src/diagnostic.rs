//! What is wrong with a law, as a record instead of a sentence.
//!
//! A law that will not load used to reach its caller as one string. An editor
//! can show that string, but it cannot open the article the problem is in or
//! mark the output it is about. A [`Diagnostic`] says the same thing in parts:
//! which law, which article, which place in it, which rule, and the message.
//!
//! The records are factual. They carry the engine's own message and no
//! wording for an end user; whoever shows them decides how to say it.
//!
//! [`EngineError::diagnostics`](crate::error::EngineError::diagnostics) turns
//! a load error into its records. The type check's findings come out one
//! record each. An error the engine still builds as a sentence comes out as
//! one record that holds that sentence and no place.

use serde::Serialize;
use std::fmt;

/// Whether the problem stops the law from loading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// The law is refused.
    Error,
    /// The law loads; something in it cannot work as written.
    Warning,
}

/// Which check found the problem.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticKind {
    /// The text could not be read as a law: invalid YAML, or YAML that does
    /// not have the shape of the law format.
    Structure,
    /// A rule the loader holds a law to before the type check.
    LoadCheck,
    /// A finding of the type check on the law itself (RFC-036, RFC-037).
    TypeCheck,
    /// A finding of the type check on another loaded law that reads this
    /// one: loading this law would leave that one ill-typed.
    CrossLaw,
}

/// The place within an article that a problem is about.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Place {
    /// The article as a whole.
    Article,
    /// The action that assigns this output.
    Output { name: String },
    /// An action without an output, by its position in the article (from 1).
    Action { index: usize },
    /// The declaration of this input.
    Input { name: String },
    /// The declaration of this definition.
    Definition { name: String },
}

impl fmt::Display for Place {
    /// The place as the type check has always printed it: `output 'x'`,
    /// `action 3`, `input 'x'`, `definition 'X'`, and nothing for the article
    /// as a whole.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Place::Article => Ok(()),
            Place::Output { name } => write!(f, "output '{name}'"),
            Place::Action { index } => write!(f, "action {index}"),
            Place::Input { name } => write!(f, "input '{name}'"),
            Place::Definition { name } => write!(f, "definition '{name}'"),
        }
    }
}

/// A position in the text of the law, counted from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SourcePosition {
    pub line: usize,
    pub column: usize,
}

/// One problem with a law.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub kind: DiagnosticKind,
    /// The law the problem is in, when the check knows it. For
    /// [`DiagnosticKind::CrossLaw`] this is the other law, not the one being
    /// loaded.
    pub law_id: Option<String>,
    /// The number of the article the problem is in.
    pub article: Option<String>,
    /// The place within that article.
    pub place: Option<Place>,
    /// Where in the text the problem is, for a text that could not be read.
    pub position: Option<SourcePosition>,
    /// The code of the rule that is broken, such as `N1`.
    pub rule: Option<String>,
    /// What is wrong, in the engine's words.
    pub message: String,
}

impl Diagnostic {
    /// An error the check reports as a sentence only, with no law, article
    /// or place to go with it.
    pub fn unplaced(kind: DiagnosticKind, message: String) -> Self {
        Self {
            severity: Severity::Error,
            kind,
            law_id: None,
            article: None,
            place: None,
            position: None,
            rule: None,
            message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::EngineError;
    use crate::resolver::RuleResolver;

    /// A law whose only fault is a type error in one output.
    const ILL_TYPED: &str = r#"
$id: t
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: 'B 1'
    text: tekst
    machine_readable:
      execution:
        parameters:
          - name: huur
            type: number
        output:
          - name: geen_huur
            type: boolean
        actions:
          - output: geen_huur
            value:
              operation: EQUALS
              subject: $huur
              value: null
"#;

    #[test]
    fn a_type_check_finding_keeps_its_article_place_and_rule() {
        let err = RuleResolver::new().load_from_yaml(ILL_TYPED).unwrap_err();
        let diagnostics = err.diagnostics();
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.kind, DiagnosticKind::TypeCheck);
        assert_eq!(diagnostic.law_id.as_deref(), Some("t"));
        assert_eq!(diagnostic.article.as_deref(), Some("B 1"));
        assert_eq!(
            diagnostic.place,
            Some(Place::Output {
                name: "geen_huur".to_string()
            })
        );
        assert_eq!(diagnostic.rule.as_deref(), Some("N1"));
        // The sentence stays what it was, and holds the finding's message.
        let sentence = err.to_string();
        assert!(
            sentence.starts_with("Failed to load law: law 't' fails the type check (1 finding):"),
            "{sentence}"
        );
        assert!(sentence.contains(&diagnostic.message), "{sentence}");
    }

    #[test]
    fn a_text_that_cannot_be_read_comes_with_its_position() {
        let err = RuleResolver::new()
            .load_from_yaml("$id: t\narticles: [unclosed")
            .unwrap_err();
        assert!(matches!(err, EngineError::YamlError(_)), "{err:?}");
        let diagnostics = err.diagnostics();
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(diagnostics[0].kind, DiagnosticKind::Structure);
        assert!(diagnostics[0].position.is_some(), "{diagnostics:?}");
        assert_eq!(diagnostics[0].article, None);
    }

    #[test]
    fn an_error_built_as_a_sentence_is_one_record_without_a_place() {
        let err = EngineError::LoadError("law 't': article 2 does something".to_string());
        let diagnostics = err.diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, DiagnosticKind::LoadCheck);
        assert_eq!(diagnostics[0].message, "law 't': article 2 does something");
        assert_eq!(diagnostics[0].place, None);
    }

    #[test]
    fn a_record_serializes_to_plain_fields() {
        let err = RuleResolver::new().load_from_yaml(ILL_TYPED).unwrap_err();
        let json = serde_json::to_value(err.diagnostics()).unwrap();
        assert_eq!(json[0]["severity"], "error");
        assert_eq!(json[0]["kind"], "type_check");
        assert_eq!(json[0]["article"], "B 1");
        assert_eq!(json[0]["place"]["kind"], "output");
        assert_eq!(json[0]["place"]["name"], "geen_huur");
        assert_eq!(json[0]["rule"], "N1");
    }
}
