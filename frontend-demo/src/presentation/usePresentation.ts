/**
 * The presentation deck, shared state for the whole app (module-level refs).
 *
 * The deck tells the story on the left; the live demo answers on the right.
 * An intro or closing slide covers the screen; a slide with a `route` shrinks
 * the deck to a rail, opens that tab, optionally switches the persona and
 * pulses the part of the screen the presenter points at. Slides come from
 * `corpus/demo/demo-config.yaml` (`slides:`), so the story is content, not code.
 *
 * Same pattern as the Begane Grond deck, reduced to what this demo needs.
 */
import { computed, nextTick, ref } from 'vue';
import type { RouteRecordNameGeneric } from 'vue-router';
import { currentLocale } from '../i18n/index.js';
import { localeRouteName, pageForConfigPath, splitConfigPath } from '../router';
import type { DemoProfileConfig, SlideConfig } from '../data/loadCorpus';
import type { PresentationMode } from '../store/demoStore';

export type { SlideConfig };

/** A route as far as the deck reads it: the router's own routes fit this. */
export interface DeckRoute {
  path: string;
  name?: RouteRecordNameGeneric | null;
  meta?: Record<string, unknown>;
}

/**
 * The router, as far as the deck uses it. The app's vue-router instance fits
 * this; the tests pass a double with only these members.
 */
export interface PresentationRouter {
  currentRoute: { value: DeckRoute };
  resolve(to: string | { name: string }): DeckRoute;
  push(to: string): Promise<unknown>;
}

/**
 * The store (`useDemo()`), as far as the deck reads it: the chosen persona,
 * its configured defaults, and the corpus to resolve a default law in.
 */
export interface PresentationDemo {
  profileKey: { value: string | null | undefined };
  profile: { value: Pick<DemoProfileConfig, 'default_law' | 'default_feature'> | null | undefined };
  corpus: { value: { lawByPath: (lawPath: string, service?: string | null) => { id: string } | null } | null | undefined };
  setProfile(key: string): void;
}

/** What `init` takes; each field it is given replaces the previous one. */
export interface PresentationInit {
  router?: PresentationRouter | null;
  demo?: PresentationDemo | null;
  slides?: SlideConfig[] | null;
}

// How the deck is shown; the store persists it.
export type { PresentationMode };

/** Options for `start`. */
export interface StartOptions {
  /** `false` leaves the keyboard alone (the recorded walkthrough drives the deck). */
  keys?: boolean;
}

/**
 * The slide's target, in the language that is on.
 *
 * `route:` in demo-config.yaml is a Dutch path (`/regelwerken`), because a file
 * about slides should not have to know the routing table of every language.
 * It is read back to its page here and resolved against the active locale, so
 * a deck presented in English opens the English tabs.
 */
export function slideTarget(path: string | null | undefined): string | null {
  if (!path || !router) return null;
  const parts = splitConfigPath(path);
  if (!parts) return path;
  // What follows the tab (`/regelwerken/zorgtoeslagwet`: the law) comes along.
  // Without it the slide opens the tab on whatever law was left open, and after
  // a rehearsal the presenter lands on the wrong one. The rest comes from
  // `splitConfigPath`, so a slide that still carries a former path keeps its law.
  // A slide that names no law gets the chosen persona's own (profileDefault).
  const { page } = parts;
  const rest = parts.rest || profileDefault(page);
  const base = router.resolve({ name: localeRouteName(page, currentLocale()) }).path;
  return rest ? `${base.replace(/\/$/, '')}${rest}` : base;
}

/**
 * Noemt een dia geen wet (`route: /regelwerken`), dan opent hij die van het gekozen
 * profiel: `default_law` op Wetten, `default_feature` op Scenario's. Wie
 * Claudia kiest en de presentatie start, landt zo bij precario in plaats van
 * bij de zorgtoeslag van Merijn. Vast in het pad blijft ook hier de wet die
 * er toevallig nog open stond buiten de deur.
 */
function profileDefault(page: string): string {
  const profile = demo?.profile?.value;
  if (!profile) return '';
  if (page === 'wetten' && profile.default_law) {
    const d = profile.default_law;
    // `profile` came from `demo`, so `demo` is set.
    const law = demo!.corpus?.value?.lawByPath(d.law_path, d.service);
    return law ? `/${encodeURIComponent(law.id)}` : '';
  }
  if (page === 'scenarios' && profile.default_feature) return `/${profile.default_feature}`;
  return '';
}

