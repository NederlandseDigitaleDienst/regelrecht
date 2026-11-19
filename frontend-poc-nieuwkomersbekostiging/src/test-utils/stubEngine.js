/**
 * Stub-engine met dezelfde API als de WasmEngine (registerDataSource,
 * clearDataSources, execute, executeMultiple) die de contract-outputs uit
 * docs/datamodel.md §3 nabootst met eenvoudige regels. Alleen voor unit-tests
 * van de simulatie- en aggregatielogica; de echte wetsuitvoering test
 * tests/scenarios.test.js tegen het corpus.
 */
import {
  LAW_PO,
  LAW_VO,
  CODES_ASIELZOEKER,
  CODES_OVERIGE_VREEMDELING,
  addMonths,
  monthsBetween,
  isEenJanuari,
} from '../lib/nieuwkomerFacts.js';

export const BEDRAG_ASIEL_JAAR = 1516816;
export const BEDRAG_OVERIG_JAAR = 471234;
export const BEDRAG_TWEEDE_JAAR = 228108;
export const TOESLAG_EERSTE_KEER = 1817457;
export const VO_EERSTE_KWARTAAL = 390908;
export const VO_TWEEDE_KWARTAAL = 142633;
export const VO_VOORBEREIDING = 2184334;

function kwartalenTussen(start, date) {
  // Aantal peildata (1 jan/apr/jul/okt) in [start, date).
  let n = 0;
  const [sj, sm] = start.split('-').map(Number);
  let j = sj;
  let m = [1, 4, 7, 10].find((x) => x >= sm) ?? null;
  if (m === null) { j += 1; m = 1; }
  // Peildatum precies op start telt als eerste peildatum (niet verstreken).
  for (;;) {
    const pd = `${j}-${String(m).padStart(2, '0')}-01`;
    if (pd >= date) break;
    if (pd >= start) n++;
    m += 3;
    if (m > 10) { m = 1; j += 1; }
  }
  return n;
}

export function createStubEngine() {
  const sources = new Map();
  const laws = new Set([LAW_PO, LAW_VO]);

  function record(source, key) {
    const s = sources.get(source);
    if (!s) throw new Error(`databron ${source} niet geregistreerd`);
    const r = s.get(String(key));
    if (!r) throw new Error(`${source}: geen record voor ${key}`);
    return r;
  }

  function poLeerling(r, date) {
    const code = r.verblijfstitel_code;
    let categorie;
    if (r.heeft_bsn === false || code === null || code === undefined) categorie = 'BESTUUR_BEOORDEELT';
    else if (CODES_ASIELZOEKER.includes(Number(code))) categorie = 'ASIELZOEKER';
    else if (CODES_OVERIGE_VREEMDELING.includes(Number(code))) categorie = 'OVERIGE_VREEMDELING';
    else categorie = 'BESTUUR_BEOORDEELT';
    const effectief = categorie === 'BESTUUR_BEOORDEELT' ? r.oordeel_bevoegd_gezag : categorie;
    const vierde = addMonths(r.geboortedatum, 48);
    const aftrekMaanden = r.datum_vestiging_nederland < vierde ? monthsBetween(r.datum_vestiging_nederland, vierde) : 0;
    const aftrekKwartalen = Math.floor(aftrekMaanden / 3);
    const basis = effectief === 'ASIELZOEKER' ? 8 : effectief === 'OVERIGE_VREEMDELING' ? 4 : 0;
    const bekostigbaar = Math.max(0, basis - aftrekKwartalen);
    const verstreken = r.eerste_inschrijfdatum <= date ? kwartalenTussen(r.eerste_inschrijfdatum, date) : 0;
    const telt = r.eerste_inschrijfdatum <= date && verstreken < bekostigbaar && effectief !== null && effectief !== undefined;
    const bekostigingsjaar = !telt ? 0 : verstreken < 4 ? 1 : 2;
    const jaarbedrag = bekostigingsjaar === 2 ? BEDRAG_TWEEDE_JAAR : effectief === 'ASIELZOEKER' ? BEDRAG_ASIEL_JAAR : BEDRAG_OVERIG_JAAR;
    return {
      categorie_nieuwkomer: categorie,
      aftrek_maanden_voor_vierde_verjaardag: aftrekMaanden,
      aftrek_kwartalen: aftrekKwartalen,
      bekostigbare_kwartalen: bekostigbaar,
      kwartalen_verstreken: verstreken,
      telt_op_peildatum: telt,
      bekostigingsjaar,
      bedrag_kwartaal: telt ? Math.round(jaarbedrag / 4) : 0,
      aanvraag_vereist: true,
    };
  }

  function poSchool(s, date) {
    const Ap = s.aantal_asielzoekers_peildatum;
    const Vp = s.aantal_overige_vreemdelingen_peildatum;
    const At = isEenJanuari(date) ? Math.min(s.aantal_asielzoekers_telling_1_februari, Ap) : 0;
    const drempel = Ap + Vp >= 4;
    const eersteOpvang = drempel
      ? Math.round(((Ap - At) * BEDRAG_ASIEL_JAAR + At * BEDRAG_TWEEDE_JAAR + Vp * BEDRAG_OVERIG_JAAR) / 4)
      : 0;
    return {
      voldoet_aan_drempel: drempel,
      bedrag_eerste_opvang_peildatum: eersteOpvang,
      bedrag_tweede_jaar_peildatum: Math.round((s.aantal_tweedejaars_asielzoekers_peildatum * BEDRAG_TWEEDE_JAAR) / 4),
      toeslag_eerste_keer: drempel && s.eerste_keer_eerste_opvang ? TOESLAG_EERSTE_KEER : 0,
    };
  }

  function voLeerling(r, date) {
    const start = r.eerste_inschrijfdatum;
    const isNieuwkomer = !!r.is_vreemdeling && start <= date && date < addMonths(start, 24);
    const vorigeTeldatum = `${Number(date.slice(0, 4)) - 1}-10-01`;
    let categorie = 'GEEN';
    if (isNieuwkomer) categorie = start > vorigeTeldatum ? 'EERSTE' : 'TWEEDE';
    return {
      is_nieuwkomer: isNieuwkomer,
      categorie_nieuwkomer_vo: categorie,
      bedrag_kwartaal: categorie === 'EERSTE' ? VO_EERSTE_KWARTAAL : categorie === 'TWEEDE' ? VO_TWEEDE_KWARTAAL : 0,
      aanvraag_vereist: false,
    };
  }

  function voSchool(s) {
    return {
      voorbereidingskosten: s.eerste_keer_eerste_opvang && s.aantal_nieuwkomers_peildatum >= 10 ? VO_VOORBEREIDING : 0,
    };
  }

  function outputsFor(lawId, params, date) {
    if (!laws.has(lawId)) throw new Error(`wet ${lawId} niet geladen`);
    if (params.school_id !== undefined) {
      const s = record('scholen', params.school_id);
      return lawId === LAW_PO ? poSchool(s, date) : voSchool(s, date);
    }
    const r = record('personas', params.bsn);
    return lawId === LAW_PO ? poLeerling(r, date) : voLeerling(r, date);
  }

  return {
    calls: 0,
    listLaws: () => [...laws],
    unloadLaw: (id) => laws.delete(id),
    loadLaw: () => {},
    hasLaw: (id) => laws.has(id),
    registerDataSource(name, keyField, records) {
      sources.set(name, new Map(records.map((r) => [String(r[keyField]), r])));
    },
    clearDataSources() {
      sources.clear();
    },
    execute(lawId, output, params, date) {
      this.calls++;
      const all = outputsFor(lawId, params, date);
      if (!(output in all)) throw new Error(`output ${output} onbekend in ${lawId}`);
      return { outputs: { [output]: all[output] } };
    },
    executeMultiple(lawId, outputs, params, date) {
      this.calls++;
      const all = outputsFor(lawId, params, date);
      const picked = {};
      for (const o of outputs) {
        if (!(o in all)) throw new Error(`output ${o} onbekend in ${lawId}`);
        picked[o] = all[o];
      }
      return { outputs: picked };
    },
    executeWithTrace(lawId, output, params, date) {
      const res = this.execute(lawId, output, params, date);
      return { ...res, trace: { node_type: 'article', name: output, result: res.outputs[output], children: [] } };
    },
  };
}

