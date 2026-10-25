"""Speel de nota's van wijziging af op het oorspronkelijke wetsvoorstel.

Het voorstel (kst-<dossier>-2.xml) wordt een boom: koppen (deel, hoofdstuk,
paragraaf) en artikelen, met daaronder leden, onderdelen en, in het
begripsartikel, begrippen. Elke knoop draagt zijn herkomst: uit welk stuk en
welk onderdeel zijn tekst komt. Daarop worden de handelingen uit
handelingen.py in volgorde toegepast. Een handeling die niet precies past --
een citaat dat niet of meer dan eens voorkomt, een adres dat niet bestaat --
wordt niet gegokt maar gemeld.

Wat er afspeelt, staat in het speelplan: de nota's van wijziging op hun
datum, en de amendementen die zijn aangenomen op de datum van de stemming
(besluiten.json, uit het Gegevensmagazijn). Een amendement waarover niet is
gestemd, dat is verworpen of dat door een ander is vervangen, speelt niet mee.

De controle is de vergelijking met de ijkpunten: de officiële teksten die
er van de wet bestaan. Het gewijzigd voorstel zoals de Tweede Kamer het
aannam (Eerste Kamerstuk A), het Staatsblad, en na de inwerkingtreding de
geldende tekst uit het BWB; zolang die er niet zijn, een met de hand
doorgewerkte bron (de Wpp: document.txt). Het laatste is het eindpunt. Elk
artikel moet daar na het afspelen woord voor woord gelijk aan zijn; een
verschil is een fout in die bron, in een nota, of in deze afspeler, en de
log zegt welke handeling het artikel raakte.

Een wijzigingswet ('De Archiefwet 1995 wordt als volgt gewijzigd: A. ...')
speelt zelf af als elk voorstel; daarnaast werkt elke officiële versie van
het voorstel door in de wet die zij wijzigt (de doelwet, als BWB-toestand of
Staatsblad in de bron van het dossier): wat de wet wordt als het voorstel zo
in werking treedt.

Wat een dossier is, zegt <kamerstukkenmap>/dossier.yaml (zie
bron/kamerdossiers/LEESMIJ.md).

    python3 afspelen.py <kamerstukkenmap> <handelingen.json> <uitmap>

Uitvoer: stand-<stuk>.txt per stadium, afspeellog.txt, vergelijking.txt,
herkomst.json.
"""
import copy
import json
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

import yaml

from handelingen import RANG, RANGTELWOORDEN, regels_xml, schoon, splits_onderdeel

NOTA = "Nota van wijziging"
AMENDEMENT = "Amendement"
# Het gewijzigd voorstel zoals de Tweede Kamer het aannam, als Eerste Kamerstuk.
GEWIJZIGD_VOORSTEL = "Gewijzigd voorstel van wet"


# ---------------------------------------------------------------- de boom

def knoop(soort, nr="", tekst="", titel="", herkomst=None, kinderen=None):
    return {"soort": soort, "nr": nr, "tekst": tekst, "titel": titel,
            "kinderen": kinderen or [], "herkomst": list(herkomst or [])}


def txt(e):
    return " ".join("".join(e.itertext()).split())


def _eigen_tekst(e, uitsluiten):
    return " ".join(txt(c) for c in e if c.tag == "al")


def lees_lijst(lijst, herkomst):
    uit = []
    for li in lijst.findall("li"):
        nr = txt(li.find("li.nr")) if li.find("li.nr") is not None else ""
        k = knoop("onderdeel", nr, " ".join(txt(a) for a in li.findall("al")), herkomst=herkomst)
        for sub in li.findall("lijst"):
            k["kinderen"] += lees_lijst(sub, herkomst)
        uit.append(k)
    return uit


def lees_inhoud(e, herkomst):
    """Leden, losse alinea's, lijsten en begrippen onder een artikel of lid."""
    uit = []
    for c in e:
        if c.tag == "lid":
            nr = txt(c.find("lidnr")) if c.find("lidnr") is not None else ""
            k = knoop("lid", nr, herkomst=herkomst)
            k["kinderen"] = lees_inhoud(c, herkomst)
            # De eerste alinea is de tekst van het lid zelf.
            if k["kinderen"] and k["kinderen"][0]["soort"] == "al":
                k["tekst"] = k["kinderen"].pop(0)["tekst"]
            uit.append(k)
        elif c.tag == "al":
            uit.append(knoop("al", tekst=txt(c), herkomst=herkomst))
        elif c.tag == "lijst":
            uit += lees_lijst(c, herkomst)
        elif c.tag == "definitielijst":
            for d in c.findall("definitie-item"):
                term = txt(d.find("term")).rstrip(": ").strip()
                df = d.find("definitie")
                k = knoop("begrip", term, " ".join(txt(a) for a in df.findall("al")), herkomst=herkomst)
                for l in df.findall("lijst"):
                    k["kinderen"] += lees_lijst(l, herkomst)
                uit.append(k)
        elif c.tag == "wat":
            uit.append(knoop("al", tekst=txt(c), herkomst=herkomst))
        elif c.tag == "wijzig-lid":
            # Een onderdeel van een wijzigingsartikel ('A. Artikel 2a wordt als
            # volgt gewijzigd: 1. ... 2. ...'): de opdracht is zijn tekst, de
            # genummerde wijzigingen eronder zijn zijn kinderen. Zo kan een nota
            # 'artikel I, onderdeel B, onder 3' adresseren.
            nr = txt(c.find("lidnr")) if c.find("lidnr") is not None else ""
            wat = c.find("wat")
            k = knoop("onderdeel", nr, txt(wat) if wat is not None else "", herkomst=herkomst)
            for w in c:
                if w.tag == "wijziging":
                    k["kinderen"] += wijzigingsknopen(w, herkomst)
            uit.append(k)
        elif c.tag == "wijzig-artikel":
            # 'Artikel 12.3a van de Jeugdwet wordt als volgt gewijzigd: 1. ...
            # 2. ...' (Archiefwet 2026, hoofdstuk 11): de genummerde wijzigingen
            # zijn onderdelen, zodat 'artikel 11.4, onderdeel 3' bestaat.
            k = c.find("kop")
            if k is not None and k.find("nr") is not None:
                uit += [knoop("al", tekst=t, herkomst=herkomst) for t in regels_xml(k)]
            uit += lees_inhoud(c, herkomst)
        elif c.tag == "wijziging":
            uit += wijzigingsknopen(c, herkomst)
        elif c.tag == "artikeltekst":
            uit += [knoop("al", tekst=t, herkomst=herkomst) for t in regels_xml(c)]
        elif c.tag == "kop" and e.tag == "wijzig-artikel":
            continue
    return uit


def wijzigingsknopen(w, herkomst):
    """Een <wijziging>: met een eigen nummer een knoop (onder 1, onder a),
    anders alleen haar regels."""
    nr = w.find("nr")
    if nr is None:
        return [knoop("al", tekst=t, herkomst=herkomst) for t in regels_xml(w)]
    wat = w.find("wat")
    k = knoop("onderdeel", txt(nr), txt(wat) if wat is not None else "", herkomst=herkomst)
    k["kinderen"] = [knoop("al", tekst=t, herkomst=herkomst)
                     for c in w if c.tag not in ("nr", "wat") for t in regels_xml(c)]
    return [k]


def lees_wettekst(pad, stuk):
    """Elke officiële tekst van de wet in dezelfde vorm: het voorstel, het
    gewijzigd voorstel van de Eerste Kamer, het Staatsblad en de BWB-toestand
    dragen allemaal een <wettekst> met dezelfde elementen. Een versie van
    wetten.overheid.nl (.html of .html.gz) leest wetten_html.py in dezelfde
    knopen."""
    naam = str(pad)
    if naam.endswith((".html", ".html.gz")):
        import gzip
        from wetten_html import lees_wetten_html
        ruw = gzip.open(pad, "rt", encoding="utf-8").read() if naam.endswith(".gz") else Path(pad).read_text(encoding="utf-8")
        return lees_wetten_html(ruw, stuk, knoop)
    return lees_voorstel(pad, stuk)


# De Staatsbladen van vóór ongeveer 2009 gebruiken andere namen voor dezelfde
# indeling: wart is wijzig-artikel, wlid wijzig-lid, wond een genummerde
# wijziging, arttkst en wlichaam de nieuwe tekst, art een artikel; de opdracht
# is daar de eerste alinea, en een nummer staat in <nr>.
OUDE_NAMEN = {"wart": "wijzig-artikel", "wlid": "wijzig-lid", "wond": "wijziging", "arttkst": "artikeltekst",
              "wlichaam": "artikeltekst", "art": "artikel", "hfdst": "hoofdstuk", "par": "paragraaf"}


def moderniseer(r):
    """Zet een Staatsblad in de oude vorm om naar de nieuwe, op zijn plek."""
    for e in r.iter():
        oud = e.tag
        if oud not in OUDE_NAMEN:
            continue
        e.tag = OUDE_NAMEN[oud]
        if oud in ("wart", "wlid", "wond"):
            al = next((c for c in e if c.tag == "al"), None)
            if al is not None:
                al.tag = "wat"
        if oud == "wlid":
            k = e.find("kop")
            if k is not None and k.find("nr") is not None:
                nr = ET.Element("lidnr")
                nr.text = txt(k.find("nr"))
                e.remove(k)
                e.insert(0, nr)
    for lid in r.iter("lid"):
        n = lid.find("nr")
        if n is not None and lid.find("lidnr") is None:
            n.tag = "lidnr"
    return r


def lees_voorstel(pad, stuk):
    herkomst = [{"stuk": stuk}]
    r = ET.parse(pad).getroot()
    wet = r.find(".//wettekst")
    uit = []

    def loop(e):
        for c in e:
            if c.tag in ("deel", "hoofdstuk", "paragraaf"):
                kop = c.find("kop")
                nr = txt(kop.find("nr")).rstrip(".") if kop is not None and kop.find("nr") is not None else ""
                titel = txt(kop.find("titel")) if kop is not None and kop.find("titel") is not None else ""
                if titel.isupper():
                    # 'AANVULLENDE REGELS ...': de hoofdletters zijn opmaak, de nota citeert gewoon.
                    titel = titel.capitalize()
                uit.append(knoop(c.tag, nr, titel=titel, herkomst=herkomst))
                loop(c)
            elif c.tag in ("artikel", "wijzig-artikel"):
                # Een wijzigingsartikel (art. 141 e.v.: wijziging van de
                # Kieswet) staat in de XML als wijzig-artikel, niet als artikel.
                kop = c.find("kop")
                nr = txt(kop.find("nr")).rstrip(".") if kop is not None and kop.find("nr") is not None else ""
                titel = txt(kop.find("titel")) if kop is not None and kop.find("titel") is not None else ""
                if not nr:
                    # Zonder <nr> staat het nummer vooraan in de titel: label 'ARTIKEL',
                    # titel 'XLVI CITEERTITEL' (verzamelwet, Kamerstuk 36481-2).
                    m = re.match(r"^([IVXLCDM]+|\d+[a-z]*)\.?\s+(.*)$", titel)
                    if m:
                        nr, titel = m[1], (m[2].capitalize() if m[2].isupper() else m[2])
                    elif kop is not None and kop.find("label") is not None:
                        nr = txt(kop.find("label"))
                a = knoop("artikel", nr, titel=titel, herkomst=herkomst)
                a["kinderen"] = lees_inhoud(c, herkomst)
                uit.append(a)
    loop(wet)
    return uit


