"""Lees de wijzigende Kamerstukken van een wetsvoorstel als afspeelbare handelingen.

Een nota van wijziging of amendement is een reeks opdrachten op de tekst
("In artikel 14, tweede lid, wordt «x» vervangen door «y»"). Dit script zet
elke opdracht om in een handeling met een soort en een adres, en haakt er
de toelichting aan die bij hetzelfde onderdeel hoort. Het past nog niets
toe: eerst moet blijken dat elke opdracht te lezen is.

De haken zijn de nummers die de stukken zelf dragen:
- het stuk: dossier en ondernummer (kst-36742-8);
- het onderdeel: de letter (A, B, ... AA, ... LLLLLL);
- de toelichting: in de XML de tussenkop "D (art. 27 Wpp)", in een docx de
  REF-velden die naar de bladwijzer van het onderdeel wijzen;
- het adres: artikel, lid, onderdeel, of het begrip in een definitie-artikel.

    python3 handelingen.py <kamerstukkenmap> <uitmap>

Uitvoer: handelingen.json (alle handelingen) en dekking.txt (wat niet
gelezen kon worden, per patroon). Een ongelezen opdracht is geen fout van
het stuk maar een gat in deze grammatica.
"""
import json
import re
import sys
import zipfile
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path

W = "{http://schemas.openxmlformats.org/wordprocessingml/2006/main}"

RANGTELWOORDEN = {
    w: i for i, w in enumerate(
        "eerste tweede derde vierde vijfde zesde zevende achtste negende "
        "tiende elfde twaalfde dertiende veertiende vijftiende zestiende "
        "zeventiende achttiende negentiende twintigste".split(), start=1)
}
RANG = "|".join(RANGTELWOORDEN)


def schoon(t):
    t = " ".join(t.split())
    # De stukken citeren met «», “” of ‘’; één vorm maakt de grammatica kleiner.
    return re.sub(r"[“”„]", lambda m: "«" if m.group() in "“„" else "»", t)


# ---------------------------------------------------------------- adressen

# Een artikelnummer is "27", "35a", in een wet met hoofdstuknummering "1.1"
# of "11.25" (Archiefwet 2026), in een wijzigingsartikel van een andere wet
# "G 2a", "I 7", "Pa 9" (Kieswet), en in een wijzigingswet zelf "I", "II".
ARTNR = r"(?:(?:[A-Z][a-z]? )?[0-9]+[a-z]*(?:[.:][0-9]+[a-z]*)*|[IVXL]+\b)"
LID = rf"(?:het )?(?P<lid>{RANG}) lid(?: \(nieuw\))?"
# Een onderdeel van een wijzigingsartikel draagt een hoofdletter ("artikel I,
# onderdeel B"), en daarbinnen een nummer ("onder 3").
ONDER = r"(?:onderdeel|onder) (?P<onderdeel>[A-Z]{1,7}\b|[a-z0-9]+°?)(?:,? onder (?P<onder2>\d+|[a-z])\b)?"
ADRES = re.compile(
    rf"(?:[Aa]rtikel|[Aa]rtikelen) (?P<artikel>{ARTNR})"
    rf"(?:,? {LID})?"
    rf"(?:,? {ONDER})?"
)
RELATIEF = re.compile(rf"\b{LID}(?:,? {ONDER})?|\b(?:onderdeel|onder) (?P<los>[a-z]\d?°?)\b,")
STRUCTUUR = re.compile(r"\b(?P<soort>[Hh]oofdstuk|[Pp]aragraaf|[Dd]eel|[Tt]itel|§) ?(?P<nr>[0-9]+[A-Za-z]*)")


VOOR_WERKWOORD = re.compile(r"^(.*?)\b(?:wordt|worden|vervalt|vervallen|komt|komen)\b")
DOELTOKEN = re.compile(
    rf"(?P<art>\b\d+[a-z]?(?:[.:]\d+[a-z]?)*)(?=[^\w.:]|[.:](?!\d)|$)|(?P<rang>\b(?:{RANG})\b)|(?P<aanhef>\baanhef\b)"
    r"|\b(?:onderdelen|onderdeel|onder) (?P<ond>[a-z]\d?°?(?:(?:, | en )[a-z]\d?°?\b)*)"
)


