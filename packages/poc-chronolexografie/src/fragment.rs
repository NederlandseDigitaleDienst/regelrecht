//! Parts of a YAML file as written there, for the tab Keten: the
//! `machine_readable` of an article, and the few blocks of it that make one
//! link of the chain (an action, an input with its source, `implements`, an
//! open term, a line of a cell's catalogue).
//!
//! The cuts work on the text, not on parsed YAML, so a fragment keeps the
//! keys in the order and the notation of the file. Full-line comments and
//! blank lines are left out of a fragment, and each block loses its own
//! indentation; every other line is a line of the file.

/// The longest block of a fragment, in lines. A longer one ends with
/// [`AFGEKAPT`], and the full article is one click away.
pub const MAX_REGELS: usize = 30;

/// The last line of a fragment that was cut at [`MAX_REGELS`].
pub const AFGEKAPT: &str = "# … (zie het volledige artikel)";

fn inspring(regel: &str) -> usize {
    regel.len() - regel.trim_start().len()
}

/// A line that holds no YAML: blank, or only a comment.
fn leeg(regel: &str) -> bool {
    let t = regel.trim_start();
    t.is_empty() || t.starts_with('#')
}

/// A key that opens a block (`input:`), as opposed to a list item.
fn opent_blok(regel: &str) -> bool {
    let t = regel.trim();
    t.ends_with(':') && !t.starts_with("- ")
}

/// The end (exclusive) of the block that starts at line `i`: the lines
/// indented deeper, and for a key the list items at its own indentation,
/// which YAML allows.
fn einde(regels: &[&str], i: usize) -> usize {
    let eigen = inspring(regels[i]);
    let sleutel = opent_blok(regels[i]);
    let mut laatste = i;
    for (j, regel) in regels.iter().enumerate().skip(i + 1) {
        if leeg(regel) {
            continue;
        }
        let k = inspring(regel);
        if k > eigen || (sleutel && k == eigen && regel.trim_start().starts_with("- ")) {
            laatste = j;
        } else {
            break;
        }
    }
    laatste + 1
}

/// The key of the block that holds line `i`: the nearest line above it
/// that opens a block and is indented less, or as much for a list item.
fn ouder<'a>(regels: &[&'a str], i: usize) -> Option<&'a str> {
    let eigen = inspring(regels[i]);
    let item = regels[i].trim_start().starts_with("- ");
    regels[..i]
        .iter()
        .rev()
        .filter(|r| !leeg(r))
        .find(|r| inspring(r) < eigen || (item && inspring(r) == eigen && opent_blok(r)))
        .map(|r| r.trim().trim_start_matches("- ").trim_end_matches(':'))
}

/// Lines `van..tot` without comments and blank lines, less the indentation
/// of the first.
fn snede(regels: &[&str], van: usize, tot: usize) -> Vec<String> {
    let eigen = inspring(regels[van]);
    regels[van..tot]
        .iter()
        .filter(|r| !leeg(r))
        .map(|r| {
            r.get(eigen..)
                .unwrap_or(r.trim_start())
                .trim_end()
                .to_string()
        })
        .collect()
}

/// The value of `key: value` on a line, without quotes.
fn waarde<'a>(regel: &'a str, key: &str) -> Option<&'a str> {
    let rest = regel.trim().strip_prefix(key)?.strip_prefix(':')?;
    Some(rest.trim().trim_matches(|c| c == '\'' || c == '"'))
}

/// The lines of the article `number` in `yaml`: from its `- number:` line
/// to the end of the item.
fn artikel(regels: &[&str], number: &str) -> Option<(usize, usize)> {
    let begin = regels
        .iter()
        .position(|r| item(r, "number") == Some(number))?;
    Some((begin, einde(regels, begin)))
}

/// The `machine_readable` block of the article `number` in `yaml`, as
/// written there (comments included), without its indentation.
pub fn machine_readable(yaml: &str, number: &str) -> Option<String> {
    let regels: Vec<&str> = yaml.lines().collect();
    let (begin, eind) = artikel(&regels, number)?;
    let mr = (begin + 1..eind).find(|&i| regels[i].trim() == "machine_readable:")?;
    let eigen = inspring(regels[mr]);
    let tot = einde(&regels, mr);
    Some(
        regels[mr..tot]
            .iter()
            .map(|r| r.get(eigen..).unwrap_or("").trim_end())
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// What a fragment of an article holds: its `implements`, the inputs named
/// in `invoer` (with their source), the open term `begrip` it declares, and
/// the actions that set `uitvoer`.
#[derive(Debug, Default)]
pub struct Keuze<'a> {
    pub implements: bool,
    pub invoer: &'a [String],
    pub begrip: Option<&'a str>,
    pub uitvoer: &'a [String],
}

/// The blocks of the article `number` in `yaml` that `keuze` names, in the
/// order of [`Keuze`], each as written (an open term without its
/// `description`) and each one panel.
pub fn van_artikel(yaml: &str, number: &str, keuze: &Keuze<'_>) -> Vec<String> {
    let regels: Vec<&str> = yaml.lines().collect();
    let Some((begin, eind)) = artikel(&regels, number) else {
        return Vec::new();
    };
    let binnen = begin + 1..eind;
    let mut blokken: Vec<Vec<String>> = Vec::new();
    let blok = |i: usize| snede(&regels, i, einde(&regels, i));
    if keuze.implements {
        if let Some(i) = binnen.clone().find(|&i| regels[i].trim() == "implements:") {
            blokken.push(blok(i));
        }
    }
    for naam in keuze.invoer {
        if let Some(i) = binnen.clone().find(|&i| {
            item(regels[i], "name") == Some(naam.as_str()) && ouder(&regels, i) == Some("input")
        }) {
            blokken.push(blok(i));
        }
    }
    if let Some(id) = keuze.begrip {
        if let Some(i) = binnen
            .clone()
            .find(|&i| item(regels[i], "id") == Some(id) && ouder(&regels, i) == Some("open_terms"))
        {
            blokken.push(zonder_beschrijving(blok(i)));
        }
    }
    for output in keuze.uitvoer {
        if let Some(i) = binnen.clone().find(|&i| {
            item(regels[i], "output") == Some(output.as_str())
                && ouder(&regels, i) == Some("actions")
        }) {
            blokken.push(blok(i));
        }
    }
    blokken
        .into_iter()
        .map(|mut blok| {
            if blok.len() > MAX_REGELS {
                blok.truncate(MAX_REGELS - 1);
                blok.push(AFGEKAPT.to_string());
            }
            blok.join("\n")
        })
        .collect()
}

/// `blok` without its `description` and what belongs to it: the text
/// beside the fragment already says it.
fn zonder_beschrijving(mut blok: Vec<String>) -> Vec<String> {
    if let Some(d) = blok
        .iter()
        .position(|r| r.trim_start().starts_with("description:"))
    {
        let diep = inspring(&blok[d]);
        let tot = blok[d + 1..]
            .iter()
            .position(|r| inspring(r) <= diep)
            .map_or(blok.len(), |j| d + 1 + j);
        blok.drain(d..tot);
    }
    blok
}

/// The value of `key` when `regel` is the first line of a list item that
/// starts with it (`- name: x`).
fn item<'a>(regel: &'a str, key: &str) -> Option<&'a str> {
    waarde(regel.trim_start().strip_prefix("- ")?, key)
}

