/**
 * The replay engine: the presenter's voice plays, and at the moments the
 * presenter acted the engine does the same in the live demo.
 *
 * The voice is the clock. Every event in the timeline has a time on it; when
 * the audio reaches that time the event is applied: a slide change through
 * the deck (which opens the tab and the persona, as in a presentation), a
 * click or typing through `locator` and `actions`. An event whose element is
 * not there yet (a tab still loading, the engine still computing) waits for
 * it, and the voice waits along.
 *
 * Jumping does not replay from the start. Each chapter carries the demo state
 * as it was when its slide came up; a jump restores that, mounts the tabs
 * fresh and replays the chapter's events up to the target, fast.
 *
 * The viewer can click along: a trusted click in the demo pauses the replay
 * and leaves the demo to them. Play again and the state of that moment comes
 * back. While it runs, the viewer's own state is set aside and not written
 * to storage; leaving restores it.
 */
import { nextTick, reactive, watch } from 'vue';
import type { WatchStopHandle } from 'vue';
import type { Router } from 'vue-router';
import { adoptLocale, currentLocale, localeDef, setViewerLocale, t } from '../i18n/index.js';
import scenarioTitles from '../i18n/scenarioTitles.generated.js';
import { humanize } from '../data/format';
import { loadCorpus } from '../data/loadCorpus';
import { stepTemplatePairs } from '../data/gherkinNl';
import { emphasiseArguments } from '../data/scenarioSteps';
import { setPersistence } from '../store/demoStore';
import type { DemoStore } from '../store/demoStore';
import type { Presentation } from '../presentation/usePresentation';
import type { SlideConfig } from '../data/loadCorpus';
import { localeRouteName, pageForConfigPath, splitConfigPath } from '../router';
import { click, setChecked, setValue, key as pressKey, scrollTo } from './actions';
import { installClock, uninstallClock } from './clock';
import { resolve, scrollAnchor, scrollTarget } from './locator';
import { makeTranslator, parallelPairs } from './translateText';
import { graphView, moveNode, showView } from './graphBridge';
import { CAPTIONS_BASE, MEDIA_BASE, chapterAt, dubOf, parseVtt, pauseDue, pickClip } from './timeline';
import type { Chapter, RecordedChangeEvent, Cue, FaqEntry, RecordedInputEvent, RecordedState, RecordedScrollEvent, Timeline, TimelineEvent, Track } from './timeline';
import type { DescriptionStep } from './locator';
import type { TextPair, Translator } from './translateText';
import { resetViews } from './viewEpoch';
import type { NowSource } from './clock';

const SOURCE_LOCALE = 'nl';
const POSITION_KEY = 'rr-walkthrough-position-v2';
/** 'original' when the viewer chose the presenter's own voice over a dub. */
const VOICE_KEY = 'rr-walkthrough-voice';

/** What the replay drives: the router, the store (`useDemo()`) and the deck (`usePresentation()`). */
export interface ReplayContext {
  router: Router;
  demo: DemoStore;
  presentation: Presentation;
}

/** A ripple where the replay clicked, in viewport pixels. */
export interface ReplayRipple {
  id: string;
  x: number;
  y: number;
}

/** An action the replay could not do as recorded (`misses`, `loose`). */
export interface ReplayNote {
  t: number;
  type: string;
  /** What names the element: its text, label, tag, or the node id (values of a `DescriptionStep`). */
  what: string | number | true;
}

/** The player's state, for the overlay and the controls. */
export interface ReplayState {
  active: boolean;
  playing: boolean;
  now: number;
  speed: number;
  captionsOn: boolean;
  camOn: boolean;
  originalVoice: boolean;
  dubbed: boolean;
  waiting: boolean;
  diverged: boolean;
  restoring: boolean;
  faq: FaqEntry | null;
  cues: Cue[];
  captionsLang: string;
  demoLocale: string;
  cursor: { x: number; y: number; visible: boolean };
  ripples: ReplayRipple[];
  misses: ReplayNote[];
  loose: ReplayNote[];
  failed: boolean;
  invite: 'pause' | 'wait' | null;
  inviteHint: string | null;
}

/** What an action carries that `noteMiss` and `noteLoose` report. */
interface NotedAction {
  t: number;
  type: string;
  target?: DescriptionStep[];
  id?: string;
}

export const replay = reactive<ReplayState>({
  active: false,
  playing: false,
  now: 0,
  speed: 1,
  captionsOn: true,
  camOn: true,
  /** The viewer chose the presenter's own (Dutch) voice over the dub. */
  originalVoice: false,
  /**
   * The voice playing is the cloned one in the viewer's language
   * (`track.dub`). The presenter's bubble goes then: its lips move in Dutch.
   */
  dubbed: false,
  waiting: false,
  /** The viewer clicked along; play restores the recorded state first. */
  diverged: false,
  /** A jump is putting the demo back; the replay's fast catch-up is covered. */
  restoring: false,
  faq: null,
  cues: [],
  /** The language of the captions on screen: the viewer's, or Dutch when there are none in it. */
  captionsLang: SOURCE_LOCALE,
  /** The language the demo runs in during the replay. */
  demoLocale: SOURCE_LOCALE,
  cursor: { x: 0, y: 0, visible: false },
  ripples: [],
  /** Actions whose element was not found: what a recording needs redone. */
  misses: [],
  /** Actions found only by a looser match: worth a look after a demo change. */
  loose: [],
  /** The media did not load: the page says so instead of spinning. */
  failed: false,
  /**
   * Why the player waits for the viewer, with a hint at the play button:
   * 'pause' at a moment the presenter invites them to look around, 'wait'
   * when they took over themselves. Null while it plays.
   */
  invite: null,
  /** The pause's own hint, from walkthrough.yaml; else the standard text. */
  inviteHint: null,
});

