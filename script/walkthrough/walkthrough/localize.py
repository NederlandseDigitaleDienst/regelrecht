"""The walkthrough in a language other than the one it was recorded in.

The voice stays Dutch. What a viewer reads is translated: the captions, the
slides, the questions, and what the presenter typed into a form. The demo
itself runs in the viewer's language too; the replay finds what the presenter
clicked through the demo's own translations (frontend-demo/src/walkthrough/
translateText.ts), so nothing about the clicks is stored here.

The translations live in `corpus/demo/i18n/walkthrough.<code>.yaml`, next to
the deck's own overlay, for anyone to correct by hand:

    terms:                # fixed translations the model is told to use
      regelwerk: rulework (plural ruleworks)
    questions:            # per question id
      open-normen: What happens to open norms?
    slides:               # slide text the deck overlay does not cover
      - nl: Per artikel de wettekst, en daaronder hoe hij werkt.
        en: Each article's text, and below it how it works.
    captions:             # per caption as shown in Dutch
      - nl: Dat is nog steeds zo.
        en: That is still the case.
    typed:                # sentences typed into a form
      - nl: Dit klopt niet
        en: This is not correct

Keyed by the Dutch text, not by position: a recut or a rebuild keeps every
line that still occurs. `walkthrough translate <code>` asks a language model
for what is missing; a build applies the file.
"""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

import yaml

from .timeline import norm_cue, to_vtt, wrap

ROOT = Path(__file__).resolve().parents[3]
I18N = ROOT / "corpus" / "demo" / "i18n"
DECK = ROOT / "corpus" / "demo" / "demo-config.yaml"
SOURCE = "nl"

# The fields of a slide that carry text, as in PresentationDeck.vue.
SLIDE_TEXT = ("title", "subtitle", "footer", "overline", "lead", "note")
SLIDE_LISTS = ("lines", "bullets")


def path_for(code: str) -> Path:
    return I18N / f"walkthrough.{code}.yaml"


def locales() -> list[str]:
    """The languages the walkthrough has a translation for."""
    return sorted(p.name.split(".")[1] for p in I18N.glob("walkthrough.*.yaml"))


def load(code: str) -> dict:
    data = (yaml.safe_load(path_for(code).read_text()) if path_for(code).exists() else None) or {}
    # An empty translation is one: a fragment ("gaan.") whose words the
    # translation of the line before already carries. Its caption goes.
    pairs = lambda key: {norm_cue(e["nl"]): e[code] or "" for e in data.get(key) or [] if e.get("nl") and code in e}  # noqa: E731
    return {
        "terms": data.get("terms") or {},
        "questions": data.get("questions") or {},
        "slides": pairs("slides"),
        "captions": pairs("captions"),
        "typed": pairs("typed"),
    }


def deck_pairs(code: str, prefix: str | None = "slides.") -> dict[str, str]:
    """Dutch text -> its translation, from the demo's own overlay
    (`corpus/demo/i18n/<code>.yaml`, keyed by path into demo-config.yaml).
    The walkthrough's slides are mostly the deck's; what the deck already
    translates needs no second translation. Without `prefix`, every path:
    the names the demo shows (a portal tile, a chart's label)."""
    overlay = I18N / f"{code}.yaml"
    if not overlay.exists():
        return {}
    deck = yaml.safe_load(DECK.read_text()) or {}
    out = {}
    for path, value in (yaml.safe_load(overlay.read_text()) or {}).items():
        if (prefix and not path.startswith(prefix)) or not isinstance(value, str):
            continue
        node = deck
        for part in path.split("."):
            node = node[int(part)] if isinstance(node, list) and part.isdigit() and int(part) < len(node) else (node.get(part) if isinstance(node, dict) else None)
            if node is None:
                break
        if isinstance(node, str):
            out[norm_cue(node)] = value
    return out


def slide_strings(slide: dict) -> list[str]:
    out = [slide[k] for k in SLIDE_TEXT if isinstance(slide.get(k), str) and slide[k].strip()]
    for k in SLIDE_LISTS:
        out += [x for x in slide.get(k) or [] if isinstance(x, str) and x.strip()]
    return out


def localize_slide(slide: dict, table: dict[str, str]) -> tuple[dict, list[str]]:
    """The slide with every text that has a translation translated, and the
    texts that have none (those stay Dutch)."""
    missing = []

    def tr(s):
        hit = table.get(norm_cue(s))
        if hit is None:
            missing.append(s)
            return s
        return hit

    out = dict(slide)
    for k in SLIDE_TEXT:
        if isinstance(slide.get(k), str) and slide[k].strip():
            out[k] = tr(slide[k])
    for k in SLIDE_LISTS:
        if isinstance(slide.get(k), list):
            out[k] = [tr(x) if isinstance(x, str) and x.strip() else x for x in slide[k]]
    return out, missing


def parse_vtt(text: str) -> list[dict]:
    """The cues of a WebVTT file as `to_vtt` writes it."""
    cues = []
    for block in re.split(r"\n\s*\n", text.strip()):
        lines = block.splitlines()
        timing = next((i for i, line in enumerate(lines) if "-->" in line), None)
        if timing is None:
            continue
        a, b = (x.strip().split()[0] for x in lines[timing].split("-->"))
        cues.append({"start": vtt_seconds(a), "end": vtt_seconds(b), "text": "\n".join(lines[timing + 1 :])})
    return cues


