/**
 * Taal van de demo: Nederlands is de bron, de rest is er de vertaling van.
 *
 * Dutch is the source of truth. Every key exists in `nl.js` first; `en.js` is
 * a translation of it, and `en.sources.js` records which Dutch string each
 * translation was made from, so a changed original shows up as a stale
 * translation instead of silently keeping an old English sentence. See
 * `i18n.test.ts` for what that buys.
 *
 * Shaped after `useColorScheme()` from @regelrecht/frontend-shared, which this
 * app already consumes: a module-level ref, a readonly accessor, and writes
 * that go through one setter. The locale deliberately is NOT a component-scoped
 * value, because the strings are needed in plain modules that have no setup()
 * — `format.ts` (imported by its own tests), `delegation.ts`, `stats.ts`.
 * That is also the reason this is not vue-i18n: its `useI18n()` only works
 * inside a component, so every one of those modules would need the parallel
 * `global.t` API instead.
 *
 * Every dictionary is bundled eagerly. Lazy-loading a locale would be the
 * wrong trade for a tool that is driven live from a laptop: a language switch
 * halfway through a presentation must never show a loading state.
 */
import { computed, ref } from 'vue';
import en from './en.js';
import fr from './fr.js';
import fy from './fy.js';
import nl from './nl.js';

/**
 * De talen van de demo. Nederlands is de bron; de rest is er de vertaling van.
 *
 * Eén tabel, en elke plek die een taal moet kennen leest hieruit. Vóór deze
 * tabel stond een taalcode op negen plekken los in de code — in een `===`, in
 * een objectsleutel, in een bestandsnaam — en een derde taal toevoegen betekende
 * ze alle negen vinden. De plekken die je dan mist falen niet: `localeFromPath`
 * geeft gewoon `nl` terug voor een pad dat hij niet kent, en dan staat er een
 * Nederlandse UI onder een Fries adres zonder dat er iets stukgaat.
 *
 * - `prefix` is het URL-segment, leeg voor de bron: de Nederlandse paden zijn de
 *   originelen en houden hun kale vorm.
 * - `intl` is de BCP 47-tag waarmee elke `Intl`-formatter wordt gebouwd. Engels
 *   is `en-GB` en niet `en-US`: dat geeft "22 September 2026" en een 24-uurs
 *   klok, zoals een Nederlands overheidsscherm een datum noteert.
 * - `label` staat in de taal zelf. Een taalmenu dat "Dutch" zegt tegen wie geen
 *   Engels leest, helpt precies de persoon niet die het menu zoekt.
 */
export const LOCALES = [
  { code: 'nl', prefix: '', intl: 'nl-NL', label: 'Nederlands', dict: nl },
  { code: 'en', prefix: '/en', intl: 'en-GB', label: 'English', dict: en },
  // Fries is aanwezig maar nog niet vertaald: `fy.js` draagt voorlopig de
  // Nederlandse tekst. Zie de kop van dat bestand voor waarom daar geen
  // machinevertaling staat.
  { code: 'fy', prefix: '/fy', intl: 'fy-NL', label: 'Frysk', dict: fy },
  // Frans is vertaald door een taalmodel en nog niet nagekeken; zie
  // corpus/demo/i18n/REVIEW-fr.md.
  { code: 'fr', prefix: '/fr', intl: 'fr-FR', label: 'Français', dict: fr },
];

export const DEFAULT_LOCALE = 'nl';

/** Alleen de codes, voor waar een lijst strings handiger is dan de tabel. */
export const LOCALE_CODES = LOCALES.map((l) => l.code);

const BY_CODE = new Map(LOCALES.map((l) => [l.code, l]));

/**
 * De tabelregel van een taal, met de bron als terugval.
 *
 * Nooit `undefined`, zodat een aanroeper niet hoeft te controleren: een
 * onbekende code levert het Nederlands op, en dat is precies wat er moet
 * gebeuren bij een taal die niet (meer) bestaat.
 */
export function localeDef(code) {
  return BY_CODE.get(code) ?? BY_CODE.get(DEFAULT_LOCALE);
}

/** Of `code` een taal is die de demo kent. */
export function isLocale(code) {
  return BY_CODE.has(code);
}

