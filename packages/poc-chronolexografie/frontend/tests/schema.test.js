import { readdirSync, readFileSync, statSync } from 'fs';
import { join, relative } from 'path';
import { fileURLToPath } from 'url';
import { describe, expect, it } from 'vitest';
import { createSSRApp, h } from 'vue';
import { renderToString } from 'vue/server-renderer';
import SchemaOverzicht from '../src/presentation/SchemaOverzicht.vue';
import { LIJNSOORTEN, andereStap, keten, sleutel, stapBeeld, stapZin } from '../src/presentation/schema.js';
import { CORPUS } from '../src/corpus.js';
import { keten as ketenUitleg } from '../src/presentation/uitleg.js';

const repo = fileURLToPath(new URL('../../../../', import.meta.url));

/** The YAML files of the corpus of the poc, per path, as the bundle has them (scripts/copy-assets.js). */
function bundel() {
  const alle = {};
  const lees = (dir) => {
    for (const e of readdirSync(dir)) {
      const pad = join(dir, e);
      if (statSync(pad).isDirectory()) {
        if (e !== 'scenarios') lees(pad);
      } else if (pad.endsWith('.yaml')) {
        alle[relative(repo, pad).split('\\').join('/')] = readFileSync(pad, 'utf8');
      }
    }
  };
  lees(join(repo, CORPUS));
  return alle;
}
const alle = bundel();
const map = (pad) => pad.split('/').at(-2);
const heeft = (s, soort, van, naar) =>
  s.lijnen.some((l) => l.soort === soort && map(l.van.pad) === van[0] && l.van.begrip.endsWith(`#${van[1]}`) && map(l.naar.pad) === naar[0] && l.naar.begrip.endsWith(`#${naar[1]}`));