let timeline: Timeline | null = null;
// Set by begin() before anything else in this module runs against the demo.
let ctx: ReplayContext | null = null; // { router, demo, presentation }
let audio: HTMLAudioElement | null = null;
let next = 0; // index of the next event to apply
/** The seek token of whoever is applying events now (pump or seek), or null. */
let busyOwner: number | null = null;
/** The seek token of a seek in progress, or null. */
let seeking: number | null = null;
let raf = 0;
let mainPosition = 0;
let backup: RecordedState | null = null;
let previousLocale: string | null = null;
/**
 * The language the demo runs in during the replay: the viewer's, when the
 * walkthrough is translated into it (`timeline.locales`), else Dutch, as
 * recorded.
 */
let playLocale = SOURCE_LOCALE;
/** Recorded texts in `playLocale` (translateText.ts); null while that is Dutch. */
let translate: Translator | null = null;
let unwatchMode: WatchStopHandle | null = null;
let seekToken = 0;

export function currentTrack(): Track | null {
  return replay.faq ?? timeline?.main ?? null;
}

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));
const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v));

// ---- the demo's language --------------------------------------------------------------

/** What the presenter typed, in the language the demo runs in (walkthrough.<code>.yaml). */
const valueOf = (e: RecordedInputEvent | RecordedChangeEvent) => e.values?.[playLocale] ?? e.value;

/**
 * The translator for the language the demo runs in, from every source the
 * demo takes its texts from: the interface dictionaries, the corpus with its
 * overlays, the scenario titles and steps, and the field names.
 */
async function translatorFor(code: string): Promise<Translator | null> {
  if (code === SOURCE_LOCALE) return null;
  const corpus = await loadCorpus();
  const tr: Translator = makeTranslator({
    // localeDef never returns undefined (an unknown code gives Dutch); its
    // type, inferred from the Map in i18n/index.js, says it might.
    dicts: { source: localeDef(SOURCE_LOCALE)!.dict, local: localeDef(code)!.dict },
    pairs: [
      ...parallelPairs(corpus?.configByLocale?.[SOURCE_LOCALE], corpus?.configByLocale?.[code]),
      ...parallelPairs(corpus?.profilesByLocale?.[SOURCE_LOCALE], corpus?.profilesByLocale?.[code]),
      // The generated module's inferred type has one key per language it carries.
      ...Object.entries((scenarioTitles as Record<string, Record<string, string>>)[code] ?? {}),
    ],
    // A scenario's steps, as its text cells show them.
    // map() keeps the pair a pair.
    templates: stepTemplatePairs(SOURCE_LOCALE, code).map((pair) => pair.map(emphasiseArguments) as TextPair),
    humanize: (name) => humanize(name, { locale: code, report: false }),
  });
  tr.path = localPath;
  return tr;
}

/** A recorded (Dutch) path in the language the demo runs in. */
function localPath(path: string): string {
  if (playLocale === SOURCE_LOCALE) return path;
  const parts = splitConfigPath(path);
  if (!parts) return path;
  // Only the translator calls this, and that exists only during a replay.
  const base = ctx!.router.resolve({ name: localeRouteName(parts.page, playLocale) }).path;
  return parts.rest ? `${base.replace(/\/$/, '')}${parts.rest}` : base;
}

/** The viewer's language, which the slides, captions and questions follow. */
function viewerLocale(): string {
  return previousLocale ?? currentLocale();
}

/** A question in the viewer's language, when it has a translation. */
export function questionOf(faq: FaqEntry | null | undefined): string {
  return faq?.i18n?.[viewerLocale()]?.question ?? faq?.question ?? '';
}

/** The language `questionOf` gives the question in. */
export function questionLang(faq: FaqEntry | null | undefined): string {
  return faq?.i18n?.[viewerLocale()]?.question ? viewerLocale() : SOURCE_LOCALE;
}

/** The slide a chapter opens with, in the viewer's language when there is a translation. */
export function chapterSlide(track: Track | null | undefined, chapter: Chapter | null | undefined): SlideConfig | null {
  // Without a chapter the index is undefined and finds no slide, as before.
  return track?.i18n?.[viewerLocale()]?.slides?.[chapter?.slideIndex as number] ?? chapter?.slide ?? null;
}

function recordedAt(track: Track | null | undefined): number | null {
  const t = Date.parse(track?.recordedAt ?? '');
  return Number.isFinite(t) ? t : null;
}

// ---- the demo's state -----------------------------------------------------------

function applyState(state: RecordedState | null | undefined): void {
  if (!state || !ctx) return;
  const { demo } = ctx;
  Object.assign(demo.state, clone(state));
  // The replay always shows the rail, whatever mode the presenter was in.
  demo.state.presentationMode = 'zelfstandig';
  demo.reregister();
}

/** The deck's slides as recorded, with corrections applied by the pipeline. */
function slidesOf(track: Track | null | undefined): SlideConfig[] {
  const slides = track?.i18n?.[viewerLocale()]?.slides ?? track?.slides;
  return slides?.length ? slides : ctx?.demo?.corpus?.value?.config?.slides ?? [];
}

/**
 * The slides a track shows. In an answer every slide carries the question
 * as its title, whatever slide the answer was recorded on: the viewer chose
 * that question and should keep seeing it. Route and persona stay, so the
 * deck still opens the tab the answer talks about.
 */
