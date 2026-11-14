//! What a visitor may name. Everything that reaches a path or the worker's
//! command line passes one of these first.

/// A BWB id: `BWBR` and seven digits.
pub fn is_bwb(s: &str) -> bool {
    s.len() == 11 && s.starts_with("BWBR") && s[4..].bytes().all(|b| b.is_ascii_digit())
}

/// A Kamerdossier: five digits, optionally `-` and a part of one to six
/// capitals or digits (`36228-XV`).
pub fn is_dossier(s: &str) -> bool {
    let (nr, deel) = s.split_once('-').map_or((s, None), |(a, b)| (a, Some(b)));
    nr.len() == 5
        && nr.bytes().all(|b| b.is_ascii_digit())
        && deel.is_none_or(|d| {
            (1..=6).contains(&d.len())
                && d.bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        })
}

/// The id of a timeline under `gegevens/`: a dossier or a BWB id, never a path.
pub fn is_id(s: &str) -> bool {
    (1..=24).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alleen_wat_een_id_is() {
        assert!(is_bwb("BWBR0007376"));
        assert!(
            !is_bwb("BWBR000737")
                && !is_bwb("bwbr0007376")
                && !is_bwb("BWBR0007376 ")
                && !is_bwb("x; rm -rf /")
        );
        assert!(is_dossier("36889") && is_dossier("36228-XV"));
        assert!(
            !is_dossier("../1")
                && !is_dossier("3688")
                && !is_dossier("36889-")
                && !is_dossier("36889-abc")
        );
        assert!(is_id("BWBR0007376") && is_id("36889"));
        assert!(!is_id("..%2F..%2Fgeheim") && !is_id("../x") && !is_id(""));
    }
}
