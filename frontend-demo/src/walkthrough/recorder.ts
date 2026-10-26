/**
 * The walkthrough recorder: captures a narrated run through the demo.
 *
 * Three tracks, one clock:
 *  - the app, as video: the whole current tab through `getDisplayMedia`, with
 *    the microphone as its audio track. The whole tab and not a crop of the
 *    workspace: side sheets and dialogs are placed against the window, and a
 *    crop next to the deck rail cut the law list out of the picture. The
 *    take is recorded in zaal mode, where the deck steps aside on a demo
 *    slide, so the demo fills the window; the player puts the slide text
 *    back next to it;
 *  - the webcam, as a separate video, for the presenter bubble;
 *  - an event log: slide changes, routes, clicks, typing and flub marks.
 *
 * The event log is metadata, not a replay script. Playback shows the video;
 * the log tells post-processing where the chapters are, where a click should
 * ripple, and where it must not cut (in the middle of typing).
 *
 * Chunks stream to the Vite dev server (scripts/walkthrough-dev-server.mjs),
 * which appends them to `.walkthrough/takes/<take>/`. A crash halfway through
 * a take therefore loses seconds, not the take. This module is only imported
 * in dev (see App.vue); production has no microphone or camera permission.
 */
import { reactive } from 'vue';
import { micConstraints } from './micCheck';
import { camConstraints } from './camCheck';
import { escapeStops } from '../presentation/usePresentation';

export const SNAPSHOT_KEY = 'rr-walkthrough-snapshots-v1';
const ENDPOINT = '/__walkthrough/takes';
const CHUNK_MS = 4000;

export type RecorderPhase = 'idle' | 'starting' | 'recording' | 'saving' | 'done' | 'error';

/**
 * An entry for the event log: its `type` and whatever else it carries. `ago`:
 * the action began that many ms before it was logged.
 */
export interface LoggedEvent {
  type: string;
  ago?: number;
  [key: string]: unknown;
}

/** An entry as it lands in events.json: on the take's clock. */
interface TimedEvent {
  t: number | null;
  type: string;
  [key: string]: unknown;
}

/** What a failed step throws: an Error, maybe with the i18n key the panel shows, or `fatal` when retrying is pointless. */
interface RecorderError {
  message?: unknown;
  key?: string;
  fatal?: boolean;
}

/** The Screen Capture options Chromium takes that the DOM lib does not list. */
type CaptureOptions = DisplayMediaStreamOptions & {
  preferCurrentTab?: boolean;
  selfBrowserSurface?: 'include' | 'exclude';
  surfaceSwitching?: 'include' | 'exclude';
  monitorTypeSurfaces?: 'include' | 'exclude';
};

interface Uploader {
  push(blob: Blob, options?: { replace?: boolean }): Promise<void>;
  done: () => Promise<void>;
}

interface Session {
  takeId: string;
  streams: MediaStream[];
  app: MediaRecorder;
  camRec: MediaRecorder | null;
  up: { app: Uploader; cam: Uploader | null; events: Uploader };
  events: TimedEvent[];
  t0: number | null;
  camStart: number | null;
  meta: Record<string, unknown>;
  tick: ReturnType<typeof setInterval> | undefined;
}

export const recorder = reactive({
  phase: 'idle' as RecorderPhase, // idle | starting | recording | saving | done | error
  takeId: null as string | null,
  error: null as string | null,
  errorKey: null as string | null, // an i18n key when the error is one the panel can explain
  elapsed: 0,
  withCamera: true,
  flubs: 0,
});

let session: Session | null = null;