function slidesFor(track: (Track & { id?: string }) | null | undefined): SlideConfig[] {
  const slides = slidesOf(track);
  if (!replay.faq || track?.id !== replay.faq.id) return slides;
  return slides.map((s) => ({
    ...s,
    kind: s.route ? s.kind : 'section',
    overline: t('walkthrough.faq.overline'),
    title: questionOf(replay.faq),
    lead: undefined,
    lines: undefined,
    bullets: undefined,
    note: undefined,
  }));
}

function samePage(path: string): boolean {
  const want = pageForConfigPath(path) ?? path;
  // Only apply() calls this, during a replay.
  const here = ctx!.router.currentRoute.value;
  return (here.meta?.page ?? here.path) === want || here.path === path;
}

// ---- finding and doing --------------------------------------------------------

/**
 * Whether work started under `token` may still touch the demo: not after a
 * jump (a newer token), not after leaving, not once the viewer took over.
 */
function current(token: number): boolean {
  return token === seekToken && replay.active && !replay.diverged;
}

/**
 * The element of `target`, waiting for it to appear. If the wait gets long
 * the voice stops too, so the picture never runs behind the words. Only when
 * the exact element does not show up is a looser match tried: trying it at
 * once would click a look-alike while the real one is still rendering.
 */
async function find(
  target: DescriptionStep[] | undefined,
  { timeout = 4000, fast = false, token = seekToken, event = null }: { timeout?: number; fast?: boolean; token?: number; event?: NotedAction | null } = {},
): Promise<Element | null> {
  const started = performance.now();
  // Fast, the demo still needs its time: the engine computes after an answer
  // before the next field is there. Waiting costs only when nothing comes.
  const limit = fast ? Math.min(timeout, 3000) : timeout;
  let paused = false;
  for (;;) {
    if (!current(token)) {
      if (paused) replay.waiting = false;
      return null;
    }
    const waited = performance.now() - started;
    const exact = resolve(target, { translate });
    const el = exact ?? (waited > limit / 2 ? resolve(target, { loose: true, translate }) : null);
    if (el) {
      if (!exact && event) noteLoose(event);
      if (paused) {
        replay.waiting = false;
        if (replay.playing) audio?.play().catch(() => {});
      }
      return el;
    }
    if (waited > limit) break;
    if (!fast && !paused && waited > 250 && replay.playing) {
      paused = true;
      replay.waiting = true;
      audio?.pause();
    }
    await sleep(60);
  }
  if (paused) {
    replay.waiting = false;
    if (replay.playing) audio?.play().catch(() => {});
  }
  return null;
}

/**
 * Remember an action whose element was not found. The replay goes on (the
 * route still gets the viewer to the right tab), but this is the list of
 * what to record again after a demo change; the verify run reports it.
 */
function noteMiss(e: NotedAction): void {
  const last: Partial<DescriptionStep> = e.target?.at?.(-1) ?? {};
  replay.misses.push({ t: e.t, type: e.type, what: last.text ?? last['@aria-label'] ?? last.textContent ?? last.tag ?? e.id ?? '?' });
  if (import.meta.env.DEV) console.warn('[walkthrough] niet gevonden:', e.type, last);
}

/**
 * Remember an action that found its element only with the numbers in its
 * description masked ("Wetten, 79" became "Wetten, 81"). Usually harmless, a
 * count that moved; but it can also be a different element that now carries
 * those words, and then the recording means something else than it did.
 * The verify run lists these as warnings, for a person to look at.
 */
function noteLoose(e: NotedAction): void {
  const last: Partial<DescriptionStep> = e.target?.at?.(-1) ?? {};
  replay.loose.push({ t: e.t, type: e.type, what: last.text ?? last['@aria-label'] ?? last.textContent ?? last.tag ?? e.id ?? '?' });
}

function pointIn(el: Element, fx = 0.5, fy = 0.5): { x: number; y: number } {
  const r = el.getBoundingClientRect();
  return { x: r.left + r.width * fx, y: r.top + r.height * fy };
}

function showCursorAt(p: { x: number; y: number }): void {
  replay.cursor.x = p.x;
  replay.cursor.y = p.y;
  replay.cursor.visible = true;
}

function ripple(p: { x: number; y: number }): void {
  const id = `${performance.now()}`;
  replay.ripples.push({ id, ...p });
  setTimeout(() => {
    const i = replay.ripples.findIndex((r) => r.id === id);
    if (i >= 0) replay.ripples.splice(i, 1);
  }, 700);
}

/** A control that does not take a click now, itself or the component it is in. */
function disabled(el: Element): boolean {
  const host = (el.getRootNode?.() as Partial<ShadowRoot> | undefined)?.host;
  return [el, host].some((n) => n && ((n as { disabled?: unknown }).disabled === true || n.getAttribute?.('aria-disabled') === 'true'));
}

