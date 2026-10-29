import { readdirSync, readFileSync, statSync } from 'fs';
import { join, relative } from 'path';
import { fileURLToPath } from 'url';
import { describe, expect, it } from 'vitest';
import { createSSRApp, h } from 'vue';
import { renderToString } from 'vue/server-renderer';
import SchemaOverzicht from '../src/presentation/SchemaOverzicht.vue';
import { LIJNSOORTEN, schema } from '../src/presentation/schema.js';

const repo = fileURLToPath(new URL('../../../../', import.meta.url));

/** The YAML files of the corpus and the cases, per path, as the bundle has them. */
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
  for (const d of ['corpus/regulation', 'corpus/cells', 'packages/poc-chronolexografie/casussen']) lees(join(repo, d));
  return alle;
}
const alle = bundel();
const casussen = readdirSync(join(repo, 'packages/poc-chronolexografie/casussen')).map((f) => f.replace(/\.yaml$/, ''));
const map = (pad) => pad.split('/').at(-2);
const heeft = (s, soort, van, naar) =>
  s.lijnen.some((l) => l.soort === soort && map(l.van.pad) === van[0] && l.van.begrip.endsWith(`#${van[1]}`) && map(l.naar.pad) === naar[0] && l.naar.begrip.endsWith(`#${naar[1]}`));

describe('the overview of the files of a case', () => {
  it.each(casussen)('%s: every line starts and ends at a name in a box', (id) => {
    const s = schema(alle, id);
    const namen = new Set(s.bestanden.flatMap((b) => b.begrippen.map((g) => `${b.pad}|${g.key}`)));
    expect(s.lijnen.length).toBeGreaterThan(0);
    for (const l of s.lijnen) {
      expect(namen.has(`${l.van.pad}|${l.van.begrip}`), JSON.stringify(l)).toBe(true);
      expect(namen.has(`${l.naar.pad}|${l.naar.begrip}`), JSON.stringify(l)).toBe(true);
      expect(l.van.pad).not.toBe(l.naar.pad);
      expect(LIJNSOORTEN.map((k) => k.soort)).toContain(l.soort);
    }
    // Every name in a box has a line: a box shows only what connects.
    const verbonden = new Set(s.lijnen.flatMap((l) => [`${l.van.pad}|${l.van.begrip}`, `${l.naar.pad}|${l.naar.begrip}`]));
    for (const n of namen) expect(verbonden.has(n), n).toBe(true);
  });

  it('follows the business parking permit from the case to the chronicle', () => {
    const s = schema(alle, 'bedrijfsparkeervergunning');
    // The regulation's open term, filled in by the implementing decision.
    expect(heeft(s, 'vult_in', ['uitvoeringsbesluit_parkeren_gemeente_wilgendaal', 'maximum_bedrijfsparkeervergunningen_bereikt'], ['parkeerverordening_gemeente_wilgendaal', 'maximum_bedrijfsparkeervergunningen_bereikt'])).toBe(true);
    // The policy asks the cell, and the cell names its reduction.
    expect(heeft(s, 'vraagt', ['beleid_bedrijfsparkeervergunning_gemeente_wilgendaal', 'antwoord_aantal_geldig'], ['gemeente_wilgendaal', 'aantal_geldige_bedrijfsparkeervergunningen'])).toBe(true);
    expect(heeft(s, 'vraagt', ['gemeente_wilgendaal', 'aantal_geldige_bedrijfsparkeervergunningen'], ['reductie_geldige_bedrijfsparkeervergunningen', 'aantal_geldige_bedrijfsparkeervergunningen'])).toBe(true);
    // The Awb reduction reads the decisions under the article its caller names.
    expect(heeft(s, 'kroniek', ['reductie_algemene_wet_bestuursrecht', 'besluiten'], ['parkeerverordening_gemeente_wilgendaal', 'besluiten onder artikel 2'])).toBe(true);
    // The KvK reduction reads the facts its policy requires.
    expect(heeft(s, 'kroniek', ['reductie_vestiging_in_handelsregister', 'inschrijvingen'], ['beleid_handelsregister_kamer_van_koophandel', 'vestiging_ingeschreven'])).toBe(true);
  });

  it('leaves out what only other cases reach through a shared reduction', () => {
    const pad = schema(alle, 'bedrijfsparkeervergunning').bestanden.map((b) => b.pad).join('\n');
    expect(pad).not.toMatch(/gehandicapt|babw/);
  });

  it('draws a box per file, with its names', async () => {
    const s = schema(alle, 'bedrijfsparkeervergunning');
    const html = await renderToString(createSSRApp({ render: () => h(SchemaOverzicht, { bestanden: alle, casus: 'bedrijfsparkeervergunning' }) }));
    expect(html.match(/<nldd-box/g)).toHaveLength(s.bestanden.length);
    expect(html.match(/data-begrip=/g)).toHaveLength(s.bestanden.reduce((n, b) => n + b.begrippen.length, 0));
  });
});