# ---------------------------------------------------------------- nieuwe tekst

# Een artikelkop heeft een punt na het nummer ('Artikel 35a. Voormalig ...');
# zonder punt is het een zin ('Artikel 20 van Boek 2 ... is niet van toepassing').
# Archiefwet 2026: 'Artikel 11.26 Verwerking definitieve citeertitel', een nummer
# met een punt erin en geen punt erachter; dan begint de titel met een hoofdletter.
KOP_ART = re.compile(r"^Artikel (?P<nr>(?:[A-Z][a-z]? )?\d+[a-z]*(?:[.:]\d+[a-z]*)*)(?:\.|(?= [A-Z])|$)(?: (?P<titel>.*))?$")
KOP_STRUCT = re.compile(r"^(?P<soort>DEEL|Deel|HOOFDSTUK|Hoofdstuk|§|Paragraaf) ?(?P<nr>[0-9]+[A-Za-z]*)[.:]? ?(?P<titel>.*)$")
LIDNR = re.compile(r"^(?P<nr>\d+[a-z]?\.) (?P<tekst>.*)$")
ONDNR = re.compile(r"^(?P<nr>(?:[a-z]{1,2}|\d+°)\.?) (?P<tekst>.*)$")
BEGRIP = re.compile(r"^(?P<term>[^:]{2,60}): (?P<tekst>.*)$")


def lees_nieuwe_tekst(regels, herkomst, begrippen=False):
    """Zet de tekst na 'luidende:' om in knopen. Koppen en artikelen komen
    bovenaan; leden en onderdelen onder het laatste artikel of lid."""
    uit, art, lid = [], None, None
    for r in regels:
        r = schoon(r)
        if not r:
            continue
        m = KOP_ART.match(r)
        if m:
            art = knoop("artikel", m["nr"], titel=(m["titel"] or "").strip(), herkomst=herkomst)
            uit.append(art)
            lid = None
            continue
        m = KOP_STRUCT.match(r)
        if m:
            soort = {"§": "paragraaf", "paragraaf": "paragraaf"}.get(m["soort"].lower(), m["soort"].lower())
            uit.append(knoop(soort, m["nr"], titel=m["titel"].strip(), herkomst=herkomst))
            art = lid = None
            continue
        doel = art["kinderen"] if art else uit
        m = LIDNR.match(r)
        if m:
            lid = knoop("lid", m["nr"], m["tekst"], herkomst=herkomst)
            doel.append(lid)
            continue
        m = ONDNR.match(r)
        if m:
            o = knoop("onderdeel", m["nr"] if m["nr"].endswith(".") else m["nr"] + ".",
                      m["tekst"], herkomst=herkomst)
            (lid["kinderen"] if lid else doel).append(o)
            continue
        m = BEGRIP.match(r) if begrippen else None
        if m:
            doel.append(knoop("begrip", m["term"], m["tekst"], herkomst=herkomst))
            continue
        (lid["kinderen"] if lid else doel).append(knoop("al", tekst=r, herkomst=herkomst))
    return uit


# ---------------------------------------------------------------- adressering

class Fout(Exception):
    pass


# Opdrachten die alleen met een aanname konden worden toegepast; de
# vergelijking met de bron laat zien of de doorwerker dezelfde lezing koos.
AANNAMES = []


def zoek_artikel(wet, nr):
    for i, k in enumerate(wet):
        if k["soort"] == "artikel" and k["nr"] == nr:
            return i, k
    raise Fout(f"artikel {nr} bestaat niet (meer)")


def nummer(nr):
    return nr.rstrip(".").strip()


def zoek_kind(ouder, soort, nr):
    for i, k in enumerate(ouder["kinderen"]):
        if k["soort"] == soort and nummer(k["nr"]) == str(nr):
            return i, k
    raise Fout(f"{soort} {nr} bestaat niet in {ouder['soort']} {ouder['nr']}")


def bereik(wet, adres):
    """Het knooppunt waarop een opdracht werkt, en zijn ouder."""
    if not adres or "artikel" not in adres:
        raise Fout(f"geen artikeladres ({adres})")
    _, k = zoek_artikel(wet, adres["artikel"])
    ouder = None
    if "lid" in adres:
        ouder, k = k, zoek_kind(k, "lid", adres["lid"])[1]
    if "onderdeel" in adres:
        ouder, k = k, zoek_kind(k, "onderdeel", adres["onderdeel"])[1]
    if "onder2" in adres:
        # 'artikel I, onderdeel B, onder 3': een wijziging binnen een onderdeel.
        ouder, k = k, zoek_kind(k, "onderdeel", adres["onder2"])[1]
    return ouder, k


def aanhef_of_geheel(k, opschrift, aanhef):
    if aanhef and k["soort"] == "artikel" and any(c["soort"] == "lid" for c in k["kinderen"]):
        # '41, aanhef' in een artikel met leden: bedoeld is de aanhef van het eerste lid.
        AANNAMES.append(f"'aanhef' van artikel {k['nr']}, dat leden heeft: gelezen als de aanhef van het eerste lid")
        k = next(c for c in k["kinderen"] if c["soort"] == "lid")
    return tekstknopen(k, opschrift, aanhef)


def tekstknopen(k, opschrift=False, aanhef=False):
    if opschrift:
        return [(k, "titel")]
    if aanhef:
        # De aanhef: de eigen tekst van het lid of artikel, zonder onderdelen.
        if k["soort"] == "artikel" and k["kinderen"] and k["kinderen"][0]["soort"] == "al":
            return [(k["kinderen"][0], "tekst")]
        return [(k, "tekst")] if k["tekst"] else []
    uit = [(k, "tekst")] if k["tekst"] else []
    for c in k["kinderen"]:
        uit += tekstknopen(c)
    return uit


# ---------------------------------------------------------------- tekstopdrachten

LEESTEKEN = {"punt": ".", "puntkomma": ";", "komma": ",", "dubbele punt": ":"}
CLAUSULES = [
    ("vervang", re.compile(r"«(?P<a>[^»]*)» (?:wordt |worden )?(?P<t>telkens )?vervangen door «(?P<b>[^»]*)»")),
    # Vóór 2009: "«a» vervangen door: b." De nieuwe tekst loopt tot het eind van de zin.
    ("vervang", re.compile(r"«(?P<a>[^»]*)» (?:wordt |worden )?(?P<t>telkens )?vervangen door: (?P<b>[^«»]+?)\.?$")),
    ("na", re.compile(r"(?:wordt |worden )?(?:na|achter) «(?P<a>[^»]*)» (?P<t>telkens )?(?:wordt |worden )?ingevoegd «(?P<b>[^»]*)»")),
    ("na", re.compile(r"(?:na|achter) «(?P<a>[^»]*)» (?P<t>telkens )?(?:wordt |worden )?(?:een )?(?P<b>komma|puntkomma|punt) ingevoegd")),
    ("na", re.compile(r"(?:een )?(?P<b>komma|puntkomma|punt) ingevoegd (?:na|achter) «(?P<a>[^»]*)»")),
    ("leesteken_weg", re.compile(r"vervalt de (?P<b>komma|puntkomma|punt) (?:na|achter) «(?P<a>[^»]*)»")),
    ("voor", re.compile(r"(?:voor|vóór) «(?P<a>[^»]*)» (?P<t>telkens )?(?:wordt |worden )?ingevoegd «(?P<b>[^»]*)»")),
    # Vóór 2009: "na «a» ingevoegd: b." De nieuwe tekst loopt tot het eind van de zin.
    ("na", re.compile(r"(?:na|achter) «(?P<a>[^»]*)» (?P<t>telkens )?(?:wordt |worden )?ingevoegd: (?P<b>[^«»]+?)\.?$")),
    ("schrap", re.compile(r"(?:vervalt|vervallen) (?:de zinsnede |de woorden |het woord )?(?P<t>telkens )?«(?P<a>[^»]*)»")),
    ("schrap", re.compile(r"«(?P<a>[^»]*)» (?P<t>telkens )?(?:vervalt|vervallen|wordt geschrapt)")),
    ("slot", re.compile(r"(?i)(?:de |het )?(?P<a>puntkomma|punt|komma|dubbele punt) aan het slot (?:van [^«]*? )?vervangen door (?:een|de) (?P<b>puntkomma|punt|komma|dubbele punt)")),
    ("slot", re.compile(r"(?i)aan het slot van [^«]*? de (?P<a>puntkomma|punt|komma|dubbele punt) vervangen door (?:een|de) (?P<b>puntkomma|punt|komma|dubbele punt)")),
    ("toevoeg", re.compile(r"(?:wordt|worden) (?:aan het slot )?(?:toegevoegd )?«(?P<b>[^»]*)»(?: toegevoegd)?")),
]


def clausules(opdracht):
    """Alle tekstbewerkingen in één opdracht, in volgorde. 'In het tweede lid
    wordt «a» vervangen door «b» en wordt «c» vervangen door «d»' zijn er twee.
    Elk citaat in de opdracht moet door een herkende bewerking zijn gebruikt:
    een opdracht die maar half gelezen is, wordt niet half uitgevoerd."""
    gevonden, bezet = [], []
    try:
        return _clausules(opdracht, gevonden, bezet)
    finally:
        pass


def _clausules(opdracht, gevonden, bezet):
    for soort, rx in CLAUSULES:
        for m in rx.finditer(opdracht):
            if any(m.start() < e and s < m.end() for s, e in bezet):
                continue
            bezet.append((m.start(), m.end()))
            gevonden.append((m.start(), soort, m["a"] if "a" in rx.groupindex else None,
                             m["b"] if "b" in rx.groupindex else None,
                             bool("t" in rx.groupindex and m["t"])))
    for c in re.finditer(r"«[^»]*»", opdracht):
        if not any(s <= c.start() and c.end() <= e for s, e in bezet):
            raise Fout(f"het citaat {c.group()[:60]} hoort bij geen herkende bewerking")
    return [g[1:] for g in sorted(gevonden)]


