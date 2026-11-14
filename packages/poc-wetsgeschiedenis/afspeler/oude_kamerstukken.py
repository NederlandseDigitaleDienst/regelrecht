# -*- coding: utf-8 -*-
"""De Kamerstukken van vóór 1995, uit Staten-Generaal Digitaal.

officielebekendmakingen.nl heeft de Kamerstukken van vóór 1995 overgenomen
uit Staten-Generaal Digitaal: niet als XML, zoals de latere, maar als scan met
OCR, één bestand per pagina. Het hele dossier van de Algemene wet
bestuursrecht staat er zo (21221: de memorie van toelichting van 1989, 163
pagina's; gemeten 22-10-2026). wetten.overheid.nl koppelt zulke dossiers niet
aan de versies van een wet: haar koppeling begint in 1995. Dit script vindt
ze, leest ze en hangt hun toelichting aan de artikelen.

DRIE DINGEN ZIJN HIER ANDERS dan bij een dossier met XML (kamertoelichting.py):

- De tekst is OCR. Elke <p> is een alinea of een regel; kop- en voetregels van
  de pagina vallen weg, en een alinea die over een pagina doorloopt wordt weer
  heel. De koppen staan als losse regel ('Artikel 3.2.2'), soms aan de tekst
  vast ('Artikel2.1.1 1. Een ieder ...').
- Het voorstel nummerde anders dan de wet: 'artikel 3.2.2' (hoofdstuk,
  afdeling, artikel) werd bij de bekendmaking '3:3'. En het voorstel
  vernummerde onderweg: de memorie van toelichting van 1989 telt in de
  nummering van het voorstel van 1989, de memorie van antwoord van 1990 in die
  van 1990. Per versie van het voorstel maakt dit script daarom een
  vertaaltabel: elk artikel van het voorstel gaat naar het artikel van de
  oudste tekst op wetten.overheid.nl dat er het meest op lijkt (woordparen,
  Jaccard), eerst binnen hetzelfde hoofdstuk, dan daarbuiten met een
  strengere drempel. Een stuk leest in de tabel van het laatste voorstel van
  vóór zijn datum. Wat niet koppelt, telt als niet gehaakt, met die reden.
- Er wordt niets afgespeeld. Wat dit script levert is toelichting, geen
  tekst: de tekst van de wet zelf begint op wetten.overheid.nl in 2002.

Een dossier is 'oud' als officielebekendmakingen.nl het alleen uit
Staten-Generaal Digitaal kent. De server weert een vlaag verzoeken (429 bij
acht tegelijk); de pagina's komen daarom na elkaar, met een korte pauze.

    python3 generatoren/oude_kamerstukken.py BWBR0005537 21221

Voorstel M526.
"""
import json
import re
import subprocess
import sys
import time
import urllib.parse
from pathlib import Path

HIER = Path(__file__).resolve().parent
sys.path.insert(0, str(HIER))
import kamertoelichting as K  # noqa: E402

SRU = "https://repository.overheid.nl/sru"
PAUZE = 0.25          # tussen twee pagina's
DREMPEL = 0.2         # gelijkenis binnen hetzelfde hoofdstuk
DREMPEL_ELDERS = 0.4  # gelijkenis met een artikel in een ander hoofdstuk
BONUS = 0.15          # voorsprong van het eigen hoofdstuk bij het kiezen

# Welke stukken toelichting dragen, naar hun omschrijving (hoofdletters, zoals
# Staten-Generaal Digitaal ze geeft); de eerste die past telt. Het advies van
# de Raad van State telt in de nummering van de adviesversie
# (kamertoelichting.py) en wordt niet gelezen.
SOORTEN = (
    ("OORSPRONKELIJKE TEKST", None),
    ("ADVIES", None),
    ("MEMORIE VAN TOELICHTING", "memorie"),
    ("MEMORIE VAN ANTWOORD", "antwoord"),
    ("NADERE MEMORIE VAN ANTWOORD", "antwoord"),
    ("NOTA NAAR AANLEIDING", "antwoord"),
    ("VERSLAG", "vraag"),
    ("VOORLOPIG VERSLAG", "vraag"),
    ("NADER VOORLOPIG VERSLAG", "vraag"),
    ("EINDVERSLAG", "vraag"),
    ("BRIEF", "brief"),
)
# Het voorstel in zijn versies; niet de oorspronkelijke tekst die de Raad van
# State zag (die telt in de nummering van de adviesversie).
VOORSTEL = re.compile(r"^(?:(?:NADER )?GEWIJZIGD )?VOORSTEL VAN WET")

