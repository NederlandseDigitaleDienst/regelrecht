# -*- coding: utf-8 -*-
"""Haal een Kamerdossier binnen, ook als je alleen het BWB-id van de wet weet.

Met een BWB-id leest dit script de pagina 'informatie' van de regeling op
wetten.overheid.nl. Die zegt per versie van de wet wat er gebeurde (nieuwe
regeling, wijziging, intrekking, of een dossier dat nog aanhangig is), welk
Kamerdossier het deed, in welk Staatsblad het stond en wanneer het in werking
trad. Daaruit volgt welk dossier je wilt, en wat het begin en het eindpunt
van zijn wetsgeschiedenis zijn:

- een wijzigingswet begint bij de wet zoals zij gold op de dag dat het
  voorstel werd ingediend (de BWB-toestand van dat moment), en eindigt, als
  zij in werking is, bij de toestand die zij maakte;
- een nieuwe wet eindigt bij het Staatsblad, en na de inwerkingtreding bij
  haar eerste BWB-toestand.

Daarna haalt het de stukken op (haal-kamerstukken.py) naar
bron/kamerdossiers/<nummer>/ en schrijft het een dossier.yaml, als die er
nog niet is. Een bestaande dossier.yaml wordt nooit overschreven; het script
zegt dan wat het anders zou schrijven.

    python3 generatoren/kamerdossier.py BWBR0007376                # de dossiers van deze wet
    python3 generatoren/kamerdossier.py BWBR0007376 --dossier 36889
    python3 generatoren/kamerdossier.py BWBR0007376 --laatste      # het jongste dossier
    python3 generatoren/kamerdossier.py --dossier 35968 --staatsblad stb-2026-149
    python3 generatoren/kamerdossier.py BWBR0007376 --keten       # de hele wet door de tijd

DE KETEN. Met --keten haalt het script elke versie van de wet op die
wetten.overheid.nl kent, en per overgang tussen twee versies het Staatsblad
van de wet die haar maakte. Dat is genoeg om de hele geschiedenis af te
spelen: elke overgang is een wijzigingswet waarvan begin en eind officieel
vaststaan (keten.py). Het schrijft bron/regelingen/<BWB>/keten.json, de
versies als toestanden/<datum>.html.gz en de Staatsbladen in stb/.

De BWB-toestanden zelf komen van repository.officiele-overheidspublicaties.nl.
Is die niet bereikbaar, dan zoekt het script een momentopname onder bron/
(bron/regelingen/<BWB>/ of een bestaande bron/*-bron/), en zegt het welke
het neemt.

Voorstel M526.
"""
import html as htmlmod
import importlib.util
import json
import os
import re
import subprocess
import sys
from pathlib import Path

import yaml

HIER = Path(__file__).resolve().parent
WERKBANK = HIER.parent
# WETSGESCHIEDENIS_BRON verlegt de bron, voor de poc-dienst die in een eigen
# datamap schrijft (voor-regelrecht/poc-wetsgeschiedenis); standaard die van de werkbank.
BRON = Path(os.environ.get("WETSGESCHIEDENIS_BRON") or WERKBANK / "bron")
KAMERDOSSIERS = BRON / "kamerdossiers"
REGELINGEN = BRON / "regelingen"
WETTEN = "https://wetten.overheid.nl/"
# De informatiepagina van een grote regeling breekt soms halverwege af (curl 18,
# de Regeling zorgverzekering: 2,4 MB, gemeten 20-10-2026); --retry alleen
# herhaalt dat niet.
CURL = ["curl", "-sfL", "--http1.1", "--retry", "3", "--retry-all-errors", "-A", "Mozilla/5.0 (regelrecht-werkbank)"]
# Een regeling met honderden versies (diezelfde regeling: 718) speelt de keten
# niet helemaal af: alleen de jongste.
KETEN_MAX = int(os.environ.get("WETSGESCHIEDENIS_KETEN_MAX") or 40)
CURL_BEKENDMAKING = ["curl", "-sfL", "--http1.1", "--retry", "3", "-A", "Mozilla/5.0 (regelrecht-werkbank)"]
# Een bekendmaking: het Staatsblad (wet, besluit) of de Staatscourant
# (ministeriële regeling).
BEKENDMAKING = r"(?:stb|stcrt)-\d{4}-\d+"
BWB_REPO = "https://repository.officiele-overheidspublicaties.nl/bwb/"

spec = importlib.util.spec_from_file_location("haal_kamerstukken", HIER / "haal-kamerstukken.py")
haal_kamerstukken = importlib.util.module_from_spec(spec)
spec.loader.exec_module(haal_kamerstukken)
haal = haal_kamerstukken.haal


