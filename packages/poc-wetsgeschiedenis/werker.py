# -*- coding: utf-8 -*-
"""De werker van de wetsgeschiedenis: één opdracht van de afspeelketen, als
eigen proces. De dienst (src/, Rust) start hem per opdracht en leest wat hij
zegt.

    python3 werker.py regeling BWBR0007376 --uit regeling.json
    python3 werker.py keten    BWBR0007376 --uit t.json --gegevens a:b --status s.json [--alles]
    python3 werker.py dossier  BWBR0007376 36889 --uit t.json
    python3 werker.py opbouw   BWBR0007376 --uit t.json --gegevens a:b --status s.json

regeling  wat wetten.overheid.nl over de regeling weet (de versies en hun dossiers)
keten     de versies en bekendmakingen ophalen en de wet door de tijd opbouwen;
          zonder --alles alleen de jongste (WETSGESCHIEDENIS_KETEN_MAX)
dossier   een Kamerdossier ophalen en afspelen; een dossier van vóór 1995
          (alleen een scan in Staten-Generaal Digitaal) wordt gelezen, niet
          afgespeeld
opbouw    de wet door de tijd opnieuw opbouwen uit wat er al is (na een dossier)

De uitkomst gaat als JSON naar --uit; de rest is log. Stdout en stderr zijn
gewone regels voor het log van de opdracht, behalve twee soorten op stderr:

  @uit {...}   een vraag naar buiten (host, pad, duur, en bij een fout de reden)
  @fout tekst  waarom de opdracht mislukte; daarna is de afsluitcode 1

--gegevens noemt de mappen met tijdlijnen (de opgehaalde eerst), --status een
JSON met per dossier waarom het er (nog) niet is. Die twee bepalen welke
toelichting de keten aan haar overgangen hangt.

De afspeelketen zelf staat in afspeler/ (een export van werkbank/generatoren
in het dossier waar zij is ontwikkeld). Alleen de standaardbibliotheek, pyyaml
en curl.
"""
import importlib.util
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from urllib.parse import urlsplit

HIER = Path(__file__).resolve().parent
AFSPELER = HIER / "afspeler"
VERBINDEN = str(int(os.environ.get("WG_CONNECT_TIMEOUT") or 15))

# Wat curl met een afsluitcode bedoelt; de meeste fouten in een nieuwe omgeving
# zijn er een van deze.
CURL_FOUT = {
    2: "curl kon niet starten",
    5: "de proxy is niet te vinden",
    6: "de host is niet te vinden (DNS)",
    7: "geen verbinding met de host (uitgaand verkeer dicht?)",
    18: "de overdracht brak halverwege af (een grote pagina; opnieuw proberen helpt meestal)",
    22: "de server antwoordde met een HTTP-fout",
    28: "geen antwoord binnen de tijd (uitgaand verkeer dicht, of de host is traag)",
    35: "de TLS-verbinding mislukte",
    47: "te vaak doorgestuurd",
    52: "de server gaf een leeg antwoord",
    56: "de verbinding brak af",
    60: "het certificaat van de host wordt niet vertrouwd",
}


# ---------------------------------------------------------------- vragen naar buiten
def meld(soort, **velden):
    sys.stderr.write(f"@{soort} " + json.dumps(velden, ensure_ascii=False) + "\n")
    sys.stderr.flush()


def _kort(url):
    u = urlsplit(url)
    pad = u.path + ("?" + u.query if u.query else "")
    return u.netloc, (pad[:160] + "…") if len(pad) > 160 else pad


def _stderr(b):
    t = b.decode("utf-8", "replace") if isinstance(b, bytes) else (b or "")
    return " ".join(t.split())[:300] or None


def _uit(via, url, t0, **velden):
    host, pad = _kort(url)
    velden = {k: v for k, v in velden.items() if v is not None}
    meld("uit", via=via, host=host, pad=pad, ms=int((time.monotonic() - t0) * 1000), **velden)


_run = subprocess.run


def _curl(cmd, *args, **kw):
    """subprocess.run, maar een curl-aanroep wordt gemeten en krijgt -S (zodat
    een fout haar reden noemt) en een verbindingstijd (zodat een host die niet
    antwoordt na seconden faalt in plaats van na minuten)."""
    if not (isinstance(cmd, (list, tuple)) and cmd and cmd[0] == "curl"):
        return _run(cmd, *args, **kw)
    cmd = ["curl", "-S", "--connect-timeout", VERBINDEN, *cmd[1:]]
    url = next((x for x in reversed(cmd) if str(x).startswith("http")), "?")
    t0 = time.monotonic()
    try:
        r = _run(cmd, *args, **kw)
    except subprocess.CalledProcessError as e:
        _uit("curl", url, t0, code=e.returncode, uitleg=CURL_FOUT.get(e.returncode), fout=_stderr(e.stderr))
        raise
    _uit("curl", url, t0, code=r.returncode or None, uitleg=CURL_FOUT.get(r.returncode),
         fout=_stderr(r.stderr) if r.returncode else None, bytes=len(r.stdout or b"") if not r.returncode else None)
    return r


_urlopen = urllib.request.urlopen


