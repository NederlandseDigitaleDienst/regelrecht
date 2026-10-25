# -*- coding: utf-8 -*-
"""Bouwt de wetsgeschiedenis: de wettekst van het voorstel, handeling voor
handeling afgespeeld door de nota's van wijziging, als pagina met een speler.

WAT ZIJ TOONT. Een tijdlijn met per handeling een stap. Op elke stap staat de
hele wet zoals zij dan luidt, met gemarkeerd wat die stap veranderde. Per regel
de keten van stukken waar haar tekst vandaan komt, met de opdracht en de
toelichting erbij. Daarnaast twee andere blikken op dezelfde gegevens: twee
momenten naast elkaar (wijzigingen) en de laatste stand naast de doorgewerkte
bron (controle).

WAAR HET VANDAAN KOMT. Niets op deze pagina is met de hand geschreven over de
wet. De tekst en de herkomst komen uit `afspelen.py` (tijdlijn.json), die op
haar beurt alleen de Kamerstukken leest die `haal-kamerstukken.py` vastlegde
in bron/kamerdossiers/<nummer>, met een dossier.yaml erbij (kamerdossier.py
haalt een dossier op, ook vanaf een BWB-id). Elk dossier daar komt op de
pagina, met een keuzelijst. Dit script draait de keten zelf eerst, zodat de
pagina nooit een oude afspeling toont naast nieuwe stukken.

DE VORM IS DIE VAN DE EDITOR. De werkbalk, de knoppen en de labels zijn de
webcomponenten van het NLDD Design System waarmee de regelrecht-editor haar
schermen bouwt; de kleuren en maten zijn haar tokens, zodat ook de donkere
stand vanzelf meekomt. Alles wordt ingebakken: de pagina laadt geen byte van
buiten. Zie bron/huisstijl/LEESMIJ.md.

Er wordt geen bouwdatum geinjecteerd: twee keer draaien geeft een
byte-identiek bestand.

  python3 generatoren/bouw-wetsgeschiedenis.py
  python3 generatoren/bouw-wetsgeschiedenis.py --poc <checkout van NederlandseDigitaleDienst/regelrecht>

DE POC-VORM. Achter het poc-portaal (poc.regelrecht.rijks.app) laat de CSP
geen inline script toe en geen lettertype als data-URI. `--poc` schrijft
dezelfde pagina daarom in losse delen in een checkout van de monorepo: de
vaste bestanden uit voor-regelrecht/poc-wetsgeschiedenis/ (package.json,
vite.config.js, de tests), de pagina, de opmaak en het script uit dit
sjabloon als src/index.html, src/stijl.css en src/pagina.js, en per dossier een JSON in
corpus-poc/wetsgeschiedenis/gegevens/, die de pagina ophaalt. Het
ontwerpsysteem komt daar uit het npm-pakket, niet uit bron/huisstijl/nldd.
Er is één sjabloon voor beide vormen; zie voor-regelrecht/poc-wetsgeschiedenis/LEESMIJ.md.

DE TOELICHTING PER ARTIKEL. `kamertoelichting.py` knipt de memorie van toelichting,
de verslagen, de nota's naar aanleiding daarvan en de amendementen in
brokken, elk aan een artikel; de pagina toont ze rechts, in de tijd waarin ze
verschenen, tussen de wijzigingen van datzelfde artikel door.

Voorstel M526.
"""
import base64
import json
import os
import re
import sys

HIER = os.path.dirname(os.path.abspath(__file__))
WERKBANK = os.path.dirname(HIER)
BASIS = os.path.dirname(WERKBANK)
DOCS = os.environ.get("OUTDIR") or os.path.join(BASIS, "werkdocumenten")
sys.path.insert(0, HIER)

import afspelen  # noqa: E402
import handelingen  # noqa: E402
import kamertoelichting  # noqa: E402
import keten  # noqa: E402