def doelen(span, artikel=None):
    """'9, eerste lid, en 65, derde lid' of 'het vierde lid, onder a en b':
    elke combinatie van artikel, lid en onderdeel die de opdracht noemt."""
    # Een onderdeel of 'aanhef' hoort bij het laatst genoemde lid: in
    # '22, tweede, derde en vierde lid, onder a' alleen bij het vierde.
    ruw, cur = [], None
    for m in DOELTOKEN.finditer(span):
        if m["art"]:
            cur = {"artikel": m["art"], "leden": []}
            ruw.append(cur)
            continue
        if cur is None:
            if artikel is None:
                continue
            cur = {"artikel": artikel, "leden": []}
            ruw.append(cur)
        if m["rang"]:
            cur["leden"].append([RANGTELWOORDEN[m["rang"].lower()], [], False])
            continue
        if not cur["leden"]:
            cur["leden"].append([None, [], False])
        if m["aanhef"]:
            cur["leden"][-1][2] = True
        else:
            cur["leden"][-1][1] += re.split(r", | en ", m["ond"])
    uit = []
    for c in ruw:
        for lid, onds, aanhef in c["leden"] or [[None, [], False]]:
            for ond in onds or [None]:
                d = {"artikel": c["artikel"]}
                if lid:
                    d["lid"] = lid
                if ond:
                    d["onderdeel"] = ond
                if aanhef and not ond:
                    d["aanhef"] = True
                uit.append(d)
            if aanhef and onds:
                uit.append(dict({"artikel": c["artikel"], "aanhef": True}, **({"lid": lid} if lid else {})))
    return uit


def adres(tekst, context=None):
    """Het adres van een opdracht. Zonder artikelnummer is het relatief:
    'het tweede lid' binnen het artikel uit de kop van het onderdeel. Noemt
    de opdracht er meer ('de artikelen 9, eerste lid, en 65, derde lid'; 'het
    vierde lid, onder a en b'), dan krijgt zij ze allemaal als doelen."""
    # Een artikelnummer tussen aanhalingstekens is geciteerde tekst, geen adres.
    tekst = re.sub(r"«[^»]*»", "«»", tekst)
    tekst = re.sub(r"(\d[a-z]?)(wordt|worden)\b", r"\1 \2", tekst)  # '80wordt' (nr. 20, U)
    reeks = re.search(r"[Dd]e artikelen (\d+) tot en met (\d+)", tekst)
    if reeks:
        return {"doelen": [{"artikel": str(n)} for n in range(int(reeks[1]), int(reeks[2]) + 1)]}
    kop = VOOR_WERKWOORD.match(tekst)
    kop = kop[1] if kop else tekst
    if re.search(r"\bartikelen\b|\b(?:%s) en (?:het )?(?:%s)\b|\bonder(?:deel|delen)? [a-z]\d?°?(?:, [a-z]\d?°?)* en [a-z]\b" % (RANG, RANG), kop):
        ds = doelen(re.sub(r"^.*?\b(?=artikel)", "", kop), (context or {}).get("artikel"))
        if len(ds) > 1:
            return {"doelen": ds}
    m = ADRES.search(tekst)
    if m:
        a = {"artikel": m["artikel"]}
        # 'In artikel 45 vervalt het tweede lid': het lid staat na het werkwoord.
        na = re.search(rf"\bverval(?:t|len) het ({RANG}) lid", tekst)
        if na and not m["lid"]:
            a["lid"] = RANGTELWOORDEN[na[1].lower()]
            return a
        # 'In artikel 8.3 komt het eerste lid te luiden:' (Archiefwet 2026, nr. 8):
        # ook hier staat het lid na het werkwoord, en zonder dat lid zou het
        # hele artikel opnieuw luiden.
        lu = re.search(rf"\b(?:komt|komen) het ({RANG}) lid te luiden", tekst)
        if lu and not m["lid"]:
            a["lid"] = RANGTELWOORDEN[lu[1].lower()]
            return a
    elif context and re.search(r"\bvervalt onderdeel [a-z]", tekst):
        # 'In het tweede lid, vervalt onderdeel a': het onderdeel staat na het werkwoord.
        a = adres(tekst.split(" vervalt")[0], context)
        a["onderdeel"] = re.search(r"\bvervalt onderdeel ([a-z]\d?)", tekst)[1]
        return a
    elif context and "doelen" in context:
        # Een onderdeel over 'de artikelen 11 en 12' met 'In het vierde lid,
        # onder c': het relatieve adres geldt in elk van die artikelen.
        uit = [adres(tekst, d) for d in context["doelen"]]
        uit = [d for u in uit for d in (u.get("doelen") or [u])]
        return {"doelen": uit} if len(uit) > 1 else uit[0]
    elif context and re.match(r"^[Oo]nderdeel ([a-z]\d?°?) (?:komt|komen|vervalt|wordt)\b", tekst):
        # 'Onderdeel d komt te luiden:' binnen 'Het eerste lid wordt als volgt gewijzigd:'.
        return dict(context, onderdeel=re.match(r"^[Oo]nderdeel ([a-z]\d?°?)", tekst)[1])
    elif context:
        a = {"artikel": context["artikel"]} if "artikel" in context else dict(context)
        m = RELATIEF.search(tekst)
        if m is None:
            return dict(context)
        if m["los"]:
            return dict(context, onderdeel=m["los"])
    else:
        st = STRUCTUUR.findall(tekst)
        return {k.lower().replace("§", "paragraaf"): v for k, v in st} or None
    if m["lid"]:
        a["lid"] = RANGTELWOORDEN[m["lid"].lower()]
    if m["onderdeel"]:
        a["onderdeel"] = m["onderdeel"]
    if m["onder2"]:
        a["onder2"] = m["onder2"]
    if re.search(r"\baanhef\b", kop) and "onderdeel" not in a:
        a["aanhef"] = True
    vz = re.search(rf"\b({RANG}) volzin", kop)
    if vz:
        a["volzin"] = RANGTELWOORDEN[vz[1].lower()]
    return a