/** Alle dia's uit demo-config.yaml; het dek zelf is de keuze daaruit bij de start. */
let allSlides: SlideConfig[] = [];
/** Voor welke persona het lopende dek gekozen is (vast vanaf `start`). */
let deckKey: string | null | undefined = null;

/**
 * Het dek van een persona: de dia's zonder `decks` (opening, de wet, de
 * simulatie, het slot) plus die met deze persona in `decks`. Wie Claudia kiest
 * en start, krijgt haar verhaal en wisselt niet halverwege naar Merijn.
 */
export function deckFor(slides: SlideConfig[], profileKey: string | null | undefined): SlideConfig[] {
  // Widened so a missing key can be looked up as before (it matches no deck).
  return slides.filter((s) => !s.decks || (s.decks as readonly unknown[]).includes(profileKey));
}

const active = ref(false);
/** Whether Escape stops the deck. The recorder switches it off for the length of a take. */
export const escapeStops = ref(true);
const index = ref(0);
const slidesRef = ref<SlideConfig[]>([]);
/**
 * 'zaal' of 'zelfstandig'. Een eigen ref en geen greep in de store, want de
 * store haalt zelf het corpus op en zou dan om deze module heen cirkelen. App
 * houdt hem gelijk aan `state.presentationMode`.
 */
const mode = ref<PresentationMode>('zaal');

let router: PresentationRouter | null = null;
let demo: PresentationDemo | null = null;
let listening = false;

const current = computed<SlideConfig | null>(() => slidesRef.value[index.value] ?? null);
const total = computed(() => slidesRef.value.length);
/** Full-screen slides: title, statement, closing, or anything without a route. */
const isFull = computed(() => !current.value?.route);
/**
 * Staat het dek in beeld?
 *
 * In de zaal wel op een dia die het scherm dekt (het verhaal), niet op een dia
 * die de demo opent: daar is het scherm van de demo en zou een rail ernaast
 * juist verkleinen wat het publiek moet zien. Zelfstandig staat het dek er
 * altijd, want dan is er niemand die het verhaal vertelt.
 *
 * De presentatie blijft in beide gevallen actief: alleen het renderen stopt,
 * zodat pijltjes en spatie blijven bladeren en `stop()` de toetsen niet
 * loskoppelt.
 */
const visible = computed(() => (mode.value === 'zaal' ? isFull.value : true));

/**
 * Luistert het dek nog naar de toetsen?
 *
 * Staat het dek in beeld, dan hoort het toetsenbord erbij. Staat het er niet
 * (zaalmodus op een dia die de demo opent), dan alleen zolang het scherm nog
 * van díe dia is: de presentator klikt in de demo die de dia geopend heeft en
 * bladert ondertussen door. Loopt hij daarna zelf naar een ander tabblad, dan
 * gaat de presentatie nergens meer over en moeten spatie en de pijltjes terug
 * naar de pagina. Anders drukt spatie op de voorgrond niets in en springt er
 * onzichtbaar een dia verder.
 *
 * Een functie en geen `computed`: dit wordt gelezen op het moment van een
 * toetsaanslag, niet in een template. Een computed zou de route cachen en is
 * daarmee juist onbetrouwbaar op het enige moment dat telt.
 */
function isOnStage(): boolean {
  if (!active.value) return false;
  if (visible.value) return true;
  const slideRoute = current.value?.route;
  // Geen router (init is nog niet langsgekomen): dan valt niet vast te stellen
  // waar we zijn, en houdt het dek de toetsen niet vast. Dat is de veilige
  // kant: onzichtbaar bladeren is precies wat hier misging.
  if (!slideRoute || !router) return false;
  // Op het tabblad vergelijken en niet op het pad. `/regelwerken/:lawId?`,
  // `/scenarios/:featurePath(.*)?` en `/zaaksysteem/:caseId?` verdiepen hun
  // eigen pad: WettenView en ScenariosView doen bij binnenkomst meteen een
  // `router.replace` naar de standaardwet of -feature van het profiel, nog
  // voordat de presentator iets aanraakt. Op het pad vergelijken zou het dek
  // dus doof maken op precies de dia die dat tabblad zojuist opende.
  const here = router.currentRoute?.value;
  // Op de pagina vergelijken en niet op de routenaam: dezelfde pagina heeft
  // per taal een eigen naam (`wetten` en `wetten:en`), en `meta.page` is wat
  // die twee delen.
  const page = pageForConfigPath(slideRoute);
  if (page && here?.meta?.page) return here.meta.page === page;
  const target = router.resolve?.(slideTarget(slideRoute) ?? slideRoute);
  if (target?.name && here?.name) return here.name === target.name;
  return here?.path === slideRoute;
}

