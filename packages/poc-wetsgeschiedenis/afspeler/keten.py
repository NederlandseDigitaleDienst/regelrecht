# -*- coding: utf-8 -*-
"""Een wet door de tijd: elke versie, en elke overgang getoetst.

kamerdossier.py --keten legt in bron/regelingen/<BWB>/ elke versie van een
wet vast zoals wetten.overheid.nl haar kent, en per overgang het Staatsblad
van de wet die haar maakte. Van elke overgang staan dus beide uiteinden
officieel vast. Dit script doet per overgang twee dingen:

- het neemt de tekst van de nieuwe versie over (die is de waarheid), en houdt
  per regel bij sinds welke overgang zij zo luidt;
- het speelt de wijzigingsartikelen uit het Staatsblad af op de oude versie
  (afspelen.py, dezelfde motor als voor de nota's) en legt de uitkomst naast
  de nieuwe versie. Komt het uit, dan sluit de keten op die schakel; zo niet,
  dan zegt het welke artikelen afwijken: een opdracht die deze grammatica niet
  leest, een wijziging die niet uit een wet komt (een besluit, een
  rectificatie), of een fout in een van beide.

De uitkomst heeft de vorm van een tijdlijn van afspelen.py, zodat de
wetsgeschiedenis haar toont zoals een Kamerdossier: elke overgang een moment,
de wet op elk moment, per artikel zijn levensloop, en in Doorwerking per
overgang de vergelijking met het Staatsblad.

    python3 generatoren/keten.py BWBR0007376

Voorstel M526.
"""
import copy
import json
import os
import re
import sys
from pathlib import Path

HIER = Path(__file__).resolve().parent
sys.path.insert(0, str(HIER))
import afspelen as A  # noqa: E402
import kamertoelichting as K  # noqa: E402
from handelingen import splits_onderdeel  # noqa: E402

REGELINGEN = Path(os.environ.get("WETSGESCHIEDENIS_BRON") or HIER.parent / "bron") / "regelingen"
STATUS_GRENS = 0.5  # boven dit deel van de geraakte artikelen gelijk: 'met aanname', anders 'niet'

SOORT = {"Nieuwe-regeling": "Nieuwe wet", "Wijziging": "Wijzigingswet", "Intrekking-regeling": "Intrekking",
         "Aanhangig": "Aanhangig voorstel"}
# Een besluit of een ministeriële regeling heet niet 'wet'.
SOORT_VAN = {"Besluit": {"Nieuwe wet": "Nieuw besluit", "Wijzigingswet": "Wijzigingsbesluit"},
             "Regeling": {"Nieuwe wet": "Nieuwe regeling", "Wijzigingswet": "Wijzigingsregeling"}}


def bekendmaking(s):
    """'stb-2018-461' -> 'Stb. 2018, 461'; 'stcrt-2026-32101' -> 'Stcrt. 2026, 32101'."""
    m = re.fullmatch(r"(stb|stcrt)-(\d{4})-(\d+)", s)
    return f"{'Stb.' if m[1] == 'stb' else 'Stcrt.'} {m[2]}, {m[3]}" if m else s


def aard(pads):
    """'Wet', 'Besluit' of 'Regeling': hoe de bekendmaking zichzelf noemt."""
    for pad in pads:
        try:
            r = A.ET.parse(pad).getroot()
        except (A.ET.ParseError, OSError):
            continue
        t = " ".join(" ".join(r.itertext()).split())[:1500]
        m = re.search(r"\b(Rijkswet|Wet|Besluit|Regeling)\s+van\s+(?:de\s+)?\d|\b(Regeling)\s+van\s+de\s+[Mm]inister", t)
        if m:
            return {"Rijkswet": "Wet"}.get(m[1] or m[2], m[1] or m[2])
    return "Wet"


def regels(k, diepte=0):
    """De regels van een knoop: [tekst, diepte, soort], zoals Opname ze maakt."""
    if k["soort"] == "begrip":
        t = f"{k['nr']}: {k['tekst']}".strip()
    elif k["soort"] in ("lid", "onderdeel"):
        t = f"{k['nr']} {k['tekst']}".strip()
    else:
        t = k["tekst"]
    uit = [[t, diepte, k["soort"]]] if t else []
    for c in k["kinderen"]:
        uit += regels(c, diepte + (k["soort"] in ("lid", "onderdeel")))
    return uit


