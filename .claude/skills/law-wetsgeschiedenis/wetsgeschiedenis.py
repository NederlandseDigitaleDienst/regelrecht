#!/usr/bin/env python3
"""Query the legislative history that the wetsgeschiedenis replay chain builds.

The chain (packages/poc-wetsgeschiedenis/) fetches a Kamerdossier, replays every
nota van wijziging and adopted amendment on the original bill, and records for
every line of the law which document its text came from. It also hangs the
explanatory passages (memorie van toelichting, nota naar aanleiding van het
verslag, amendment explanations) on the article they discuss. The result is one
timeline JSON per dossier, the same file the wetsgeschiedenis PoC renders.

This script answers the questions a researcher drafting a regelwerk asks of
that file, per article and lid, with a citable document for every answer:

    lijst                                  which timelines are available
    stukken     <id>                       the documents of a dossier, in order
    artikel     <id> <nr> [--lid N]        the text, line by line with its origin
    herkomst    <id> <nr> [--lid N]        the amendments that touched it, and its versions
    toelichting <id> <nr> [--lid N]        what the explanatory documents say about it
    toelichting <id> --doelwet <nr>        same, for an article of the law a bill amends
    wet         <id> [--herkomst]          the whole law at a moment, checked per article
    vergelijk   <id> [<nr>] --van M [--tot M]   what changed between two moments, word by word
    haal        <BWBR...|dossier> [dossier] [--keten]   fetch and replay (needs network)
    haal        <BWBR...> --opbouw         rebuild a law through time from what is fetched

<id> is a dossier number (35968), or a BWB id for a law through time. The moment
is --op DATE (the state on that day), --na STUK (right after that document) or
--stap N (right after replay step N); the default is the last one. For
vergelijk a moment M is written as a date, a document id, or stap:N. --json
prints the answer as JSON instead of text.

The replay is not perfect, and an answer that hides that is worse than none:
every text answer states whether the replayed article equals the official text
(the bill as adopted, the Staatsblad, or the BWB), and which amendments did not
replay. Python standard library only; `haal` additionally needs pyyaml and curl.
"""
from __future__ import annotations

import argparse
import difflib
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
BUNDLED = ROOT / "corpus-poc" / "wetsgeschiedenis" / "gegevens"
WORKER = ROOT / "packages" / "poc-wetsgeschiedenis" / "werker.py"

KIND_NAMES = {
    "voorstel": "voorstel van wet", "nota": "nota van wijziging", "ek": "Eerste Kamer: gewijzigd voorstel",
    "memorie": "memorie van toelichting", "advies": "Raad van State", "vraag": "verslag (vragen)",
    "antwoord": "nota n.a.v. verslag (antwoorden)", "amendement": "amendement", "brief": "brief of ander stuk",
}
STATUS_NAMES = {"ok": "afgespeeld", "aanname": "afgespeeld met aanname", "niet": "niet afgespeeld"}
PUBLICATION = re.compile(r"^(kst|stb|stcrt|ah|h-tk|h-ek|blg)-", re.I)
show_origin = False  # `wet --herkomst`: print each line's origin in the text output


class Fault(Exception):
    """A question the data cannot answer; the message says why."""


def cache_dir() -> Path:
    return Path(os.environ.get("WETSGESCHIEDENIS_CACHE") or Path.home() / ".cache" / "regelrecht" / "wetsgeschiedenis")


def data_dirs() -> list[Path]:
    """Fetched timelines first, so a fresh fetch wins over the bundled copy."""
    extra = [Path(p) for p in (os.environ.get("WETSGESCHIEDENIS_GEGEVENS") or "").split(os.pathsep) if p]
    return [*extra, cache_dir() / "gegevens", BUNDLED]


def valid_id(ident: str) -> bool:
    return bool(re.fullmatch(r"\d{5}(?:-[A-Z0-9]{1,6})?|BWBR\d{7}", ident))


def load(ident: str) -> tuple[dict, Path]:
    if not valid_id(ident):
        raise Fault(f"'{ident}' is geen dossiernummer (vijf cijfers) en geen BWB-id (BWBR0000000)")
    for d in data_dirs():
        p = d / f"{ident}.json"
        if p.is_file():
            try:
                return json.loads(p.read_text(encoding="utf-8")), p
            except ValueError as e:
                raise Fault(f"{p} is geen leesbare tijdlijn ({e}); haal hem opnieuw op") from None
    raise Fault(f"geen tijdlijn voor {ident}; haal hem op met: wetsgeschiedenis.py haal {ident}")


def norm(text: str) -> str:
    """The comparison the PoC uses: quote styles and whitespace do not count."""
    text = re.sub(r"[“„]", "«", text).replace("”", "»")
    text = re.sub(r"[‘’']", "'", text)
    return " ".join(text.split())


def article_nr(raw: str) -> str:
    return re.sub(r"^(artikel|art\.?)\s+", "", raw.strip(), flags=re.I)