# ---------------------------------------------------------------- grammatica
# Elk patroon: (soort, regex). De volgorde telt: het specifieke eerst.
# Alles na "luidende:" of "komt te luiden:" is de nieuwe tekst en staat in
# de vervolgparagrafen van de handeling.

CIT = r"«(?P<oud>[^»]*)»"
NIEUW = r"«(?P<nieuw>[^»]*)»"
WORDT = r"(?:wordt|worden)"
PATRONEN = [
    ("vervang_tekst", re.compile(
        rf"{WORDT} (?:in de (?:definitie|omschrijving) van (?:het begrip )?(?P<begrip>[^«]+?) )?"
        rf"{CIT} (?P<telkens>telkens )?vervangen door {NIEUW}")),
    ("vervang_tekst", re.compile(rf"(?:[Dd]e zinsnede )?{CIT} {WORDT} (?P<telkens>telkens )?vervangen door {NIEUW}")),
    # De oude schrijfwijze: "«a» vervangen door: b." (vóór 2009); de nieuwe tekst loopt
    # tot het eind van de zin.
    ("vervang_tekst", re.compile(rf"{CIT} (?P<telkens>telkens )?vervangen door: (?P<nieuw>[^«»]+?)\.?$")),
    # Met een plaats ertussen: "wordt in het voorgestelde tweede lid, onderdeel b,
    # «a» vervangen door «b»" (een wijzigingswet, of een nota op een wijzigingsartikel).
    ("vervang_tekst", re.compile(rf"{WORDT} in (?:het|de|een) [^«]{{1,120}}? {CIT} (?P<telkens>telkens )?vervangen door {NIEUW}")),
    ("vervang_leesteken", re.compile(
        r"(?:de |het )?(?P<oud>puntkomma|punt|komma|dubbele punt) aan het slot (?:van .*? )?"
        r"vervangen door (?:een|de) (?P<nieuw>puntkomma|punt|komma|dubbele punt)")),
    ("vervang_leesteken", re.compile(
        r"aan het slot van .*? de (?P<oud>puntkomma|punt|komma|dubbele punt) "
        r"vervangen door (?:een|de) (?P<nieuw>puntkomma|punt|komma|dubbele punt)")),
    ("voeg_leesteken_in", re.compile(rf"{WORDT} een (?P<nieuw>komma|punt|puntkomma) ingevoegd (?:na|voor) {CIT}")),
    ("schrap_tekst", re.compile(rf"verval(?:t|len) (?:de zinsnede |de woorden |het woord )?(?P<telkens>telkens )?{CIT}")),
    ("schrap_tekst", re.compile(rf"{WORDT} {CIT} (?P<telkens>telkens )?(?:geschrapt|verwijderd)")),
    ("voeg_tekst_in", re.compile(rf"wordt (?:na|achter|voor) {CIT} (?:telkens )?ingevoegd {NIEUW}")),
    ("voeg_tekst_in", re.compile(rf"wordt na {CIT}.*?ingevoegd")),
    ("voeg_tekst_toe", re.compile(rf"(?:Aan|aan het slot van).*?wordt (?:toegevoegd )?{NIEUW}(?: toegevoegd)?")),
    ("opschrift", re.compile(rf"[Hh]et opschrift van .* komt te luiden: {NIEUW}")),
    ("vernummer_aanduiding", re.compile(r"(?:Voor de tekst|Voor de tekst van) wordt de aanduiding «[^»]*» geplaatst")),
    ("begrip_invoegen", re.compile(r"In de alfabetische volgorde wordt(?:en)? ingevoegd")),
    ("begrip_invoegen", re.compile(r"\bin de alfabetische (?:volgorde|rangschikking) (?:een|twee|drie|vier) begripsbepaling(?:en)? (?:ingevoegd|toegevoegd)")),
    ("begrip_vervangen", re.compile(r"[Dd]e zinsnede met betrekking tot (?:de |het )?(?P<begrip>.+?) komt te luiden")),
    ("begrip_vervalt", re.compile(r"[Dd]e zinsneden? met betrekking tot (?:de |het )?(?P<begrip>.+?) verval")),
    ("begrip_wijzigen", re.compile(r"(?:[Ii]n|[Aa]an(?: het slot van)?) de zinsneden? met betrekking tot (?:de |het )?(?P<begrip>.+?)(?: wordt|,)")),
    ("lid_invoegen", re.compile(rf"(?:[Oo]nder vernummering van (?P<vernummering>.+?),? )?{WORDT}.*?(?:een|twee|drie) (?:nieuwe? )?(?:lid|leden) ingevoegd")),
    ("lid_invoegen", re.compile(r"[Oo]nder vernummering van (?P<vernummering>.+?) wordt .*?ingevoegd")),
    # "Na het eerste lid, wordt twee leden toegevoegd" (Stb. 2024, 164, zo geschreven).
    ("lid_invoegen", re.compile(rf"^Na het (?:{RANG}) lid,? {WORDT} (?:een|twee|drie) (?:nieuwe )?(?:lid|leden) toegevoegd")),
    ("onderdeel_invoegen", re.compile(r"[Oo]nder verlettering van (?P<vernummering>.+?) wordt .*?onderdeel ingevoegd")),
    ("toevoegen", re.compile(rf"(?:^|\b)(?:Aan|aan|Er) .*?{WORDT},? .*?toegevoegd")),
    ("invoegen_na", re.compile(rf"(?:Achter|Na|Voor|Vóór|na|achter) .*?{WORDT} .*?ingevoegd")),
    ("invoegen_na", re.compile(rf"{WORDT} (?:na|achter|voor|vóór) (?:artikel|hoofdstuk|paragraaf) .*?ingevoegd")),
    # "Na artikel 11.25 wordt een artikel toegevoegd, dat luidt:" (Archiefwet 2026, nr. 11)
    ("invoegen_na", re.compile(rf"^(?:Na|Achter) artikel \S+ {WORDT} (?:een|twee|drie) artikel(?:en)? toegevoegd")),
    ("vervang_geheel", re.compile(r"(?:komt|komen) te luiden")),
    # "In artikel 8.3 komt het eerste lid te luiden:"
    ("vervang_geheel", re.compile(r"(?:komt|komen) (?:het|de) .{1,60}? te luiden")),
    ("verplaats", re.compile(rf"{WORDT} .*?(?:verplaatst|vernummerd tot)")),
    ("vervalt", re.compile(r"\bverval(?:t|len)\b")),
    ("samengesteld", re.compile(rf"{WORDT} als volgt gewijzigd|{WORDT} gewijzigd als volgt")),
]