# Elk Kamerdossier in deze map met een dossier.yaml komt op de pagina; het
# eerste in VOORAAN staat bovenaan, de rest op nummer.
# De poc-dienst verlegt bron en werk naar haar eigen datamap (zie kamerdossier.py).
KAMERDOSSIERS = os.path.join(os.environ.get("WETSGESCHIEDENIS_BRON") or os.path.join(WERKBANK, "bron"), "kamerdossiers")
VOORAAN = ("36742",)
WERK = os.environ.get("WETSGESCHIEDENIS_WERK") or os.path.join(WERKBANK, "werk", "afspelen")
SJABLOON = os.path.join(HIER, "wetsgeschiedenis-sjabloon.html")
UIT = os.path.join(DOCS, "Concept Wetsgeschiedenis — Napp.html")
HUISSTIJL = os.path.join(WERKBANK, "bron", "huisstijl")
NLDD = os.path.join(HUISSTIJL, "nldd")
# In deze volgorde: de kleurpaletten, de tokens die ernaar wijzen, de reset en
# de opmaak van lopende tekst. De @import-regels vallen weg; zij wijzen naar
# bestanden die hier al aan elkaar staan.
NLDD_CSS = ("colors.generated.css", "variables.css", "document-reset.css", "rich-text.css")


def lees(pad, modus="r"):
    with open(pad, modus, **({} if "b" in modus else {"encoding": "utf-8"})) as fh:
        return fh.read()


def huisstijl_css():
    delen = [re.sub(r"^@import [^;]+;\s*$", "", lees(os.path.join(NLDD, f)), flags=re.M) for f in NLDD_CSS]
    return "\n".join(delen)


def huisstijlfont():
    """Rijksoverheid Sans als @font-face met het bestand erin gebakken; zonder
    het bestand valt de pagina terug op de systeemstapel van de tokens."""
    pad = os.path.join(HUISSTIJL, "RijksSansWeb-Regular.woff2")
    if not os.path.exists(pad):
        print("  (huisstijlfont niet ingebakken: %s ontbreekt)" % pad)
        return ""
    data = base64.b64encode(lees(pad, "rb")).decode("ascii")
    return ("@font-face{font-family:'RijksSans';"
            "src:url(data:font/woff2;base64,%s) format('woff2-variations');"
            "font-weight:100 900;font-style:normal;font-display:swap}" % data)


def dossiers():
    """De Kamerdossiers met een dossier.yaml, in hun volgorde op de pagina."""
    nrs = [d for d in os.listdir(KAMERDOSSIERS)
           if os.path.exists(os.path.join(KAMERDOSSIERS, d, "dossier.yaml"))]
    return sorted(nrs, key=lambda d: (d not in VOORAAN, VOORAAN.index(d) if d in VOORAAN else 0, d))


def tijdlijn(nr):
    """Speel één dossier af, knip de toelichting en lees wat het opleverde."""
    map_ = os.path.join(KAMERDOSSIERS, nr)
    werk = os.path.join(WERK, nr)
    stukken = handelingen.main(map_, werk, stil=True)
    uitkomst = afspelen.main(map_, stukken, uit=werk, stil=True)
    with open(os.path.join(werk, "tijdlijn.json"), encoding="utf-8") as fh:
        t = json.load(fh)
    t["commentaar"] = kamertoelichting.main(map_, stukken, t)
    with open(os.path.join(werk, "toelichting.json"), "w", encoding="utf-8") as fh:
        json.dump(t["commentaar"], fh, ensure_ascii=False, indent=1)
    return t, uitkomst


def in_script(tekst):
    # Binnen <script> mag nergens '</' staan, anders sluit de browser het blok.
    return tekst.replace("</", "<\\/")