class History:
    """One timeline: the replayed states, the documents and the commentary."""

    def __init__(self, data: dict):
        self.d = data
        # One id can name two entries: in a law through time the oldest BWB version
        # shares its id with the act that made it. The lowest `volgorde` is what the
        # timeline means by that id (its starting text).
        self.docs: dict[str, dict] = {}
        for s in sorted(data["stukken"], key=lambda s: s.get("volgorde", 0)):
            self.docs.setdefault(s["stuk"], s)
        self.chain = data.get("soort") == "keten"
        self.steps = data["stappen"]
        self.ident = data.get("identiteit") or {}
        # Positions on the timeline, exactly as the page builds them: a moment,
        # then each replay step that moment brings.
        self.positions: list[dict] = []
        self.doc_pos: dict[str, int] = {}
        self.step_moment: dict[int, dict] = {}
        done = 0
        for m in data["momenten"]:
            if m.get("soort") == "stuk":
                self.doc_pos[m["stuk"]] = len(self.positions)
            self.positions.append({"moment": m, "steps": done})
            if m.get("stappen"):
                first, last = m["stappen"]
                for i in range(first, last + 1):
                    done = i
                    self.step_moment[i] = m
                    self.positions.append({"moment": m, "steps": i})
        self._cache: dict[int, tuple[dict, list]] = {}

    # ---------------------------------------------------------------- moments
    def last_of(self, p: int) -> int:
        while p + 1 < len(self.positions) and self.positions[p + 1]["moment"] is self.positions[p]["moment"]:
            p += 1
        return p

    def position(self, on: str | None = None, after: str | None = None, step: int | None = None) -> int:
        if step is not None:
            if not 0 <= step <= len(self.steps):
                raise Fault(f"er zijn {len(self.steps)} handelingen; --stap gaat van 0 (het begin) tot {len(self.steps)}")
            hits = [i for i, x in enumerate(self.positions) if x["steps"] == step]
            # Step 0 is the text before the first step; step N the position of that step itself.
            return hits[-1] if step == 0 else hits[0]
        if after:
            if after not in self.doc_pos:
                raise Fault(f"stuk {after} staat niet in dit dossier; zie 'stukken'")
            return self.last_of(self.doc_pos[after])
        if on:
            start = self.docs.get(self.d.get("voorstel"), {}).get("datum") or ""
            if self.chain and on < start:
                # Earlier moments are on the timeline, but their text is not: the replay starts here.
                raise Fault(f"de afspeling van deze wet begint bij {start}, de oudste versie op wetten.overheid.nl "
                            f"die is opgehaald; een tekst van {on} is er niet. Met 'haal {self.d['dossier']} --keten "
                            "--alles' komen alle versies mee, maar wetten.overheid.nl gaat niet verder terug dan haar oudste versie")
            hits = [i for i, x in enumerate(self.positions) if (x["moment"].get("datum") or "") <= on]
            if not hits:
                raise Fault(f"{on} ligt voor het eerste stuk ({self.positions[0]['moment'].get('datum')})")
            return self.last_of(hits[-1])
        return len(self.positions) - 1

    def resolve(self, spec: str) -> tuple[int, str]:
        """A moment written as a date, a document id, or stap:N; returns the position and the check cut-off."""
        spec = spec.strip()
        if m := re.fullmatch(r"stap:(\d+)", spec):
            p = self.position(step=int(m.group(1)))
            return p, self.date_at(p)
        if re.fullmatch(r"\d{4}-\d{2}-\d{2}", spec):
            return self.position(on=spec), spec
        p = self.position(after=spec)
        return p, self.date_at(p)

    def date_at(self, p: int) -> str:
        return self.positions[p]["moment"].get("datum") or ""

    def describe(self, p: int) -> str:
        """The moment in words: its date, and after which step when it sits inside a document."""
        x = self.positions[p]
        m = x["moment"]
        what = f"stand op {self.date_at(p)}"
        n = x["steps"]
        if n and p != self.last_of(p):
            s = self.steps[n - 1]
            what += (f", na handeling {n} van {len(self.steps)} ("
                     + self.cite(s["stuk"], *(() if self.chain else (s.get("onderdeel"), s.get("sub"))))["verwijzing"] + ")")
        elif m.get("soort") == "stemming":
            what += ", na de stemming"
        elif m.get("stuk") and not self.chain:
            # Several documents can share a day; name the one this state follows.
            what += f", na {self.cite(m['stuk'])['verwijzing']}"
        return what

    def state(self, steps: int) -> tuple[dict, list]:
        if steps not in self._cache:
            blocks = dict(self.d["begin"]["blokken"])
            order = self.d["begin"]["volgorde"]
            for s in self.steps[:steps]:
                blocks.update(s["wijzig"])
                for gone in s["weg"]:
                    blocks.pop(gone, None)
                if s.get("volgorde"):
                    order = s["volgorde"]
            self._cache[steps] = (blocks, order)
        return self._cache[steps]

    def identity(self, key: str) -> str:
        return self.ident.get(key, key)

    def article(self, steps: int, nr: str) -> tuple[str | None, dict | None]:
        blocks, order = self.state(steps)
        for k in order:
            b = blocks.get(k)
            if b and b["soort"] == "artikel" and b["nr"] == nr:
                return k, b
        return None, None

    def article_by_identity(self, steps: int, ident: str) -> dict | None:
        blocks, order = self.state(steps)
        for k in order:
            b = blocks.get(k)
            if b and b["soort"] == "artikel" and self.identity(k) == ident:
                return b
        return None

    # ------------------------------------------------------------- documents
    def cite(self, stuk: str, part: str | None = None, sub: str | None = None) -> dict:
        doc = self.docs.get(stuk, {})
        kst = re.fullmatch(r"kst-(\d{5}(?:-[A-Z0-9]+)?)-(\w+)", stuk)
        # A Kamerstuk is cited by dossier and number, also one from another dossier.
        label = f"Kamerstuk {kst.group(1)}, nr. {kst.group(2)}" if kst else doc.get("titel") or stuk
        if self.chain and stuk == self.d.get("voorstel"):
            # Not an act: the oldest text the BWB has. Who wrote it is not replayed.
            label = (f"de tekst zoals op wetten.overheid.nl op {doc.get('datum')} "
                     "(de oudste versie die is opgehaald; wat haar zo maakte is niet afgespeeld)")
        if part:
            label += f", onderdeel {part}" + (f" onder {sub}" if sub else "")
        return {"stuk": stuk, "verwijzing": label, "soort": doc.get("soort") or "", "datum": doc.get("datum") or "",
                "url": self.url(stuk)}

    def url(self, stuk: str) -> str | None:
        if PUBLICATION.match(stuk):
            return f"https://zoek.officielebekendmakingen.nl/{stuk}"
        m = re.fullmatch(r"versie-(\d{4}-\d{2}-\d{2})", stuk)
        if m and re.fullmatch(r"BWBR\d{7}", self.d.get("dossier", "")):
            return f"https://wetten.overheid.nl/{self.d['dossier']}/{m.group(1)}"
        return None

    def ref(self, i: int) -> dict:
        r = self.d["refs"][i]
        return self.cite(r["stuk"], r.get("onderdeel"), r.get("sub"))

    # ----------------------------------------------------------- the check
    def check(self, nr: str, lines: list[str], until: str) -> dict:
        """Does the replayed article equal the last official text dated on or before `until`?

        Checkpoints are not always moments on the timeline, so the cut-off is the
        date asked for, not the date of the moment that answers it."""
        if self.chain:
            # keten.py takes every version's text from wetten.overheid.nl; only the
            # Staatsblad is replayed, as a check on the transition, not as the text.
            return {"oordeel": "bwb", "uitleg": "dit is de tekst van wetten.overheid.nl zelf, niet afgespeeld"}
        points = [x for x in self.d.get("ijkpunten") or [] if (x.get("datum") or "") <= until]
        if not points:
            return {"oordeel": "geen ijkpunt", "uitleg": "er is op dit moment nog geen officiële tekst om mee te vergelijken"}
        point = points[-1]
        official = (point.get("artikelen") or {}).get(nr)
        base = {"ijkpunt": point.get("naam"), "datum": point.get("datum")}
        if official is None:
            return {**base, "oordeel": "onbekend", "uitleg": f"{point.get('naam')} kent geen artikel {nr}"}
        # Line by line, as the page compares: a different split into leden is a difference.
        if [norm(x) for x in official] == [norm(x) for x in lines]:
            return {**base, "oordeel": "gelijk", "uitleg": f"woord voor woord gelijk aan {point.get('naam')}"}
        return {**base, "oordeel": "wijkt af", "officieel": official,
                "uitleg": f"wijkt af van {point.get('naam')}: de afspeling is hier niet betrouwbaar, citeer de officiële tekst"}

    def known_article(self, nr: str) -> bool | None:
        """Is there an article with this number in the last official text? None when there is none to ask."""
        points = self.d.get("ijkpunten") or []
        if self.chain or not points:
            return None
        return nr in (points[-1].get("artikelen") or {})

    def notes(self, nr: str | None, until: str, membership: bool = True) -> list[str]:
        """What the text alone does not say: the article is not in the official text, the law was
        repealed, or a pending bill changes this article."""
        out = []
        if membership and nr and self.known_article(nr) is False:
            out.append(f"artikel {nr} staat niet in {self.d['ijkpunten'][-1].get('naam')}: mogelijk geen artikel van "
                       "deze wet, maar een artikel van een wet die zij wijzigt, dat de afspeling als eigen artikel las")
        for s in self.d["stukken"]:
            kind = s.get("soort") or ""
            if kind == "Intrekking" and (s.get("datum") or "") <= until:
                out.append(f"ingetrokken: {s.get('titel')}. Deze tekst geldt niet meer.")
            elif kind == "Aanhangig voorstel":
                named = re.search(r"raakt artikel(?:en)?\s+(.*)$", s.get("titel") or "")
                touched = {x.strip() for x in re.split(r",|\ben\b", named.group(1))} if named else None
                if nr is None or touched is None or nr in touched:
                    out.append(f"aanhangig: {s.get('titel')}. Bekijk het met: wetsgeschiedenis.py haal "
                               f"{self.d['dossier']} {s.get('ondernummer')}")
        return out

    # -------------------------------------------------------------- steps
    def steps_touching(self, ident: str) -> list[int]:
        """Steps that changed this article, or that did not replay but name it."""
        out = []
        for i, s in enumerate(self.steps, start=1):
            ids = {self.identity(k) for k, b in s["wijzig"].items() if b.get("soort") == "artikel"}
            if not ids:
                m = re.search(r"artikel ((?:[A-Z][a-z]? )?\d+[a-z]*(?:[.:]\d+[a-z]*)*|[IVXL]+)", s.get("opdracht") or "", re.I)
                if m:
                    k, _ = self.article(i - 1, m.group(1))
                    if k:
                        ids.add(self.identity(k))
            if ident in ids:
                out.append(i)
        return out