def lees_opdracht(tekst, context=None):
    t = schoon(tekst)
    for soort, rx in PATRONEN:
        m = rx.search(t)
        if m:
            h = {"soort": soort, "adres": adres(t, context), "opdracht": t}
            for k in ("oud", "nieuw", "begrip", "vernummering"):
                if k in rx.groupindex and m[k]:
                    h[k] = m[k]
            if "telkens" in rx.groupindex and m["telkens"]:
                h["telkens"] = True
            if soort == "invoegen_na":
                pos = re.search(r"\b(Achter|Na|Voor|Vóór|achter|na|voor|vóór) (?:artikel|hoofdstuk|paragraaf|§)", t)
                if pos:
                    h["positie"] = "na" if pos[1].lower() in ("achter", "na") else "voor"
            v = VERNUMMERING.search(t)
            if v and "vernummering" not in h:
                h["vernummering"] = v[1]
            return h
    return {"soort": None, "adres": adres(t, context), "opdracht": t}


VERNUMMERING = re.compile(r"onder vernummering van (.+?)(?:,| wordt| worden|\.$|$)")
SUBNR = re.compile(r"^(\d+)\. (.*)$")
LETTERNR = re.compile(r"^([a-z])\. (.*)$")


def splits_onderdeel(paragrafen):
    """Een onderdeel is één opdracht, of 'Artikel N wordt als volgt gewijzigd:'
    met genummerde subopdrachten. Een subopdracht kan zelf weer 'als volgt
    gewijzigd' worden, met opdrachten onder a, b, ... (een wijzigingswet:
    '2. Het eerste lid wordt als volgt gewijzigd: a. In de aanhef ...').
    Paragrafen die geen opdracht zijn, horen als nieuwe tekst bij de opdracht
    ervoor."""
    if not paragrafen:
        return []
    kop = lees_opdracht(paragrafen[0])
    if kop["soort"] != "samengesteld":
        return [dict(kop, tekst=paragrafen[1:] or tekst_na_dubbele_punt(kop))]
    context = kop["adres"]
    uit, huidig, verwacht, ouder, letter = [], None, 1, None, "a"
    for p in paragrafen[1:]:
        m = SUBNR.match(schoon(p))
        kandidaat = lees_opdracht(m[2], context) if m else None
        # Een genummerde regel is de volgende subopdracht als zij het volgende
        # nummer draagt en, na een 'luidende:', ook als opdracht te lezen is;
        # anders is zij nieuwe tekst ('1. ...' van een ingevoegd lid).
        if m and int(m[1]) == verwacht and (
                kandidaat["soort"] or not (huidig and huidig["soort"] in LUIDENDE)):
            huidig = dict(kandidaat, subnummer=int(m[1]), tekst=[])
            uit.append(huidig)
            verwacht += 1
            ouder, letter = (huidig, "a") if huidig["soort"] == "samengesteld" else (None, "a")
            continue
        ml = LETTERNR.match(schoon(p)) if ouder is not None else None
        if ml and ml[1] == letter:
            sub = lees_opdracht(ml[2], ouder["adres"])
            # Na een 'luidende:' kan 'b. ...' nieuwe tekst zijn; dan telt de regel
            # alleen als opdracht als zij er een is. Anders is zij de volgende
            # opdracht, ook als deze grammatica haar niet leest.
            na_luidende = huidig is not ouder and huidig is not None and (
                huidig["soort"] in LUIDENDE or huidig["opdracht"].rstrip().endswith(":"))
            if sub["soort"] or not na_luidende:
                if ouder in uit:
                    # De samengestelde opdracht zelf voert niets uit; haar
                    # onderdelen doen het werk.
                    uit.remove(ouder)
                huidig = dict(sub, subnummer=f"{ouder['subnummer']}{letter}", tekst=[])
                uit.append(huidig)
                letter = chr(ord(letter) + 1)
                continue
        if huidig is not None:
            huidig["tekst"].append(p)
        else:
            uit.append(dict(lees_opdracht(p, context), tekst=[]))
    return uit


