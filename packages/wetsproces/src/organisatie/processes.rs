//! The processes of an organisation, as the file `WETSPROCES_PROCESSEN`
//! lists them. The Fundament plugin writes it into the values of the
//! front's release, and the chart mounts it from a ConfigMap:
//!
//! ```yaml
//! processen:
//!   - id: tnt-voorbeeld--parkeerkaart/parkeerkaart
//!     naam: parkeerkaart
//!     burger:
//!       intern: http://parkeerkaart-burger.tnt-voorbeeld--parkeerkaart.svc.cluster.local:8000
//!       publiek: https://parkeerkaart.cluster-a.example.org
//!     backoffice:
//!       intern: http://parkeerkaart-backoffice.tnt-voorbeeld--parkeerkaart.svc.cluster.local:8000
//!       publiek: https://parkeerkaart-medewerkers.cluster-a.example.org
//! ```
//!
//! `intern` is where the front calls the process, `publiek` where it sends a
//! browser. An id appears once; an empty list is a front with nothing behind
//! it yet, which starts and shows so.

use std::path::Path;

use serde::Deserialize;

use crate::surface::Surface;

/// The side of a process a statement is for: the `aud` of a statement is
/// `<process id>#<side>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Citizen,
    Backoffice,
}

impl Side {
    /// Its name in an `aud`.
    pub fn name(self) -> &'static str {
        self.surface().name()
    }

    /// The surface of a process instance that serves only this side.
    pub fn surface(self) -> Surface {
        match self {
            Self::Citizen => Surface::Citizen,
            Self::Backoffice => Surface::Backoffice,
        }
    }

    /// The side `name` names in an `aud`.
    pub fn from_name(name: &str) -> Option<Self> {
        [Self::Citizen, Self::Backoffice]
            .into_iter()
            .find(|s| s.name() == name)
    }

    /// Whether a server on `surface` serves this side.
    pub fn served_on(self, surface: Surface) -> bool {
        match self {
            Self::Citizen => surface.serves_citizens(),
            Self::Backoffice => surface.serves_employees(),
        }
    }
}

/// The two addresses of one side of a process.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Addresses {
    /// Where the front calls it, inside the cluster.
    pub intern: String,
    /// Where a browser reaches it.
    pub publiek: String,
}

/// A process of the organisation.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Process {
    /// `<namespace>/<name>` of its object: the `aud` of a statement for it.
    pub id: String,
    /// Its name, until its case gives a title.
    pub naam: String,
    pub burger: Addresses,
    pub backoffice: Addresses,
}

impl Process {
    /// The addresses of `side`.
    pub fn side(&self, side: Side) -> &Addresses {
        match side {
            Side::Citizen => &self.burger,
            Side::Backoffice => &self.backoffice,
        }
    }

    /// The `aud` of a statement for `side` of this process.
    pub fn audience(&self, side: Side) -> String {
        audience(&self.id, side)
    }
}

/// The `aud` of a statement for `side` of the process `id`.
pub fn audience(id: &str, side: Side) -> String {
    format!("{id}#{}", side.name())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    processen: Vec<Process>,
}

/// Why the file gives no processes.
#[derive(Debug, thiserror::Error)]
pub enum ProcessesError {
    #[error("reading {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("{path} is no list of processes: {source}")]
    Parse {
        path: String,
        source: serde_yaml_ng::Error,
    },
    #[error("the process `{0}` is listed twice")]
    Duplicate(String),
    #[error("the process `{id}` has an address that is no http(s) URL: `{address}`")]
    BadAddress { id: String, address: String },
}

/// The processes in the file at `path`, in its order, with the trailing
/// slashes of their addresses left out.
///
/// # Errors
///
/// When the file cannot be read or parsed, names a process twice, or holds
/// an address that is no `http(s)` URL.
pub fn load(path: &Path) -> Result<Vec<Process>, ProcessesError> {
    let text = std::fs::read_to_string(path).map_err(|source| ProcessesError::Read {
        path: path.display().to_string(),
        source,
    })?;
    parse(&text).map_err(|e| match e {
        ProcessesError::Parse { source, .. } => ProcessesError::Parse {
            path: path.display().to_string(),
            source,
        },
        other => other,
    })
}

fn parse(text: &str) -> Result<Vec<Process>, ProcessesError> {
    let file: File = serde_yaml_ng::from_str(text).map_err(|source| ProcessesError::Parse {
        path: String::new(),
        source,
    })?;
    let mut processes = file.processen;
    let mut seen = std::collections::BTreeSet::new();
    for process in &mut processes {
        if !seen.insert(process.id.clone()) {
            return Err(ProcessesError::Duplicate(process.id.clone()));
        }
        let id = process.id.clone();
        for address in [
            &mut process.burger.intern,
            &mut process.burger.publiek,
            &mut process.backoffice.intern,
            &mut process.backoffice.publiek,
        ] {
            let trimmed = address.trim().trim_end_matches('/');
            if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
                return Err(ProcessesError::BadAddress {
                    id,
                    address: address.clone(),
                });
            }
            *address = trimmed.to_string();
        }
    }
    Ok(processes)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const TWO: &str = "
processen:
  - id: tnt-a/parkeerkaart
    naam: parkeerkaart
    burger: { intern: 'http://parkeerkaart-burger:8000/', publiek: 'https://parkeerkaart.example.org' }
    backoffice: { intern: 'http://parkeerkaart-backoffice:8000', publiek: 'https://parkeerkaart-medewerkers.example.org' }
  - id: tnt-b/bedrijfsparkeren
    naam: bedrijfsparkeren
    burger: { intern: 'http://b:8000', publiek: 'https://b.example.org' }
    backoffice: { intern: 'http://b-bo:8000', publiek: 'https://b-medewerkers.example.org' }
";

    #[test]
    fn the_file_lists_the_processes_in_its_order() {
        let processes = parse(TWO).unwrap();
        assert_eq!(processes.len(), 2);
        assert_eq!(processes[0].id, "tnt-a/parkeerkaart");
        assert_eq!(
            processes[0].burger.intern,
            "http://parkeerkaart-burger:8000"
        );
        assert_eq!(
            processes[0].audience(Side::Backoffice),
            "tnt-a/parkeerkaart#backoffice"
        );
        assert!(parse("processen: []").unwrap().is_empty());
        assert!(parse("{}").unwrap().is_empty());
    }

    #[test]
    fn a_process_twice_or_an_address_without_a_scheme_is_refused() {
        let twice = format!("{TWO}{}", &TWO["\nprocessen:\n".len()..]);
        assert!(matches!(parse(&twice), Err(ProcessesError::Duplicate(_))));
        let bad = TWO.replace("http://b:8000", "b:8000");
        assert!(matches!(
            parse(&bad),
            Err(ProcessesError::BadAddress { .. })
        ));
        assert!(matches!(
            parse("processen: [{ id: x }]"),
            Err(ProcessesError::Parse { .. })
        ));
    }

    #[test]
    fn a_side_is_served_where_its_surface_is() {
        assert!(Side::Citizen.served_on(Surface::All));
        assert!(!Side::Citizen.served_on(Surface::Backoffice));
        assert_eq!(Side::from_name("backoffice"), Some(Side::Backoffice));
        assert_eq!(Side::from_name("all"), None);
    }
}
