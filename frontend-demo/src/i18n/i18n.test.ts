/**
 * What keeps the two dictionaries honest.
 *
 * The demo is plain JavaScript, so there is no type checker to notice a
 * missing key the way `LandingContent` does for the docs landing page. These
 * tests are that backstop — and they go one step further, because a type
 * system cannot see a translation that is merely *out of date*.
 */
import { describe, expect, it } from 'vitest';
import enMessages from './en.js';
import nlMessages from './nl.js';
import sourceHashes from './en.sources.js';
import { LOCALES } from './index.js';
import { hash } from './hash.js';
import { FEATURES } from '../store/demoStore';
import { DEFAULT_LOCALE, LOCALE_CODES, adoptLocale, currentLocale, setLocale, t } from './index.js';

/** A dictionary as a string map: these tests index one with keys taken from another. */
type Dict = Record<string, string | undefined>;
const en: Dict = enMessages;
const nl: Dict = nlMessages;
const sources: Dict = sourceHashes;

/**
 * Keys whose English is legitimately identical to the Dutch: proper names, and
 * the language menu, which names each language in that language.
 */
const IDENTICAL_BY_DESIGN = new Set([
  'app.tabs.home',
  'app.demo.label',
  'app.features.label',
  // "item" and "items" happen to be the same word in both languages. Both
  // forms still exist on both sides, because the key shape has to match.
  'format.items.one',
  'format.items.other',
  // Zelfde woord in beide talen. 'Curator' en 'mentor' zijn allebei juridische
  // termen die het Engels uit het Latijn heeft, net als het Nederlands.
  'sim.dimension.partner',
  'delegation.type.curator',
  'delegation.type.mentor',
  // 'Trace' is in beide talen de term voor wat de engine uitschrijft.
  'scenario.trace',
  // Het Britse "postcode" is hetzelfde woord als het Nederlandse.
  'sheet.change.field.postcode',
  // Een bestandsformaat en een domeinnaam; die hebben geen vertaling.
  'wet.view.raw',
  'wet.source.link',
  // Alleen opmaak: de woorden zitten in de waarden die erin komen, niet in het
  // sjabloon zelf. Er valt hier niets te vertalen.
  'wet.tree.law_state',
  'wet.tile.outcome.missing',
]);

