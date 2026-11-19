import { readFileSync } from 'fs';
import { join } from 'path';
import { fileURLToPath } from 'url';
import { load } from 'js-yaml';
import { describe, expect, it } from 'vitest';
import { createSSRApp, h } from 'vue';
import { renderToString } from 'vue/server-renderer';
import TheoriePagina from '../src/presentation/TheoriePagina.vue';
import CasusKeuze from '../src/components/CasusKeuze.vue';
import { begrippen, besluit, casus, cellen, dagTekst, intro, kroniek, scenario, tabbladen } from '../src/presentation/theorie.js';

const repo = fileURLToPath(new URL('../../../../', import.meta.url));
const frontend = join(repo, 'packages/poc-chronolexografie/frontend/src');
const corpus = join(repo, 'packages/poc-chronolexografie/corpus');
const doc = load(readFileSync(join(corpus, 'casussen', `${casus}.yaml`), 'utf8'));
const blokken = [kroniek, besluit, cellen, ...begrippen];
const texts = [
  intro.title,
  intro.lead,
  ...blokken.flatMap((b) => [b.title, ...b.text]),
  kroniek.bijschrift,
  ...kroniek.vastleggingen.flatMap((v) => [v.soort, v.tekst]),
  cellen.bijschrift,
  cellen.pijl,
  cellen.van.vastlegging,
  cellen.naar.vastlegging,
  tabbladen.title,
  ...tabbladen.rijen.flatMap((r) => [r.naam, r.tekst]),
];
// Without the attribute Vue adds for the component's own CSS.
const render = async () =>
  (await renderToString(createSSRApp({ render: () => h(TheoriePagina) }))).replace(/ data-v-[0-9a-f]+/g, '');
// A day in the case file, read as a date or as text.
const iso = (d) => (d instanceof Date ? d.toISOString().slice(0, 10) : String(d));

describe('the example of the theory page', () => {
  const stappen = doc.scenarios.find((s) => s.naam === scenario)?.stappen;

  it('follows the steps of one scenario of its case, each on its day', () => {
    expect(stappen, scenario).toBeDefined();
    expect(kroniek.vastleggingen.map((v) => v.stap)).toEqual(stappen.map((s) => s.stap));
    // A step without a day of its own is played on the first day of the case.
    const dag = (s) => iso(s.datum ?? doc.speeldatum);
    expect(kroniek.vastleggingen.map((v) => v.dag)).toEqual(stappen.map(dag));
  });

  it('marks the decisions, and says how each step of the scenario comes out', () => {
    const uitkomst = Object.fromEntries(stappen.filter((s) => s.uitkomst).map((s) => [s.stap, s.uitkomst]));
    expect(uitkomst).toEqual({ besluit: 'afgewezen', beslissing_op_bezwaar: 'gegrond' });
    expect(kroniek.vastleggingen.filter((v) => v.besluit).map((v) => v.stap)).toEqual(Object.keys(uitkomst));
    expect(kroniek.vastleggingen[2].tekst).toMatch(/geweigerd/);
    expect(kroniek.vastleggingen[5].tekst).toMatch(/gegrond/);
    // The text on the objection names the entry of the new advice by its number.
    expect(kroniek.vastleggingen[4].tekst).toMatch(/nieuw advies/);
    expect(begrippen.find((b) => b.title === 'Bezwaar').text.join(' ')).toContain('vastlegging 5');
  });

  it('names the two cells and their facts as the case and its policy do', () => {
    const namen = doc.andere_cellen.map((c) => c.cel_naam);
    expect(namen).toContain(cellen.van.naam);
    expect(cellen.naar.naam.toLowerCase()).toBe(doc.cel_naam.toLowerCase());
    expect(cellen.dag).toBe(kroniek.vastleggingen[1].dag);
    const beleid = readFileSync(
      join(corpus, 'regulation/nl/uitvoeringsbeleid/beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal/2026-01-01.yaml'),
      'utf8',
    );
    const feit = (tekst) => tekst.toLowerCase().replaceAll(' ', '_');
    expect(beleid).toContain(`fact: ${feit(cellen.van.vastlegging)}`);
    expect(beleid).toContain(`fact: ${feit(cellen.naar.vastlegging)}`);
  });

  it('writes a day as the deck does', () => {
    expect(dagTekst('2026-10-13')).toBe('13 okt 2026');
    expect(dagTekst('2027-03-01')).toBe('1 mrt 2027');
  });
});

