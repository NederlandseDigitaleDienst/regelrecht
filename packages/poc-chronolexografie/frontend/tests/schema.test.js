import { describe, expect, it } from 'vitest';
import { createSSRApp, h } from 'vue';
import { renderToString } from 'vue/server-renderer';
import SchemaOverzicht from '../src/presentation/SchemaOverzicht.vue';
import { KRONIEK, LIJNSOORTEN, SOORTEN, andereStap, genummerd, keten, sleutel, stapBeeld, stapZin } from '../src/presentation/schema.js';
import { keten as ketenUitleg } from '../src/presentation/uitleg.js';
import { bundel } from './bundel.js';

const alle = bundel();
const map = (pad) => (pad === KRONIEK ? KRONIEK : pad.split('/').at(-2));
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
    expect(heeft(k, 'kroniek', [KRONIEK, 'parkeerverordening_gemeente_wilgendaal#2#'], ['reductie_algemene_wet_bestuursrecht', 'kroniek'])).toBe(true);
    // A permit revoked at the holder's request (art. 5) does not count.
    expect(heeft(k, 'kroniek', [KRONIEK, 'parkeerverordening_gemeente_wilgendaal#5#'], ['reductie_algemene_wet_bestuursrecht', 'kroniek'])).toBe(true);
    expect(heeft(k, 'vraagt', ['gemeente_wilgendaal', 'aantal_geldige_bedrijfsparkeervergunningen'], ['beleid_bedrijfsparkeervergunning_gemeente_wilgendaal', 'antwoord_aantal_geldig'])).toBe(true);
    // The maximum: where the firm is and how many work there, from the KvK.
    expect(heeft(k, 'kroniek', [KRONIEK, 'beleid_handelsregister_kamer_van_koophandel#1#vestiging_ingeschreven'], ['reductie_vestiging_in_handelsregister', 'inschrijvingen'])).toBe(true);
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
    expect(heeft(k, 'rekent', [awb, 'kroniek'], [awb, 'geldende_besluiten'])).toBe(true);
    // One input reads the chronicle in its order (RFC-056), with a line for
    // each read it holds: the decisions under art. 2 and art. 5, those on
    // objection and the judgments.
    expect(namen(awb).filter((n) => n === 'kroniek')).toHaveLength(1);
    expect(k.lijnen.filter((l) => l.soort === 'kroniek' && l.naar.begrip.endsWith('#kroniek'))).toHaveLength(4);
    // A law keeps its steps in between.
    expect(namen('uitvoeringsbesluit_parkeren_gemeente_wilgendaal')).toContain('maximum_bedrijfsparkeervergunningen');
  });

  describe('its steps', () => {
    const alleNamen = k.bestanden.flatMap((b) => b.begrippen.map((g) => ({ ...g, pad: b.pad, map: map(b.pad) })));
    const stapVan = (bestand, naam) => alleNamen.find((g) => g.map === bestand && g.naam === naam).stap;
    const bron = (vorm) => alleNamen.find((g) => g.map === KRONIEK && g.vorm.startsWith(vorm));
    const eind = (e) => sleutel(e.pad, e.begrip);

    const beleid = 'beleid_bedrijfsparkeervergunning_gemeente_wilgendaal';
    const besluit = 'uitvoeringsbesluit_parkeren_gemeente_wilgendaal';

    it('gives every name a step', () => {
      for (const g of alleNamen) {
        expect(g.stap).toBeGreaterThanOrEqual(1);
        expect(g.stap).toBeLessThanOrEqual(k.stappen);
      }
    });

    it('starts at the value itself, alone', () => {
      const eerste = alleNamen.filter((g) => g.stap === 1);
      expect(eerste.map((g) => sleutel(g.pad, g.key))).toEqual([eind(k.doel)]);
    });

    it('takes the names of an article that compute together as one step', () => {
      const stap = stapVan(beleid, 'aantal_geldige_bedrijfsparkeervergunningen');
      expect(stapVan(beleid, 'antwoord_aantal_geldig')).toBe(stap);
      expect(stapVan(beleid, 'aantal_geldige_bedrijfsparkeervergunningen_volgens_college')).toBe(stap);
      expect(alleNamen.filter((g) => g.stap === stap)).toHaveLength(3);
    });

    it('takes a reduction as one step, also over its articles', () => {
      const awb = 'reductie_algemene_wet_bestuursrecht';
      const stap = stapVan(awb, 'geldende_besluiten');
      expect(stapVan(awb, 'kroniek')).toBe(stap);
    });

    it('does not take a source into the group of its article', () => {
      expect(bron('Parkeerverordening').stap).toBeGreaterThan(1);
    });

    it('walks the longest path, on a tie the one with the most sources', () => {
      const pad = [beleid, 'gemeente_wilgendaal', 'reductie_geldige_bedrijfsparkeervergunningen', 'reductie_algemene_wet_bestuursrecht'];
      const stappen = pad.map((b) => alleNamen.find((g) => g.map === b && g.stap < k.reststap)?.stap);
      expect(stappen).toEqual([3, 4, 5, 6]);
    });

    it('ends the path at its sources, together', () => {
      const laatste = alleNamen.filter((g) => g.stap === k.reststap - 1);
      expect(laatste.map((g) => `${g.naam} · ${g.vorm}`).sort()).toEqual([
        'afschrift_van_de_uitspraak · Algemene wet bestuursrecht art. 8:79',
        'beschikking · Algemene wet bestuursrecht art. 7:11',
        'beschikking · Parkeerverordening Wilgendaal 2026 (fictief) art. 2',
        'beschikking · Parkeerverordening Wilgendaal 2026 (fictief) art. 5',
      ]);
      for (const g of laatste) expect(k.lijnen.some((l) => eind(l.naar) === sleutel(g.pad, g.key))).toBe(false);
    });

    it('shows all the rest in one last step', () => {
      expect(k.reststap).toBe(k.stappen);
      expect(stapVan(beleid, 'aantal_werkzame_personen_volgens_handelsregister')).toBe(k.reststap);
      expect(stapVan(beleid, 'postcode_vestiging_volgens_handelsregister')).toBe(k.reststap);
      expect(stapVan(besluit, 'vergunninggebied_van_postcode')).toBe(k.reststap);
      expect(stapVan(besluit, 'vergunninggebied_van_vestiging')).toBe(k.reststap);
      expect(stapVan('kamer_van_koophandel', 'postcode_vestiging')).toBe(k.reststap);
      expect(stapVan('reductie_vestiging_in_handelsregister', 'inschrijvingen')).toBe(k.reststap);
      expect(stapVan(KRONIEK, 'vestiging_ingeschreven')).toBe(k.reststap);
    });

    it('takes, of two paths as long, the one with the most sources behind it', () => {
      // doel <- a <- (x, y)   and   doel <- b <- z: as long, but a has two sources.
      const naam = (k) => ({ key: k, naam: k });
      const bestanden = [
        { pad: 'w', soort: 'wet', begrippen: ['doel', 'b', 'a'].map(naam) },
        { pad: 'k', soort: 'kroniek', begrippen: ['z', 'x', 'y'].map(naam) },
      ];
      const lijn = (van, naar) => ({ soort: 'leest', van: { pad: van[0], begrip: van[1] }, naar: { pad: naar[0], begrip: naar[1] } });
      const lijnen = [lijn(['w', 'b'], ['w', 'doel']), lijn(['w', 'a'], ['w', 'doel']), lijn(['k', 'z'], ['w', 'b']), lijn(['k', 'x'], ['w', 'a']), lijn(['k', 'y'], ['w', 'a'])];
      const r = genummerd({ bestanden, lijnen }, { pad: 'w', begrip: 'doel' });
      const stap = Object.fromEntries(r.bestanden.flatMap((b) => b.begrippen.map((g) => [g.key, g.stap])));
      expect(stap).toEqual({ doel: 1, a: 2, x: 3, y: 3, b: 4, z: 4 });
      expect(r.reststap).toBe(4);
    });

    it('has no rest step when the path takes in everything', () => {
      const een = keten(alle, 'bedrijfsparkeervergunning', { law: beleid, article: '1', naam: 'postcode_vestiging_volgens_handelsregister' });
      expect(een.reststap).toBeNull();
      expect(Math.max(...een.bestanden.flatMap((b) => b.begrippen.map((g) => g.stap)))).toBe(een.stappen);
      expect(stapZin(een, een.stappen)).toContain('hier komt de keten bij de bron uit');
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
        eerder = gehad;
      }
      expect(eerder.size).toBe(alleNamen.length);
    });

    it('lights every line in exactly one step, and keeps it after that', () => {
      for (const l of k.lijnen) {
        const lijn = { van: eind(l.van), naar: eind(l.naar) };
        const standen = Array.from({ length: k.stappen }, (_, i) => stapBeeld(k, i + 1).lijn(lijn));
        expect(standen.filter((x) => x === 'nu')).toHaveLength(1);
        const nu = standen.indexOf('nu');
        expect(standen.slice(0, nu).every((x) => x === null)).toBe(true);
        expect(standen.slice(nu + 1).every((x) => x === 'gehad')).toBe(true);
      }
    });

    it('asks the question first, in the words of the law', () => {
      expect(stapZin(k, null)).toBe(`Het hele schema, in ${k.stappen} stappen.`);
      expect(stapZin(k, 1)).toBe(
        `Stap 1 van ${k.stappen}, de vraag: of het bedrijf op de dag van het besluit al het maximum aantal geldige bedrijfsparkeervergunningen heeft dat het college vaststelt?`,
      );
    });

    it('says what a step needs and where it comes from, briefly', () => {
      const zinnen = Array.from({ length: k.reststap - 2 }, (_, i) => stapZin(k, i + 2));
      for (const z of zinnen.slice(0, -1)) expect(z).toContain(', daarvoor is nodig: ');
      expect(zinnen.at(-1)).toContain(', hier komt de keten bij de bron uit: ');
      for (const z of [...zinnen, stapZin(k, k.reststap)]) {
        expect(z.length).toBeLessThanOrEqual(150);
        expect(z).not.toContain('_');
        expect(z.match(/\.$/g)).toHaveLength(1);
      }
      const alles = zinnen.join(' ');
      for (const waar of ['uit een ander regelwerk', 'van een cel', 'van een reductie', 'uit de kroniek']) expect(alles).toContain(waar);
      // Names that are alike are counted, so the names add up to the count.
      expect(stapZin(k, k.reststap - 1)).toBe(
        `Stap ${k.reststap - 1} van ${k.stappen}, hier komt de keten bij de bron uit: 4 bronnen uit de kroniek (beschikking 3 keer, afschrift van de uitspraak).`,
      );
    });

    it('says of the rest that it works the same way, and where it ends up', () => {
      expect(stapZin(k, k.reststap)).toBe(
        `Stap ${k.reststap} van ${k.stappen}, de andere invoer werkt op dezelfde manier en komt via Kamer van Koophandel bij de bron uit (vestiging ingeschreven).`,
      );
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

  describe('its chronicle', () => {
    const kroniek = k.bestanden.find((b) => b.pad === KRONIEK);

    it('has one box for every source in the chronicle, in a column of its own, last', () => {
      expect(k.bestanden.filter((b) => b.soort === 'kroniek')).toEqual([kroniek]);
      expect(SOORTEN.at(-1).soort).toBe('kroniek');
      const bronnen = k.lijnen.filter((l) => ['source.decisions', 'source.facts', 'source.chronicle'].includes(l.yaml));
      expect(bronnen.length).toBeGreaterThan(0);
      for (const l of bronnen) {
        expect(l.soort).toBe('kroniek');
        expect(l.van.pad).toBe(KRONIEK);
        expect(k.bestanden.find((b) => b.pad === l.naar.pad).soort).toBe('reductie');
      }
      expect(kroniek.begrippen.map((g) => `${g.naam} · ${g.vorm}`).sort()).toEqual([
        'afschrift_van_de_uitspraak · Algemene wet bestuursrecht art. 8:79',
        'beschikking · Algemene wet bestuursrecht art. 7:11',
        'beschikking · Parkeerverordening Wilgendaal 2026 (fictief) art. 2',
        'beschikking · Parkeerverordening Wilgendaal 2026 (fictief) art. 5',
        'vestiging_ingeschreven · Inschrijving van een vestiging in het handelsregister (fictief beleid) art. 1',
      ]);
    });

    it('leaves no source in the box of the law or policy that gives its form', () => {
      for (const b of k.bestanden.filter((x) => x.pad !== KRONIEK)) {
        for (const g of b.begrippen) expect(['kroniek', 'decisions', 'fact']).not.toContain(g.rol);
      }
    });

    it('drops the boxes that had nothing else', () => {
      const mappen = k.bestanden.map((b) => map(b.pad));
      expect(mappen).not.toContain('algemene_wet_bestuursrecht');
      expect(mappen).not.toContain('beleid_handelsregister_kamer_van_koophandel');
      expect(mappen).toContain('parkeerverordening_gemeente_wilgendaal');
      for (const b of k.bestanden) expect(b.begrippen.length).toBeGreaterThan(0);
    });

    it('says in its label where the form comes from', async () => {
      const html = await renderToString(createSSRApp({ render: () => h(SchemaOverzicht, { schema: k, label: 'x' }) }));
      expect(html).toContain('uit de kroniek');
      expect(html).toContain('vorm uit Algemene wet bestuursrecht art. 7:11');
      expect(html).toContain('van beschikking, vorm uit Parkeerverordening Wilgendaal 2026 (fictief) art. 2 in Kroniek');
    });
  });

  it('puts a name before what is computed from it', () => {
    const n = namen('uitvoeringsbesluit_parkeren_gemeente_wilgendaal');
    expect(n.indexOf('maximum_bedrijfsparkeervergunningen')).toBeLessThan(n.indexOf('maximum_bedrijfsparkeervergunningen_bereikt'));
  });
});