def toon(pad):
    """Een pad zoals het in een log hoort: vanaf de bron, waar die ook staat."""
    try:
        return str(Path(pad).relative_to(BRON.parent))
    except ValueError:
        return str(pad)


def wetten_versie(bwb, datum, log):
    """De versie van `datum` zoals wetten.overheid.nl haar toont, ingepakt
    bewaard; de HTML-lezer (wetten_html.py) is getoetst tegen de BWB-XML."""
    import gzip
    doel = REGELINGEN / bwb / "toestanden" / f"{datum}.html.gz"
    if doel.exists():
        return doel
    kop = CURL
    r = subprocess.run(kop + [f"{WETTEN}{bwb}/{datum}/0"], capture_output=True)
    ruw = r.stdout
    if r.returncode or not (b'class="wettekst"' in ruw or b'class="regeling-tekst"' in ruw):
        log.append(f"versie {datum}: geen wettekst op wetten.overheid.nl")
        return None
    doel.parent.mkdir(parents=True, exist_ok=True)
    with gzip.GzipFile(doel, "wb", mtime=0) as fh:
        fh.write(ruw)
    log.append(f"versie {datum}: opgehaald")
    return doel


def keten(reg, log):
    """De wet door de tijd: elke versie, en per overgang de dossiers en de
    Staatsbladen die haar maakten. Een rij zonder eigen datum van
    inwerkingtreding noemt die in 'Betreft'."""
    bwb = reg["bwb"]
    per_datum = {}
    for v in reg["versies"]:
        d = v["inwerking"] or iso((re.search(r"Datum inwerkingtreding: (\d{2}-\d{2}-\d{4})", v.get("betreft_vol") or "") or [None, ""])[1])
        if d:
            per_datum.setdefault(d, []).append(v)
    data = sorted(per_datum)
    afgekapt = None
    if len(data) > KETEN_MAX:
        log.append(f"keten: {len(data)} versies; alleen de jongste {KETEN_MAX} (WETSGESCHIEDENIS_KETEN_MAX)")
        afgekapt = {"versies": len(data), "sinds": data[0]}
        data = data[-KETEN_MAX:]
        per_datum = {d: per_datum[d] for d in data}
    stb_map = REGELINGEN / bwb / "stb"
    uit = {"bwb": bwb, "titel": reg["titel"], "afkorting": reg.get("afkorting"), "versies": [], "overgangen": [],
           "afgekapt": afgekapt, "ontstaan": []}
    beschikbaar = set(reg.get("beschikbaar") or data)
    for d in data:
        pad = wetten_versie(bwb, d, log) if d in beschikbaar else None
        if d not in beschikbaar:
            log.append(f"versie {d}: niet op wetten.overheid.nl (de site gaat niet zo ver terug)")
        uit["versies"].append({"datum": d, "pad": str(pad.relative_to(REGELINGEN / bwb)) if pad else None})
    for i, d in enumerate(data):
        rijen = per_datum[d]
        stbs = list(dict.fromkeys(s for r in rijen for s in (r.get("staatsbladen") or [r["staatsblad"]]) if s))
        for s in stbs:
            doel = stb_map / f"{s}.xml"
            if not doel.exists():
                # Met curl: urllib haalde de 375 Staatsbladen van de Awb via de
                # proxy in een uur binnen, curl in een halve minuut (22-10-2026).
                # Gewoon --retry herhaalt een 429, maar geen 404 (een Staatsblad
                # van vóór 1995 is er niet als XML).
                stb_map.mkdir(parents=True, exist_ok=True)
                r = subprocess.run(CURL_BEKENDMAKING + [f"https://zoek.officielebekendmakingen.nl/{s}.xml"],
                                   capture_output=True)
                if r.returncode:
                    log.append(f"{s}: NIET opgehaald (curl {r.returncode})")
                else:
                    doel.write_bytes(r.stdout)
        uit["overgangen"].append({
            "van": data[i - 1] if i else None, "tot": d,
            "betreft": sorted({r["betreft"] for r in rijen}),
            "dossiers": list(dict.fromkeys(x for r in rijen for x in r["dossiers"])),
            "staatsbladen": stbs,
            "ondertekening": min((r["ondertekening"] for r in rijen if r["ondertekening"]), default=None),
        })
    # De oudste versie zonder dossier, van vóór 1996: wetten.overheid.nl koppelt
    # pas vanaf 1995. De dossiers die haar maakten staan dan in Staten-Generaal
    # Digitaal (oude_kamerstukken.py); alleen als de hele geschiedenis er is,
    # want anders is de oudste overgang een latere.
    eerste = uit["overgangen"][0] if uit["overgangen"] else None
    if eerste and not afgekapt and not eerste["dossiers"] and eerste["tot"] < "1996":
        sys.path.insert(0, str(HIER))
        import oude_kamerstukken
        try:
            uit["ontstaan"] = oude_kamerstukken.ontstaan(reg["titel"].split("|")[0].strip(), eerste["tot"])
            eerste["dossiers"] = list(uit["ontstaan"])
            log.append(f"ontstaan: {', '.join(uit['ontstaan']) or 'geen dossier gevonden'} (Staten-Generaal Digitaal)")
        except Exception as e:  # noqa: BLE001 -- het falen hoort in het log
            log.append(f"ontstaan: NIET gezocht ({e})")
    uit["aanhangig"] = [v for v in reg["versies"] if v["betreft"] == "Aanhangig"]
    uit["ingetrokken"] = [v for v in reg["versies"] if v["betreft"].startswith("Intrekking")]
    (REGELINGEN / bwb / "keten.json").write_text(json.dumps(uit, ensure_ascii=False, indent=1), encoding="utf-8")
    log.append(f"keten: {len(data)} versies, {sum(1 for o in uit['overgangen'] if o['van'])} overgangen")
    return uit


