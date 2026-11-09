"""The walkthrough pipeline, from a raw take to what the demo plays.

    walkthrough prepare <take>   ingest + clean + transcribe + correct + suggest
    walkthrough ingest <take>    remux, take the voice out, read the event log
    walkthrough clean <take>     denoise and tidy the voice (no loudness yet)
    walkthrough transcribe <take>  words with timestamps (WhisperX, Dutch)
    walkthrough correct <take>   a language model corrects what Whisper misheard
    walkthrough suggest <take>   draft cuts: long silences and flub marks
    walkthrough transcript <take|main>  the text per slide, to check the slides
    walkthrough build            walkthrough.yaml -> media, timeline.json, captions
    walkthrough export           a shareable MP4 per track
    walkthrough status           what is recorded and processed so far
    walkthrough script <take>    a draft script per slide, from what was said and done
    walkthrough voices           the voices on the ElevenLabs account (the clone's id)
    walkthrough check <take>     how a take sounds, in numbers, with a verdict per line
    walkthrough anchors          scroll anchors for takes recorded without them (dev server on)
    walkthrough states           the demo's state at chapters added in walkthrough.yaml (dev server on)
    walkthrough subtitles        shorter subtitles for what was said, in subtitles.yaml (then build)
    walkthrough translate <code> captions, slides and questions in another language (then build)
    walkthrough dub <code>       the walkthrough spoken in that language, in the cloned voice
    (verify: `just walkthrough verify` runs frontend-demo/scripts/verify-walkthrough.mjs)
    walkthrough publish <tag>    the media into a GitHub release (asks first)

Raw takes live in `.walkthrough/takes/<take>/` (not in git). What decides the
result is `corpus/demo/walkthrough/walkthrough.yaml`, in git: which takes,
which cuts, which slide texts, which questions. `build` is a pure function of
that file and the takes, so a correction is an edit there and a rebuild.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import yaml

from . import dub, localize, media
from .media import FPS, ffmpeg
from .timeline import (
    CutError,
    apply_subtitles,
    find_invitations,
    add_chapters,
    place_pauses,
    build_track,
    captions,
    chapters,
    check_cuts,
    norm_cue,
    protected_spans,
    remap_actions,
    remap_words,
    suggest_cuts,
    to_vtt,
)

ROOT = Path(__file__).resolve().parents[3]
WORK = ROOT / ".walkthrough"
TAKES = WORK / "takes"
CORPUS = ROOT / "corpus" / "demo" / "walkthrough"
CONFIG = CORPUS / "walkthrough.yaml"
# The Dutch sentences per track with their times: what a dub is made from.
SENTENCES = CORPUS / "sentences.json"
PUBLIC = ROOT / "frontend-demo" / "public" / "walkthrough"
HERE = Path(__file__).resolve().parent

DENOISE = ["--python", "3.11", "--with", "deepfilternet==0.5.6", "--with", "torch==2.0.1", "--with", "torchaudio==2.0.2", "--with", "numpy<2"]
WHISPERX = ["--python", "3.12", "--with", "whisperx"]
OPENCV = ["--with", "opencv-python-headless<5"]
PLAYWRIGHT = ["--with", "playwright"]

MAX_WIDTH = 2560
CAM_SIZE = 480


def say(msg: str) -> None:
    print(msg, file=sys.stderr)


def take_dir(take: str) -> Path:
    d = TAKES / take
    if not d.is_dir():
        raise SystemExit(f"opname {take} niet gevonden in {TAKES}")
    return d


def latest_take() -> str:
    takes = sorted(p.name for p in TAKES.glob("*") if p.is_dir())
    if not takes:
        raise SystemExit(f"nog geen opnames in {TAKES}; neem op met `just demo` en ?record")
    return takes[-1]


def read_json(path: Path, default=None):
    return json.loads(path.read_text()) if path.exists() else default


def uv_run(env: list[str], *args) -> subprocess.CompletedProcess:
    uv = shutil.which("uv") or "uv"
    cmd = [uv, "run", "--quiet", "--no-project", *env, *[str(a) for a in args]]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        raise SystemExit(f"mislukt: {' '.join(cmd[:8])} ...\n{proc.stderr[-2000:]}")
    return proc


# ---- per take ----------------------------------------------------------------


def ingest(take: str) -> dict:
    d = take_dir(take)
    events = read_json(d / "events.json", [])
    meta = read_json(d / "meta.json", {})
    if not (d / "app.webm").exists():
        raise SystemExit(f"{take}: app.webm ontbreekt")
    # MediaRecorder writes WebM without a duration or a seek index. A remux
    # into Matroska gives both, without touching the streams.
    ffmpeg("-i", d / "app.webm", "-c", "copy", d / "app.mkv")
    if (d / "cam.webm").exists():
        ffmpeg("-i", d / "cam.webm", "-c", "copy", d / "cam.mkv")
    # The voice, as 48 kHz mono, padded at the start if the audio began a
    # moment after the video, so take time is the same for both.
    ffmpeg("-i", d / "app.mkv", "-map", "0:a:0", "-af", "aresample=async=1:first_pts=0", "-ac", "1", "-ar", "48000", d / "mic.wav")
    # Tab capture has a variable frame rate: on a still screen Chrome sends
    # hardly any frames. Cutting such a file at a second would start the piece
    # at the next frame that happens to exist, seconds late on a still slide.
    # So the take becomes constant frame rate once, here, every held frame
    # repeated, and pieces are cut from that on the frame grid.
    w0, h0 = media.video_size(d / "app.mkv")
    scale = min(1.0, MAX_WIDTH / w0)
    w, h = media.even(w0 * scale), media.even(h0 * scale)
    cfr = ["-c:v", "libx264", "-preset", "veryfast", "-g", FPS]
    ffmpeg("-i", d / "app.mkv", "-an", "-vf", f"fps={FPS}:start_time=0,scale={w}:{h},setsar=1,format=yuv420p", *cfr, "-crf", "14", d / "app.cfr.mp4")
    if (d / "cam.mkv").exists():
        ffmpeg("-i", d / "cam.mkv", "-an", "-vf", f"fps={FPS}:start_time=0,format=yuv420p", *cfr, "-crf", "18", d / "cam.cfr.mp4")
    video_len = media.duration(d / "app.cfr.mp4")
    end = next((e["t"] / 1000 for e in reversed(events) if e.get("type") == "end"), video_len)
    info = {
        "take": take,
        "duration": round(min(video_len, end), 3),
        "width": w,
        "height": h,
        "camOffset": (meta.get("camOffsetMs") or 0) / 1000 if (d / "cam.mkv").exists() else None,
        "slides": sorted({e.get("index") for e in events if e.get("type") == "slide"}),
    }
    (d / "take.json").write_text(json.dumps(info, indent=1))
    say(f"{take}: {info['duration']:.1f}s, {w}x{h}, dia's {info['slides']}")
    return info


def clean(take: str, denoise: bool = True) -> None:
    d = take_dir(take)
    if not (d / "mic.wav").exists():
        ingest(take)
    src = d / "mic.wav"
    if denoise:
        say(f"{take}: ruis verwijderen (DeepFilterNet)")
        with tempfile.TemporaryDirectory() as tmp:
            uv_run(DENOISE, "deepFilter", src, "-o", tmp)
            out = next(Path(tmp).glob("*.wav"))
            shutil.copy(out, d / "denoised.wav")
        src = d / "denoised.wav"
    # Below 80 Hz is rumble, not voice. The de-esser takes the edge off sharp
    # s-sounds that a laptop or USB microphone exaggerates; the compressor
    # evens out a voice that leans in and out. Loudness comes later, over the
    # whole cut, so every chapter ends up equally loud.
    chain = "highpass=f=80,deesser=i=0.4,acompressor=threshold=-21dB:ratio=3:attack=5:release=120:makeup=2"
    if not denoise:
        chain = "highpass=f=80,afftdn=nf=-25," + chain.split(",", 1)[1]
    ffmpeg("-i", src, "-af", chain, "-ac", "1", "-ar", "48000", d / "voice.wav")
    say(f"{take}: voice.wav klaar")


def glossary() -> str:
    cfg = load_config(required=False)
    words = cfg.get("glossary") or []
    # A sentence, not a bare list: Whisper copies the prompt's style, and a
    # list without punctuation gave a transcript without full stops, which
    # leaves the captions nothing to break on.
    if not words:
        return "Dit is een rondleiding door de demo van RegelRecht."
    return f"Dit is een rondleiding door de demo van RegelRecht. Begrippen die erin voorkomen: {', '.join(words)}."


def transcribe(take: str, model: str = "large-v3") -> None:
    d = take_dir(take)
    if not (d / "voice.wav").exists():
        clean(take)
    say(f"{take}: transcriberen met WhisperX ({model}); dit duurt even")
    uv_run(WHISPERX, HERE / "whisperx_run.py", d / "voice.wav", d / "words.json", "--model", model, "--prompt", glossary())
    say(f"{take}: words.json klaar")


CORRECT_PROMPT = """Hieronder staat een automatisch transcript (Whisper) van een gesproken
rondleiding door de demo van RegelRecht: wetten als machine-uitvoerbare
regels. Verbeter wat de spraakherkenning verkeerd verstond: verkeerd
gespelde of verkeerd gehoorde woorden, vakbegrippen, namen en interpunctie.

Wat er tijdens het praten op het scherm stond, staat in de map frames:
één beeld per moment, de bestandsnaam is de tijd in seconden. Bekijk ze met
Read voordat je verbetert. Een woord dat de spreker voorleest of noemt staat
vaak letterlijk in beeld (een veldnaam, een wet, "bsn"), en dat beeld wint van
wat Whisper meende te horen. Het transcript met tijden staat erbij, zodat je
weet welk beeld bij welke zin hoort.

Regels:
- Verbeter wat er verkeerd verstaan is, niet hoe het gezegd is. Spreektaal,
  herhalingen, "eh" en "jada jada" blijven staan; herschrijf geen zinnen.
- Houd de woordvolgorde aan. Voeg geen inhoud toe en laat niets weg.
- Twijfel je, laat het woord dan staan.
- Antwoord met alleen de verbeterde tekst, zonder toelichting.

{context}
Transcript met tijden (alleen om de beelden bij te zoeken):
{timed}