# ------------------------------------------------------------------ members
def members(block: dict) -> list[tuple[str | None, list]]:
    """Split an article into (lid number, lines); the heading has no number."""
    groups: list[tuple[str | None, list]] = []
    for line in block.get("regels", []):
        text, _, depth, kind = line
        if kind == "lid" and depth == 0:
            # A bill writes "1. De …", the BWB "1 De …".
            m = re.match(r"(\d+[a-z]*)\.?\s", text)
            groups.append((m.group(1) if m else None, [line]))
        elif groups and groups[-1][0] is not None and kind != "kop":
            groups[-1][1].append(line)
        else:
            groups.append((None, [line]))
    return groups


def lines_of(block: dict, lid: str | None) -> list:
    if lid is None:
        return block.get("regels", [])
    for nr, lines in members(block):
        if nr == lid:
            return lines
    found = [n for n, _ in members(block) if n]
    raise Fault(f"artikel {block['nr']} heeft geen lid {lid}" + (f" (wel: {', '.join(found)})" if found else " (het is niet in leden verdeeld)"))


# ---------------------------------------------------------------- commands
def until(h: History, a, p: int) -> str:
    """The cut-off for the check: the date asked for, the moment after a document or step, or no limit."""
    return a.op or (h.date_at(p) if a.na or getattr(a, "stap", None) is not None else "9999-12-31")


def moment(h: History, a) -> int:
    chosen = [x for x in (a.op, a.na, getattr(a, "stap", None)) if x is not None]
    if len(chosen) > 1:
        raise Fault("kies één moment: --op, --na of --stap")
    return h.position(a.op, a.na, getattr(a, "stap", None))


def find(h: History, nr: str, p: int) -> tuple[str, dict]:
    steps = h.positions[p]["steps"]
    key, block = h.article(steps, nr)
    if not block:
        raise Fault(f"artikel {nr} bestaat niet op {h.date_at(p)}; kies een ander moment met --op of --na")
    return key, block


def quality(h: History) -> dict:
    t = h.d.get("tel") or {}
    counted = {"ok": "afgespeeld", "aanname": "aanname", "niet": "niet"}  # keys of `tel`
    return {"handelingen": {STATUS_NAMES[k]: t.get(counted[k], 0) for k in STATUS_NAMES},
            "ijkpunten": [{"naam": x.get("naam"), "gelijk": x.get("gelijk"), "verschillend": x.get("verschillend")}
                          for x in h.d.get("ijkpunten") or []]}


def cmd_lijst(_a) -> dict:
    seen, rows = set(), []
    for d in data_dirs():
        for p in sorted(d.glob("*.json")) if d.is_dir() else []:
            if p.stem in seen or not valid_id(p.stem):
                continue
            seen.add(p.stem)
            try:
                x = json.loads(p.read_text(encoding="utf-8"))
            except ValueError:
                x = {"naam": "(onleesbaar: haal opnieuw op)", "soort": "?"}
            rows.append({"id": p.stem, "naam": x.get("naam"), "soort": x.get("soort"),
                         "bron": "opgehaald" if d != BUNDLED else "meegeleverd"})
    return {"tijdlijnen": rows}


