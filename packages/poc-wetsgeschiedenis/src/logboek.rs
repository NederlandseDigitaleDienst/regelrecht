//! The log: every request, every task and every call the worker makes to the
//! outside, as one JSON line on stderr (the pod logs) and in a ring in memory
//! that `api/log` shows. That is what lets a visitor, or the beheerder, see
//! why fetching failed without access to the pod.

use std::collections::VecDeque;
use std::sync::Mutex;

use serde_json::{Map, Value};

pub struct Logboek {
    regels: Mutex<VecDeque<Value>>,
    max: usize,
}

impl Logboek {
    pub fn new(max: usize) -> Self {
        Self {
            regels: Mutex::new(VecDeque::with_capacity(max)),
            max,
        }
    }

    /// One line: `soort` and the fields of `velden` (an object), with the time.
    pub fn schrijf(&self, soort: &str, velden: Value) {
        let mut regel = Map::new();
        regel.insert(
            "t".into(),
            chrono::Local::now()
                .format("%Y-%m-%dT%H:%M:%S")
                .to_string()
                .into(),
        );
        regel.insert("soort".into(), soort.into());
        if let Value::Object(v) = velden {
            regel.extend(v.into_iter().filter(|(_, x)| !x.is_null()));
        }
        let regel = Value::Object(regel);
        eprintln!("{regel}");
        let mut r = self.regels.lock().unwrap_or_else(|e| e.into_inner());
        if r.len() == self.max {
            r.pop_front();
        }
        r.push_back(regel);
    }

    pub fn alles(&self) -> Vec<Value> {
        self.regels
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .collect()
    }
}