def blokken(wet):
    """{sleutel: (soort, nr, [[tekst, diepte, soort]])} en de volgorde."""
    uit, volgorde = {}, []
    for k in wet:
        s = f"{k['soort']}:{k['nr']}"
        if k["soort"] == "artikel":
            r = [[f"Artikel {k['nr']}. {k['titel']}".strip(), 0, "kop"]] + [x for c in k["kinderen"] for x in regels(c)]
        else:
            r = [[f"{k['soort'].capitalize()} {k['nr']}. {k['titel']}".strip(), 0, k["soort"]]]
        uit[s] = (k["soort"], k["nr"], r)
        volgorde.append(s)
    return uit, volgorde


def speel_overgang(basis, staatsbladen, titels, stuk):
    """De wijzigingsartikelen van de Staatsbladen, na elkaar op de oude versie."""
    wet = copy.deepcopy(basis)
    handelingen, tel, artnrs = [], {"afgespeeld": 0, "aanname": 0, "niet": 0}, {}
    for pad in staatsbladen:
        try:
            artnr, onderdelen = A.wijzigingsonderdelen(pad, titels)
        except A.ET.ParseError:
            # Een bekendmaking van vóór de XML (ongeveer 2009) is HTML.
            artnr, onderdelen = None, []
        artnrs[Path(pad).stem] = artnr
        for o in onderdelen:
            for h in splits_onderdeel(o["paragrafen"]):
                A.AANNAMES.clear()
                try:
                    if h["soort"] is None:
                        raise A.Fout("de opdracht is niet te lezen")
                    if h["soort"] == "samengesteld":
                        raise A.Fout("een samengestelde opdracht zonder te lezen onderdelen")
                    A.speel(wet, h, {"stuk": stuk, "onderdeel": o["letter"], "artikel": artnr})
                    status, reden = ("aanname" if A.AANNAMES else "ok"), "; ".join(A.AANNAMES[:3])
                except A.Fout as e:
                    status, reden = "niet", str(e)
                tel[{"ok": "afgespeeld"}.get(status, status)] += 1
                a = h["adres"] or {}
                handelingen.append({"onderdeel": o["letter"], "sub": h.get("subnummer"), "opdracht": h["opdracht"],
                                    "status": status, "reden": reden, "stb": Path(pad).stem,
                                    "artikelen": sorted({d["artikel"] for d in a.get("doelen", [])} or {a.get("artikel")} - {None})})
    A.ontmerk(wet)
    return wet, handelingen, tel, artnrs


def tekst_per_artikel(wet):
    # Een lidnummer telt zonder punt: het Staatsblad schrijft '1.', het BWB '1'.
    # Een spatie na een komma ook niet: het BWB schrijft in artikel 2b '5,6,'.
    return {k["nr"]: [re.sub(r"\s*,\s*", ", ", re.sub(r"^(\d+[a-z]?)\. ", r"\1 ", r)) for r in A.artikeltekst(k)]
            for k in wet if k["soort"] == "artikel"}


def toelichting(stuk, datum, pads, artnrs, handelingen, titels, voor, na, nieuw):
    """De nota's van toelichting van de bekendmakingen van één overgang, als
    brokken aan de artikelen (sleutels van de blokken) van deze regeling."""
    brokken, niet, gelezen = [], [], []
    for pad in pads:
        stem = Path(pad).stem
        doelen = {"*": set()}
        for h in handelingen:
            if h["stb"] == stem:
                doelen.setdefault(K._letter(h["onderdeel"]), set()).update(h["artikelen"])
                doelen["*"].update(h["artikelen"])
        bestaat = {b["nr"] for b in list(voor.values()) + list(na.values()) if b["soort"] == "artikel"}
        try:
            bb, nn, ja = K.nota_van_toelichting(pad, titels, artnrs.get(stem), doelen, bestaat, nieuw)
        except A.ET.ParseError:
            niet.append({"stuk": stem, "reden": "geen XML (een bekendmaking van vóór ongeveer 2009)", "tekst": ""})
            continue
        if not ja:
            continue
        gelezen.append(stem)
        for b in bb:
            b.update(stuk=stuk, datum=datum, bron={"soort": "nota van toelichting", "publicatie": stem})
        brokken += bb
        niet += nn
    return brokken, niet, gelezen