def cmd_stukken(a) -> dict:
    h = History(load(a.id)[0])
    return {"dossier": h.d["dossier"], "naam": h.d["naam"], "soort": h.d["soort"], "kwaliteit": quality(h),
            "stukken": [{**h.cite(s["stuk"]), "titel": s.get("titel"), "speelt_mee": s.get("speelt_mee"),
                         "uitslag": (s.get("uitslag") or {}).get("uitslag"), "niet_mee": s.get("niet_mee")}
                        for s in h.d["stukken"]]}


def cmd_artikel(a) -> dict:
    h = History(load(a.id)[0])
    p = moment(h, a)
    nr = article_nr(a.nr)
    _, block = find(h, nr, p)
    lines = lines_of(block, a.lid)
    # A line's refs are a chain, oldest first: the last one wrote the current wording.
    return {"dossier": h.d["dossier"], "artikel": nr, "lid": a.lid, "moment": h.date_at(p), "omschrijving": h.describe(p),
            "controle": h.check(nr, [x[0] for x in block["regels"]], until(h, a, p)),
            "let_op": h.notes(nr, until(h, a, p)),
            "regels": [{"tekst": t, "herkomst": [h.ref(r) for r in refs]} for t, refs, _, _ in lines]}


def cmd_herkomst(a) -> dict:
    h = History(load(a.id)[0])
    p = moment(h, a)
    nr = article_nr(a.nr)
    key, block = find(h, nr, p)
    lines_of(block, a.lid)  # fails early on a lid that does not exist
    ident = h.identity(key)
    touching = h.steps_touching(ident)
    touched = []
    for i in touching:
        if i > h.positions[p]["steps"]:
            break
        s = h.steps[i - 1]
        at = h.step_moment.get(i) or {}
        if h.chain:
            # A law through time: the step is a new version from the BWB, made by
            # the bill in `onderdeel`; its status says whether the Staatsblad of
            # that bill could be replayed onto the old version as a check.
            touched.append({"handeling": i, **h.cite(s["stuk"]), "werkt_door_op": at.get("datum"),
                            "bij": "inwerkingtreding", "opdracht": s.get("opdracht"),
                            "status": "Staatsblad nagespeeld" if s["status"] == "ok" else "Staatsblad niet nagespeeld",
                            "reden": s.get("reden") if s["status"] != "ok" else None,
                            "dossiers": (s.get("bronnen") or {}).get("dossiers") or [],
                            "publicaties": (s.get("bronnen") or {}).get("publicaties") or []})
            continue
        # An amendment is filed on one day and takes effect on the day of the vote.
        touched.append({"handeling": i, **h.cite(s["stuk"], s.get("onderdeel"), s.get("sub")),
                        "werkt_door_op": at.get("datum"),
                        "bij": "stemming" if at.get("soort") == "stemming" else "indiening",
                        "opdracht": s.get("opdracht"), "status": STATUS_NAMES.get(s["status"], s["status"]),
                        "reden": s.get("reden") if s["status"] != "ok" else None})
    # The versions: every moment at which the article (or the lid) reads differently,
    # each cited to what made the change.
    versions, last, before = [], None, -1
    for q, x in enumerate(h.positions):
        if q > p or q != h.last_of(q):
            continue
        b = h.article_by_identity(x["steps"], ident)
        text = None
        if b:
            try:
                text = norm(" ".join(t for t, *_ in lines_of(b, a.lid)))
            except Fault:
                text = None  # this lid does not exist at that moment
        if text != last:
            versions.append({"datum": x["moment"].get("datum"), "moment": x["moment"].get("soort"),
                             "door": cause(h, x, before, touching), "nummer": b["nr"] if b else None,
                             "tekst": text or "(bestaat niet)"})
            if before < 0 and x["steps"] == 0 and versions[-1]["door"]:
                versions[-1]["datum"] = versions[-1]["door"][0]["datum"] or versions[-1]["datum"]
            last = text
        before = x["steps"]
    return {"dossier": h.d["dossier"], "artikel": nr, "lid": a.lid, "moment": h.date_at(p), "omschrijving": h.describe(p),
            "controle": h.check(nr, [t for t, *_ in block["regels"]], until(h, a, p)),
            "let_op": h.notes(nr, until(h, a, p)) + (
                ["een lid wordt op zijn nummer gevolgd: na een vernummering verschijnt het lid dat nu dat nummer "
                 "draagt als nieuwe versie"] if a.lid else []),
            "handelingen": touched, "versies": versions}


def cause(h: History, x: dict, before: int, touching: list[int]) -> list[dict]:
    """The documents that made the text read as it does at position x.

    The replay steps since the previous position that touched this article;
    at the very start, the bill (or the oldest BWB text); otherwise the moment's
    own document, or at a vote the amendments adopted there."""
    steps = [i for i in touching if before < i <= x["steps"]]
    if steps:
        return [h.cite(h.steps[i - 1]["stuk"], *(() if h.chain else (h.steps[i - 1].get("onderdeel"), h.steps[i - 1].get("sub"))))
                for i in steps]
    m = x["moment"]
    if x["steps"] == 0:
        return [h.cite(h.d["voorstel"])]
    if m.get("stuk"):
        return [h.cite(m["stuk"])]
    return [h.cite(s) for s in m.get("stukken") or []]