# Wat geen alinea is: de kop- en voetregels van een pagina.
RUIS = re.compile(
    r"^(?:\S?(?:weede|erste) Kamer(?:,| der)|Vergaderjaar \d{4}|Nr\.\s*\w+$|\d{1,3}$|\d{2} \d{3}$|"
    r".*\bISSN\b|.*Gravenhage \d{4}|.*Sdu Uitgeverij)")
# 'Artikel2.1.1' en 'Artikel 3.2.2': een kop, eventueel met tekst erachter.
ARTKOP = re.compile(r"^Artikel(?:en)?\s*(?P<nr>\d+(?:\.\d+)*[a-z]?)(?:\s*$|\s+(?P<rest>(?:\d+\.\s|[A-Z(]).*))")
SECTIE = re.compile(r"^(?:HOOFDSTUK|Hoofdstuk|AFDELING|Afdeling|TITEL|Titel|§|Paragraaf|[IVX]+\.?\s+[A-Z])\s*\S")


# ---------------------------------------------------------------- ophalen
def _haal(url):
    """Een antwoord, met curl: urllib krijgt een groot antwoord via de proxy
    niet altijd heel binnen (IncompleteRead, gemeten 22-10-2026), en
    --retry-all-errors wacht ook een 429 af."""
    r = subprocess.run(["curl", "-sfL", "--http1.1", "--retry", "5", "--retry-delay", "2", "--retry-all-errors",
                        "-A", "regelrecht-werkbank", url], capture_output=True, check=True)
    return r.stdout


def _sru(query, maximum=1000):
    q = urllib.parse.urlencode({"operation": "searchRetrieve", "version": "2.0",
                                "maximumRecords": str(maximum), "query": query})
    return _haal(f"{SRU}?{q}").decode("utf-8")


def _records(x):
    for r in re.findall(r"<sru:record>(.*?)</sru:record>", x, re.S):
        g = lambda t: (re.search(rf"<{t}[^>]*>([^<]*)</{t}>", r) or [None, ""])[1].strip()  # noqa: E731
        yield r, g


def _volgorde(ond):
    """'3' < '10' < '174' < '174a'; letters ('A', 'B') vooraan."""
    m = re.match(r"(\d+)(.*)", ond or "")
    return (1, int(m[1]), m[2]) if m else (0, 0, ond or "")


def stukken(nr):
    """De stukken van dossier `nr` uit Staten-Generaal Digitaal, in volgorde,
    met hun OCR-pagina's. Een stuk zonder datum krijgt die van het stuk ervoor
    (het voorstel en de memorie dragen vaak geen eigen datum)."""
    uit = []
    for r, g in _records(_sru(f"w.dossiernummer=={nr}")):
        if "<c:product-area>sgd</c:product-area>" not in r:
            continue
        omschrijving = re.findall(r"<dcterms:description>([^<]*)</dcterms:description>", r)
        uit.append({"id": g("dcterms:identifier"), "ondernummer": g("overheidwetgeving:ondernummer"),
                    "omschrijving": " ".join((omschrijving[0] if omschrijving else "").split()),
                    "titel": g("dcterms:title"), "datum": g("dcterms:temporal") or None,
                    "url": g("gzd:preferredUrl"), "pdf": g("gzd:url"),
                    # Alleen pagina's van de repository zelf; een ander adres in het
                    # antwoord volgt curl niet.
                    "ocr": [u for u in re.findall(r'manifestation="ocr">([^<]+)<', r)
                            if u.startswith("https://repository.overheid.nl/")]})
    uit.sort(key=lambda s: _volgorde(s["ondernummer"]))
    vorige = next((s["datum"] for s in uit if s["datum"]), None)
    for s in uit:
        s["datum"] = s["datum"] or vorige
        vorige = s["datum"]
    return uit