/// The line of `antwoord` under `answers` in the catalogue `cel_yaml`
/// (`cel.yaml`), as written there.
pub fn van_catalogus(cel_yaml: &str, antwoord: &str) -> Option<String> {
    let regels: Vec<&str> = cel_yaml.lines().collect();
    let i = (0..regels.len())
        .find(|&i| waarde(regels[i], antwoord).is_some() && ouder(&regels, i) == Some("answers"))?;
    Some(regels[i].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "\
$id: test
articles:
  - number: '1'
    text: een
    machine_readable:
      # a comment
      open_terms:
        - id: b
          type: boolean
          # why
          description: >-
            Of b.
      implements:
      - law: wet
        article: '2'
        open_term: a
      execution:
        parameters:
          - name: x
            type: string
        input:
          - name: x
            type: number
            source:
              reduction: antwoord_x
              parameters:
                y: $y
        output:
          - name: a
            type: boolean
        actions:
          - output: a
            value:
              operation: NOT
              value: $b

  - number: 2
    text: twee
    machine_readable:
      open_terms:
        - id: c
    url: https://example.org
";

    #[test]
    fn takes_the_block_of_the_article_as_written() {
        let mr = machine_readable(YAML, "2").unwrap();
        assert_eq!(mr, "machine_readable:\n  open_terms:\n    - id: c");
        assert!(machine_readable(YAML, "1").unwrap().contains("# a comment"));
    }

    #[test]
    fn an_article_without_one_has_none() {
        assert_eq!(machine_readable(YAML, "3"), None);
        assert_eq!(
            machine_readable("articles:\n  - number: '4'\n    text: x\n", "4"),
            None
        );
    }

    #[test]
    fn a_fragment_holds_the_blocks_that_make_the_link_without_comments() {
        let invoer = ["x".to_string()];
        let uitvoer = ["a".to_string()];
        let keuze = Keuze {
            implements: true,
            invoer: &invoer,
            begrip: None,
            uitvoer: &uitvoer,
        };
        assert_eq!(
            van_artikel(YAML, "1", &keuze),
            [
                "implements:\n- law: wet\n  article: '2'\n  open_term: a",
                "- name: x\n  type: number\n  source:\n    reduction: antwoord_x\n    parameters:\n      y: $y",
                "- output: a\n  value:\n    operation: NOT\n    value: $b",
            ]
        );
    }

    #[test]
    fn an_open_term_and_not_a_parameter_of_the_same_name() {
        let keuze = Keuze {
            begrip: Some("b"),
            ..Keuze::default()
        };
        assert_eq!(van_artikel(YAML, "1", &keuze), ["- id: b\n  type: boolean"]);
        assert!(van_artikel(YAML, "1", &Keuze::default()).is_empty());
    }

    #[test]
    fn a_long_fragment_ends_with_a_mark() {
        let mut yaml = String::from("articles:\n  - number: '1'\n    machine_readable:\n      execution:\n        actions:\n          - output: a\n            value:\n");
        for i in 0..40 {
            yaml.push_str(&format!("              - v{i}\n"));
        }
        let uitvoer = ["a".to_string()];
        let keuze = Keuze {
            uitvoer: &uitvoer,
            ..Keuze::default()
        };
        let f = van_artikel(&yaml, "1", &keuze).remove(0);
        assert_eq!(f.lines().count(), MAX_REGELS);
        assert_eq!(f.lines().last(), Some(AFGEKAPT));
    }

    #[test]
    fn a_line_of_the_catalogue() {
        let cel = "name: X\n# answers\nanswers:\n  a: reductie_a\n  b: reductie_b\n";
        assert_eq!(van_catalogus(cel, "b").as_deref(), Some("b: reductie_b"));
        assert_eq!(van_catalogus(cel, "name"), None);
    }
}
