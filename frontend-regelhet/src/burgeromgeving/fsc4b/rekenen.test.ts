// @vitest-environment node
// Replaying the signed records, comparing the answers and predicting the
// decision, on a kern that stands in for the WASM build: what the app asks
// of the kern and what it makes of the answers. rekenen.wasm.test.ts plays
// the same on the real kern when it is built.
import { describe, expect, it } from 'vitest';
import type { Gegevens } from './gegevens';
import { Rekenkern, hypothesen, samenvatten } from './rekenen';
import type { PocWasm } from './rekenen';

const GGD_ADVIES = 'beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_uitgebracht';
const ARTIKEL = 'besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49';

const casus = {
  id: 'gehandicaptenparkeerkaart',
  cel: 'gemeente_wilgendaal',
  cel_naam: 'gemeente Wilgendaal',
  uitkomst: 'gehandicaptenparkeerkaart_verstrekt',
  andere_cellen: [
    { cel: 'ggd_wilgendaal', cel_naam: 'GGD Wilgendaal' },
    { cel: 'rechtbank_wilgendaal', cel_naam: 'Rechtbank Wilgendaal' },
  ],
};

const ggdVelden = [
  { naam: 'bsn_aanvrager', type: 'string' },
  { naam: 'datum_onderzoek', type: 'date' },
  { naam: 'langdurige_loopbeperking', type: 'boolean' },
  { naam: 'permanent_rolstoelgebonden', type: 'boolean' },
];

/**
 * A kern that grants the card when the GGD advised a long-term walking
 * impairment or a wheelchair, and refuses it otherwise; it keeps its state
 * as `bewaren` and `herstellen` pass it, so a test sees what is left behind.
 */
function nepKern(lokaal: Record<string, { waarde: unknown }> = {}, regelwerken: unknown[] = []) {
  let staat: { records: unknown[]; advies: Record<string, unknown> | null; ingediend: boolean } = { records: [], advies: null, ingediend: false };
  const log: string[] = [];
  const poc: PocWasm = {
    casussen: () => JSON.stringify({ casussen: [{ id: casus.id }], bronnen: [{ fout: null, fouten: [] }] }),
    kies: () => JSON.stringify(casus),
    vormen: () =>
      JSON.stringify({
        eigen: { cel: casus.cel, cel_naam: casus.cel_naam, handelingen: [] },
        andere_cellen: [{ cel: 'ggd_wilgendaal', cel_naam: 'GGD Wilgendaal', handelingen: [{ feit: GGD_ADVIES, velden: ggdVelden }] }],
      }),
    indienen: (artikel, velden) => {
      log.push(`indienen ${artikel} ${velden}`);
      staat = { ...staat, ingediend: true };
      return JSON.stringify({ id: 'aanvraag-1' });
    },
    vastleggen: (cel, feit, velden) => {
      log.push(`vastleggen ${cel}`);
      if (feit !== GGD_ADVIES) throw new Error('unknown fact');
      staat = { ...staat, advies: JSON.parse(velden) as Record<string, unknown> };
      return '{}';
    },
    voorBeslissen: () => {
      if (!staat.ingediend) throw new Error('no application');
      const a = staat.advies;
      const ja = !!a && (a.langdurige_loopbeperking === true || a.permanent_rolstoelgebonden === true);
      return JSON.stringify({ uitvoer: { gehandicaptenparkeerkaart_verstrekt: ja } });
    },
    brongegevens: () => JSON.stringify({ antwoorden: lokaal, regelwerken, bij_aanvraag: [] }),
    bewaren: () => JSON.stringify(staat),
    herstellen: (b) => {
      const v = JSON.parse(b) as { kronieken?: unknown[][] } & typeof staat;
      if (v.kronieken) {
        log.push(`herstellen ${v.kronieken.map((k) => k.length).join(',')}`);
        staat = { records: v.kronieken.flat(), advies: null, ingediend: false };
      } else staat = v;
      return '{}';
    },
  };
  return { poc, log, staat: () => staat };
}