def iso(d):
    m = re.match(r"(\d{2})-(\d{2})-(\d{4})", d or "")
    return f"{m[3]}-{m[2]}-{m[1]}" if m else None


def schoon(h):
    return " ".join(htmlmod.unescape(re.sub(r"<[^>]+>", " ", h)).split())


def wetten(bwb):
    """De informatiepagina van wetten.overheid.nl. Zonder datum stuurt de site
    door naar de geldende versie; de informatie hangt onder die versie. Zonder
    browserkenmerk antwoordt zij niet (gemeten 19-10-2026)."""
    kop = CURL
    r = subprocess.run(kop + ["-o", os.devnull, "-w", "%{url_effective}", f"{WETTEN}{bwb}/"],
                       check=True, capture_output=True, text=True)
    basis = r.stdout.strip().rstrip("/")
    if not re.search(r"/\d{4}-\d{2}-\d{2}(?:/\d+)?$", basis):
        raise SystemExit(f"wetten.overheid.nl kent {bwb} niet (doorverwezen naar {basis})")
    if not re.search(r"/\d+$", basis):
        basis += "/0"
    return subprocess.run(kop + [f"{basis}/informatie"], check=True, capture_output=True).stdout


def regeling(bwb):
    """De informatiepagina van een regeling, als {titel, afkorting, versies}."""
    x = wetten(bwb).decode("utf-8")
    titel = schoon(re.search(r"<h1[^>]*>(.*?)</h1>", x, re.S)[1])
    afk = re.search(r'data-before="Afkorting"[^>]*>(.*?)</td>', x, re.S)
    versies = []
    for r in re.findall(r"<tr[^>]*>(.*?)</tr>", x, re.S):
        cellen = [(k, schoon(v)) for k, v in re.findall(r'<td data-before="([^"]+)"[^>]*>(.*?)</td>', r, re.S)]
        betreft = next((v for k, v in cellen if k == "Betreft"), None)
        if not betreft:
            continue
        stb = list(dict.fromkeys(re.findall(BEKENDMAKING, r)))
        # De eerste cel 'Bekendmaking' noemt de wet(ten), ook 'Stb. 2021, 500 samen
        # met Stb. 2021, 499'; de tweede het besluit dat haar in werking zet.
        cel = re.findall(r'<td data-before="Bekendmaking"[^>]*>(.*?)</td>', r, re.S)
        wetten_stb = list(dict.fromkeys(re.findall(BEKENDMAKING, cel[0]))) if cel else stb[:1]
        art = re.search(r"Artikelen: ([^<]+)", r)
        versies.append({
            "inwerking": iso(next((v for k, v in cellen if k == "Datum van inwerkingtreding"), "")),
            "betreft": betreft.split()[0],
            "betreft_vol": betreft,
            "ondertekening": iso(next((v for k, v in cellen if k == "Ondertekening"), "")),
            # De eerste bekendmaking is de wet zelf; de tweede het besluit dat
            # haar in werking zet.
            "staatsblad": stb[0] if stb else None,
            "staatsbladen": wetten_stb,
            "inwerkingtredingsbesluit": stb[1] if len(stb) > 1 and stb[1] != stb[0] else None,
            "dossiers": list(dict.fromkeys(re.findall(r"dossier/(\d+)", r))),
            "artikelen": [a.strip() for a in art[1].split(",")] if art else [],
        })
    # De versies die wetten.overheid.nl zelf heeft (de knop 'vergelijk'); oudere
    # staan in de tabel maar niet als tekst.
    beschikbaar = sorted(set(re.findall(r'value="(\d{4}-\d{2}-\d{2})" class="wti-compare', x)))
    # Zonder afkorting zet de site er 'Geen' (BWBR0029404, gemeten 20-10-2026).
    afk = schoon(afk[1]) if afk else None
    return {"bwb": bwb, "titel": titel, "afkorting": afk if afk and afk != "Geen" else None, "versies": versies,
            "beschikbaar": beschikbaar}