def tekst_na_dubbele_punt(h):
    """'Aan artikel 7.5 wordt toegevoegd: Het verantwoordelijk overheidsorgaan
    ...': de nieuwe tekst staat in dezelfde alinea als de opdracht."""
    m = re.search(r"\b(?:toegevoegd|ingevoegd|te luiden|luidt|luiden|luidende):\s+(\S.*)$", h["opdracht"])
    return [m[1]] if m and h["soort"] in LUIDENDE else []


LUIDENDE = {"vervang_geheel", "lid_invoegen", "toevoegen", "invoegen_na",
            "begrip_invoegen", "begrip_vervangen"}


# ---------------------------------------------------------------- bronnen

def txt(e):
    return " ".join("".join(e.itertext()).split())


LETTER = re.compile(r"^[A-Z]{1,7}$")
# "D (art. 27 Wpp)" (Wpp), "Onderdeel B (artikel 1.2)" en "Onderdelen A en Q
# (opschrift en artikel 12.13)" (Archiefwet 2026).
TOEL_KOP = re.compile(r"^(?P<sleutels>[A-Z].*?)\s*\((?P<art>(?:art\.|(?:artikel|artikelen|opschrift)\b).*)\)$")



def regels_xml(e):
    """De tekst onder een element als regels, in de vorm waarin een nota
    nieuwe tekst schrijft ('Artikel 2a. Titel', '1. ...', 'a. ...', 'term:
    omschrijving'), zodat lees_nieuwe_tekst en splits_onderdeel haar lezen."""
    uit = []

    def kop(k):
        nr = txt(k.find("nr")) if k.find("nr") is not None else ""
        titel = txt(k.find("titel")) if k.find("titel") is not None else ""
        label = txt(k.find("label")) if k.find("label") is not None else ""
        return " ".join(x for x in (label, nr.rstrip(".") + "." if nr else "", titel) if x)

    def loop(x):
        if x.tag in ("meta-data", "noot", "redactie"):
            return
        if x.tag == "kop":
            uit.append(kop(x))
            return
        if x.tag in ("al", "tussenkop"):
            t = txt(x)
            if t:
                uit.append(t)
            return
        if x.tag == "wat":
            uit.append(txt(x))
            return
        if x.tag in ("artikel", "hoofdstuk", "paragraaf", "afdeling", "deel", "titeldeel"):
            k = x.find("kop")
            if k is not None:
                t = kop(k)
                uit.append(t if x.tag != "artikel" or t.startswith("Artikel") else "Artikel " + t)
            for c in x:
                if c.tag != "kop":
                    loop(c)
            return
        if x.tag == "lid":
            nr = txt(x.find("lidnr")) if x.find("lidnr") is not None else ""
            eerst = True
            for c in x:
                if c.tag == "lidnr":
                    continue
                if eerst and c.tag == "al":
                    uit.append(f"{nr} {txt(c)}".strip())
                    eerst = False
                else:
                    loop(c)
            return
        if x.tag == "li":
            nr = txt(x.find("li.nr")) if x.find("li.nr") is not None else ""
            eerst = True
            for c in x:
                if c.tag == "li.nr":
                    continue
                if eerst and c.tag == "al":
                    uit.append(f"{nr} {txt(c)}".strip())
                    eerst = False
                else:
                    loop(c)
            if eerst and nr:
                uit.append(nr)
            return
        if x.tag == "definitie-item":
            nr = txt(x.find("li.nr")) if x.find("li.nr") is not None else ""
            term = txt(x.find("term")) if x.find("term") is not None else ""
            df = x.find("definitie")
            als = [txt(a) for a in df.findall("al")] if df is not None else []
            uit.append(" ".join(t for t in (nr, term, als[0] if als else "") if t))
            for a in als[1:]:
                uit.append(a)
            if df is not None:
                for c in df:
                    if c.tag != "al":
                        loop(c)
            return
        if x.tag == "wijziging" and x.find("nr") is not None:
            wat = x.find("wat")
            uit.append(f"{txt(x.find('nr'))} {txt(wat) if wat is not None else ''}".strip())
            for c in x:
                if c.tag not in ("nr", "wat"):
                    loop(c)
            return
        for c in x:
            loop(c)

    loop(e)
    return uit


