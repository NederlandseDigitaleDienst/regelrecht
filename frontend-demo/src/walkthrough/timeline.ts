/**
 * The walkthrough's timeline, and the pure functions the player runs on it.
 *
 * `timeline.json` is written by post-processing (script/walkthrough/) and
 * copied into `/data/walkthrough/` with the rest of the demo corpus. The media
 * it names live under `/walkthrough/` and are not in git: the Docker build
 * fetches them from a GitHub release, pinned by checksum
 * (scripts/fetch-walkthrough-media.mjs).
 *
 * Shape, in seconds:
 *
 *   { version: 1, presenter: { name, role },
 *     main: TRACK,
 *     faq: [{ id, question, offer, ...TRACK }] }
 *
 *   TRACK = { duration, recordedAt, viewport,
 *             audio: { src },                    // the voice: the replay's clock
 *             video: { src, width, height },     // the window, for a phone and the MP4
 *             cam: { src } | null,               // the presenter bubble
 *             captions: { nl: 'file.vtt' },
 *             slides: [...],                     // the deck as recorded
 *             chapters: [{ start, end, slideIndex, slide, profile, state }],
 *             events: [{ t, type, ... }] }       // what replay.ts does again
 *
 * Everything here is free of the DOM, so the player's decisions can be tested
 * without a browser.
 */

import type { SlideConfig } from '../data/loadCorpus';
import type { DemoState } from '../store/demoStore';
import type { DescriptionStep, ScrollAnchor } from './locator';

/** A media file of the recording, served from `MEDIA_BASE`. */
export interface MediaFile {
  src: string;
  sha256?: string;
  bytes?: number;
}

/** The recorded window, for a phone and the MP4. */
export interface VideoMedia extends MediaFile {
  width: number;
  height: number;
}

/** The presenter bubble; the webcam covers the recorded opening up to `until`. */
export interface CamMedia extends MediaFile {
  until?: number | null;
}

/** One of the presenter's short lines (`bumpers`), with its length in seconds. */
export interface BumperClip extends MediaFile {
  duration?: number;
}

/** The window size the track was recorded in. */
export interface RecordedViewport {
  width: number;
  height: number;
  dpr?: number;
}

/**
 * The demo's state as the store held it when a chapter's slide came up: a
 * snapshot of `useDemo().state`. Partial: a take recorded before a field
 * existed lacks it.
 */
export type RecordedState = Partial<DemoState>;

export interface Chapter {
  start: number;
  end: number;
  slideIndex: number;
  slide?: SlideConfig | null;
  profile?: string | null;
  /** Absent on a chapter added after recording (`just walkthrough states` fills it). */
  state?: RecordedState | null;
}

/** A moment the player stops and hands the demo to the viewer. */
export interface Pause {
  t: number;
  hint?: string | null;
}

/** Where a stretch of a take landed in the track. */
export interface TrackPiece {
  take: string;
  from: number;
  to: number;
  at: number;
}

/** What a track carries translated into one language. */
export interface TrackTranslation {
  slides?: SlideConfig[];
  /** Only on a FAQ entry. */
  question?: string;
}

interface EventBase {
  /** Seconds into the track. */
  t: number;
}

/** The demo put back in a state (a "Demo resetten", the start of a take). */
export interface RecordedRestoreEvent extends EventBase {
  type: 'restore';
  state?: RecordedState | null;
  slideIndex?: number | null;
}

export interface RecordedSlideEvent extends EventBase {
  type: 'slide';
  index: number;
}

export interface RecordedRouteEvent extends EventBase {
  type: 'route';
  path: string;
}

export interface RecordedClickEvent extends EventBase {
  type: 'click';
  target?: DescriptionStep[];
  /** Where in the element, as fractions. */
  fx?: number;
  fy?: number;
}

/** Typing into a field; `values` holds the text per language (walkthrough.<code>.yaml). */
export interface RecordedInputEvent extends EventBase {
  type: 'input';
  target?: DescriptionStep[];
  value?: string;
  values?: Record<string, string>;
}

export interface RecordedChangeEvent extends EventBase {
  type: 'change';
  target?: DescriptionStep[];
  value?: string;
  values?: Record<string, string>;
  checked?: boolean;
}