def plak(links, rechts):
    r, l = rechts.lstrip(NIEUW_OPEN + NIEUW_DICHT), links.rstrip(NIEUW_OPEN + NIEUW_DICHT)
    if not r or r[0] in ",.;:)" or not l or l[-1] in "(":
        return links + rechts
    return links + " " + rechts


# Tekst die de lopende nota zelf invoegt, staat tot het eind van die nota
# tussen deze twee tekens. Een veegactie van dezelfde nota slaat haar over:
# de opsteller schreef nieuwe tekst al in de nieuwe termen (gemeten
# 12-10-2026: zonder dit werd het 'landelijke landelijke politieke partij').
NIEUW_OPEN, NIEUW_DICHT = "\ue000", "\ue001"
MERK = f"[{NIEUW_OPEN}{NIEUW_DICHT}]*"


def citaat(a):
    """Een citaat matcht op woordgrenzen: «gift» niet in 'giften', «verwachtte
    aantal» niet in 'verwachtte aantallen'; en dwars door de merktekens heen."""
    links = r"(?<!\w)" if a[:1].isalnum() else ""
    rechts = r"(?!\w)" if a[-1:].isalnum() else ""
    return re.compile(links + MERK.join(re.escape(c) for c in a) + rechts)


def in_nieuwe_tekst(t, m):
    voor = t[:m.start()]
    return voor.count(NIEUW_OPEN) > voor.count(NIEUW_DICHT) or NIEUW_OPEN in m.group(0) or NIEUW_DICHT in m.group(0)


def ontmerk(wet):
    for k in alle_knopen(wet):
        for veld in ("tekst", "titel"):
            k[veld] = k[veld].replace(NIEUW_OPEN, "").replace(NIEUW_DICHT, "")


def volzin_bereik(t, n):
    grenzen = [0] + [m.end() for m in re.finditer(r"[.;:](?=\s+[A-Z])", t)] + [len(t)]
    if n >= len(grenzen):
        raise Fout(f"er is geen {n}e volzin")
    return grenzen[n - 1], grenzen[n]


def bewerk(knopen, cl, herkomst, volzin=None, veeg=False):
    """Alle clausules van een opdracht, eerst op proef: een opdracht met drie
    vervangingen waarvan de derde faalt, verandert niets."""
    proef = [({veld: k[veld], "herkomst": []}, veld) for k, veld in knopen]
    voor = len(AANNAMES)
    for c in cl:
        pas_tekst_toe(proef, *c, herkomst, volzin=volzin, veeg=veeg)
    del AANNAMES[voor:]
    for c in cl:
        pas_tekst_toe(knopen, *c, herkomst, volzin=volzin, veeg=veeg)


def pas_tekst_toe(knopen, soort, a, b, telkens, herkomst, volzin=None, veeg=False):
    if soort == "slot":
        k, veld = knopen[-1]
        t = k[veld].rstrip()
        if not t.endswith(LEESTEKEN[a]):
            raise Fout(f"tekst eindigt niet op een {a}: …{t[-30:]}")
        k[veld] = t[: -len(LEESTEKEN[a])] + LEESTEKEN[b]
        k["herkomst"].append(herkomst)
        return
    if soort == "toevoeg":
        k, veld = knopen[0]
        t = k[veld].rstrip()
        slot = t[-1:] if t[-1:] in ".;:" else ""
        if slot and (b[:1] in ",;" or b[:1].islower()):
            # 'kalenderjaar;' + «waarvoor subsidie ...» -> 'kalenderjaar waarvoor subsidie ...;'
            k[veld] = plak(t[:-1], b) + ("" if b[-1:] in ".;:" else slot)
        else:
            k[veld] = plak(t, b)
        k["herkomst"].append(herkomst)
        return
    rx = citaat(a)
    treffers, overgeslagen = [], 0
    for k, veld in knopen:
        van, tot = volzin_bereik(k[veld], volzin) if volzin else (0, len(k[veld]))
        for m in rx.finditer(k[veld], van, tot):
            if veeg and in_nieuwe_tekst(k[veld], m):
                overgeslagen += 1
            else:
                treffers.append((k, veld, m))
    if overgeslagen and treffers:
        AANNAMES.append(f"«{a}»: {overgeslagen} keer overgeslagen in tekst die deze nota zelf schreef")
    if not treffers:
        rx = re.compile(citaat(a).pattern, re.I)
        for k, veld in knopen:
            van, tot = volzin_bereik(k[veld], volzin) if volzin else (0, len(k[veld]))
            treffers += [(k, veld, m) for m in rx.finditer(k[veld], van, tot)]
        if treffers:
            AANNAMES.append(f"«{a}» staat er alleen met andere hoofdletters")
    if not treffers:
        raise Fout(f"«{a}» komt niet voor" + (f" in de {volzin}e volzin" if volzin else ""))
    if len(treffers) > 1 and not telkens:
        AANNAMES.append(f"«{a}» staat er {len(treffers)} keer en 'telkens' ontbreekt; alle vervangen")
    nieuw = LEESTEKEN.get(b, b) if b else ""
    # Van achter naar voren vervangen, zodat de posities kloppen.
    for k, veld, m in sorted(treffers, key=lambda x: -x[2].start()):
        t = k[veld]
        if soort == "vervang":
            mid = NIEUW_OPEN + nieuw + NIEUW_DICHT if nieuw else ""
        elif soort == "schrap":
            mid = ""
        elif soort == "leesteken_weg":
            na = t[m.end():]
            if not na.startswith(nieuw):
                raise Fout(f"na «{a}» staat geen {b}")
            t = t[:m.end()] + na[len(nieuw):]
            mid = m.group(0)
        elif soort == "na":
            mid = plak(m.group(0), NIEUW_OPEN + nieuw + NIEUW_DICHT)
        else:  # voor
            mid = plak(NIEUW_OPEN + nieuw + NIEUW_DICHT, m.group(0))
        t = t[:m.start()] + mid + t[m.end():]
        t = " ".join(t.split())
        k[veld] = re.sub(rf"(?:^| )({MERK})([,.;])", r"\1\2", t)
        if herkomst not in k["herkomst"]:
            k["herkomst"].append(herkomst)


# ---------------------------------------------------------------- structuuropdrachten

def hernummer(ouder, soort):
    """Na een vernummering: leden 1, 2, 3 …; onderdelen a, b, c … in volgorde."""
    i = 0
    for k in ouder["kinderen"]:
        if k["soort"] == soort:
            i += 1
            k["nr"] = f"{i}." if soort == "lid" else f"{chr(ord('a') + i - 1)}."


def begrippen_in(opdracht):
    m = re.search(r"met betrekking tot (.+?)(?: komt| komen| vervalt| vervallen| wordt| worden|,)", opdracht)
    if not m:
        return []
    deel = re.sub(r"(?:en )?de zinsnede met betrekking tot ", " en ", m[1])
    # 'de gift en en de gift in natura' (nr. 20, A onder 4): de dubbele 'en' leest als één.
    namen = [re.sub(r"^(?:en )?(?:de|het) ", "", n.strip()) for n in re.split(r",| en ", deel)]
    return [n for n in namen if n]


def zoek_begrip(art, naam):
    for i, k in enumerate(art["kinderen"]):
        if k["soort"] == "begrip" and k["nr"].casefold() == naam.casefold():
            return i, k
    raise Fout(f"begrip '{naam}' bestaat niet in artikel {art['nr']}")


def voeg_in(lijst, index, nieuw):
    lijst[index:index] = nieuw


def aanhalingstekens_kloppen(t):
    diepte = 0
    for c in t:
        if c == "«":
            if diepte:
                return False
            diepte = 1
        elif c == "»":
            if not diepte:
                return False
            diepte = 0
    return diepte == 0


def speel(wet, h, herkomst):
    """Speel één handeling af op `wet`. Een opdracht waarvan het adres mist
    wat zij nodig heeft (een Staatsblad van 1998 adresseert 'na afdeling 3.4',
    geen artikel; gemeten 22-10-2026 in de hele Awb) speelt niet af, met die
    reden, in plaats van dat de hele opbouw stopt."""
    try:
        return _speel(wet, h, herkomst)
    except KeyError as e:
        raise Fout(f"het adres van de opdracht noemt geen {e.args[0]}") from None