const gegevens = (antwoorden: Gegevens['antwoorden']): Gegevens => ({
  iss: '00000099000000000001',
  sub: '999990032',
  iat: 0,
  bundel_hash: 'sha256:00',
  peildatum: '2026-10-13',
  casus: casus.id,
  cel: casus.cel,
  regelwerken: [],
  antwoorden,
  records: [],
  bij_aanvraag: [{ feit: GGD_ADVIES, taak: 'Geneeskundig advies uitbrengen' }],
});
const parameters = { bsn_aanvrager: '999990032' };

describe('the kern on the device', () => {
  it('loads the one case of the source', () => {
    const { poc } = nepKern();
    const kern = Rekenkern.laad(() => poc, {}, casus.id);
    expect(kern.casus.id).toBe('gehandicaptenparkeerkaart');
  });

  it('chooses the case the data are about, and refuses a source without it', () => {
    const { poc } = nepKern();
    poc.casussen = () => JSON.stringify({ casussen: [{ id: 'andere' }, { id: casus.id }], bronnen: [{ fout: null, fouten: [] }] });
    expect(Rekenkern.laad(() => poc, {}, casus.id).casus.id).toBe(casus.id);
    expect(() => Rekenkern.laad(() => poc, {}, 'onbekend')).toThrow(/onbekend/);
  });

  it('refuses a source that does not load', () => {
    const { poc } = nepKern();
    poc.casussen = () => JSON.stringify({ casussen: [], bronnen: [{ fout: 'regulation/x.yaml: not YAML', fouten: [] }] });
    expect(() => Rekenkern.laad(() => poc, {}, casus.id)).toThrow(/not YAML/);
  });

  it('replays the records as the chronicle of each cell, in the case’s order and their own', () => {
    const { poc, log } = nepKern();
    const kern = Rekenkern.laad(() => poc, {}, casus.id);
    kern.naspelen([
      { id: 'a', recording_actor: 'gemeente_wilgendaal' },
      { id: 'b', recording_actor: 'ggd_wilgendaal' },
      { id: 'c', recording_actor: 'gemeente_wilgendaal' },
    ]);
    expect(log).toEqual(['herstellen 2,1,0']);
    expect(() => kern.naspelen([{ id: 'x', recording_actor: 'elders' }])).toThrow(/elders/);
  });

  it('compares every signed answer with the one computed here, and a silent cell is no no', () => {
    const { poc } = nepKern({
      aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt: { waarde: true },
      advies_langdurige_loopbeperking: { waarde: false },
      verzoek_om_geneeskundig_onderzoek_staat_open: { waarde: false },
    });
    const kern = Rekenkern.laad(() => poc, {}, casus.id);
    const uit = kern.vergelijk(
      gegevens({
        aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt: { waarde: true, parameters, herkomst: [] },
        advies_langdurige_loopbeperking: { waarde: null, parameters, herkomst: [] },
        verzoek_om_geneeskundig_onderzoek_staat_open: { waarde: false, parameters, herkomst: [] },
      }),
      'bsn_aanvrager',
    );
    expect(uit.regelwerkenGelijk).toBe(true);
    expect(uit.antwoorden).toEqual([
      { naam: 'advies_langdurige_loopbeperking', ondertekend: null, lokaal: false, gelijk: false },
      { naam: 'aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt', ondertekend: true, lokaal: true, gelijk: true },
      { naam: 'verzoek_om_geneeskundig_onderzoek_staat_open', ondertekend: false, lokaal: false, gelijk: true },
    ].sort((a, b) => a.naam.localeCompare(b.naam)));
  });

  it('flags an answer only one side has', () => {
    const { poc } = nepKern({ extra: { waarde: 1 } });
    const kern = Rekenkern.laad(() => poc, {}, casus.id);
    const uit = kern.vergelijk(gegevens({ eigen: { waarde: 1, parameters, herkomst: [] } }), 'bsn_aanvrager');
    expect(uit.antwoorden.map((v) => [v.naam, v.gelijk])).toEqual([
      ['eigen', false],
      ['extra', false],
    ]);
  });

  it('notices when the device computed on other versions of the regelwerken', () => {
    const versie = { law_id: 'beleid_x', valid_from: '2026-01-01', hash: 'sha256:aa' };
    const { poc } = nepKern({}, [{ ...versie, hash: 'sha256:bb' }]);
    const kern = Rekenkern.laad(() => poc, {}, casus.id);
    const g = { ...gegevens({ a: { waarde: 1, parameters, herkomst: [] } }), regelwerken: [versie] };
    expect(kern.vergelijk(g, 'bsn_aanvrager').regelwerkenGelijk).toBe(false);
  });

  it('computes the answers here also when the executor gave none, and says there was nothing to check', () => {
    const { poc } = nepKern({ extra: { waarde: true } });
    const kern = Rekenkern.laad(() => poc, {}, casus.id);
    expect(kern.vergelijk(gegevens({}), 'bsn_aanvrager').antwoorden).toEqual([{ naam: 'extra', ondertekend: undefined, lokaal: true, gelijk: false }]);
    const { poc: leeg } = nepKern();
    expect(Rekenkern.laad(() => leeg, {}, casus.id).vergelijk(gegevens({}), 'bsn_aanvrager').leeg).toBe(true);
  });

  it('predicts conditionally on the GGD’s advice: the least that grants, and what follows otherwise', () => {
    const { poc, staat } = nepKern();
    const kern = Rekenkern.laad(() => poc, {}, casus.id);
    const v = kern.voorspel({
      artikel: ARTIKEL,
      velden: { kaart_als_bestuurder: true },
      datum: '2026-10-13',
      bijAanvraag: gegevens({}).bij_aanvraag,
      inlogveld: 'bsn_aanvrager',
      bsn: '999990032',
    });
    expect(v.zeker).toBe(false);
    expect(v.nu.uitkomst).toBe('afgewezen');
    expect(v.takken.map((t) => [t.celNaam, t.vastgesteld, t.uitkomst, !!t.anders])).toEqual([
      ['GGD Wilgendaal', ['langdurige loopbeperking'], 'toegekend', false],
      ['GGD Wilgendaal', ['permanent rolstoelgebonden'], 'toegekend', false],
      ['GGD Wilgendaal', [], 'afgewezen', true],
    ]);
    expect(v.takken[0]?.taak).toBe('Geneeskundig advies uitbrengen');
    // The kern is put back as it was: nothing of the trial stays.
    expect(staat()).toEqual({ records: [], advies: null, ingediend: false });
  });

  it('is certain when every branch comes out the same', () => {
    const { poc } = nepKern();
    poc.voorBeslissen = () => JSON.stringify({ uitvoer: { gehandicaptenparkeerkaart_verstrekt: true } });
    const kern = Rekenkern.laad(() => poc, {}, casus.id);
    const v = kern.voorspel({ artikel: ARTIKEL, velden: {}, datum: '2026-10-13', bijAanvraag: gegevens({}).bij_aanvraag, inlogveld: 'bsn_aanvrager', bsn: '1' });
    expect(v.zeker).toBe(true);
  });

  it('tries findings together, so a grant that needs two of them is found', () => {
    const { poc } = nepKern();
    let advies: Record<string, unknown> = {};
    poc.vastleggen = (_c, _f, velden) => ((advies = JSON.parse(velden) as Record<string, unknown>), '{}');
    // A passenger's card needs both (Regeling gehandicaptenparkeerkaart, art. 2).
    poc.voorBeslissen = () =>
      JSON.stringify({ uitvoer: { gehandicaptenparkeerkaart_verstrekt: advies.langdurige_loopbeperking === true && advies.permanent_rolstoelgebonden === true } });
    const kern = Rekenkern.laad(() => poc, {}, casus.id);
    const v = kern.voorspel({ artikel: ARTIKEL, velden: {}, datum: '2026-10-13', bijAanvraag: gegevens({}).bij_aanvraag, inlogveld: 'bsn_aanvrager', bsn: '1' });
    expect(v.zeker).toBe(false);
    expect(v.takken.map((t) => [t.vastgesteld, t.uitkomst, !!t.anders])).toEqual([
      [['langdurige loopbeperking', 'permanent rolstoelgebonden'], 'toegekend', false],
      [[], 'afgewezen', true],
    ]);
  });

  it('is not certain when nothing can be predicted', () => {
    const { poc } = nepKern();
    poc.voorBeslissen = () => JSON.stringify({ uitvoer: {} });
    const kern = Rekenkern.laad(() => poc, {}, casus.id);
    expect(kern.voorspel({ artikel: ARTIKEL, velden: {}, datum: '2026-10-13', bijAanvraag: [], inlogveld: 'bsn_aanvrager', bsn: '1' }).zeker).toBe(false);
  });

  it('reports the failure of the prediction, not one of putting the kern back', () => {
    const { poc } = nepKern();
    poc.indienen = () => {
      throw new Error('the application lacks a field');
    };
    poc.herstellen = () => {
      throw new Error('could not put back');
    };
    const kern = Rekenkern.laad(() => poc, {}, casus.id);
    expect(() => kern.voorspel({ artikel: ARTIKEL, velden: {}, datum: '2026-10-13', bijAanvraag: [], inlogveld: 'bsn_aanvrager', bsn: '1' })).toThrow(/lacks a field/);
  });

  it('says why when the kern cannot decide, and puts it back all the same', () => {
    const { poc, staat } = nepKern();
    poc.voorBeslissen = () => {
      throw new Error("the policy leaves 'x' to the authority for this case");
    };
    const kern = Rekenkern.laad(() => poc, {}, casus.id);
    const v = kern.voorspel({ artikel: ARTIKEL, velden: {}, datum: '2026-10-13', bijAanvraag: [], inlogveld: 'bsn_aanvrager', bsn: '1' });
    expect(v.nu).toEqual({ uitkomst: 'onbekend', reden: "the policy leaves 'x' to the authority for this case" });
    expect(staat().ingediend).toBe(false);
  });
});

