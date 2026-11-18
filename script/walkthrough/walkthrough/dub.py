"""The walkthrough spoken in another language, in the presenter's cloned voice.

The clock of a track stays the Dutch one: every click, scroll, pause and
chapter keeps its time. What changes is the voice. Each sentence the
presenter said gets a time slot, from where it starts to where the next one
starts (or a pause, or the end of the track), and its translation is spoken
inside that slot:

- shorter than the slot: it starts where the Dutch started, silence after;
- longer: the voice is generated again, a little faster (`SPEED_MAX`);
- still longer: stretched in time on top of that (`TEMPO_MAX`);
- still longer: the translation is shortened, once, by a language model;
- and if even that does not fit, squeezed up to `TEMPO_LIMIT` and reported,
  so a person shortens the sentence by hand.

A sentence never crosses a pause: the player stops there, and an English
voice that carries on would talk into the viewer's own clicking. Chapter
boundaries are not hard: the presenter often talks over a slide change, and
half a sentence is not something to translate.

The sentences with their times are written by `build` into
corpus/demo/walkthrough/sentences.json, so a dub needs no raw takes: only
that file, the translations in corpus/demo/i18n/walkthrough.<code>.yaml and
the voice.

Pure functions on plain data; generation and rendering are in cli.py.
"""

from __future__ import annotations

import array
import hashlib
import json
import re
import wave
from dataclasses import dataclass
from pathlib import Path

from .timeline import norm_cue
from .timeline import sentences as split_sentences

SPEED_MAX = 1.15  # the voice's own speed setting (ElevenLabs goes to 1.2)
TEMPO_MAX = 1.10  # time-stretch on top of that, still sounds natural
TEMPO_LIMIT = 1.25  # the most a sentence that does not fit gets squeezed
WORDS_PER_SECOND = 2.5  # the budget the translator gets, per second of slot
RATE = 48000


def sentences(words: list[dict], breaks: list[float], duration: float) -> list[dict]:
    """The track's sentences with their slots: `start` and `end` of what was
    said, `until` where the slot ends.

    `words` are the track's words on its own clock, `breaks` the moments a
    sentence may not cross (pauses, seams between recorded and generated
    parts). A sentence that does cross one is split there.
    """
    breaks = sorted(b for b in breaks if 0 < b < duration)
    pieces: list[list[dict]] = []
    for s in split_sentences(words):
        cur: list[dict] = []
        for w in s:
            if cur and any(cur[-1]["end"] <= b <= w["start"] or cur[0]["start"] < b <= w["start"] for b in breaks):
                pieces.append(cur)
                cur = []
            cur.append(w)
        if cur:
            pieces.append(cur)
    out = []
    for i, p in enumerate(pieces):
        start, end = p[0]["start"], p[-1]["end"]
        nxt = pieces[i + 1][0]["start"] if i + 1 < len(pieces) else duration
        # A break inside the last word does not split the sentence; it still
        # ends the slot.
        hard = next((b for b in breaks if b > start), duration)
        until = max(end, min(nxt, hard, duration))
        out.append({"start": round(start, 3), "end": round(end, 3), "until": round(until, 3), "nl": norm_cue(" ".join(w["word"] for w in p))})
    return out


def basis(track: dict) -> str:
    """A fingerprint of what a dub was made against: the Dutch sentences and
    their times. A build that changes it makes the dub stale."""
    blob = json.dumps({"duration": track["duration"], "sentences": track["sentences"]}, sort_keys=True, ensure_ascii=False)
    return hashlib.sha256(blob.encode()).hexdigest()[:16]


def budget(sentence: dict, words_per_second: float = WORDS_PER_SECOND) -> int:
    """How many words a translation of the sentence may have. A slower
    voice (eleven_v4) gets fewer words per second."""
    return max(2, int((sentence["until"] - sentence["start"]) * words_per_second))


def echoes(answer: str, source: str) -> bool:
    """Whether `answer` is mostly the words of the Dutch `source`: a model
    asked to shorten a translation sometimes answers in Dutch ("Dat is onze
    interpretatie."), even with no Dutch in the question."""
    bare = lambda s: [w.strip(".,;:!?'\"()").lower() for w in s.split()]
    words, dutch = bare(answer), set(bare(source))
    return bool(words) and sum(w in dutch for w in words) / len(words) > 0.6


@dataclass(frozen=True)
class Fit:
    """What to do with a generated line of `duration` seconds in its slot."""

    action: str  # place | speed | shorten | squeeze
    speed: float = 1.0  # for `speed`: generate again at this speed
    tempo: float = 1.0  # for `place` and `squeeze`: stretch by this factor


def fit(duration: float, slot: float, speed: float = 1.0, shortened: bool = False, can_speed: bool = True, tempo_max: float = TEMPO_MAX) -> Fit:
    """The next step for a line that came out `duration` long at `speed`.
    `can_speed`: the voice follows a speed setting (eleven_v4 does not).
    `tempo_max`: how far to stretch before shortening."""
    if duration <= slot:
        return Fit("place")
    need = duration / slot
    if can_speed and speed < SPEED_MAX - 1e-6:
        # A few percent extra: the voice does not keep exactly to its speed.
        return Fit("speed", speed=round(min(SPEED_MAX, speed * need * 1.03), 3))
    if need <= tempo_max:
        return Fit("place", tempo=round(need, 4))
    if not shortened:
        return Fit("shorten")
    return Fit("squeeze", tempo=round(min(need, max(TEMPO_LIMIT, tempo_max)), 4))


def aligned_words(text: str, alignment: dict, at: float, tempo: float = 1.0) -> list[dict]:
    """The words of a generated line on the track's clock, from the time per
    character the voice returned."""
    starts = alignment["character_start_times_seconds"]
    ends = alignment.get("character_end_times_seconds") or starts
    out = []
    for m in re.finditer(r"\S+", text):
        a, b = m.start(), min(m.end(), len(starts)) - 1
        if a >= len(starts):
            break
        out.append({"word": m.group(), "start": round(at + float(starts[a]) / tempo, 3), "end": round(at + float(ends[b]) / tempo, 3)})
    return out


def read_pcm(path: Path) -> array.array:
    with wave.open(str(path)) as w:
        if (w.getnchannels(), w.getsampwidth(), w.getframerate()) != (1, 2, RATE):
            raise ValueError(f"{path}: verwacht 48 kHz mono 16 bit")
        samples = array.array("h")
        samples.frombytes(w.readframes(w.getnframes()))
    return samples


def assemble(clips: list[tuple[array.array, float]], total: float) -> array.array:
    """The clips on silence at their times, exactly `total` seconds long.

    Where a clip runs into the next one the two are mixed, not cut: a
    clipped word is worse than a moment of two voices. The caller reports a
    clip longer than its slot.
    """
    n = round(total * RATE)
    out = array.array("h", bytes(2 * n))
    filled = 0
    for samples, at in sorted(clips, key=lambda c: c[1]):
        i = round(at * RATE)
        if i >= n:
            continue
        part = samples[: n - i]
        mixed = max(0, min(filled - i, len(part)))
        for k in range(mixed):
            out[i + k] = max(-32768, min(32767, out[i + k] + part[k]))
        out[i + mixed : i + len(part)] = part[mixed:]
        filled = max(filled, i + len(part))
    return out


def write_pcm(samples: array.array, path: Path) -> None:
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(samples.tobytes())
