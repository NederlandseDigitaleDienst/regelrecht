// Shared grammar model + translation. include!'d by build scripts and the
// codegen binary. No external deps beyond serde/serde_yaml_ng.
//
// NOTE: the workspace pins `serde_yaml_ng` (a maintained fork of the
// unmaintained `serde_yaml`) — its API is identical, so it is used here.
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Grammar {
    pub steps: Vec<Step>,
    #[serde(default)]
    pub value_typing: ValueTyping,
}

/// How a captured value is typed on its way to the engine. Owned by
/// `bdd/grammar.yaml` so the Rust and JS dispatchers cannot drift apart on it.
#[derive(Debug, Default, Deserialize)]
pub struct ValueTyping {
    #[serde(default)]
    pub quoted: String,
    #[serde(default)]
    pub bare: String,
    #[serde(default)]
    pub table_cell: String,
}

#[derive(Debug, Deserialize)]
pub struct Step {
    pub id: String,
    pub action: String,
    pub keyword: String, // given|when|then
    #[serde(default)]
    pub tier: String,
    pub text: String,
    #[serde(default)]
    pub args: Vec<Arg>,
    #[serde(default)]
    pub datatable: bool,
    #[serde(default)]
    pub literals: Vec<serde_yaml_ng::Value>,
}

#[derive(Debug, Deserialize)]
pub struct Arg {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String, // string|number
}

pub fn load_grammar(path: &std::path::Path) -> Grammar {
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read grammar {}: {e}", path.display()));
    serde_yaml_ng::from_str(&raw)
        .unwrap_or_else(|e| panic!("invalid grammar yaml {}: {e}", path.display()))
}

/// A base grammar with the steps of an extension appended, such as the cell's
/// own steps (`packages/cel/bdd/grammar.yaml`) on top of the canonical
/// language. The extension takes the base's value typing and declares none of
/// its own, so a step means the same in both; a step id it repeats is an
/// error. Only the cell's build script calls it.
#[allow(dead_code)]
pub fn load_grammar_extended(base: &std::path::Path, extension: &std::path::Path) -> Grammar {
    let mut grammar = load_grammar(base);
    let extra = load_grammar(extension);
    let typing = &extra.value_typing;
    if !(typing.quoted.is_empty() && typing.bare.is_empty() && typing.table_cell.is_empty()) {
        panic!(
            "grammar {} declares value_typing; an extension takes the base's",
            extension.display()
        );
    }
    for step in extra.steps {
        if grammar.steps.iter().any(|s| s.id == step.id) {
            panic!(
                "grammar {} repeats step id '{}' of {}",
                extension.display(),
                step.id,
                base.display()
            );
        }
        grammar.steps.push(step);
    }
    grammar
}

/// True when the step has at least one numeric arg (forces a regex binding).
pub fn needs_regex(step: &Step) -> bool {
    step.args.iter().any(|a| a.ty == "number")
}

/// Translate canonical `text` into a Cucumber Expression (quoted args -> {string}).
/// Only valid when !needs_regex(step).
pub fn to_cucumber_expr(step: &Step) -> String {
    // Replace each "{name}" (quoted) with {string}; bare {name} not allowed here.
    let mut out = step.text.clone();
    for a in &step.args {
        let quoted = format!("\"{{{}}}\"", a.name); // "{name}"
        out = out.replace(&quoted, "{string}");
    }
    out
}

/// Translate canonical `text` into an anchored regex with one capture per arg.
///
/// Quoted string arg `"{name}"` -> `"([^"]*)"`, numeric `{name}` ->
/// `(-?\d+(?:\.\d+)?)`. Literal text (including the quote chars around string
/// args) is regex-escaped; the capture groups themselves are NOT escaped.
///
/// The `\u{0}STR\u{0}` / `\u{0}NUM\u{0}` NUL sentinels mark capture positions
/// before escaping, so `regex_escape` only touches literal text. They are then
/// swapped for the real, unescaped capture groups.
pub fn to_regex(step: &Step) -> String {
    let mut working = step.text.clone();
    for a in &step.args {
        if a.ty == "string" {
            let ph = format!("\"{{{}}}\"", a.name);
            working = working.replacen(&ph, "\u{0}STR\u{0}", 1);
        } else {
            let ph = format!("{{{}}}", a.name);
            working = working.replacen(&ph, "\u{0}NUM\u{0}", 1);
        }
    }
    let body = regex_escape(&working)
        .replace("\u{0}STR\u{0}", "\"([^\"]*)\"")
        .replace("\u{0}NUM\u{0}", "(-?\\d+(?:\\.\\d+)?)");
    format!("^{body}$")
}

fn regex_escape(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        if "\\^$.|?*+()[]{}".contains(c) {
            o.push('\\');
        }
        o.push(c);
    }
    o
}
