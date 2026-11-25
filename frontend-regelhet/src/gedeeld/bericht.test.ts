import { mount } from '@vue/test-utils';
import { describe, expect, it } from 'vitest';
import {
  STANDAARDSOORT,
  berichtsoortVoor,
  heeftBrief,
  isVerzending,
  maakBericht,
  rechtsmiddelenVan,
  sha256,
  verzendingenVan,
  verzendvelden,
  verzondenBericht,
} from './bericht';
import Bericht from './Bericht.vue';
import type { Berichtenspec, Berichtsoort, Gram, Stand, Vormen, Waarom } from './types';

// A fictitious allowance of a fictitious municipality: two parts that add up
// to the amount granted. Neutral on purpose; the template is generic.
const aanvraag: Gram = {
  id: 'a1',
  kind: 'AANVRAAG',
  basis: 'voorbeeldwet#3',
  fields: { naam_aanvrager: 'Anna Voorbeeld', adres_aanvrager: 'Voorbeeldlaan 2, Wilgendaal', jaar: 2027, kanaal: 'portaal' },
  recorded_on: '2026-10-13',
  recording_actor: 'gemeente',
};

const besluit = (outputs: Record<string, unknown>, kind = 'BESCHIKKING'): Gram => ({
  id: 'b2c3d4e5-0000-4000-8000-000000000000',
  kind,
  basis: 'voorbeeldwet#4',
  refers_to: 'a1',
  outputs,
  fields: {},
  recorded_on: '2026-10-20',
  recording_actor: 'gemeente',
});

/** The chain of the decision, with the outcomes the specification reads. */
const keten = (a: number, b: number): Waarom => ({
  ingevuld: [],
  antwoorden: [],
  bevindingen: [],
  keten: [
    {
      soort: 'artikel',
      wet: 'voorbeeldwet',
      artikel: '4',
      stappen: [
        { soort: 'uitkomst', naam: 'deel_wonen', waarde: a },
        { soort: 'uitkomst', naam: 'deel_reizen', waarde: b },
        { soort: 'uitkomst', naam: 'totaal', waarde: a + b },
        { soort: 'hook', wet: 'algemene_wet_bestuursrecht', artikel: '3:46' },
      ],
    },
  ],
} as unknown as Waarom);

const vormen = {
  indieningen: {
    'voorbeeldwet#3': { soort: 'AANVRAAG', basis: 'voorbeeldwet#3', velden: [], besluit: 'voorbeeldwet#4', cel: 'gemeente' },
    'algemene_wet_bestuursrecht#7:1': {
      soort: 'BEZWAAR',
      basis: 'algemene_wet_bestuursrecht#7:1',
      velden: [],
      besluit: 'algemene_wet_bestuursrecht#7:11',
      cel: 'gemeente',
      termijn: 'termijn_einddatum',
    },
  },
  besluiten: {},
  eigen: { cel: 'gemeente', cel_naam: 'gemeente Wilgendaal', handelingen: [] },
  andere_cellen: [],
  teksten: {},
} as unknown as Vormen;