Transcript om te verbeteren:
{text}
"""


def correct(take: str) -> Path | None:
    """Let a language model correct the transcript, as a person would.

    Whisper gets Dutch policy vocabulary wrong ("machine uit voorwaarde
    formaat"). The model gets the glossary, the slide texts, what was clicked
    and stills of the screen as context and
    returns the same speech with the mishearings fixed; `words_for` puts that
    text back onto Whisper's timing. Runs headless Claude Code (`claude -p`);
    without it the raw transcript stays, and the file can be edited by hand.
    """
    d = take_dir(take)
    words = read_json(d / "words.json", {}).get("words", [])
    if not words:
        raise SystemExit(f"{take}: nog geen transcript; draai eerst `just walkthrough transcribe {take}`")
    if not shutil.which("claude"):
        say(f"{take}: geen `claude` op het pad; het transcript blijft zoals Whisper het hoorde")
        return None
    slides = []
    for e in read_json(d / "events.json", []):
        sl = e.get("slide") or {}
        text = " / ".join(x for x in [sl.get("title"), *(sl.get("lines") or []), sl.get("body")] if isinstance(x, str) and x)
        if e.get("type") == "slide" and text and text not in slides:
            slides.append(text)
    clicked: list[str] = []
    terms = (load_config(required=False).get("glossary") or [])
    context = ""
    if terms:
        context += "Begrippen die erin voorkomen: " + ", ".join(terms) + ".\n"
    if slides:
        context += "Teksten op de dia's die erbij te zien waren:\n" + "\n".join(f"- {x}" for x in slides) + "\n"
    for e in read_json(d / "events.json", []):
        if e.get("type") == "click":
            label = " ".join(str(x.get("textContent") or x.get("accessibleLabel") or "") for x in e.get("target") or []).strip()
            if label:
                clicked.append(f"{e['t'] / 1000:.0f}s: {label}")
    if clicked:
        context += "Waar de spreker op klikte:\n" + "\n".join(f"- {x}" for x in clicked) + "\n"
    raw = " ".join(w["word"] for w in words)
    timed = _timed_text(words)
    frames = screen_frames(take)
    say(f"{take}: transcript nakijken met een taalmodel ({len(frames)} schermbeelden erbij)")
    r = subprocess.run(
        ["claude", "-p", "--model", "sonnet", "--tools", "Read", "--allowedTools", "Read"],
        input=CORRECT_PROMPT.format(context=context, timed=timed, text=raw),
        capture_output=True,
        text=True,
        cwd=d,
    )
    fixed = r.stdout.strip()
    if r.returncode != 0 or not fixed:
        say(f"{take}: nakijken lukte niet ({r.stderr.strip()[:200] or 'leeg antwoord'}); het ruwe transcript blijft")
        return None
    # A model that rewrote rather than corrected is caught here: the timing
    # only carries over when most words stayed.
    from difflib import SequenceMatcher

    same = SequenceMatcher(None, raw.lower().split(), fixed.lower().split(), autojunk=False).ratio()
    if same < 0.8:
        say(f"{take}: het taalmodel veranderde te veel ({same:.0%} gelijk); niet overgenomen")
        return None
    out = d / "corrected.txt"
    out.write_text(fixed + "\n")
    changed = [(a, b) for a, b in _changes(raw, fixed)]
    say(f"{take}: {len(changed)} verbeteringen in {out}")
    for a, b in changed:
        print(f"    {a}  ->  {b}")
    return out


def _timed_text(words: list[dict], every: float = 10.0) -> str:
    lines, line, mark = [], [], None
    for w in words:
        if mark is None or w["start"] - mark >= every:
            if line:
                lines.append(f"[{mark:.0f}s] " + " ".join(line))
            line, mark = [], w["start"]
        line.append(w["word"])
    if line:
        lines.append(f"[{mark:.0f}s] " + " ".join(line))
    return "\n".join(lines)


def screen_frames(take: str, every: float = 6.0, limit: int = 60) -> list[Path]:
    """Stills of the app at each route change, each click and every few
    seconds in between: what was on screen while the presenter spoke."""
    d = take_dir(take)
    video = d / "app.cfr.mp4"
    if not video.exists():
        return []
    out = d / "frames"
    if out.exists():
        return sorted(out.glob("*.jpg"))
    end = media.duration(video)
    times = [e["t"] / 1000 + 0.8 for e in read_json(d / "events.json", []) if e.get("type") in ("route", "click", "slide")]
    times += [k * every for k in range(int(end // every) + 1)]
    picked: list[float] = []
    for t in sorted(t for t in times if t < end):
        if not picked or t - picked[-1] >= 2.0:
            picked.append(t)
    if len(picked) > limit:
        picked = [picked[round(k * (len(picked) - 1) / (limit - 1))] for k in range(limit)]
    out.mkdir()
    for t in picked:
        ffmpeg("-ss", f"{t:.2f}", "-i", video, "-frames:v", "1", "-vf", "scale=1280:-2", "-q:v", "4", out / f"{t:06.1f}.jpg")
    return sorted(out.glob("*.jpg"))


def _changes(raw: str, fixed: str) -> list[tuple[str, str]]:
    from difflib import SequenceMatcher

    a, b = raw.split(), fixed.split()
    out = []
    for op, i1, i2, j1, j2 in SequenceMatcher(None, [x.lower().strip(".,;:!?") for x in a], [x.lower().strip(".,;:!?") for x in b], autojunk=False).get_opcodes():
        if op != "equal":
            out.append((" ".join(a[i1:i2]) or "(niets)", " ".join(b[j1:j2]) or "(weg)"))
    return out


def suggest(take: str) -> list[dict]:
    d = take_dir(take)
    words = read_json(d / "words.json", {}).get("words", [])
    events = read_json(d / "events.json", [])
    quiet = media.silences(d / "voice.wav") if (d / "voice.wav").exists() else None
    cuts = suggest_cuts(words, events, silences=quiet)
    text = yaml.safe_dump({"takes": {take: {"cuts": cuts}}}, allow_unicode=True, sort_keys=False)
    (d / "cuts.suggested.yaml").write_text(text)
    say(f"{take}: {len(cuts)} voorgestelde knippen in {d / 'cuts.suggested.yaml'}")
    return cuts


def cam_crop(take: str) -> dict | None:
    d = take_dir(take)
    if not (d / "cam.mkv").exists():
        return None
    cached = d / "cam_crop.json"
    if cached.exists():
        return read_json(cached)
    say(f"{take}: gezicht zoeken voor de webcam-uitsnede")
    crop = json.loads(uv_run(OPENCV, "python", HERE / "camcrop_run.py", d / "cam.mkv").stdout)
    cached.write_text(json.dumps(crop))
    return crop


def prepare(take: str, model: str, denoise: bool) -> None:
    ingest(take)
    clean(take, denoise=denoise)
    transcribe(take, model=model)
    correct(take)
    suggest(take)
    cam_crop(take)


# ---- the config ----------------------------------------------------------------


def load_config(required: bool = True) -> dict:
    if not CONFIG.exists():
        if required:
            raise SystemExit(f"{CONFIG.relative_to(ROOT)} ontbreekt")
        return {}
    return yaml.safe_load(CONFIG.read_text()) or {}


def words_for(cfg: dict, take: str) -> list[dict]:
    """A take's words, corrected when `correct` (or a person) wrote
    `corrected.txt`: the captions, the transcript and the draft scripts all
    read these."""
    from .timeline import align_text

    d = take_dir(take)
    words = read_json(d / "words.json", {}).get("words", [])
    fixed = d / "corrected.txt"
    return align_text(words, fixed.read_text()) if fixed.exists() else words


def pauses_for(cfg: dict, take: str, words: list[dict]) -> list[dict]:
    """The pauses of a take, in take time.

    Found in the words where the presenter invites the viewer to look around
    (`find_invitations`), unless the take sets `auto_pauses: false`; plus the
    ones in `pauses:` in walkthrough.yaml, where `off: true` removes a found
    one within a second of `at`.
    """
    take_cfg = (cfg.get("takes") or {}).get(take) or {}
    found = find_invitations(words) if take_cfg.get("auto_pauses", True) else []
    for p in cfg.get("pauses") or []:
        if p.get("take") != take:
            continue
        at = float(p["at"])
        # YAML 1.1 reads a bare `off` as the boolean false, also as a key.
        if p.get("off", p.get(False)):
            found = [f for f in found if abs(f["at"] - at) > 1.0]
        else:
            found.append({"at": at, "start": at, **({"hint": p["hint"]} if p.get("hint") else {})})
    return sorted(found, key=lambda f: f["at"])


def cuts_for(cfg: dict, take: str) -> list[tuple[float, float]]:
    raw = ((cfg.get("takes") or {}).get(take) or {}).get("cuts") or []
    return [(float(c["from"]), float(c["to"])) for c in raw]


def take_info(take: str) -> dict:
    info = read_json(take_dir(take) / "take.json")
    if not info or not (take_dir(take) / "app.cfr.mp4").exists():
        return ingest(take)
    return info


# ---- build -----------------------------------------------------------------------


def sha(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def hashed(path: Path, stem: str, dest: Path) -> dict:
    digest = sha(path)
    name = f"{stem}-{digest[:12]}{path.suffix}"
    shutil.move(path, dest / name)
    return {"src": name, "sha256": digest, "bytes": (dest / name).stat().st_size}


def concat_files(parts: list[Path], out: Path, workdir: Path) -> None:
    listing = workdir / f"{out.stem}.txt"
    listing.write_text("".join(f"file '{p}'\n" for p in parts))
    ffmpeg("-f", "concat", "-safe", "0", "-i", listing, "-c", "copy", out)


def deck_of(take: str, overrides: dict) -> list[dict]:
    """The deck as it stood when `take` was recorded, with the slide
    corrections from walkthrough.yaml: the replay shows these slides."""
    meta = read_json(take_dir(take) / "meta.json", {})
    deck = [dict(s) for s in meta.get("slides") or []]
    for i, fix in overrides.items():
        if 0 <= i < len(deck):
            deck[i].update(fix or {})
    return deck


def render_recorded(name: str, segments: list[dict], cfg: dict, overrides: dict, workdir: Path) -> dict:
    """A stretch of recorded takes: voice, video and webcam, cut as written."""
    takes = sorted({s["take"] for s in segments})
    infos = {t: take_info(t) for t in takes}
    durations = {t: infos[t]["duration"] for t in takes}
    events = {t: read_json(take_dir(t) / "events.json", []) for t in takes}
    # Chapters written after recording get their slides behind the deck of the
    # take that leads the track, since that is the deck the replay shows.
    lead = segments[0]["take"]
    added: list[dict] = []
    for t in takes:
        extra = ((cfg.get("takes") or {}).get(t) or {}).get("chapters") or []
        if not extra:
            continue
        deck_len = len(deck_of(lead, {})) + len(added)
        events[t], more = add_chapters(events[t], extra, deck_len, t, read_json(take_dir(t) / "states.json", {}))
        added += more
    # Actions written after recording (`actions:` under a take): what the
    # presenter had done before the take began and the log therefore lacks,
    # such as a scenario already run in a tab that was still open.
    for t in takes:
        extra = ((cfg.get("takes") or {}).get(t) or {}).get("actions") or []
        if extra:
            events[t] = sorted(events[t] + [{"t": float(a["at"]) * 1000, **{k: v for k, v in a.items() if k != "at"}} for a in extra], key=lambda e: e.get("t", 0))
    words = {t: words_for(cfg, t) for t in takes}
    cuts = {t: cuts_for(cfg, t) for t in takes}
    for t in takes:
        check_cuts(cuts[t], protected_spans(events[t]))
        if not (take_dir(t) / "voice.wav").exists():
            clean(t)

    track = build_track(segments, cuts, durations, fps=FPS)
    # The deck and the size are the opening take's, not the alphabetically
    # first: a retake spliced in later may be older or newer.
    lead = segments[0]["take"]
    first = infos[lead]
    width, height = first["width"], first["height"]
    has_cam = all((take_dir(t) / "cam.cfr.mp4").exists() for t in takes)
    say(f"{name}: {len(track.pieces)} opgenomen stukken, {track.duration:.1f}s{', met webcam' if has_cam else ''}")

    # The webcam piece by piece, each a whole number of frames, then joined
    # without re-encoding. One filter graph over all pieces would have ffmpeg
    # buffer raw frames of one take while it waits on another. The recording
    # of the window is not rendered: the walkthrough is the demo replaying,
    # and a screen too small for that is told so instead of shown a video
    # whose slides are the ones of the recording day.
    clist = []
    for i, p in enumerate(track.pieces):
        n = media.frames(p.length)
        if has_cam:
            crop = cam_crop(p.take)
            offset = infos[p.take].get("camOffset") or 0
            cvf = f"crop={crop['size']}:{crop['size']}:{crop['x']}:{crop['y']},scale={CAM_SIZE}:{CAM_SIZE},setsar=1,format=yuv420p"
            cout = workdir / f"{name}-c{i:04d}.mp4"
            ffmpeg("-ss", max(0.0, p.start - offset), "-i", take_dir(p.take) / "cam.cfr.mp4", "-an", "-vf", cvf, "-frames:v", n, "-c:v", "libx264", "-preset", "medium", "-crf", "26", "-g", FPS * 4, cout)
            clist.append(cout)

    # The voice: the same pieces, with a 15 ms fade on each side of every cut
    # so a cut does not click. Loudness comes later, over the whole track.
    inputs, graph, labels = [], [], []
    take_index = {}
    for t in takes:
        take_index[t] = len(inputs) // 2
        inputs += ["-i", take_dir(t) / "voice.wav"]
    for i, p in enumerate(track.pieces):
        fade = min(0.015, p.length / 4)
        graph.append(
            f"[{take_index[p.take]}:a]atrim=start={p.start}:end={p.end},asetpts=PTS-STARTPTS,"
            f"afade=t=in:d={fade},afade=t=out:st={p.length - fade:.4f}:d={fade}[a{i}]"
        )
        labels.append(f"[a{i}]")
    graph.append(f"{''.join(labels)}concat=n={len(labels)}:v=0:a=1[out]")
    raw = workdir / f"{name}-voice-raw.wav"
    script = workdir / f"{name}-audio.filter"
    script.write_text(";\n".join(graph))
    ffmpeg(*inputs, "-filter_complex_script", script, "-map", "[out]", "-ac", "1", "-ar", "48000", "-t", f"{track.duration:.4f}", raw)

    meta = read_json(take_dir(track.pieces[0].take) / "meta.json", {}) if track.pieces else {}
    return {
        "duration": round(track.duration, 3),
        "voice": raw,
        "video": None,
        "cam": clist if has_cam else None,
        "size": (width, height),
        "chapters": chapters(track, events, overrides),
        "events": remap_actions(track, events),
        "words": remap_words(track, words),
        "pauses": place_pauses(track, {t: pauses_for(cfg, t, words[t]) for t in takes}),
        "slides": deck_of(lead, overrides) + added,
        "recordedAt": meta.get("startedAt"),
        "viewport": meta.get("viewport"),
        "pieces": [{"take": p.take, "from": p.start, "to": p.end, "at": p.out} for p in track.pieces],
    }


def render_scripted(name: str, seg: dict, cfg: dict, overrides: dict, workdir: Path) -> dict:
    """A chapter spoken from its script, in the generated voice, with the
    actions of its silent take laid on the words (script.py)."""
    from .script import beats, layout, parse_line
    from .voice import speak

    path = CORPUS / "script" / f"{seg['script']}.yaml"
    if not path.exists():
        raise SystemExit(f"script ontbreekt: {path.relative_to(ROOT)}")
    doc = yaml.safe_load(path.read_text()) or {}
    lines = [parse_line(str(raw)) for raw in doc.get("lines") or []]
    if not lines:
        raise SystemExit(f"{path.name}: geen regels")
    voice_cfg = {k: v for k, v in (cfg.get("voice") or {"provider": "say"}).items() if k != "dub"}
    if voice_cfg.get("provider", "say") == "elevenlabs" and not voice_cfg.get("voice_id"):
        raise SystemExit("walkthrough.yaml: voice.voice_id ontbreekt; zoek de id van de kloon op met `walkthrough voices`")
    texts = [line.text for line in lines]
    audio = [speak(text, voice_cfg, WORK, texts[i - 1] if i else "", texts[i + 1] if i + 1 < len(texts) else "") for i, text in enumerate(texts)]
    take = doc.get("take")
    events = read_json(take_dir(take) / "events.json", []) if take else []
    # A script may cover one stretch of a longer take (`walkthrough script`
    # writes one per slide). The state is the one the stretch begins with.
    lo, hi = doc.get("from"), doc.get("to")
    if lo is not None or hi is not None:
        lo_ms = float(lo or 0) * 1000
        hi_ms = float(hi) * 1000 if hi is not None else float("inf")
        start_slide = next((e for e in reversed(events) if e.get("type") == "slide" and e["t"] <= lo_ms + 1), None)
        events = ([start_slide] if start_slide else []) + [e for e in events if lo_ms < e["t"] < hi_ms and e is not start_slide]
    runs = beats(events)
    try:
        plan = layout(lines, audio, runs)
    except ValueError as e:
        raise SystemExit(f"{path.name}: {e}")
    say(f"{name}: script {seg['script']}, {len(lines)} regels, {len(runs)} handelingen, {plan['duration']:.1f}s ({voice_cfg.get('provider', 'say')})")

    # The lines at their places, with silence between them where a line waits.
    pieces, at = [], 0.0
    for clip in plan["clips"]:
        gap = clip["at"] - at
        if gap > 0.001:
            silence = workdir / f"{name}-gap{len(pieces):04d}.wav"
            ffmpeg("-f", "lavfi", "-i", "anullsrc=r=48000:cl=mono", "-t", f"{gap:.4f}", silence)
            pieces.append(silence)
        pieces.append(audio[clip["line"]]["wav"])
        at = clip["at"] + audio[clip["line"]]["duration"]
    tail = plan["duration"] - at
    if tail > 0.001:
        silence = workdir / f"{name}-tail.wav"
        ffmpeg("-f", "lavfi", "-i", "anullsrc=r=48000:cl=mono", "-t", f"{tail:.4f}", silence)
        pieces.append(silence)
    raw = workdir / f"{name}-voice-raw.wav"
    listing = workdir / f"{name}-voice.txt"
    listing.write_text("".join(f"file '{p}'\n" for p in pieces))
    ffmpeg("-f", "concat", "-safe", "0", "-i", listing, "-ac", "1", "-ar", "48000", raw)

    slide_index = int(doc.get("slide", 0))
    first = next((e for e in events if e.get("type") == "slide"), None)
    state = first.get("state") if first else None
    deck = deck_of(take, overrides) if take else []
    slide = deck[slide_index] if 0 <= slide_index < len(deck) else {}
    meta = read_json(take_dir(take) / "meta.json", {}) if take else {}
    acts = [{"t": 0.0, "type": "restore", "state": state, "slideIndex": slide_index}] if state else [{"t": 0.0, "type": "slide", "index": slide_index}]
    return {
        "duration": plan["duration"],
        "voice": raw,
        "video": None,
        "cam": None,
        "size": None,
        "chapters": [{"start": 0.0, "end": plan["duration"], "slideIndex": slide_index, "slide": slide, "profile": first.get("profile") if first else None, "state": state}],
        "events": acts + plan["events"],
        "words": plan["words"],
        "slides": deck,
        "recordedAt": meta.get("startedAt"),
        "viewport": meta.get("viewport"),
        "pieces": [{"script": seg["script"], "at": 0.0}],
        # A script says its invitations too; its words are already in output time.
        "pauses": [{"t": round(f["at"], 3)} for f in find_invitations(plan["words"])],
        "generated": True,
    }


def render_track(name: str, segments: list[dict], cfg: dict, overrides: dict, dest: Path, vtt_dest: Path, workdir: Path) -> dict:
    """A playable track from its segments: runs of recorded takes and
    scripted chapters, laid end to end on one clock."""
    parts, run = [], []
    for seg in segments:
        if "script" in seg:
            if run:
                parts.append(render_recorded(f"{name}-p{len(parts)}", run, cfg, overrides, workdir))
                run = []
            parts.append(render_scripted(f"{name}-p{len(parts)}", seg, cfg, overrides, workdir))
        else:
            run.append(seg)
    if run:
        parts.append(render_recorded(f"{name}-p{len(parts)}", run, cfg, overrides, workdir))

    offset = 0.0
    seams = []  # where one part hands over to the next
    chs, acts, words, pieces, pauses = [], [], [], [], []
    for part in parts:
        pauses += [{**x, "t": round(x["t"] + offset, 3)} for x in part.get("pauses") or []]
        for c in part["chapters"]:
            chs.append({**c, "start": round(c["start"] + offset, 3), "end": round(c["end"] + offset, 3)})
        acts += [{**e, "t": round(e["t"] + offset, 3)} for e in part["events"]]
        words += [{**w, "start": round(w["start"] + offset, 3), "end": round(w["end"] + offset, 3)} for w in part["words"]]
        pieces += [{**p, "at": round(p["at"] + offset, 3)} for p in part["pieces"]]
        offset += part["duration"]
        seams.append(round(offset, 3))
    duration = round(offset, 3)
    # Neighbouring chapters on the same slide (a recorded opening that ends on
    # the slide a script continues on) are one chapter.
    merged = []
    for c in chs:
        if merged and merged[-1]["slideIndex"] == c["slideIndex"]:
            merged[-1]["end"] = c["end"]
        else:
            merged.append(c)

    # One voice: the parts end to end, then loudness over all of it, so the
    # recorded opening and the generated chapters sound equally loud.
    raw = workdir / f"{name}-voice-raw.wav"
    listing = workdir / f"{name}-voices.txt"
    listing.write_text("".join(f"file '{p['voice']}'\n" for p in parts))
    ffmpeg("-f", "concat", "-safe", "0", "-i", listing, "-ac", "1", "-ar", "48000", raw)
    voice = workdir / f"{name}-voice.wav"
    media.loudnorm(raw, voice)
    audio_out = workdir / f"{name}-voice.m4a"
    ffmpeg("-i", voice, "-c:a", "aac", "-b:a", "96k", "-ac", "1", "-movflags", "+faststart", audio_out)

    entry: dict = {"duration": duration, "audio": hashed(audio_out, f"{name}-voice", dest)}

    # The window's video only when every part was recorded: a generated
    # chapter has no picture of its own. A phone then gets the MP4 made from
    # the replay itself (`walkthrough export`).
    if all(p["video"] for p in parts):
        video_only = workdir / f"{name}-video.mp4"
        concat_files([v for p in parts for v in p["video"]], video_only, workdir)
        main = workdir / f"{name}.mp4"
        ffmpeg("-i", video_only, "-i", voice, "-map", "0:v", "-map", "1:a", "-c:v", "copy", "-c:a", "aac", "-b:a", "128k", "-shortest", "-movflags", "+faststart", main)
        w, h = parts[0]["size"]
        entry["video"] = {**hashed(main, name, dest), "width": w, "height": h}
    else:
        entry["video"] = None

    # The presenter's bubble: the webcam of the recorded parts at the start
    # (the opening). Where the generated voice takes over, the bubble goes.
    lead = []
    for p in parts:
        if not p["cam"]:
            break
        lead.append(p)
    if lead:
        cam_only = workdir / f"{name}-cam.mp4"
        concat_files([c for p in lead for c in p["cam"]], cam_only, workdir)
        cam_final = workdir / f"{name}-cam-final.mp4"
        ffmpeg("-i", cam_only, "-c", "copy", "-movflags", "+faststart", cam_final)
        entry["cam"] = {**hashed(cam_final, f"{name}-cam", dest), "until": round(sum(p["duration"] for p in lead), 3)}
    else:
        entry["cam"] = None

    first = next((p for p in parts if p["slides"]), parts[0])
    entry["slides"] = first["slides"]
    entry["recordedAt"] = next((p["recordedAt"] for p in parts if p["recordedAt"]), None)
    entry["viewport"] = first["viewport"]
    entry["generatedVoice"] = any(p.get("generated") for p in parts)
    entry["chapters"] = merged
    entry["events"] = sorted(acts, key=lambda e: e["t"])
    # Where the player stops by itself and invites the viewer to look around.
    entry["pauses"] = sorted(pauses, key=lambda x: x["t"])
    # A caption ends at a chapter and at a seam between parts: a sentence cut
    # off at the end of the recorded opening is not continued by the voice.
    cues = captions(sorted(words, key=lambda w: w["start"]), breaks=[c["start"] for c in merged[1:]] + seams[:-1])
    (WORK / f"{name}.said.json").write_text(json.dumps([norm_cue(c["text"]) for c in cues], ensure_ascii=False))
    vtt = to_vtt(apply_subtitles(cues, shown_subtitles()))
    vtt_name = f"{name}-{hashlib.sha256(vtt.encode()).hexdigest()[:12]}.nl.vtt"
    (vtt_dest / vtt_name).write_text(vtt)
    entry["captions"] = {"nl": vtt_name}
    entry["pieces"] = pieces
    # For a dub, not for the player: `build` moves these to sentences.json.
    entry["sentences"] = dub.sentences(sorted(words, key=lambda w: w["start"]), breaks=[x["t"] for x in pauses] + seams[:-1], duration=duration)
    return entry


SUBTITLES = CORPUS / "subtitles.yaml"

SUBTITLE_PROMPT = """Hieronder staan de ondertitels van een gesproken rondleiding door de demo
van RegelRecht (wetten als machine-uitvoerbare regels), genummerd, in
volgorde. Het is spreektaal, letterlijk uitgeschreven. Maak er ondertitels
van die prettig lezen.

Regels:
- Kort: hooguit twee regels van 42 tekens, liefst één.
- Laat vulwoorden, herhalingen en valse starts weg ("eigenlijk", "dus",
  "best wel", "En dan zien we hier").
- Houd de betekenis, de vakbegrippen, namen en getallen precies zoals ze
  zijn. Voeg niets toe dat niet gezegd is.
- Een ondertitel die al kort en helder is, laat je staan.
- Een ondertitel hoort bij zijn eigen moment: schuif geen tekst naar een
  andere.

{context}
De invoer is een JSON-object van nummer naar ondertitel. Antwoord met
alleen een JSON-object met precies dezelfde nummers, elk met zijn korte
ondertitel.

{cues}
"""


# How many captions go to the model in one question.
SUBTITLE_BATCH = 50


def shown_subtitles() -> dict[str, str]:
    """What was said -> the subtitle to show, from subtitles.yaml."""
    data = yaml.safe_load(SUBTITLES.read_text()) if SUBTITLES.exists() else None
    return {norm_cue(e["gezegd"]): e["ondertitel"] for e in (data or []) if e.get("gezegd") and e.get("ondertitel")}


def subtitles() -> None:
    """Shorter subtitles for the captions that have none yet.

    Reads the captions of the last build, asks a language model (headless
    Claude Code) for a short version of each one not in subtitles.yaml yet,
    and appends them there: a list of `gezegd` and `ondertitel`, in git,
    for anyone to correct. A rebuild puts them on screen. The words as said
    stay the key, so an edited line keeps holding when the cuts change.
    """
    said = []
    for f in sorted(WORK.glob("*.said.json")):
        said += [t for t in json.loads(f.read_text()) if t not in said]
    if not said:
        raise SystemExit("geen ondertitels gevonden; draai eerst `just walkthrough build`")
    have = shown_subtitles()
    todo = [t for t in said if t not in have]
    if not todo:
        say("alle ondertitels hebben al een korte versie")
        return
    if not shutil.which("claude"):
        raise SystemExit("geen `claude` op het pad; vul subtitles.yaml met de hand")
    terms = load_config(required=False).get("glossary") or []
    context = ("Begrippen: " + ", ".join(terms) + ".\n") if terms else ""
    say(f"{len(todo)} ondertitels inkorten met een taalmodel")
    header = (
        "---\n# Ondertitels: wat er gezegd is, en wat er in beeld komt.\n"
        "# `just walkthrough subtitles` vult nieuwe aan; corrigeer gerust met de hand.\n"
        "# Een rebuild (`just walkthrough build`) zet ze in beeld.\n"
    )
    entries = (yaml.safe_load(SUBTITLES.read_text()) if SUBTITLES.exists() else None) or []
    done = shorter = 0
    # In batches: over a few hundred lines the model merges or drops one, and
    # then the answer no longer lines up with the question. Each batch that
    # does line up is written straight away, so a rerun only asks what is left.
    for i in range(0, len(todo), SUBTITLE_BATCH):
        batch = todo[i : i + SUBTITLE_BATCH]
        short = None
        for _ in range(2):
            r = subprocess.run(
                ["claude", "-p", "--model", "sonnet", "--tools", ""],
                input=SUBTITLE_PROMPT.format(context=context, cues=json.dumps({str(n + 1): c for n, c in enumerate(batch)}, ensure_ascii=False, indent=0)),
                capture_output=True,
                text=True,
            )
            out = r.stdout.strip()
            try:
                got = json.loads(out[out.find("{") : out.rfind("}") + 1])
            except json.JSONDecodeError:
                continue
            # Numbered, so a line the model merged into its neighbour shows
            # as a missing number instead of shifting every line after it.
            got = [got.get(str(n + 1)) for n in range(len(batch))] if isinstance(got, dict) else []
            if got and all(isinstance(x, str) and x.strip() for x in got):
                short = got
                break
        if short is None:
            say(f"  {i + 1}-{i + len(batch)}: het antwoord paste niet op de vraag; overgeslagen")
            continue
        entries += [{"gezegd": a, "ondertitel": " ".join(b.split())} for a, b in zip(batch, short)]
        SUBTITLES.write_text(header + yaml.safe_dump(entries, allow_unicode=True, sort_keys=False, width=1000))
        done += len(batch)
        shorter += sum(1 for a, b in zip(batch, short) if len(b) < len(a))
    say(f"{done} van {len(todo)} ondertitels in {SUBTITLES.relative_to(ROOT)}, {shorter} korter; draai `just walkthrough build`")
    if done < len(todo):
        raise SystemExit("niet alles is ingekort; draai `just walkthrough subtitles` nog een keer")


def build(release: str | None = None) -> None:
    cfg = load_config()
    main_cfg = cfg.get("main") or {}
    if not main_cfg.get("segments"):
        raise SystemExit("walkthrough.yaml: main.segments is leeg")
    overrides = {int(k): v for k, v in (cfg.get("slides") or {}).items()}
    WORK.mkdir(parents=True, exist_ok=True)
    # Everything is rendered aside and only swapped in once all of it worked:
    # a failed or interrupted build leaves the previous timeline and the files
    # it names in place.
    with tempfile.TemporaryDirectory(dir=WORK) as tmp:
        work = Path(tmp)
        media_out, vtt_out = work / "media", work / "captions"
        media_out.mkdir()
        vtt_out.mkdir()
        try:
            main = render_track("main", main_cfg["segments"], cfg, overrides, media_out, vtt_out, work)
            faqs = []
            for f in cfg.get("faq") or []:
                segs = f.get("segments") or ([{"script": f["script"]}] if f.get("script") else [{"take": f["take"], "from": f.get("from"), "to": f.get("to")}])
                entry = render_track(f"faq-{f['id']}", segs, cfg, {}, media_out, vtt_out, work)
                offer = f.get("offer") or {}
                at = None
                if "take" in offer:
                    at = next((round(p["at"] + offer["at"] - p["from"], 3) for p in main["pieces"] if p.get("take") == offer["take"] and p["from"] <= offer["at"] < p["to"]), None)
                    if at is None:
                        say(f"let op: het aanbiedmoment van '{f['id']}' valt in een knip of buiten de rondleiding")
                elif "chapter" in offer:
                    idx = int(offer["chapter"])
                    if not 0 <= idx < len(main["chapters"]):
                        raise SystemExit(f"walkthrough.yaml: vraag '{f['id']}' wijst naar hoofdstuk {idx}, er zijn er {len(main['chapters'])}")
                    at = main["chapters"][idx]["start"] + float(offer.get("after", 0))
                faqs.append({"id": f["id"], "question": f["question"], "offer": at, **entry})
            bumpers = render_bumpers(cfg, media_out, work)
        except CutError as e:
            raise SystemExit(f"walkthrough.yaml: {e}")
        said = {"main": {"duration": main["duration"], "sentences": main.pop("sentences")}}
        for f in faqs:
            said[f"faq-{f['id']}"] = {"duration": f["duration"], "sentences": f.pop("sentences")}
        old = read_json(CORPUS / "timeline.json", {}) or {}
        dubs, keep = kept_dubs(old, said)
        # A rebuild replaces everything: stale media under an old hash would be
        # served and fetched for nothing. A dub made against the same Dutch
        # sentences still fits, and stays.
        PUBLIC.mkdir(parents=True, exist_ok=True)
        for f in [*PUBLIC.glob("*"), *CORPUS.glob("*.vtt")]:
            if f.is_file() and f.name not in keep:
                f.unlink()
        for f in [*media_out.iterdir()]:
            shutil.move(f, PUBLIC / f.name)
        for f in [*vtt_out.iterdir()]:
            shutil.move(f, CORPUS / f.name)
    timeline = {
        "version": 1,
        "release": release or cfg.get("release"),
        "presenter": cfg.get("presenter") or {},
        "main": main,
        "faq": faqs,
        "bumpers": bumpers,
    }
    for name, track in (("main", main), *((f"faq-{f['id']}", f) for f in faqs)):
        if dubs.get(name):
            track["dub"] = dubs[name]
    if old.get("dub"):
        timeline["dub"] = old["dub"]
    SENTENCES.write_text(json.dumps(said, ensure_ascii=False, indent=1) + "\n")
    report_translations(localize.apply(timeline, CORPUS), timeline)
    (CORPUS / "timeline.json").write_text(json.dumps(timeline, ensure_ascii=False, indent=1) + "\n")
    total = sum(sum((t.get(k) or {}).get("bytes") or 0 for k in ("audio", "video", "cam")) for t in [main, *faqs])
    say(f"klaar: {main['duration']:.0f}s rondleiding, {len(faqs)} vragen, {total / 1e6:.0f} MB media in {PUBLIC.relative_to(ROOT)}")


def tracks_of(timeline: dict) -> dict[str, dict]:
    """The playable tracks by name: `main` and `faq-<id>`."""
    return {"main": timeline["main"], **{f"faq-{f['id']}": f for f in timeline.get("faq") or []}}


def kept_dubs(old: dict, said: dict) -> tuple[dict[str, dict], set[str]]:
    """The dubs of the previous timeline that still fit the new one, per
    track, and the files they use. A dub fits when it was made against the
    same Dutch sentences at the same times; one that does not goes, with a
    word on how to make it again."""
    if not old.get("main"):
        return {}, set()
    dubs, keep = {}, set()
    for name, track in tracks_of(old).items():
        for code, d in (track.get("dub") or {}).items():
            if name in said and d.get("basis") == dub.basis(said[name]):
                dubs.setdefault(name, {})[code] = d
                keep |= {d["audio"]["src"], d["captions"]}
            else:
                say(f"{name}: de nagesproken versie ({code}) past niet meer bij de nieuwe build; draai `walkthrough dub {code}`")
    for d in (old.get("dub") or {}).values():
        keep |= {c["src"] for clips in (d.get("bumpers") or {}).values() for c in clips}
    return dubs, keep


def render_bumpers(cfg: dict, dest: Path, workdir: Path) -> dict:
    """The short lines the player says when the viewer takes over.

    `bumpers: {wait: [...], resume: [...]}` in walkthrough.yaml, each a
    stretch of a take (`take`, `from`, `to`): "Ga je gang, kijk maar even
    rond, ik wacht." when the viewer starts clicking, "Oké, we gaan verder."
    when they press play again. Each becomes its own small audio file, as
    loud as the walkthrough; the player picks one at random. None configured:
    the player only shows its hint.
    """
    out: dict = {}
    for kind in ("wait", "resume"):
        clips = []
        for i, c in enumerate(((cfg.get("bumpers") or {}).get(kind)) or []):
            take, lo, hi = c["take"], float(c["from"]), float(c["to"])
            if hi <= lo:
                raise SystemExit(f"walkthrough.yaml: bumpers.{kind}[{i}] eindigt voor het begint")
            src = take_dir(take) / "voice.wav"
            if not src.exists():
                clean(take)
            raw = workdir / f"bumper-{kind}-{i}-raw.wav"
            fade = min(0.03, (hi - lo) / 4)
            ffmpeg("-i", src, "-af", f"atrim=start={lo}:end={hi},asetpts=PTS-STARTPTS,afade=t=in:d={fade},afade=t=out:st={hi - lo - fade:.4f}:d={fade}", "-ac", "1", "-ar", "48000", raw)
            norm = workdir / f"bumper-{kind}-{i}.wav"
            media.loudnorm(raw, norm)
            m4a = workdir / f"bumper-{kind}-{i}.m4a"
            ffmpeg("-i", norm, "-c:a", "aac", "-b:a", "96k", "-ac", "1", "-movflags", "+faststart", m4a)
            clips.append({**hashed(m4a, f"bumper-{kind}", dest), "duration": round(hi - lo, 3)})
        if clips:
            out[kind] = clips
    if out:
        say(f"tussenzinnen: {len(out.get('wait', []))} wachten, {len(out.get('resume', []))} verder")
    return out


# ---- reading back ------------------------------------------------------------------


def transcript(which: str) -> None:
    """The spoken text per slide: what Claude reads to bring the slides in line."""
    if which == "main" or which.startswith("faq-"):
        timeline = read_json(CORPUS / "timeline.json")
        if not timeline:
            raise SystemExit("nog geen timeline.json; draai eerst `walkthrough build`")
        track = timeline["main"] if which == "main" else next(f for f in timeline["faq"] if f"faq-{f['id']}" == which)
        from .timeline import Piece, Track

        tr = Track([Piece(p["take"], p["from"], p["to"], p["at"]) for p in track["pieces"] if "take" in p])
        words = remap_words(tr, {t: words_for(load_config(required=False), t) for t in {p.take for p in tr.pieces}})
        chs = track["chapters"]
    else:
        words = words_for(load_config(required=False), which)
        events = read_json(take_dir(which) / "events.json", [])
        from .timeline import Track, Piece

        tr = Track([Piece(which, 0, 1e9, 0)])
        chs = chapters(tr, {which: events})
    for i, c in enumerate(chs):
        end = chs[i + 1]["start"] if i + 1 < len(chs) else 1e9
        text = " ".join(w["word"] for w in words if c["start"] <= w["start"] < end)
        s = c.get("slide") or {}
        title = s.get("title") or " / ".join(s.get("lines") or []) or f"dia {c['slideIndex']}"
        print(f"\n## {c['slideIndex']}. {title}  [{c['start']:.1f}s]\n")
        print(text or "(stil)")


def draft_scripts(take: str) -> list[Path]:
    """A script per slide of `take`, from what was said and done in it.

    The presenter's own words, one line per sentence, with a marker where each
    action began. Corrections go in the files; a rerun does not overwrite a
    script that already exists.
    """
    from .script import beat_starts, draft_lines

    d = take_dir(take)
    words = words_for(load_config(required=False), take)
    if not words:
        raise SystemExit(f"{take}: nog geen transcript; draai eerst `just walkthrough prepare {take}`")
    events = read_json(d / "events.json", [])
    end = next((e["t"] / 1000 for e in reversed(events) if e.get("type") == "end"), words[-1]["end"] + 1)
    slides = [e for e in events if e.get("type") == "slide"]
    # One stretch per slide; a slide shown only for a moment (the deck passing
    # through) is not a chapter.
    stretches = []
    for i, e in enumerate(slides):
        lo = e["t"] / 1000
        hi = slides[i + 1]["t"] / 1000 if i + 1 < len(slides) else end
        if hi - lo >= 1.0:
            stretches.append((e.get("index", 0), lo, hi))
    out_dir = CORPUS / "script"
    out_dir.mkdir(parents=True, exist_ok=True)
    written = []
    for index, lo, hi in stretches:
        part_words = [w for w in words if lo <= w["start"] < hi]
        if not part_words:
            continue
        part_events = [e for e in events if lo * 1000 < e["t"] < hi * 1000]
        lines = draft_lines(part_words, beat_starts(part_events))
        path = out_dir / f"{take}-dia{index}.yaml"
        if path.exists():
            say(f"bestaat al, niet overschreven: {path.relative_to(ROOT)}")
            continue
        doc = {"slide": index, "take": take, "from": round(lo, 3), "to": round(hi, 3), "lines": lines}
        header = (
            f"# Concept uit opname {take}, dia {index}: wat er gezegd is, met [n] waar een\n"
            "# handeling begon. Pas de zinnen gerust aan; elke [n] moet blijven staan.\n"
            "---\n"
        )
        path.write_text(header + yaml.safe_dump(doc, allow_unicode=True, sort_keys=False, width=1000))
        written.append(path)
        say(f"{path.relative_to(ROOT)}: {len(lines)} regels")
    return written


def voices() -> None:
    """The voices on the ElevenLabs account, to find the clone's voice id."""
    import urllib.request

    from .voice import api_key

    req = urllib.request.Request("https://api.elevenlabs.io/v1/voices", headers={"xi-api-key": api_key(WORK)})
    with urllib.request.urlopen(req, timeout=30) as res:
        data = json.loads(res.read())
    for v in data.get("voices", []):
        print(f"{v['voice_id']}  {v.get('category', ''):12} {v.get('name', '')}")
    print("\nZet de id van je kloon in walkthrough.yaml onder voice.voice_id.", file=sys.stderr)


def publish(tag: str, yes: bool) -> None:
    """Put the built media in a GitHub release and point the timeline at it.

    Creates the release (or adds to it), uploads every file timeline.json
    names, and writes the tag into timeline.json, so the Docker build fetches
    exactly these files. Asks first: a release is public.
    """
    timeline = read_json(CORPUS / "timeline.json")
    if not timeline:
        raise SystemExit("nog geen timeline.json; draai eerst `just walkthrough build`")
    tracks = [timeline["main"], *(timeline.get("faq") or [])]
    files = [PUBLIC / m["src"] for t in tracks for m in (t.get("audio"), t.get("video"), t.get("cam")) if m]
    files += [PUBLIC / b["src"] for clips in (timeline.get("bumpers") or {}).values() for b in clips]
    # The walkthrough in other languages (`dub`) and its short lines.
    files += [PUBLIC / d["audio"]["src"] for t in tracks for d in (t.get("dub") or {}).values()]
    files += [PUBLIC / b["src"] for d in (timeline.get("dub") or {}).values() for clips in (d.get("bumpers") or {}).values() for b in clips]
    missing = [f.name for f in files if not f.exists()]
    if missing:
        raise SystemExit(f"ontbreken in {PUBLIC.relative_to(ROOT)}: {missing}; draai `just walkthrough build` opnieuw")
    size = sum(f.stat().st_size for f in files) / 1e6
    repo = "NederlandseDigitaleDienst/regelrecht"
    say(f"Release {tag} op {repo}: {len(files)} bestanden, {size:.0f} MB. Een release is openbaar.")
    if not yes:
        if input("Doorgaan? [j/N] ").strip().lower() not in ("j", "ja", "y", "yes"):
            raise SystemExit("niets gepubliceerd")
    gh = shutil.which("gh") or "gh"
    exists = subprocess.run([gh, "release", "view", tag, "-R", repo], capture_output=True).returncode == 0
    if not exists:
        subprocess.run([gh, "release", "create", tag, "-R", repo, "--title", f"Rondleiding demo ({tag})", "--notes", "Media van de opgenomen rondleiding door de demo (corpus/demo/walkthrough/timeline.json). Geen softwarerelease.", "--latest=false"], check=True)
    subprocess.run([gh, "release", "upload", tag, "-R", repo, "--clobber", *map(str, files)], check=True)
    timeline["release"] = tag
    (CORPUS / "timeline.json").write_text(json.dumps(timeline, ensure_ascii=False, indent=1) + "\n")
    say(f"klaar: timeline.json wijst naar {tag}. Commit corpus/demo/walkthrough/ en de Docker-build haalt de media op.")


def states(url: str) -> None:
    """The demo's state at each chapter written after recording.

    Replays the main line on the dev server at `url` and stores, per added
    chapter (`chapters:` under a take in walkthrough.yaml), the state the demo
    is in when it starts, in the take's states.json. A rebuild then gives
    those chapters a snapshot, so a jump to one restores it directly.
    """
    timeline = read_json(CORPUS / "timeline.json")
    if not timeline:
        raise SystemExit("nog geen timeline.json; draai eerst `walkthrough build`")
    added = [c for c in timeline["main"]["chapters"] if c.get("added")]
    if not added:
        say("geen hoofdstukken die achteraf zijn toegevoegd")
        return
    subprocess.run(["node", str(ROOT / "frontend-demo" / "scripts" / "copy-demo-corpus.mjs")], check=True, capture_output=True)
    vp = timeline["main"].get("viewport") or {"width": 1600, "height": 1000}
    with tempfile.TemporaryDirectory() as tmp:
        (Path(tmp) / "vp.json").write_text(json.dumps(vp))
        say(f"{len(added)} hoofdstukken: de rondleiding afspelen en de stand vastleggen (dit duurt een paar minuten)")
        uv_run(PLAYWRIGHT, "python", HERE / "states_run.py", CORPUS / "timeline.json", Path(tmp) / "vp.json", Path(tmp) / "out.json", url)
        found = json.loads((Path(tmp) / "out.json").read_text())
    by_take: dict[str, dict] = {}
    for key, state in found.items():
        take, at = json.loads(key)
        by_take.setdefault(take, {})[str(float(at))] = state
    for take, per in by_take.items():
        (take_dir(take) / "states.json").write_text(json.dumps(per, ensure_ascii=False))
        say(f"{take}: {len(per)} standen; draai `just walkthrough build` opnieuw")


def anchors(url: str) -> None:
    """Scroll anchors for the takes in the walkthrough that lack them.

    Replays the built walkthrough on the dev server, per take in the layout
    it was recorded in, and writes what was in the middle of each scroller
    into the take's events.json (the original is kept as events.orig.json).
    A rebuild then carries the anchors into timeline.json.
    """
    timeline = read_json(CORPUS / "timeline.json")
    if not timeline:
        raise SystemExit("nog geen timeline.json; draai eerst `walkthrough build`")
    tracks = [timeline["main"], *(timeline.get("faq") or [])]
    takes = sorted({e["src"][0] for t in tracks for e in t.get("events") or [] if e.get("type") == "scroll" and e.get("src") and not e.get("anchor")})
    # The dev server plays its own copy of the timeline; it has to be this one.
    subprocess.run(["node", str(ROOT / "frontend-demo" / "scripts" / "copy-demo-corpus.mjs")], check=True, capture_output=True)
    if not takes:
        say("alle scrolls hebben al een anker")
        return
    for take in takes:
        d = take_dir(take)
        vp = (read_json(d / "meta.json", {}) or {}).get("viewport")
        if not vp:
            say(f"{take}: geen viewport in meta.json; overgeslagen")
            continue
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / "vp.json").write_text(json.dumps(vp))
            say(f"{take}: ankers meten in {vp['width']}x{vp['height']} (dit duurt een paar minuten)")
            uv_run(PLAYWRIGHT, "python", HERE / "anchors_run.py", CORPUS / "timeline.json", Path(tmp) / "vp.json", Path(tmp) / "out.json", url)
            found = json.loads((Path(tmp) / "out.json").read_text())
        events = read_json(d / "events.json", [])
        if not (d / "events.orig.json").exists():
            (d / "events.orig.json").write_text(json.dumps(events))
        n = 0
        for e in events:
            hit = found.get(json.dumps([take, e["t"]])) if e.get("type") == "scroll" else None
            if hit and hit.get("anchor"):
                e.update(hit)
                n += 1
        (d / "events.json").write_text(json.dumps(events))
        say(f"{take}: {n} scrolls met een anker; draai `just walkthrough build` opnieuw")