describe('what a cell could establish', () => {
  it('is every combination of its yes-or-no fields, with the login field and the day filled in', () => {
    const h = hypothesen(ggdVelden, { datum: '2026-10-13', inlogveld: 'bsn_aanvrager', bsn: '999990032' });
    if (!('hypothesen' in h)) throw new Error(h.reden);
    const vast = { bsn_aanvrager: '999990032', datum_onderzoek: '2026-10-13' };
    expect(h.hypothesen).toEqual([
      { velden: { ...vast, langdurige_loopbeperking: false, permanent_rolstoelgebonden: false }, vastgesteld: [] },
      { velden: { ...vast, langdurige_loopbeperking: true, permanent_rolstoelgebonden: false }, vastgesteld: ['langdurige loopbeperking'] },
      { velden: { ...vast, langdurige_loopbeperking: false, permanent_rolstoelgebonden: true }, vastgesteld: ['permanent rolstoelgebonden'] },
      { velden: { ...vast, langdurige_loopbeperking: true, permanent_rolstoelgebonden: true }, vastgesteld: ['langdurige loopbeperking', 'permanent rolstoelgebonden'] },
    ]);
  });

  it('is nothing, with a reason, when a field asks what the app cannot guess or there are too many', () => {
    expect(hypothesen([{ naam: 'conclusie', type: 'string' }], { datum: 'd', inlogveld: 'bsn', bsn: '1' })).toEqual({ reden: expect.stringMatching(/conclusie/) });
    const vijf = ['a', 'b', 'c', 'd', 'e'].map((naam) => ({ naam, type: 'boolean' }));
    expect('reden' in hypothesen(vijf, { datum: 'd', inlogveld: 'bsn', bsn: '1' })).toBe(true);
  });
});

describe('the summary of the branches', () => {
  const u = (vastgesteld: string[], uitkomst: 'toegekend' | 'afgewezen' | 'onbekend') => ({ vastgesteld, uitkomst });

  it('names each least combination that grants, and one line for the rest when it all comes out the same', () => {
    expect(samenvatten([u([], 'afgewezen'), u(['a'], 'toegekend'), u(['b'], 'afgewezen'), u(['a', 'b'], 'toegekend')])).toEqual([
      u(['a'], 'toegekend'),
      { vastgesteld: [], anders: true, uitkomst: 'afgewezen' },
    ]);
  });

  it('lists every other combination when the rest differs', () => {
    expect(samenvatten([u([], 'afgewezen'), u(['a'], 'toegekend'), u(['b'], 'onbekend')])).toEqual([
      u(['a'], 'toegekend'),
      u([], 'afgewezen'),
      u(['b'], 'onbekend'),
    ]);
  });

  it('is one line when nothing grants and everything comes out the same', () => {
    expect(samenvatten([u([], 'afgewezen'), u(['a'], 'afgewezen')])).toEqual([{ vastgesteld: [], anders: true, uitkomst: 'afgewezen' }]);
  });
});