const soort: Berichtsoort = {
  id: 'beschikking',
  patroon: 'beschikking',
  vastlegging: { rol: 'besluit', artikel: 'voorbeeldwet#4', rechtskarakter: 'BESCHIKKING', uitkomst: 'toegekend' },
  sjabloon: 'besluit',
  sleutels: {
    toekenning: { uitkomst: 'toegekend', is: true },
    afwijzing: { uitkomst: 'toegekend', is: false },
    bedrag: { uitkomst: 'bedrag', vorm: 'bedrag' },
    jaar: { veld: 'jaar', vorm: 'tekst' },
    aanvraagdag: { record: 'recorded_on', vorm: 'datum' },
    wonen: { keten: 'deel_wonen', vorm: 'bedrag' },
    reizen: { keten: 'deel_reizen', vorm: 'bedrag' },
    totaal: { keten: 'totaal', vorm: 'bedrag' },
    bezwaarzin: { zin: 'bezwaarzin' },
    bezwaardag: { uitkomst: 'termijn_einddatum', vorm: 'datum' },
    kanaal: { veld: 'kanaal' },
    // A value that is also a condition: the paragraph stands when it has one.
    toelichting: { veld: 'toelichting', vorm: 'tekst' },
  },
  delen: [
    { id: 'kop', plek: 'kop', bron: { regels: ['naam_aanvrager', 'adres_aanvrager'] } },
    { id: 'inleiding', plek: 'inleiding', bron: { tekst: [''] } },
    { id: 'dictum', plek: 'dictum', vereist_door: 'Awb 1:3', bron: { tekst: ['Besluit'] }, nldd: 'nldd-box' },
    { id: 'grondslag', plek: 'grondslag', vereist_door: 'Awb 3:47', bron: { wet: 'grondslag' } },
    { id: 'motivering', plek: 'motivering', vereist_door: 'Awb 3:46', bron: { tekst: ['Motivering'], wet: 'keten' } },
    { id: 'specificatie', plek: 'specificatie', als: 'toekenning', bron: { tabel: { rijen: ['wonen', 'reizen'], totaal: 'totaal' } }, nldd: 'nldd-table' },
    { id: 'ondertekening', plek: 'ondertekening', vereist_door: 'Awb 10:10', bron: { wet: 'ondertekening' } },
    { id: 'rechtsmiddelen', plek: 'rechtsmiddelen', vereist_door: 'Awb 3:45', bron: { tekst: ['Bezwaar'], wet: 'rechtsmiddelen' }, nldd: 'nldd-box' },
  ],
  invarianten: [
    { soort: 'bedrag_is_totaal', bedrag: 'bedrag', specificatie: 'specificatie' },
    {
      soort: 'oordeel_past',
      oordelen: {
        toekenning: { sleutel: 'bedrag', groter_dan: 0 },
        afwijzing: { een_van: [{ sleutel: 'bedrag', gelijk: 0 }, { sleutel: 'bedrag', niet_gevuld: true }] },
      },
    },
  ],
  kanaal: { sleutel: 'kanaal', waarden: { portaal: 'portaal', papier: 'papier' } },
  status: { bekendgemaakt: { uitkomst: 'bekendgemaakt', is: true } },
};

const spec: Berichtenspec = {
  berichtsoorten: [soort],
  tekstlaag: {
    afzender: { organisatie: 'Het college van Wilgendaal', in_mandaat: true, namens: 'namens deze,', rollen: { beslisser: 'de teamleider' } },
    zinnen: { bezwaarzin: ', uiterlijk op {bezwaardag}' },
    sjablonen: [
      {
        id: 'besluit',
        betreft_bij: { toekenning: 'Toelage {jaar} toegekend', afwijzing: 'Uw aanvraag voor {jaar}' },
        aanhef: 'Geachte mevrouw Voorbeeld,',
        ondertekenaar: 'beslisser',
        slot: 'Met vriendelijke groet,',
        alineas: [
          { tekst: 'U vroeg op {aanvraagdag} een toelage aan.' },
          { kop: 'Besluit', als: 'toekenning', tekst: 'Wij beoordelen uw aanvraag positief en kennen u {bedrag} toe.' },
          { kop: 'Besluit', als: 'afwijzing', tekst: 'Wij wijzen uw aanvraag af.' },
          { kop: 'Motivering', tekst: 'U voldoet aan de voorwaarden.' },
          { kop: 'Motivering', als: 'toelichting', tekst: '{toelichting}' },
          { kop: 'Bezwaar', tekst: 'U kunt bezwaar maken binnen zes weken{bezwaarzin}.' },
        ],
      },
    ],
  },
};

/** Text with ordinary spaces: an amount is written with a no-break space after the euro sign. */
const t = (s: string | undefined) => s?.replace(/\u00a0/g, ' ');

const namen = (cel: string) => (cel === 'rechtbank' ? 'Rechtbank Wilgendaal' : 'gemeente Wilgendaal');
const maak = (g: Gram, w: Waarom, extra: Partial<Parameters<typeof maakBericht>[1]> = {}) =>
  maakBericht(soort, { gram: g, rol: 'besluit', vormen, grammen: [aanvraag, g], waarom: w, afzender: 'gemeente', celNaam: namen, ...extra }, spec);