# ---- other languages ------------------------------------------------------------------


def report_translations(report: dict, timeline: dict) -> None:
    for code, miss in report.items():
        gaps = ", ".join(f"{len(v)} {k}" for k, v in miss.items() if v)
        say(f"{code}: " + (f"nog niet vertaald: {gaps}; draai `walkthrough translate {code}`" if gaps else "alles vertaald"))


class IndentedDumper(yaml.SafeDumper):
    """Lists under a key indented, the way yamllint wants them in the corpus."""

    def increase_indent(self, flow=False, indentless=False):
        return super().increase_indent(flow, False)


LANGUAGE_NAMES = {"en": "Engels (Brits gespeld, zoals de Engelse demo: organisation, behaviour)", "fy": "Fries", "fr": "Frans"}

TRANSLATE_PROMPT = """Je vertaalt de tekst bij een gesproken rondleiding door de demo van
RegelRecht (wetten als machine-uitvoerbare regels) naar het {language}. De
presentator spreekt Nederlands; {what}.

Regels:
- Vertaal wat er staat, niet meer en niet minder. Houd getallen, bedragen,
  namen en wetsnamen zoals ze zijn.
- {brevity}
- Vaste vertalingen, altijd zo:
{terms}
- Staat iets op het scherm onder een andere naam in de demo, gebruik dan die
  naam, ook voor een toeslag of wet ("Zorgtoeslag" is in de demo
  "Healthcare allowance"). Deze labels gebruikt de demo in het {language}:
{labels}
- Een regel hoort bij zijn eigen moment: schuif geen tekst naar een andere.
  Een zin die over twee regels loopt, mag in de vertaling ook over twee
  regels lopen.
{extra}
De invoer is een JSON-object van nummer naar Nederlandse tekst, in volgorde.
Antwoord met alleen een JSON-object met precies dezelfde nummers, elk met zijn
vertaling.

{items}
"""