describe('the text of the theory page', () => {
  it('names the tabs as the app does', () => {
    const app = readFileSync(join(frontend, 'App.vue'), 'utf8');
    for (const r of tabbladen.rijen) {
      const q = r.naam.includes("'") ? '"' : "'";
      expect(app, r.naam).toContain(`tekst: ${q}${r.naam}${q}`);
    }
    for (const t of blokken.flatMap((b) => b.text).join(' ').matchAll(/tabblad (\S+?)[.,]/g)) {
      expect(tabbladen.rijen.map((r) => r.naam), t[0]).toContain(t[1]);
    }
  });

  it('keeps each term to a few sentences', () => {
    for (const b of blokken) expect(b.text.length, b.title).toBeLessThanOrEqual(3);
  });

  it('has no em dash or en dash as punctuation, in the data or the component', () => {
    for (const t of texts) expect(t, t).not.toMatch(/[—–]/);
    const template = readFileSync(join(frontend, 'presentation/TheoriePagina.vue'), 'utf8').split('<template>')[1];
    expect(template).not.toMatch(/[—–]/);
  });
});

describe('the theory page', () => {
  it('is a page of the app: one heading, a section per term, and the way back', async () => {
    const html = await render();
    expect(html.match(/heading-level="1"/g)).toHaveLength(1);
    expect(html.match(/heading-level="2"/g)).toHaveLength(blokken.length + 1);
    expect(html).not.toMatch(/heading-level="[3-6]"/);
    expect(html.match(/<nldd-simple-section/g)).toHaveLength(blokken.length + 2);
    for (const t of texts) expect(html, t).toContain(t.replaceAll("'", '&#39;'));
    // Links, so a click with a modifier key opens the choice screen in a tab of its own.
    expect(html.match(/<nldd-button[^>]*href="\/"/g)).toHaveLength(2);
    expect(html).toMatch(/<nldd-button[^>]*href="\/"[^>]*text="Naar de casuskeuze"/);
  });

  it('draws the chronicle as a numbered list, each entry with its day and its kind in words', async () => {
    const html = await render();
    const lijst = html.match(/<ol[^>]*class="tijdlijn"[^>]*>(.*?)<\/ol>/s)[1];
    expect(lijst.match(/<li[ >]/g)).toHaveLength(kroniek.vastleggingen.length);
    for (const v of kroniek.vastleggingen) {
      expect(lijst).toContain(`<time datetime="${v.dag}">${dagTekst(v.dag)}</time>`);
      expect(lijst).toContain(`<strong>${v.soort}</strong>`);
    }
    // The filled marker repeats what the kind already says; the number is the list's own.
    expect(lijst.match(/class="nummer besluit"/g)).toHaveLength(2);
    expect(lijst.match(/aria-hidden="true"/g)).toHaveLength(kroniek.vastleggingen.length);
  });

  it('gives each picture a caption, and the arrow between the cells a word', async () => {
    const html = await render();
    expect(html.match(/<figure/g)).toHaveLength(2);
    expect(html.match(/<figcaption/g)).toHaveLength(2);
    const at = (s) => html.indexOf(s);
    expect(at(cellen.van.naam)).toBeLessThan(at(`>${cellen.pijl}<`));
    expect(at(`>${cellen.pijl}<`)).toBeLessThan(at(cellen.naar.naam));
    expect(html).toMatch(/<svg[^>]*aria-hidden="true"/);
  });

  it('is what the choice screen links to, by its path', async () => {
    const html = await renderToString(createSSRApp({ render: () => h(CasusKeuze) }));
    expect(html).toMatch(/<nldd-link[^>]*href="\/theorie"[^>]*text="Theorie"/);
  });
});
