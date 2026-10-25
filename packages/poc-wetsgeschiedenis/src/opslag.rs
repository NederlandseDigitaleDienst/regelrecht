//! What has been fetched, next to what is in the image. A timeline is a JSON
//! file named after its id; the list for the dropdown is `dossiers.json`. What
//! a visitor fetched is shared by everyone and capped at `max`: the oldest
//! goes first. A fetched timeline wins over the one in the image (a law
//! fetched again carries the toelichting of its dossiers).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{json, Map, Value};

use crate::{id, Instellingen};

pub struct Opslag {
    vast: PathBuf,
    data: PathBuf,
    max: usize,
    /// Writes to the list go one at a time.
    slot: Mutex<()>,
}

/// A JSON file, or `anders` when it is missing or broken.
fn lees(pad: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(pad).ok()?).ok()
}

/// Can we write here? Creates the directory if it is missing.
pub fn schrijfbaar(map: &Path) -> bool {
    let proef = map.join(".schrijfproef");
    fs::create_dir_all(map).is_ok()
        && fs::write(&proef, b"ok").is_ok()
        && fs::remove_file(&proef).is_ok()
}

impl Opslag {
    pub fn new(inst: &Instellingen) -> Self {
        Self {
            vast: inst.vast.clone(),
            data: inst.data.clone(),
            max: inst.max,
            slot: Mutex::new(()),
        }
    }

    pub fn data(&self) -> &Path {
        &self.data
    }
    pub fn opgehaald(&self) -> PathBuf {
        self.data.join("gegevens")
    }
    pub fn bron(&self) -> PathBuf {
        self.data.join("bron")
    }
    pub fn werk(&self) -> PathBuf {
        self.data.join("werk")
    }
    pub fn tijdelijk(&self) -> PathBuf {
        self.data.join("tijdelijk")
    }
    /// The directories the worker looks in for a timeline, fetched first.
    pub fn mappen(&self) -> String {
        format!("{}:{}", self.opgehaald().display(), self.vast.display())
    }

    /// The dropdown: first what is in the image, then what was fetched.
    pub fn index(&self) -> Vec<Value> {
        let vast = lees(&self.vast.join("dossiers.json"))
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        let ids: Vec<String> = vast
            .iter()
            .filter_map(|d| d["dossier"].as_str().map(String::from))
            .collect();
        let eigen = self.lijst().into_iter().filter(|d| {
            !ids.iter()
                .any(|i| Some(i.as_str()) == d["dossier"].as_str())
        });
        vast.into_iter()
            .chain(eigen.map(|mut d| {
                d["opgehaald"] = true.into();
                d
            }))
            .collect()
    }

    fn lijst(&self) -> Vec<Value> {
        lees(&self.opgehaald().join("dossiers.json"))
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
    }

    /// The timeline `id`, as bytes (it can be a few megabytes; it is not parsed).
    pub fn tijdlijn(&self, id: &str) -> Option<Vec<u8>> {
        if !id::is_id(id) {
            return None;
        }
        [self.opgehaald(), self.vast.clone()]
            .iter()
            .find_map(|m| fs::read(m.join(format!("{id}.json"))).ok())
    }

    pub fn heeft(&self, id: &str) -> bool {
        id::is_id(id)
            && [self.opgehaald(), self.vast.clone()]
                .iter()
                .any(|m| m.join(format!("{id}.json")).is_file())
    }

    /// Put the timeline the worker wrote at `bestand` with the fetched ones;
    /// `bij` is the law the queue fetched this dossier for. Returns its id.
    pub fn bewaar(&self, bestand: &Path, bij: Option<&str>) -> io::Result<String> {
        let t: Value = serde_json::from_slice(&fs::read(bestand)?)?;
        let id = t["dossier"]
            .as_str()
            .filter(|i| id::is_id(i))
            .ok_or_else(|| io::Error::other("tijdlijn zonder geldig id"))?
            .to_string();
        let _slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        let map = self.opgehaald();
        fs::create_dir_all(&map)?;
        fs::rename(bestand, map.join(format!("{id}.json")))?;
        let mut lijst = self.lijst();
        let oud_bij = lijst
            .iter()
            .find(|d| d["dossier"] == id.as_str())
            .and_then(|d| d["bij"].as_str().map(String::from));
        lijst.retain(|d| d["dossier"] != id.as_str());
        let mut rij = Map::new();
        rij.insert("dossier".into(), id.clone().into());
        rij.insert("naam".into(), t["naam"].clone());
        rij.insert("soort".into(), t["soort"].clone());
        if let Some(b) = bij.map(String::from).or(oud_bij) {
            rij.insert("bij".into(), b.into());
        }
        lijst.push(Value::Object(rij));
        let weg = lijst.len().saturating_sub(self.max);
        for d in lijst.drain(..weg) {
            if let Some(i) = d["dossier"].as_str().filter(|i| id::is_id(i)) {
                let _ = fs::remove_file(map.join(format!("{i}.json")));
            }
        }
        fs::write(
            map.join("dossiers.json"),
            serde_json::to_vec_pretty(&lijst)?,
        )?;
        Ok(id)
    }

    /// `keten.json` of a law fetched here: its versions and the dossiers per transition.
    pub fn keten(&self, bwb: &str) -> Option<Value> {
        id::is_bwb(bwb)
            .then(|| lees(&self.bron().join("regelingen").join(bwb).join("keten.json")))?
    }

    // A dossier that failed: the queue does not try it again by itself.
    fn mislukt_pad(&self) -> PathBuf {
        self.opgehaald().join("mislukt.json")
    }
    pub fn mislukt(&self) -> Map<String, Value> {
        lees(&self.mislukt_pad())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }
    pub fn zet_mislukt(&self, nr: &str, reden: &str) {
        self.wijzig_mislukt(|m| {
            m.insert(
                nr.into(),
                json!({ "reden": reden, "t": chrono::Local::now().format("%Y-%m-%d").to_string() }),
            );
        });
    }
    pub fn wis_mislukt(&self, nr: &str) {
        self.wijzig_mislukt(|m| {
            m.remove(nr);
        });
    }
    fn wijzig_mislukt(&self, f: impl FnOnce(&mut Map<String, Value>)) {
        let _slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        let mut m = self.mislukt();
        f(&mut m);
        let _ = fs::create_dir_all(self.opgehaald());
        let _ = fs::write(
            self.mislukt_pad(),
            serde_json::to_vec_pretty(&m).unwrap_or_default(),
        );
    }
}