# Zonder artikel tussen haakjes: "Onderdeel A", "Onderdelen D tot en met P".
TOEL_KOP_LOS = re.compile(r"^Onderde(?:el|len)\s+(?P<sleutels>[A-Z]{1,7}\b.*)$")


def letter_nr(letter):
    # A=1 … Z=26, AA=27 … ZZ=52: de reeks van letter_van, omgekeerd.
    return (len(letter) - 1) * 26 + ord(letter[0]) - ord("A") + 1


def toelichtingsletters(sleutels):
    """De onderdeelletters die een toelichtingskop noemt, met een reeks
    ('D tot en met P') uitgeschreven."""
    uit = []
    for m in re.finditer(r"\b([A-Z]{1,7})\b(?:\s+(?:tot en met|t/m)\s+([A-Z]{1,7})\b)?", sleutels):
        if m[2] and len(set(m[1])) == 1 and len(set(m[2])) == 1:
            uit += [letter_van(n) for n in range(letter_nr(m[1]), letter_nr(m[2]) + 1)]
        else:
            uit += [m[1]] + ([m[2]] if m[2] else [])
    return uit


def lees_xml_nvw(pad):
    """Nota van wijziging uit officielebekendmakingen-XML: tussenkop = letter."""
    r = ET.parse(pad).getroot()
    onderdelen, toel, deel, huidig = [], {}, "wijziging", None
    for e in r.iter():
        if e.tag == "kop" and txt(e).upper() == "TOELICHTING":
            deel, huidig = "toelichting", None
            continue
        if e.tag not in ("tussenkop", "al"):
            continue
        t = txt(e)
        if deel == "wijziging":
            if e.tag == "tussenkop" and LETTER.match(t):
                huidig = {"letter": t, "paragrafen": []}
                onderdelen.append(huidig)
            elif huidig is not None:
                huidig["paragrafen"].append(t)
        else:
            m = (TOEL_KOP.match(t) or TOEL_KOP_LOS.match(t)) if e.tag == "tussenkop" else None
            if m:
                huidig = {"kop": t, "tekst": []}
                for letter in toelichtingsletters(m["sleutels"]):
                    toel.setdefault(letter, []).append(huidig)
            elif huidig is not None:
                huidig["tekst"].append(t)
    return onderdelen, toel