function init({ router: r, demo: d, slides }: PresentationInit): void {
  if (r) router = r;
  if (d) demo = d;
  if (slides) allSlides = slides;
  // Loopt de presentatie, dan blijft het dek van die persona, maar in de nieuwe
  // dia's: een taalwissel levert dezelfde dia's vertaald.
  slidesRef.value = deckFor(allSlides, active.value ? deckKey : demo?.profileKey?.value);
}

/**
 * De klasse op <html> die de body naast het dek schuift, of juist niet.
 * De body krijgt geen ruimte ingeruimd als het dek het scherm dekt én niet als
 * het er helemaal niet staat; alleen de rail schuift op.
 */
function applyLayout(slide: SlideConfig | null = current.value): void {
  // In de zaal schuift de body nooit op: het dek dekt het scherm of het staat
  // er niet. Zelfstandig schuift hij alleen voor een dia met een route, want
  // dan krimpt het dek tot de rail ernaast.
  const full = mode.value === 'zaal' || !slide?.route;
  document.documentElement.classList.toggle('rr-presenting-full', full);
  // Staat het dek er niet, dan neemt het ook de persona niet meer voor zijn
  // rekening, en moet de balk zijn eigen knoppen terugkrijgen.
  const hidden = mode.value === 'zaal' && !!slide?.route;
  document.documentElement.classList.toggle('rr-deck-hidden', hidden);
}

function setMode(m: string | null | undefined): void {
  mode.value = m === 'zelfstandig' ? 'zelfstandig' : 'zaal';
  if (active.value) applyLayout();
}

/** Pulse-highlight what the presenter points at; failures never break the talk. */
function highlight(selector: string): void {
  try {
    const nodes = [...document.querySelectorAll(selector)];
    nodes.forEach((n) => n.classList.add('rr-present-pulse'));
    setTimeout(() => nodes.forEach((n) => n.classList.remove('rr-present-pulse')), 1400);
  } catch {
    /* invalid selector */
  }
}

async function runSlide(i: number): Promise<void> {
  const s = slidesRef.value[i];
  if (!s) return;
  applyLayout(s);
  // The persona is a function of the slide index: the most recent `profile`
  // at or before this slide, so prev/next/goto agree. Before the first such
  // slide it is the persona the deck was started for: wie in Merijns dek
  // terugbladert van de Claudia-dia, hoort weer bij de zorgtoeslag uit te komen.
  if (demo) {
    let persona = deckKey;
    for (let j = i; j >= 0; j -= 1) {
      const p = slidesRef.value[j]?.profile;
      if (p) {
        persona = p;
        break;
      }
    }
    if (persona && demo.profileKey.value !== persona) demo.setProfile(persona);
  }
  const target = slideTarget(s.route);
  if (target && router && router.currentRoute.value.path !== target) {
    try {
      await router.push(target);
    } catch {
      /* redundant navigation */
    }
  }
  await nextTick();
  // `!`: checked on the same line; a slide is not changed after it is loaded.
  if (s.highlight) setTimeout(() => highlight(s.highlight!), 350);
}

function goTo(i: number): Promise<void> {
  const n = Math.max(0, Math.min(total.value - 1, i));
  index.value = n;
  return runSlide(n);
}
function next(): Promise<void> | void {
  if (index.value < total.value - 1) return goTo(index.value + 1);
  return stop();
}
function prev(): Promise<void> | undefined {
  if (index.value > 0) return goTo(index.value - 1);
  return undefined;
}

/**
 * Waar kwam deze toetsaanslag vandaan?
 *
 * Niet `e.target.closest(...)`, want de invoervelden van het ontwerpsysteem
 * (`nldd-text-field`, `nldd-date-field`, ...) zetten hun `<input>` in een open
 * shadow root. Het event retarget dan naar de host, en `closest('input')` op
 * die host vindt niets: gemeten in Chrome is `e.target` `NLDD-TEXT-FIELD` en
 * staat de `<input>` alleen op `composedPath()`. Zonder dit liep een spatie in
 * een bedragveld niet het veld in maar bladerde hij een dia verder.
 *
 * `composedPath()` loopt dwars door shadow-grenzen heen, dus daar staat alles
 * tussen de echte `<input>` en `window`. Lukt dat niet (oudere browser,
 * gesynthetiseerd event), dan valt hij terug op `closest`.
 */