def _open(req, *args, **kw):
    url = getattr(req, "full_url", None) or str(req)
    t0 = time.monotonic()
    try:
        r = _urlopen(req, *args, **kw)
    except urllib.error.HTTPError as e:
        _uit("urllib", url, t0, http=e.code)
        raise
    except Exception as e:
        _uit("urllib", url, t0, fout=f"{type(e).__name__}: {e}"[:300])
        raise
    _uit("urllib", url, t0, http=getattr(r, "status", None))
    return r


def uitleg(e):
    """Een fout in woorden die een bezoeker kan doorgeven."""
    if isinstance(e, subprocess.CalledProcessError) and e.cmd and e.cmd[0] == "curl":
        url = next((x for x in reversed(e.cmd) if str(x).startswith("http")), "?")
        detail = _stderr(e.stderr)
        return (f"{_kort(url)[0]}: {CURL_FOUT.get(e.returncode, 'onbekende fout')} "
                f"(curl {e.returncode}{': ' + detail if detail else ''})")
    if isinstance(e, urllib.error.HTTPError):
        return f"{_kort(e.url or '?')[0]} antwoordde met HTTP {e.code}"
    return f"{type(e).__name__}: {e}"


# ---------------------------------------------------------------- de keten laden
def _laad(naam, bestand):
    spec = importlib.util.spec_from_file_location(naam, AFSPELER / bestand)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


def keten_modules():
    subprocess.run = _curl
    urllib.request.urlopen = _open
    sys.path.insert(0, str(AFSPELER))
    kamerdossier = _laad("kamerdossier", "kamerdossier.py")
    bouw = _laad("bouw_wetsgeschiedenis", "bouw-wetsgeschiedenis.py")
    import keten
    return kamerdossier, bouw, keten


def oude_kamerstukken():
    import oude_kamerstukken
    return oude_kamerstukken


def tijdlijn_van(mappen):
    def van(nr):
        # Het nummer komt uit een antwoord van buiten (wetten.overheid.nl, SRU);
        # alleen een dossiernummer wordt een bestandsnaam.
        if not re.fullmatch(r"\d{5}(?:-[A-Z0-9]{1,6})?", str(nr)):
            return None
        for m in mappen:
            p = Path(m) / f"{nr}.json"
            if p.is_file():
                try:
                    return json.loads(p.read_text(encoding="utf-8"))
                except ValueError:
                    return None
        return None
    return van


def status_van(pad):
    status = json.loads(Path(pad).read_text(encoding="utf-8")) if pad and Path(pad).is_file() else {}
    return lambda nr: status.get(nr) or {"status": "niet opgehaald"}


def optie(args, naam, anders=None):
    return args[args.index(naam) + 1] if naam in args else anders


def schrijf(pad, x):
    tmp = Path(str(pad) + ".tmp")
    tmp.write_text(json.dumps(x, ensure_ascii=False, separators=(",", ":")), encoding="utf-8")
    tmp.replace(pad)


def main(args):
    wat, uit = args[0], optie(args, "--uit")
    mappen = [m for m in (optie(args, "--gegevens") or "").split(":") if m]
    if "--alles" in args:
        # kamerdossier leest de grens bij het laden.
        os.environ["WETSGESCHIEDENIS_KETEN_MAX"] = "100000"
    kamerdossier, bouw, keten = keten_modules()
    if wat == "regeling":
        schrijf(uit, kamerdossier.regeling(args[1]))
    elif wat == "keten":
        print(f"{'Alle' if '--alles' in args else 'De jongste'} versies van {args[1]} en de bekendmakingen ertussen ophalen",
              flush=True)
        kamerdossier.main([args[1], "--keten"])
        print("Elke overgang afspelen, met de toelichting die er al is", flush=True)
        schrijf(uit, keten.bouw(args[1], tijdlijn_van=tijdlijn_van(mappen), status_van=status_van(optie(args, "--status"))))
    elif wat == "opbouw":
        schrijf(uit, keten.bouw(args[1], tijdlijn_van=tijdlijn_van(mappen), status_van=status_van(optie(args, "--status"))))
    elif wat == "dossier" and oude_kamerstukken().is_oud(args[2]):
        print(f"Dossier {args[2]} is van vóór 1995: de scans uit Staten-Generaal Digitaal lezen", flush=True)
        log = []
        t = oude_kamerstukken().tijdlijn(args[1], args[2], kamerdossier.REGELINGEN, kamerdossier.KAMERDOSSIERS, log)
        print("\n".join(log), flush=True)
        schrijf(uit, t)
    elif wat == "dossier":
        print(f"Dossier {args[2]} ophalen", flush=True)
        kamerdossier.main([args[1], "--dossier", args[2]])
        print(f"Dossier {args[2]} afspelen", flush=True)
        t, _ = bouw.tijdlijn(args[2])
        schrijf(uit, t)
    else:
        raise SystemExit(f"onbekende opdracht: {wat}")


if __name__ == "__main__":
    try:
        main(sys.argv[1:])
    except SystemExit as e:
        if e.code not in (None, 0):
            meld("fout", tekst=str(e.code))
            sys.exit(1)
    except Exception as e:  # noqa: BLE001 -- de dienst hoort wat er misging
        import traceback
        traceback.print_exc()
        meld("fout", tekst=uitleg(e))
        sys.exit(1)