describe('a record as a message', () => {
  it('fills every part from the record and the text layer, and every invariant holds', () => {
    const g = besluit({ toegekend: true, bedrag: 32272, termijn_einddatum: '2026-12-01', bekendgemaakt: true });
    const b = maak(g, keten(20000, 12272));
    expect(b.schendingen).toEqual([]);
    expect(b.onderwerp).toBe('Toelage 2027 toegekend');
    expect(b.geadresseerde).toEqual(['Anna Voorbeeld', 'Voorbeeldlaan 2, Wilgendaal']);
    const deel = (id: string) => b.onderdelen.find((o) => o.id === id);
    expect(t(deel('dictum')?.alineas[0]?.tekst)).toBe('Wij beoordelen uw aanvraag positief en kennen u € 322,72 toe.');
    expect(deel('grondslag')?.opsomming.length).toBe(1);
    expect(t(deel('specificatie')?.tabel?.totaal?.waarde)).toBe('€ 322,72');
    expect(deel('rechtsmiddelen')?.alineas[0]?.tekst).toBe('U kunt bezwaar maken binnen zes weken, uiterlijk op 1 december 2026.');
    expect(deel('ondertekening')?.alineas.map((a) => a.tekst)).toEqual(['Het college van Wilgendaal', 'namens deze,', 'de teamleider']);
    expect(b.status).toBe('bekendgemaakt');
    expect(deel('motivering')?.alineas.map((a) => a.tekst)).toEqual(['U voldoet aan de voorwaarden.']);
    const met = maak({ ...g, fields: { toelichting: 'Uw kosten zijn aangetoond.' } }, keten(20000, 12272));
    expect(met.onderdelen.find((o) => o.id === 'motivering')?.alineas.map((a) => a.tekst)).toEqual([
      'U voldoet aan de voorwaarden.',
      'Uw kosten zijn aangetoond.',
    ]);
    expect(b.kanaal?.grond).toEqual(['Awb 2:8', 'Awb 2:10']);
    // The parts stand in the order of the places, whatever order the spec lists them in.
    expect(b.onderdelen.map((o) => o.plek)).toEqual(['inleiding', 'dictum', 'grondslag', 'motivering', 'specificatie', 'ondertekening', 'rechtsmiddelen']);
  });

  it('catches an amount in the decision that is not the total of its specification', () => {
    // The fault this guards against: "positief beoordeeld", € 0,00 in the decision, € 322,72 in the specification.
    const g = besluit({ toegekend: true, bedrag: 0, termijn_einddatum: '2026-12-01' });
    const b = maak(g, keten(20000, 12272));
    expect(b.schendingen.map((s) => s.soort).sort()).toEqual(['bedrag_is_totaal', 'oordeel_past']);
    expect(t(b.schendingen.find((s) => s.soort === 'bedrag_is_totaal')?.tekst)).toBe(
      'Het bedrag in het besluit (€ 0,00) is niet het totaal van de specificatie (€ 322,72).',
    );
    expect(b.status).toBe('vastgesteld');
  });

  it('holds for a refusal without a specification, and catches a refusal with an amount', () => {
    const af = maak(besluit({ toegekend: false, bedrag: 0 }), keten(0, 0));
    expect(af.schendingen).toEqual([]);
    expect(af.onderdelen.find((o) => o.id === 'specificatie')).toBeUndefined();
    expect(af.onderdelen.find((o) => o.id === 'dictum')?.alineas[0]?.tekst).toBe('Wij wijzen uw aanvraag af.');
    const fout = maak(besluit({ toegekend: false, bedrag: 32272 }), keten(20000, 12272));
    expect(fout.schendingen.map((s) => s.tekst)).toContain("Het oordeel 'afwijzing' past niet bij de uitkomst van het stuk.");
  });

  it('catches a specification whose rows do not add up to its total', () => {
    const w = keten(20000, 12272);
    (w.keten![0]!.stappen![2] as { waarde: number }).waarde = 30000;
    const b = maak(besluit({ toegekend: true, bedrag: 30000 }), w);
    expect(b.schendingen.map((s) => t(s.tekst))).toContain('Het totaal van de specificatie (€ 300,00) is niet de som van haar regels (€ 322,72).');
  });

  it('shows a required part that is empty as missing, and a key without a value as a gap', () => {
    const zonder = { ...soort, delen: soort.delen.map((d) => (d.id === 'motivering' ? { ...d, bron: {} } : d)) };
    const g = besluit({ toegekend: true, bedrag: 32272, termijn_einddatum: '2026-12-01' });
    const b = maakBericht(zonder, { gram: g, rol: 'besluit', vormen, grammen: [g], waarom: keten(20000, 12272), afzender: 'gemeente' }, spec);
    expect(b.onderdelen.find((o) => o.id === 'motivering')?.ontbreekt).toBe(true);
    // Without the application, the day and the year are not there: a gap, not an empty word.
    expect(b.onderdelen.find((o) => o.id === 'inleiding')?.alineas[0]?.tekst).toBe('U vroeg op 20 oktober 2026 een toelage aan.');
    expect(b.onderwerp).toBe('Toelage [jaar ontbreekt] toegekend');
    expect(b.schendingen.map((s) => s.soort)).toEqual(expect.arrayContaining(['ontbreekt', 'sleutel_zonder_waarde']));
  });

  it('shows a key the spec gives no source as a violation, and a wrong legal character', () => {
    const half = { ...soort, sleutels: { ...soort.sleutels, jaar: undefined } } as unknown as Berichtsoort;
    const b = maakBericht(half, { gram: besluit({ toegekend: true, bedrag: 32272 }, 'EXECUTOGRAM'), rol: 'besluit', vormen, grammen: [aanvraag], waarom: keten(20000, 12272), afzender: 'gemeente' }, spec);
    expect(b.onderwerp).toBe('Toelage [jaar: geen bron] toegekend');
    expect(b.schendingen.map((s) => s.soort)).toEqual(expect.arrayContaining(['sleutel_zonder_bron', 'rechtskarakter']));
  });

  it('finds the kind of message by role, article and fact, and gives a decision without one the Awb parts', () => {
    const g = besluit({ toegekend: true });
    expect(berichtsoortVoor(g, 'besluit', spec)?.id).toBe('beschikking');
    expect(berichtsoortVoor(aanvraag, 'indiening', spec)).toBeUndefined();
    expect(berichtsoortVoor({ ...g, basis: 'anderewet#1' }, 'besluit', spec)).toBe(STANDAARDSOORT);
    // Two kinds on one article: the condition of the binding picks.
    const buiten = { ...soort, id: 'buiten_behandeling', vastlegging: { ...soort.vastlegging, als: { uitkomst: 'buiten', is: true } } };
    const twee = { ...spec, berichtsoorten: [buiten, soort] };
    expect(berichtsoortVoor(besluit({ buiten: true }), 'besluit', twee)?.id).toBe('buiten_behandeling');
    expect(berichtsoortVoor(besluit({ buiten: false }), 'besluit', twee)?.id).toBe('beschikking');
    const b = maakBericht(STANDAARDSOORT, { gram: g, rol: 'besluit', vormen, grammen: [aanvraag, g], waarom: keten(1, 1), afzender: 'gemeente Wilgendaal', celNaam: namen });
    expect(b.onderwerp).toBe('Besluit op uw aanvraag van 13 oktober 2026');
    expect(b.onderdelen.find((o) => o.plek === 'rechtsmiddelen')?.alineas[0]?.tekst).toBe('U kunt bezwaar indienen bij gemeente Wilgendaal.');
    expect(rechtsmiddelenVan({ ...g, outputs: { termijn_einddatum: '2026-12-01' } }, vormen, namen).termijn).toEqual({ einde: '2026-12-01' });
  });

  it('shows the employee the concept with its violations, and the citizen a notice', () => {
    const g = besluit({ toegekend: true, bedrag: 0 });
    const props = { gram: g, rol: 'besluit' as const, vormen, grammen: [aanvraag, g], waarom: keten(20000, 12272), afzender: 'gemeente', spec };
    const m = mount(Bericht, { props: { ...props, concept: true, weergave: 'medewerker' as const } });
    const banners = m.findAll('nldd-banner').map((x) => x.attributes('text'));
    expect(banners).toEqual(['Concept: nog niet vastgesteld', '2 invarianten geschonden']);
    expect(m.find('nldd-tag').attributes('text')).toBe('Concept');
    expect(m.find('nldd-table').exists()).toBe(true);
    const burger = mount(Bericht, { props: { ...props, weergave: 'burger' as const } });
    expect(burger.findAll('nldd-banner').map((x) => x.attributes('text'))).toEqual(['Dit bericht is niet in orde']);
    expect(burger.find('.brief-lint-tekst').text()).toBe('Het college van Wilgendaal');
    expect(m.find('.brief-stempel').text()).toBe('Concept');
  });
});