/** A take id that sorts by time and is safe as a directory name. */
export function newTakeId(date = new Date()) {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}-${pad(date.getMinutes())}-${pad(date.getSeconds())}`;
}

/** The first MIME type this browser's MediaRecorder accepts. */
function pickMime(candidates: string[]) {
  return candidates.find((m) => window.MediaRecorder?.isTypeSupported?.(m)) ?? '';
}

/**
 * Uploads one file's chunks in order. MediaRecorder hands out chunks faster
 * than a slow request may finish, and an append out of order corrupts the
 * WebM, so each file gets its own promise chain.
 */
function uploader(takeId: string, file: string): Uploader {
  let chain = Promise.resolve();
  let first = true;
  const url = (mode: string) => `${ENDPOINT}/${encodeURIComponent(takeId)}/${encodeURIComponent(file)}?mode=${mode}`;
  // A dev server that restarts or hiccups should not cost the take: each
  // chunk is retried a few times before the recording gives up.
  async function send(blob: Blob, mode: string) {
    let last: unknown;
    for (let attempt = 0; attempt < 4; attempt += 1) {
      try {
        // The custom header makes a cross-site POST need a preflight, which
        // the dev server does not answer; only this page can write takes.
        const r = await fetch(url(mode), { method: 'POST', body: blob, headers: { 'x-walkthrough': '1', 'content-type': 'application/octet-stream' } });
        if (r.ok) return;
        // No endpoint at all: the dev server was not started for recording.
        if (r.status === 404) throw Object.assign(new Error('no endpoint'), { fatal: true, key: 'recorder.no_endpoint' });
        last = new Error(`upload ${file}: HTTP ${r.status}`);
      } catch (e) {
        if ((e as RecorderError).fatal) throw e;
        last = e;
      }
      await new Promise((res) => setTimeout(res, 500 * 2 ** attempt));
    }
    throw last;
  }
  return {
    push(blob, { replace = false } = {}) {
      const mode = replace || first ? 'replace' : 'append';
      first = false;
      chain = chain.then(() => send(blob, mode));
      chain.catch((e) => fail(e));
      return chain;
    },
    done: () => chain,
  };
}

/** Stop every recorder and every track, so nothing keeps the camera or mic on. */
function teardown() {
  const s = session;
  if (!s) return;
  clearInterval(s.tick);
  for (const rec of [s.app, s.camRec]) {
    try {
      if (rec && rec.state !== 'inactive') rec.stop();
    } catch {
      /* already stopped */
    }
  }
  s.streams.forEach((st) => st.getTracks().forEach((tr) => tr.stop()));
  session = null;
}

function fail(e: unknown) {
  if (recorder.phase === 'error') return;
  const err = e as RecorderError | null | undefined;
  recorder.error = String(err?.message ?? e);
  recorder.errorKey = err?.key ?? null;
  recorder.phase = 'error';
  escapeStops.value = true;
  teardown();
}

function now() {
  return performance.now();
}

/** Milliseconds since the app recorder started; the clock every event is on. */
function clock() {
  return session?.t0 == null ? null : Math.round(now() - session.t0);
}

export function logEvent(event: LoggedEvent) {
  if (!session || recorder.phase !== 'recording') return;
  const t = clock();
  if (t == null) return;
  // `ago`: the action began that many ms before it was logged (a drag ends
  // where it is reported). The build sorts the events by time.
  const { ago = 0, ...rest } = event;
  session.events.push({ t: t - ago, ...rest });
}

/** Keep the last state seen at the start of every slide, for a retake from there. */
export function rememberSnapshot(index: number, state: unknown) {
  try {
    const all = JSON.parse(window.localStorage.getItem(SNAPSHOT_KEY) || '{}');
    all[index] = state;
    window.localStorage.setItem(SNAPSHOT_KEY, JSON.stringify(all));
  } catch {
    /* storage full or unavailable: a retake then starts from the live state */
  }
}

export function snapshotFor(index: number): unknown {
  try {
    return JSON.parse(window.localStorage.getItem(SNAPSHOT_KEY) || '{}')[index] ?? null;
  } catch {
    return null;
  }
}

export function hasSnapshots() {
  try {
    return Object.keys(JSON.parse(window.localStorage.getItem(SNAPSHOT_KEY) || '{}')).length > 0;
  } catch {
    return false;
  }
}

/**
 * Start a take. Must run from a click: `getDisplayMedia` needs a user gesture.
 *
 * `meta` lands in meta.json as is (viewport, slide count, locale).
 */
export async function startRecording({ meta = {} }: { meta?: Record<string, unknown> } = {}): Promise<string | null | undefined> {
  if (recorder.phase === 'recording' || recorder.phase === 'starting' || recorder.phase === 'saving') return;
  // Whatever an earlier take left running (after an error) goes first.
  teardown();
  recorder.phase = 'starting';
  recorder.error = null;
  recorder.errorKey = null;
  recorder.flubs = 0;
  recorder.elapsed = 0;
  const takeId = newTakeId();
  recorder.takeId = takeId;
  const streams: MediaStream[] = [];
  try {
    if (!navigator.mediaDevices?.getDisplayMedia) {
      throw Object.assign(new Error('no getDisplayMedia'), { key: 'recorder.no_capture' });
    }
    const captureOptions: CaptureOptions = {
      video: { frameRate: 30, width: { ideal: 3840 }, height: { ideal: 2160 } },
      audio: false,
      preferCurrentTab: true,
      selfBrowserSurface: 'include',
      surfaceSwitching: 'exclude',
      monitorTypeSurfaces: 'exclude',
    };
    const display = await navigator.mediaDevices.getDisplayMedia(captureOptions);
    streams.push(display);
    // A display capture with `video` has its video track.
    const screenTrack = display.getVideoTracks()[0]!;
    // Text and thin lines: favour sharpness over motion.
    screenTrack.contentHint = 'detail';

    // The microphone chosen in the panel's test, untouched by the browser's
    // voice processing: the cleaning is done afterwards, and better.
    const mic = await navigator.mediaDevices.getUserMedia({ audio: micConstraints() });
    streams.push(mic);

    let cam: MediaStream | null = null;
    if (recorder.withCamera) {
      cam = await navigator.mediaDevices.getUserMedia({ video: camConstraints(), audio: false });
      streams.push(cam);
    }

    const appMime = pickMime(['video/webm;codecs=vp9,opus', 'video/webm;codecs=vp8,opus', 'video/webm']);
    const camMime = pickMime(['video/webm;codecs=vp9', 'video/webm;codecs=vp8', 'video/webm']);
    const app = new MediaRecorder(new MediaStream([screenTrack, ...mic.getAudioTracks()]), {
      mimeType: appMime,
      videoBitsPerSecond: 12_000_000,
      audioBitsPerSecond: 192_000,
    });
    const camRec = cam ? new MediaRecorder(cam, { mimeType: camMime, videoBitsPerSecond: 4_000_000 }) : null;

    const up = {
      app: uploader(takeId, 'app.webm'),
      cam: camRec ? uploader(takeId, 'cam.webm') : null,
      // One chain for the log too: a periodic rewrite that lands after the
      // final one would put an older log back.
      events: uploader(takeId, 'events.json'),
    };
    session = {
      takeId,
      streams,
      app,
      camRec,
      up,
      events: [],
      t0: null,
      camStart: null,
      meta: {
        takeId,
        startedAt: new Date().toISOString(),
        appMime,
        camMime: camRec ? camMime : null,
        captureSize: { width: window.innerWidth, height: window.innerHeight },
        screenSettings: screenTrack.getSettings?.() ?? null,
        camSettings: cam?.getVideoTracks()[0]?.getSettings?.() ?? null,
        micSettings: mic.getAudioTracks()[0]?.getSettings?.() ?? null,
        // Which microphone, by name: the settings only carry an opaque id.
        micLabel: mic.getAudioTracks()[0]?.label ?? null,
        ...meta,
      },
      tick: undefined,
    };

    app.ondataavailable = (e) => e.data.size && up.app.push(e.data);
    // `up.cam` exists exactly when `camRec` does.
    if (camRec) camRec.ondataavailable = (e) => e.data.size && up.cam!.push(e.data);
    // The handlers below run while this take's session is current: they
    // fire on start, before anything can end the take.
    const started = new Promise<void>((resolve) => {
      app.onstart = () => {
        session!.t0 = now();
        resolve();
      };
    });
    if (camRec) {
      camRec.onstart = () => {
        session!.camStart = now();
      };
    }
    // The user can end sharing from Chrome's own bar; treat that as stop.
    screenTrack.addEventListener('ended', () => stopRecording());

    app.start(CHUNK_MS);
    camRec?.start(CHUNK_MS);
    await started;
    recorder.phase = 'recording';
    escapeStops.value = false;
    session.tick = setInterval(() => {
      recorder.elapsed = clock() ?? 0;
      // The log is small; rewriting it every few seconds means a crash keeps it.
      flushEvents();
    }, 1000);
    return takeId;
  } catch (e) {
    streams.forEach((s) => s.getTracks().forEach((tr) => tr.stop()));
    session = null;
    fail(e);
    return null;
  }
}

let lastFlush = 0;
function flushEvents(force = false) {
  if (!session) return Promise.resolve();
  if (!force && now() - lastFlush < 5000) return Promise.resolve();
  lastFlush = now();
  const body = new Blob([JSON.stringify(session.events)], { type: 'application/json' });
  return session.up.events.push(body, { replace: true });
}

function stopped(rec: MediaRecorder | null) {
  if (!rec || rec.state === 'inactive') return Promise.resolve();
  return new Promise<void>((resolve) => {
    rec.addEventListener('stop', () => resolve(), { once: true });
    rec.stop();
  });
}

export async function stopRecording() {
  if (!session || recorder.phase !== 'recording') return;
  const s = session;
  recorder.phase = 'saving';
  escapeStops.value = true;
  s.events.push({ t: clock(), type: 'end' });
  clearInterval(s.tick);
  await Promise.all([stopped(s.app), stopped(s.camRec)]);
  s.streams.forEach((st) => st.getTracks().forEach((tr) => tr.stop()));
  const meta = {
    ...s.meta,
    endedAt: new Date().toISOString(),
    // The phase was 'recording', which `app.onstart` set `t0` for.
    durationMs: Math.round(now() - s.t0!),
    // Where the camera started relative to the app clock. Post-processing
    // shifts the webcam by this much so the two line up.
    camOffsetMs: s.camStart == null ? null : Math.round(s.camStart - s.t0!),
  };
  try {
    await Promise.all([s.up.app.done(), s.up.cam?.done()]);
    await flushEvents(true);
    await uploader(s.takeId, 'meta.json').push(new Blob([JSON.stringify(meta, null, 2)], { type: 'application/json' }), { replace: true });
    recorder.phase = 'done';
  } catch (e) {
    fail(e);
  } finally {
    session = null;
  }
}

export function markFlub() {
  if (recorder.phase !== 'recording') return;
  recorder.flubs += 1;
  logEvent({ type: 'flub' });
}