async function apply(e: TimelineEvent, { fast = false, token = seekToken }: { fast?: boolean; token?: number } = {}): Promise<void> {
  // Only pump() and seek() apply events, during a replay.
  const { router, presentation } = ctx!;
  const opts = { fast, token };
  switch (e.type) {
    case 'restore':
      applyState(e.state);
      resetViews();
      await nextTick();
      if (e.slideIndex != null) await presentation.goTo(e.slideIndex);
      return;
    case 'slide':
      if (presentation.index.value !== e.index || !presentation.active.value) {
        await presentation.goTo(e.index);
        // The slide opens its tab on its own time; a click applied right
        // behind it (fast) would land before the tab is there.
        if (fast) await sleep(200);
      }
      return;
    case 'route': {
      // Routes follow from clicks. Only when a click did not get there (a
      // control that moved, a tab that is not there) does the replay go
      // itself, so the story stays on the tab the voice talks about.
      const deadline = performance.now() + (fast ? 300 : 1500);
      while (performance.now() < deadline && !samePage(e.path)) {
        await sleep(60);
        if (!current(token)) return;
      }
      if (!samePage(e.path) && current(token)) await router.push(localPath(e.path)).catch(() => {});
      return;
    }
    case 'click': {
      // A link to a tab that is not on screen: next to the deck the tab bar
      // has no room and is hidden, while the presenter may have recorded
      // with the deck off and clicked it. Go where the link goes.
      const href = (e.target ?? []).map((step) => step.href).find((h): h is string => typeof h === 'string' && h.startsWith('/'));
      if (href) {
        const link = resolve(e.target, { translate });
        const box = link?.getBoundingClientRect?.();
        if (!box || !box.width || !box.height) {
          if (!samePage(href)) await router.push(localPath(href)).catch(() => {});
          return;
        }
      }
      const el = await find(e.target, { ...opts, event: e });
      if (!el && current(token)) noteMiss(e);
      if (!el || !current(token)) return;
      el.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
      if (!fast) {
        const p = pointIn(el, e.fx, e.fy);
        showCursorAt(p);
        ripple(p);
      }
      // Fast, the clicks come faster than the demo answers: a button that
      // waits for the engine (submit, after the calculation) is still
      // disabled, and a click on it would be lost with everything after it.
      if (fast) {
        const until = performance.now() + 2000;
        while (disabled(el) && performance.now() < until && current(token)) await sleep(40);
      }
      click(el, e);
      // And let the demo take the click in before the next one.
      if (fast) await sleep(150);
      return;
    }
    case 'input': {
      const el = await find(e.target, { ...opts, event: e });
      if (!el && current(token)) noteMiss(e);
      if (!el || !current(token)) return;
      if (!fast) showCursorAt(pointIn(el, 0.15, 0.5));
      setValue(el, valueOf(e) ?? '');
      return;
    }
    case 'change': {
      const el = await find(e.target, { ...opts, event: e });
      if (!el && current(token)) noteMiss(e);
      if (!el || !current(token)) return;
      if (typeof e.checked === 'boolean') setChecked(el, e.checked);
      else setValue(el, valueOf(e) ?? '', { change: true });
      return;
    }
    case 'key': {
      const el = e.target ? await find(e.target, { ...opts, timeout: 1000, event: e }) : null;
      if (!current(token)) return;
      if (el) pressKey(el, e.key);
      else document.dispatchEvent(new KeyboardEvent('keydown', { key: e.key, bubbles: true }));
      return;
    }
    case 'viewport': {
      // The graph may still be mounting after the tab opened; wait briefly.
      const deadline = performance.now() + (fast ? 800 : 1500);
      while (!graphView.api && performance.now() < deadline) {
        await sleep(60);
        if (!current(token)) return;
      }
      // A short glide between the samples (ten a second) looks like the
      // presenter's own drag; a jump just lands.
      if (current(token)) showView(e, fast ? 0 : 140);
      return;
    }
    case 'node': {
      const deadline = performance.now() + (fast ? 800 : 1500);
      while (!graphView.api && performance.now() < deadline) {
        await sleep(60);
        if (!current(token)) return;
      }
      if (!current(token)) return;
      // The drag at its own pace, capped: a slow drag still lands while the voice is on it.
      if (!moveNode(e.id, e.dx ?? 0, e.dy ?? 0, fast ? 0 : Math.min(e.ms ?? 400, 1200))) noteMiss(e);
      return;
    }
    case 'scroll': {
      const el = e.target ? await find(e.target, { ...opts, timeout: 1000, event: e }) : document.scrollingElement;
      if (!el || !current(token)) return;
      scrollTo(el, scrollTarget(el, e, translate), e.left ?? 0);
      // For `walkthrough anchors`: what an older take lacks, measured in the
      // layout it was recorded in. Dev only.
      if (import.meta.env.DEV) window.__rrReplay?.afterScroll?.(e, el);
      return;
    }
    default:
  }
}

// ---- the clock ---------------------------------------------------------------------

/**
 * Move the cursor toward the next click a moment before it happens, so the
 * viewer sees where it goes instead of a ripple out of nowhere.
 */
let leadFor = -1;
function lead(events: TimelineEvent[], now: number): void {
  const e = events[next];
  if (!e || e.type !== 'click' || leadFor === next || e.t - now > 0.45) return;
  leadFor = next;
  const el = resolve(e.target, { translate });
  if (el) showCursorAt(pointIn(el, e.fx, e.fy));
}

async function pump() {
  if (busyOwner !== null || replay.diverged) return;
  const track = currentTrack();
  const events = track?.events ?? [];
  const token = seekToken;
  busyOwner = token;
  try {
    // `next < events.length` is checked first, so events[next] exists.
    while (next < events.length && events[next]!.t <= replay.now) {
      await apply(events[next]!, { token });
      if (!current(token)) return; // a jump, a stop or the viewer meanwhile
      next += 1;
    }
    lead(events, replay.now);
  } finally {
    if (busyOwner === token) busyOwner = null;
  }
}

let lastTick = 0;
function tick(): void {
  if (audio && seeking === null) {
    const prev = lastTick;
    replay.now = audio.currentTime;
    lastTick = replay.now;
    // A moment the presenter hands the demo to the viewer: stop there.
    const due = replay.playing && !replay.waiting ? pauseDue(currentTrack()?.pauses, prev, replay.now) : null;
    if (due) {
      audio.pause();
      audio.currentTime = due.t;
      replay.now = due.t;
      lastTick = due.t;
      replay.playing = false;
      replay.invite = 'pause';
      replay.inviteHint = due.hint ?? null;
      savePosition();
    }
  }
  pump();
  raf = replay.playing ? requestAnimationFrame(tick) : 0;
}