function pathMatches(e: Event, selector: string): boolean {
  const path = e.composedPath?.();
  if (Array.isArray(path) && path.length) {
    // Not every target on the path is an element (document, window).
    return path.some((n) => !!(n as MaybeElement)?.matches?.(selector));
  }
  return !!(e.target as MaybeElement)?.closest?.(selector);
}

/** An event target that may or may not be an element. */
type MaybeElement = Partial<Pick<Element, 'matches' | 'closest'>> | null;

function onKey(e: KeyboardEvent): void {
  if (pathMatches(e, 'input, textarea, select, [contenteditable]')) return;
  // Escape blijft altijd de noodrem, ook als het dek niet in beeld staat. Het
  // is de enige toets die een presentatie beeindigt zonder haar uit te lopen,
  // en juist off-stage moet dat kunnen: dan leeft ze onzichtbaar door.
  // Bewust vóór de poort hieronder, want die is precies de toestand waarin je
  // eruit wilt.
  if (e.key === 'Escape' && active.value) {
    // An open sheet or dialog takes its own Escape: closing the trace must not
    // end the presentation with it. During a take Escape never stops the deck,
    // because every slide after it would be missing from the recording.
    if (!escapeStops.value || pathMatches(e, 'dialog[open], [popover]')) return;
    stop();
    return;
  }
  // De bladertoetsen gaan wél door de poort: is het scherm niet meer van de
  // presentatie, dan vangt ze niets meer af.
  if (!isOnStage()) return;
  // Spatie bedient ook de knop die focus heeft. Zonder dek in beeld (zaalmodus)
  // klikt de presentator in de demo zelf, en dan zou één spatie tegelijk de
  // knop indrukken én een dia verder springen. De pijltjes blijven wel werken,
  // want die doen op een knop niets.
  if (e.key === ' ' && pathMatches(e, 'button, [role="button"], a[href], summary')) return;
  switch (e.key) {
    case 'ArrowRight':
    case ' ':
    case 'PageDown':
      e.preventDefault();
      next();
      break;
    case 'ArrowLeft':
    case 'PageUp':
      e.preventDefault();
      prev();
      break;
    case 'Home':
      goTo(0);
      break;
    case 'End':
      goTo(total.value - 1);
      break;
    // Escape staat hierboven, vóór de poort.
    case 'f':
      if (document.fullscreenElement) document.exitFullscreen?.();
      else document.documentElement.requestFullscreen?.();
      break;
    default:
  }
}

/**
 * Start the deck at slide `i`. `keys: false` leaves the keyboard alone: the
 * recorded walkthrough drives the deck itself and has its own keys.
 */
function start(i = 0, { keys = true }: StartOptions = {}): void {
  // Het dek ligt vast zodra de presentatie loopt: een dia die van persona
  // wisselt (Merijns dek eindigt bij Claudia) gooit het niet halverwege om.
  if (!active.value) {
    deckKey = demo?.profileKey?.value;
    slidesRef.value = deckFor(allSlides, deckKey);
  }
  if (!total.value) return;
  active.value = true;
  document.documentElement.classList.add('rr-presenting');
  if (keys && !listening) {
    window.addEventListener('keydown', onKey);
    listening = true;
  }
  goTo(i);
}

function stop(): void {
  active.value = false;
  document.documentElement.classList.remove('rr-presenting', 'rr-presenting-full', 'rr-deck-hidden');
  if (listening) {
    window.removeEventListener('keydown', onKey);
    listening = false;
  }
}

export function usePresentation() {
  // `mode` zelf gaat er niet uit: de bron daarvan is `state.presentationMode`
  // in de store, en twee plekken om dezelfde waarde te lezen lopen uiteen.
  // Wie wil weten wat het dek doet, leest `visible`.
  return { active, index, current, total, isFull, visible, isOnStage, setMode, slides: slidesRef, init, start, stop, next, prev, goTo };
}

/** What `usePresentation()` returns: the deck's shared state and its controls. */
export type Presentation = ReturnType<typeof usePresentation>;
