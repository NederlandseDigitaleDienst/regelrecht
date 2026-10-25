# -*- coding: utf-8 -*-
"""Een werker zonder internet, met hetzelfde contract als werker.py: de
uitkomst naar --uit, @uit en @fout op stderr. Een dossier wacht zolang het
bestand `poort` in de bron bestaat, zodat een test de rij kan stilzetten."""
import json
import os
import sys
import time
from pathlib import Path

args = sys.argv[1:]
wat = args[0]
optie = lambda n: args[args.index(n) + 1] if n in args else None  # noqa: E731
uit = optie("--uit")
bron = Path(os.environ["WETSGESCHIEDENIS_BRON"])


def schrijf(x):
    Path(uit).write_text(json.dumps(x), encoding="utf-8")


def fout(tekst):
    sys.stderr.write("@fout " + json.dumps({"tekst": tekst}) + "\n")
    sys.exit(1)


def er(nr):
    return any((Path(m) / f"{nr}.json").is_file() for m in (optie("--gegevens") or "").split(":") if m)


sys.stderr.write("@uit " + json.dumps({"via": "curl", "host": "wetten.overheid.nl", "pad": f"/{args[1]}/", "ms": 3}) + "\n")
if wat == "regeling":
    if args[1] == "BWBR0029404":
        sys.stderr.write("@uit " + json.dumps({"via": "curl", "host": "wetten.overheid.nl", "pad": "/x", "ms": 15001,
                                               "code": 28, "uitleg": "geen antwoord binnen de tijd"}) + "\n")
        fout("wetten.overheid.nl: geen antwoord binnen de tijd (curl 28)")
    schrijf({"bwb": args[1], "titel": "Archiefwet 1995|x", "afkorting": None, "versies": [
        {"inwerking": "2020-01-01", "betreft": "Wijziging", "staatsblad": "stb-2020-1", "dossiers": ["36889"], "artikelen": ["1"]},
        {"inwerking": "2019-01-01", "betreft": "Wijziging", "staatsblad": "stb-2019-1", "dossiers": [], "artikelen": []}]})
elif wat in ("keten", "opbouw"):
    if wat == "keten":
        k = bron / "regelingen" / args[1]
        k.mkdir(parents=True, exist_ok=True)
        keten = {"overgangen": [
            {"tot": "2001-01-01", "dossiers": ["10101"]},
            {"tot": "2020-01-01", "dossiers": ["30303", "35968"]},   # 35968 zit in het image
            {"tot": "2010-01-01", "dossiers": ["20202", "40404"]}], "afgekapt": None, "ontstaan": []}
        # Een grote wet: eerst de jongste versies; met --alles de hele
        # geschiedenis, met de dossiers van vóór 1995 die haar maakten.
        if args[1] == "BWBR0005537" and "--alles" not in args:
            keten["afgekapt"] = {"versies": 218, "sinds": "1994-01-01"}
        elif args[1] == "BWBR0005537":
            keten["ontstaan"] = ["21221", "22495"]
            keten["overgangen"].insert(0, {"tot": "1994-01-01", "dossiers": ["21221", "22495"]})
        (k / "keten.json").write_text(json.dumps(keten))
        print("versies opgehaald")
    status = json.loads(Path(optie("--status")).read_text()) if optie("--status") else {}
    schrijf({"dossier": args[1], "naam": f"{args[1]} door de tijd", "soort": "keten",
             "dossiers_er": [nr for nr in ("35968", "10101", "20202", "30303") if er(nr)], "status": status})
elif wat == "dossier":
    while (bron / "poort").exists():
        time.sleep(0.02)
    if args[2] == "40404":
        fout("geen XML")
    print("stukken opgehaald")
    schrijf({"dossier": args[2], "naam": f"Dossier {args[2]}", "soort": "wijzigingswet"})
