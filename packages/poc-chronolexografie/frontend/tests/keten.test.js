import { beforeEach, describe, expect, it } from 'vitest';
import { createSSRApp, h } from 'vue';
import { renderToString } from 'vue/server-renderer';
import KetenSchakel from '../src/components/KetenSchakel.vue';
import { korteUitleg, laagNaam, schakelTekst, verwijzing } from '../src/keten.js';
import { state } from '../src/state.js';

beforeEach(() => {
  state.casus = {
    cel: 'gemeente_wilgendaal',
    cel_naam: 'gemeente Wilgendaal',
    bron: null,
    andere_cellen: [{ cel: 'kamer_van_koophandel', cel_naam: 'Kamer van Koophandel' }],
    regelwerken: [{ id: 'fictieve_verordening', naam: 'Verordening (fictief)' }],
  };
});

// A chain as `api.keten` gives it (src/keten.rs, `Schakel`), with made-up ids.
const reductie = {
  soort: 'artikel',
  wet: 'reductie_algemene_wet_bestuursrecht',
  artikel: '2',
  laag: 'REDUCTIE',
  naam: null,
  bestand: 'cells/reductions/reductie_algemene_wet_bestuursrecht/2026-01-01.yaml',
  url: null,
  tekst: 'Geen regelgeving. Het systeem leest de geldende besluiten.',
  yaml: 'machine_readable:\n  execution: {}',
  fictief: true,
  leest: ['geldende_besluiten'],
  vult: null,
  vraagt: [],
  fragment: '- name: geldende_besluiten\n  source:\n    parameters:\n      basis: fictieve_verordening#2',
  parameters: { basis: 'fictieve_verordening#2' },
  kinderen: [
    { soort: 'kroniek', besluiten_onder: 'fictieve_verordening#2', executogrammen_van: null, cel: 'gemeente_wilgendaal' },
  ],
};
const keten = {
  soort: 'artikel',
  wet: 'fictieve_verordening',
  artikel: '2',
  laag: 'GEMEENTELIJKE_VERORDENING',
  naam: 'Verordening (fictief)',
  bestand: 'regulation/nl/gemeentelijke_verordening/fictieve_verordening/2026-01-01.yaml',
  url: 'https://example.org/verordening',
  tekst: 'FICTIEF, naar het model van een andere verordening. 1 Het college verleent op aanvraag een vergunning.',
  yaml: 'machine_readable:\n  open_terms: []',
  fictief: true,
  leest: [],
  parameters: {},
  kinderen: [
    {
      soort: 'begrip',
      wet: 'fictieve_verordening',
      artikel: '2',
      id: 'maximum_bereikt',
      beschrijving: 'Of het maximum is bereikt.',
      ingevuld_door: null,
      kinderen: [
        {
          soort: 'cel',
          cel: 'gemeente_wilgendaal',
          antwoord: 'aantal_geldig',
          reductie: 'reductie_geldig',
          kinderen: [reductie],
        },
      ],
    },
    { soort: 'cel', cel: null, antwoord: 'postcode', reductie: null, kinderen: [] },
  ],
};

describe('keten.js', () => {
  it('names the layers as a visitor reads them', () => {
    expect(laagNaam('UITVOERINGSBELEID')).toBe('beleid');
    expect(laagNaam('GEMEENTELIJKE_VERORDENING')).toBe('verordening');
    expect(laagNaam('ONBEKEND_SOORT')).toBe('onbekend soort');
  });

  it('takes the first sentence that says what an article does', () => {
    expect(korteUitleg(keten.tekst)).toBe('Het college verleent op aanvraag een vergunning.');
    expect(korteUitleg(reductie.tekst)).toBe('Het systeem leest de geldende besluiten.');
    expect(korteUitleg(`${'woord '.repeat(60)}eind.`, 30)).toMatch(/^woord( woord)* …$/);
  });

  it('puts each step in words', () => {
    expect(schakelTekst(keten).tekst).toBe('Verordening (fictief) artikel 2');
    expect(schakelTekst(reductie).toelichting).toBe('geeft geldende_besluiten · basis: Verordening (fictief) artikel 2');
    expect(schakelTekst(keten.kinderen[0]).toelichting).toBe('ingevuld door het artikel hieronder');
    expect(schakelTekst(keten.kinderen[0].kinderen[0]).tekst).toBe('gemeente Wilgendaal: aantal geldig');
    expect(schakelTekst(keten.kinderen[1]).toelichting).toContain('blijft het leeg');
    expect(schakelTekst(reductie.kinderen[0]).tekst).toBe('leest de besluiten onder Verordening (fictief) artikel 2');
    expect(verwijzing('x#5#feit_a')).toBe('feit a (X artikel 5)');
  });
});

describe('KetenSchakel', () => {
  const render = (schakel, props = {}) =>
    renderToString(createSSRApp({ render: () => h(KetenSchakel, { schakel, ...props }) }));

  it('shows every step as a row of the tree, the fictional ones marked', async () => {
    const html = await render(keten, { open: true });
    expect(html).toContain('Verordening (fictief) artikel 2');
    expect(html).toContain('maximum bereikt');
    expect(html).toContain('gemeente Wilgendaal: aantal geldig');
    expect(html).toContain('uit de kroniek van gemeente Wilgendaal');
    expect(html.match(/text="fictief"/g)).toHaveLength(2);
    expect(html).toContain('slot="children"');
  });

  it('opens an article onto its explanation, its file, its YAML and its source', async () => {
    const html = await render(keten, { open: true });
    expect(html).toContain('Het college verleent op aanvraag een vergunning.');
    expect(html.match(/Het college verleent/g)).toHaveLength(1);
    expect(html).toContain('corpus/regulation/nl/gemeentelijke_verordening/fictieve_verordening/2026-01-01.yaml');
    expect(html).toContain('Volledig artikel');
    expect(html).toContain('href="https://example.org/verordening"');
  });

});

describe('korteUitleg with references', () => {
  it('keeps only the text of a markdown link or reference', () => {
    expect(korteUitleg('Een beroep, bedoeld in [artikel 11, onder f][ref1], en [lid 2](https://example.org).')).toBe(
      'Een beroep, bedoeld in artikel 11, onder f, en lid 2.',
    );
  });
});

describe('korteUitleg on one long word', () => {
  it('keeps every character before the cut', () => {
    expect(korteUitleg('a'.repeat(40), 30)).toBe(`${'a'.repeat(30)} …`);
  });
});

describe('korteUitleg on abbreviations', () => {
  it('keeps "art. 13a" in its sentence', () => {
    expect(korteUitleg('Het hoofd beslist, bedoeld in art. 13a van de wet. Meer.')).toBe(
      'Het hoofd beslist, bedoeld in art. 13a van de wet.',
    );
  });
});

describe('the fragment of a step', () => {
  const render = (schakel) =>
    renderToString(createSSRApp({ render: () => h(KetenSchakel, { schakel }) }));

  it('shows under the step without a click, as YAML', async () => {
    const html = await render(reductie);
    expect(html).toMatch(/<nldd-code-viewer[^>]*language="yaml"/);
    expect(html).toContain('basis: fictieve_verordening#2');
  });

  it('names in the heading what leads to the next step, as the YAML writes it', () => {
    const t = schakelTekst({ ...reductie, leest: [], parameters: {}, vult: 'maximum_bereikt', vraagt: ['aantal_geldig'] });
    expect(t.toelichting).toBe('vult maximum_bereikt · vraagt antwoord aantal_geldig');
  });
});
