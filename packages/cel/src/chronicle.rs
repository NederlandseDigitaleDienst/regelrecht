//! The chronolexogram, and the chronicle a cell appends chronolexograms to.
//!
//! A chronicle is append-only. It lives in memory, or in a JSON Lines file with
//! one chronolexogram per line, written (and synced) before the chronolexogram
//! is added in memory. A chronicle has one writer: two cells open on the same
//! file do not see each other's chronolexograms.

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{refused, setup, Result};
use regelrecht_law_model::stored;

/// A chronolexogram (`Gram` for short): one recorded application or
/// objection, or the decision on one, as the law describes it.
///
/// The chronolexogram does not repeat the law. Its establishing article and the
/// day it was recorded name the law version in force; the provisions each field
/// and output rest on follow from that version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gram {
    /// Its own id, a uuid v7.
    pub id: String,
    /// What the law says arises, in its own word: the kind of the
    /// submission (`produces.submission.kind`), such as `AANVRAAG` or
    /// `BEZWAAR`, or the
    /// legal character of a decision (`produces.legal_character`), such as
    /// `BESCHIKKING`.
    pub kind: String,
    /// The article that establishes it or decides, `<regulation>#<article>`
    /// (RFC-022 `grondslag`).
    pub grondslag: String,
    /// The chronolexogram it refers to: for a decision, the application or
    /// objection it is taken on (RFC-047, RFC-050); for an objection, the
    /// decision it contests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refers_to: Option<String>,
    /// The cell that recorded it (RFC-022 `recording_actor`).
    pub recording_actor: String,
    /// The day the cell recorded it.
    pub recorded_on: NaiveDate,
    /// The moment that counts, when the law names one (`produces.moment`).
    /// It is not part of what the application holds (RFC-045).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moment: Option<GramMoment>,
    /// What the application holds: the values supplied, and `null` for a
    /// field left out that can be absent, as the engine received them.
    pub fields: BTreeMap<String, Value>,
    // Which fields hold a finding of the deciding authority is not stored:
    // a replay of the decision says so ([`crate::Cell::findings`]).
    /// Fields that cannot be absent and that the application left out. An
    /// incomplete application is still recorded (Awb 4:5).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    /// What executing the establishing article gave: its own outputs and
    /// those of the hooks on the submission, such as the last day of the
    /// decision period (Awb 4:13).
    pub outputs: BTreeMap<String, Value>,
}

impl Gram {
    /// The value it holds under `name`: a field, or else an output (what a
    /// hook on a submission gave, RFC-050).
    pub fn held(&self, name: &str) -> Option<&Value> {
        self.fields.get(name).or_else(|| self.outputs.get(name))
    }
}

/// The moment that counts for a chronolexogram, as recorded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GramMoment {
    /// The parameter the law names for it, which the engine received the
    /// moment in. Absent when the law names none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter: Option<String>,
    /// The day it fell on. The cell takes the day it records the
    /// chronolexogram.
    pub on: NaiveDate,
    /// The article that names the moment.
    pub declared_by: String,
}

/// The chronolexograms of one cell, in the order they were recorded.
#[derive(Debug, Default)]
pub struct Chronicle {
    /// The file it is kept in; `None` for a chronicle in memory.
    path: Option<PathBuf>,
    grams: Vec<Gram>,
}

/// Whether the values `gram` holds read back as engine values
/// ([`stored::from_stored`]): a damaged decimal is an error.
fn readable(gram: &Gram) -> Result<()> {
    for value in gram.fields.values().chain(gram.outputs.values()) {
        stored::from_stored(value)
            .map_err(|e| setup(format!("chronolexogram '{}': {e}", gram.id)))?;
    }
    Ok(())
}

impl Chronicle {
    /// An empty chronicle in memory.
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// Open the chronicle in the JSON Lines file at `path`, reading the
    /// chronolexograms already there. A missing file is an empty chronicle. The
    /// chronolexograms in the file must keep the rules of
    /// [`Chronicle::append`].
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => return Err(e.into()),
        };
        let mut chronicle = Self::in_memory();
        for (i, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let gram: Gram = serde_json::from_str(line)
                .map_err(|e| setup(format!("{} line {}: {e}", path.display(), i + 1)))?;
            readable(&gram)
                .map_err(|e| setup(format!("{} line {}: {e}", path.display(), i + 1)))?;
            chronicle.check(&gram)?;
            chronicle.grams.push(gram);
        }
        chronicle.path = Some(path);
        Ok(chronicle)
    }

    pub fn grams(&self) -> &[Gram] {
        &self.grams
    }

    /// The same chronolexograms, in a chronicle in memory: what is appended
    /// to the copy stays out of this chronicle and its file.
    pub(crate) fn in_memory_copy(&self) -> Self {
        Self {
            path: None,
            grams: self.grams.clone(),
        }
    }

    /// Append a chronolexogram: to the file first (if there is one), then to
    /// memory.
    ///
    /// The chronicle has one clock and it only moves forward: a chronolexogram
    /// dated before the last chronolexogram is refused. The same day is fine.
    pub fn append(&mut self, gram: Gram) -> Result<&Gram> {
        self.append_all(vec![gram])?;
        Ok(&self.grams[self.grams.len() - 1])
    }

    /// Append `grams` together: all of them, or none. Such as the decision
    /// on an objection and the new decision that takes the place of the
    /// revoked one (RFC-050), which half written would leave a revocation
    /// without its replacement.
    pub fn append_all(&mut self, grams: Vec<Gram>) -> Result<&[Gram]> {
        let before = self.grams.len();
        for gram in &grams {
            // Checked against the chronicle and the ones before it, and
            // read back as `open` reads it, so no file is written that does
            // not open again.
            if let Err(e) = readable(gram).and_then(|()| self.check(gram)) {
                self.grams.truncate(before);
                return Err(e);
            }
            self.grams.push(gram.clone());
        }
        self.grams.truncate(before);
        if let Some(path) = &self.path {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            let mut line = String::new();
            for gram in &grams {
                line.push_str(&serde_json::to_string(gram).map_err(|e| setup(e.to_string()))?);
                line.push('\n');
            }
            let mut file = OpenOptions::new().create(true).append(true).open(path)?;
            // All or nothing: a write that fails halfway is cut off again, so
            // no half line is left for the next chronolexogram or the next
            // `open`.
            let before = file.metadata()?.len();
            if let Err(e) = file
                .write_all(line.as_bytes())
                .and_then(|()| file.sync_data())
            {
                let _ = file.set_len(before);
                return Err(e.into());
            }
        }
        self.grams.extend(grams);
        Ok(&self.grams[before..])
    }

    fn check(&self, gram: &Gram) -> Result<()> {
        if self.grams.iter().any(|g| g.id == gram.id) {
            return Err(refused(format!(
                "chronolexogram '{}' is already in the chronicle",
                gram.id
            )));
        }
        match self.grams.last() {
            Some(last) if gram.recorded_on < last.recorded_on => Err(refused(format!(
                "chronolexogram '{}' is dated {}, before the last chronolexogram of the chronicle ('{}', {}): \
                 the chronicle does not go back in time",
                gram.id, gram.recorded_on, last.id, last.recorded_on
            ))),
            _ => Ok(()),
        }
    }
}