function onEnded(): void {
  if (replay.faq) backToMain();
  else pause();
}

function makeAudio(): HTMLAudioElement {
  audio = new Audio();
  audio.preload = 'auto';
  audio.addEventListener('ended', onEnded);
  return audio;
}

async function loadCaptions(track: Track | null | undefined): Promise<void> {
  replay.cues = [];
  // With the cloned voice, the captions of what it says.
  const dub = dubOf(track, viewerLocale(), replay.originalVoice);
  const file = dub?.captions ?? track?.captions?.[viewerLocale()] ?? track?.captions?.[SOURCE_LOCALE];
  replay.captionsLang = dub || track?.captions?.[viewerLocale()] ? viewerLocale() : SOURCE_LOCALE;
  if (!file) return;
  try {
    const res = await fetch(`${CAPTIONS_BASE}${file}`);
    if (res.ok) replay.cues = parseVtt(await res.text());
  } catch {
    /* captions are a help, not a condition */
  }
}

async function loadTrack(track: Track): Promise<void> {
  const a = audio ?? makeAudio();
  const dub = dubOf(track, viewerLocale(), replay.originalVoice);
  replay.dubbed = !!dub;
  await loadVoice(a, (dub?.audio ?? track.audio).src);
  a.playbackRate = replay.speed;
  const base = recordedAt(track);
  if (base != null) installClock(replayClock(base));
  // Called from begin() and switchTo(), during a replay.
  ctx!.presentation.init({ slides: slidesFor(track) });
  loadCaptions(track);
}

/** Point the audio at `file`, and wait until its length is known. */
async function loadVoice(a: HTMLAudioElement, file: string): Promise<void> {
  const src = `${MEDIA_BASE}${file}`;
  if (!a.src.endsWith(file)) {
    a.src = src;
    // A missing or broken file must end the wait: without an error path the
    // page shows its spinner forever.
    await new Promise<void>((resolve, reject) => {
      if (a.readyState >= 1) return resolve();
      const timer = setTimeout(() => done(new Error(`timeout: ${src}`)), MEDIA_TIMEOUT);
      const done = (err?: Error) => {
        clearTimeout(timer);
        a.removeEventListener('loadedmetadata', ok);
        a.removeEventListener('error', fail);
        if (err) reject(err);
        else resolve();
      };
      const ok = () => done();
      const fail = () => done(new Error(`media: ${src}`));
      a.addEventListener('loadedmetadata', ok);
      a.addEventListener('error', fail);
    });
  }
}

/**
 * The presenter's own voice, or the cloned one in the viewer's language.
 * Both run on the same clock, so the replay goes on from the moment it was
 * at, in the other voice.
 */
export async function setOriginalVoice(on: boolean): Promise<void> {
  replay.originalVoice = on;
  try {
    if (on) window.localStorage.setItem(VOICE_KEY, 'original');
    else window.localStorage.removeItem(VOICE_KEY);
  } catch {
    /* a preference, not a condition */
  }
  const track = currentTrack();
  if (!replay.active || !track || !audio) return;
  // Like a track switch: whatever was jumping stops here, and a jump, a
  // question or leaving during the load wins over this.
  seekToken += 1;
  const token = seekToken;
  const at = replay.now;
  const playAfter = replay.playing;
  replay.playing = false;
  audio.pause();
  const dub = dubOf(track, viewerLocale(), on);
  replay.dubbed = !!dub;
  try {
    await loadVoice(audio, (dub?.audio ?? track.audio).src);
  } catch {
    if (still(token)) fail();
    return;
  }
  if (!still(token)) return;
  audio.playbackRate = replay.speed;
  loadCaptions(track);
  await seek(at, { play: playAfter });
}

function readVoicePreference(): boolean {
  try {
    return window.localStorage.getItem(VOICE_KEY) === 'original';
  } catch {
    return false;
  }
}

const MEDIA_TIMEOUT = 20000;

/**
 * The time the demo sees during the replay: the recording's own date, moving
 * with the voice. While the voice stands still (paused, waiting, the viewer
 * clicking along) the clock keeps running on wall time, so code that stamps
 * ids or measures durations with `Date.now()` does not see time stop.
 */
function replayClock(base: number): NowSource {
  let extra = 0;
  let lastReal = performance.now();
  return () => {
    const real = performance.now();
    if (!replay.playing || replay.waiting || replay.diverged) extra += real - lastReal;
    lastReal = real;
    return base + replay.now * 1000 + extra;
  };
}

/** A token check for the steps after an await: a jump, a stop, the viewer leaving. */
function still(token: number): boolean {
  return token === seekToken && replay.active;
}

// ---- the controls ------------------------------------------------------------------

/**
 * Bring the demo to how it was at `t` and continue from there. The chapter's
 * recorded state, then the chapter's events up to `t`, applied fast.
 */