/** Minimale distributions.yaml-vorm (docs/datamodel.md §5). */
export const STUB_DISTRIBUTIONS = {
  basisjaar: 2025,
  jaren: [2025, 2026, 2027, 2028],
  instroom_per_jaar: {
    2023: { po: 12000, vo: 10000 },
    2024: { po: 11000, vo: 9000 },
    2025: { po: 11500, vo: 9500 },
    2026: { po: 11000, vo: 9000 },
    2027: { po: 11000, vo: 9000 },
    2028: { po: 11000, vo: 9000 },
  },
  categorie_verdeling: {
    po: { asielzoeker: 0.55, overige_vreemdeling: 0.35, ambigu_code: 0.06, zonder_bsn: 0.04 },
    vo: { asielzoeker: 0.6, overige_vreemdeling: 0.3, ambigu_code: 0.06, zonder_bsn: 0.04 },
  },
  verblijfstitel_codes: {
    asielzoeker: { 26: 0.3, 27: 0.2, 46: 0.5 },
    overige_vreemdeling: { 22: 0.4, 25: 0.3, 40: 0.3 },
    ambigu_code: { 21: 0.6, 33: 0.2, 34: 0.1, 98: 0.1 },
  },
  leeftijd_bij_vestiging: {
    po: [{ p: 0.1, value: 2 }, { p: 0.5, value: 6 }, { p: 0.9, value: 10 }, { p: 1, value: 12 }],
    vo: [{ p: 0.1, value: 12 }, { p: 0.5, value: 14 }, { p: 0.9, value: 17 }, { p: 1, value: 18 }],
  },
  wachttijd_tot_inschrijving_dagen: [{ p: 0.5, value: 45 }, { p: 0.9, value: 120 }, { p: 1, value: 365 }],
  aankomstmaand: { 1: 0.08, 2: 0.07, 3: 0.08, 4: 0.08, 5: 0.08, 6: 0.08, 7: 0.09, 8: 0.1, 9: 0.1, 10: 0.09, 11: 0.08, 12: 0.07 },
  scholen: {
    po: { aantal: 3000, nieuwkomers_per_school: [{ p: 0.5, value: 2 }, { p: 0.9, value: 8 }, { p: 0.99, value: 50 }, { p: 1, value: 150 }] },
    vo: { aantal: 450, nieuwkomers_per_school: [{ p: 0.5, value: 10 }, { p: 0.9, value: 60 }, { p: 1, value: 300 }] },
  },
  in_telling_1_februari_prior: 0.3,
  oordeel_bevoegd_gezag_asielzoeker_fractie: 0.6,
};