def is_oud(nr):
    """Kent officielebekendmakingen.nl dit dossier alleen uit Staten-Generaal
    Digitaal?"""
    x = _sru(f"w.dossiernummer=={nr}", 50)
    soorten = re.findall(r"<c:product-area>(\w+)</c:product-area>", x)
    return bool(soorten) and set(soorten) == {"sgd"}


def ontstaan(titel, voor):
    """De dossiers van vóór 1995 die deze wet maakten of wijzigden voordat zij
    in werking trad: hun titel noemt haar, hun laatste stuk is van vóór `voor`
    (de datum van de oudste versie), en ze passen niet andere wetten aan haar
    aan ('Aanpassing van een aantal wetten aan de ...')."""
    per = {}
    for r, g in _records(_sru(f'c.product-area==sgd and dt.title="{titel}"')):
        nr = g("overheidwetgeving:dossiernummer")
        t = g("dcterms:title")
        if not re.fullmatch(r"\d{5}", nr) or re.search(r"\baanpassing\b.*\baan de\b", t, re.I) \
                or titel.lower() not in t.lower():
            continue
        d = g("dcterms:temporal")
        per[nr] = max(per.get(nr, ""), d)
    return sorted(nr for nr, d in per.items() if d and d < voor)


def haal(nr, map_, log):
    """De stukken en hun tekst, in `map_`: stukken.json en per stuk
    <ondernummer>.json (de pagina's zoals de OCR ze geeft; het opschonen
    gebeurt bij het lezen, zodat een verbetering ook geldt voor wat er al
    staat). Wat er al staat, blijft."""
    map_.mkdir(parents=True, exist_ok=True)
    lijst = stukken(nr)
    (map_ / "stukken.json").write_text(json.dumps(lijst, ensure_ascii=False, indent=1), encoding="utf-8")
    for s in lijst:
        if not (soort_van(s["omschrijving"]) or VOORSTEL.search(s["omschrijving"])):
            continue
        doel = map_ / f"{_bestand(s)}.json"
        if doel.exists():
            continue
        paginas = []
        for u in s["ocr"]:
            x = _haal(u).decode("utf-8", "replace")
            paginas.append([_schoon(p) for p in re.findall(r"<p>(.*?)</p>", x, re.S)])
            time.sleep(PAUZE)
        doel.write_text(json.dumps(paginas, ensure_ascii=False), encoding="utf-8")
        log.append(f"{nr}-{s['ondernummer']}: {s['omschrijving'].lower()}, {len(paginas)} pagina's")
    return lijst


def _bestand(s):
    return re.sub(r"[^0-9A-Za-z]+", "_", s["ondernummer"] or s["id"])


def _schoon(t):
    import html
    return " ".join(html.unescape(re.sub(r"<[^>]+>", "", t)).split())


# ---------------------------------------------------------------- de tekst
def alineas(paginas):
    """De alinea's van een stuk: zonder kop- en voetregels, en een alinea die
    over een pagina (of een regel) doorloopt weer heel."""
    uit = []
    for pagina in paginas:
        for p in pagina:
            if not p or RUIS.match(p):
                continue
            if uit and not re.search(r"[.:;?!»”\")]$", uit[-1]) and re.match(r"[a-zà-ÿ(]", p) \
                    and not ARTKOP.match(uit[-1]) and not SECTIE.match(uit[-1]):
                uit[-1] = (uit[-1][:-1] if uit[-1].endswith("-") else uit[-1] + " ") + p
            else:
                uit.append(p)
    return uit


def _woorden(t):
    t = re.sub(r"(\w)-\s?(\w)", r"\1\2", t.lower())
    return re.findall(r"[a-zà-ÿ]+|\d+", t)


def _paren(t):
    w = _woorden(t)
    return {f"{w[i]} {w[i + 1]}" for i in range(len(w) - 1)}


def _kop(regel):
    """('3.2.2', rest) als de regel een artikelkop is; anders None."""
    # OCR: 'Artikel2.1.1', 'Artike/3.3.1', 'ArtikeM.4' (de 'l 1' als één teken).
    regel = re.sub(r"^ArtikeM(?=\.\d)", "Artikel 1", regel)
    regel = re.sub(r"^Artike[l1I/|]\s*(?=\d)", "Artikel ", regel)
    m = ARTKOP.match(regel)
    return (m["nr"], m["rest"] or "") if m else None


