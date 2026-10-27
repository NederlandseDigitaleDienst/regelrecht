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
  cel: 'gemeente_wilgendaal',
  fragmenten: ['- name: geldende_besluiten\n  source:\n    parameters:\n      basis: fictieve_verordening#2', '- output: geldende_besluiten'],
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
  leest: ['verleend'],
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

  it('gives each step a short heading', () => {
    expect(schakelTekst(keten, { wortel: true }).tekst).toBe('Verordening art. 2');
    expect(schakelTekst(reductie).tekst).toBe('Reductie geldende_besluiten');
    expect(schakelTekst(keten.kinderen[0]).tekst).toBe('Open begrip maximum_bereikt');
    expect(schakelTekst(keten.kinderen[0].kinderen[0]).tekst).toBe('Antwoord van gemeente Wilgendaal');
    expect(schakelTekst(reductie.kinderen[0]).tekst).toBe('Kroniek van gemeente Wilgendaal');
    expect(verwijzing('x#5#feit_a')).toBe('feit a (X artikel 5)');
  });

  it('explains each step by the pattern of its kind, with the names from the YAML', () => {
    expect(schakelTekst(keten, { wortel: true }).uitleg).toBe(
      'Dit artikel beslist over de aanvraag en bepaalt verleend. Het college verleent op aanvraag een vergunning. ' +
        'Het laat één open begrip over aan andere regels: maximum_bereikt.',
    );
    expect(schakelTekst(reductie).uitleg).toBe(
      'Deze reductie leest de kroniek van gemeente Wilgendaal en geeft geldende_besluiten. ' +
        'De stap erboven geeft basis = Verordening (fictief) artikel 2 mee.',
    );
    expect(schakelTekst(keten.kinderen[0]).uitleg).toBe(
      'Verordening (fictief) artikel 2 laat dit begrip open. Of het maximum is bereikt. Het artikel hieronder vult het in.',
    );
    expect(schakelTekst(keten.kinderen[0].kinderen[0]).uitleg).toContain('geeft het antwoord aantal_geldig uit haar reductie reductie_geldig');
    expect(schakelTekst(keten.kinderen[1]).uitleg).toContain('blijft de invoer leeg');
    const vult = { ...reductie, laag: 'UITVOERINGSBELEID', leest: [], parameters: {}, vult: 'maximum_bereikt', vult_in: 'fictieve_verordening#2', vraagt: ['aantal_geldig'], kinderen: [keten.kinderen[0].kinderen[0]] };
    expect(schakelTekst(vult).uitleg).toBe(
      'Dit artikel vult het open begrip maximum_bereikt van Verordening (fictief) artikel 2. ' +
        'Het neemt daarvoor het antwoord aantal_geldig van gemeente Wilgendaal.',
    );
    // Of its own open terms only those no rule fills, per judge.
    const eigen = (id, ingevuld_door, kinderen = []) => ({ soort: 'begrip', id, ingevuld_door, kinderen });
    const metEigen = {
      ...vult,
      kinderen: [
        ...vult.kinderen,
        eigen('gevuld', 'de Kamer', [reductie]),
        eigen('a_volgens_college', 'het college'),
        eigen('b_volgens_college', 'het college'),
      ],
    };
    expect(schakelTekst(metEigen).uitleg).toContain(
      'Daarnaast laat het a_volgens_college en b_volgens_college open. Het college beoordeelt die per geval.',
    );
    expect(schakelTekst(metEigen).uitleg).not.toContain('gevuld open');
    expect(schakelTekst({ soort: 'eerder', wet: 'x', artikel: '4', laag: 'UITVOERINGSBELEID', naam: 'Uitvoeringsbesluit X' }).tekst).toBe(
      'Uitvoeringsbesluit art. 4',
    );
    expect(schakelTekst({ ...reductie, laag: 'UITVOERINGSBELEID', naam: 'Begin en einde van het schooljaar (fictief beleid)', artikel: '1' }).tekst).toBe(
      'Beleid art. 1',
    );
  });
});

describe('KetenSchakel', () => {
  const render = (schakel, props = {}) =>
    renderToString(createSSRApp({ render: () => h(KetenSchakel, { schakel, ...props }) }));

  it('shows every step as a row of the tree, the fictional ones marked', async () => {
    const html = await render(keten, { open: true });
    expect(html).toContain('Verordening art. 2');
    expect(html).toContain('Open begrip maximum_bereikt');
    expect(html).toContain('Antwoord van gemeente Wilgendaal');
    expect(html).toContain('uit de kroniek van gemeente Wilgendaal');
    expect(html.match(/text="fictief"/g)).toHaveLength(2);
    expect(html).toContain('slot="children"');
  });

  it('opens an article onto its explanation, its file, its YAML and its source', async () => {
    const html = await render(keten, { open: true });
    expect(html).toContain('Het college verleent op aanvraag een vergunning.');
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

  it('shows one YAML panel per block under the step, without a click', async () => {
    const html = await render(reductie);
    expect(html.match(/<nldd-code-viewer[^>]*language="yaml"/g)).toHaveLength(2);
    expect(html).toContain('basis: fictieve_verordening#2');
  });

  it('names in the heading what leads to the next step, as the YAML writes it', () => {
    const t = schakelTekst({ ...reductie, leest: [], parameters: {}, vult: 'maximum_bereikt', vraagt: ['aantal_geldig'] });
    expect(t.toelichting).toBe('vult maximum_bereikt · vraagt antwoord aantal_geldig');
  });
});