export async function seek(t: number, { play: playAfter = replay.playing, careful = false }: { play?: boolean; careful?: boolean } = {}): Promise<void> {
  const track = currentTrack();
  if (!track || !ctx) return;
  seekToken += 1;
  const token = seekToken;
  // From here the events are this seek's: a pump or a play in the meantime
  // would apply the old position's events onto the restored chapter.
  busyOwner = token;
  seeking = token;
  cancelAnimationFrame(raf);
  raf = 0;
  audio?.pause();
  // Play or pause pressed during the seek only changes what happens after it.
  replay.playing = playAfter;
  replay.invite = null;
  stopBumper();
  replay.waiting = false;
  replay.diverged = false;
  replay.cursor.visible = false;
  let i = 0;
  const target = Math.max(0, Math.min(track.duration, t));
  // A jump that takes a moment replays the chapter's clicks at speed, which
  // looks like the demo going every which way. Cover it after a beat, so a
  // quick jump does not flash.
  const cover = setTimeout(() => {
    if (seeking === token) replay.restoring = true;
  }, 250);
  try {
    // A chapter written after recording has no snapshot of its own: start
    // from the last one that has, and replay through to the target.
    // A valid track has a chapter (validTrack), and ci stays within them.
    let ci = Math.max(0, chapterAt(track, target));
    while (ci > 0 && !track.chapters[ci]!.state) ci -= 1;
    const chapter = track.chapters[ci]!;
    replay.now = chapter.start;
    applyState(chapter.state);
    resetViews();
    await nextTick();
    if (!still(token)) return;
    // seek() returned above without a context.
    if (!ctx!.presentation.active.value) ctx!.presentation.start(chapter.slideIndex, { keys: false });
    await ctx!.presentation.goTo(chapter.slideIndex);
    await sleep(250);
    if (!still(token)) return;
    const events = track.events ?? [];
    // The chapter's snapshot was taken when its slide came up, so whatever comes
    // before that slide event in the list (actions moved out of a cut to the
    // same moment) is already in it. Replay from just after the slide event.
    const slideAt = events.findIndex((e) => e.type === 'slide' && e.index === chapter.slideIndex && e.t >= chapter.start - 0.001);
    i = slideAt >= 0 ? slideAt + 1 : events.findIndex((e) => e.t >= chapter.start);
    if (i < 0) i = events.length;
    // The loop bound checks i first, so events[i] exists.
    for (; i < events.length && events[i]!.t <= target; i += 1) {
      if (!still(token)) return;
      const e = events[i]!;
      replay.now = e.t;
      await apply(e, { fast: true, token });
      // Where the presenter waited before going on, the demo was busy: a
      // sheet closing, the engine recalculating. Leave it a moment as well.
      const after = events[i + 1];
      const gap = after && after.t <= target ? after.t - e.t : 0;
      // Careful (the verify run's second look at a chapter that missed
      // something): the recorded pace, with long waits shortened, so a miss
      // that remains is the demo's and not the speed's.
      if (careful) await sleep(Math.min(gap, 2) * 1000);
      else if (gap > 1.5 && (e.type === 'click' || e.type === 'key' || e.type === 'change')) await sleep(400);
    }
  } finally {
    clearTimeout(cover);
    if (busyOwner === token) busyOwner = null;
    if (seeking === token) {
      seeking = null;
      replay.restoring = false;
    }
  }
  if (!still(token)) return;
  next = i;
  leadFor = -1;
  replay.now = target;
  lastTick = target;
  if (audio) audio.currentTime = target;
  if (replay.playing) play();
}

export async function play(): Promise<void> {
  // Still loading: the start of the replay plays when it is ready.
  if (!audio) {
    if (replay.active) replay.playing = true;
    return;
  }
  // A seek in progress plays when it is done.
  if (seeking !== null) {
    replay.playing = true;
    return;
  }
  if (replay.diverged) {
    // "Oké, we gaan verder" while the demo is put back the way it was.
    const said = sayBumper('resume');
    await seek(replay.now, { play: false });
    const token = seekToken;
    replay.playing = true;
    await said;
    // Paused, jumped or left during the line: that wins.
    if (token !== seekToken || !replay.active || !replay.playing || replay.diverged) return;
    replay.playing = false;
    await play();
    return;
  }
  replay.invite = null;
  replay.inviteHint = null;
  lastTick = replay.now;
  replay.playing = true;
  // During a wait the voice stays where it is; the wait resumes it.
  if (!replay.waiting) {
    await audio.play().catch(() => {
      replay.playing = false;
    });
  }
  if (!raf) raf = requestAnimationFrame(tick);
}

export function pause(): void {
  replay.playing = false;
  audio?.pause();
  stopBumper();
  savePosition();
}

export function togglePlay(): void | Promise<void> {
  return replay.playing ? pause() : play();
}

export function setSpeed(s: number): void {
  replay.speed = s;
  if (audio) audio.playbackRate = s;
}

export function openFaq(entry: FaqEntry | null | undefined): void {
  if (!entry || !timeline) return;
  if (!replay.faq) mainPosition = replay.now;
  replay.faq = entry;
  switchTo(entry, 0);
}

export function backToMain(): void {
  if (!replay.faq) return;
  replay.faq = null;
  // A question plays only in a replay begin() loaded the timeline for.
  switchTo(timeline!.main, mainPosition);
}

/** Load another track and play it from `at`, unless something else happened first. */
function switchTo(track: Track, at: number): void {
  // Whatever was jumping or playing stops here.
  seekToken += 1;
  const token = seekToken;
  // Not pause(): that would store the question's position as the main line's.
  replay.playing = false;
  audio?.pause();
  loadTrack(track).then<unknown, unknown>(
    () => still(token) && seek(at, { play: true }),
    () => still(token) && fail(),
  );
}

/** The media did not load: leave the replay and let the page say so. */
function fail(): void {
  stopReplay();
  replay.failed = true;
}

function savePosition(): void {
  if (replay.faq) return;
  try {
    window.localStorage.setItem(POSITION_KEY, String(replay.now));
  } catch {
    /* no storage: start at the beginning next time */
  }
}

export function savedPosition(): number {
  try {
    const n = Number(window.localStorage.getItem(POSITION_KEY));
    return Number.isFinite(n) && n > 0 && n < (timeline?.main?.duration ?? 0) - 5 ? n : 0;
  } catch {
    return 0;
  }
}