/** The `{placeholder}` names in a string, sorted. */
function placeholders(s: unknown) {
  return [...String(s).matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
}

/**
 * Hoeveel van de sleutels in een taal nog letterlijk het Nederlands mogen zijn.
 *
 * Voor het Engels is dat nul: elke uitzondering staat met reden in
 * `IDENTICAL_BY_DESIGN`, en die lijst is kort genoeg om te lezen.
 *
 * Voor het Fries werkt diezelfde aanpak niet. Fries en Nederlands delen een
 * groot deel van hun woordenschat, dus een vertaalde demo houdt legitiem
 * tientallen tot honderden identieke sleutels over. Een uitzonderingenlijst zou
 * van 19 naar 200+ groeien en dan een lijst zijn die zichzelf vult: niemand
 * leest hem meer na, en hij bewaakt niets.
 *
 * Een vastgelegd getal doet dat wel. Het meet niet of de vertaling goed is (dat
 * kan geen test), maar het maakt van "er blijft steeds meer Nederlands staan"
 * een zichtbare gebeurtenis. Het getal gaat met de hand omlaag naarmate er
 * vertaald is, met de reden in de commit.
 *
 * Voor `fy` staat het op 0,06. Gemeten is 5,32%: dat zijn woorden die in het
 * Fries werkelijk hetzelfde zijn ("Ja", "Nee", "Titel", "Adres", "Totaal",
 * "Seed", "Mediaan"), en de vertalers is per sleutel gevraagd of dat klopte.
 * De drempel ligt er net boven, zodat een enkele nieuwe sleutel die nog
 * onvertaald binnenkomt niet meteen de build breekt, maar een terugval van
 * tientallen sleutels wel.
 */
const MAX_IDENTICAL_SHARE: Record<string, number> = { en: 0, fy: 0.06 };

/** De vertaalde talen: alles behalve de bron. */
const TRANSLATED: { code: string; dict: Dict }[] = LOCALES.filter((l) => l.code !== 'nl');

describe('i18n parity', () => {
  it.each(TRANSLATED)('$code has a translation for every Dutch key', ({ dict }) => {
    expect(Object.keys(nl).filter((k) => !(k in dict))).toEqual([]);
  });

  it.each(TRANSLATED)('$code carries no key Dutch does not have', ({ dict }) => {
    // A key renamed on the Dutch side and left behind here would otherwise sit
    // in the file forever, looking translated and reaching nothing.
    expect(Object.keys(dict).filter((k) => !(k in nl))).toEqual([]);
  });

  it('no English string is still the untranslated Dutch', () => {
    const same = Object.keys(nl).filter((k) => nl[k] === en[k] && !IDENTICAL_BY_DESIGN.has(k));
    expect(same).toEqual([]);
  });

  it.each(TRANSLATED)('$code keeps its share of untranslated strings within bounds', ({ code, dict }) => {
    // De sleutels die per definitie gelijk zijn tellen niet mee: die zijn geen
    // onvertaald Nederlands maar een woord dat in beide talen hetzelfde is.
    const keys = Object.keys(nl).filter((k) => !IDENTICAL_BY_DESIGN.has(k));
    const share = keys.filter((k) => nl[k] === dict[k]).length / keys.length;
    const max = MAX_IDENTICAL_SHARE[code] ?? 0;
    expect(
      share,
      `${code}: ${(share * 100).toFixed(1)}% van de sleutels is nog letterlijk het Nederlands, ` +
        `en dat mag hoogstens ${(max * 100).toFixed(1)}% zijn. Vertaal verder, of verlaag ` +
        'MAX_IDENTICAL_SHARE met een reden als er juist vertaald is.',
    ).toBeLessThanOrEqual(max);
  });

  it('every key listed as identical by design really is identical', () => {
    // Keeps the exception list from outliving its reason: translate one of
    // these later and the entry has to go, rather than silently excusing it.
    expect([...IDENTICAL_BY_DESIGN].filter((k) => nl[k] !== en[k])).toEqual([]);
  });

  it.each(TRANSLATED)('$code keeps every placeholder', ({ dict }) => {
    // A dropped `{n}` renders "to review" instead of "3 to review", and does
    // so silently.
    for (const key of Object.keys(nl)) {
      expect(placeholders(dict[key]), key).toEqual(placeholders(nl[key]));
    }
  });

  it('no translation is stale', () => {
    const stale = Object.keys(nl).filter((k) => sources[k] !== hash(nl[k]));
    expect(
      stale,
      'De Nederlandse tekst is gewijzigd sinds deze vertaald zijn. Vertaal opnieuw en draai `node scripts/i18n-bless.mjs`.',
    ).toEqual([]);
  });

  it('the source hashes cover exactly the Dutch keys', () => {
    expect(Object.keys(sources).sort()).toEqual(Object.keys(nl).sort());
  });
});

describe('the feature flags', () => {
  it('every flag has a label in both dictionaries', () => {
    // App.vue renders these as `t(`app.features.${f.key}`)`, a template key
    // that check-i18n.mjs cannot see. A flag added to the store without a
    // label would render its own key in the menu.
    for (const f of FEATURES) {
      expect(nl[`app.features.${f.key}`], f.key).toBeTypeOf('string');
      expect(en[`app.features.${f.key}`], f.key).toBeTypeOf('string');
    }
  });

  it('carries no Dutch label of its own any more', () => {
    // The dictionaries are the single source; a `label` here would sit next to
    // them looking authoritative while nothing read it.
    // `FeatureFlag` has no `label`; this pins that the objects carry none either.
    for (const f of FEATURES) expect((f as { label?: unknown }).label, f.key).toBeUndefined();
  });
});

describe('t', () => {
  it('returns the Dutch string by default', () => {
    expect(currentLocale()).toBe(DEFAULT_LOCALE);
    expect(t('app.tabs.wetten')).toBe('Regelwerken');
  });

  it('returns the English string once switched', () => {
    setLocale('en');
    expect(t('app.tabs.wetten')).toBe('Ruleworks');
    setLocale('nl');
  });

  it('falls back to Dutch for a key the locale lacks', () => {
    // Not to the key itself: a Dutch word among English ones is a blemish,
    // `app.tabs.wetten` on screen is a defect.
    setLocale('en');
    expect(t('app.tabs.home')).toBe('Home');
    setLocale('nl');
  });

  it('returns the key when nothing has it', () => {
    expect(t('does.not.exist')).toBe('does.not.exist');
  });

  it('fills placeholders', () => {
    expect(t('app.cases.pending.other', { n: 3 })).toBe('3 te beoordelen');
  });

  it('leaves an unknown placeholder alone rather than blanking it', () => {
    expect(t('app.cases.pending.other', { other: 1 })).toBe('{n} te beoordelen');
  });

  it('picks the plural form by count', () => {
    setLocale('en');
    expect(t.plural(1, 'app.cases.pending')).toBe('1 to review');
    expect(t.plural(4, 'app.cases.pending')).toBe('4 to review');
    setLocale('nl');
  });

  it('refuses a locale it does not have', () => {
    setLocale('de');
    expect(currentLocale()).toBe('nl');
  });
});

describe('adoptLocale', () => {
  it('switches without recording a choice', () => {
    // A shared `/en/...` link shows English but must not overwrite the
    // reader's own preference.
    window.localStorage.removeItem('rr-lang');
    adoptLocale('en');
    expect(currentLocale()).toBe('en');
    expect(window.localStorage.getItem('rr-lang')).toBe(null);
    adoptLocale('nl');
  });

  it('ignores a locale it does not have', () => {
    adoptLocale('xx');
    expect(LOCALE_CODES).not.toContain('xx');
    expect(currentLocale()).toBe('nl');
  });
});