def _speel(wet, h, herkomst):
    soort, adres, opdracht = h["soort"], h["adres"] or {}, h["opdracht"]
    if not aanhalingstekens_kloppen(opdracht):
        # nr. 20, UU en CCC: een openend teken waar een sluitend hoort. Wat
        # geciteerd is valt dan niet vast te stellen; niet raden.
        raise Fout("de aanhalingstekens in de opdracht sluiten niet")
    tekst = h.get("tekst") or []
    vernummerd = "vernummering" in h or "verlettering" in opdracht

    # -- begrippen in het begripsartikel ('Aan de zinsnede met betrekking tot
    # de peildatum wordt toegevoegd «...»' is ook een begripsopdracht)
    if soort and not soort.startswith("begrip") and "zinsnede met betrekking tot" in opdracht:
        soort = "begrip_wijzigen"
    if soort and soort.startswith("begrip"):
        _, art = zoek_artikel(wet, adres.get("artikel", "1"))
        if soort == "begrip_invoegen":
            for nieuw in lees_nieuwe_tekst(tekst, [herkomst], begrippen=True):
                if nieuw["soort"] != "begrip":
                    raise Fout(f"geen begrip in de in te voegen tekst: {nieuw['tekst'][:60]}")
                i = next((i for i, k in enumerate(art["kinderen"])
                          if k["soort"] == "begrip" and k["nr"].casefold() > nieuw["nr"].casefold()),
                         len(art["kinderen"]))
                voeg_in(art["kinderen"], i, [nieuw])
            return
        namen = begrippen_in(opdracht)
        if not namen:
            raise Fout("geen begrip in de opdracht")
        for naam in namen:
            i, k = zoek_begrip(art, naam)
            if soort == "begrip_vervalt":
                art["kinderen"].pop(i)
            elif soort == "begrip_vervangen":
                nieuw = lees_nieuwe_tekst(tekst, [herkomst], begrippen=True)
                art["kinderen"][i:i + 1] = nieuw
            else:
                cl = clausules(opdracht)
                if not cl:
                    raise Fout("geen tekstbewerking in de opdracht")
                bewerk(tekstknopen(k), cl, herkomst)
        return

    # -- een begrip als adres van een gewone tekstopdracht (nr. 8, A)
    if h.get("begrip") and soort == "vervang_tekst":
        _, art = zoek_artikel(wet, adres["artikel"])
        _, k = zoek_begrip(art, re.sub(r"^(?:de|het) ", "", h["begrip"]))
        bewerk(tekstknopen(k), clausules(opdracht), herkomst)
        return

    # -- opschriften en structuur (deel, hoofdstuk, paragraaf)
    m = re.search(r"[Hh]et opschrift(?: van (?P<van>.+?))? (?:komt|komen) te luiden: (?P<nieuw>.+?)\.?$", opdracht)
    if m:
        if m["van"] and re.search(r"paragraaf|hoofdstuk|deel", m["van"]):
            k = zoek_kop(wet, structuuradres(m["van"]))
        else:
            k = bereik(wet, adres)[1]
        k["titel"] = m["nieuw"].strip().strip("«»").strip()
        k["herkomst"].append(herkomst)
        return
    m = re.search(r"opschrift «DEEL (\d+)[^»]*» wordt «DEEL \d+» vervangen door «DEEL (\d+)»", opdracht)
    if m:
        k = zoek_kop(wet, {"deel": m[1]})
        k["nr"] = m[2]
        k["herkomst"].append(herkomst)
        return
    if "artikel" not in adres and "doelen" not in adres and any(x in adres for x in ("hoofdstuk", "paragraaf", "deel")):
        if soort == "vervalt":
            verval_structuur(wet, adres, vernummerd, herkomst)
            return
        k = zoek_kop(wet, adres)
        if "opschrift" in opdracht.split("«")[0] or not h.get("telkens"):
            for c in clausules(opdracht):
                pas_tekst_toe([(k, "titel")], *c, herkomst)
            return
        # 'In hoofdstuk 11 wordt «Archiefwet 2021» telkens vervangen door
        # «Archiefwet 20..»': in elk artikel onder die kop.
        i = next(i for i, x in enumerate(wet) if x is k)
        j = einde_van(wet, k["soort"], k["nr"])
        knopen = [t for x in wet[i + 1:j] if x["soort"] == "artikel" for t in tekstknopen(x)]
        bewerk(knopen, clausules(opdracht), herkomst)
        return

    # -- meer doelen tegelijk: elk doel krijgt de bewerking, en per doel
    # geldt de eis dat het citaat er precies een keer staat
    if "doelen" in adres and soort == "vervalt":
        for d in adres["doelen"]:
            i, _ = zoek_artikel(wet, d["artikel"])
            wet.pop(i)
        return
    if "doelen" in adres:
        opschrift = "opschrift" in opdracht.split("«")[0]
        cl = clausules(opdracht)
        if not cl:
            raise Fout("geen tekstbewerking in de opdracht")
        fouten, gebonden = [], h.get("_gebonden") or [None] * len(adres["doelen"])
        levend = {id(x) for x in alle_knopen(wet)}
        for d, vast in zip(adres["doelen"], gebonden):
            try:
                if isinstance(vast, Fout):
                    k = bereik(wet, d)[1]
                    AANNAMES.append(f"doel {d} bestaat pas na een ander onderdeel van deze nota")
                else:
                    k = vast if vast is not None else bereik(wet, d)[1]
                if id(k) not in levend or k.get("vervangen_door") == herkomst["stuk"] or (
                        is_veeg(h) and k["herkomst"] and k["herkomst"][0].get("stuk") == herkomst["stuk"]):
                    # Dezelfde nota liet dit doel vervallen of gaf het een
                    # geheel nieuwe tekst; de veegactie heeft er niets meer te doen.
                    AANNAMES.append(f"doel {d} is door deze nota al vervangen of vervallen; overgeslagen")
                    continue
                bewerk(aanhef_of_geheel(k, opschrift, d.get("aanhef")), cl, herkomst,
                       veeg=is_veeg(h))
            except Fout as e:
                fouten.append(f"{d}: {e}")
        if fouten and len(fouten) == len(adres["doelen"]):
            raise Fout(f"geen van de {len(fouten)} doelen: " + "; ".join(fouten[:3]))
        if fouten:
            # De andere doelen zijn wel bewerkt; dit is een deelfout.
            raise Fout(f"deels: {len(fouten)} van {len(adres['doelen'])} doelen niet: " + "; ".join(fouten[:3]))
        return

    if soort in ("vervang_tekst", "schrap_tekst", "voeg_tekst_in", "voeg_tekst_toe",
                 "vervang_leesteken", "voeg_leesteken_in", "opschrift"):
        _, k = bereik(wet, adres)
        opschrift = "opschrift" in opdracht.split("«")[0]
        cl = clausules(opdracht)
        if soort == "opschrift":
            k["titel"] = h["nieuw"]
            k["herkomst"].append(herkomst)
            return
        if not cl:
            raise Fout("geen tekstbewerking in de opdracht")
        bewerk(aanhef_of_geheel(k, opschrift, adres.get("aanhef")), cl, herkomst, adres.get("volzin"))
        return

    if soort == "vervalt" and re.search(r"verval(?:t|len) (?:de zinsnede |de woorden |het woord )?«", opdracht):
        # Een citaat na 'vervalt' is tekst die vervalt, geen lid of artikel
        # (nr. 20, TTTTT: zonder deze wacht verdween het hele tweede lid).
        raise Fout("'vervalt' met een citaat is als structuuropdracht gelezen; grammatica nalopen")
    if soort == "vervalt" and adres.get("volzin") and "artikel" in adres:
        # 'Het eerste lid, tweede volzin, vervalt': alleen die zin, niet het lid
        # (Stb. 2024, 164; zonder deze wacht verdween het hele lid).
        _, k = bereik(wet, adres)
        s, e = volzin_bereik(k["tekst"], adres["volzin"])
        k["tekst"] = (k["tekst"][:s] + k["tekst"][e:]).strip()
        k["herkomst"].append(herkomst)
        return
    if soort == "vervalt":
        if "artikel" not in adres:
            raise Fout(f"geen adres om te laten vervallen ({adres})")
        if set(adres) == {"artikel"}:
            i, _ = zoek_artikel(wet, adres["artikel"])
            wet.pop(i)
            return
        ouder, k = bereik(wet, adres)
        n_vervanging = len(re.findall(r"\bvervanging\b", opdracht))
        n_gelezen = len(re.findall(r"onder vervanging van (?:de (?:punt|puntkomma|komma)|«[^»]*») achter onderdeel \w+ door", opdracht))
        if n_vervanging != n_gelezen:
            raise Fout(f"{n_vervanging - n_gelezen} vervanging(en) in de opdracht niet te lezen")
        # 'onder vervanging van de puntkomma achter onderdeel a door «; en»'
        for m in re.finditer(r"onder vervanging van (?:de (?P<ot>punt|puntkomma|komma)|«(?P<oc>[^»]*)») achter onderdeel "
                             r"(?P<o>\w+) door (?:een (?P<nt>punt|puntkomma|komma)|«(?P<nc>[^»]*)»)", opdracht):
            doel = zoek_kind(ouder, "onderdeel", m["o"])[1]
            oud = LEESTEKEN.get(m["ot"]) if m["ot"] else m["oc"]
            nieuw = LEESTEKEN.get(m["nt"]) if m["nt"] else m["nc"]
            t = doel["tekst"].rstrip()
            if not t.endswith(oud):
                raise Fout(f"onderdeel {m['o']} eindigt niet op «{oud}»")
            doel["tekst"] = t[: -len(oud)].rstrip() + nieuw
            doel["herkomst"].append(herkomst)
        ouder["kinderen"] = [c for c in ouder["kinderen"] if c is not k]
        if vernummerd:
            hernummer(ouder, k["soort"])
        # '... vervalt het tweede lid alsmede de aanduiding «1.» voor het eerste lid'
        if "de aanduiding «1.»" in opdracht:
            leden = [c for c in ouder["kinderen"] if c["soort"] == "lid"]
            if len(leden) != 1:
                raise Fout("aanduiding «1.» vervalt, maar er blijft niet precies één lid over")
            i = next(i for i, c in enumerate(ouder["kinderen"]) if c is leden[0])
            # Dezelfde knoop, zonder nummer: een veegactie die het 'eerste lid'
            # van vóór de nota adresseert, vindt hem nog.
            lid, kinderen = leden[0], leden[0]["kinderen"]
            lid["soort"], lid["nr"], lid["kinderen"] = "al", "", []
            ouder["kinderen"][i + 1:i + 1] = kinderen
        ouder["herkomst"].append(herkomst)
        return

    if soort == "vernummer_aanduiding":
        _, art = zoek_artikel(wet, adres["artikel"])
        if any(k["soort"] == "lid" for k in art["kinderen"]):
            raise Fout("artikel heeft al leden")
        eerste = art["kinderen"].pop(0) if art["kinderen"] and art["kinderen"][0]["soort"] == "al" else knoop("al")
        lid = knoop("lid", "1.", eerste["tekst"], herkomst=eerste["herkomst"] + [herkomst])
        lid["kinderen"], art["kinderen"] = art["kinderen"], [lid]
        return

    nieuw = lees_nieuwe_tekst(tekst, [herkomst], begrippen=adres.get("artikel") == "1")
    if not nieuw:
        raise Fout("geen nieuwe tekst na 'luidende:'")

    if soort == "vervang_geheel":
        reeks = re.search(rf"[Hh]et ({RANG}) tot en met ({RANG}) lid", opdracht)
        if reeks:
            _, art = zoek_artikel(wet, adres["artikel"])
            van, tot = RANGTELWOORDEN[reeks[1].lower()], RANGTELWOORDEN[reeks[2].lower()]
            idx = [i for i, c in enumerate(art["kinderen"])
                   if c["soort"] == "lid" and van <= int(nummer(c["nr"]) or 0) <= tot]
            if not idx:
                raise Fout(f"lid {van} bestaat niet in artikel {art['nr']}")
            if len(idx) != tot - van + 1:
                AANNAMES.append(f"'het {reeks[1]} tot en met {reeks[2]} lid komen te luiden', maar artikel "
                                f"{art['nr']} heeft er {len(idx)}; gelezen als: lid {van} en volgende worden vervangen")
            art["kinderen"][idx[0]:idx[-1] + 1] = nieuw
            return
        if set(adres) == {"artikel"} and "opschrift" not in opdracht:
            i, oud = zoek_artikel(wet, adres["artikel"])
            if nieuw[0]["soort"] == "artikel":
                wet[i:i + 1] = nieuw
            else:
                oud["kinderen"] = nieuw
                oud["herkomst"].append(herkomst)
            return
        ouder, k = bereik(wet, adres)
        if len(nieuw) == 1 and nieuw[0]["soort"] in (k["soort"], "al"):
            n = nieuw[0]
            k["tekst"] = n["tekst"]
            k["kinderen"] = n["kinderen"]
            k["herkomst"] = [herkomst]
            k["vervangen_door"] = herkomst["stuk"]
            return
        if k["soort"] == "lid" and all(n["soort"] in ("onderdeel", "al") for n in nieuw[1:]) and nieuw[0]["soort"] == "al":
            k["tekst"], k["kinderen"], k["herkomst"] = nieuw[0]["tekst"], nieuw[1:], [herkomst]
            k["vervangen_door"] = herkomst["stuk"]
            return
        raise Fout(f"nieuwe tekst past niet op {k['soort']} {k['nr']}: {[n['soort'] for n in nieuw]}")

    if soort in ("invoegen_na", "lid_invoegen", "toevoegen"):
        if nieuw[0]["soort"] in ("artikel", "deel", "hoofdstuk", "paragraaf"):
            if soort == "toevoegen":
                m = re.search(r"[Aa]an (hoofdstuk|paragraaf|deel) (\d+[a-z]*)", opdracht)
                if not m:
                    raise Fout("onbekend waaraan toe te voegen")
                i = einde_van(wet, m[1], m[2])
            else:
                i, _ = zoek_artikel(wet, adres["artikel"])
                i += 1 if h.get("positie", "na") == "na" else 0
            voeg_in(wet, i, nieuw)
            return
        # leden of onderdelen in een artikel of lid
        if soort == "toevoegen" and "lid" in adres and nieuw[0]["soort"] == "onderdeel":
            ouder, doel = bereik(wet, adres)
        elif nieuw[0]["soort"] == "lid":
            _, doel = zoek_artikel(wet, adres["artikel"])
        elif nieuw[0]["soort"] == "onderdeel":
            a = {k: v for k, v in adres.items() if k != "onderdeel"}
            doel = bereik(wet, a)[1]
        else:
            raise Fout(f"onbekend wat in te voegen: {nieuw[0]['soort']}")
        m = re.search(r"onder vervanging van (?:de|het) (punt|puntkomma|komma) aan het slot van onderdeel (\w+) door (?:een|de) (punt|puntkomma|komma)", opdracht)
        if m:
            k = zoek_kind(doel, "onderdeel", m[2])[1]
            pas_tekst_toe([(k, "tekst")], "slot", m[1], m[3], False, herkomst)
        if soort == "toevoegen":
            doel["kinderen"] += nieuw
        else:
            nr = nummer(nieuw[0]["nr"])
            i = next((i for i, k in enumerate(doel["kinderen"])
                      if k["soort"] == nieuw[0]["soort"] and nummer(k["nr"]) == nr), None)
            # Een lid met een eigen nummer ('Na het tweede lid wordt een lid
            # ingevoegd, luidende: 2a. ...') schuift niets op; het komt na het
            # lid of onderdeel dat de opdracht noemt.
            na = re.search(rf"\b(?:Na|na|Achter|achter) (?:het|de) ({RANG}) lid\b", opdracht)
            na_o = re.search(r"\b(?:Na|na|Achter|achter) onderdeel ([a-z]\d?)\b", opdracht)
            eigen_nr = i is None and (na or na_o)
            if eigen_nr:
                vorig = (str(RANGTELWOORDEN[na[1].lower()]) if na else na_o[1])
                j = next((j for j, k in enumerate(doel["kinderen"])
                          if k["soort"] == nieuw[0]["soort"] and nummer(k["nr"]) == vorig), None)
                if j is None:
                    raise Fout(f"geen {nieuw[0]['soort']} {vorig} om na in te voegen")
                i = j + 1
            if i is None:
                raise Fout(f"geen {nieuw[0]['soort']} {nr} om vóór in te voegen")
            voeg_in(doel["kinderen"], i, nieuw)
        if vernummerd or (soort == "lid_invoegen" and not (soort != "toevoegen" and eigen_nr)):
            hernummer(doel, nieuw[0]["soort"])
        doel["herkomst"].append(herkomst)
        return

    raise Fout(f"soort '{soort}' wordt (nog) niet afgespeeld")