def cmd_toelichting(a) -> dict:
    h = History(load(a.id)[0])
    p = moment(h, a)
    if a.doelwet and a.lid:
        # `leden` in a passage refers to the bill's own articles, not to the law it amends.
        raise Fault("--lid gaat niet samen met --doelwet: de leden in de toelichting zijn die van het voorstel")
    if a.doelwet:
        nr, ident = article_nr(a.doelwet), None
        chunks = [b for b in h.d["commentaar"]["brokken"] if nr in (b.get("doelartikelen") or [])]
    else:
        if not a.nr:
            raise Fault("noem een artikel, of --doelwet <nr> voor een artikel van de gewijzigde wet")
        nr = article_nr(a.nr)
        key, block = find(h, nr, p)
        ident = h.identity(key)
        if a.lid:
            lines_of(block, a.lid)  # fails early on a lid that does not exist
        chunks = [b for b in h.d["commentaar"]["brokken"] if ident in (b.get("ids") or [])]
    explicit = a.op or a.na or a.stap is not None
    out = []
    for b in chunks:
        if explicit and h.doc_pos.get(b["stuk"], 0) > p:
            continue  # not yet written at the chosen moment
        leden = {str(k): [str(x) for x in v] for k, v in (b.get("leden") or {}).items()}
        # `leden` is keyed by the article number the passage itself used, which a
        # renumbering may have changed since; the page looks it up the same way.
        ids, named = b.get("ids") or [], b.get("artikelen") or []
        own = named[ids.index(ident)] if ident in ids and ids.index(ident) < len(named) else nr
        about = leden.get(own)
        if a.lid and about is not None and a.lid not in about:
            continue
        # In a law through time the passage hangs on a version, but comes from a Kamerstuk.
        origin = (b.get("bron") or {}).get("stuk")
        cited = {**h.cite(origin), "datum": b["bron"].get("datum") or b.get("datum")} if origin else h.cite(b["stuk"])
        if origin:
            cited["bij_versie"] = b["stuk"]
        out.append({**cited, "soort": KIND_NAMES.get(b["soort"], b["soort"]), "rol": b.get("rol"),
                    "kop": b.get("kop"), "titel": b.get("titel"),
                    "over": (f"lid {', '.join(about)}" + (f" van artikel {own}" if own != nr else "")) if about
                    else "het hele artikel",
                    "tekst": [t for t, _ in b.get("tekst") or []]})
    loose = []
    if not a.doelwet:
        # "artikel 1", "art. 1", "de artikelen 9 en 1", "artikelen 1 tot en met 3" — but not "artikel 1.2" for 1.
        num = r"\d+[a-z]*(?:[.:]\d+[a-z]*)*"
        pattern = re.compile(rf"\b(?:artikel(?:en)?|art\.)\s+(?:{num}(?:,|\s+en|\s+tot en met)?\s+)*{re.escape(nr)}(?![.:]?\w)", re.I)
        loose = [{**h.cite(x["stuk"]), "reden": x.get("reden"), "tekst": x.get("tekst")}
                 for x in h.d["commentaar"].get("niet") or [] if pattern.search(x.get("tekst") or "")]
    notes = [] if h.d["commentaar"]["brokken"] else [
        "deze tijdlijn draagt geen toelichting. Bij een wet door de tijd: haal de Kamerdossiers achter de overgangen op "
        "(herkomst noemt ze) en daarna 'haal <BWB> --opbouw'"]
    return {"dossier": h.d["dossier"], "artikel": nr, "lid": a.lid, "doelwet": bool(a.doelwet),
            "toelichting": out, "niet_gekoppeld": loose, "let_op": notes}


def cmd_wet(a) -> dict:
    """The whole law at one moment: every block in order, every article checked."""
    h = History(load(a.id)[0])
    p = moment(h, a)
    cut = until(h, a, p)
    blocks, order = h.state(h.positions[p]["steps"])
    out, differs, unknown = [], [], []
    for k in order:
        b = blocks.get(k)
        if not b:
            continue
        item = {"soort": b["soort"], "nr": b.get("nr"),
                "regels": [{"tekst": t, "herkomst": [h.ref(r)["verwijzing"] for r in refs]} for t, refs, _, _ in b.get("regels", [])]}
        if b["soort"] == "artikel":
            c = h.check(b["nr"], [t for t, *_ in b["regels"]], cut)
            item["controle"] = c["oordeel"]
            if c["oordeel"] == "wijkt af":
                differs.append(b["nr"])
            elif c["oordeel"] == "onbekend":
                unknown.append(b["nr"])
        out.append(item)
    articles = [x for x in out if x["soort"] == "artikel"]
    points = [x for x in h.d.get("ijkpunten") or [] if (x.get("datum") or "") <= cut]
    return {"dossier": h.d["dossier"], "naam": h.d["naam"], "moment": h.date_at(p), "omschrijving": h.describe(p),
            "controle": {"ijkpunt": points[-1]["naam"] if points and not h.chain else None, "bwb": h.chain,
                         "artikelen": len(articles), "wijkt_af": differs, "onbekend": unknown},
            "let_op": h.notes(None, cut), "blokken": out}


def word_diff(old: str, new: str) -> str:
    """One line, with removed words as [-…-] and added words as {+…+}."""
    a, b = old.split(), new.split()
    parts = []
    for op, i1, i2, j1, j2 in difflib.SequenceMatcher(a=a, b=b, autojunk=False).get_opcodes():
        if op == "equal":
            parts.append(" ".join(a[i1:i2]))
        if op in ("delete", "replace"):
            parts.append("[-" + " ".join(a[i1:i2]) + "-]")
        if op in ("insert", "replace"):
            parts.append("{+" + " ".join(b[j1:j2]) + "+}")
    return " ".join(parts)


LABEL = re.compile(r"^(\d+[a-z]*\.?|[a-z]\.|\d+°\.)\s+")  # "2." / "2" (BWB) / "a." / "1°."


def unlabelled(line: str) -> str:
    return LABEL.sub("", norm(line), count=1)


def similar(a: str, b: str) -> float:
    return difflib.SequenceMatcher(a=unlabelled(a).split(), b=unlabelled(b).split(), autojunk=False).ratio()


