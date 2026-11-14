//! What a beheerder wants to know when fetching fails, without looking in the
//! pod: does every host answer, can the service write in its data directory,
//! is the worker there (python3, pyyaml, curl), is there a proxy; and with a
//! BWB id, the lookup itself and how long it took.

use std::time::Instant;

use serde_json::{json, Map, Value};
use tokio::process::Command;
use tokio::task::JoinSet;

use crate::{opslag, werker, AppState};

/// The hosts the worker fetches from, and what for.
pub const HOSTS: [(&str, &str); 5] = [
    (
        "wetten.overheid.nl",
        "nodig: de versies van een wet en de dossiers die haar maakten",
    ),
    (
        "zoek.officielebekendmakingen.nl",
        "nodig: Kamerstukken, Staatsbladen en Staatscourant",
    ),
    (
        "repository.overheid.nl",
        "nodig: de zoekdienst naar Kamerstukken (SRU)",
    ),
    (
        "gegevensmagazijn.tweedekamer.nl",
        "nodig: documenten en stemmingen van de Tweede Kamer",
    ),
    (
        "repository.officiele-overheidspublicaties.nl",
        "niet nodig: de BWB-XML, alleen als wetten.overheid.nl een versie mist",
    ),
];

/// What curl means by an exit code.
pub fn curl_fout(code: i32) -> Option<&'static str> {
    Some(match code {
        5 => "de proxy is niet te vinden",
        6 => "de host is niet te vinden (DNS)",
        7 => "geen verbinding met de host (uitgaand verkeer dicht?)",
        18 => "de overdracht brak halverwege af",
        28 => "geen antwoord binnen de tijd (uitgaand verkeer dicht, of de host is traag)",
        35 => "de TLS-verbinding mislukte",
        52 => "de server gaf een leeg antwoord",
        56 => "de verbinding brak af",
        60 => "het certificaat van de host wordt niet vertrouwd",
        _ => return None,
    })
}

async fn uit(cmd: &str, args: &[&str]) -> Option<(bool, String, String)> {
    let o = Command::new(cmd).args(args).output().await.ok()?;
    Some((
        o.status.success(),
        String::from_utf8_lossy(&o.stdout).trim().into(),
        String::from_utf8_lossy(&o.stderr).trim().into(),
    ))
}

/// One real request to the host. Any HTTP answer, a 404 too, means outgoing
/// traffic gets through.
async fn host(host: &'static str, verbinden: u64) -> Value {
    let url = format!("https://{host}/");
    let tijd = verbinden.to_string();
    let args = [
        "-sS",
        "-o",
        "/dev/null",
        "--connect-timeout",
        &tijd,
        "--max-time",
        "20",
        "-A",
        "Mozilla/5.0 (regelrecht-werkbank)",
        "-w",
        "%{http_code} %{remote_ip} %{time_namelookup} %{time_connect} %{time_total}",
        &url,
    ];
    let o = match Command::new("curl").args(args).output().await {
        Ok(o) => o,
        Err(e) => return json!({ "ok": false, "fout": format!("curl start niet: {e}") }),
    };
    let s = String::from_utf8_lossy(&o.stdout).to_string();
    let mut d = s.split_whitespace();
    let (code, ip, dns, verbinding, totaal) = (
        d.next().unwrap_or(""),
        d.next(),
        d.next(),
        d.next(),
        d.next(),
    );
    let rc = o.status.code().unwrap_or(-1);
    let fout = String::from_utf8_lossy(&o.stderr)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    json!({ "ok": o.status.success() && !code.is_empty() && code != "000", "http": code, "ip": ip, "dns_s": dns,
            "verbinding_s": verbinding, "totaal_s": totaal, "curl": (rc != 0).then_some(rc),
            "uitleg": curl_fout(rc), "fout": (!fout.is_empty()).then_some(fout) })
}

/// A proxy URL without its password; curl also takes one without a scheme
/// (`user:pass@proxy:3128`).
fn zonder_wachtwoord(url: &str) -> String {
    let Some(b) = url.rfind('@') else {
        return url.into();
    };
    let a = url
        .find("//")
        .map(|a| a + 2)
        .filter(|a| *a <= b)
        .unwrap_or(0);
    format!("{}***@{}", &url[..a], &url[b + 1..])
}