def verzamel():
    """Speel elk dossier en elke keten af. Geeft de index voor de keuzelijst,
    de gegevens per id en de slotregels."""
    index, gegevens_, regels = [], {}, []
    for nr in dossiers():
        t, uitkomst = tijdlijn(nr)
        index.append({"dossier": nr, "naam": t["naam"], "soort": t["soort"]})
        gegevens_[nr] = t
        tel, c = uitkomst["tel"], t["commentaar"]["tel"]
        eind = uitkomst["ijkpunten"][-1] if uitkomst["ijkpunten"] else None
        regels.append(f"  {nr} {t['naam']}: {len(t['stappen'])} handelingen ({tel['afgespeeld']} afgespeeld, "
                      f"{tel.get('aanname', 0)} met aanname, {tel['niet']} niet); "
                      + (f"{eind[1]} artikelen gelijk aan {eind[0]}" if eind else "geen ijkpunt")
                      + f"; {c['brokken']} brokken toelichting"
                      + "".join(f"; doorwerking in de {d[0]} volgens {d[1]}: {d[2]['afgespeeld']} van "
                                f"{sum(d[2].values())}" for d in uitkomst["doorwerking"]))
    # Een wet door de tijd (keten.py): elke versie van bron/regelingen/<BWB>.
    # Elk dossier dat hier is, hangt zijn toelichting aan zijn overgang; een
    # dossier dat er niet is, haal je met kamerdossier.py --dossier.
    for bwb in keten.regelingen():
        t = keten.bouw(bwb, tijdlijn_van=gegevens_.get)
        werk = os.path.join(WERK, bwb)
        os.makedirs(werk, exist_ok=True)
        with open(os.path.join(werk, "tijdlijn.json"), "w", encoding="utf-8") as fh:
            json.dump(t, fh, ensure_ascii=False, separators=(",", ":"))
        index.append({"dossier": bwb, "naam": t["naam"], "soort": t["soort"]})
        gegevens_[bwb] = t
        sluit = sum(1 for d in t["doorwerking"] if not d["versies"][0]["ijk"]["verschillend"])
        regels.append(f"  {bwb} {t['naam']}: {len(t['stappen'])} overgangen tussen versies op wetten.overheid.nl, "
                      f"{sluit} sluiten (het Staatsblad levert precies de volgende versie)")
    return index, gegevens_, regels


def json_kort(x):
    return json.dumps(x, ensure_ascii=False, separators=(",", ":"))


def vul(html, plekken):
    for plek, inhoud in plekken:
        if html.count(plek) != 1:
            sys.exit(f"sjabloon: '{plek}' moet precies eenmaal voorkomen")
        voor, na = html.split(plek)
        html = voor + inhoud + na
    return html