def line_diff(old: list[str], new: list[str]) -> list[dict]:
    """Changed lines only: removed, added, renumbered, or changed (with a word diff).

    Within a changed block lines are paired by content, not by position: when a
    nota inserts a lid, the leden after it are renumbered, not rewritten, and
    pairing by position would diff two unrelated leden against each other."""
    out = []
    for op, i1, i2, j1, j2 in difflib.SequenceMatcher(a=[norm(x) for x in old], b=[norm(x) for x in new], autojunk=False).get_opcodes():
        if op == "equal":
            continue
        olds, news = old[i1:i2], new[j1:j2]
        # Greedy, order-preserving: each old line takes the first sufficiently similar new line after the last match.
        match, nxt = {}, 0
        for i, o in enumerate(olds):
            best = max(range(nxt, len(news)), key=lambda j: similar(o, news[j]), default=None)
            if best is not None and similar(o, news[best]) >= 0.6:
                match[best], nxt = i, best + 1
        done_old = 0
        for j, n in enumerate(news):
            if j not in match:
                out.append({"soort": "nieuw", "nieuw": n})
                continue
            i = match[j]
            out += [{"soort": "weg", "oud": olds[k]} for k in range(done_old, i)]
            done_old = i + 1
            o = olds[i]
            if unlabelled(o) == unlabelled(n):
                out.append({"soort": "vernummerd", "oud": o, "nieuw": n})
            else:
                out.append({"soort": "gewijzigd", "oud": o, "nieuw": n, "verschil": word_diff(o, n)})
        out += [{"soort": "weg", "oud": olds[k]} for k in range(done_old, len(olds))]
    return out


def step_status(h: History, s: dict) -> str:
    """In a law through time the status is about the Staatsblad check; the text is the BWB's either way."""
    if h.chain:
        return "Staatsblad nagespeeld" if s["status"] == "ok" else "Staatsblad niet nagespeeld; de tekst is die van wetten.overheid.nl"
    return STATUS_NAMES.get(s["status"], s["status"])


def cmd_vergelijk(a) -> dict:
    """What changed between two moments, per article, with the documents that changed it.

    Articles are matched by identity, so a renumbered article is one change, not a removal and an addition."""
    h = History(load(a.id)[0])
    p_from, _ = h.resolve(a.van)
    p_to, cut = h.resolve(a.tot) if a.tot else (len(h.positions) - 1, "9999-12-31")
    s_from, s_to = h.positions[p_from]["steps"], h.positions[p_to]["steps"]
    if s_from > s_to:
        raise Fault(f"--van ({h.describe(p_from)}) ligt na --tot ({h.describe(p_to)})")

    def articles(steps):
        blocks, order = h.state(steps)
        return {h.identity(k): blocks[k] for k in order if blocks.get(k) and blocks[k]["soort"] == "artikel"}

    old, new = articles(s_from), articles(s_to)
    order = list(new) + [i for i in old if i not in new]
    if a.nr:
        nr = article_nr(a.nr)
        hit = [i for i in order if (new.get(i) or old.get(i))["nr"] == nr]
        if not hit:
            raise Fault(f"artikel {nr} bestaat op geen van beide momenten")
        order = hit[:1]
    changes = []
    for ident in order:
        before, after = old.get(ident), new.get(ident)
        lines_old = [t for t, *_ in before["regels"]] if before else []
        lines_new = [t for t, *_ in after["regels"]] if after else []
        diff = line_diff(lines_old, lines_new)
        renumbered = bool(before and after and before["nr"] != after["nr"])
        if not diff and not renumbered:
            continue
        steps = [i for i in h.steps_touching(ident) if s_from < i <= s_to]
        changes.append({
            "artikel": (after or before)["nr"], "was": before["nr"] if renumbered else None,
            "wat": "nieuw" if not before else "vervallen" if not after else "gewijzigd",
            "door": [{**h.cite(h.steps[i - 1]["stuk"], *(() if h.chain else (h.steps[i - 1].get("onderdeel"), h.steps[i - 1].get("sub")))),
                      "status": step_status(h, h.steps[i - 1])} for i in steps],
            "controle": h.check(after["nr"], lines_new, cut)["oordeel"] if after else None,
            # Whether the article belongs to this law at all is a question for the last official text,
            # whatever the moment: the replay can read an article of an amended law as one of its own.
            "in_ijkpunt": h.known_article((after or before)["nr"]),
            "regels": diff})
    return {"dossier": h.d["dossier"], "van": h.describe(p_from), "tot": h.describe(p_to), "artikel": a.nr and article_nr(a.nr),
            "wijzigingen": changes, "let_op": h.notes(article_nr(a.nr) if a.nr else None, cut, membership=False)}


def cmd_haal(a) -> dict:
    if not WORKER.is_file():
        raise Fault(f"de afspeelketen ontbreekt ({WORKER.relative_to(ROOT)})")
    target = a.dossier or a.id
    if not valid_id(a.id) or (a.dossier and not valid_id(a.dossier)):
        raise Fault("geef een BWB-id, een dossiernummer, of beide")
    base = cache_dir()
    chain = a.keten or a.opbouw
    if chain and not a.id.startswith("BWBR"):
        raise Fault("--keten en --opbouw gaan over een wet door de tijd: geef haar BWB-id")
    if chain and a.dossier:
        raise Fault("--keten en --opbouw nemen geen dossiernummer; haal het dossier apart op")
    if a.keten and a.opbouw:
        raise Fault("kies --keten (ophalen) of --opbouw (opnieuw opbouwen uit wat er is), niet beide")
    if a.alles and not a.keten:
        raise Fault("--alles hoort bij --keten")
    out = base / "gegevens" / f"{a.id if chain else target}.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, "WETSGESCHIEDENIS_BRON": str(base / "bron"), "WETSGESCHIEDENIS_WERK": str(base / "werk")}
    timelines = os.pathsep.join(str(d) for d in data_dirs())
    if a.opbouw:
        # Rebuild from what is already fetched: hangs newly fetched dossiers' explanations on the transitions.
        cmd = [sys.executable, str(WORKER), "opbouw", a.id, "--uit", str(out), "--gegevens", timelines]
    elif a.keten:
        cmd = [sys.executable, str(WORKER), "keten", a.id, "--uit", str(out),
               "--gegevens", timelines] + (["--alles"] if a.alles else [])
    else:
        cmd = [sys.executable, str(WORKER), "dossier", *([a.id, a.dossier] if a.dossier else [a.id]), "--uit", str(out)]
    if a.opbouw and not (base / "bron" / "regelingen" / a.id / "keten.json").is_file():
        raise Fault(f"er is nog geen keten van {a.id} opgehaald; eerst: wetsgeschiedenis.py haal {a.id} --keten")
    limit = int(os.environ.get("WETSGESCHIEDENIS_TIMEOUT") or 1800)
    try:
        # curl in the chain only bounds the connect, not a stalled transfer.
        r = subprocess.run(cmd, env=env, text=True, capture_output=True, timeout=limit)
    except subprocess.TimeoutExpired:
        raise Fault(f"ophalen duurde langer dan {limit} seconden en is afgebroken (WETSGESCHIEDENIS_TIMEOUT)") from None
    errors = []
    for line in r.stderr.splitlines():
        if line.startswith("@fout "):
            raw = line[len("@fout "):]
            try:
                errors.append(json.loads(raw).get("tekst") or raw)
            except ValueError:
                errors.append(raw)
    if r.returncode != 0:
        reason = "; ".join(errors) or r.stderr.strip()[-600:] or f"afsluitcode {r.returncode}"
        # Some failures surface as a bare exception name; the worker's own log says where it stopped.
        log = [l for l in r.stdout.splitlines() if l.strip()][-6:]
        if any(re.search(r"\b0 ondernummers\b", l) for l in r.stdout.splitlines()):
            reason += (f" — het Gegevensmagazijn van de Tweede Kamer gaf geen stukken voor dossier {target}: "
                       "een verkeerd nummer, of een dossier dat het Gegevensmagazijn niet kent")
        raise Fault(f"ophalen mislukt: {reason}" + ("\nlaatste regels van de keten:\n  " + "\n  ".join(log) if log else ""))
    h = History(json.loads(out.read_text(encoding="utf-8")))
    return {"opgeslagen": str(out), "dossier": h.d["dossier"], "naam": h.d["naam"], "kwaliteit": quality(h),
            "log": r.stdout.strip().splitlines()[-15:]}


