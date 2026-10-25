"""Haal de Kamerstukken van een wetsvoorstel op als bronmomentopname.

Drie bronnen, elk voor wat zij het best weet:
- het Gegevensmagazijn van de Tweede Kamer: welke stukken er zijn, hun soort
  (voorstel, nota van wijziging, amendement, ...), hun ondernummer, en wat
  er per zaak is besloten (de stemmingen over amendementen en het voorstel);
- de SRU-zoekdienst van officielebekendmakingen (repository.overheid.nl):
  de stukken van de Eerste Kamer (35968-A, -B, ...), die het Gegevensmagazijn
  van de Tweede Kamer niet kent;
- officielebekendmakingen.nl: de tekst van elk stuk als XML, en van het
  Staatsblad als het wordt meegegeven.

Uitvoer in <uitmap>:
  documenten.json   ruwe antwoorden van het Gegevensmagazijn (ongewijzigd)
  stukken.json      per ondernummer (of letter, Eerste Kamer): soort, titel,
                    datum, kamer, documentnummer
  besluiten.json    per ondernummer: de stemming (aangenomen, verworpen,
                    ingetrokken) met haar datum, en die over het voorstel
  kst-<dossier>-<nr>.xml   de tekst, zoals officielebekendmakingen haar serveert
  kst-<dossier>-<nr>.docx  alleen als er (nog) geen XML is: het stuk zoals
                           de Kamer het ontving
  stb-<jaar>-<nr>.xml      het Staatsblad, als het is meegegeven
  ophaallog.txt     wat lukte en wat niet

Een stuk dat er al staat wordt niet opnieuw opgehaald; een docx die er staat
omdat de XML de vorige keer niet kwam, krijgt een nieuwe kans.

    python3 haal-kamerstukken.py 36742 ../bron/kamerdossiers/36742
    python3 haal-kamerstukken.py 35968 ../bron/kamerdossiers/35968 --staatsblad stb-2026-149
"""
import http.client
import json
import re
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ODATA = "https://gegevensmagazijn.tweedekamer.nl/OData/v4/2.0/"
OBP = "https://zoek.officielebekendmakingen.nl/"
SRU = "https://repository.overheid.nl/sru"


def haal(url, pogingen=4):
    """Een antwoord van de server. Een verbroken of afgekapte verbinding wordt
    opnieuw geprobeerd, en daarna met curl (urllib krijgt grote stukken via de
    proxy soms nooit heel binnen; gemeten 12-10-2026). Een 404 is een antwoord
    en wordt niet herhaald."""
    req = urllib.request.Request(url, headers={"User-Agent": "regelrecht-werkbank"})
    for poging in range(pogingen):
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return r.read()
        except urllib.error.HTTPError:
            raise
        except (http.client.IncompleteRead, http.client.RemoteDisconnected,
                urllib.error.URLError, ConnectionError, TimeoutError):
            time.sleep(2 * (poging + 1))
    return subprocess.run(["curl", "-sfL", "--http1.1", "--retry", "3", url],
                          check=True, capture_output=True).stdout


def documenten(dossier):
    filt = (f"Verwijderd eq false and "
            f"Kamerstukdossier/any(k: k/Nummer eq {dossier})")
    query = urllib.parse.urlencode({
        "$filter": filt,
        "$select": "Id,Soort,Titel,Onderwerp,Volgnummer,DocumentNummer,Datum",
        "$expand": "Kamerstukdossier($select=Nummer,Toevoeging)",
        "$orderby": "Volgnummer",
    }, safe="$(),=/: ")
    url = ODATA + "Document?" + query.replace(" ", "%20")
    paginas = []
    while url:
        blad = json.loads(haal(url))
        paginas.append(blad)
        url = blad.get("@odata.nextLink")
    return paginas


def besluiten(dossier):
    """Per ondernummer de laatste stemming over zijn zaak. Een zaak kan meer
    documenten hebben (het voorstel met zijn nota's); dan geldt de uitslag
    voor het voorstel, en bij een amendement voor dat amendement."""
    query = urllib.parse.urlencode({
        "$filter": f"Verwijderd eq false and Kamerstukdossier/any(k: k/Nummer eq {dossier})",
        "$select": "Nummer,Soort",
        "$expand": "Besluit($select=BesluitSoort,BesluitTekst,GewijzigdOp,StemmingsSoort;"
                   "$filter=Verwijderd eq false),Document($select=Volgnummer,Soort)",
    }, safe="$(),=/:; ")
    url = ODATA + "Zaak?" + query.replace(" ", "%20")
    uit = {}
    while url:
        blad = json.loads(haal(url))
        for z in blad["value"]:
            stemmingen = sorted((b for b in z["Besluit"] if (b["BesluitSoort"] or "").startswith("Stemmen")),
                                key=lambda b: b["GewijzigdOp"])
            if not stemmingen:
                continue
            b = stemmingen[-1]
            uitslag = b["BesluitSoort"].split("-", 1)[1].strip().lower()
            docs = [d for d in z["Document"] if (d["Volgnummer"] or -1) > 0]
            # De zaak van het wetsvoorstel draagt alle stukken; de uitslag hoort
            # bij het voorstel zelf.
            doel = [d for d in docs if d["Soort"] == "Voorstel van wet"] or docs[:1]
            for d in doel:
                uit[str(d["Volgnummer"])] = {"uitslag": uitslag, "datum": b["GewijzigdOp"][:10],
                                             "zaak": z["Nummer"], "soort": z["Soort"]}
        url = blad.get("@odata.nextLink")
    return uit