def toestand(bwb, datum, log):
    """De BWB-toestand die op `datum` gold: van de repository, of anders een
    momentopname onder bron/. Geeft het pad, of None."""
    doel = REGELINGEN / bwb
    for kandidaat in sorted(list(doel.glob(f"{bwb}_*.xml")) + list(BRON.glob(f"*-bron/{bwb}_*.xml"))):
        # De datum in de bestandsnaam telt niet; die in het bestand wel. Zo heette
        # bron/archiefwet-bron/BWBR0007376_2024-06-19.xml naar 2024, maar is het de
        # toestand van 2001-09-01 (gemeten 19-10-2026).
        kop = kandidaat.read_text(encoding="utf-8", errors="replace")[:2000]
        echt = re.search(r'<toestand[^>]*\binwerkingtreding="(\d{4}-\d{2}-\d{2})"', kop)
        if echt and echt[1] == datum:
            log.append(f"BWB-toestand {bwb} van {datum}: {toon(kandidaat)}")
            return kandidaat
        if echt and datum in kandidaat.name and echt[1] != datum:
            log.append(f"LET OP: {toon(kandidaat)} heet naar {datum} maar is de toestand van {echt[1]}")
    html = doel / "toestanden" / f"{datum}.html.gz"
    if html.exists() or wetten_versie(bwb, datum, log):
        log.append(f"BWB-toestand {bwb} van {datum}: de versie van wetten.overheid.nl")
        return html
    try:
        x = haal(f"{BWB_REPO}{bwb}/{datum}_0/xml/{bwb}_{datum}_0.xml")
        doel.mkdir(parents=True, exist_ok=True)
        pad = doel / f"{bwb}_{datum}_0.xml"
        pad.write_bytes(x)
        log.append(f"BWB-toestand {bwb} van {datum}: opgehaald")
        return pad
    except Exception as e:  # noqa: BLE001 -- het falen hoort in het log
        log.append(f"BWB-toestand {bwb} van {datum}: NIET opgehaald ({e}); "
                   f"zet hem in {toon(doel)}/ of voeg de host toe aan de netwerklijst")
        return None


def geldend_op(versies, datum):
    """De datum van de versie die op `datum` gold: de laatste inwerkingtreding
    op of vóór die dag."""
    data = sorted({v["inwerking"] for v in versies if v["inwerking"] and v["inwerking"] <= (datum or "9999")})
    return data[-1] if data else None


def relatief(pad, map_):
    return os.path.relpath(pad, map_) if pad else None


