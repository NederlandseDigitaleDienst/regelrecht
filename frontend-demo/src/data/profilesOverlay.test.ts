/**
 * De Engelse persona's verschillen alléén in hun beschrijving.
 *
 * Dit is de test die ertoe doet, en niet de vraag of er Engels staat.
 * `profiles.yaml` is niet alleen schermtekst: het is de invoer van de wet. De
 * engine leest er zijn tabellen uit (`useDemoEngine.ts`), de simulatie bouwt er
 * zijn populatie mee (`population.ts`), en de materialisatie registreert de
 * brongegevens eruit (`materialize.js`). Een overlay die per ongeluk een
 * geboortedatum, een inkomen of een BSN raakt, laat de demo in het Engels iets
 * anders uitrekenen dan in het Nederlands.
 *
 * Dat zou niemand opvallen: er komt gewoon een ander bedrag uit, en er is geen
 * scherm dat de twee talen naast elkaar zet. Vandaar deze test, die de twee
 * documenten ontdoet van hun beschrijving en daarna eist dat er niets meer over
 * is om over te verschillen.
 *
 * De test leest het corpus en niet `public/data/`. Die map is het resultaat van
 * `copy-demo-corpus.mjs`, dat aan `predev`/`prebuild` hangt en niet aan `test`:
 * in CI draait `npm test -w frontend-demo` zonder build, en een test die daar
 * overslaat bewaakt niets op de enige plek waar het moet. Hij past de overlay
 * hier dus zelf toe, op dezelfde manier als het script.
 */
import { readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as yaml from 'js-yaml';
import { describe, expect, it } from 'vitest';
import type { ProfilesDocument } from './loadCorpus';

const here = dirname(fileURLToPath(import.meta.url));
const demoDir = resolve(here, '..', '..', '..', 'corpus', 'demo');

const nl = yaml.load(readFileSync(join(demoDir, 'profiles.yaml'), 'utf8')) as ProfilesDocument;
/** dotted path -> translated text */
const overlay = (yaml.load(readFileSync(join(demoDir, 'i18n', 'en.yaml'), 'utf8')) ?? {}) as Record<string, unknown>;

/** A node of the YAML document while walking a path; any object can be indexed. */
type Node = Record<string, unknown>;

/**
 * De overlay toegepast, zoals `copy-demo-corpus.mjs` het doet.
 *
 * Alleen de paden die in dit document bestaan: de rest van `en.yaml` gaat over
 * `demo-config.yaml`. Een pad dat in geen van beide bestaat wordt daar door de
 * build afgevangen, en dat hoeft deze test niet over te doen.
 */
function englishProfiles(): ProfilesDocument {
  const doc = structuredClone(nl);
  for (const [path, value] of Object.entries(overlay)) {
    const parts = path.split('.');
    let cur: unknown = doc;
    for (const part of parts.slice(0, -1)) {
      if (cur === null || cur === undefined) break;
      cur = (cur as Node)[part];
    }
    // `split` always yields at least one part.
    const last = parts.at(-1) as string;
    if (cur !== null && cur !== undefined && (cur as Node)[last] !== undefined) (cur as Node)[last] = value;
  }
  return doc;
}

const en = englishProfiles();

describe("de Engelse persona's", () => {
  /** Hetzelfde document zonder de beschrijvingen: dit is wat gelijk moet zijn. */
  function withoutDescriptions(doc: ProfilesDocument): ProfilesDocument {
    const copy = structuredClone(doc);
    for (const profile of Object.values(copy.profiles ?? {})) delete profile.description;
    return copy;
  }

  it('dragen precies dezelfde gegevens als het Nederlands', () => {
    expect(withoutDescriptions(en)).toEqual(withoutDescriptions(nl));
  });

  it('houden elke persona, op dezelfde sleutel', () => {
    expect(Object.keys(en.profiles)).toEqual(Object.keys(nl.profiles));
  });

  it('houden de BSN als tekst en niet als getal', () => {
    // Een BSN mag met een nul beginnen. Wordt de sleutel een getal, dan valt die
    // nul weg en vindt geen enkele opzoeking de persoon nog.
    for (const key of Object.keys(en.profiles)) expect(typeof key).toBe('string');
  });

  it('hebben overal een beschrijving, en nergens meer de Nederlandse', () => {
    for (const [bsn, profile] of Object.entries(en.profiles)) {
      const original = nl.profiles[bsn];
      expect(original, `persona ${bsn} bestaat niet in het Nederlands`).toBeDefined();
      expect(profile.description, `persona ${bsn} heeft geen Engelse beschrijving`).toBeTruthy();
      expect(profile.description, `persona ${bsn} staat nog in het Nederlands`).not.toBe(
        original?.description,
      );
    }
  });

  it('laten de namen staan', () => {
    // Een naam is een naam. Vertalen zou van `Merijn van der Meer` iets maken
    // waar de rest van de demo niet meer naar kan verwijzen.
    for (const [bsn, profile] of Object.entries(en.profiles)) {
      const original = nl.profiles[bsn];
      expect(original, `persona ${bsn} bestaat niet in het Nederlands`).toBeDefined();
      expect(profile.name).toBe(original?.name);
    }
  });
});