def structuuradres(t):
    return {k.lower(): v for k, v in re.findall(r"\b(hoofdstuk|paragraaf|deel) (\d+[a-z]?)", t, re.I)}


def zoek_kop(wet, adres):
    """Een kop in de platte lijst; een paragraaf wordt binnen haar hoofdstuk gezocht."""
    begin, eind = 0, len(wet)
    if "hoofdstuk" in adres:
        begin = next((i for i, k in enumerate(wet) if k["soort"] == "hoofdstuk" and k["nr"] == adres["hoofdstuk"]), None)
        if begin is None:
            raise Fout(f"hoofdstuk {adres['hoofdstuk']} bestaat niet")
        if "paragraaf" not in adres:
            return wet[begin]
        eind = einde_van(wet, "hoofdstuk", adres["hoofdstuk"])
    for soort in ("paragraaf", "deel"):
        if soort in adres:
            for k in wet[begin:eind]:
                if k["soort"] == soort and k["nr"] == adres[soort]:
                    return k
            raise Fout(f"{soort} {adres[soort]} bestaat niet")
    raise Fout(f"geen kop in {adres}")


def verval_structuur(wet, adres, vernummerd, herkomst):
    """'Hoofdstuk 4, paragraaf 1, vervalt onder vernummering van de paragrafen
    2, 3 en 5 tot de paragrafen 1 tot en met 4': de kop en wat eronder staat."""
    kop = zoek_kop(wet, adres)
    i = next(i for i, k in enumerate(wet) if k is kop)
    rang = ["deel", "hoofdstuk", "paragraaf"]
    j = next((j for j in range(i + 1, len(wet)) if wet[j]["soort"] in rang[: rang.index(kop["soort"]) + 1]), len(wet))
    weg = [k["nr"] for k in wet[i + 1:j] if k["soort"] == "artikel"]
    if weg:
        # Een paragraaf die vervalt, neemt haar artikelen mee (de bron toont
        # ze als '[Artikel 38 vervallen]').
        AANNAMES.append(f"met {kop['soort']} {kop['nr']} vervallen ook artikel(en) {', '.join(weg)}")
    del wet[i:j]
    if vernummerd and kop["soort"] == "paragraaf":
        eind = einde_van(wet, "hoofdstuk", adres["hoofdstuk"])
        begin = next(i for i, k in enumerate(wet) if k["soort"] == "hoofdstuk" and k["nr"] == adres["hoofdstuk"])
        n = 0
        for k in wet[begin:eind]:
            if k["soort"] == "paragraaf":
                n += 1
                k["nr"] = str(n)
                k["herkomst"].append(herkomst)


def einde_van(wet, soort, nr):
    rang = ["deel", "hoofdstuk", "paragraaf"]
    begin = next((i for i, k in enumerate(wet) if k["soort"] == soort and k["nr"] == nr), None)
    if begin is None:
        raise Fout(f"{soort} {nr} bestaat niet")
    for i in range(begin + 1, len(wet)):
        if wet[i]["soort"] in rang[: rang.index(soort) + 1]:
            return i
    return len(wet)


# ---------------------------------------------------------------- weergave

def regels_van(k):
    if k["soort"] == "begrip":
        uit = [f"{k['nr']}: {k['tekst']}".strip()]
    elif k["soort"] in ("lid", "onderdeel"):
        uit = [f"{k['nr']} {k['tekst']}".strip()]
    else:
        uit = [k["tekst"]] if k["tekst"] else []
    for c in k["kinderen"]:
        uit += regels_van(c)
    return uit


def artikeltekst(a):
    return [f"Artikel {a['nr']}. {a['titel']}".strip()] + [r for c in a["kinderen"] for r in regels_van(c)]


def weergave(wet):
    uit = []
    for k in wet:
        if k["soort"] == "artikel":
            uit += artikeltekst(k) + [""]
        else:
            uit += [f"{k['soort'].capitalize()} {k['nr']}. {k['titel']}".strip(), ""]
    return "\n".join(uit)


# ---------------------------------------------------------------- tijdlijn

class Opname:
    """Legt na elke handeling vast welke blokken (artikelen en koppen) zij
    veranderde, met per regel de keten van stukken waar die regel vandaan
    komt. Daaruit speelt de wetsgeschiedenis de tekst stap voor stap af.

    Een blok houdt zijn sleutel zolang de knoop dezelfde is: een artikel dat
    wordt vernummerd blijft hetzelfde blok, een artikel dat geheel komt te
    luiden is een nieuw."""

    def __init__(self, wet):
        self.sleutels, self.refs, self.refnr, self.bewaard = {}, [], {}, []
        self.vorig, self.volgorde = self._neem(wet)
        self.begin = {"volgorde": self.volgorde, "blokken": dict(self.vorig)}
        self.ident = {k: k for k, b in self.vorig.items() if b["soort"] == "artikel"}
        self.stappen = []

    def _ref(self, x):
        r = (x["stuk"], x.get("onderdeel"), x.get("sub"))
        if r not in self.refnr:
            self.refnr[r] = len(self.refs)
            self.refs.append({"stuk": r[0], "onderdeel": r[1], "sub": r[2]})
        return self.refnr[r]

    def _regels(self, k, diepte):
        schoon_ = lambda t: t.replace(NIEUW_OPEN, "").replace(NIEUW_DICHT, "")
        if k["soort"] == "begrip":
            t = f"{k['nr']}: {k['tekst']}".strip()
        elif k["soort"] in ("lid", "onderdeel"):
            t = f"{k['nr']} {k['tekst']}".strip()
        else:
            t = k["tekst"]
        uit = [[schoon_(t), [self._ref(x) for x in k["herkomst"]], diepte, k["soort"]]] if t else []
        for c in k["kinderen"]:
            uit += self._regels(c, diepte + (k["soort"] in ("lid", "onderdeel")))
        return uit

    def _blok(self, k):
        refs = [self._ref(x) for x in k["herkomst"]]
        titel = k["titel"].replace(NIEUW_OPEN, "").replace(NIEUW_DICHT, "")
        if k["soort"] == "artikel":
            regels = [[f"Artikel {k['nr']}. {titel}".strip(), refs, 0, "kop"]]
            regels += [r for c in k["kinderen"] for r in self._regels(c, 0)]
        else:
            regels = [[f"{k['soort'].capitalize()} {k['nr']}. {titel}".strip(), refs, 0, k["soort"]]]
        return {"soort": k["soort"], "nr": k["nr"], "regels": regels}

    def _neem(self, wet):
        blokken, volgorde = {}, []
        for k in wet:
            if id(k) not in self.sleutels:
                # De knoop blijft vastgehouden: een vrijgekomen id zou anders
                # bij een nieuwe knoop de sleutel van een oude erven.
                self.bewaard.append(k)
            s = self.sleutels.setdefault(id(k), f"b{len(self.sleutels)}")
            blokken[s] = self._blok(k)
            volgorde.append(s)
        return blokken, volgorde

    def stap(self, wet, **wat):
        nu, volgorde = self._neem(wet)
        wat["wijzig"] = {s: b for s, b in nu.items() if self.vorig.get(s) != b}
        wat["weg"] = [s for s in self.vorig if s not in nu]
        if volgorde != self.volgorde:
            wat["volgorde"] = volgorde
        # Een artikel dat 'komt te luiden' is een nieuwe knoop met hetzelfde
        # nummer; het blijft hetzelfde artikel. Een vernummerd artikel houdt
        # zijn knoop en dus zijn sleutel.
        weg_nr = {self.vorig[k]["nr"]: k for k in wat["weg"] if self.vorig[k]["soort"] == "artikel"}
        for k, b in nu.items():
            if k not in self.ident and b["soort"] == "artikel":
                self.ident[k] = self.ident.get(weg_nr.get(b["nr"]), k)
        self.vorig, self.volgorde = nu, volgorde
        self.stappen.append(wat)

    def identiteit(self):
        """{sleutel: artikel}: elke sleutel van een artikelblok, met de
        sleutel waarmee dat artikel begon."""
        return dict(self.ident)