def dossier_yaml(nr, map_, reg=None, versie=None, log=None):
    """De dossier.yaml die uit de regeling en de stukken volgt."""
    stukken = json.loads((map_ / "stukken.json").read_text(encoding="utf-8"))
    voorstel = next((n for n, xs in stukken.items() if xs[0]["soort"] == "Voorstel van wet"), "2")
    indiening = (stukken.get(voorstel, [{}])[0].get("datum") or "")[:10] or None
    titel = (stukken.get(voorstel, [{}])[0].get("titel") or "").split(";")[0].strip()
    cfg = {"naam": titel or nr, "dossier": nr, "voorstel": voorstel}
    wijziging = bool(re.match(r"Wijziging van", titel)) or (versie or {}).get("betreft") in ("Wijziging", "Aanhangig")
    cfg["soort"] = "wijzigingswet" if wijziging else "nieuwe wet"
    cfg["deze_wet"] = [] if wijziging else [x for x in (reg or {}).get("titel", "").split("|") if x]
    ijk = []
    stb = (versie or {}).get("staatsblad")
    if stb and (map_ / f"{stb}.xml").exists() and not wijziging:
        ijk.append({"naam": "Staatsblad " + stb[4:].replace("-", ", ", 1).replace("-", ""),
                    "soort": "staatsblad", "pad": f"{stb}.xml", "datum": (versie or {}).get("ondertekening"),
                    "uitleg": "de wet zoals zij is bekendgemaakt"})
    if reg and not wijziging and versie and versie.get("inwerking"):
        pad = toestand(reg["bwb"], versie["inwerking"], log)
        if pad:
            ijk.append({"naam": f"BWB {reg['bwb']}, {versie['inwerking']}", "soort": "bwb",
                        "pad": relatief(pad, map_), "datum": versie["inwerking"],
                        "uitleg": "de geldende tekst op wetten.overheid.nl, na de inwerkingtreding"})
    if ijk:
        cfg["ijkpunten"] = ijk
    if reg and wijziging:
        basisdatum = geldend_op(reg["versies"], indiening)
        basis = toestand(reg["bwb"], basisdatum, log) if basisdatum else None
        doel = {"titel": reg["titel"], "bwb": reg["bwb"], "bron": relatief(basis, map_),
                "basisnaam": f"de geldende tekst bij indiening (BWB, toestand {basisdatum})"}
        if versie and versie.get("inwerking"):
            na = toestand(reg["bwb"], versie["inwerking"], log)
            if na:
                doel["na"] = {"bron": relatief(na, map_), "naam": f"BWB, toestand {versie['inwerking']}"}
        cfg["doelwetten"] = [doel] if basis else []
    return cfg


def main(args):
    bwb = next((a for a in args if re.fullmatch(r"BWBR\d{7}", a)), None)
    nr = args[args.index("--dossier") + 1] if "--dossier" in args else None
    stb = args[args.index("--staatsblad") + 1] if "--staatsblad" in args else None
    log, reg, versie = [], None, None
    if bwb:
        reg = regeling(bwb)
        REGELINGEN.joinpath(bwb).mkdir(parents=True, exist_ok=True)
        (REGELINGEN / bwb / "informatie.json").write_text(json.dumps(reg, ensure_ascii=False, indent=1), encoding="utf-8")
        print(f"{reg['titel']} ({bwb}{', ' + reg['afkorting'] if reg['afkorting'] else ''}): "
              f"{len(reg['versies'])} gebeurtenissen")
        for v in reg["versies"]:
            if v["dossiers"]:
                print(f"  {', '.join(v['dossiers']):14s} {v['betreft']:18s} "
                      f"{('in werking ' + v['inwerking']) if v['inwerking'] else '':22s} {v['staatsblad'] or ''}"
                      + (f"  artikelen {', '.join(v['artikelen'][:8])}{' …' if len(v['artikelen']) > 8 else ''}"
                         if v["artikelen"] else ""))
        if "--keten" in args:
            keten(reg, log)
            print("\n".join(log[-8:]))
            print("daarna: python3 generatoren/bouw-wetsgeschiedenis.py")
            return
        if "--laatste" in args:
            nr = next((d for v in reg["versies"] for d in v["dossiers"]), None)
        if not nr:
            print("kies er een met --dossier <nummer>, of --laatste")
            return
        versie = next((v for v in reg["versies"] if nr in v["dossiers"]), None)
        if versie is None:
            sys.exit(f"dossier {nr} staat niet in de geschiedenis van {bwb}")
        stb = stb or versie["staatsblad"]
    if not nr:
        sys.exit(__doc__)
    map_ = KAMERDOSSIERS / nr
    print(f"== dossier {nr} -> {toon(map_)}")
    haal_kamerstukken.main(nr, map_, stb)
    cfg = dossier_yaml(nr, map_, reg, versie, log)
    pad = map_ / "dossier.yaml"
    tekst = ("# Wat dit Kamerdossier is, voor de wetsgeschiedenis; geschreven door\n"
             "# kamerdossier.py en daarna met de hand bij te werken. Zie ../LEESMIJ.md.\n"
             + yaml.safe_dump(cfg, allow_unicode=True, sort_keys=False, width=100))
    if pad.exists():
        if pad.read_text(encoding="utf-8") != tekst:
            (map_ / "dossier.voorstel.yaml").write_text(tekst, encoding="utf-8")
            log.append("dossier.yaml bestaat al en blijft staan; wat dit script zou schrijven staat in "
                       "dossier.voorstel.yaml")
    else:
        pad.write_text(tekst, encoding="utf-8")
        log.append("dossier.yaml geschreven")
    print("\n".join(log))
    print("daarna: python3 generatoren/bouw-wetsgeschiedenis.py")


if __name__ == "__main__":
    main(sys.argv[1:])