export interface RecordedKeyEvent extends EventBase {
  type: 'key';
  key: string;
  target?: DescriptionStep[];
}

/** The graph's viewport: its centre and zoom. */
export interface RecordedViewportEvent extends EventBase {
  type: 'viewport';
  cx: number;
  cy: number;
  zoom: number;
}

/** A graph node dragged by `dx`, `dy` over `ms` milliseconds. */
export interface RecordedNodeEvent extends EventBase {
  type: 'node';
  id: string;
  dx?: number;
  dy?: number;
  ms?: number;
}

export interface RecordedScrollEvent extends EventBase {
  type: 'scroll';
  /** The scroller; absent for the page itself. */
  target?: DescriptionStep[];
  top?: number;
  left?: number;
  /** How far the scroller could scroll when recorded. */
  max?: number;
  anchor?: ScrollAnchor | null;
  src?: string;
}

/** What replay.ts does again, at time `t`. */
export type TimelineEvent =
  | RecordedRestoreEvent
  | RecordedSlideEvent
  | RecordedRouteEvent
  | RecordedClickEvent
  | RecordedInputEvent
  | RecordedChangeEvent
  | RecordedKeyEvent
  | RecordedViewportEvent
  | RecordedNodeEvent
  | RecordedScrollEvent;

/** One track: the main line or the answer to a question. Times in seconds. */
export interface Track {
  duration: number;
  /** ISO timestamp of the recording; the demo's clock during the replay. */
  recordedAt?: string;
  viewport?: RecordedViewport | null;
  /** The voice: the replay's clock. */
  audio: MediaFile;
  video?: VideoMedia | null;
  cam?: CamMedia | null;
  generatedVoice?: boolean;
  /** locale code -> caption file under `CAPTIONS_BASE`. */
  captions?: Record<string, string>;
  /** The deck as recorded. */
  slides?: SlideConfig[];
  chapters: Chapter[];
  events: TimelineEvent[];
  pauses?: Pause[];
  pieces?: TrackPiece[];
  /** locale code -> translations. */
  i18n?: Record<string, TrackTranslation>;
  /** locale code -> the track spoken in that language (`walkthrough dub`). */
  dub?: Record<string, TrackDub>;
}

/**
 * A track in another language, in the presenter's cloned voice. Exactly as
 * long as the recorded voice, with every sentence in the slot of the Dutch
 * one, so the events keep their times.
 */
export interface TrackDub {
  audio: MediaFile;
  /** What this voice says, as captions, under `CAPTIONS_BASE`. */
  captions: string;
  /** The Dutch sentences it was made against (dub.py); for the pipeline. */
  basis?: string;
}

/** A frequently asked question with the track that answers it. */
export interface FaqEntry extends Track {
  id: string;
  question: string;
  /** When the question appears in the main line, in seconds. */
  offer?: number;
}

/** `timeline.json`. */
export interface Timeline {
  version: 1;
  release?: string;
  presenter?: { name: string; role: string };
  main: Track;
  faq?: FaqEntry[];
  /** kind ('wait', 'resume') -> the lines to pick from. */
  bumpers?: Record<string, BumperClip[]>;
  /** The languages the walkthrough is translated into. */
  locales?: string[];
  /** locale code -> what the cloned voice says outside the tracks. */
  dub?: Record<string, { bumpers?: Record<string, BumperClip[]> }>;
}

/**
 * The cloned voice of `track` in `locale`, or null: when there is none, or
 * when the viewer chose the presenter's own voice (`original`).
 */
export function dubOf(track: Pick<Track, 'dub'> | null | undefined, locale: string, original = false): TrackDub | null {
  return original ? null : (track?.dub?.[locale] ?? null);
}

/** A caption cue from a WebVTT file. */
export interface Cue {
  start: number;
  end: number;
  text: string;
}

/** A box in pixels. */
export interface FitRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** What a follower should do: always a rate, and a jump when it drifted far. */
export interface FollowerCorrection {
  seek?: number;
  rate: number;
}

/** What `loadTimeline` needs from `fetch`. */
export type FetchLike = (url: string) => Promise<{ ok: boolean; json(): Promise<unknown> }>;

/** The part of a track the chapter functions read. */
interface ChapterStarts {
  chapters?: readonly { start: number }[] | null;
}