def norm(t):
    t = schoon(t)
    t = re.sub(r"[‘’']", "'", t)
    return " ".join(t.split())


KOP_BRON = re.compile(r"^(?:Artikel (?P<nr>\d+[a-z]*)\.(?: (?P<titel>[A-Z].*))?$|Deel \d|DEEL|Hoofdstuk \d|HOOFDSTUK|§ |\[|Lasten en bevelen)")


def lees_bron(pad):
    """De doorgewerkte bron per artikel: kop en regels tot de volgende kop."""
    arts, huidig = {}, None
    for r in Path(pad).read_text(encoding="utf-8").splitlines():
        m = KOP_BRON.match(r)
        if m and m["nr"] and huidig is not None and huidig[0] == "_wijzig":
            pass
        if m:
            if m["nr"]:
                huidig = [m["nr"], [f"Artikel {m['nr']}. {m['titel'] or ''}".strip()]]
                arts.setdefault(m["nr"], huidig[1])
            else:
                huidig = None
            continue
        if huidig and r.strip():
            huidig[1].append(r.strip())
    return arts


def diff_kort(a, b):
    """Het eerste punt waar twee teksten uiteenlopen, met wat context."""
    i = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
    return f"…{a[max(0, i - 50):i + 70]}…\n      bron:      …{b[max(0, i - 50):i + 70]}…"


# ---------------------------------------------------------------- volgorde

def alle_knopen(wet):
    stapel, uit = list(wet), []
    while stapel:
        k = stapel.pop()
        uit.append(k)
        stapel += k["kinderen"]
    return uit


def bind_veegacties(wet, paren):
    """Een veegactie adresseert, net als haar citaat, de stand van vóór de nota
    ('45, eerste lid' terwijl III dat lid ontnummert). Leg haar doelen vast
    voordat er iets verandert; een knoop houdt zijn identiteit."""
    for _, h in paren:
        if is_veeg(h):
            h["_gebonden"] = []
            for d in h["adres"]["doelen"]:
                try:
                    h["_gebonden"].append(bereik(wet, d)[1])
                except Fout as e:
                    h["_gebonden"].append(e)


def is_veeg(h):
    """Een veegactie: dezelfde tekstbewerking over meer artikelen."""
    d = (h["adres"] or {}).get("doelen", [])
    return len({x["artikel"] for x in d}) > 1


def volgorde(stuk):
    """Een nota van wijziging leest als gelijktijdig: elk onderdeel citeert de
    tekst van vóór de nota (gemeten 12-10-2026: nr. 20, NN citeert 'giften' in
    art. 27 nadat S daar 'giften' al verving). Gerichte opdrachten gaan dus
    eerst, de veegacties over meer artikelen daarna."""
    paren = [(o, h) for o in stuk["onderdelen"] for h in o["handelingen"]]
    return [p for p in paren if not is_veeg(p[1])] + [p for p in paren if is_veeg(p[1])]


def subnummers(spec):
    """'4 en 5' / '15 t/m 17' / '1 tot en met 4' -> {4, 5} / {15, 16, 17} / {1, 2, 3, 4}."""
    uit = set()
    for van, tot, los in re.findall(r"(\d+)\s*(?:t/m|tot en met)\s*(\d+)|(\d+)", spec):
        uit |= set(range(int(van), int(tot) + 1)) if van else {int(los)}
    return uit


def kies_toelichting(koppen, letter, sub):
    """De toelichtingskop die precies dit (sub)onderdeel noemt: 'NN onder 4'
    hoort bij de kop die 'NN onder 4' noemt, niet bij die met 'NN onder 3'."""
    if not letter or not koppen:
        return koppen
    passend = []
    for kop in koppen:
        sleutels = kop.split("(art.")[0]
        for m in re.finditer(rf"\b{letter}\b(?: onder (?P<s>[\d ,t/men]+?(?=,? [A-Z]|\s*$| \())?)?", sleutels):
            if not m["s"] or (sub and sub in subnummers(m["s"])):
                passend.append(kop)
                break
    return passend or koppen


GROEPEN = {
    "nota": "een handeling van de nota is niet afgespeeld (gebrek in de nota); de bron volgde de bedoeling",
    "aanname": "afgespeeld met een aanname; de bron koos misschien een andere lezing",
    "bron": "geen nota raakt dit artikel; de bron wijkt af van het voorstel zelf",
    "controle": "alle handelingen zijn afgespeeld en toch verschilt het: de bron mist een wijziging, of de afspeler leest verkeerd",
    "structuur": "een artikel staat aan één kant",
}


def woorddiff(x, y):
    """De woorden waarin afgespeeld (−) en bron (+) verschillen, met wat context."""
    import difflib
    a, b = x.split(), y.split()
    uit = []
    for op, i1, i2, j1, j2 in difflib.SequenceMatcher(None, a, b, autojunk=False).get_opcodes():
        if op == "equal":
            continue
        ctx = " ".join(a[max(0, i1 - 4):i1])
        uit.append(f"      …{ctx} [− {' '.join(a[i1:i2])}] [+ {' '.join(b[j1:j2])}]")
    return "\n".join(uit[:6]) + (f"\n      … en nog {len(uit) - 6}" if len(uit) > 6 else "")


# ---------------------------------------------------------------- het dossier

def lees_dossier(map_):
    """dossier.yaml naast de stukken: wat dit dossier is en waar zijn ijkpunten
    staan. Zonder dat bestand: een nieuwe wet, met het voorstel als nr. 2."""
    pad = Path(map_) / "dossier.yaml"
    cfg = yaml.safe_load(pad.read_text(encoding="utf-8")) if pad.exists() else {}
    cfg.setdefault("voorstel", "2")
    cfg.setdefault("doelwetten", [])
    return cfg


def bronpad(map_, pad):
    return (Path(map_) / pad).resolve() if pad else None


def stuklijst(map_):
    """stukken.json als lijst, in de volgorde van de Kamers: eerst de Tweede
    Kamer op ondernummer, dan de Eerste Kamer op letter."""
    lijst = json.loads((Path(map_) / "stukken.json").read_text(encoding="utf-8"))
    return [(n, xs[0]) for n, xs in lijst.items()]


def besluiten(map_):
    pad = Path(map_) / "besluiten.json"
    return json.loads(pad.read_text(encoding="utf-8")) if pad.exists() else {}


def speelplan(stukken, uitslagen):
    """Wat afspeelt en wanneer: de nota's van wijziging op hun datum, de
    aangenomen amendementen op de datum van de stemming, op ondernummer. Elk
    ander stuk krijgt een reden waarom het niet meespeelt."""
    plan, niet = [], {}
    vervangen = {s["vervangt"]: s["stuk"].rsplit("-", 1)[1] for s in stukken if s.get("vervangt")}
    for s in stukken:
        nr = s["stuk"].rsplit("-", 1)[1]
        if s["soort"].startswith(NOTA):
            plan.append(((s["datum"] or "")[:10], 0, int(nr), s))
        elif s["soort"].startswith(AMENDEMENT):
            b = uitslagen.get(nr)
            if b and b["uitslag"] == "aangenomen":
                s["moment"] = b["datum"]
                plan.append((b["datum"], 1, int(nr), s))
            elif b:
                niet[s["stuk"]] = f"{b['uitslag']} op {b['datum']}"
            elif int(nr) in vervangen:
                niet[s["stuk"]] = f"vervangen door nr. {vervangen[int(nr)]}"
            else:
                niet[s["stuk"]] = "er is (nog) niet over gestemd"
    plan.sort(key=lambda p: p[:3])
    return [p[3] for p in plan], niet


def ijkpunten(map_, cfg, dossier, stukken_json, bron=None):
    """De officiële teksten waaraan de afspeling wordt gemeten, in de tijd:
    het gewijzigd voorstel van de Eerste Kamer, het Staatsblad, de BWB; en
    als die er niet zijn de doorgewerkte bron. Elk als {nr: regels}."""
    uit = []
    for n, x in stukken_json:
        if x.get("kamer") == "Eerste Kamer" and (x.get("soort") or "").startswith(GEWIJZIGD_VOORSTEL):
            pad = Path(map_) / f"kst-{dossier}-{n}.xml"
            if pad.exists():
                uit.append({"naam": f"Gewijzigd voorstel (nr. {n})", "soort": "tweede-kamer",
                            "uitleg": "de tekst zoals de Tweede Kamer haar aannam, als stuk van de Eerste Kamer",
                            "datum": (x.get("datum") or "")[:10] or None, "pad": pad})
    for e in cfg.get("ijkpunten") or []:
        uit.append(dict(e, pad=bronpad(map_, e["pad"])))
    if bron:
        # Een andere doorgewerkte bron dan het dossier noemt (de poort muteert haar).
        uit = [x for x in uit if x["soort"] != "doorgewerkt"] + [
            {"naam": "Doorgewerkte bron", "soort": "doorgewerkt", "uitleg": "", "pad": Path(bron)}]
    for x in uit:
        if x["soort"] == "doorgewerkt":
            x["artikelen"] = lees_bron(x["pad"])
        else:
            w = lees_wettekst(x["pad"], x.get("stuk") or x["pad"].stem)
            x["artikelen"] = {k["nr"]: artikeltekst(k) for k in w if k["soort"] == "artikel"}
    return uit


def vergelijk(eigen, artikelen, raak):
    """Per artikel: gelijk, of in welke groep het verschil valt."""
    groepen = {g: [] for g in GROEPEN}
    gelijk, verschillend = 0, {}
    for nr in list(eigen) + [n for n in artikelen if n not in eigen]:
        if nr not in artikelen:
            groepen["structuur"].append(f"Artikel {nr}: staat na afspelen in de wet, niet in het ijkpunt")
            verschillend[nr] = "structuur"
            continue
        if nr not in eigen:
            groepen["structuur"].append(f"Artikel {nr}: staat in het ijkpunt, niet na afspelen")
            verschillend[nr] = "structuur"
            continue
        x, y = norm(" ".join(artikeltekst(eigen[nr]))), norm(" ".join(artikelen[nr]))
        if x == y:
            gelijk += 1
            continue
        geraakt = raak.get(nr, [])
        g = ("nota" if any(r.endswith("NIET") for r in geraakt) else
             "aanname" if any(r.endswith("AANN") for r in geraakt) else
             "bron" if not geraakt else "controle")
        verschillend[nr] = g
        groepen[g].append(f"Artikel {nr}  [{', '.join(geraakt) or 'door geen stuk geraakt'}]\n" + woorddiff(x, y))
    return gelijk, verschillend, groepen