TRANSLATE_WHAT = {
    "captions": "de kijker leest jouw vertaling als ondertitel, terwijl hij het Nederlands hoort",
    "slides": "de kijker leest jouw vertaling op de dia's naast de demo; **vet** blijft **vet**",
    "questions": "de kijker leest jouw vertaling als vraag in de lijst met veelgestelde vragen",
    "typed": "de kijker leest jouw vertaling als tekst die de presentator in een formulier van de demo typt",
    "spoken": (
        "de kijker hoort jouw vertaling, uitgesproken door de stem van de presentator, "
        "precies op het moment dat hij de Nederlandse zin zei, terwijl de demo doet wat hij toen deed"
    ),
}

TRANSLATE_BREVITY = {
    "spoken": (
        "Spreektaal, zoals een presentator het zegt, geen schrijftaal. Laat vulwoorden\n"
        "  en valse starts weg. Elke zin heeft een tijdvak: blijf onder `hooguit_woorden`,\n"
        "  liever ruim eronder; een zin die niet past wordt versneld of ingekort.\n"
        "  \"Hier\", \"dit\" en \"nu\" wijzen naar wat de demo op dat moment doet: houd ze.\n"
        "  Een Nederlands brokstuk dat de vorige zin al afmaakt, mag leeg blijven (\"\")."
    ),
}
BREVITY = "Kort en helder; ondertitels hooguit twee regels van 42 tekens."


