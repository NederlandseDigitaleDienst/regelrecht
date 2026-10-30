import { readdirSync, readFileSync, statSync } from 'fs';
import { join, relative } from 'path';
import { fileURLToPath } from 'url';
import { describe, expect, it } from 'vitest';
import { createSSRApp, h } from 'vue';
import { renderToString } from 'vue/server-renderer';
import SchemaOverzicht from '../src/presentation/SchemaOverzicht.vue';
import { LIJNSOORTEN, keten } from '../src/presentation/schema.js';
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
    // An output that computes something keeps its input.
    expect(namen('beleid_bedrijfsparkeervergunning_gemeente_wilgendaal')).toContain('antwoord_aantal_geldig');
  });

  it('shows of a reduction only what it reads and what it answers', () => {
    const awb = 'reductie_algemene_wet_bestuursrecht';
    expect(namen(awb)).not.toContain('vernietigde_besluiten');
    expect(heeft(k, 'rekent', [awb, 'besluiten'], [awb, 'geldende_besluiten'])).toBe(true);
    // A law keeps its steps in between.
    expect(namen('uitvoeringsbesluit_parkeren_gemeente_wilgendaal')).toContain('maximum_bedrijfsparkeervergunningen');
  });

  it('puts a name before what is computed from it', () => {
    const n = namen('uitvoeringsbesluit_parkeren_gemeente_wilgendaal');
    expect(n.indexOf('maximum_bedrijfsparkeervergunningen')).toBeLessThan(n.indexOf('maximum_bedrijfsparkeervergunningen_bereikt'));
  });
});