# ---------------------------------------------------------------- doorwerking

def wijzigingsonderdelen(pad, titels):
    """De onderdelen van het wijzigingsartikel dat `titel` wijzigt ('De
    Archiefwet 1995 wordt als volgt gewijzigd:'), elk als de paragrafen van
    een opdracht: haar eigen zin, dan haar genummerde wijzigingen en de nieuwe
    tekst eronder. Een wijzigingsartikel zonder onderdelen is zelf één."""
    r = moderniseer(ET.parse(pad).getroot())
    for wa in r.iter("wijzig-artikel"):
        wat = wa.find("wat")
        kop = wa.find("kop")
        if wat is None:
            continue
        w, titel = txt(wat), txt(kop.find("titel")) if kop is not None and kop.find("titel") is not None else ""
        # 'De Archiefwet 1995 wordt ...', maar ook 'Het Besluit zorgverzekering BES
        # wordt ...' (een besluit of regeling heeft 'het'; gemeten 20-10-2026). Een
        # langere naam is een andere regeling: 'Besluit zorgverzekering' is niet
        # 'Besluit zorgverzekering BES'.
        einde = r"(?=\s|$|,)(?!\s+(?:[A-Z]{2,}|\d{4})\b)"
        geheel = any(re.match(rf"^(?:In )?(?:[Dd]e|[Hh]et) {re.escape(t)}{einde}", w) for t in titels)
        # 'Artikel 3.2 Archiefwet 1995: In artikel 2a van de Archiefwet 1995 wordt ...':
        # één opdracht in een eigen artikel dat de wet noemt.
        los = not geheel and any(re.search(rf"\bvan (?:de|het) {re.escape(t)}{einde}", w) or titel == t for t in titels)
        if not (geheel or los):
            continue
        nr = re.sub(r"^ARTIKEL\s+", "", txt(kop.find("nr"))).rstrip(".") if kop is not None and kop.find("nr") is not None else ""
        leden = wa.findall("wijzig-lid")
        if not leden:
            return nr, [{"letter": None, "paragrafen": [w] + [t for c in wa if c.tag not in ("kop", "wat")
                                                               for t in regels_xml(c)]}]
        uit = []
        for wl in leden:
            letter = txt(wl.find("lidnr")) if wl.find("lidnr") is not None else ""
            par = [txt(wl.find("wat"))] if wl.find("wat") is not None else []
            for c in wl:
                if c.tag not in ("lidnr", "wat"):
                    par += regels_xml(c)
            uit.append({"letter": letter, "paragrafen": par})
        return nr, uit
    return None, []


def doorwerk(doel, versies, map_):
    """Elke officiële versie van het voorstel, doorgewerkt in de doelwet. Geeft
    per versie de handelingen met hun uitkomst en de artikelen die
    veranderden, met de tekst van ervoor en erna."""
    basis = lees_wettekst(bronpad(map_, doel["bron"]), doel.get("bwb") or "doelwet")
    basisregels = {k["nr"]: artikeltekst(k) for k in basis if k["soort"] == "artikel"}
    uit = []
    for v in versies:
        artnr, onderdelen = wijzigingsonderdelen(v["pad"], [doel["titel"]] + list(doel.get("ook_genoemd") or []))
        if not onderdelen:
            continue
        wet = copy.deepcopy(basis)
        handelingen, tel = [], {"afgespeeld": 0, "aanname": 0, "niet": 0}
        for o in onderdelen:
            for h in splits_onderdeel(o["paragrafen"]):
                herkomst = {"stuk": v["stuk"], "onderdeel": o["letter"], "artikel": artnr}
                if "subnummer" in h:
                    herkomst["sub"] = h["subnummer"]
                AANNAMES.clear()
                try:
                    if h["soort"] is None:
                        raise Fout("de opdracht is niet te lezen")
                    if h["soort"] == "samengesteld":
                        raise Fout("een samengestelde opdracht zonder te lezen onderdelen")
                    speel(wet, h, herkomst)
                    status = "aanname" if AANNAMES else "ok"
                    reden = "; ".join(AANNAMES[:3])
                except Fout as e:
                    status, reden = "niet", str(e)
                tel[{"ok": "afgespeeld"}.get(status, status)] += 1
                handelingen.append({"onderdeel": o["letter"], "sub": h.get("subnummer"), "opdracht": h["opdracht"],
                                    "status": status, "reden": reden,
                                    "artikelen": sorted({d["artikel"] for d in (h["adres"] or {}).get("doelen", [])}
                                                        or {(h["adres"] or {}).get("artikel")} - {None})})
        ontmerk(wet)
        na = {k["nr"]: artikeltekst(k) for k in wet if k["soort"] == "artikel"}
        veranderd = {nr: {"voor": [norm(r) for r in basisregels.get(nr, [])], "na": [norm(r) for r in na.get(nr, [])]}
                     for nr in list(dict.fromkeys(list(basisregels) + list(na)))
                     if basisregels.get(nr) != na.get(nr)}
        uit.append({"versie": v["naam"], "stuk": v["stuk"], "datum": v.get("datum"), "artikel": artnr,
                    "tel": tel, "handelingen": handelingen, "artikelen": veranderd, "_regels": na})
    # Is de wijziging in werking, dan is de BWB-toestand daarna het ijkpunt van
    # de laatste versie: elk artikel moet daar woord voor woord op uitkomen.
    if doel.get("na") and uit:
        na_wet = lees_wettekst(bronpad(map_, doel["na"]["bron"]), "na")
        na_regels = {k["nr"]: artikeltekst(k) for k in na_wet if k["soort"] == "artikel"}
        wet_regels = uit[-1].pop("_regels")
        anders = sorted(nr for nr in set(na_regels) | set(wet_regels)
                        if norm(" ".join(na_regels.get(nr, []))) != norm(" ".join(wet_regels.get(nr, []))))
        uit[-1]["ijk"] = {"naam": doel["na"].get("naam") or "BWB na inwerkingtreding",
                          "gelijk": len(set(na_regels) | set(wet_regels)) - len(anders), "verschillend": anders,
                          "artikelen": {nr: [norm(r) for r in na_regels.get(nr, [])] for nr in anders}}
    for v in uit:
        v.pop("_regels", None)
    nummers = list(dict.fromkeys(list(basisregels) + [nr for v in uit for nr in v["artikelen"]]))
    return {"titel": doel["titel"], "bwb": doel.get("bwb"), "basis": doel.get("basisnaam") or doel["bron"],
            "nummers": nummers, "versies": uit}


# ---------------------------------------------------------------- hoofdlijn

