// Bewijst dat een variant die de gebruiker zelf bewaart ook echt door de
// engine gaat, en niet alleen in een lijstje staat.
//
// Waarom dit er moet zijn: "bewaar als variant" is het enige in deze demo dat
// werk vasthoudt. Als de bewaarde YAML wél in de opslag komt maar niet in de
// engine, ziet de gebruiker zijn variant in de keuzelijst staan terwijl de
// kolom ernaast met de oude wet rekent. Dat leest als "mijn wijziging doet
// niets", en dat is precies de verkeerde conclusie.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { readFileSync } from 'fs';
import { resolve } from 'path';
import { createEngineWithLaws, lawsDir } from './helpers/nodeEngine.js';
import { patchDefinitionValue } from '../src/lib/yamlPatch.js';

const LAW_PATH = 'laws/regulation/nl/wet/wet_studiefinanciering_2000/2025-01-01.yaml';

function verseOpslag() {
  const data = new Map();
  return {
    getItem: (k) => (data.has(k) ? data.get(k) : null),
    setItem: (k, v) => data.set(k, String(v)),
    removeItem: (k) => data.delete(k),
    get length() { return data.size; },
    key: (i) => [...data.keys()][i],
    _data: data,
  };
}

async function laadOpslag() {
  vi.resetModules();
  const stand = await import('../src/composables/useBewaardeStand.js');
  return { stand, varianten: await import('@regelrecht/frontend-shared/browserVariants') };
}

describe('een zelf bewaarde variant', () => {
  let opslag;

  beforeEach(() => {
    opslag = verseOpslag();
    vi.stubGlobal('localStorage', new Proxy(opslag, {
      ownKeys: () => [...opslag._data.keys()],
      getOwnPropertyDescriptor: () => ({ enumerable: true, configurable: true }),
    }));
    vi.stubGlobal('window', { location: { reload: vi.fn() } });
  });

  it('levert een wet op die de engine accepteert en die van de basis afwijkt', async () => {
    const { varianten } = await laadOpslag();
    const basisYaml = readFileSync(resolve(lawsDir, 'regulation/nl/wet/wet_studiefinanciering_2000/2025-01-01.yaml'), 'utf-8');

    // Dezelfde bewerking die het parameterpaneel maakt: één waarde in de YAML.
    const bewerkt = patchDefinitionValue(basisYaml, '6.10', 'voet_ratio_overig_sf35', 1.2);
    expect(bewerkt).not.toBe(basisYaml);

    const variant = varianten.saveVariant({
      title: 'Voet naar 120%',
      files: [{ path: LAW_PATH, yaml: bewerkt, original: basisYaml }],
    });

    // Wat de lawStore doet bij het activeren: de bewaarde tekst over de basis
    // heen laden. Zelfde $id, dus dit vervangt de basisversie.
    const tekst = varianten.browserVariantYaml(variant.id, LAW_PATH);
    expect(tekst).toBe(bewerkt);

    // De engine moet deze tekst aannemen. Een bewaarde variant die de engine
    // weigert, zou pas bij het activeren stukgaan, en dan is het werk al weg.
    const engine = await createEngineWithLaws();
    expect(() => engine.loadLaw(tekst)).not.toThrow();

    // En de wijziging staat er ook echt in, niet alleen in de titel.
    expect(tekst).toMatch(/voet_ratio_overig_sf35[\s\S]{0,120}?1\.2/);
    expect(tekst).not.toBe(basisYaml);
  });

  it('draagt de basistekst mee zodat corpus-drift op te merken is', async () => {
    const { varianten } = await laadOpslag();
    const basisYaml = 'articles: []\n';

    const v = varianten.saveVariant({
      title: 'Met uitgangspunt',
      files: [{ path: LAW_PATH, yaml: 'articles: [{}]\n', original: basisYaml }],
    });

    expect(varianten.checkDrift(v.id, () => basisYaml).drifted).toBe(false);
    expect(varianten.checkDrift(v.id, () => 'articles: [{iets: anders}]\n').drifted).toBe(true);
  });

  it('overleeft een ververs met zijn YAML erin', async () => {
    const eerste = await laadOpslag();
    eerste.varianten.saveVariant({
      title: 'Blijft',
      files: [{ path: LAW_PATH, yaml: 'articles: []\n', original: 'articles: []\n' }],
    });

    const opnieuw = await laadOpslag();
    const [v] = opnieuw.varianten.browserVariants();

    expect(v).toBeTruthy();
    expect(opnieuw.varianten.browserVariantYaml(v.id, LAW_PATH)).toBe('articles: []\n');
  });
});