export const TIMELINE_URL = '/data/walkthrough/timeline.json';
export const MEDIA_BASE = '/walkthrough/';
export const CAPTIONS_BASE = '/data/walkthrough/';

/** The timeline, or null when this build carries no recording. */
export async function loadTimeline(fetchImpl: FetchLike = fetch): Promise<Timeline | null> {
  try {
    const res = await fetchImpl(TIMELINE_URL);
    if (!res.ok) return null;
    const data = await res.json();
    return validTimeline(data) ? data : null;
  } catch {
    return null;
  }
}

/** The fields of an unchecked track that `validTrack` looks at. */
interface UncheckedTrack {
  duration?: unknown;
  audio?: { src?: unknown } | null;
  events?: unknown;
  chapters?: unknown;
}

// The guard checks the fields replay depends on before it starts (duration,
// audio, events and at least one chapter), not every event and chapter: the
// timeline is written by our own tooling, and the narrowing trusts it beyond that.
export function validTrack(value: unknown): value is Track {
  const track = value as UncheckedTrack | null | undefined;
  return (
    !!track &&
    Number.isFinite(track.duration) &&
    typeof track.audio?.src === 'string' &&
    Array.isArray(track.events) &&
    Array.isArray(track.chapters) &&
    track.chapters.length > 0
  );
}

export function validTimeline(value: unknown): value is Timeline {
  const data = value as { version?: unknown; main?: unknown; faq?: { id?: unknown }[] | null } | null | undefined;
  return !!data && data.version === 1 && validTrack(data.main) && (data.faq ?? []).every((f) => typeof f.id === 'string' && validTrack(f));
}

/** Index of the chapter that plays at `t`; the last one past the end. */
export function chapterAt(track: ChapterStarts | null | undefined, t: number): number {
  const chapters = track?.chapters ?? [];
  if (!chapters.length) return -1;
  for (let i = chapters.length - 1; i >= 0; i -= 1) {
    if (t >= chapters[i]!.start) return i;
  }
  return 0;
}

/**
 * Where "previous chapter" goes. Within the first seconds of a chapter it is
 * the one before; later it is the start of the current one, the way a music
 * player's back button works. Otherwise a single press would only ever restart.
 */
export function previousChapterStart(track: ChapterStarts | null | undefined, t: number, grace = 3): number {
  const i = chapterAt(track, t);
  if (i < 0) return 0;
  // chapterAt found chapter i, so the track and its chapters exist.
  const chapters = track?.chapters ?? [];
  const start = chapters[i]!.start;
  if (t - start > grace || i === 0) return start;
  return chapters[i - 1]!.start;
}

export function nextChapterStart(track: ChapterStarts | null | undefined, t: number): number | null {
  const i = chapterAt(track, t);
  const next = track?.chapters?.[i + 1];
  return next ? next.start : null;
}

/** The FAQ entries offered by time `t`, in the order they appeared. */
export function offeredFaq<F extends { offer?: number }>(
  timeline: { faq?: readonly F[] | null } | null | undefined,
  t: number,
): (F & { offer: number })[] {
  return (timeline?.faq ?? [])
    .filter((f): f is F & { offer: number } => typeof f.offer === 'number' && Number.isFinite(f.offer) && f.offer <= t)
    .sort((a, b) => a.offer - b.offer);
}

/** Clicks whose ripple is still visible at `t`. */
export function ripplesAt<C extends { t: number }>(clicks: readonly C[] | null | undefined, t: number, length = 0.7): C[] {
  return (clicks ?? []).filter((c) => c.t <= t && t - c.t < length);
}

/**
 * The box the video's picture actually occupies inside its element, with
 * `object-fit: contain`. A click is stored as a fraction of the picture, so its
 * ripple has to land on the picture and not on the letterbox around it.
 */
export function fitRect(boxW: number, boxH: number, mediaW: number, mediaH: number): FitRect {
  if (!(boxW > 0 && boxH > 0 && mediaW > 0 && mediaH > 0)) return { left: 0, top: 0, width: boxW || 0, height: boxH || 0 };
  const scale = Math.min(boxW / mediaW, boxH / mediaH);
  const width = mediaW * scale;
  const height = mediaH * scale;
  return { left: (boxW - width) / 2, top: (boxH - height) / 2, width, height };
}