def uit_dossiers(stuk, datum, nrs, tijdlijn_van, status_van, bwb, titels):
    """Wat de Kamerdossiers van een overgang over deze regeling zeggen; en per
    dossier of het er is."""
    brokken, status = [], []
    for nr in nrs:
        t = tijdlijn_van(nr) if tijdlijn_van else None
        if not t:
            status.append({"nr": nr, **(status_van(nr) if status_van else {"status": "niet opgehaald"})})
            continue
        gevonden, artikel = K.uit_dossier(t, bwb, titels)
        for b, arts, rol in gevonden:
            brokken.append({"stuk": stuk, "datum": datum, "soort": b["soort"], "rol": rol, "kop": b["kop"],
                            "titel": b.get("titel"), "artikelen": list(arts),
                            "leden": {a: b["leden"][a] for a in arts if (b.get("leden") or {}).get(a)},
                            "tekst": b["tekst"],
                            "bron": {"soort": "Kamerstuk", "stuk": b["stuk"], "datum": b["datum"], "dossier": nr,
                                     **({"url": b["url"], "oud": b["oud"]} if b.get("url") else {})}})
        status.append({"nr": nr, "status": "opgehaald", "naam": t.get("naam"), "artikel": artikel,
                       "brokken": len(gevonden)})
    return brokken, status


