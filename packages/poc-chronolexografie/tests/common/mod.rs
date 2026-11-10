//! Helpers the integration tests of the PoC share. Every test binary
//! compiles this module on its own.

#![allow(dead_code)]

use serde_json::{Map, Value, json};

/// `bevindingen` for a decision on an objection, with what the authority
/// judges on admissibility (Awb 7:11) where `bevindingen` says nothing: the
/// objector is an interested party (1:2, 7:1 and 8:1), and was at fault for
/// a late objection, so 6:11 does not save it.
pub fn ontvankelijk(bevindingen: &Map<String, Value>) -> Map<String, Value> {
    let mut uit = bevindingen.clone();
    uit.entry("indiener_bezwaarschrift_is_belanghebbende")
        .or_insert(json!(true));
    uit.entry("indiener_bezwaarschrift_niet_in_verzuim")
        .or_insert(json!(false));
    uit
}
