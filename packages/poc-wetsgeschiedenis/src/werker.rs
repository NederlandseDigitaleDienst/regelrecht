//! The worker: `werker.py`, one process per task. Its stdout and stderr are
//! lines for the task's log, except two kinds on stderr: `@uit {...}` (a call
//! to the outside, for the log book) and `@fout {"tekst": ...}` (why it
//! failed). The result goes to the file named by `--uit`.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::AppState;

/// What the worker says, line by line.
pub enum Regel {
    Log(String),
    Uit(Value),
}

/// Run the worker with `args`; `regel` hears every line. Fails with the
/// worker's own reason, or with how it stopped.
pub async fn draai(
    state: &AppState,
    args: &[&str],
    tijd: Option<Duration>,
    mut regel: impl FnMut(Regel),
) -> Result<(), String> {
    let o = &state.opslag;
    let mut kind = Command::new(&state.inst.python)
        .arg(&state.inst.werker)
        .args(args)
        .env("WETSGESCHIEDENIS_BRON", o.bron())
        .env("WETSGESCHIEDENIS_WERK", o.werk())
        .env("WG_CONNECT_TIMEOUT", state.inst.verbinden.to_string())
        .env("PYTHONUNBUFFERED", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("de werker start niet ({}: {e})", state.inst.python))?;

    let (tx, mut rx) = mpsc::unbounded_channel::<(bool, String)>();
    if let Some(o) = kind.stdout.take() {
        tokio::spawn(stuur(o, false, tx.clone()));
    }
    if let Some(e) = kind.stderr.take() {
        tokio::spawn(stuur(e, true, tx.clone()));
    }
    drop(tx);

    let mut reden = None;
    let lees = async {
        while let Some((stderr, r)) = rx.recv().await {
            let opdracht = if stderr { r.split_once(' ') } else { None };
            match opdracht {
                Some(("@uit", v)) => {
                    regel(Regel::Uit(serde_json::from_str(v).unwrap_or(Value::Null)))
                }
                Some(("@fout", v)) => {
                    reden = serde_json::from_str::<Value>(v)
                        .ok()
                        .and_then(|x| x["tekst"].as_str().map(String::from))
                }
                _ => regel(Regel::Log(r)),
            }
        }
        kind.wait().await
    };
    let status = match tijd {
        Some(t) => tokio::time::timeout(t, lees)
            .await
            .map_err(|_| format!("de werker antwoordde niet binnen {} s", t.as_secs()))?,
        None => lees.await,
    }
    .map_err(|e| format!("de werker: {e}"))?;
    match (status.success(), reden) {
        (true, _) => Ok(()),
        (false, Some(r)) => Err(r),
        (false, None) => Err(format!("de werker stopte ({status})")),
    }
}

/// Every line of one stream into the channel, marked stderr or not.
async fn stuur<R: tokio::io::AsyncRead + Unpin>(
    r: R,
    stderr: bool,
    tx: mpsc::UnboundedSender<(bool, String)>,
) {
    let mut regels = BufReader::new(r).lines();
    while let Ok(Some(r)) = regels.next_line().await {
        if tx.send((stderr, r)).is_err() {
            break;
        }
    }
}

/// The `@uit` line of a call that failed, as the visitor reads it in the task log.
pub fn mislukte_vraag(v: &Value) -> Option<String> {
    let fout = v["uitleg"]
        .as_str()
        .or(v["fout"].as_str())
        .map(String::from)
        .or_else(|| {
            v["http"]
                .as_u64()
                .filter(|h| *h >= 400)
                .map(|h| format!("HTTP {h}"))
        })?;
    let pad: String = v["pad"].as_str().unwrap_or("").chars().take(80).collect();
    Some(format!(
        "  ! {}{}: {} ({} ms)",
        v["host"].as_str().unwrap_or("?"),
        pad,
        fout,
        v["ms"]
    ))
}

/// What wetten.overheid.nl knows about a regulation (the worker's `regeling`).
pub async fn regeling(state: &AppState, bwb: &str) -> Result<Value, String> {
    static TELLER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = TELLER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let uit = state
        .opslag
        .tijdelijk()
        .join(format!("regeling-{bwb}-{n}.json"));
    let _ = std::fs::create_dir_all(state.opslag.tijdelijk());
    let logboek = state.logboek.clone();
    let r = draai(
        state,
        &["regeling", bwb, "--uit", pad(&uit)],
        Some(Duration::from_secs(120)),
        |r| {
            if let Regel::Uit(v) = r {
                logboek.schrijf("uit", v);
            }
        },
    )
    .await;
    let antwoord = r.and_then(|_| {
        serde_json::from_slice(&std::fs::read(&uit).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    });
    let _ = std::fs::remove_file(&uit);
    antwoord
}

pub fn pad(p: &Path) -> &str {
    p.to_str().unwrap_or_default()
}