describe('a message as a letter, with an annex', () => {
  /** The rows of a calculation per item, as a model gives them: grouped by kind, the amount per row. */
  const regels = [
    { soort: 'Wonen', post: 'huur', plaats: 'Wilgendaal', aantal: 3, deel: 1, per_eenheid: 65800, bedrag: 197400 },
    { soort: 'Wonen', post: 'energie', plaats: 'Voorbeeldstad', aantal: 1, deel: 1, per_eenheid: 113700, bedrag: 113700 },
    { soort: 'Wonen en reizen', post: 'vervoer', plaats: 'Elders', aantal: 2, deel: 2, per_eenheid: 85600, bedrag: 85600 },
  ];
  const metBijlage: Berichtsoort = {
    ...soort,
    sleutels: { ...soort.sleutels, regels: { uitkomst: 'regels' }, modeltotaal: { uitkomst: 'modeltotaal', vorm: 'bedrag' } },
    delen: [
      ...soort.delen.filter((d) => d.id !== 'specificatie'),
      {
        id: 'berekening',
        plek: 'bijlage',
        als: 'toekenning',
        bron: {
          bijlage: {
            labels: 'per_post',
            rijen: 'regels',
            groep: 'soort',
            bedrag: 'bedrag',
            bedragen: ['per_eenheid', 'bedrag'],
            kolommen: [
              { kop: 'kop_post', tekst: 'cel_post' },
              { kop: 'kop_aantal', tekst: 'cel_aantal', onder: 'cel_deel', onder_als: { veld: 'deel', groter_dan: 1 } },
              { kop: 'kop_bedrag', tekst: 'cel_bedrag' },
            ],
            totaal: 'modeltotaal',
          },
        },
      },
    ],
    invarianten: [{ soort: 'bedrag_is_totaal', bedrag: 'bedrag', specificatie: 'berekening' }],
  };
  const laag = {
    ...spec.tekstlaag!,
    briefhoofd: { lint: 'Gemeente Wilgendaal', retouradres: 'Retouradres Postbus 1, 9999 ZZ Wilgendaal', voettekst: ['Gemeente Wilgendaal', 'Postbus 1'] },
    bijlagen: {
      per_post: {
        titel: 'Berekening per post',
        groep: '{soort}',
        kop_post: 'Post',
        cel_post: '{post} {plaats}',
        kop_aantal: 'Aantal',
        cel_aantal: '{aantal}',
        cel_deel: 'gedeeld door {deel}',
        kop_bedrag: 'Bedrag',
        cel_bedrag: '{bedrag}',
        samen: 'Samen voor {groep}',
        totaal: 'Totaal',
      },
    },
  };
  const metSpec: Berichtenspec = { berichtsoorten: [metBijlage], tekstlaag: laag };
  const g = (bedrag: number) => besluit({ toegekend: true, bedrag, regels, modeltotaal: 396700, termijn_einddatum: '2026-12-01' });
  const bouw = (x: Gram, extra: Partial<Parameters<typeof maakBericht>[1]> = {}) =>
    maakBericht(metBijlage, { gram: x, rol: 'besluit', vormen, grammen: [aanvraag, x], waarom: keten(1, 1), afzender: 'gemeente', celNaam: namen, ...extra }, metSpec);

  it('groups the rows of the model with a sum per group and the total of the model, which is the amount decided', () => {
    const b = bouw(g(396700));
    expect(b.schendingen).toEqual([]);
    const bijlage = b.onderdelen.find((o) => o.id === 'berekening')!.bijlage!;
    expect(bijlage.kolommen).toEqual(['Post', 'Aantal', 'Bedrag']);
    expect(bijlage.groepen.map((x) => x.titel)).toEqual(['Wonen', 'Wonen en reizen']);
    expect(bijlage.groepen[0]!.rijen.map((r) => r.cellen[0]!.tekst)).toEqual(['huur Wilgendaal', 'energie Voorbeeldstad']);
    expect(bijlage.groepen[1]!.rijen[0]!.cellen[1]).toEqual({ tekst: '2', onder: 'gedeeld door 2' });
    expect(bijlage.groepen[0]!.rijen[0]!.cellen[1]!.onder).toBeUndefined();
    expect(bijlage.groepen.map((x) => t(x.subtotaal?.waarde))).toEqual(['€ 3.111,00', '€ 856,00']);
    expect(t(bijlage.totaal?.waarde)).toBe('€ 3.967,00');
    // The letter: the band with the sender, the return address, the references, the annex, the footer.
    expect(b.lint).toBe('Gemeente Wilgendaal');
    expect(b.retouradres).toBe('Retouradres Postbus 1, 9999 ZZ Wilgendaal');
    expect(b.uwBriefVan).toBe('13 oktober 2026');
    expect(b.voettekst).toEqual(['Gemeente Wilgendaal', 'Postbus 1']);
  });

  it('catches a total that is not the amount decided, and rows that do not make the total of the model', () => {
    const nul = bouw(g(0));
    expect(nul.schendingen.map((s) => t(s.tekst))).toContain('Het bedrag in het besluit (€ 0,00) is niet het totaal van de specificatie (€ 3.967,00).');
    const scheef = bouw(besluit({ toegekend: true, bedrag: 400000, regels, modeltotaal: 400000 }));
    expect(scheef.schendingen.map((s) => t(s.tekst))).toContain('De regels van de bijlage tellen op tot € 3.967,00, maar het totaal is € 4.000,00.');
  });

  it('prints as a letter: the band, the return address, the column of references, the stamp of a concept', () => {
    const x = g(396700);
    const props = { gram: x, rol: 'besluit' as const, vormen, grammen: [aanvraag, x], waarom: keten(1, 1), afzender: 'gemeente', spec: metSpec };
    const m = mount(Bericht, { props: { ...props, weergave: 'burger' as const } });
    expect(m.find('.brief-lint-tekst').text()).toBe('Gemeente Wilgendaal');
    expect(m.find('.brief-retour').text()).toBe('Retouradres Postbus 1, 9999 ZZ Wilgendaal');
    expect(m.findAll('.brief-kenmerken dt').map((d) => d.text())).toEqual(['Datum', 'Onze referentie', 'Uw brief van', 'Betreft', 'Verzonden via', 'Bijlage']);
    expect(m.find('.brief-bijlage').exists()).toBe(true);
    expect(m.find('.brief-voet').text()).toContain('Postbus 1');
    expect(m.find('.brief-stempel').exists()).toBe(false);
    const concept = mount(Bericht, { props: { ...props, concept: true, weergave: 'medewerker' as const } });
    expect(concept.find('.brief-stempel').text()).toBe('Concept');
  });
});

