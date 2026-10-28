//! Search over law ids and names for `GET /api/corpus/laws?q=`.
//!
//! A query matches a law when every word in it occurs in the law's id (read
//! with spaces for underscores) or in its name, in any order, ignoring case
//! and accents. Matches are ranked, so a broad term such as "wet" brings the
//! laws that are called that to the top instead of an alphabetical slice:
//!
//! 0. the whole text is the query ("participatiewet");
//! 1. the text starts with the query, as characters, so a word still being
//!    typed counts ("wet op de zorgtoeslag" and "wetboek van strafrecht" for
//!    "wet");
//! 2. every word sits at the start or the end of a word in the text
//!    ("algemene wet bestuursrecht", or "zorgverzekeringswet", since a Dutch
//!    compound ends in its head);
//! 3. any other match ("aanbestedingswet 2012" for "besteding").

use unicode_normalization::UnicodeNormalization;

/// The longest query looked at, in characters.
pub(crate) const MAX_QUERY_CHARS: usize = 200;
/// The most distinct words a law is checked for.
pub(crate) const MAX_WORDS: usize = 10;

/// How well a law matches; lower is better. See the module docs.
pub(crate) type Rank = u8;

/// A parsed search query.
pub(crate) struct LawQuery {
    /// The query folded like the texts it is compared to, words joined by a
    /// single space.
    phrase: String,
    words: Vec<String>,
}

impl LawQuery {
    /// `None` for a query without any word to search on.
    ///
    /// The endpoint is public and every word is checked against every law, so
    /// a query is cut to [`MAX_QUERY_CHARS`] and its distinct words to
    /// [`MAX_WORDS`]; no law title needs more to be found.
    pub(crate) fn parse(q: &str) -> Option<Self> {
        let cut: String = q.chars().take(MAX_QUERY_CHARS).collect();
        let phrase = fold(&cut);
        if phrase.is_empty() {
            return None;
        }
        let mut words: Vec<String> = Vec::new();
        for word in phrase.split(' ') {
            if words.len() == MAX_WORDS {
                break;
            }
            if !words.iter().any(|w| w == word) {
                words.push(word.to_string());
            }
        }
        Some(Self { phrase, words })
    }

    /// The rank of a law with this id and name, or `None` when it does not
    /// match. The better of the two texts counts.
    pub(crate) fn rank(&self, law_id: &str, name: Option<&str>) -> Option<Rank> {
        let id_text = fold(law_id);
        let name_text = name.map(fold);
        // Every word must occur, but each may be found in either text.
        let all_words = self.words.iter().all(|w| {
            id_text.contains(w.as_str())
                || name_text.as_deref().is_some_and(|n| n.contains(w.as_str()))
        });
        if !all_words {
            return None;
        }
        let id_rank = self.rank_text(&id_text);
        let name_rank = name_text.as_deref().map_or(3, |n| self.rank_text(n));
        Some(id_rank.min(name_rank))
    }

    fn rank_text(&self, text: &str) -> Rank {
        if text == self.phrase {
            0
        } else if text.starts_with(&self.phrase) {
            1
        } else if self.words.iter().all(|w| at_word_edge(text, w)) {
            2
        } else {
            3
        }
    }
}

/// Lowercase, accents dropped (é → e), and every run of anything that is not
/// a letter or digit (underscores included) turned into one space, trimmed.
/// Ids are written that way already; this brings names and queries in line.
fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut gap = false;
    for c in s
        .nfkd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
    {
        if c.is_alphanumeric() {
            if gap && !out.is_empty() {
                out.push(' ');
            }
            gap = false;
            out.extend(c.to_lowercase());
        } else {
            gap = true;
        }
    }
    out
}

/// Whether `word` occurs in `text` at the start or the end of one of its words.
fn at_word_edge(text: &str, word: &str) -> bool {
    text.split(' ')
        .any(|t| t.starts_with(word) || t.ends_with(word))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rank(q: &str, law_id: &str, name: Option<&str>) -> Option<Rank> {
        LawQuery::parse(q).unwrap().rank(law_id, name)
    }

    #[test]
    fn an_empty_query_searches_nothing() {
        assert!(LawQuery::parse("  -_ ").is_none());
    }

    #[test]
    fn every_word_must_occur_in_any_order() {
        assert!(rank("zorgtoeslag wet", "wet_op_de_zorgtoeslag", None).is_some());
        assert!(rank("wet huurtoeslag", "wet_op_de_zorgtoeslag", None).is_none());
    }

    #[test]
    fn a_word_may_match_the_id_or_the_name() {
        assert!(rank(
            "awir inkomensafhankelijke",
            "awir",
            Some("Algemene wet inkomensafhankelijke regelingen")
        )
        .is_some());
    }

    #[test]
    fn case_and_accents_do_not_matter() {
        assert!(rank("Beëindiging", "wet_beeindiging_adoptie", None).is_some());
        assert!(rank("beeindiging", "x", Some("Wet beëindiging adoptie")).is_some());
    }

    #[test]
    fn ranks_exact_then_prefix_then_word_edge_then_anywhere() {
        assert_eq!(rank("participatiewet", "participatiewet", None), Some(0));
        assert_eq!(rank("wet", "wet_op_de_zorgtoeslag", None), Some(1));
        assert_eq!(rank("wet", "algemene_wet_bestuursrecht", None), Some(2));
        assert_eq!(rank("wet", "zorgverzekeringswet", None), Some(2));
        assert_eq!(rank("besteding", "aanbestedingswet_2012", None), Some(3));
    }

    #[test]
    fn digits_are_words_too() {
        assert_eq!(rank("2001", "wet_inkomstenbelasting_2001", None), Some(2));
        assert!(rank("2012", "wet_inkomstenbelasting_2001", None).is_none());
    }

    #[test]
    fn repeated_and_surplus_words_are_dropped() {
        let q = LawQuery::parse("wet wet wet").unwrap();
        assert_eq!(q.words, vec!["wet"]);
        let many: String = (0..50).map(|i| format!("w{i} ")).collect();
        assert_eq!(LawQuery::parse(&many).unwrap().words.len(), MAX_WORDS);
        let long = "a".repeat(10 * MAX_QUERY_CHARS);
        assert_eq!(
            LawQuery::parse(&long).unwrap().phrase.len(),
            MAX_QUERY_CHARS
        );
    }

    #[test]
    fn the_better_of_id_and_name_counts() {
        assert_eq!(
            rank("zorgtoeslag", "wet_op_de_zorgtoeslag", Some("Zorgtoeslag")),
            Some(0)
        );
    }
}