def ui_labels(code: str, texts: list[str]) -> dict[str, str]:
    """What the demo calls things in `code`: the interface's short labels
    (frontend-demo/src/i18n) whose Dutch occurs in `texts`. When the
    presenter says "klik op Regelwerken", the English caption says
    "Ruleworks", which is what the viewer sees."""
    script = (
        "const [nl, local] = await Promise.all([import('./src/i18n/nl.js'), import(`./src/i18n/${process.argv[1]}.js`)]);"
        "const out = {};"
        "for (const [k, v] of Object.entries(nl.default)) {"
        "  const l = local.default[k];"
        "  if (typeof v === 'string' && typeof l === 'string' && v !== l && v.length <= 40 && !v.includes('{')) out[v] = l;"
        "}"
        "console.log(JSON.stringify(out));"
    )
    r = subprocess.run(["node", "--input-type=module", "-e", script, code], cwd=ROOT / "frontend-demo", capture_output=True, text=True)
    pairs = {**{a: b for a, b in localize.deck_pairs(code, prefix=None).items() if len(a) <= 40}, **json.loads(r.stdout or "{}")}
    said = " ".join(texts).lower()
    return {a: b for a, b in pairs.items() if len(a) > 3 and f" {a.lower()}" in f" {said}"}


# Smaller than for shortening: a translation answer is longer, and a batch
# of fifty came back cut off now and then.
TRANSLATE_BATCH = 25


