//! The chain of each case (tab Keten), derived from the regelwerken the case
//! loads.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::Path;

use regelrecht_poc_chronolexografie::kern::Corpus;
use regelrecht_poc_chronolexografie::spel::Spel;
use serde_json::Value;

fn keten(id: &str) -> Value {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut spel =
        Spel::new(Corpus::alle(&dir.join("../../corpus"), &dir.join("casussen")).unwrap());
    spel.kiezen(id).unwrap();
    spel.keten("2026-10-20".parse().unwrap()).unwrap()
}

/// The chain as indented lines, one per step, to read and to match.
fn regels(s: &Value, diepte: usize, uit: &mut Vec<String>) {
    let tekst = match s["soort"].as_str().unwrap() {
        "artikel" => format!(
            "artikel {}#{}{}{}",
            s["wet"].as_str().unwrap(),
            s["artikel"].as_str().unwrap(),
            if s["parameters"].as_object().unwrap().is_empty() {
                String::new()
            } else {
                format!(" {}", s["parameters"])
            },
            if s["fictief"] == true {
                " (fictief)"
            } else {
                ""
            },
        ),
        "begrip" => format!("begrip {}", s["id"].as_str().unwrap()),
        "cel" => format!(
            "cel {} {} {}",
            s["cel"],
            s["antwoord"].as_str().unwrap(),
            s["reductie"]
        ),
        "kroniek" => format!(
            "kroniek {} {} {}",
            s["besluiten_onder"],
            s["executogrammen_van"],
            s["cel"].as_str().unwrap()
        ),
        "eerder" => format!(
            "eerder {}#{}",
            s["wet"].as_str().unwrap(),
            s["artikel"].as_str().unwrap()
        ),
        x => format!("unknown step {x}"),
    };
    uit.push(format!("{}{tekst}", "  ".repeat(diepte)));
    for k in s["kinderen"].as_array().into_iter().flatten() {
        regels(k, diepte + 1, uit);
    }
}

fn boom(id: &str) -> Vec<String> {
    let mut uit = Vec::new();
    regels(&keten(id), 0, &mut uit);
    uit
}

/// Whether `stappen` lie on one path down the chain, each a step under the
/// one before: each is the start of a line of [`regels`], without indentation.
fn op_een_pad(boom: &[String], stappen: &[&str]) -> bool {
    fn diepte(r: &str) -> usize {
        (r.len() - r.trim_start().len()) / 2
    }
    fn zoek(boom: &[String], van: usize, onder: Option<usize>, stappen: &[&str]) -> bool {
        let Some((eerste, rest)) = stappen.split_first() else {
            return true;
        };
        let einde = onder.map_or(boom.len(), |o| {
            let d = diepte(&boom[o]);
            boom[o + 1..]
                .iter()
                .position(|r| diepte(r) <= d)
                .map_or(boom.len(), |i| o + 1 + i)
        });
        (van..einde)
            .filter(|&i| boom[i].trim_start().starts_with(eerste))
            .any(|i| zoek(boom, i + 1, Some(i), rest))
    }
    zoek(boom, 0, None, stappen)
}

#[test]
fn bedrijfsparkeervergunning_from_the_ordinance_to_the_chamber_and_the_awb() {
    let boom = boom("bedrijfsparkeervergunning");
    assert!(
        boom[0].starts_with("artikel fictieve_parkeerverordening_gemeente_wilgendaal#2"),
        "{boom:#?}"
    );
    assert!(
        op_een_pad(&boom, &[
            "artikel fictieve_parkeerverordening_gemeente_wilgendaal#2",
            "begrip bedrijf_gevestigd_in_vergunninggebied",
            "artikel fictief_uitvoeringsbesluit_parkeren_gemeente_wilgendaal#2",
            "begrip postcode_vestiging_volgens_handelsregister",
            "artikel fictief_uitvoeringsbesluit_parkeren_gemeente_wilgendaal#4",
            "cel \"kamer_van_koophandel\" postcode_vestiging \"reductie_vestiging_in_handelsregister\"",
            "artikel reductie_vestiging_in_handelsregister#1",
            "kroniek null \"fictief_beleid_handelsregister_kamer_van_koophandel#1#vestiging_ingeschreven\" kamer_van_koophandel",
        ]),
        "{boom:#?}"
    );
    assert!(
        op_een_pad(&boom, &[
            "artikel fictieve_parkeerverordening_gemeente_wilgendaal#2",
            "begrip maximum_bedrijfsparkeervergunningen_bereikt",
            "artikel fictief_uitvoeringsbesluit_parkeren_gemeente_wilgendaal#3",
            "begrip aantal_geldige_bedrijfsparkeervergunningen",
            "artikel fictief_uitvoeringsbesluit_parkeren_gemeente_wilgendaal#5",
            "cel \"gemeente_wilgendaal\" aantal_geldige_bedrijfsparkeervergunningen \"reductie_geldige_bedrijfsparkeervergunningen\"",
            "artikel reductie_geldige_bedrijfsparkeervergunningen#1",
            "artikel reductie_algemene_wet_bestuursrecht#2 {\"basis\":\"fictieve_parkeerverordening_gemeente_wilgendaal#2\"}",
            "artikel reductie_algemene_wet_bestuursrecht#1 {\"basis\":\"fictieve_parkeerverordening_gemeente_wilgendaal#2\"}",
            "kroniek \"fictieve_parkeerverordening_gemeente_wilgendaal#2\" null gemeente_wilgendaal",
        ]),
        "{boom:#?}"
    );
}