/**
 * How a follower (the webcam) should catch up with the master (the app
 * video). A small drift is nudged with the playback rate, which is invisible;
 * a large one (after a seek, a stall) is a jump.
 */
export function followerCorrection(masterT: number, followerT: number, baseRate = 1): FollowerCorrection {
  const drift = followerT - masterT;
  if (Math.abs(drift) > 0.3) return { seek: masterT, rate: baseRate };
  if (Math.abs(drift) > 0.04) return { rate: baseRate * (drift > 0 ? 0.95 : 1.05) };
  return { rate: baseRate };
}

function vttTime(s: string): number {
  const m = /^(?:(\d+):)?(\d{1,2}):(\d{2})[.,](\d{3})$/.exec(s.trim());
  if (!m) return NaN;
  return Number(m[1] ?? 0) * 3600 + Number(m[2]) * 60 + Number(m[3]) + Number(m[4]) / 1000;
}

/**
 * The cues of a WebVTT file. Only what post-processing writes: a header, then
 * blocks of an optional id, a timing line and text. Settings after the timing
 * are ignored, and so are NOTE and STYLE blocks.
 */
export function parseVtt(text: unknown): Cue[] {
  const cues: Cue[] = [];
  const blocks = String(text ?? '')
    .replace(/\r\n?/g, '\n')
    .split(/\n{2,}/);
  for (const block of blocks) {
    const lines = block.split('\n').filter((l) => l.length);
    const timing = lines.findIndex((l) => l.includes('-->'));
    if (timing < 0) continue;
    // The timing line contains '-->' (findIndex above), so it splits in at least two.
    const [a, b] = lines[timing]!.split('-->') as [string, string];
    const start = vttTime(a);
    // split() always returns at least one part.
    const end = vttTime(b.trim().split(/\s+/)[0]!);
    if (!Number.isFinite(start) || !Number.isFinite(end)) continue;
    cues.push({ start, end, text: lines.slice(timing + 1).join('\n') });
  }
  return cues;
}

export function cueAt<C extends Cue>(cues: readonly C[] | null | undefined, t: number): C | null {
  return (cues ?? []).find((c) => t >= c.start && t < c.end) ?? null;
}

/** `m:ss`, or `h:mm:ss` from an hour. */
export function formatTime(seconds: number | null | undefined): string {
  const s = Math.max(0, Math.floor(seconds || 0));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const r = String(s % 60).padStart(2, '0');
  return h ? `${h}:${String(m).padStart(2, '0')}:${r}` : `${m}:${r}`;
}

/**
 * Whether the presenter's bubble shows at `t`. The webcam covers the recorded
 * opening (`cam.until`); where the generated voice takes over there is no
 * picture of the presenter, so the bubble goes.
 */
export function camVisible(track: { cam?: { until?: number | null } | null } | null | undefined, t: number): boolean {
  return !!track?.cam && (track.cam.until == null || t < track.cam.until);
}

/** The most recent `profile` at or before chapter `i`, as the deck does it. */
export function profileAt(
  track: { chapters?: readonly { profile?: string | null; slide?: Partial<SlideConfig> | null }[] | null } | null | undefined,
  i: number,
): string | null {
  for (let j = i; j >= 0; j -= 1) {
    const p = track?.chapters?.[j]?.profile ?? track?.chapters?.[j]?.slide?.profile;
    if (p) return p;
  }
  return null;
}

/**
 * The pause the clock just reached, or null: one with `prev < t <= now`.
 * Only for a clock that ran (a tick of a few frames); a jump (a seek, a
 * chapter) passes pauses without stopping at them.
 */
export function pauseDue<P extends { t: number }>(pauses: readonly P[] | null | undefined, prev: number, now: number, maxStep = 1): P | null {
  if (!(now > prev) || now - prev > maxStep) return null;
  return (pauses ?? []).find((p) => p.t > prev && p.t <= now) ?? null;
}

/** One of `clips` at random, or null. */
export function pickClip<C>(clips: readonly C[] | null | undefined, random: () => number = Math.random): C | null {
  // The modulo keeps the index within the list for a random() in [0, 1).
  return clips?.length ? clips[Math.floor(random() * clips.length) % clips.length]! : null;
}