def ask_translations(kind: str, items: list[str], code: str, labels: dict[str, str], terms: dict[str, str], budgets: dict[str, int] | None = None) -> dict[str, str]:
    """Translations for `items` from a language model, in batches. With
    `budgets`, each item goes with the most words its translation may have."""
    language = LANGUAGE_NAMES.get(code, code)
    names = load_config(required=False).get("glossary") or []
    extra = ("- Namen die blijven zoals ze zijn: " + ", ".join(n for n in names if n.lower() not in {t.lower() for t in terms}) + ".\n") if names else ""
    label_lines = "\n".join(f"  {a} = {b}" for a, b in sorted(labels.items())) or "  (geen)"
    term_lines = "\n".join(f"  {a} = {b}" for a, b in terms.items()) or "  (geen)"
    out: dict[str, str] = {}
    for i in range(0, len(items), TRANSLATE_BATCH):
        batch = items[i : i + TRANSLATE_BATCH]
        got = None
        for _ in range(2):
            r = subprocess.run(
                ["claude", "-p", "--model", "sonnet", "--tools", ""],
                input=TRANSLATE_PROMPT.format(
                    language=language,
                    what=TRANSLATE_WHAT[kind],
                    brevity=TRANSLATE_BREVITY.get(kind, BREVITY),
                    terms=term_lines,
                    labels=label_lines,
                    extra=extra,
                    items=json.dumps({str(n + 1): ({"tekst": c, "hooguit_woorden": budgets[c]} if budgets and c in budgets else c) for n, c in enumerate(batch)}, ensure_ascii=False, indent=0),
                ),
                capture_output=True,
                text=True,
            )
            text = r.stdout.strip()
            try:
                answer = json.loads(text[text.find("{") : text.rfind("}") + 1])
            except json.JSONDecodeError:
                continue
            answer = [answer.get(str(n + 1)) for n in range(len(batch))] if isinstance(answer, dict) else []
            # A caption may come back empty: a fragment ("gaan.") that the
            # line before already says in the other language's word order.
            if answer and all(isinstance(x, str) and (x.strip() or kind in ("captions", "spoken")) for x in answer):
                got = answer
                break
        if got is None:
            say(f"  {kind} {i + 1}-{i + len(batch)}: het antwoord paste niet op de vraag; overgeslagen")
            continue
        out.update({a: " ".join(b.split()) for a, b in zip(batch, got)})
        say(f"  {kind}: {min(i + len(batch), len(items))} van {len(items)}")
    return out


def save_translations(code: str, data: dict) -> None:
    """corpus/demo/i18n/walkthrough.<code>.yaml, with its header."""
    header = (
        "---\n"
        f"# De rondleiding in het {LANGUAGE_NAMES.get(code, code).split(' (')[0]}: wat de kijker leest terwijl de\n"
        "# presentator Nederlands spreekt, en onder `spoken:` wat zijn gekloonde stem\n"
        "# zegt. Gesleuteld op het Nederlands, zodat een nieuwe knip of build elke\n"
        "# regel houdt die nog voorkomt.\n"
        f"# `just walkthrough translate {code}` vult aan wat ontbreekt; corrigeer gerust\n"
        "# met de hand. Een build (`just walkthrough build`) zet het in beeld,\n"
        f"# `just walkthrough dub {code}` laat het uitspreken.\n"
    )
    # Folded at 100: a spoken sentence is longer than yamllint allows on one line.
    localize.path_for(code).write_text(header + yaml.dump(data, Dumper=IndentedDumper, allow_unicode=True, sort_keys=False, width=100))


def dub_languages() -> set[str]:
    """The languages with a cloned voice: `voice.dub` in walkthrough.yaml."""
    return set(((load_config(required=False).get("voice") or {}).get("dub")) or {})


def voice_for(code: str, provider: str | None = None) -> dict:
    """The voice settings for `code`: `voice` in walkthrough.yaml with
    `voice.dub.<code>` over it."""
    voice = dict(load_config().get("voice") or {})
    lang = (voice.pop("dub", None) or {}).get(code)
    if lang is None:
        raise SystemExit(f"walkthrough.yaml: geen voice.dub.{code}; zet daar de stem voor deze taal")
    out = {**voice, **lang, "settings": {**(voice.get("settings") or {}), **(lang.get("settings") or {})}}
    out.setdefault("language", code)
    if provider:
        out["provider"] = provider
    if out.get("provider", "say") == "elevenlabs" and not out.get("voice_id"):
        raise SystemExit("walkthrough.yaml: voice.voice_id ontbreekt; zoek de id van de kloon op met `walkthrough voices`")
    return out


# Without the Dutch original: with it next to the translation, the model
# answered in Dutch, twice out of two. With the sentences around it: without
# them it wrote a different sentence ("Still so.", "Right on that.").
SHORTEN_PROMPT = """Hieronder staat een zin in het {language} uit een gesproken rondleiding
door de demo van RegelRecht (wetten als machine-uitvoerbare regels). Hij is te
lang voor het moment waarop hij klinkt. Schrijf hem korter, in dezelfde taal:
hooguit {words} woorden, spreektaal.

- Hij moet hetzelfde zeggen en aansluiten op de zinnen eromheen. Liever een
  zin die iets te lang blijft dan een die iets anders zegt.
- Vakbegrippen, namen en getallen blijven precies zoals ze zijn.

Ervoor: {before}
De zin: {local}
Erna: {after}

Antwoord met alleen de kortere zin, in het {language}.
"""


def ask_shorter(local: str, words: int, code: str, before: str = "", after: str = "", source: str = "") -> str | None:
    """A shorter version of a spoken translation, or None: without a model,
    or when the answer is not shorter or is the Dutch `source` again."""
    if not shutil.which("claude"):
        return None
    r = subprocess.run(
        ["claude", "-p", "--model", "sonnet", "--tools", ""],
        input=SHORTEN_PROMPT.format(language=LANGUAGE_NAMES.get(code, code), words=words, local=local, before=before or "-", after=after or "-"),
        capture_output=True,
        text=True,
        check=False,
    )
    out = " ".join(r.stdout.split()).strip('"')
    if not out or len(out.split()) >= len(local.split()) or dub.echoes(out, source):
        return None
    return out