// ---- the viewer taking over -------------------------------------------------------

/** Keys the player answers to; see ReplayOverlay.vue. */
export const PLAYER_KEYS = new Set([' ', 'ArrowLeft', 'ArrowRight', 'Escape']);

/** `matches`, on whatever an event's path holds (a window or a document has none). */
const pathMatches = (n: EventTarget | null | undefined, selector: string) => (n as Partial<Element> | null | undefined)?.matches?.(selector);

function fromViewer(e: Event): boolean {
  if (!e.isTrusted || !replay.active) return false;
  // The player's own controls and the deck are not the demo.
  return !(e.composedPath?.() ?? []).some((n) => pathMatches(n, '.deck, .wt-chrome'));
}

function onViewerAct(e: PointerEvent | KeyboardEvent): void {
  if (!fromViewer(e)) return;
  // A key is the viewer taking over only when it goes into the demo's own
  // fields. While it plays, Space, the arrows and Escape are the player's,
  // also when the replay itself left focus in a field it typed into.
  if (e.type === 'keydown') {
    if (replay.playing && PLAYER_KEYS.has((e as KeyboardEvent).key)) return;
    if (!(e.composedPath?.() ?? []).some((n) => pathMatches(n, 'input, textarea, select, [contenteditable]'))) return;
  }
  takeOver({ say: replay.playing });
}

/**
 * The viewer takes the demo over (clicked into it, or "try it yourself"):
 * pause, and wait for them with a hint at the play button. `say` adds a line
 * in the presenter's voice ("Ga je gang, kijk maar even rond, ik wacht"),
 * when the walkthrough has one; only when it was playing, not when it had
 * already stopped at a pause that said as much.
 */
export function takeOver({ say = false }: { say?: boolean } = {}): void {
  if (replay.playing) pause();
  replay.diverged = true;
  replay.cursor.visible = false;
  if (!replay.invite) {
    replay.invite = 'wait';
    replay.inviteHint = null;
  }
  if (say) sayBumper('wait');
}

// ---- the presenter's short lines ---------------------------------------------------

let bumperAudio: HTMLAudioElement | null = null;
let bumperDone: (() => void) | null = null;

/**
 * Play one of the walkthrough's short lines of `kind` ('wait', 'resume'),
 * picked at random; resolves when it has been said, or at once when there
 * is none. Its own audio element: it must not move the main clock.
 */
function sayBumper(kind: string): Promise<void> {
  stopBumper();
  // With the cloned voice its own lines, or none: a Dutch "ga je gang"
  // between English sentences would be a third voice.
  const clip = pickClip(replay.dubbed ? timeline?.dub?.[viewerLocale()]?.bumpers?.[kind] : timeline?.bumpers?.[kind]);
  if (!clip) return Promise.resolve();
  bumperAudio = bumperAudio ?? new Audio();
  bumperAudio.src = `${MEDIA_BASE}${clip.src}`;
  bumperAudio.muted = audio?.muted ?? false;
  bumperAudio.playbackRate = replay.speed;
  // bumperAudio was set just above; the executor runs synchronously, before anything can clear it.
  const a = bumperAudio;
  return new Promise<void>((resolve) => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const done = () => {
      clearTimeout(timer);
      bumperAudio?.removeEventListener('ended', done);
      bumperAudio?.removeEventListener('error', done);
      if (bumperDone === done) bumperDone = null;
      resolve();
    };
    bumperDone = done;
    a.addEventListener('ended', done);
    a.addEventListener('error', done);
    // A line that never ends (a stalled download) does not hold the replay.
    timer = setTimeout(done, ((clip.duration ?? 3) / replay.speed + 2) * 1000);
    a.play().catch(done);
  });
}

function stopBumper(): void {
  bumperAudio?.pause();
  bumperDone?.();
}

// ---- starting and stopping ---------------------------------------------------------

/**
 * Take over the demo for the walkthrough. `context` is the router, the
 * store (`useDemo()`) and the deck (`usePresentation()`).
 */
export async function startReplay(
  data: Timeline,
  context: ReplayContext,
  { at = 0, faqId = null }: { at?: number; faqId?: string | null } = {},
): Promise<void> {
  // A kept-alive page is mounted and activated in the same tick, and both
  // start the walkthrough; the second would take the first one's Dutch for
  // the viewer's language and run a second replay over it.
  if (replay.active || starting) return;
  starting = true;
  try {
    await begin(data, context, { at, faqId });
  } finally {
    starting = false;
  }
}

/** The demo is not shown smaller than this, however narrow the window. */
const MIN_FIT = 0.7;

/**
 * Show the demo at the size it was recorded in. A take made in a wide window
 * replays in a narrower one with the same words on less room: lines wrap,
 * and what the presenter points at slides out of view. On a window narrower
 * than the recording the demo (not the rail) is scaled down to fit, the way
 * the presenter zoomed out his own browser.
 */
function fitDemo(): void {
  const recorded = currentTrack()?.viewport?.width ?? timeline?.main?.viewport?.width;
  const zoom = recorded ? Math.max(MIN_FIT, Math.min(1, window.innerWidth / recorded)) : 1;
  const root = document.documentElement;
  root.classList.toggle('rr-replay-fit', zoom < 0.999);
  root.style.setProperty('--rr-demo-zoom', String(Math.round(zoom * 1000) / 1000));
}

let starting = false;