# ---------------------------------------------------------------- rendering
def render(cmd: str, x: dict) -> str:
    out: list[str] = []
    w = out.append
    if cmd == "lijst":
        for r in x["tijdlijnen"]:
            w(f"{r['id']:<12} {r['soort']:<14} {r['naam']}  ({r['bron']})")
    elif cmd == "stukken":
        w(f"# {x['naam']} ({x['dossier']}, {x['soort']})")
        w(render_quality(x["kwaliteit"]))
        for s in x["stukken"]:
            mee = "" if s["speelt_mee"] is None else (" [speelt mee]" if s["speelt_mee"] else "")
            extra = f" — {s['uitslag']}" if s.get("uitslag") else ""
            w(f"- {s['datum']}  {s['verwijzing']}: {s['titel']}{mee}{extra}" + (f"  <{s['url']}>" if s["url"] else ""))
    elif cmd == "artikel":
        w(head(x))
        w(f"Controle: {x['controle']['uitleg']}")
        notes(w, x)
        for r in x["regels"]:
            # The chain is ordered; the last link wrote the wording as it stands.
            chain = [c["verwijzing"] for c in r["herkomst"]]
            src = (" → ".join(chain) + (" (huidige formulering)" if len(chain) > 1 else "")) if chain else "herkomst onbekend"
            w(f"\n{r['tekst']}\n  ↳ {src}")
        if x["controle"].get("officieel"):
            w("\nOfficiële tekst:\n" + "\n".join(x["controle"]["officieel"]))
    elif cmd == "herkomst":
        w(head(x))
        w(f"Controle van de laatste stand: {x['controle']['uitleg']}")
        notes(w, x)
        w("\n## Handelingen die dit artikel raakten")
        if not x["handelingen"]:
            w("Geen: de tekst staat er zo sinds het begin.")
        for s in x["handelingen"]:
            if s["bij"] == "inwerkingtreding":
                dossiers = ", ".join(f"{d['nr']} ({d['status']})" for d in s["dossiers"]) or "geen bekend"
                missing = [d["nr"] for d in s["dossiers"] if d.get("status") != "opgehaald"]
                w(f"- {s['werkt_door_op']}  {s['verwijzing']} — {s['status']}{': ' + s['reden'] if s['reden'] else ''}\n"
                  f"  Kamerdossier: {dossiers}."
                  + "".join(f"\n  Toelichting ophalen: wetsgeschiedenis.py haal {x['dossier']} {nr}, "
                            f"daarna wetsgeschiedenis.py haal {x['dossier']} --opbouw" for nr in missing))
                continue
            when = f"ingediend {s['datum']}" + (f", werkt door bij de stemming van {s['werkt_door_op']}" if s["bij"] == "stemming" else "")
            w(f"- {s['verwijzing']} ({when}; {s['status']}){': ' + s['reden'] if s['reden'] else ''}\n  {s['opdracht']}")
        w("\n## Versies")
        for v in x["versies"]:
            what = "; ".join(c["verwijzing"] for c in v["door"]) or "bron onbekend"
            if v["moment"] == "stemming":
                what += " (bij de stemming)"
            w(f"- {v['datum']}  {what}" + (f" (artikel {v['nummer']})" if v["nummer"] and v["nummer"] != x["artikel"] else "")
              + f"\n  {v['tekst'][:400]}{'…' if len(v['tekst']) > 400 else ''}")
    elif cmd == "toelichting":
        w(head(x))
        notes(w, x)
        if not x["toelichting"]:
            w("Geen toelichting gekoppeld aan dit artikel.")
        for t in x["toelichting"]:
            w(f"\n## {t['datum']}  {t['verwijzing']} — {t['soort']} ({t['over']})")
            if t["url"]:
                w(f"<{t['url']}>")
            if t.get("kop"):
                w(f"_{t['kop']}{' › ' + t['titel'] if t.get('titel') else ''}_")
            w("\n".join(t["tekst"]))
        if x["niet_gekoppeld"]:
            w("\n## Niet gekoppeld, maar noemt dit artikel")
            for t in x["niet_gekoppeld"]:
                w(f"- {t['verwijzing']} ({t['reden']}): {t['tekst'][:500]}")
    elif cmd == "wet":
        c = x["controle"]
        w(f"# {x['naam']} ({x['dossier']}) — {x['omschrijving']}")
        if c["bwb"]:
            w("Dit is de tekst van wetten.overheid.nl zoals die toen gold; zij is overgenomen, niet afgespeeld.")
        elif c["ijkpunt"]:
            w(f"Controle tegen {c['ijkpunt']}: {c['artikelen'] - len(c['wijkt_af']) - len(c['onbekend'])} van de "
              f"{c['artikelen']} artikelen gelijk."
              + (f" Wijkt af (citeer de officiële tekst, zie 'artikel'): {', '.join(c['wijkt_af'])}." if c["wijkt_af"] else "")
              + (f" Niet in het ijkpunt: {', '.join(c['onbekend'])}." if c["onbekend"] else ""))
        else:
            w("Controle: er is op dit moment nog geen officiële tekst om mee te vergelijken.")
        notes(w, x)
        for b in x["blokken"]:
            w("")
            for r in b["regels"]:
                w(r["tekst"] + (f"  ⟨{' → '.join(r['herkomst'])}⟩" if show_origin and r["herkomst"] else ""))
            if b.get("controle") == "wijkt af":
                w("  ⚠ wijkt af van het ijkpunt")
    elif cmd == "vergelijk":
        w(f"# Dossier {x['dossier']}" + (f", artikel {x['artikel']}" if x["artikel"] else "")
          + f": van {x['van']} tot {x['tot']}")
        notes(w, x)
        if not x["wijzigingen"]:
            w("Geen verschil.")
        for c in x["wijzigingen"]:
            title = f"Artikel {c['artikel']}" + (f" (was {c['was']})" if c["was"] else "")
            w(f"\n## {title} — {c['wat']}")
            for d in c["door"]:
                w(f"  door {d['verwijzing']} ({d['status']})")
            if not c["door"]:
                w("  door: geen handeling gevonden die dit artikel noemt")
            if c["controle"] == "wijkt af":
                w("  ⚠ de nieuwe tekst wijkt af van het ijkpunt: dit verschil is uit de afspeling, niet uit de officiële tekst")
            if c["in_ijkpunt"] is False and c["wat"] != "vervallen":
                w("  ⚠ dit artikel staat niet in de officiële eindtekst: mogelijk geen artikel van deze wet, maar een "
                  "artikel van een wet die zij wijzigt, dat de afspeling als eigen artikel las")
            for r in c["regels"]:
                if r["soort"] == "vernummerd":
                    old_label, new_label = LABEL.match(norm(r["oud"])), LABEL.match(norm(r["nieuw"]))
                    w(f"  ↻ {old_label.group(1) if old_label else '?'} wordt {new_label.group(1) if new_label else '?'}, "
                      f"verder gelijk: {unlabelled(r['nieuw'])[:90]}{'…' if len(unlabelled(r['nieuw'])) > 90 else ''}")
                elif r["soort"] == "gewijzigd":
                    w(f"  ~ {r['verschil']}")
                elif r["soort"] == "weg":
                    w(f"  - {r['oud']}")
                else:
                    w(f"  + {r['nieuw']}")
    elif cmd == "haal":
        w(f"{x['naam']} ({x['dossier']}) opgeslagen in {x['opgeslagen']}")
        w(render_quality(x["kwaliteit"]))
    return "\n".join(out)