def los(index, gegevens_):
    """Het losse bestand: alles ingebakken, het laadt geen byte van buiten."""
    blokken = [f'<script type="application/json" id="{i}">' + in_script(json_kort(t)) + "</script>"
               for i, t in gegevens_.items()]
    html = vul(lees(SJABLOON), (("/*NLDD_CSS*/", huisstijl_css()),
                                ("/*FONT*/", huisstijlfont()),
                                ("/*DOSSIERS*/", in_script(json.dumps(index, ensure_ascii=False))),
                                ("<!--GEGEVENS-->", "\n".join(blokken)),
                                ("<!--NLDD-->", "<script>" + in_script(lees(os.path.join(NLDD, "nldd.min.js"))) + "</script>")))
    os.makedirs(DOCS, exist_ok=True)
    with open(UIT, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(html)
    return UIT


# De poc-vorm. De vaste bestanden staan in voor-regelrecht; wat uit het
# sjabloon komt, schrijft dit script erbij.
POC = os.path.join(BASIS, "voor-regelrecht", "poc-wetsgeschiedenis")
POC_APP = "frontend-poc-wetsgeschiedenis"
POC_GEGEVENS = os.path.join("corpus-poc", "wetsgeschiedenis", "gegevens")
POC_DIENST = os.path.join("packages", "poc-wetsgeschiedenis")
# Wat de dienst draait: de keten van BWB-id tot tijdlijn, ongewijzigd.
AFSPELER = ("kamerdossier.py", "haal-kamerstukken.py", "handelingen.py", "afspelen.py", "kamertoelichting.py",
            "keten.py", "wetten_html.py", "oude_kamerstukken.py", "bouw-wetsgeschiedenis.py")
STIJL = re.compile(r"<style>\n(.*?)</style>\n", re.S)
SCRIPT = re.compile(r'<script type="module">\n(.*?)</script>\n', re.S)


def schrijf(pad, tekst):
    os.makedirs(os.path.dirname(pad), exist_ok=True)
    with open(pad, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(tekst)


def in_poc(i):
    """Of een dossier mee mag naar de publieke monorepo; `poc: nee` in zijn
    dossier.yaml houdt het thuis. Een keten (bron/regelingen) komt alleen uit
    wetten.overheid.nl en de Staatsbladen en gaat altijd mee."""
    pad = os.path.join(KAMERDOSSIERS, i, "dossier.yaml")
    if not os.path.exists(pad):
        return True
    return afspelen.lees_dossier(os.path.dirname(pad)).get("poc", True) not in (False, "nee")


def poc(doel, index, gegevens_, vorm="html"):
    """Schrijf de poc in een checkout van de monorepo (`doel`).

    vorm "html": de pagina zoals het sjabloon haar maakt (src/index.html).
    vorm "vue": een proef (21-10-2026) met Vue als schil van de app. Dan
    schrijft dit script alleen wat uit het sjabloon komt: src/Bekijker.vue
    (de opmaak van de pagina als template, zonder het venster 'Wet ophalen',
    dat een eigen component is), src/pagina.js, src/stijl.css, de afspeler en
    de gegevens. De vaste bestanden van die vorm (App.vue, het venster, main.js,
    package.json) staan in de checkout zelf."""
    index = [d for d in index if in_poc(d["dossier"])]
    gegevens_ = {i: t for i, t in gegevens_.items() if in_poc(i)}
    if not os.path.exists(os.path.join(doel, "pocs", "registry.yaml")):
        sys.exit(f"--poc: {doel} is geen checkout van NederlandseDigitaleDienst/regelrecht (pocs/registry.yaml ontbreekt)")
    html = lees(SJABLOON)
    stijl, script = STIJL.findall(html), SCRIPT.findall(html)
    if len(stijl) != 1 or len(script) != 1:
        sys.exit("sjabloon: de poc-vorm verwacht precies een <style> en een <script type=\"module\">")
    css = vul(stijl[0], (("/*NLDD_CSS*/", ""), ("/*FONT*/", "")))
    js = vul(script[0], (('const BASIS = "./";', "const BASIS = import.meta.env.BASE_URL;"),))
    html = STIJL.sub("", html, count=1)
    html = SCRIPT.sub('<script type="module" src="./main.js"></script>\n', html, count=1)
    html = vul(html, (('<script type="application/json" id="dossiers">/*DOSSIERS*/</script>\n', ""),
                      ("<!--GEGEVENS-->\n", ""), ("<!--NLDD-->\n", "")))
    # De CSP van het portaal (POC_CSP) laat geen inline script toe, ook geen
    # onclick="...": wat er dan overblijft, draait daar niet.
    inline = [m for m in re.finditer(r"<script\b([^>]*)>(.*?)</script>", html, re.S)
              if "src=" not in m[1] or m[2].strip()]
    if inline or re.search(r"\son[a-z]+=", html):
        sys.exit("sjabloon: de poc-vorm houdt inline script over; de CSP van het portaal weigert dat")
    app = os.path.join(doel, POC_APP)
    kop = ("/* Gemaakt door werkbank/generatoren/bouw-wetsgeschiedenis.py --poc uit\n"
           "   wetsgeschiedenis-sjabloon.html in het Wpp-dossier; wijzig het daar. */\n")
    if vorm == "vue":
        lijf = re.search(r"<body>\n?(.*?)</body>", html, re.S)[1]
        lijf = re.sub(r'<dialog class="ophaal".*?</dialog>\n?', "", lijf, count=1, flags=re.S)
        lijf = re.sub(r'<script type="module" src="\./main\.js"></script>\n?', "", lijf)
        if "<script" in lijf or "{{" in lijf:
            sys.exit("sjabloon: de opmaak van de pagina past niet in een Vue-template")
        schrijf(os.path.join(app, "src", "Bekijker.vue"),
                "<!-- Gemaakt door werkbank/generatoren/bouw-wetsgeschiedenis.py --poc --vue uit\n"
                "     wetsgeschiedenis-sjabloon.html in het Wpp-dossier; wijzig het daar. -->\n"
                "<template>\n  <div class=\"bekijker\">\n" + lijf.rstrip() + "\n  </div>\n</template>\n\n"
                "<script setup>\n// De bekijker is de pagina van het sjabloon; pagina.js tekent erin zodra\n"
                "// deze opmaak in het document staat. Vue raakt haar daarna niet meer aan.\n"
                "import { onMounted } from 'vue';\n\nonMounted(() => import('./pagina.js'));\n</script>\n")
    else:
        schrijf(os.path.join(app, "src", "index.html"), html)
    schrijf(os.path.join(app, "src", "stijl.css"), kop + css)
    schrijf(os.path.join(app, "src", "pagina.js"), kop + js)
    # De vaste bestanden: package.json, vite.config.js, de tests, de LEESMIJ.
    for wortel, mappen, namen in (os.walk(POC) if vorm == "html" else ()):
        mappen[:] = [m for m in mappen if m not in ("__pycache__", "data", "node_modules", "dist")]
        for naam in namen:
            bron = os.path.join(wortel, naam)
            rel = os.path.relpath(bron, POC)
            if rel in ("LEESMIJ.md", "registry-fragment.yaml", "bedrading.patch"):
                continue
            schrijf(os.path.join(doel, rel), lees(bron))
    # De afspeelketen zelf, voor de dienst die een wet ophaalt vanaf haar BWB-id.
    afspeler = os.path.join(doel, POC_DIENST, "afspeler")
    if os.path.isdir(afspeler):
        for naam in os.listdir(afspeler):
            if naam.endswith(".py"):
                os.remove(os.path.join(afspeler, naam))
    for naam in AFSPELER:
        schrijf(os.path.join(afspeler, naam), lees(os.path.join(HIER, naam)))
    map_ = os.path.join(doel, POC_GEGEVENS)
    if os.path.isdir(map_):
        for naam in os.listdir(map_):
            if naam.endswith(".json"):
                os.remove(os.path.join(map_, naam))
    schrijf(os.path.join(map_, "dossiers.json"), json.dumps(index, ensure_ascii=False, indent=1) + "\n")
    for i, t in gegevens_.items():
        schrijf(os.path.join(map_, f"{i}.json"), json_kort(t) + "\n")
    return app


def main():
    args = sys.argv[1:]
    doel = None
    vorm = "vue" if "--vue" in args else "html"
    args = [a for a in args if a != "--vue"]
    if args[:1] == ["--poc"] and len(args) == 2:
        doel = os.path.abspath(args[1])
    elif args:
        sys.exit("gebruik: bouw-wetsgeschiedenis.py [--poc <checkout van NederlandseDigitaleDienst/regelrecht> [--vue]]")
    index, gegevens_, regels = verzamel()
    uit = los(index, gegevens_)
    print(f"wetsgeschiedenis: {len(index)} dossiers, {os.path.getsize(uit) // 1024} kB")
    print("\n".join(regels))
    print(f"  -> {os.path.relpath(uit, BASIS)}")
    if doel:
        app = poc(doel, index, gegevens_, vorm)
        omvang = sum(os.path.getsize(os.path.join(doel, POC_GEGEVENS, f)) for f in os.listdir(os.path.join(doel, POC_GEGEVENS)))
        print(f"  -> poc in {app} en {POC_GEGEVENS} ({omvang // 1024} kB gegevens)")


if __name__ == "__main__":
    main()