def letter_van(n):
    # Word 'upperLetter': 1=A … 26=Z, 27=AA … 52=ZZ, 53=AAA.
    return chr(ord("A") + (n - 1) % 26) * ((n - 1) // 26 + 1)


def lees_docx_nvw(pad):
    """Nota van wijziging als docx: de letters zijn automatische nummering op
    lege Kop2-koppen; de toelichting verwijst er met REF-velden naar."""
    z = zipfile.ZipFile(pad)
    x = ET.fromstring(z.read("word/document.xml"))
    onderdelen, per_anker, toel = [], {}, {}
    deel, huidig, n = "wijziging", None, 0
    for p in x.iter(W + "p"):
        t = "".join(r.text or "" for r in p.iter(W + "t")).strip()
        stijl = p.find(f"{W}pPr/{W}pStyle")
        stijl = stijl.get(W + "val") if stijl is not None else ""
        if t.upper() == "TOELICHTING":
            deel, huidig = "toelichting", None
            continue
        if deel == "wijziging":
            if stijl == "Kop2":
                n += 1
                huidig = {"letter": letter_van(n), "paragrafen": []}
                for b in p.iter(W + "bookmarkStart"):
                    if b.get(W + "name", "").startswith("_Ref"):
                        huidig["anker"] = b.get(W + "name")
                        per_anker[b.get(W + "name")] = huidig["letter"]
                onderdelen.append(huidig)
            elif t and huidig is not None:
                huidig["paragrafen"].append(t)
        else:
            refs = [i.text.split()[1] for i in p.iter(W + "instrText")
                    if i.text and i.text.strip().startswith("REF _Ref")]
            if refs and TOEL_KOP.match(t):
                huidig = {"kop": t, "tekst": []}
                for a in refs:
                    if a in per_anker:
                        toel.setdefault(per_anker[a], []).append(huidig)
            elif t and huidig is not None:
                huidig["tekst"].append(t)
    return onderdelen, toel


def lees_amendement(pad):
    """Amendement: elk <amendement-lid> is een onderdeel met zijn opdracht in
    <wat>. Een <wijziging> eronder met een eigen <wat> is een subopdracht;
    zonder is het nieuwe tekst. Opdrachten BINNEN die nieuwe tekst (een
    wijzigingsartikel voor de Kieswet) zijn tekst van het wetsvoorstel en
    geen opdracht op het wetsvoorstel. De toelichting is één geheel."""
    r = ET.parse(pad).getroot()
    am = r.find(".//amendement")
    leden = am.findall("amendement-lid") or [am]
    onderdelen = []
    for i, lid in enumerate(leden, start=1):
        wat = lid.find("wat")
        paragrafen = [txt(wat)] if wat is not None else []
        for w in lid.findall("wijziging"):
            nr = w.find("nr")
            eigen = w.find("wat")
            if eigen is None:
                eigen = next((c.find("wat") for c in w if c.find("wat") is not None), None)
            if eigen is not None and eigen is not wat:
                # De nummers zijn die van het amendement zelf ('2. Na het derde
                # lid worden twee leden ingevoegd, luidende:'); een wijziging
                # zonder nummer voorop is de kop ('Het voorgestelde artikel 10.3
                # wordt als volgt gewijzigd:').
                paragrafen.append(f"{txt(nr)} {txt(eigen)}" if nr is not None else txt(eigen))
            # Wat er verder onder staat is de nieuwe tekst: leden, onderdelen,
            # een heel artikel.
            for c in w:
                if c is not eigen and c.tag not in ("nr", "wat"):
                    paragrafen += regels_xml(c)
        onderdelen.append({"letter": romeins(i) if len(leden) > 1 else "I",
                           "paragrafen": paragrafen})
    toelichting, aan = [], False
    for e in r.iter():
        if e.tag == "kop" and txt(e).lower() == "toelichting":
            aan = True
        elif aan and e.tag == "al":
            toelichting.append(txt(e))
    toel = {o["letter"]: [{"kop": "Toelichting", "tekst": toelichting}] for o in onderdelen}
    return onderdelen, toel


def romeins(n):
    uit = ""
    for w, r in ((10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I")):
        while n >= w:
            uit, n = uit + r, n - w
    return uit


# ---------------------------------------------------------------- hoofdlijn

WIJZIGEND = {"Nota van wijziging", "Amendement", "Amendement (gewijzigd/nader/vervangend)"}


def main(map_, uit, stil=False):
    map_, uit = Path(map_), Path(uit)
    uit.mkdir(parents=True, exist_ok=True)
    stukken = json.loads((map_ / "stukken.json").read_text(encoding="utf-8"))
    dossier = next(map_.glob("kst-*-*.xml")).name.split("-")[1]

    alles, dekking, ongelezen = [], Counter(), Counter()
    for nr, docs in stukken.items():
        soort = docs[0]["soort"]
        if soort not in WIJZIGEND:
            continue
        xmlpad, docxpad = map_ / f"kst-{dossier}-{nr}.xml", map_ / f"kst-{dossier}-{nr}.docx"
        if soort.startswith("Amendement"):
            onderdelen, toel = lees_amendement(xmlpad)
        elif xmlpad.exists():
            onderdelen, toel = lees_xml_nvw(xmlpad)
        else:
            onderdelen, toel = lees_docx_nvw(docxpad)
        vervangt = re.search(r"ter vervanging van nr\. (\d+)", docs[0]["onderwerp"] or "")
        stuk = {"stuk": f"kst-{dossier}-{nr}", "ondernummer": int(nr), "soort": soort,
                "datum": docs[0]["datum"], "onderwerp": docs[0]["onderwerp"],
                "vervangt": int(vervangt[1]) if vervangt else None, "onderdelen": []}
        for o in onderdelen:
            hs = splits_onderdeel(o["paragrafen"])
            for h in hs:
                dekking[(nr, h["soort"] is not None)] += 1
                if h["soort"] is None:
                    ongelezen[" ".join(h["opdracht"].split()[:6])] += 1
            stuk["onderdelen"].append({
                "letter": o["letter"], "anker": o.get("anker"),
                "handelingen": hs,
                "toelichting": [t["kop"] for t in toel.get(o["letter"], [])],
                "toelichtingstekst": [{"kop": t["kop"], "tekst": t["tekst"]} for t in toel.get(o["letter"], [])],
            })
        alles.append(stuk)

    (uit / "handelingen.json").write_text(
        json.dumps(alles, ensure_ascii=False, indent=2), encoding="utf-8")

    regels = []
    for s in alles:
        nr = str(s["ondernummer"])
        n_ok, n_niet = dekking[(nr, True)], dekking[(nr, False)]
        zonder_toel = [o["letter"] for o in s["onderdelen"] if not o["toelichting"]]
        regels.append(
            f"{s['stuk']} {s['soort']}: {len(s['onderdelen'])} onderdelen, "
            f"{n_ok + n_niet} opdrachten, {n_ok} gelezen, {n_niet} niet; "
            f"zonder toelichting: {len(zonder_toel)}"
            + (f" ({', '.join(zonder_toel[:12])}{' …' if len(zonder_toel) > 12 else ''})" if zonder_toel else ""))
    # Signalen: wat het afspelen straks niet kan, en dat in het stuk zelf zit
    # of in deze grammatica. Een ongelezen opdracht kan beide zijn.
    regels.append("")
    regels.append("Signalen:")
    for s in alles:
        if s["vervangt"]:
            regels.append(f"  {s['stuk']}: vervangt nr. {s['vervangt']}; dat amendement speelt niet meer mee")
        for o in s["onderdelen"]:
            for h in o["handelingen"]:
                if h["soort"] is None:
                    regels.append(f"  {s['stuk']} {o['letter']}: niet te lezen: {h['opdracht'][:140]}")
                elif not h["adres"] and h["soort"] not in ("begrip_invoegen",):
                    regels.append(f"  {s['stuk']} {o['letter']}: geen adres: {h['opdracht'][:140]}")
            if not o["toelichting"]:
                regels.append(f"  {s['stuk']} {o['letter']}: geen artikelsgewijze toelichting gevonden: "
                              f"{o['handelingen'][0]['opdracht'][:100] if o['handelingen'] else ''}")
    regels.append("")
    regels.append("Soorten:")
    soorten = Counter(h["soort"] for s in alles for o in s["onderdelen"] for h in o["handelingen"])
    regels += [f"  {n:4d} {k}" for k, n in soorten.most_common()]
    regels.append("")
    regels.append("Ongelezen, per begin van de opdracht:")
    regels += [f"  {n:3d}  {k}" for k, n in ongelezen.most_common()]
    (uit / "dekking.txt").write_text("\n".join(regels) + "\n", encoding="utf-8")
    if stil:
        return alles
    print("\n".join(regels[:len(alles)]))
    return alles


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