def eerste_kamer(dossier):
    """De stukken van de Eerste Kamer: kst-<dossier>-A, -B, ..."""
    query = urllib.parse.urlencode({
        "operation": "searchRetrieve", "version": "2.0", "maximumRecords": "200",
        "query": f"w.dossiernummer=={dossier}"})
    x = haal(f"{SRU}?{query}").decode("utf-8")
    uit = {}
    for r in re.findall(r"<sru:record>(.*?)</sru:record>", x, re.S):
        g = lambda t: (re.search(rf"<{t}[^>]*>([^<]*)</{t}>", r) or [None, ""])[1]  # noqa: E731
        ident = g("dcterms:identifier")
        m = re.fullmatch(rf"kst-{dossier}-([A-Z]{{1,3}})", ident)
        if not m:
            continue
        uit[m[1]] = [{"id": ident, "soort": g("overheidwetgeving:documenttitel") or "Eerste Kamerstuk",
                      "titel": g("dcterms:title"), "onderwerp": g("overheidwetgeving:documenttitel"),
                      "datum": g("dcterms:date"), "kamer": "Eerste Kamer", "documentnummer": None}]
    return uit


def letters_volgorde(k):
    return (len(k), k)


def main(dossier, uitmap, staatsblad=None):
    uit = Path(uitmap)
    uit.mkdir(parents=True, exist_ok=True)
    log = []

    paginas = documenten(dossier)
    (uit / "documenten.json").write_text(
        json.dumps(paginas, ensure_ascii=False, indent=2), encoding="utf-8")

    stukken = {}
    for blad in paginas:
        for d in blad["value"]:
            nr = d.get("Volgnummer")
            if nr is None or nr < 0:
                continue
            stukken.setdefault(str(nr), []).append({
                "id": d["Id"],
                "soort": d["Soort"],
                "titel": d.get("Titel"),
                "onderwerp": d.get("Onderwerp"),
                "datum": d.get("Datum"),
                "kamer": "Tweede Kamer",
                "documentnummer": d.get("DocumentNummer"),
            })
    log.append(f"Gegevensmagazijn: {len(stukken)} ondernummers")
    try:
        ek = eerste_kamer(dossier)
        log.append(f"Eerste Kamer: {len(ek)} stukken ({', '.join(sorted(ek, key=letters_volgorde))})")
    except Exception as e:  # noqa: BLE001 -- het falen hoort in het log
        ek = {}
        log.append(f"Eerste Kamer: NIET opgehaald ({e})")
    volgorde = sorted(stukken, key=int) + sorted(ek, key=letters_volgorde)
    stukken.update(ek)
    (uit / "stukken.json").write_text(
        json.dumps({k: stukken[k] for k in volgorde}, ensure_ascii=False, indent=2), encoding="utf-8")

    b = besluiten(dossier)
    (uit / "besluiten.json").write_text(json.dumps(dict(sorted(b.items(), key=lambda kv: int(kv[0]))),
                                                   ensure_ascii=False, indent=2), encoding="utf-8")
    log.append(f"besluiten: {len(b)} stemmingen")

    for nr in volgorde:
        doel = uit / f"kst-{dossier}-{nr}.xml"
        reserve = uit / f"kst-{dossier}-{nr}.docx"
        if doel.exists():
            continue
        try:
            doel.write_bytes(haal(f"{OBP}kst-{dossier}-{nr}.xml"))
            if reserve.exists():
                reserve.unlink()
                log.append(f"nr. {nr}: XML opgehaald, de docx van eerder vervalt")
            else:
                log.append(f"nr. {nr}: opgehaald")
        except Exception as e:  # noqa: BLE001 -- elk falen hoort in het log
            if reserve.exists():
                log.append(f"nr. {nr}: nog steeds geen XML ({e}); de docx blijft")
                continue
            if nr not in stukken or stukken[nr][0].get("kamer") == "Eerste Kamer":
                log.append(f"nr. {nr}: NIET opgehaald ({e})")
                continue
            # Nog niet bekendgemaakt (nr. 20 op 12-10-2026): dan het document
            # zoals de Kamer het ontving, uit het Gegevensmagazijn.
            try:
                reserve.write_bytes(haal(f"{ODATA}Document({stukken[nr][0]['id']})/resource"))
                log.append(f"nr. {nr}: geen XML ({e}); docx uit het Gegevensmagazijn")
            except Exception as e2:  # noqa: BLE001
                log.append(f"nr. {nr}: NIET opgehaald ({e}; {e2})")
        time.sleep(0.3)

    if staatsblad:
        doel = uit / f"{staatsblad}.xml"
        if not doel.exists():
            try:
                doel.write_bytes(haal(f"{OBP}{staatsblad}.xml"))
                log.append(f"{staatsblad}: opgehaald")
            except Exception as e:  # noqa: BLE001
                log.append(f"{staatsblad}: NIET opgehaald ({e})")

    (uit / "ophaallog.txt").write_text("\n".join(log) + "\n", encoding="utf-8")
    print("\n".join(log[-6:]))
    print(f"klaar: {uit}")


if __name__ == "__main__":
    args = sys.argv[1:]
    stb = None
    if "--staatsblad" in args:
        i = args.index("--staatsblad")
        stb = args[i + 1]
        del args[i:i + 2]
    main(args[0], args[1], stb)