pub async fn diagnose(state: &AppState, bwb: Option<&str>) -> Value {
    let t0 = Instant::now();
    let data = state.opslag.data().to_path_buf();
    let schrijft = opslag::schrijfbaar(&data);
    let mut d = Map::new();
    d.insert(
        "tijd".into(),
        chrono::Local::now()
            .format("%Y-%m-%dT%H:%M:%S")
            .to_string()
            .into(),
    );
    d.insert("verbindingstijd_s".into(), state.inst.verbinden.into());

    let curl = uit("curl", &["--version"])
        .await
        .map(|(_, o, _)| o.lines().next().unwrap_or("").to_string());
    d.insert(
        "curl".into(),
        curl.clone().unwrap_or_else(|| "ontbreekt".into()).into(),
    );
    let werker = uit(
        &state.inst.python,
        &["-c", "import sys, yaml; print(sys.version.split()[0])"],
    )
    .await;
    let werker_ok = werker.as_ref().is_some_and(|w| w.0) && state.inst.werker.is_file();
    d.insert(
        "werker".into(),
        json!({ "ok": werker_ok, "python": werker.as_ref().map(|w| w.1.clone()), "pad": state.inst.werker,
                "fout": werker.as_ref().filter(|w| !w.0).map(|w| w.2.clone()) }),
    );

    let uid = uit("id", &["-u"]).await.map(|x| x.1).unwrap_or_default();
    let gid = uit("id", &["-g"]).await.map(|x| x.1).unwrap_or_default();
    let mut opslag =
        json!({ "pad": data, "schrijfbaar": schrijft, "proces": format!("uid {uid}, gid {gid}") });
    if schrijft {
        if let Some((true, df, _)) = uit("df", &["-k", data.to_str().unwrap_or("/")]).await {
            let vrij = df
                .lines()
                .nth(1)
                .and_then(|r| r.split_whitespace().nth(3))
                .and_then(|k| k.parse::<u64>().ok());
            opslag["vrij_mb"] = vrij.map(|k| k / 1024).into();
        }
    }
    if let Some(reden) = &state.inst.uitgeweken {
        opslag["uitgeweken"] = reden.clone().into();
    }
    d.insert("data".into(), opslag);

    let proxy: Map<String, Value> = [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "NO_PROXY",
        "no_proxy",
    ]
    .iter()
    .filter_map(|k| {
        std::env::var(k)
            .ok()
            .map(|v| (k.to_string(), zonder_wachtwoord(&v).into()))
    })
    .collect();
    d.insert("proxy".into(), proxy.into());
    d.insert(
        "dossiers".into(),
        json!({ "in_het_image": state.opslag.index().iter().filter(|x| x["opgehaald"] != true).count(),
                "opgehaald": state.opslag.index().iter().filter(|x| x["opgehaald"] == true).count(), "max": state.inst.max }),
    );

    let mut taken = JoinSet::new();
    for (h, _) in HOSTS {
        let v = state.inst.verbinden;
        taken.spawn(async move { (h, host(h, v).await) });
    }
    let mut hosts = Map::new();
    while let Some(Ok((h, v))) = taken.join_next().await {
        hosts.insert(h.into(), v);
    }
    let hosts: Map<String, Value> = HOSTS
        .iter()
        .map(|(h, w)| {
            let mut v = hosts.remove(*h).unwrap_or_default();
            v["waarvoor"] = (*w).into();
            (h.to_string(), v)
        })
        .collect();

    if let Some(bwb) = bwb {
        let t1 = Instant::now();
        let mut o = match werker::regeling(state, bwb).await {
            Ok(r) => {
                json!({ "bwb": bwb, "ok": true, "titel": r["titel"].as_str().unwrap_or("").split('|').next().unwrap_or("").trim(),
                             "versies_met_dossier": r["versies"].as_array().into_iter().flatten().filter(|v| v["dossiers"].as_array().is_some_and(|x| !x.is_empty())).count() })
            }
            Err(e) => json!({ "bwb": bwb, "ok": false, "fout": e }),
        };
        o["s"] = ((t1.elapsed().as_secs_f64() * 10.0).round() / 10.0).into();
        d.insert("opzoeking".into(), o);
    }

    let mut faalt: Vec<String> = HOSTS
        .iter()
        .filter(|(h, w)| w.starts_with("nodig") && hosts[*h]["ok"] != true)
        .map(|(h, _)| format!("{h} niet bereikbaar"))
        .collect();
    if !schrijft {
        faalt.push(format!("{} niet schrijfbaar", data.display()));
    }
    if curl.is_none() {
        faalt.push("curl ontbreekt".into());
    }
    if !werker_ok {
        faalt.push("de werker (python3 met pyyaml) ontbreekt".into());
    }
    let oordeel = match (&faalt[..], &state.inst.uitgeweken) {
        ([], None) => "ophalen kan".to_string(),
        ([], Some(r)) => format!("ophalen kan, maar niet blijvend: {r}"),
        (f, _) => format!("ophalen faalt: {}", f.join("; ")),
    };
    d.insert("hosts".into(), hosts.into());
    d.insert("taak".into(), state.rij.huidig().unwrap_or(Value::Null));
    d.insert("rij".into(), state.rij.overzicht());
    d.insert("oordeel".into(), oordeel.clone().into());
    let s = (t0.elapsed().as_secs_f64() * 10.0).round() / 10.0;
    d.insert("s".into(), s.into());
    state
        .logboek
        .schrijf("diagnose", json!({ "oordeel": oordeel, "s": s }));
    Value::Object(d)
}

#[cfg(test)]
mod tests {
    use super::zonder_wachtwoord;

    #[test]
    fn een_proxywachtwoord_komt_niet_in_beeld_ook_niet_met_een_apenstaart() {
        assert_eq!(
            zonder_wachtwoord("http://jan:ge@heim@proxy:3128"),
            "http://***@proxy:3128"
        );
        assert_eq!(zonder_wachtwoord("http://proxy:3128"), "http://proxy:3128");
        assert_eq!(zonder_wachtwoord("jan:geheim@proxy:3128"), "***@proxy:3128");
    }
}