def artikelen_van_voorstel(regels):
    """{nummer: tekst} van een voorstel van wet."""
    uit, nr = {}, None
    for r in regels:
        k = _kop(r)
        if k:
            nr = k[0]
            uit.setdefault(nr, [])
            if k[1]:
                uit[nr].append(k[1])
            continue
        if SECTIE.match(r) and len(r) < 120:
            nr = None
            continue
        if nr:
            uit[nr].append(r)
    return {k: " ".join(v) for k, v in uit.items() if v}


def _hoofdstuk(nr):
    return re.split(r"[.:]", nr)[0] if re.search(r"[.:]", nr) else None


def vertaaltabel(oud, nieuw):
    """{oud nummer: (nieuw nummer, gelijkenis)}: elk artikel van het voorstel
    naar het artikel van de wet dat er het meest op lijkt; elk nieuw nummer één
    keer. Binnen hetzelfde hoofdstuk telt een paar zwaarder, daarbuiten moet
    het sterker lijken: een verhuisd artikel (het oude 6.3 werd hoofdstuk 7)
    vindt zo zijn plek zonder dat een zwakke buur in het eigen hoofdstuk het
    voor is."""
    p_oud = {k: _paren(t) for k, t in oud.items()}
    p_nieuw = {k: _paren(" ".join(t) if isinstance(t, list) else t) for k, t in nieuw.items()}
    kandidaten = []
    for o, po in p_oud.items():
        if not po:
            continue
        for n, pn in p_nieuw.items():
            if not pn:
                continue
            j = len(po & pn) / len(po | pn)
            zelfde = _hoofdstuk(o) is not None and _hoofdstuk(o) == _hoofdstuk(n)
            if j >= (DREMPEL if zelfde else DREMPEL_ELDERS):
                kandidaten.append((-(j + (BONUS if zelfde else 0)), j, o, n))
    kandidaten.sort()
    uit, bezet = {}, set()
    for _, j, o, n in kandidaten:
        if o in uit or n in bezet:
            continue
        uit[o] = (n, round(j, 2))
        bezet.add(n)
    return uit


def soort_van(omschrijving):
    o = omschrijving.upper()
    for k, s in SOORTEN:
        if o.startswith(k) or (k in ("VERSLAG",) and "VERSLAG" in o and not o.startswith("NOTA")):
            return s
    return None


# ---------------------------------------------------------------- de brokken
def lees(regels, stuk, datum, soort, tabel, titels, kop0):
    """De brokken van één stuk: per artikelkop een artikelsgewijs brok, en
    elders per kop de alinea's die een artikel van het voorstel noemen."""
    brokken, losse, niet = [], {}, []
    pad, brok = [kop0], None

    def kop():
        return " › ".join(pad)

    for r in regels:
        k = _kop(r)
        if k and len(r) < 400:
            oud = k[0]
            if oud in tabel:
                nieuw, _ = tabel[oud]
                brok = {"stuk": stuk, "datum": datum, "soort": soort, "rol": "artikelsgewijs", "kop": kop(),
                        "titel": f"Artikel {oud} van het voorstel (artikel {nieuw})", "oud": oud,
                        "artikelen": [nieuw], "leden": {}, "tekst": []}
                brokken.append(brok)
                if k[1]:
                    brok["tekst"].append([k[1], "al"])
            else:
                brok = None
                niet.append({"stuk": stuk, "reden": f"artikel {oud} van het voorstel koppelt aan geen artikel van de wet",
                             "tekst": r[:160]})
            continue
        if SECTIE.match(r) and len(r) < 120 and not r.endswith("."):
            brok = None
            pad = [kop0, r]
            continue
        if brok is not None:
            brok["tekst"].append([r, "al"])
            continue
        for oud, lid in K.vermeldingen(r, tuple(titels) + ("wetsvoorstel",)):
            if "." not in oud:
                continue  # 'artikel 2' zonder naam is in deze stukken meestal een andere wet
            if oud not in tabel:
                niet.append({"stuk": stuk, "reden": f"artikel {oud} van het voorstel koppelt aan geen artikel van de wet",
                             "tekst": r[:160]})
                continue
            nieuw = tabel[oud][0]
            b = losse.get((kop(), nieuw))
            if not b:
                b = losse[(kop(), nieuw)] = {"stuk": stuk, "datum": datum, "soort": soort, "rol": "vermelding",
                                             "kop": kop(), "oud": oud, "artikelen": [nieuw], "leden": {}, "tekst": []}
            if not b["tekst"] or b["tekst"][-1][0] != r:
                b["tekst"].append([r, "al"])
            if lid:
                b["leden"].setdefault(nieuw, []).append(lid)
    return [b for b in brokken if b["tekst"]] + list(losse.values()), niet