// Same key the docs landing page uses. The two run on separate subdomains
// (separate ZAD components), so they do not in fact share storage; the name
// matches so that anyone reading both finds the same thing.
//
// Alleen geschreven, niet gelezen: het inline script in `index.html` leest hem
// vóór de app start, en alleen op de kale `/`. Een deeplink houdt de taal van
// zijn eigen adres, want een link die iemand doorstuurt hoort niet af te hangen
// van wat de ontvanger ooit in een menu koos.
const STORAGE_KEY = 'rr-lang';

const DICTS = Object.fromEntries(LOCALES.map((l) => [l.code, l.dict]));

const locale = ref(DEFAULT_LOCALE);

/** The active locale, for the plain modules that cannot call `useI18n()`. */
export function currentLocale() {
  return locale.value;
}

/**
 * The active locale as a ref, for a computed that has to re-run on a switch.
 *
 * `currentLocale()` does track when it is called inside a reactive effect —
 * it reads the same ref — but it reads as a plain function call, and a reader
 * checking whether some computed follows the language should not have to go
 * and look. This makes the dependency visible at the use site.
 */
export const activeLocale = computed(() => locale.value);

/**
 * The string for `key`, with `{placeholder}`s filled from `vars`.
 *
 * A key missing from the active locale falls back to Dutch, not to the key
 * itself: during a live demo a Dutch word among English ones is a blemish,
 * while `portaal.heading.lead` on screen is a visible defect. The fallback is
 * only safe because the parity test refuses to let it become a hiding place —
 * without that test this would quietly permit a half-translated app.
 */
export function t(key, vars) {
  return translate(locale.value, key, vars);
}

function translate(code, key, vars) {
  const s = DICTS[code]?.[key] ?? nl[key] ?? key;
  if (!vars) return s;
  return s.replace(/\{(\w+)\}/g, (whole, name) => (name in vars ? String(vars[name]) : whole));
}

/**
 * `t.plural(n, 'app.cases.pending')` → key `…​.one` or `…​.other`, with `{n}`.
 *
 * Both languages here are one/other, so this is a ternary rather than a plural
 * engine. The two forms exist on both sides even where one language does not
 * need the distinction (Dutch "1 te beoordelen" / "2 te beoordelen"), because
 * the key shape has to be the same in both dictionaries for the parity check.
 */
t.plural = (n, key, vars) => t(`${key}.${n === 1 ? 'one' : 'other'}`, { n, ...vars });

/**
 * Switch language and remember the choice.
 *
 * `document.documentElement.lang` is not decoration: screen readers pick their
 * voice from it, and it drives hyphenation. The one place that deliberately
 * overrides it back to `nl` is the pane showing verbatim statutory text.
 */
export function setLocale(next) {
  if (!isLocale(next)) return;
  locale.value = next;
  try {
    window.localStorage?.setItem(STORAGE_KEY, next);
  } catch {
    // Private mode, quota, storage disabled. The only consequence is that the
    // choice is not remembered on the next load.
  }
  if (typeof document !== 'undefined') document.documentElement.lang = next;
}

/**
 * Adopt a locale that came from the URL, without recording it as a choice.
 *
 * Opening a shared `/en/...` link shows English but must not overwrite the
 * reader's own preference — same rule the docs landing page follows, for the
 * same reason: a link someone sends you is not you picking a language.
 */
export function adoptLocale(next) {
  if (!isLocale(next) || locale.value === next) return;
  locale.value = next;
  if (typeof document !== 'undefined') document.documentElement.lang = next;
}

export function useI18n() {
  return { t, locale: computed(() => locale.value), setLocale };
}

/**
 * The viewer's own language while the demo itself runs in another.
 *
 * The recorded walkthrough plays in Dutch: the voice names the Dutch labels,
 * and the replay finds what to click by them. So the app switches to Dutch
 * for the length of the replay, but the player's controls around it stay in
 * the language the viewer came in with.
 */
const viewer = ref(null);

export function setViewerLocale(code) {
  viewer.value = isLocale(code) ? code : null;
}

/** Like `useI18n()`, in the viewer's language (see `setViewerLocale`). */
export function useViewerI18n() {
  const code = computed(() => viewer.value ?? locale.value);
  return { t: (key, vars) => translate(code.value, key, vars), locale: code };
}