async function begin(data: Timeline, context: ReplayContext, { at, faqId }: { at: number; faqId: string | null }): Promise<void> {
  timeline = data;
  ctx = context;
  // A live presentation that was still running would keep its arrow keys and
  // move the deck under the replay.
  ctx.presentation.stop();
  backup = clone(ctx.demo.state);
  setPersistence(false);
  // The replay always shows the rail. A recorded "Demo resetten" puts the
  // demo back in its default mode, which has no rail; keep it.
  unwatchMode = watch(
    // ctx is set above and never cleared.
    () => ctx!.demo.state.presentationMode,
    (mode) => {
      if (replay.active && mode !== 'zelfstandig') ctx!.demo.state.presentationMode = 'zelfstandig';
    },
    { flush: 'sync' },
  );
  // The recording is Dutch: the controls it clicks are described by their
  // Dutch text. Where the walkthrough is translated into the viewer's
  // language, the demo stays in it and the replay finds each control through
  // its translation; otherwise the demo runs in Dutch for the length of the
  // replay. The player's own controls, the slides and the captions follow the
  // viewer either way.
  previousLocale = currentLocale();
  setViewerLocale(previousLocale);
  playLocale = timeline.locales?.includes(previousLocale) ? previousLocale : SOURCE_LOCALE;
  translate = await translatorFor(playLocale);
  replay.demoLocale = playLocale;
  replay.originalVoice = readVoicePreference();
  adoptLocale(playLocale);
  await nextTick();
  replay.active = true;
  window.addEventListener('resize', fitDemo);
  const loading = seekToken;
  replay.faq = null;
  document.addEventListener('pointerdown', onViewerAct, true);
  document.addEventListener('keydown', onViewerAct, true);
  const faq = faqId ? timeline.faq?.find((f) => f.id === faqId) : null;
  if (faq) mainPosition = at;
  replay.faq = faq ?? null;
  try {
    // A timeline from useWalkthrough() is valid, so there is a track; were
    // there none, loadTrack would throw here and end in fail(), as before.
    await loadTrack(currentTrack() as Track);
  } catch {
    fail();
    return;
  }
  fitDemo();
  // A jump made while the track was loading (a chapter picked, the arrows)
  // already put the demo where the viewer wants it.
  if (seekToken !== loading) return;
  // The track loaded above, and a valid track has a chapter.
  ctx.presentation.start(currentTrack()!.chapters[0]!.slideIndex, { keys: false });
  // Play pressed while loading plays from here, rather than being undone.
  await seek(faq ? 0 : at, { play: replay.playing });
}

export function stopReplay(): void {
  if (!replay.active) return;
  pause();
  seekToken += 1;
  cancelAnimationFrame(raf);
  raf = 0;
  document.removeEventListener('pointerdown', onViewerAct, true);
  document.removeEventListener('keydown', onViewerAct, true);
  uninstallClock();
  window.removeEventListener('resize', fitDemo);
  document.documentElement.classList.remove('rr-replay-fit');
  document.documentElement.style.removeProperty('--rr-demo-zoom');
  unwatchMode?.();
  unwatchMode = null;
  replay.active = false;
  replay.faq = null;
  replay.invite = null;
  replay.cursor.visible = false;
  stopBumper();
  replay.cues = [];
  if (audio) {
    audio.removeAttribute('src');
    audio.load();
  }
  // An active replay has a context (begin() set it).
  const { demo, presentation } = ctx!;
  presentation.stop();
  if (backup) Object.assign(demo.state, backup);
  setPersistence(true);
  demo.reregister();
  presentation.init({ slides: demo.corpus.value?.config?.slides ?? [] });
  if (previousLocale && previousLocale !== currentLocale()) {
    adoptLocale(previousLocale);
    // The URL decides the language on the next navigation; put the page in
    // the viewer's language now, so it does not flip back.
    const here = ctx!.router.currentRoute.value;
    const page = here.meta?.page;
    if (page) ctx!.router.replace({ name: localeRouteName(page, previousLocale), params: here.params }).catch(() => {});
  }
  resetViews();
  setViewerLocale(null);
  previousLocale = null;
  playLocale = SOURCE_LOCALE;
  translate = null;
  replay.demoLocale = SOURCE_LOCALE;
  replay.captionsLang = SOURCE_LOCALE;
  replay.dubbed = false;
  backup = null;
}

/** For the webcam bubble, which follows the voice. */
export function audioTime(): number {
  return audio?.currentTime ?? replay.now;
}

// A handle for scripted checks against the dev server (the Playwright runs
// that verify a recording); not in the production bundle.
/** The handle the scripted checks use; `afterScroll` is set by `just walkthrough anchors`. */
export interface ReplayDevHandle {
  replay: ReplayState;
  play: typeof play;
  pause: typeof pause;
  seek: typeof seek;
  openFaq: typeof openFaq;
  backToMain: typeof backToMain;
  scrollAnchor: typeof scrollAnchor;
  pathFor: (code: string, faqId?: string | null) => string;
  deckIndex: () => number | undefined;
  state: () => unknown;
  track: typeof currentTrack;
  afterScroll?: (e: RecordedScrollEvent, el: Element) => void;
}

declare global {
  interface Window {
    __rrReplay?: ReplayDevHandle;
  }
}

if (import.meta.env.DEV && typeof window !== 'undefined') {
  // Called by the scripts once a replay runs; before that ctx is null and this throws, as before.
  const pathFor = (code: string, faqId?: string | null) => ctx!.router.resolve({ name: localeRouteName('rondleiding', code), params: faqId ? { faqId } : {} }).path;
  window.__rrReplay = { replay, play, pause, seek, openFaq, backToMain, scrollAnchor, pathFor, deckIndex: () => ctx?.presentation?.index?.value, state: () => clone(ctx?.demo?.state), track: currentTrack };
}