def dub_track(name: str, track: dict, said: dict, code: str, voice: dict, tr: dict, data: dict, workdir: Path) -> list[Path]:
    """One track in the cloned voice in `code`, sentence by sentence in the
    slots of the Dutch (dub.py). Writes the audio and its captions and puts
    them in `track["dub"][code]`. Returns the files of the dub it replaces,
    for the caller to remove once the new timeline is written."""
    from .voice import speak

    sents = said["sentences"]
    # Read per sentence, not once: a sentence shortened here is shortened
    # wherever it occurs again, in this track or a later one.
    text_of = lambda i: tr["spoken"][sents[i]["nl"]]
    clips, placed, words = [], [], []
    # Where the last placed line ends. A line that ran over its slot moves the
    # next one on by that much rather than talking over it: a fraction of a
    # second later than the Dutch is not heard, two voices at once are.
    cursor = 0.0
    tally = {"sneller": 0, "gerekt": 0, "ingekort": 0, "geperst": 0}
    for i, x in enumerate(sents):
        text = text_of(i)
        if not text.strip():
            continue  # a fragment the sentence before already says
        slot = x["until"] - x["start"]
        prev = next((text_of(j) for j in range(i - 1, -1, -1) if text_of(j).strip()), "")
        nxt = next((text_of(j) for j in range(i + 1, len(sents)) if text_of(j).strip()), "")
        # A trial voice (`say`) is not the clone: a sentence too long for it
        # says nothing about the clone, so it is squeezed, never shortened.
        speed, shortened = 1.0, x["nl"] in tr["shortened"] or voice.get("provider") != "elevenlabs"
        while True:
            cfg = {**voice, "settings": {**voice["settings"], "speed": speed}} if speed != 1.0 else voice
            audio = speak(text, cfg, WORK, prev, nxt)
            step = dub.fit(audio["duration"], slot, speed, shortened, can_speed=voice.get("speed_control", True), tempo_max=float(voice.get("tempo_max", dub.TEMPO_MAX)))
            if step.action == "speed":
                speed = step.speed
                continue
            if step.action == "shorten":
                shortened = True
                tr["shortened"].add(x["nl"])
                target = max(2, int(len(text.split()) * slot / audio["duration"] * 0.9))
                new = ask_shorter(text, target, code, prev, nxt, source=x["nl"])
                if new:
                    tally["ingekort"] += 1
                    say(f"  ingekort: {text!r} -> {new!r}")
                    entry = next(e for e in data["spoken"] if e.get("nl") and norm_cue(e["nl"]) == x["nl"])
                    entry[code], entry["shortened"] = new, True
                    save_translations(code, data)
                    tr["spoken"][x["nl"]] = new
                    text, speed = new, 1.0
                continue
            break
        tally["sneller"] += speed > 1.0
        wav = audio["wav"]
        if step.tempo > 1.0001:
            tally["geperst" if step.action == "squeeze" else "gerekt"] += 1
            stretched = workdir / f"{name}-{i}.wav"
            ffmpeg("-i", wav, "-af", f"atempo={step.tempo}", "-ar", "48000", "-ac", "1", stretched)
            wav = stretched
        at = max(x["start"], cursor)
        cursor = at + audio["duration"] / step.tempo
        clips.append((dub.read_pcm(wav), at))
        placed.append((x, text, round(cursor - x["until"], 2)))
        words += dub.aligned_words(text, audio["alignment"], at, step.tempo)

    samples = dub.assemble(clips, said["duration"])
    raw = workdir / f"{name}-{code}-raw.wav"
    dub.write_pcm(samples, raw)
    norm = workdir / f"{name}-{code}.wav"
    media.loudnorm(raw, norm)
    # Exactly as long as the Dutch: the player keeps the demo on its clock.
    m4a = workdir / f"{name}-{code}.m4a"
    ffmpeg("-i", norm, "-af", f"apad,atrim=end={said['duration']}", "-c:a", "aac", "-b:a", "96k", "-ac", "1", "-movflags", "+faststart", m4a)
    old = (track.get("dub") or {}).get(code) or {}
    audio_entry = hashed(m4a, f"{name}-voice-{code}", PUBLIC)
    vtt = to_vtt(captions(sorted(words, key=lambda w: w["start"]), breaks=[p["t"] for p in track.get("pauses") or []]))
    vtt_name = f"{name}-{hashlib.sha256(vtt.encode()).hexdigest()[:12]}.{code}.dub.vtt"
    (CORPUS / vtt_name).write_text(vtt)
    track.setdefault("dub", {})[code] = {"audio": audio_entry, "captions": vtt_name, "basis": dub.basis(said)}

    steps = ", ".join(f"{n} {k}" for k, n in tally.items() if n)
    say(f"{name} ({code}): {len(clips)} zinnen" + (f" ({steps})" if steps else "") + f"; {sum(len(t) for _, t, _ in placed)} tekens")
    for x, text, over in placed:
        if over > 0.05:
            say(f"  te lang ({over:.1f}s over het tijdvak) bij {x['start']:.1f}s: {text!r}; maak de zin korter in spoken: en draai dub opnieuw")
    stale = [(PUBLIC, (old.get("audio") or {}).get("src")), (CORPUS, old.get("captions"))]
    return [where / f for where, f in stale if f and f not in (audio_entry["src"], vtt_name)]


def dub_bumpers(code: str, voice: dict, tr: dict, timeline: dict, workdir: Path) -> list[Path]:
    """The player's short lines ("Go ahead, I'll wait.") in the cloned
    voice, from `bumpers:` in walkthrough.<code>.yaml. Returns the files of
    the lines they replace."""
    from .voice import speak

    old = ((timeline.get("dub") or {}).get(code) or {}).get("bumpers") or {}
    out: dict = {}
    for kind in ("wait", "resume"):
        clips = []
        for i, text in enumerate((tr["bumpers"] or {}).get(kind) or []):
            audio = speak(text, voice, WORK)
            norm = workdir / f"bumper-{kind}-{code}-{i}.wav"
            media.loudnorm(audio["wav"], norm)
            m4a = workdir / f"bumper-{kind}-{code}-{i}.m4a"
            ffmpeg("-i", norm, "-c:a", "aac", "-b:a", "96k", "-ac", "1", "-movflags", "+faststart", m4a)
            clips.append({**hashed(m4a, f"bumper-{kind}-{code}", PUBLIC), "duration": round(audio["duration"], 3)})
        if clips:
            out[kind] = clips
    if out:
        timeline.setdefault("dub", {}).setdefault(code, {})["bumpers"] = out
        say(f"tussenzinnen ({code}): {len(out.get('wait', []))} wachten, {len(out.get('resume', []))} verder")
    else:
        say(f"geen tussenzinnen in {localize.path_for(code).relative_to(ROOT)} (bumpers:); de speler toont dan alleen de hint")
    kept = {c["src"] for clips in out.values() for c in clips}
    return [PUBLIC / c["src"] for clips in old.values() for c in clips if c["src"] not in kept]


def dub_cmd(code: str, only: str | None = None, provider: str | None = None) -> None:
    """The walkthrough in `code`, spoken in the presenter's cloned voice."""
    if code == localize.SOURCE:
        raise SystemExit("het Nederlands is de opname zelf; kies een andere taal")
    timeline = read_json(CORPUS / "timeline.json")
    said = read_json(SENTENCES)
    if not timeline or not said:
        raise SystemExit("nog geen timeline.json of sentences.json; draai eerst `walkthrough build`")
    voice = voice_for(code, provider)
    tr = localize.load(code)
    data = yaml.safe_load(localize.path_for(code).read_text()) or {}
    tracks = tracks_of(timeline)
    if only and only not in (*tracks, "bumpers"):
        raise SystemExit(f"onbekend spoor {only}; kies uit {', '.join([*tracks, 'bumpers'])}")
    todo = {name: track for name, track in tracks.items() if only in (None, name)}
    # Everything that can stop the run is checked before anything is spent.
    for name, track in todo.items():
        if said.get(name, {}).get("duration") != track["duration"]:
            raise SystemExit(f"{name}: sentences.json hoort bij een andere build; draai `walkthrough build`")
        missing = {x["nl"] for x in said[name]["sentences"] if x["nl"] not in tr["spoken"]}
        if missing:
            raise SystemExit(f"{name}: {len(missing)} zinnen zonder vertaling in {localize.path_for(code).relative_to(ROOT)}; draai `walkthrough translate {code}`")
    PUBLIC.mkdir(parents=True, exist_ok=True)
    WORK.mkdir(parents=True, exist_ok=True)

    def save() -> None:
        (CORPUS / "timeline.json").write_text(json.dumps(timeline, ensure_ascii=False, indent=1) + "\n")

    # Saved after every track, and the files a track replaces removed only
    # once the timeline no longer names them: a run that stops halfway (a
    # quota, Ctrl-C) leaves a timeline whose files all exist.
    with tempfile.TemporaryDirectory(dir=WORK) as tmp:
        for name, track in todo.items():
            stale = dub_track(name, track, said[name], code, voice, tr, data, Path(tmp))
            save()
            for f in stale:
                f.unlink(missing_ok=True)
        if only in (None, "bumpers"):
            stale = dub_bumpers(code, voice, tr, timeline, Path(tmp))
            save()
            for f in stale:
                f.unlink(missing_ok=True)
    say(f"klaar; luister in de dev-server en draai `walkthrough publish <tag>` voor de media ({voice.get('provider', 'say')})")


def translate(code: str) -> None:
    """Captions, slides, questions and typed sentences in `code`, for what is
    not translated yet.

    Asks a language model (headless Claude Code) for each missing line and
    appends it to corpus/demo/i18n/walkthrough.<code>.yaml, keyed by the
    Dutch, for anyone to correct by hand. Then applies the file to the
    timeline, which a build would also do.
    """
    import copy

    if code == localize.SOURCE:
        raise SystemExit("het Nederlands is de bron; kies een andere taal")
    timeline = read_json(CORPUS / "timeline.json")
    if not timeline:
        raise SystemExit("nog geen timeline.json; draai eerst `walkthrough build`")
    if not shutil.which("claude"):
        raise SystemExit(f"geen `claude` op het pad; vul {localize.path_for(code).relative_to(ROOT)} met de hand")
    path = localize.path_for(code)
    if not path.exists():
        path.write_text("---\n")
    miss = localize.apply(copy.deepcopy(timeline), CORPUS, write=False).get(code) or {}
    labels = ui_labels(code, [t for v in miss.values() for t in v])
    data = yaml.safe_load(path.read_text()) or {}

    def save():
        save_translations(code, data)

    questions = {f["question"]: f["id"] for f in timeline.get("faq") or []}
    for kind in ("questions", "slides", "typed", "captions"):
        todo = list(dict.fromkeys(miss.get(kind) or []))
        if not todo:
            continue
        say(f"{len(todo)} {kind} vertalen naar {code}")
        got = ask_translations(kind, todo, code, labels, localize.load(code)["terms"])
        if kind == "questions":
            data.setdefault("questions", {}).update({questions[q]: v for q, v in got.items()})
        else:
            data.setdefault(kind, [])
            data[kind] += [{"nl": a, code: b} for a, b in got.items()]
        save()
    # What the cloned voice says, for a language that has one (voice.dub in
    # walkthrough.yaml): every sentence as said, with its word budget.
    if code in dub_languages():
        said = read_json(SENTENCES, {}) or {}
        sents = [x for t in said.values() for x in t["sentences"]]
        have = localize.load(code)["spoken"]
        todo = list(dict.fromkeys(x["nl"] for x in sents if x["nl"] not in have))
        wps = float(((load_config().get("voice") or {}).get("dub") or {}).get(code, {}).get("words_per_second", dub.WORDS_PER_SECOND))
        if todo:
            say(f"{len(todo)} gesproken zinnen vertalen naar {code}")
            budgets: dict[str, int] = {}
            for x in sents:
                budgets[x["nl"]] = min(budgets.get(x["nl"], 999), dub.budget(x, wps))
            got = ask_translations("spoken", todo, code, ui_labels(code, todo), localize.load(code)["terms"], budgets=budgets)
            data.setdefault("spoken", [])
            data["spoken"] += [{"nl": a, code: b} for a, b in got.items()]
            save()
    report = localize.apply(timeline, CORPUS)
    (CORPUS / "timeline.json").write_text(json.dumps(timeline, ensure_ascii=False, indent=1) + "\n")
    report_translations(report, timeline)