describe('the picture of one value', () => {
  const k = keten(alle, 'bedrijfsparkeervergunning', ketenUitleg.start);
  const namen = (pad) => k.bestanden.find((b) => map(b.pad) === pad)?.begrippen.map((g) => g.naam) ?? [];

  it('starts and ends every line at a name in a box', () => {
    const namen = new Set(k.bestanden.flatMap((b) => b.begrippen.map((g) => `${b.pad}|${g.key}`)));
    expect(k.lijnen.length).toBeGreaterThan(0);
    for (const l of k.lijnen) {
      expect(namen.has(`${l.van.pad}|${l.van.begrip}`), JSON.stringify(l)).toBe(true);
      expect(namen.has(`${l.naar.pad}|${l.naar.begrip}`), JSON.stringify(l)).toBe(true);
      expect(LIJNSOORTEN.map((s) => s.soort)).toContain(l.soort);
    }
    // Every name in a box has a line: a box shows only what connects.
    const verbonden = new Set(k.lijnen.flatMap((l) => [`${l.van.pad}|${l.van.begrip}`, `${l.naar.pad}|${l.naar.begrip}`]));
    for (const n of namen) expect(verbonden.has(n), n).toBe(true);
  });

  it('draws a box per file, with its names', async () => {
    const html = await renderToString(createSSRApp({ render: () => h(SchemaOverzicht, { schema: k, label: 'x' }) }));
    expect(html.match(/<nldd-box/g)).toHaveLength(k.bestanden.length);
    expect(html.match(/data-begrip=/g)).toHaveLength(k.bestanden.reduce((n, b) => n + b.begrippen.length, 0));
    // The title, not the name of the directory.
    expect(html).not.toMatch(/<code[^>]*>parkeerverordening_gemeente_wilgendaal</);
  });

  it('starts at the open term of the regulation and marks it', () => {
    expect(k.doel.begrip).toBe('2#open_term#maximum_bedrijfsparkeervergunningen_bereikt');
    expect(k.lijnen.some((l) => l.naar.begrip === k.doel.begrip && l.soort === 'vult_in')).toBe(true);
  });

  it('follows the value back to the chronicles of both cells', () => {
    // The count of permits: the municipality's own decisions, with the Awb.
    expect(heeft(k, 'kroniek', ['parkeerverordening_gemeente_wilgendaal', 'besluiten onder artikel 2'], ['reductie_algemene_wet_bestuursrecht', 'besluiten'])).toBe(true);
    expect(heeft(k, 'vraagt', ['gemeente_wilgendaal', 'aantal_geldige_bedrijfsparkeervergunningen'], ['beleid_bedrijfsparkeervergunning_gemeente_wilgendaal', 'antwoord_aantal_geldig'])).toBe(true);
    // The maximum: where the firm is and how many work there, from the KvK.
    expect(heeft(k, 'kroniek', ['beleid_handelsregister_kamer_van_koophandel', 'vestiging_ingeschreven'], ['reductie_vestiging_in_handelsregister', 'inschrijvingen'])).toBe(true);
    expect(heeft(k, 'rekent', ['uitvoeringsbesluit_parkeren_gemeente_wilgendaal', 'vergunninggebied_van_postcode'], ['uitvoeringsbesluit_parkeren_gemeente_wilgendaal', 'vergunninggebied_van_vestiging'])).toBe(true);
  });

  it('leaves out what the value does not need, and the parameters', () => {
    expect(namen('parkeerverordening_gemeente_wilgendaal')).not.toContain('bedrijf_gevestigd_in_vergunninggebied');
    expect(k.bestanden.flatMap((b) => b.begrippen).some((g) => g.rol === 'parameter' || g.naam === 'kvk_nummer')).toBe(false);
  });

  it('shows an output that only passes on an input as one name', () => {
    // Policy art. 1 takes the answers of the KvK as they are.
    expect(namen('beleid_bedrijfsparkeervergunning_gemeente_wilgendaal')).not.toContain('antwoord_aantal_werkzame_personen');
    expect(namen('beleid_bedrijfsparkeervergunning_gemeente_wilgendaal')).not.toContain('antwoord_postcode');
    expect(heeft(k, 'vraagt', ['kamer_van_koophandel', 'aantal_werkzame_personen'], ['beleid_bedrijfsparkeervergunning_gemeente_wilgendaal', 'aantal_werkzame_personen_volgens_handelsregister'])).toBe(true);
    // An input that only takes an output of its own file is that output.
    const besluit = 'uitvoeringsbesluit_parkeren_gemeente_wilgendaal';
    expect(namen(besluit)).not.toContain('vergunninggebied');
    expect(heeft(k, 'rekent', [besluit, 'vergunninggebied_van_vestiging'], [besluit, 'maximum_bedrijfsparkeervergunningen'])).toBe(true);
    // An output that computes something keeps its input.
    expect(namen('beleid_bedrijfsparkeervergunning_gemeente_wilgendaal')).toContain('antwoord_aantal_geldig');
  });

  it('shows of a reduction only what it reads and what it answers', () => {
    const awb = 'reductie_algemene_wet_bestuursrecht';
    expect(namen(awb)).not.toContain('vernietigde_besluiten');
    expect(heeft(k, 'rekent', [awb, 'besluiten'], [awb, 'geldende_besluiten'])).toBe(true);
    // The same input in two articles is one name, with one line to the chronicle.
    expect(namen(awb).filter((n) => n === 'uitspraken')).toHaveLength(1);
    expect(k.lijnen.filter((l) => l.soort === 'kroniek' && l.naar.begrip.endsWith('#uitspraken'))).toHaveLength(1);
    // A law keeps its steps in between.
    expect(namen('uitvoeringsbesluit_parkeren_gemeente_wilgendaal')).toContain('maximum_bedrijfsparkeervergunningen');
  });

  describe('its steps', () => {
    const alleNamen = k.bestanden.flatMap((b) => b.begrippen.map((g) => ({ ...g, pad: b.pad, map: map(b.pad) })));
    const stapVan = (bestand, naam) => alleNamen.find((g) => g.map === bestand && g.naam === naam).stap;
    const eind = (e) => sleutel(e.pad, e.begrip);

    it('gives every name a step, one after the last name that uses it', () => {
      const stap = new Map(alleNamen.map((g) => [sleutel(g.pad, g.key), g.stap]));
      for (const n of stap.values()) expect(n).toBeGreaterThanOrEqual(1);
      for (const l of k.lijnen) expect(stap.get(eind(l.van))).toBeGreaterThan(stap.get(eind(l.naar)));
      for (const [naam, n] of stap) {
        const gebruikers = k.lijnen.filter((l) => eind(l.van) === naam).map((l) => stap.get(eind(l.naar)));
        expect(n).toBe(1 + Math.max(0, ...gebruikers));
      }
    });

    it('starts at the value itself, alone', () => {
      const eerste = alleNamen.filter((g) => g.stap === 1);
      expect(eerste.map((g) => sleutel(g.pad, g.key))).toEqual([eind(k.doel)]);
    });

    it('ends at sources, which take nothing in', () => {
      const laatste = alleNamen.filter((g) => g.stap === k.stappen);
      expect(laatste.length).toBeGreaterThan(0);
      for (const g of laatste) expect(k.lijnen.some((l) => eind(l.naar) === sleutel(g.pad, g.key))).toBe(false);
      expect(stapVan('algemene_wet_bestuursrecht', 'afschrift_van_de_uitspraak')).toBeGreaterThan(stapVan('reductie_algemene_wet_bestuursrecht', 'uitspraken'));
    });

    it('gives names that are needed at the same moment the same step', () => {
      const awb = 'reductie_algemene_wet_bestuursrecht';
      expect(stapVan(awb, 'besluiten')).toBe(stapVan(awb, 'uitspraken'));
      expect(stapVan(awb, 'besluiten')).toBe(stapVan(awb, 'beslissingen_op_bezwaar'));
    });

    it('walks from the whole picture to the end and stops there', () => {
      expect(andereStap(null, 1, k.stappen)).toBe(1);
      expect(andereStap(1, 1, k.stappen)).toBe(2);
      expect(andereStap(k.stappen, 1, k.stappen)).toBe(k.stappen);
      expect(andereStap(2, -1, k.stappen)).toBe(1);
      expect(andereStap(1, -1, k.stappen)).toBe(1);
    });

    it('keeps what has been, adds the names of the step, and ends with everything', () => {
      let eerder = new Set();
      for (let n = 1; n <= k.stappen; n++) {
        const { nu, gehad } = stapBeeld(k, n);
        expect(nu.size).toBeGreaterThan(0);
        expect([...gehad].sort()).toEqual([...eerder, ...nu].sort());
        // A line out of this step goes to what asked for it, which has been.
        for (const l of k.lijnen.filter((x) => nu.has(eind(x.van)))) expect(eerder.has(eind(l.naar))).toBe(true);
        eerder = gehad;
      }
      expect(eerder.size).toBe(alleNamen.length);
    });

    it('asks the question first, in the words of the law', () => {
      expect(stapZin(k, null)).toBe(`Het hele schema, in ${k.stappen} stappen.`);
      expect(stapZin(k, 1)).toBe(
        `Stap 1 van ${k.stappen}, de vraag: of het bedrijf op de dag van het besluit al het maximum aantal geldige bedrijfsparkeervergunningen heeft dat het college vaststelt?`,
      );
    });

    it('says what a step needs and where it comes from, briefly', () => {
      const zinnen = Array.from({ length: k.stappen - 1 }, (_, i) => stapZin(k, i + 2));
      for (const z of zinnen.slice(0, -1)) expect(z).toContain(', daarvoor is nodig: ');
      expect(zinnen.at(-1)).toContain(', hier komt de keten bij de bron uit: ');
      for (const z of zinnen) {
        expect(z.length).toBeLessThanOrEqual(150);
        expect(z).not.toContain('_');
        expect(z.match(/\.$/g)).toHaveLength(1);
      }
      // Every kind of origin in this chain comes up.
      const alles = zinnen.join(' ');
      for (const waar of ['uit hetzelfde regelwerk', 'uit een andere regeling', 'aan een cel', 'van een reductie', 'uit de kroniek']) expect(alles).toContain(waar);
    });

    it('counts instead of naming when a sentence would run too long', () => {
      const naam = 'een_naam_die_niet_kort_is_en_die_de_zin_lang_genoeg_maakt';
      const bestanden = [{ pad: 'a', soort: 'wet', begrippen: [1, 2, 3, 4].map((i) => ({ key: `k${i}`, naam: `${naam}_${i}`, stap: 2 })) }];
      const lijnen = [1, 2, 3, 4].map((i) => ({ soort: 'kroniek', van: { pad: 'a', begrip: `k${i}` }, naar: { pad: 'a', begrip: 'k' } }));
      expect(stapZin({ bestanden, lijnen, stappen: 3 }, 2)).toBe('Stap 2 van 3, daarvoor is nodig: 4 bronnen uit de kroniek.');
    });

    it('draws the steps and the buttons', async () => {
      const html = await renderToString(createSSRApp({ render: () => h(SchemaOverzicht, { schema: k, label: 'x' }) }));
      expect(html.match(/<nldd-badge/g)).toHaveLength(alleNamen.length);
      expect(html).toContain(`Stap ${k.stappen}; naar`);
      for (const knop of ['Vorige stap', 'Volgende stap', 'Toon alles']) expect(html).toContain(`text="${knop}"`);
      expect(html).toContain('aria-live="polite"');
      expect(html).toContain(stapZin(k, null));
    });
  });

  it('puts a name before what is computed from it', () => {
    const n = namen('uitvoeringsbesluit_parkeren_gemeente_wilgendaal');
    expect(n.indexOf('maximum_bedrijfsparkeervergunningen')).toBeLessThan(n.indexOf('maximum_bedrijfsparkeervergunningen_bereikt'));
  });
});
