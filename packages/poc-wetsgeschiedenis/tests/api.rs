//! The service without the internet: every request goes through the router
//! (no port), and the worker is `tests/nepwerker.py`, with the same contract
//! as `werker.py`. One test, `echte_wet`, uses the real worker and the
//! internet; it only runs with `--ignored`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use regelrecht_poc_wetsgeschiedenis::{router, AppState, Instellingen};
use serde_json::{json, Value};
use tower::ServiceExt;

struct Proef {
    app: Router,
    state: AppState,
    map: PathBuf,
}

/// Next to this file. `file!()` is absolute when the crate is built on its
/// own, and relative to the workspace when built in it.
fn nepwerker() -> PathBuf {
    let hier = Path::new(file!());
    if hier.is_absolute() {
        hier.with_file_name("nepwerker.py")
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/nepwerker.py")
    }
}

fn proef(max: usize) -> Proef {
    proef_met(max, nepwerker())
}

fn proef_met(max: usize, werker: PathBuf) -> Proef {
    static N: AtomicU32 = AtomicU32::new(0);
    let map = std::env::temp_dir().join(format!(
        "wg-proef-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&map);
    let vast = map.join("vast");
    let dist = map.join("dist");
    std::fs::create_dir_all(&vast).unwrap();
    std::fs::create_dir_all(&dist).unwrap();
    std::fs::write(
        vast.join("dossiers.json"),
        r#"[{"dossier": "35968", "naam": "Archiefwet 2026", "soort": "nieuwe wet"}]"#,
    )
    .unwrap();
    std::fs::write(
        vast.join("35968.json"),
        r#"{"dossier": "35968", "naam": "Archiefwet 2026"}"#,
    )
    .unwrap();
    std::fs::write(dist.join("index.html"), "<!doctype html><title>t</title>").unwrap();
    std::fs::write(map.join("geheim.txt"), "buiten de frontend").unwrap();
    let state = AppState::new(Instellingen {
        vast,
        data: map.join("data"),
        max,
        python: "python3".into(),
        werker,
        verbinden: 15,
        uitgeweken: None,
    });
    state.start();
    let app = router(state.clone(), "/wetsgeschiedenis/", dist.to_str().unwrap());
    Proef { app, state, map }
}

impl Proef {
    async fn vraag(
        &self,
        methode: &str,
        pad: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value, String) {
        let req = Request::builder().method(methode).uri(pad);
        let req = match body {
            Some(b) => req
                .header("content-type", "application/json")
                .body(Body::from(b.to_string())),
            None => req.body(Body::empty()),
        }
        .unwrap();
        let res = self.app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let tekst = String::from_utf8_lossy(&bytes).to_string();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            tekst,
        )
    }

    async fn get(&self, pad: &str) -> (StatusCode, Value, String) {
        self.vraag("GET", pad, None).await
    }

    async fn haal(&self, body: Value) -> (StatusCode, Value) {
        let (s, v, _) = self
            .vraag("POST", "/wetsgeschiedenis/api/haal", Some(body))
            .await;
        (s, v)
    }

    async fn wacht_op(&self, id: &str) -> Value {
        for _ in 0..300 {
            let t = self
                .get(&format!("/wetsgeschiedenis/api/taak?id={id}"))
                .await
                .1;
            if t["status"] == "klaar" || t["status"] == "fout" {
                return t;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("opdracht {id} werd niet klaar");
    }

    async fn rij_leeg(&self) {
        for _ in 0..500 {
            let r = self.get("/wetsgeschiedenis/api/status").await.1["rij"].clone();
            if r["huidig"].is_null() && r["wachtend"].as_array().is_some_and(Vec::is_empty) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("de rij werd niet leeg");
    }

    fn poort(&self, dicht: bool) {
        let p = self.state.opslag.bron().join("poort");
        if dicht {
            std::fs::create_dir_all(self.state.opslag.bron()).unwrap();
            std::fs::write(p, "").unwrap();
        } else {
            let _ = std::fs::remove_file(p);
        }
    }
}

impl Drop for Proef {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.map);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn status_zegt_dat_er_opgehaald_kan_worden() {
    let p = proef(10);
    let (s, v, _) = p.get("/wetsgeschiedenis/api/status").await;
    assert_eq!((s, v["ophalen"].clone()), (StatusCode::OK, json!(true)));
    assert_eq!(p.get("/healthz").await.0, StatusCode::OK);
}

#[tokio::test(flavor = "multi_thread")]
async fn de_keuzelijst_heeft_wat_in_het_image_zit() {
    let p = proef(10);
    let (s, v, _) = p.get("/wetsgeschiedenis/gegevens/dossiers.json").await;
    assert_eq!(
        (s, v[0]["dossier"].clone()),
        (StatusCode::OK, json!("35968"))
    );
    let (s, v, _) = p.get("/wetsgeschiedenis/gegevens/35968.json").await;
    assert_eq!(
        (s, v["naam"].clone()),
        (StatusCode::OK, json!("Archiefwet 2026"))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn een_vreemd_id_of_pad_geeft_niets_van_buiten() {
    let p = proef(10);
    assert_eq!(
        p.get("/wetsgeschiedenis/gegevens/..%2F..%2Fgeheim.json")
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    for pad in [
        "/wetsgeschiedenis/../geheim.txt",
        "/wetsgeschiedenis/assets/../../geheim.txt",
        "/wetsgeschiedenis/%2e%2e/geheim.txt",
    ] {
        assert!(!p.get(pad).await.2.contains("buiten de frontend"), "{pad}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn een_onbekend_pad_krijgt_de_pagina() {
    let p = proef(10);
    let (s, _, t) = p.get("/wetsgeschiedenis/iets/anders").await;
    assert_eq!(s, StatusCode::OK);
    assert!(t.contains("<!doctype html>"));
}

#[tokio::test(flavor = "multi_thread")]
async fn een_verkeerd_bwb_id_wordt_geweigerd() {
    let p = proef(10);
    assert_eq!(
        p.get("/wetsgeschiedenis/api/regeling/BWB123").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        p.get("/wetsgeschiedenis/api/diagnose?bwb=x").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        p.haal(json!({"bwb": "x; rm -rf /", "keten": true})).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        p.haal(json!({"bwb": "BWBR0007376", "dossier": "../1"}))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let (s, v, _) = p.vraag("POST", "/wetsgeschiedenis/api/haal", None).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert!(
        v["fout"].is_string(),
        "ook een lege body krijgt een fout als JSON"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn de_opzoeking_geeft_de_dossiers_per_versie_en_een_fout_met_haar_reden() {
    let p = proef(10);
    let (s, v, _) = p.get("/wetsgeschiedenis/api/regeling/BWBR0007376").await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["titel"], "Archiefwet 1995");
    assert_eq!(
        v["versies"].as_array().unwrap().len(),
        1,
        "alleen versies met een dossier"
    );
    assert_eq!(
        v["versies"][0]["dossiers"][0],
        json!({"nr": "36889", "beschikbaar": false, "rij": null})
    );
    let (s, v, _) = p.get("/wetsgeschiedenis/api/regeling/BWBR0029404").await;
    assert_eq!(s, StatusCode::BAD_GATEWAY);
    assert!(
        v["fout"]
            .as_str()
            .unwrap()
            .contains("geen antwoord binnen de tijd"),
        "{v}"
    );
    let log = p.get("/wetsgeschiedenis/api/log").await.1;
    let uit: Vec<&Value> = log
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["soort"] == "uit")
        .collect();
    assert!(
        uit.iter().any(|r| r["code"] == 28),
        "de vraag naar buiten staat in het logboek"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn opdrachten_lopen_na_elkaar_en_hun_uitkomst_komt_in_de_lijst() {
    let p = proef(10);
    p.poort(true);
    let (s, t1) = p
        .haal(json!({"bwb": "BWBR0007376", "dossier": "36889"}))
        .await;
    assert_eq!(s, StatusCode::ACCEPTED);
    // Eerst loopt de eerste; dan wacht de tweede achter haar.
    for _ in 0..300 {
        if p.get(&format!(
            "/wetsgeschiedenis/api/taak?id={}",
            t1["id"].as_str().unwrap()
        ))
        .await
        .1["status"]
            == "bezig"
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let (_, t2) = p
        .haal(json!({"bwb": "BWBR0007376", "dossier": 20202}))
        .await;
    assert_eq!(
        (t2["status"].clone(), t2["plaats"].clone()),
        (json!("wacht"), json!(1))
    );
    // Dezelfde opdracht nog eens: geen tweede in de rij.
    assert_eq!(
        p.haal(json!({"bwb": "BWBR0007376", "dossier": "20202"}))
            .await
            .1["id"],
        t2["id"]
    );
    p.poort(false);
    let (a, b) = (
        p.wacht_op(t1["id"].as_str().unwrap()).await,
        p.wacht_op(t2["id"].as_str().unwrap()).await,
    );
    assert_eq!(
        (
            a["status"].clone(),
            a["uitkomst"].clone(),
            b["uitkomst"].clone()
        ),
        (json!("klaar"), json!("36889"), json!("20202"))
    );
    assert!(a["log"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r == "stukken opgehaald"));
    let lijst = p.get("/wetsgeschiedenis/gegevens/dossiers.json").await.1;
    let d = lijst
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["dossier"] == "36889")
        .expect("36889 in de lijst");
    assert_eq!(d["opgehaald"], true);
}

#[tokio::test(flavor = "multi_thread")]
async fn een_wet_zet_haar_dossiers_op_de_achtergrond_de_jongste_eerst() {
    let p = proef(10);
    p.state.opslag.zet_mislukt("40404", "geen XML");
    p.poort(true);
    let (s, _) = p.haal(json!({"bwb": "BWBR0099999", "keten": true})).await;
    assert_eq!(s, StatusCode::ACCEPTED);
    let mut rij = Value::Null;
    for _ in 0..300 {
        rij = p.get("/wetsgeschiedenis/api/status").await.1["rij"].clone();
        if rij["wachtend"].as_array().is_some_and(|w| w.len() >= 2) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // Jongste eerst; wat er al is of mislukte, niet; elk voor deze wet.
    let alle: Vec<&Value> = rij["huidig"]
        .as_object()
        .map(|_| &rij["huidig"])
        .into_iter()
        .chain(rij["wachtend"].as_array().unwrap())
        .collect();
    let nrs: Vec<&str> = alle
        .iter()
        .filter(|t| t["wat"] == "dossier")
        .filter_map(|t| t["dossier"].as_str())
        .collect();
    assert_eq!(nrs, ["30303", "20202", "10101"]);
    assert!(alle
        .iter()
        .filter(|t| t["wat"] == "dossier")
        .all(|t| t["voor"] == "BWBR0099999"));
    // Een bezoeker die 10101 vraagt, gaat voor op de achtergrond.
    p.haal(json!({"bwb": "BWBR0099999", "dossier": "10101"}))
        .await;
    let wachtend = p.get("/wetsgeschiedenis/api/status").await.1["rij"]["wachtend"].clone();
    assert_eq!(wachtend[0]["dossier"], "10101");
    p.poort(false);
    p.rij_leeg().await;
    // Na elk dossier is de wet opnieuw opgebouwd, met wat er nu is.
    let w = p.get("/wetsgeschiedenis/gegevens/BWBR0099999.json").await.1;
    assert_eq!(
        w["dossiers_er"],
        json!(["35968", "10101", "20202", "30303"])
    );
    // De opgehaalde dossiers staan in de lijst, bij hun wet.
    let lijst = p.get("/wetsgeschiedenis/gegevens/dossiers.json").await.1;
    assert!(
        lijst
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["bij"] == "BWBR0099999")
            .count()
            == 3
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn een_grote_wet_haalt_eerst_haar_hele_geschiedenis_en_dan_de_dossiers_die_haar_maakten() {
    let p = proef(10);
    p.poort(true);
    let (_, t) = p.haal(json!({"bwb": "BWBR0005537", "keten": true})).await;
    assert_eq!(
        p.wacht_op(t["id"].as_str().unwrap()).await["status"],
        "klaar"
    );
    let mut nrs: Vec<String> = Vec::new();
    for _ in 0..300 {
        let rij = p.get("/wetsgeschiedenis/api/status").await.1["rij"].clone();
        nrs = rij["huidig"]
            .as_object()
            .map(|_| &rij["huidig"])
            .into_iter()
            .chain(rij["wachtend"].as_array().unwrap())
            .filter(|t| t["wat"] == "dossier")
            .filter_map(|t| t["dossier"].as_str().map(String::from))
            .collect();
        if nrs.first().is_some_and(|n| n == "21221") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // Eerst de hele geschiedenis (niet meer afgekapt), dan de dossiers die de
    // wet maakten, de oudste eerst, en dan de rest, de jongste eerst.
    assert!(p.state.opslag.keten("BWBR0005537").unwrap()["afgekapt"].is_null());
    assert_eq!(nrs, ["21221", "22495", "30303", "20202", "40404", "10101"]);
    p.poort(false);
    p.rij_leeg().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn een_wet_van_voor_de_twee_stappen_krijgt_na_een_herstart_alsnog_haar_hele_geschiedenis() {
    let p = proef(10);
    // Zo stond de Awb er na de vorige versie van de dienst: 40 versies, zonder
    // `afgekapt`.
    let k = p.state.opslag.bron().join("regelingen").join("BWBR0005537");
    std::fs::create_dir_all(&k).unwrap();
    let versies: Vec<Value> = (0..40)
        .map(|i| json!({"datum": format!("20{:02}-01-01", i)}))
        .collect();
    std::fs::write(
        k.join("keten.json"),
        json!({"versies": versies, "overgangen": []}).to_string(),
    )
    .unwrap();
    let g = p.state.opslag.opgehaald();
    std::fs::create_dir_all(&g).unwrap();
    std::fs::write(
        g.join("dossiers.json"),
        json!([{"dossier": "BWBR0005537", "soort": "keten"}]).to_string(),
    )
    .unwrap();
    assert!(regelrecht_poc_wetsgeschiedenis::rij::herstel(&p.state) >= 1);
    for _ in 0..300 {
        if p.state
            .opslag
            .keten("BWBR0005537")
            .is_some_and(|k| k["ontstaan"][0] == "21221")
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("de hele geschiedenis werd na de herstart niet opgehaald");
}

#[tokio::test(flavor = "multi_thread")]
async fn een_mislukt_dossier_wordt_onthouden_en_een_bezoeker_mag_het_opnieuw() {
    let p = proef(10);
    let (_, t) = p
        .haal(json!({"bwb": "BWBR0007376", "dossier": "40404"}))
        .await;
    let t = p.wacht_op(t["id"].as_str().unwrap()).await;
    assert_eq!(
        (t["status"].clone(), t["fout"].clone()),
        (json!("fout"), json!("geen XML"))
    );
    assert!(p.state.opslag.mislukt().contains_key("40404"));
    let (s, t) = p
        .haal(json!({"bwb": "BWBR0007376", "dossier": "40404"}))
        .await;
    assert_eq!(
        (s, t["status"].clone()),
        (StatusCode::ACCEPTED, json!("wacht"))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn wat_is_opgehaald_blijft_onder_het_maximum() {
    let p = proef(2);
    for nr in ["10001", "10002", "10003"] {
        let (_, t) = p.haal(json!({"bwb": "BWBR0007376", "dossier": nr})).await;
        p.wacht_op(t["id"].as_str().unwrap()).await;
    }
    let lijst = p.get("/wetsgeschiedenis/gegevens/dossiers.json").await.1;
    let eigen: Vec<&str> = lijst
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["opgehaald"] == true)
        .filter_map(|d| d["dossier"].as_str())
        .collect();
    assert_eq!(eigen, ["10002", "10003"]);
    assert_eq!(
        p.get("/wetsgeschiedenis/gegevens/10001.json").await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn elk_verzoek_staat_in_het_logboek_maar_het_pollen_niet() {
    let p = proef(10);
    p.get("/wetsgeschiedenis/gegevens/dossiers.json").await;
    p.get("/wetsgeschiedenis/api/taak").await;
    let log = p.get("/wetsgeschiedenis/api/log").await.1;
    let verzoeken: Vec<&Value> = log
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["soort"] == "verzoek")
        .collect();
    assert_eq!(
        verzoeken.last().unwrap()["pad"],
        "/wetsgeschiedenis/gegevens/dossiers.json"
    );
    assert!(!verzoeken
        .iter()
        .any(|r| r["pad"] == "/wetsgeschiedenis/api/taak"));
}

/// The real worker and the real chain, with the internet: the BES decree, an
/// AMvB whose toelichting comes from the explanatory notes in its Staatsblad.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "haalt echt op van wetten.overheid.nl en officielebekendmakingen.nl"]
async fn echte_wet() {
    let p = proef_met(10, nepwerker().with_file_name("../werker.py"));
    let (s, reg, _) = p.get("/wetsgeschiedenis/api/regeling/BWBR0029404").await;
    assert_eq!(
        (s, reg["titel"].clone()),
        (StatusCode::OK, json!("Besluit zorgverzekering BES"))
    );
    let (_, t) = p.haal(json!({"bwb": "BWBR0029404", "keten": true})).await;
    let id = t["id"].as_str().unwrap().to_string();
    let mut t = Value::Null;
    for _ in 0..600 {
        t = p
            .get(&format!("/wetsgeschiedenis/api/taak?id={id}"))
            .await
            .1;
        if t["status"] == "klaar" || t["status"] == "fout" {
            break;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    assert_eq!(t["status"], "klaar", "{t}");
    let w = p.get("/wetsgeschiedenis/gegevens/BWBR0029404.json").await.1;
    let tel = &w["commentaar"]["tel"];
    println!("{} | {} | {}", w["naam"], w["tel"], tel);
    assert!(
        tel["brokken"].as_u64().unwrap() > 20
            && tel["artikelen_met_toelichting"].as_u64().unwrap() > 10,
        "{tel}"
    );
    let log = p.get("/wetsgeschiedenis/api/log").await.1;
    assert!(log
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["soort"] == "uit" && r["host"] == "zoek.officielebekendmakingen.nl"));
}