describe('a message sent', () => {
  const g = besluit({ toegekend: true, bedrag: 32272, termijn_einddatum: '2026-12-01', bekendgemaakt: true });
  const verzendspec: Berichtenspec = {
    ...spec,
    verzending: { feit: 'voorbeeldbeleid#9#bericht_verzonden', doorsturen: 'voorbeeldbeleid#9#bericht_doorgestuurd', velden: { bsn: { veld: 'bsn' } } },
  };
  const feit = (velden: Record<string, unknown>, id = 'v1'): Gram => ({
    id,
    kind: 'EXECUTOGRAM',
    basis: 'voorbeeldbeleid#9',
    fact: 'bericht_verzonden',
    fields: velden,
    recorded_on: '2026-10-21',
    recording_actor: 'gemeente',
  });

  it('tells which records have a letter apart from their page: a message sent and a submission the spec writes about, never a decision', () => {
    const ontvangst: Berichtsoort = { ...soort, id: 'ontvangstbevestiging', vastlegging: { rol: 'indiening', artikel: 'voorbeeldwet#3' } };
    const metOntvangst: Berichtenspec = { ...verzendspec, berichtsoorten: [soort, ontvangst] };
    const stand: Pick<Stand, 'rollen'> = { rollen: { [aanvraag.id]: 'indiening', [g.id]: 'besluit', v1: 'vastlegging' } };
    expect(heeftBrief(aanvraag, stand, metOntvangst)).toBe(true);
    expect(heeftBrief(aanvraag, stand, verzendspec)).toBe(false);
    // A decision has a message too, but its page is that message.
    expect(heeftBrief(g, stand, metOntvangst)).toBe(false);
    expect(heeftBrief(feit({ inhoud: '{}', hash: 'x' }), stand, verzendspec)).toBe(true);
  });

  it('hashes with SHA-256', () => {
    expect(sha256('abc')).toBe('ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad');
    expect(sha256('')).toBe('e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855');
  });

  it('records the message frozen, and shows it so after the spec and the text layer change', () => {
    const model = maak(g, keten(20000, 12272));
    const velden = verzendvelden(model, g, [{ ...aanvraag, fields: { ...aanvraag.fields, bsn: '999' } }, g], verzendspec, '2026-10-21');
    expect(velden).toMatchObject({ bsn: '999', berichtsoort: 'beschikking', vastlegging: g.id, kanaal: 'portaal', verzonden_op: '2026-10-21', vervangt: null });
    expect(velden.hash).toBe(sha256(velden.inhoud as string));
    const verzonden = feit(velden);
    expect(isVerzending(verzonden, verzendspec)).toBe(true);
    expect(verzendingenVan(g.id, [g, verzonden], verzendspec).map((x) => x.id)).toEqual(['v1']);

    // The text layer changes after the message went: the letter that went stays as it went.
    const anders: Berichtenspec = JSON.parse(JSON.stringify(verzendspec));
    anders.tekstlaag!.sjablonen![0]!.alineas![1]!.tekst = 'Wij kennen u niets toe.';
    anders.tekstlaag!.afzender!.organisatie = 'Een ander college';
    const props = { gram: g, rol: 'besluit' as const, vormen, grammen: [aanvraag, g, verzonden], waarom: keten(20000, 12272), afzender: 'gemeente', spec: anders };
    const nu = mount(Bericht, { props: { ...props, weergave: 'burger' as const } });
    expect(nu.text()).toContain('Wij kennen u niets toe.');
    const toen = mount(Bericht, { props: { ...props, verzonden, weergave: 'burger' as const } });
    expect(t(toen.text())).toContain('Wij beoordelen uw aanvraag positief en kennen u € 322,72 toe.');
    expect(toen.find('.brief-lint-tekst').text()).toBe('Het college van Wilgendaal');
    expect(toen.find('.brief-stempel').exists()).toBe(false);
    expect(verzondenBericht(verzonden)?.status).toBe('bekendgemaakt');
    // The citizen opens the fact itself: the same letter.
    const zelf = mount(Bericht, { props: { ...props, gram: verzonden, rol: undefined, weergave: 'burger' as const } });
    expect(t(zelf.text())).toContain('kennen u € 322,72 toe.');
    // The employee sees that it went, and when.
    const medewerker = mount(Bericht, { props: { ...props, verzonden, weergave: 'medewerker' as const } });
    expect(medewerker.findAll('nldd-banner').map((x) => x.attributes('text'))[0]).toBe('Verzonden op 21 oktober 2026, in het portaal, met een kennisgeving');
  });

  it('shows content that no longer matches its hash as not in order', () => {
    const model = maak(g, keten(20000, 12272));
    const velden = verzendvelden(model, g, [aanvraag, g], verzendspec, '2026-10-21');
    const geknoeid = feit({ ...velden, inhoud: (velden.inhoud as string).replace('322,72', '999,99') });
    expect(verzondenBericht(geknoeid)?.schendingen.map((s) => s.tekst)).toContain(
      'De inhoud van het verzonden bericht klopt niet met de hash die bij het verzenden werd vastgelegd.',
    );
  });
});