def vtt_seconds(s: str) -> float:
    h, m, rest = s.split(":") if s.count(":") == 2 else ("0", *s.split(":"))
    return int(h) * 3600 + int(m) * 60 + float(rest)


def track_name(vtt_file: str) -> str:
    return re.sub(r"-[0-9a-f]{12}\.[a-z]+\.vtt$", "", vtt_file)


def caption_texts(track: dict, corpus: Path) -> list[str]:
    """The Dutch captions of a track, in order, as keys."""
    nl = (track.get("captions") or {}).get(SOURCE)
    if not nl or not (corpus / nl).exists():
        return []
    return [norm_cue(c["text"]) for c in parse_vtt((corpus / nl).read_text())]


def typed_runs(events: list[dict]) -> list[list[dict]]:
    """Typing into one field, keystroke by keystroke: the input events on the
    same element until another field is typed into."""
    runs: list[tuple[str, list[dict]]] = []
    for e in events:
        if e.get("type") != "input" or not isinstance(e.get("value"), str):
            continue
        # The field, not what is in it: a design-system field carries its
        # value on its host, so the description changes with every keystroke.
        key = json.dumps([{k: v for k, v in step.items() if k != "value"} for step in e.get("target") or []], sort_keys=True)
        if runs and runs[-1][0] == key:
            runs[-1][1].append(e)
        else:
            runs.append((key, [e]))
    return [run for _, run in runs]


def is_sentence(value: str) -> bool:
    """Free text worth translating: words, not a number or a search term. A
    search for a Dutch law name has to stay Dutch to find it."""
    return len(value.split()) >= 2 and not re.fullmatch(r"[\d\s.,€-]+", value)


def localize_typing(events: list[dict], code: str, typed: dict[str, str]) -> list[str]:
    """Give every keystroke of a translated sentence its share of the
    translation (in place, as `values`), so the field fills at the
    presenter's pace. Returns the sentences that have no translation."""
    missing = []
    for run in typed_runs(events):
        final = run[-1]["value"]
        if not is_sentence(final):
            continue
        local = typed.get(norm_cue(final))
        if local is None:
            missing.append(final)
            continue
        for e in run:
            v = e["value"]
            share = local if v == final else local[: round(len(v) / len(final) * len(local))]
            e.setdefault("values", {})[code] = share
    return missing


def write_captions(track: dict, code: str, table: dict[str, str], corpus: Path, write: bool = True) -> list[str]:
    """A VTT in `code` next to the Dutch one, cue for cue on the same timing.
    Returns the captions that have no translation (shown in Dutch). Without
    `write`, only that: nothing on disk or in the track changes."""
    nl = (track.get("captions") or {}).get(SOURCE)
    if not nl or not (corpus / nl).exists():
        return []
    missing, cues, translated = [], [], 0
    for c in parse_vtt((corpus / nl).read_text()):
        hit = table.get(norm_cue(c["text"]))
        if hit is None:
            missing.append(norm_cue(c["text"]))
            cues.append(c)
        elif hit.strip():
            cues.append({**c, "text": wrap(" ".join(hit.split()))})
            translated += 1
    if not write:
        return missing
    stem = track_name(nl)
    for old in corpus.glob(f"{stem}-*.{code}.vtt"):
        if re.fullmatch(rf"{re.escape(stem)}-[0-9a-f]{{12}}\.{code}\.vtt", old.name):
            old.unlink()
    # Nothing translated yet: no file, so the player keeps the Dutch and says so.
    if not translated:
        track["captions"].pop(code, None)
        return missing
    vtt = to_vtt(cues)
    name = f"{stem}-{hashlib.sha256(vtt.encode()).hexdigest()[:12]}.{code}.vtt"
    (corpus / name).write_text(vtt)
    track["captions"][code] = name
    return missing


def apply(timeline: dict, corpus: Path, write: bool = True) -> dict[str, dict[str, list[str]]]:
    """Put every translation there is into the timeline (in place) and next to
    it (the VTTs). Returns per language what is still missing. Without
    `write`, the VTTs on disk stay as they are."""
    report: dict[str, dict[str, list[str]]] = {}
    tracks = [timeline["main"], *(timeline.get("faq") or [])]
    for code in locales():
        tr = load(code)
        table = {**deck_pairs(code), **tr["slides"]}
        miss = {"captions": [], "slides": [], "questions": [], "typed": []}
        for track in tracks:
            local: dict = {}
            slides = []
            for s in track.get("slides") or []:
                out, m = localize_slide(s, table)
                slides.append(out)
                miss["slides"] += [x for x in m if x not in miss["slides"]]
            local["slides"] = slides
            if "question" in track:
                q = tr["questions"].get(track["id"])
                if q:
                    local["question"] = q
                else:
                    miss["questions"].append(track["question"])
            track.setdefault("i18n", {})[code] = local
            miss["captions"] += write_captions(track, code, tr["captions"], corpus, write)
            miss["typed"] += [x for x in localize_typing(track.get("events") or [], code, tr["typed"]) if x not in miss["typed"]]
        report[code] = miss
    # The languages the walkthrough plays in: the demo runs in the viewer's
    # language when it is one of these, and in Dutch otherwise.
    timeline["locales"] = [SOURCE, *(c for c in locales() if c != SOURCE)]
    return report