def main(map_, handelingen, bron=None, uit=None, stil=False):
    """Speel het dossier in `map_` af. `bron` vervangt de doorgewerkte bron uit
    dossier.yaml (de afspeelpoort muteert haar om te zien dat de vergelijking
    het merkt)."""
    map_, uit = Path(map_), Path(uit)
    uit.mkdir(parents=True, exist_ok=True)
    stukken = (handelingen if isinstance(handelingen, list)
               else json.loads(Path(handelingen).read_text(encoding="utf-8")))
    cfg = lees_dossier(map_)
    lijst = stuklijst(map_)
    dossier = str(cfg.get("dossier") or next(map_.glob("kst-*-*.xml")).name.split("-")[1])
    vnr = str(cfg["voorstel"])
    voorstel = f"kst-{dossier}-{vnr}"
    wet = lees_voorstel(map_ / f"{voorstel}.xml", voorstel)
    (uit / f"stand-{voorstel}.txt").write_text(weergave(wet), encoding="utf-8")
    opname = Opname(wet)
    uitslagen = besluiten(map_)
    plan, niet_mee = speelplan(stukken, uitslagen)

    log, raak, statussen = [], {}, {}
    tel = {"afgespeeld": 0, "niet": 0, "aanname": 0}
    for s in stukken:
        if s not in plan:
            log.append(f"{s['stuk']} ({s['soort']}): speelt niet mee ({niet_mee.get(s['stuk'], 'geen wijzigend stuk')})")
    for s in plan:
        paren = volgorde(s)
        bind_veegacties(wet, paren)
        uitgesteld, regels_stuk = [], {}
        for ronde, reeks in ((1, paren), (2, None)):
            for o, h in (reeks if reeks is not None else uitgesteld):
                herkomst = {"stuk": s["stuk"], "onderdeel": o["letter"]}
                if "subnummer" in h:
                    herkomst["sub"] = h["subnummer"]
                waar = f"{s['stuk']} {o['letter']}{' ' + str(h['subnummer']) if 'subnummer' in h else ''}"
                AANNAMES.clear()
                try:
                    if h["soort"] is None:
                        raise Fout("de opdracht is niet te lezen")
                    speel(wet, h, herkomst)
                    if ronde == 2:
                        AANNAMES.insert(0, "citeert de tekst zoals een ander onderdeel van deze nota die maakt")
                    status = "AANNAME: " + "; ".join(AANNAMES[:3]) if AANNAMES else "ok"
                except Fout as e:
                    if ronde == 1 and "komt niet voor" in str(e):
                        # Misschien citeert dit onderdeel de tekst van ná een
                        # ander onderdeel (nr. 20, LL); aan het eind nog eens.
                        uitgesteld.append((o, h))
                        continue
                    status = f"NIET: {e}"
                regels_stuk[id(h)] = (waar, h, status)
                opname.stap(wet, stuk=s["stuk"], onderdeel=o["letter"], sub=h.get("subnummer"),
                            soort=h["soort"], opdracht=h["opdracht"], tekst=h.get("tekst") or [],
                            status=status.split(":")[0].lower(), reden=status.partition(": ")[2],
                            ronde=ronde, veeg=is_veeg(h),
                            toelichting=kies_toelichting(o.get("toelichting", []), o["letter"], h.get("subnummer")))
        ontmerk(wet)
        for o, h in paren:
            waar, h, status = regels_stuk[id(h)]
            soort = "aanname" if status.startswith("AANNAME") else "niet" if status.startswith("NIET") else "afgespeeld"
            tel[soort] = tel.get(soort, 0) + 1
            a = h["adres"] or {}
            for nr in {d["artikel"] for d in a.get("doelen", [])} or [a.get("artikel")]:
                if nr:
                    raak.setdefault(nr, []).append(f"{waar} {status[:4]}")
            h.pop("_gebonden", None)
            log.append(f"{waar:28s} {h['soort'] or '?':20s} {status}  | {h['opdracht'][:110]}")
            statussen[waar + " | " + h["opdracht"][:60]] = soort
        (uit / f"stand-{s['stuk']}.txt").write_text(weergave(wet), encoding="utf-8")

    (uit / "afspeellog.txt").write_text(
        f"afgespeeld {tel['afgespeeld']}, met aanname {tel.get('aanname', 0)}, niet afgespeeld {tel['niet']}\n\n" + "\n".join(log) + "\n",
        encoding="utf-8")

    # De ijkpunten; de vergelijking per artikel is tegen het laatste, het
    # eindpunt, ingedeeld naar wat het verschil waarschijnlijk is. De indeling
    # is een wegwijzer, geen oordeel.
    ijk = ijkpunten(map_, cfg, dossier, lijst, bron)
    eigen = {k["nr"]: k for k in wet if k["soort"] == "artikel"}
    for x in ijk:
        x["gelijk"], x["verschillend"], _ = vergelijk(eigen, x["artikelen"], raak)
    eind = ijk[-1] if ijk else None
    if eind:
        gelijk, verschillend, groepen = vergelijk(eigen, eind["artikelen"], raak)
    else:
        gelijk, verschillend, groepen = 0, {}, {g: [] for g in GROEPEN}
    kop = (f"{gelijk} artikelen gelijk aan {eind['naam']}, {sum(len(v) for v in groepen.values())} verschillend"
           if eind else "geen ijkpunt: niets om de afspeling aan te meten")
    regels = [kop, ""]
    for x in ijk[:-1]:
        regels.append(f"ook gemeten: {x['naam']}: {x['gelijk']} gelijk, {len(x['verschillend'])} verschillend")
    regels.append("")
    for g, uitleg in GROEPEN.items():
        regels += [f"== {uitleg} ({len(groepen[g])})", ""] + [r + "\n" for r in groepen[g]]
    (uit / "vergelijking.txt").write_text("\n".join(regels) + "\n", encoding="utf-8")

    # Herkomst: per artikel en per lid de stukken waar de tekst vandaan komt,
    # met de toelichting van het onderdeel dat haar zo maakte.
    stukinfo = {s["stuk"]: s for s in stukken}
    stukinfo[voorstel] = {"soort": "Voorstel van wet", "datum": None, "onderdelen": []}

    def verrijk(hk):
        uit_ = []
        for x in hk:
            s = stukinfo.get(x["stuk"], {})
            o = next((o for o in s.get("onderdelen", []) if o["letter"] == x.get("onderdeel")), None)
            uit_.append(dict(x, soort=s.get("soort"), datum=(s.get("datum") or "")[:10] or None,
                             toelichting=kies_toelichting((o or {}).get("toelichting", []),
                                                          x.get("onderdeel"), x.get("sub"))))
        return uit_

    herkomst = {k["nr"]: {
        "titel": k["titel"],
        "artikel": verrijk(k["herkomst"]),
        "delen": [{"nr": c["nr"], "soort": c["soort"], "herkomst": verrijk(c["herkomst"])}
                  for c in k["kinderen"]],
    } for k in wet if k["soort"] == "artikel"}
    (uit / "herkomst.json").write_text(json.dumps(herkomst, ensure_ascii=False, indent=1), encoding="utf-8")

    def keten(hk):
        return " → ".join(
            f"{x['stuk']}{' ' + x['onderdeel'] if x.get('onderdeel') else ''}"
            f"{' onder ' + str(x['sub']) if x.get('sub') else ''}"
            + (f" ({x['toelichting'][0][:70]})" if x.get("toelichting") else "") for x in hk)

    regels = []
    for nr, a in herkomst.items():
        regels.append(f"Artikel {nr}. {a['titel']}")
        regels.append(f"  artikel: {keten(a['artikel'])}")
        for d in a["delen"]:
            if len(d["herkomst"]) > 1 or d["herkomst"][0]["stuk"] != voorstel:
                regels.append(f"  {d['soort']} {d['nr']}: {keten(d['herkomst'])}")
        regels.append("")
    (uit / "herkomst.txt").write_text("\n".join(regels), encoding="utf-8")

    # De doorwerking van een wijzigingswet: elke officiële versie van het
    # voorstel in de wet die het wijzigt.
    versies = [{"naam": f"Voorstel van wet (nr. {vnr})", "stuk": voorstel, "pad": map_ / f"{voorstel}.xml",
                "datum": next(((x.get("datum") or "")[:10] for n, x in lijst if n == vnr), None)}]
    versies += [{"naam": x["naam"], "stuk": x["pad"].stem, "pad": x["pad"], "datum": x.get("datum")}
                for x in ijk if x["soort"] == "tweede-kamer"]
    doorwerking = [doorwerk(d, versies, map_) for d in cfg["doelwetten"]]
    if doorwerking:
        regels = []
        for d in doorwerking:
            for v in d["versies"]:
                regels.append(f"== {d['titel']}, volgens {v['versie']}: {v['tel']}")
                regels += [f"  {h['onderdeel'] or ''}{' ' + str(h['sub']) if h['sub'] else ''}: {h['status']}"
                           f"{' (' + h['reden'] + ')' if h['reden'] else ''} | {h['opdracht'][:110]}"
                           for h in v["handelingen"]]
                regels.append("")
        (uit / "doorwerking.txt").write_text("\n".join(regels) + "\n", encoding="utf-8")

    # De tijdlijn: de begintekst en per handeling wat zij veranderde, en de
    # momenten waarop iets gebeurt, voor de wetsgeschiedenis
    # (generatoren/bouw-wetsgeschiedenis.py). De toelichting staat er eenmaal
    # per kop in; een stap noemt alleen haar koppen.
    toelichtingen = {}
    for s in stukken:
        for o in s["onderdelen"]:
            for t in o.get("toelichtingstekst", []):
                toelichtingen.setdefault(t["kop"], t["tekst"])
    plan_ids = {s["stuk"] for s in plan}
    reeks = {}
    for i_, st in enumerate(opname.stappen, 1):
        reeks.setdefault(st["stuk"], [i_, i_])[1] = i_
    stemmingen = {}
    for s in plan:
        if s.get("moment"):
            stemmingen.setdefault(s["moment"], []).append(s["stuk"])
    items = []
    for volg, (n, x) in enumerate(lijst):
        stuk = f"kst-{dossier}-{n}"
        m = {"soort": "stuk", "stuk": stuk, "datum": (x.get("datum") or "")[:10] or None}
        if stuk in reeks and not any(stuk in v for v in stemmingen.values()):
            m["stappen"] = reeks[stuk]
        items.append((m["datum"] or "9999", 1, volg, m))
    for d, ss in stemmingen.items():
        r = [reeks[x] for x in ss if x in reeks]
        m = {"soort": "stemming", "datum": d, "stukken": ss,
             "voorstel": (uitslagen.get(vnr) or {}).get("uitslag") if (uitslagen.get(vnr) or {}).get("datum") == d else None}
        if r:
            m["stappen"] = [min(a for a, _ in r), max(b for _, b in r)]
        items.append((d, 0, 0, m))
    if (uitslagen.get(vnr) or {}).get("datum") and uitslagen[vnr]["datum"] not in stemmingen:
        items.append((uitslagen[vnr]["datum"], 0, 0, {"soort": "stemming", "datum": uitslagen[vnr]["datum"],
                                                      "stukken": [], "voorstel": uitslagen[vnr]["uitslag"]}))
    for x in ijk:
        if x["soort"] not in ("tweede-kamer", "doorgewerkt") and x.get("datum"):
            items.append((x["datum"], 2, 0, {"soort": "ijkpunt", "datum": x["datum"], "naam": x["naam"]}))
    items.sort(key=lambda t: t[:3])

    tijdlijn = {
        "dossier": dossier,
        "naam": cfg.get("naam") or dossier,
        "soort": cfg.get("soort") or "nieuwe wet",
        "deze_wet": list(cfg.get("deze_wet") or []),
        "voorstel": voorstel,
        "stukken": [dict(ondernummer=n, volgorde=volg, stuk=f"kst-{dossier}-{n}", soort=x["soort"],
                         titel=x.get("onderwerp") or x.get("titel") or x["soort"],
                         datum=(x.get("datum") or "")[:10] or None, kamer=x.get("kamer") or "Tweede Kamer",
                         speelt_mee=f"kst-{dossier}-{n}" in plan_ids or n == vnr,
                         uitslag=uitslagen.get(n), niet_mee=niet_mee.get(f"kst-{dossier}-{n}"))
                    for volg, (n, x) in enumerate(lijst)],
        "momenten": [m for *_, m in items],
        "refs": opname.refs,
        "begin": opname.begin,
        "stappen": opname.stappen,
        "identiteit": opname.identiteit(),
        "toelichtingen": toelichtingen,
        "groepen": GROEPEN,
        # Per artikel dat verschilt van het eindpunt: beide kanten zoals de
        # vergelijking ze leest (genormaliseerd), zodat de pagina hetzelfde
        # verschil toont.
        "bron": {nr: {"groep": g,
                      "eigen": [norm(r) for r in artikeltekst(eigen[nr])] if nr in eigen else [],
                      "bron": [norm(r) for r in (eind["artikelen"].get(nr, []) if eind else [])]}
                 for nr, g in verschillend.items()},
        "ijkpunten": [{"naam": x["naam"], "soort": x["soort"], "uitleg": x.get("uitleg") or "",
                       "datum": x.get("datum"), "gelijk": x["gelijk"], "verschillend": len(x["verschillend"]),
                       "artikelen": {nr: [norm(r) for r in rr] for nr, rr in x["artikelen"].items()}}
                      for x in ijk],
        "doorwerking": doorwerking,
        "gelijk": gelijk,
        "tel": tel,
    }
    (uit / "tijdlijn.json").write_text(json.dumps(tijdlijn, ensure_ascii=False, separators=(",", ":")),
                                       encoding="utf-8")

    if stil:
        return {"tel": tel, "gelijk": gelijk, "verschillend": verschillend, "statussen": statussen,
                "ijkpunten": [(x["naam"], x["gelijk"], len(x["verschillend"])) for x in ijk],
                "doorwerking": [(d["titel"], v["versie"], v["tel"]) for d in doorwerking for v in d["versies"]]}
    print(f"afgespeeld {tel['afgespeeld']}, met aanname {tel.get('aanname', 0)}, niet afgespeeld {tel['niet']}")
    print(kop + " — " + ", ".join(f"{g} {len(v)}" for g, v in groepen.items()))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], uit=sys.argv[3])