def bouw(bwb, tijdlijn_van=None, status_van=None):
    """De wet door de tijd. `tijdlijn_van(nr)` geeft de tijdlijn van een
    Kamerdossier als die er is (bouw-wetsgeschiedenis.py, of de dienst), en
    `status_van(nr)` waarom niet; zo hangt de toelichting van elk dossier dat
    er is aan zijn overgang."""
    map_ = REGELINGEN / bwb
    k = json.loads((map_ / "keten.json").read_text(encoding="utf-8"))
    titel = k["titel"]
    titels = [titel] + ([k["afkorting"]] if k.get("afkorting") else [])
    versies = {v["datum"]: map_ / v["pad"] for v in k["versies"] if v["pad"]}
    data = sorted(versies)
    if not data:
        raise SystemExit(f"{titel}: geen enkele versie van wetten.overheid.nl te lezen; zie het log van kamerdossier.py --keten")
    begin_d = data[0]
    stuk_van = lambda d: f"versie-{d}"  # noqa: E731

    refs, refnr = [], {}

    def ref(stuk):
        if stuk not in refnr:
            refnr[stuk] = len(refs)
            refs.append({"stuk": stuk, "onderdeel": None, "sub": None})
        return refnr[stuk]

    def met_herkomst(bl, vorig, stuk):
        """Regels met hun herkomst: een regel die er al zo stond, houdt de
        zijne; een nieuwe komt van deze overgang."""
        soort, nr, rr = bl
        oud = {}
        for r in (vorig or {}).get("regels", []):
            oud.setdefault(r[0], r[1])
        return {"soort": soort, "nr": nr,
                "regels": [[t, oud.get(t, [ref(stuk)]), d, s] for t, d, s in rr]}

    lees = lambda d: A.lees_wettekst(versies[d], stuk_van(d))  # noqa: E731
    wet0 = lees(begin_d)
    b0, v0 = blokken(wet0)
    huidig = {s: met_herkomst(b, None, stuk_van(begin_d)) for s, b in b0.items()}
    begin = {"volgorde": v0, "blokken": dict(huidig)}
    volgorde = v0

    afgekapt = k.get("afgekapt")
    stukken = [{"ondernummer": "–", "volgorde": 0, "stuk": stuk_van(begin_d), "soort": "Versie van de wet",
                "titel": f"{titel}, zoals de wet gold op {begin_d} " + (
                    f"(de oudste versie die nu is opgehaald; de {afgekapt['versies']} versies sinds {afgekapt['sinds']} volgen)"
                    if afgekapt else "(de oudste versie op wetten.overheid.nl)"),
                "datum": begin_d, "kamer": "", "speelt_mee": True, "uitslag": None, "niet_mee": None}]
    momenten = [{"soort": "stuk", "stuk": stuk_van(begin_d), "datum": begin_d}]
    stappen, doorwerking, tel = [], [], {"afgespeeld": 0, "aanname": 0, "niet": 0}
    brokken, niet, gelezen = [], [], []
    vorige_wet = wet0
    stb_pads = lambda o: [map_ / "stb" / f"{s}.xml" for s in o["staatsbladen"] if (map_ / "stb" / f"{s}.xml").exists()]  # noqa: E731
    # De oudste versie: als haar bekendmaking de regeling zelf is, draagt die de
    # toelichting op elk artikel.
    for o in k["overgangen"]:
        if o["tot"] == begin_d:
            nieuw_ = "Nieuwe-regeling" in o["betreft"]
            if nieuw_:
                bb, nn, gg = toelichting(stuk_van(begin_d), begin_d, stb_pads(o), {}, [], titels, huidig, huidig, True)
                brokken, niet, gelezen = brokken + bb, niet + nn, gelezen + gg
            bb, ds = uit_dossiers(stuk_van(begin_d), begin_d, o["dossiers"], tijdlijn_van, status_van, bwb, titels)
            brokken += bb
            stukken[0]["bronnen"] = {"dossiers": ds, "publicaties": o["staatsbladen"],
                                     "nota": {s: sum(1 for b in brokken if b["bron"].get("publicatie") == s)
                                              for s in gelezen}}
    for o in k["overgangen"]:
        stuk = stuk_van(o["tot"])
        soort = SOORT.get(o["betreft"][0], o["betreft"][0]) if o["betreft"] else "Wijziging"
        soort_aard = aard(stb_pads(o))
        soort = SOORT_VAN.get(soort_aard, {}).get(soort, soort)
        st = {"ondernummer": ", ".join(o["dossiers"]) or "–", "volgorde": len(stukken), "stuk": stuk, "soort": soort,
              "titel": f"{soort_aard} van {o['ondertekening'] or '?'}, {', '.join(bekendmaking(s) for s in o['staatsbladen']) or 'zonder bekendmaking'}; in werking {o['tot']}",
              "datum": o["tot"], "kamer": "", "speelt_mee": False, "uitslag": None, "niet_mee": None}
        stukken.append(st)
        if o["tot"] == begin_d:
            st["bronnen"] = stukken[0].get("bronnen")
        if o["tot"] not in versies or o["tot"] == begin_d:
            st["niet_mee"] = ("de oudste versie op wetten.overheid.nl" if o["tot"] == begin_d else
                              "geen tekst van deze versie op wetten.overheid.nl")
            if o["tot"] != begin_d and o["tot"] < begin_d:
                momenten.insert(0, {"soort": "stuk", "stuk": stuk, "datum": o["tot"]})
            if o["tot"] != begin_d:
                # Geen tekst om af te spelen, wel een dossier dat erover spreekt
                # (de Awb van 1994: haar Kamerstukken uit Staten-Generaal
                # Digitaal). Het hangt aan de artikelen van de oudste versie.
                dd, ds = uit_dossiers(stuk, o["tot"], o["dossiers"], tijdlijn_van, status_van, bwb, titels)
                brokken += dd
                gelezen += [d["nr"] for d in ds if d["status"] == "opgehaald"]
                st["bronnen"] = {"dossiers": ds, "publicaties": o["staatsbladen"], "nota": {}}
            continue
        wet = lees(o["tot"])
        nb, nv = blokken(wet)
        nieuw = {s: met_herkomst(b, huidig.get(s), stuk) for s, b in nb.items()}
        wijzig = {s: b for s, b in nieuw.items() if huidig.get(s, {}).get("regels") is None
                  or [r[0] for r in huidig[s]["regels"]] != [r[0] for r in b["regels"]]}
        weg = [s for s in huidig if s not in nieuw]
        # De toets: speelt het Staatsblad deze overgang af?
        pads = stb_pads(o)
        gespeeld, handelingen, t, artnrs = speel_overgang(vorige_wet, pads, titels, stuk)
        bb, nn, gg = toelichting(stuk, o["tot"], pads, artnrs, handelingen, titels, huidig, nieuw,
                                 "Nieuwe-regeling" in o["betreft"])
        dd, ds = uit_dossiers(stuk, o["tot"], o["dossiers"], tijdlijn_van, status_van, bwb, titels)
        brokken, niet, gelezen = brokken + bb + dd, niet + nn, gelezen + gg + [d["nr"] for d in ds if d["status"] == "opgehaald"]
        bronnen = {"dossiers": ds, "publicaties": o["staatsbladen"],
                   "nota": {s: sum(1 for b in bb if b["bron"]["publicatie"] == s) for s in gg},
                   "artikel": {s: a for s, a in artnrs.items() if a}}
        st["bronnen"] = bronnen
        echt, gekregen, oud = tekst_per_artikel(wet), tekst_per_artikel(gespeeld), tekst_per_artikel(vorige_wet)
        geraakt = sorted(nr for nr in set(echt) | set(oud) if A.norm(" ".join(echt.get(nr, []))) != A.norm(" ".join(oud.get(nr, []))))
        anders = sorted(nr for nr in set(echt) | set(gekregen)
                        if A.norm(" ".join(echt.get(nr, []))) != A.norm(" ".join(gekregen.get(nr, []))))
        if not geraakt:
            status, reden = "ok", "de tekst van de artikelen veranderde niet (een kop, een opschrift of alleen de datum)"
        elif not handelingen:
            status, reden = "niet", ("geen wijzigingsartikel voor deze wet gevonden in " + ", ".join(o["staatsbladen"])
                                     if o["staatsbladen"] else "geen bekendmaking bij deze versie")
        elif not anders:
            status, reden = "ok", ""
        else:
            goed = len([nr for nr in geraakt if nr not in anders])
            status = "aanname" if goed / len(geraakt) > STATUS_GRENS else "niet"
            reden = f"{len(anders)} artikel(en) wijken af van de nieuwe versie: {', '.join(anders[:12])}"
        tel[{"ok": "afgespeeld"}.get(status, status)] += 1
        st["speelt_mee"] = True
        stappen.append({"stuk": stuk, "onderdeel": ", ".join(o["dossiers"]) or "–", "sub": None, "soort": "versie",
                        "opdracht": f"{soort}{' ' + ', '.join(o['dossiers']) if o['dossiers'] else ''}: {len(geraakt)} artikel(en) gewijzigd"
                                    f" ({', '.join(geraakt[:20])}{' …' if len(geraakt) > 20 else ''}); "
                                    f"de bekendmaking speelt {t['afgespeeld']} van {sum(t.values())} opdrachten af",
                        "tekst": [], "status": status, "reden": reden, "ronde": 1, "veeg": False, "toelichting": [],
                        "bronnen": bronnen, "wijzig": wijzig, "weg": weg, **({"volgorde": nv} if nv != volgorde else {})})
        momenten.append({"soort": "stuk", "stuk": stuk, "datum": o["tot"], "stappen": [len(stappen), len(stappen)]})
        doorwerking.append({
            "titel": f"{o['tot']}: {', '.join(o['dossiers']) or 'zonder dossier'}", "bwb": bwb,
            "basis": f"de versie van {o['van']}", "nummers": geraakt,
            "versies": [{"versie": ", ".join(bekendmaking(s) for s in o["staatsbladen"]) or "geen bekendmaking", "stuk": stuk, "datum": o["tot"],
                         "artikel": "", "tel": t, "handelingen": handelingen,
                         "artikelen": {nr: {"voor": [A.norm(r) for r in oud.get(nr, [])],
                                            "na": [A.norm(r) for r in gekregen.get(nr, [])]}
                                       for nr in sorted(set(geraakt) | set(anders))},
                         "ijk": {"naam": f"de versie van {o['tot']}", "gelijk": len(set(echt) | set(gekregen)) - len(anders),
                                 "verschillend": anders,
                                 "artikelen": {nr: [A.norm(r) for r in echt.get(nr, [])] for nr in anders}}}]})
        huidig, volgorde, vorige_wet = nieuw, nv, wet
    for v in k.get("aanhangig") or []:
        for d in v["dossiers"]:
            stukken.append({"ondernummer": d, "volgorde": len(stukken), "stuk": f"aanhangig-{d}", "soort": "Aanhangig voorstel",
                            "titel": f"Kamerdossier {d}, aanhangig; raakt artikel {', '.join(v['artikelen'])}",
                            "datum": None, "kamer": "", "speelt_mee": False, "uitslag": None,
                            "niet_mee": "nog niet aangenomen; zie het dossier zelf"})
    for v in k.get("ingetrokken") or []:
        stuk = f"intrekking-{v['staatsblad']}"
        stukken.append({"ondernummer": ", ".join(v["dossiers"]), "volgorde": len(stukken), "stuk": stuk, "soort": "Intrekking",
                        "titel": f"De wet wordt ingetrokken: {v['staatsblad']}, ondertekend {v['ondertekening']}",
                        "datum": v["ondertekening"], "kamer": "", "speelt_mee": False, "uitslag": None, "niet_mee": None})
        if v["ondertekening"]:
            momenten.append({"soort": "stuk", "stuk": stuk, "datum": v["ondertekening"]})
    momenten.sort(key=lambda m: m["datum"] or "9999")

    # De brokken aan de artikelen: een nummer is de sleutel van zijn blok op het
    # moment van de overgang, na de overgang of anders ervoor (een artikel dat
    # vervalt).
    sleutels = {s for s in begin["blokken"]} | {s for st_ in stappen for s in st_["wijzig"]}
    for i, b in enumerate(brokken):
        b["id"] = i
        b["ids"] = [f"artikel:{nr}" if f"artikel:{nr}" in sleutels else None for nr in b["artikelen"]]
        b.setdefault("doelartikelen", [])
        b["leden"] = {a: sorted(v) for a, v in (b.get("leden") or {}).items()}
    commentaar = {"brokken": brokken, "niet": niet, "tel": {
        "brokken": len(brokken),
        "artikelsgewijs": sum(b["rol"] == "artikelsgewijs" for b in brokken),
        "vermelding": sum(b["rol"] == "vermelding" for b in brokken),
        "amendement": 0,
        "overgang": sum(b["rol"] == "overgang" for b in brokken),
        "nota": sum(b["bron"]["soort"] == "nota van toelichting" for b in brokken),
        "kamerstuk": sum(b["bron"]["soort"] == "Kamerstuk" for b in brokken),
        "artikelen_met_toelichting": len({i for b in brokken if b["rol"] == "artikelsgewijs" for i in b["ids"] if i}),
        "aan_de_doelwet": 0, "niet_gehaakt": len(niet), "gelezen": gelezen}}

    # Het ijkpunt is de laatste versie zoals zij is, zonder de normalisering van de toets.
    eind = {k["nr"]: A.artikeltekst(k) for k in vorige_wet if k["soort"] == "artikel"}
    return {
        "dossier": bwb, "naam": f"{titel} door de tijd", "soort": "keten", "deze_wet": titels, "afgekapt": afgekapt,
        "voorstel": stuk_van(begin_d), "stukken": stukken, "momenten": momenten, "refs": refs, "begin": begin,
        "stappen": stappen, "identiteit": {}, "toelichtingen": {}, "groepen": A.GROEPEN, "bron": {},
        "ijkpunten": [{"naam": f"De geldende tekst ({data[-1]})", "soort": "bwb",
                       "uitleg": "de laatste versie op wetten.overheid.nl", "datum": data[-1], "gelijk": len(eind),
                       "verschillend": 0, "artikelen": {nr: [A.norm(r) for r in rr] for nr, rr in eind.items()}}],
        "doorwerking": doorwerking, "gelijk": len(eind), "tel": tel,
        "commentaar": commentaar,
    }


def regelingen():
    return sorted(p.parent.name for p in REGELINGEN.glob("*/keten.json"))


if __name__ == "__main__":
    t = bouw(sys.argv[1])
    print(t["naam"], t["tel"])
    for d in t["doorwerking"]:
        v = d["versies"][0]
        print(f"  {d['titel']:22s} {v['versie']:28s} gespeeld {v['tel']}, afwijkend {len(v['ijk']['verschillend'])} "
              f"van {len(d['nummers'])} geraakt {v['ijk']['verschillend'][:8]}")