def takes_in_use(cfg: dict) -> set[str]:
    """Every take the walkthrough draws on: directly, or through a script."""
    segments = list((cfg.get("main") or {}).get("segments") or [])
    for f in cfg.get("faq") or []:
        segments += f.get("segments") or [{"take": f.get("take"), "script": f.get("script")}]
    used = set()
    for seg in segments:
        if seg.get("take"):
            used.add(seg["take"])
        if seg.get("script"):
            path = CORPUS / "script" / f"{seg['script']}.yaml"
            if path.exists():
                take = (yaml.safe_load(path.read_text()) or {}).get("take")
                if take:
                    used.add(take)
    return used


def check(take: str) -> None:
    """What a take sounds and looks like, in numbers: so whoever runs the
    session can say right away whether a take is usable or needs doing again.

    Loudness and peaks of the raw voice, the noise floor in the pauses, how
    much the transcription caught, and what the action log holds. Each line
    ends in a verdict: ok, or what to do about it.
    """
    d = take_dir(take)
    if not (d / "mic.wav").exists():
        ingest(take)
    info = read_json(d / "take.json", {})
    events = read_json(d / "events.json", [])
    meta = read_json(d / "meta.json", {})
    words = read_json(d / "words.json", {}).get("words", [])
    report = []

    def line(label, value, verdict):
        report.append(f"  {label:<16} {value:<28} {verdict}")

    dur = info.get("duration") or 0
    line("duur", f"{dur:.0f} s", "ok" if dur >= 5 else "erg kort: per ongeluk gestopt?")

    stats = media.run([media.require("ffmpeg"), "-hide_banner", "-i", d / "mic.wav", "-af", "astats=metadata=0:reset=0,ebur128=framelog=quiet", "-f", "null", "-"]).stderr

    def grab(key):
        import re as _re

        m = _re.findall(rf"{key}:\s*(-?[\d.]+|-inf)", stats)
        return float(m[-1]) if m and m[-1] != "-inf" else None

    peak = grab("Peak level dB")
    loud = grab(r"I")
    if peak is not None:
        line("piek", f"{peak:.1f} dBFS", "te hard: oversturing, microfoon zachter" if peak > -1.0 else "ok" if peak > -20 else "te zacht: dichter bij de microfoon of gain omhoog")
    if loud is not None:
        line("luidheid", f"{loud:.1f} LUFS", "ok (de nabewerking trekt het gelijk)" if loud > -38 else "erg zacht: ruis wordt bij het versterken hoorbaar")
    gaps = media.silences(d / "mic.wav", noise_db=-50, min_len=0.5)
    floor = None
    if gaps:
        s0, s1 = max(gaps, key=lambda g: g[1] - g[0])
        probe = media.run([media.require("ffmpeg"), "-hide_banner", "-ss", s0, "-to", s1, "-i", d / "mic.wav", "-af", "astats=metadata=0", "-f", "null", "-"]).stderr
        import re as _re

        m = _re.findall(r"RMS level dB:\s*(-?[\d.]+)", probe)
        floor = float(m[-1]) if m else None
    if floor is not None:
        line("ruisvloer", f"{floor:.1f} dBFS", "ok" if floor < -55 else "hoorbaar geruis: ventilator, raam of kamer; DeepFilterNet haalt veel weg" if floor < -42 else "veel achtergrondgeluid: neem op een stillere plek op")
    if words:
        wpm = len(words) / max(dur / 60, 0.01)
        line("woorden", f"{len(words)} ({wpm:.0f}/min)", "ok" if 90 <= wpm <= 175 else "erg snel: rustiger" if wpm > 175 else "weinig gesproken")
        low = [w for w in words if (w.get("score") or 1) < 0.4]
        if words and len(low) / len(words) > 0.15:
            line("verstaanbaar", f"{len(low)} twijfelwoorden", "veel onzeker herkend: duidelijker articuleren of dichter bij de microfoon")
    else:
        line("woorden", "geen transcript", f"draai `just walkthrough prepare {take}`")
    kinds = {}
    for e in events:
        kinds[e.get("type")] = kinds.get(e.get("type"), 0) + 1
    acts = sum(kinds.get(k, 0) for k in ("click", "input", "change", "key", "scroll"))
    slides = sorted({e.get("index") for e in events if e.get("type") == "slide"})
    line("handelingen", f"{acts} ({kinds.get('click', 0)} klikken, {kinds.get('input', 0)} toetsen)", "ok" if acts or not slides else "geen klikken gelogd")
    line("dia's", str(slides), "ok" if slides else "het dek liep niet: start de presentatie in de opname")
    line("verspreking", str(kinds.get("flub", 0)), "ok" if not kinds.get("flub") else "gemarkeerd, de knipvoorstellen nemen ze mee")
    line("webcam", "ja" if (d / "cam.webm").exists() else "nee", "ok")
    if meta.get("locale") not in (None, "nl"):
        line("taal", meta["locale"], "neem op in het Nederlands: de demo zoekt knoppen op hun Nederlandse tekst")
    print(f"opname {take}")
    print("\n".join(report))


def status() -> None:
    cfg = load_config(required=False)
    used = takes_in_use(cfg)
    for d in sorted(TAKES.glob("*")):
        if not d.is_dir():
            continue
        steps = [n for n, f in (("ingest", "take.json"), ("clean", "voice.wav"), ("transcribe", "words.json"), ("suggest", "cuts.suggested.yaml")) if (d / f).exists()]
        info = read_json(d / "take.json", {})
        mark = "*" if d.name in used else " "
        print(f"{mark} {d.name}  {info.get('duration', 0):7.1f}s  dia's {info.get('slides', '?')}  [{', '.join(steps) or 'ruw'}]")
    tl = read_json(CORPUS / "timeline.json")
    if tl:
        print(f"\ntimeline.json: {tl['main']['duration']:.0f}s, {len(tl['main']['chapters'])} hoofdstukken, {len(tl['faq'])} vragen")


# ---- export ---------------------------------------------------------------------------


def export() -> None:
    from .export import export_all

    timeline = read_json(CORPUS / "timeline.json")
    if not timeline:
        raise SystemExit("nog geen timeline.json; draai eerst `walkthrough build`")
    out = WORK / "export"
    out.mkdir(parents=True, exist_ok=True)
    export_all(timeline, PUBLIC, CORPUS, out)


def main(argv: list[str] | None = None) -> None:
    ap = argparse.ArgumentParser(prog="walkthrough", description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("prepare", "ingest", "clean", "transcribe", "correct", "suggest"):
        p = sub.add_parser(name)
        p.add_argument("take", nargs="?", help="map in .walkthrough/takes (standaard: de laatste)")
        if name in ("prepare", "transcribe"):
            p.add_argument("--model", default="large-v3", help="Whisper-model, bijv. large-v3-turbo voor sneller")
        if name in ("prepare", "clean"):
            p.add_argument("--no-denoise", action="store_true", help="ffmpeg-ruisfilter in plaats van DeepFilterNet")
    p = sub.add_parser("transcript")
    p.add_argument("which", nargs="?", default="main", help="main, faq-<id> of een opname")
    p = sub.add_parser("build")
    p.add_argument("--release", help="tag van de GitHub-release met de media")
    sub.add_parser("export")
    sub.add_parser("status")
    p = sub.add_parser("script", help="een concept-script per dia uit een opname")
    p.add_argument("take", nargs="?", help="map in .walkthrough/takes (standaard: de laatste)")
    sub.add_parser("voices", help="de stemmen op je ElevenLabs-account")
    p = sub.add_parser("check", help="hoe een opname klinkt, in getallen")
    p.add_argument("take", nargs="?", help="map in .walkthrough/takes (standaard: de laatste)")
    sub.add_parser("subtitles", help="kortere ondertitels laten maken, in subtitles.yaml")
    p = sub.add_parser("translate", help="ondertitels, dia's en vragen in een andere taal")
    p.add_argument("code", help="de taal, bijvoorbeeld en")
    p = sub.add_parser("dub", help="de rondleiding in een andere taal, in de gekloonde stem")
    p.add_argument("code", help="de taal, bijvoorbeeld en")
    p.add_argument("--track", help="alleen dit spoor: main, faq-<id> of bumpers")
    p.add_argument("--provider", choices=("elevenlabs", "say"), help="in plaats van voice.provider; say om zonder account te proberen")
    p = sub.add_parser("anchors", help="scrollankers meten voor oudere opnames")
    p.add_argument("--url", default="http://127.0.0.1:7400", help="de dev-server")
    p = sub.add_parser("states", help="de demostand vastleggen bij achteraf toegevoegde hoofdstukken")
    p.add_argument("--url", default="http://127.0.0.1:7400", help="de dev-server")
    p = sub.add_parser("publish", help="media in een GitHub-release zetten")
    p.add_argument("tag", help="bijvoorbeeld walkthrough-2026-10")
    p.add_argument("--yes", action="store_true", help="niet vragen")
    args = ap.parse_args(argv)

    take = getattr(args, "take", None) or (latest_take() if args.cmd in ("prepare", "ingest", "clean", "transcribe", "correct", "suggest") else None)
    if args.cmd == "prepare":
        prepare(take, args.model, not args.no_denoise)
    elif args.cmd == "ingest":
        ingest(take)
    elif args.cmd == "clean":
        clean(take, denoise=not args.no_denoise)
    elif args.cmd == "transcribe":
        transcribe(take, args.model)
    elif args.cmd == "correct":
        correct(take)
    elif args.cmd == "suggest":
        print(yaml.safe_dump(suggest(take), allow_unicode=True, sort_keys=False))
    elif args.cmd == "transcript":
        transcript(args.which)
    elif args.cmd == "build":
        build(args.release)
    elif args.cmd == "subtitles":
        subtitles()
    elif args.cmd == "translate":
        translate(args.code)
    elif args.cmd == "dub":
        dub_cmd(args.code, args.track, args.provider)
    elif args.cmd == "anchors":
        anchors(args.url)
    elif args.cmd == "states":
        states(args.url)
    elif args.cmd == "export":
        export()
    elif args.cmd == "script":
        draft_scripts(args.take or latest_take())
    elif args.cmd == "check":
        check(args.take or latest_take())
    elif args.cmd == "voices":
        voices()
    elif args.cmd == "publish":
        publish(args.tag, args.yes)
    elif args.cmd == "status":
        status()


if __name__ == "__main__":
    main()