def tijdlijn(bwb, nr, regelingen, kamerdossiers, log=None):
    """Een oud dossier als tijdlijn in de vorm die keten.py leest
    (kamertoelichting.uit_dossier): soort 'oud dossier', en in commentaar de
    brokken, al gehaakt aan de nummers van de wet."""
    import afspelen as A
    import keten
    log = [] if log is None else log
    k = json.loads((regelingen / bwb / "keten.json").read_text(encoding="utf-8"))
    titels = [k["titel"]] + ([k["afkorting"]] if k.get("afkorting") else [])
    oudste = next(v for v in k["versies"] if v["pad"])
    wet = keten.tekst_per_artikel(A.lees_wettekst(regelingen / bwb / oudste["pad"], f"versie-{oudste['datum']}"))
    map_ = kamerdossiers / nr / "sgd"
    lijst = haal(nr, map_, log)
    lees_ = lambda s: alineas(json.loads((map_ / f"{_bestand(s)}.json").read_text(encoding="utf-8")))  # noqa: E731
    voorstellen = [(s["datum"] or "", vertaaltabel(artikelen_van_voorstel(lees_(s)), wet), s)
                   for s in lijst if VOORSTEL.search(s["omschrijving"]) and (map_ / f"{_bestand(s)}.json").exists()]
    for d, t, s in voorstellen:
        log.append(f"{nr}-{s['ondernummer']} ({s['omschrijving'].lower()}): {len(t)} artikelen gekoppeld aan de tekst van {oudste['datum']}")
    brokken, niet, stukken_ = [], [], []
    for s in lijst:
        soort = soort_van(s["omschrijving"])
        stuk = f"kst-{nr}-{s['ondernummer']}"
        stukken_.append({"ondernummer": s["ondernummer"], "stuk": stuk, "soort": s["omschrijving"].capitalize(),
                         "datum": s["datum"], "url": s["url"], "gelezen": bool(soort)})
        if not soort or not voorstellen:
            continue
        # De nummering van het laatste voorstel van vóór dit stuk.
        tabel = next((t for d, t, _ in reversed(voorstellen) if d <= (s["datum"] or "")), voorstellen[0][1])
        bb, nn = lees(lees_(s), stuk, s["datum"], soort, tabel, titels, s["omschrijving"].capitalize())
        for b in bb:
            b["url"] = s["url"]
        brokken += bb
        niet += nn
    for b in brokken:
        b["leden"] = {a: sorted(set(v)) for a, v in b["leden"].items()}
        b.setdefault("doelartikelen", [])
    titel = (lijst[0]["titel"] if lijst else "") or f"Kamerdossier {nr}"
    log.append(f"{nr}: {len(brokken)} brokken ({sum(b['rol'] == 'artikelsgewijs' for b in brokken)} artikelsgewijs), "
               f"{len(niet)} niet gehaakt")
    return {"dossier": nr, "naam": titel, "soort": "oud dossier", "bwb": bwb, "stukken": stukken_,
            "commentaar": {"brokken": brokken, "niet": niet,
                           "tel": {"brokken": len(brokken), "niet_gehaakt": len(niet)}}}


if __name__ == "__main__":
    import kamerdossier
    log = []
    t = tijdlijn(sys.argv[1], sys.argv[2], kamerdossier.REGELINGEN, kamerdossier.KAMERDOSSIERS, log)
    print("\n".join(log))