#[test]
fn parkeerkaart_through_babw_49_to_the_municipality_and_the_ggd() {
    let boom = boom("gehandicaptenparkeerkaart");
    assert!(boom[0]
        .starts_with("artikel besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49"));
    assert!(
        op_een_pad(&boom, &[
            "artikel besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49",
            "begrip bevoegdheid_tot_verstrekken_gebruikt",
            "artikel fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#1",
            "begrip aanvrager_heeft_geldige_gehandicaptenparkeerkaart",
            "artikel fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#2",
            "cel \"gemeente_wilgendaal\" aanvrager_heeft_geldige_gehandicaptenparkeerkaart \"reductie_geldige_gehandicaptenparkeerkaart\"",
            "artikel reductie_geldige_gehandicaptenparkeerkaart#1",
            "artikel reductie_algemene_wet_bestuursrecht#2 {\"basis\":\"besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49\"}",
            "artikel reductie_algemene_wet_bestuursrecht#1",
            "kroniek \"besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49\" null gemeente_wilgendaal",
        ]),
        "{boom:#?}"
    );
    assert!(
        op_een_pad(&boom, &[
            "artikel besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49",
            "begrip langdurige_loopbeperking",
            "artikel fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#6",
            "artikel fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5",
            "begrip advies_langdurige_loopbeperking",
            "artikel fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#7",
            "cel \"ggd_wilgendaal\" advies_langdurige_loopbeperking \"reductie_geneeskundig_advies\"",
            "artikel reductie_geneeskundig_advies#1",
            "kroniek null \"fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_uitgebracht\" ggd_wilgendaal",
        ]),
        "{boom:#?}"
    );
    // Several cells give one answer: each stands under the policy that asks it.
    for cel in ["gemeente_wilgendaal", "gemeente_oostmeer", "cbr"] {
        assert!(
            boom.iter().any(|r| r.trim_start().starts_with(&format!(
                "cel \"{cel}\" aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt"
            ))),
            "{cel}: {boom:#?}"
        );
    }
}

#[test]
fn vakantieverlof_through_the_policy_of_the_head_to_the_school() {
    let boom = boom("vakantieverlof");
    assert!(
        op_een_pad(&boom, &[
            "artikel leerplichtwet_1969#13a",
            "begrip verlof_al_verleend_in_schooljaar",
            "artikel fictief_beleid_vakantieverlof_obs_de_klimop#2",
            "cel \"obs_de_klimop\" verlof_al_verleend_in_schooljaar",
            "artikel reductie_verlof_al_verleend_in_schooljaar#1",
            "artikel reductie_algemene_wet_bestuursrecht#2 {\"basis\":\"leerplichtwet_1969#13a\"}",
        ]),
        "{boom:#?}"
    );
}

/// Every step of an article holds its `machine_readable` as written in its
/// file, and that file is one of the regelwerken the case loads.
#[test]
fn each_article_shows_its_yaml_from_the_file_the_case_loads() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus");
    for id in [
        "bedrijfsparkeervergunning",
        "gehandicaptenparkeerkaart",
        "vakantieverlof",
    ] {
        let mut stappen = vec![keten(id)];
        let mut artikelen = 0;
        while let Some(s) = stappen.pop() {
            if s["soort"] == "artikel" {
                artikelen += 1;
                let bestand = s["bestand"].as_str().unwrap();
                let tekst = std::fs::read_to_string(corpus.join(bestand)).unwrap();
                assert!(s["yaml"].is_string(), "{bestand} {}", s["artikel"]);
                let yaml = s["yaml"].as_str().unwrap();
                let eerste = yaml.lines().next().unwrap();
                assert_eq!(eerste, "machine_readable:");
                // Indented as in the file, the block is a part of it.
                let ingesprongen = tekst
                    .lines()
                    .find(|r| r.trim() == "machine_readable:")
                    .map(|r| r.len() - r.trim_start().len())
                    .unwrap();
                let terug: String = yaml
                    .lines()
                    .map(|r| {
                        if r.is_empty() {
                            String::new()
                        } else {
                            format!("{}{r}", " ".repeat(ingesprongen))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(tekst.contains(&terug), "{bestand} {}", s["artikel"]);
            }
            stappen.extend(s["kinderen"].as_array().into_iter().flatten().cloned());
        }
        assert!(artikelen > 5, "{id}: {artikelen}");
    }
}
