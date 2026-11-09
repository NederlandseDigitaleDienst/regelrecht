"""Speech from text, with the time of every character.

Two providers:

- `elevenlabs`: the presenter's cloned voice. Needs `ELEVENLABS_API_KEY` in
  the environment or in `.walkthrough/.env` (never in git), and the voice id
  in walkthrough.yaml. The API returns the audio with a start time per
  character, which is what puts an action on its word.
- `say`: the Mac's own Dutch voice, to try a script without an account. It
  reports no timing, so the characters are spread evenly over the line.

Generated lines are cached under `.walkthrough/voice/` by a hash of
everything that shapes them, so a rebuild only pays for lines that changed.
"""

from __future__ import annotations

import base64
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import urllib.error
import urllib.request
from pathlib import Path

from . import media
from .script import even_alignment

# MP3, not PCM: 44.1 kHz PCM is for the Pro tier and above, and at 192 kbps
# the difference does not survive the AAC the player gets anyway.
API = "https://api.elevenlabs.io/v1/text-to-speech/{voice}/with-timestamps?output_format=mp3_44100_192"


def api_key(work: Path) -> str:
    key = os.environ.get("ELEVENLABS_API_KEY")
    env = work / ".env"
    if not key and env.exists():
        for line in env.read_text().splitlines():
            if line.startswith("ELEVENLABS_API_KEY="):
                key = line.split("=", 1)[1].strip().strip('"')
    if not key:
        raise SystemExit(
            "ElevenLabs-sleutel ontbreekt. Zet hem in .walkthrough/.env (staat niet in git):\n"
            "  read -s KEY && echo \"ELEVENLABS_API_KEY=$KEY\" >> .walkthrough/.env"
        )
    return key


def speakable(text: str, say_as: dict[str, str]) -> tuple[str, list[int]]:
    """The text as it is sent to the voice, with `say_as` replacements
    ("Awb" -> "A-W-B"), and for every original character where it landed.

    The captions and the markers work on the original text; the timing comes
    from the spoken one. The map connects the two.
    """
    out, where, i = "", [], 0
    keys = sorted(say_as, key=len, reverse=True)
    while i < len(text):
        hit = next((k for k in keys if text.startswith(k, i) and (i == 0 or not text[i - 1].isalnum()) and (i + len(k) == len(text) or not text[i + len(k)].isalnum())), None)
        if hit:
            start = len(out)
            out += say_as[hit]
            where += [start] * len(hit)
            i += len(hit)
        else:
            where.append(len(out))
            out += text[i]
            i += 1
    return out, where


def remap(alignment: dict, where: list[int]) -> dict:
    """An alignment over the spoken text, read back onto the original text."""
    starts = alignment["character_start_times_seconds"]
    ends = alignment.get("character_end_times_seconds") or starts
    n = len(starts)
    if not n:
        return alignment
    pick = lambda arr, i: float(arr[min(i, n - 1)])  # noqa: E731
    return {
        "character_start_times_seconds": [pick(starts, j) for j in where],
        "character_end_times_seconds": [pick(ends, j) for j in where],
    }


def _eleven(text: str, cfg: dict, work: Path, previous: str, following: str, out_wav: Path) -> dict:
    body = {
        "text": text,
        "model_id": cfg.get("model", "eleven_multilingual_v2"),
        "language_code": cfg.get("language", "nl"),
        "previous_text": previous or None,
        "next_text": following or None,
    }
    if cfg.get("settings"):
        body["voice_settings"] = cfg["settings"]
    req = urllib.request.Request(
        API.format(voice=cfg["voice_id"]),
        data=json.dumps({k: v for k, v in body.items() if v is not None}).encode(),
        headers={"xi-api-key": api_key(work), "content-type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=120) as res:
            data = json.loads(res.read())
    except urllib.error.HTTPError as e:
        raise SystemExit(f"ElevenLabs gaf {e.code}: {e.read().decode(errors='replace')[:400]}")
    with tempfile.NamedTemporaryFile(suffix=".mp3", delete=False) as f:
        f.write(base64.b64decode(data["audio_base64"]))
        raw = Path(f.name)
    media.ffmpeg("-i", raw, "-ar", "48000", "-ac", "1", out_wav)
    raw.unlink()
    return data.get("alignment") or data.get("normalized_alignment")


def _say(text: str, cfg: dict, out_wav: Path) -> dict:
    if not shutil.which("say"):
        raise SystemExit("`say` ontbreekt (alleen op een Mac); gebruik provider: elevenlabs")
    with tempfile.TemporaryDirectory() as tmp:
        aiff = Path(tmp) / "line.aiff"
        # `say` speaks about 180 words a minute; a speed setting scales that.
        speed = (cfg.get("settings") or {}).get("speed")
        rate = ["-r", str(round(180 * float(speed)))] if speed else []
        subprocess.run(["say", "-v", cfg.get("say_voice", "Xander"), *rate, "-o", str(aiff), text], check=True)
        media.ffmpeg("-i", aiff, "-ar", "48000", "-ac", "1", out_wav)
    return even_alignment(text, media.duration(out_wav))


def speak(text: str, cfg: dict, work: Path, previous: str = "", following: str = "") -> dict:
    """Generate (or fetch from the cache) one line. Returns
    `{wav, duration, alignment}`, the alignment over `text` as written."""
    provider = cfg.get("provider", "say")
    spoken, where = speakable(text, cfg.get("say_as") or {})
    key = json.dumps([provider, cfg.get("voice_id"), cfg.get("model"), cfg.get("settings"), cfg.get("say_voice"), spoken, previous, following], sort_keys=True)
    digest = hashlib.sha256(key.encode()).hexdigest()[:20]
    cache = work / "voice"
    cache.mkdir(parents=True, exist_ok=True)
    wav, meta = cache / f"{digest}.wav", cache / f"{digest}.json"
    if not (wav.exists() and meta.exists()):
        if provider == "elevenlabs":
            alignment = _eleven(spoken, cfg, work, previous, following, wav)
        elif provider == "say":
            alignment = _say(spoken, cfg, wav)
        else:
            raise SystemExit(f"onbekende stem-provider: {provider}")
        meta.write_text(json.dumps(alignment))
    alignment = remap(json.loads(meta.read_text()), where)
    return {"wav": wav, "duration": media.duration(wav), "alignment": alignment}