def notes(w, x: dict) -> None:
    for n in x.get("let_op") or []:
        w(f"Let op: {n}")


def head(x: dict) -> str:
    what = f"artikel {x['artikel']}" + (f", lid {x['lid']}" if x.get("lid") else "")
    if x.get("doelwet"):
        what = f"artikel {x['artikel']} van de gewijzigde wet"
    when = x.get("omschrijving") or (f"stand op {x['moment']}" if x.get("moment") else "")
    return f"# Dossier {x['dossier']}, {what}" + (f" — {when}" if when else "")


def render_quality(q: dict) -> str:
    hs = ", ".join(f"{v} {k}" for k, v in q["handelingen"].items())
    ij = "; ".join(f"{p['naam']}: {p['gelijk']} artikelen gelijk, {p['verschillend']} verschillend" for p in q["ijkpunten"])
    return f"Afspeling: {hs}." + (f" Controle: {ij}." if ij else " Geen ijkpunt.")


def parser() -> argparse.ArgumentParser:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--json", action="store_true", help="antwoord als JSON")
    sub = ap.add_subparsers(dest="cmd", required=True)
    sub.add_parser("lijst")
    s = sub.add_parser("stukken")
    s.add_argument("id")
    for name in ("artikel", "herkomst", "toelichting", "wet"):
        s = sub.add_parser(name)
        s.add_argument("id")
        if name != "wet":
            s.add_argument("nr", nargs="?" if name == "toelichting" else None)
            s.add_argument("--lid")
        s.add_argument("--op", help="datum (JJJJ-MM-DD): de stand op die dag")
        s.add_argument("--na", help="stuk (kst-…): de stand direct na dat stuk")
        s.add_argument("--stap", type=int, help="handeling N: de stand direct na die handeling (0 is het begin)")
        if name == "toelichting":
            s.add_argument("--doelwet", help="artikel van de wet die het voorstel wijzigt")
        if name == "wet":
            s.add_argument("--herkomst", action="store_true", help="per regel de keten van stukken erbij")
    s = sub.add_parser("vergelijk")
    s.add_argument("id")
    s.add_argument("nr", nargs="?", help="één artikel; zonder: de hele wet")
    s.add_argument("--van", required=True, help="moment: datum, stuk (kst-…/versie-…) of stap:N")
    s.add_argument("--tot", help="moment als bij --van; zonder: de laatste stand")
    s = sub.add_parser("haal")
    s.add_argument("id", help="BWB-id of dossiernummer")
    s.add_argument("dossier", nargs="?", help="dossiernummer bij een BWB-id")
    s.add_argument("--keten", action="store_true", help="de wet door de tijd in plaats van één dossier")
    s.add_argument("--alles", action="store_true", help="met --keten: alle versies, niet alleen de jongste")
    s.add_argument("--opbouw", action="store_true",
                   help="de keten opnieuw opbouwen uit wat er al is, zonder netwerk (na het ophalen van dossiers)")
    return ap


def main(argv: list[str]) -> int:
    a = parser().parse_args(argv)
    global show_origin
    show_origin = bool(getattr(a, "herkomst", False))
    if a.cmd in ("artikel", "herkomst", "toelichting", "wet") and a.op and not re.fullmatch(r"\d{4}-\d{2}-\d{2}", a.op):
        print("fout: --op wil een datum als JJJJ-MM-DD", file=sys.stderr)
        return 2
    try:
        x = globals()[f"cmd_{a.cmd}"](a)
    except Fault as e:
        print(f"fout: {e}", file=sys.stderr)
        return 1
    print(json.dumps(x, ensure_ascii=False, indent=2) if a.json else render(a.cmd, x))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
