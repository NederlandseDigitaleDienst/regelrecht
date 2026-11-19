// Per gemodelleerd plan: welke regeling in de bron (laws/), welke artikelen, en hoe de
// feiten op het punt (uit de GML, via stapel.js) en het bouwplan naar de parameters van de
// YAML gaan. Dit is selectie en vertaling van gegevens, geen rekenwerk: rekenen en het
// eindoordeel doet de RegelRecht-engine. Een plan dat hier niet staat, is niet gemodelleerd.

import { json } from '../../bron.js';
import { afstandTot } from '../kaart/geo.js';

// De overige plannen en bestemmingen, gegenereerd uit de plannen zelf; ze staan in de bron.
const OVERIGE = json('toets/overige-plannen.json');

const WONINGTYPES = ['gestapeld', 'aaneengebouwd', 'twee-aaneen', 'vrijstaand'];

const heeftAanduiding = (f, naam) => f.bouwaanduidingen.some((b) => b.naam === naam);
const aanduidingMet = (f, re) => f.bouwaanduidingen.map((b) => b.naam.match(re)).find(Boolean);
const enkel = (f) => f.enkelbestemming.map((e) => String(e.artikel));

/** Waarde van een maatvoering op het punt, of undefined. */
export function maat(f, type) {
  for (const m of f.maatvoering) for (const w of m.waarden ?? []) if (w.type === type && w.getal != null) return w.getal;
  return undefined;
}

/** Afstand in hele meters van het punt tot een locatie van hetzelfde plan, of undefined. */
function afstandLocatie(f, naam) {
  if (!f.laag) return undefined;
  const m = afstandTot(f.laag, f.punt, (pr) => pr.naam === naam);
  return Number.isFinite(m) ? Math.round(m) : undefined;
}

/** Zet alleen de sleutels waarvan de waarde bekend is; de rest blijft weg en wordt onbekend. */
function bekend(obj) {
  return Object.fromEntries(Object.entries(obj).filter(([, v]) => v !== undefined && v !== null));
}

// Woongebieden 2018: van de bestemmingen in artikel 3 tot en met 12 staat alleen lid 1 (de
// bestemmingsomschrijving) in YAML. Of wonen past, rekent de engine; de bouwregels blijven open.
const WG_ANDERE = [
  ['3', 'Bedrijf', 'bedrijf'], ['4', 'Dienstverlening', 'dienstverlening'], ['5', 'Gemengd', 'gemengd'],
  ['6', 'Groen', 'groen'], ['7', 'Horeca', 'horeca'], ['8', 'Maatschappelijk', 'maatschappelijk'],
  ['9', 'Sport', 'sport'], ['10', 'Verkeer', 'verkeer'], ['11', 'Verkeer - Railverkeer', 'railverkeer'], ['12', 'Water', 'water'],
].map(([nr, naam, slug]) => ({
  geldtOp: (f) => enkel(f).includes(nr),
  bestemming: `${naam} (artikel ${nr}.1, alleen de bestemmingsomschrijving)`,
  gebruik: { artikel: `${nr}.1`, output: `gebruik_past_in_bestemming_${slug}`, params: () => ({ gebruik_bouwplan: 'wonen' }) },
  bouw: nr === '5' ? {
    artikel: '5.2', output: 'hoofdgebouw_voldoet_aan_bouwregels_gemengd',
    params: (f, bp, ctx) => bekend({
      hoofdgebouw_binnen_bouwvlak: f.binnenBouwvlak,
      maatvoering_maximum_bouwhoogte_ter_plaatse: maat(f, 'maximum bouwhoogte (m)') !== undefined,
      maximum_bouwhoogte_ter_plaatse_m: maat(f, 'maximum bouwhoogte (m)'),
      bouwhoogte_bouwplan_m: bp.bouwhoogte,
      parkeren_voldaan: typeof ctx.parkeren === 'boolean' ? ctx.parkeren : undefined,
    }),
  } : null,
  bouwOpen: null, // wonen past hier niet (lid 1), dus de bouwregels beslissen niets
}));

/**
 * Het deel van een plandefinitie dat op het punt geldt, of null. Een plan met `bestemmingen`
 * kiest de eerste die past; de rest van de definitie (regeling, kolom, label) geldt voor alle.
 */
export function defOpPunt(def, feiten) {
  if (!def) return null;
  if (def.bestemmingen) {
    const b = def.bestemmingen.find((x) => x.geldtOp(feiten));
    return b ? { ...def, ...b } : null;
  }
  return def.geldtOp(feiten) ? def : null;
}

const AMBACHT_49 = {
  artikel: '49', output: 'wonen_vergunning_wordt_verleend',
  geldtOp: (f) => enkel(f).includes('49'),
  params: () => ({}),
};

const KORT_OVERIG = {};

export const PLANNEN = {
  'NL.IMRO.0345.Woongeb2018-vg01': {
    regeling: 'bestemmingsplan_woongebieden_2018',
    kolom: 'tijdelijk',
    geldtOp: (f) => enkel(f).includes('13'),
    bestemming: 'Wonen (artikel 13), en van tien andere bestemmingen de bestemmingsomschrijving',
    gebruik: {
      artikel: '13.1', output: 'woningtype_past_in_bestemming',
      params: (f, bp) => ({ woningtype_bouwplan: bp.woningtype }),
    },
    bouw: {
      artikel: '13.2', output: 'hoofdgebouw_voldoet_aan_bouwregels',
      params: (f, bp, ctx) => {
        const bh = maat(f, 'maximum bouwhoogte (m)');
        const we = maat(f, 'maximum aantal wooneenheden');
        return bekend({
          woningtype_bouwplan: bp.woningtype,
          parkeren_voldaan: typeof ctx.parkeren === 'boolean' ? ctx.parkeren : undefined,
          hoofdgebouw_binnen_bouwvlak: f.binnenBouwvlak,
          woningtype_aanduiding_ter_plaatse: WONINGTYPES.find((t) => heeftAanduiding(f, t)) ?? 'geen',
          aantal_woningen_bouwplan: bp.aantalWoningen,
          aanduiding_maximum_aantal_wooneenheden: we !== undefined,
          maximum_aantal_wooneenheden_ter_plaatse: we,
          maatvoering_maximum_bouwhoogte_ter_plaatse: bh !== undefined,
          maximum_bouwhoogte_ter_plaatse_m: bh,
          bouwhoogte_bouwplan_m: bp.bouwhoogte,
          in_veiligheidszone_leiding_hoogspanningsverbinding: f.gebiedsaanduidingen.some((g) => /veiligheidszone - leiding hoogspanning/.test(g.naam)),
          bouwplan_voegt_woningen_toe: bp.voegtWoningenToe,
        });
      },
    },
    bestemmingen: null, // hieronder gevuld: Wonen eerst, dan de andere bestemmingen
  },

  'NL.IMRO.0345.StationskwCZuid-oh01': {
    regeling: 'bestemmingsplan_stationskwartier_c_pelikaanstraat',
    kolom: 'tijdelijk',
    geldtOp: (f) => enkel(f).includes('6'),
    bestemming: 'Wonen (artikel 6)',
    gebruik: {
      artikel: '6.1', output: 'gebruik_past_in_bestemming',
      params: () => ({ gebruik_bouwplan: 'wonen' }),
    },
    bouw: {
      artikel: '6.2', output: 'hoofdgebouw_voldoet_aan_bouwregels',
      params: (f, bp) => {
        const bouwwijze = ['aaneengebouwd', 'twee-aaneen'].find((t) => heeftAanduiding(f, t));
        return bekend({
          gebruik_bouwplan: 'wonen',
          hoofdgebouw_binnen_bouwvlak: f.binnenBouwvlak,
          bouwhoogte_bouwplan_m: bp.bouwhoogte,
          goothoogte_bouwplan_m: bp.goothoogte,
          maximum_bouwhoogte_ter_plaatse_m: maat(f, 'maximum bouwhoogte (m)'),
          maximum_goothoogte_ter_plaatse_m: maat(f, 'maximum goothoogte (m)'),
          aanduiding_accent: heeftAanduiding(f, 'specifieke bouwaanduiding - accent'),
          aanduiding_hoekaccent: f.bouwaanduidingen.some((b) => /hoekaccent/.test(b.naam)),
          aanduiding_bouwwijze_ter_plaatse: Boolean(bouwwijze),
          bouwwijze_aanduiding_ter_plaatse: bouwwijze,
          bouwwijze_bouwplan: bp.woningtype,
        });
      },
    },
  },

  'NL.IMRO.0345.TAMFranseGat3-vg01': {
    regeling: 'tam_omgevingsplan_franse_gat_deelgebied_3',
    kolom: 'nieuw',
    label: 'geldend',
    geldtOp: (f) => enkel(f).includes('13'),
    bestemming: "Hoofdgebouw bouwen (artikel 13), Wonen toegestaan (artikel 9)",
    nietGemodelleerd: { 18: 'TAM Franse Gat 3, bouwactiviteiten in archeologisch waardevol gebied (artikel 18)' },
    toepassing: { artikel: '2', output: 'tijdelijk_deel_niet_van_toepassing', params: () => ({ punt_in_locatie_tam_omgevingsplan: true }) },
    gebruik: {
      artikel: '9', output: 'wonen_toegestaan',
      geldtOp: (f) => enkel(f).includes('9'),
      params: (f, bp) => bekend({
        aantal_woningen_bouwplan: bp.aantalWoningen,
        aantal_wooneenheden_bouwplan: bp.aantalWoningen,
        specifieke_bouwaanduiding_ter_plaatse: aanduidingMet(f, /^specifieke bouwaanduiding - (3\w+)$/)?.[1] ?? 'geen',
      }),
    },
    bouw: {
      artikel: '13', output: 'hoofdgebouw_voldoet_aan_artikel_13',
      params: (f, bp) => bekend({
        aanduiding_aaneengebouwd: heeftAanduiding(f, 'aaneengebouwd'),
        hoofdgebouw_aaneen_gebouwd: bp.woningtype === 'aaneengebouwd',
        aanduiding_gestapeld: heeftAanduiding(f, 'gestapeld'),
        hoofdgebouw_gestapeld: bp.woningtype === 'gestapeld',
        maximum_goothoogte_ter_plaatse_m: maat(f, 'maximum goothoogte (m)'),
        goothoogte_bouwplan_m: bp.goothoogte,
        maximum_bouwhoogte_ter_plaatse_m: maat(f, 'maximum bouwhoogte (m)'),
        bouwhoogte_bouwplan_m: bp.bouwhoogte,
      }),
    },
  },

  'NL.IMRO.0345.TAMAmbacht-ow01': {
    regeling: 'tam_omgevingsplan_het_ambacht',
    variant: 'tam-het-ambacht-ontwerp',
    kolom: 'nieuw',
    label: 'beoogd',
    geldtOp: (f) => enkel(f).includes('54'),
    bestemming: 'Gebouw bouwen (artikel 54) of Hoofdgebouw bouwen (artikel 55), Wonen vergunningplicht (artikel 49)',
    nietGemodelleerd: {
      72: 'TAM Het Ambacht, windhinder bij hoogbouw (artikel 72)',
    },
    // Aanvullingen op 22.29 op hun eigen locatie. Afstanden gemeten op de kaart vanaf het punt.
    aanvullend: [
      {
        artikel: '70', output: 'stemgeluid_voldaan', geldtOp: (f) => enkel(f).includes('70'),
        params: (f) => bekend({
          afstand_tot_ikc_m: afstandLocatie(f, 'Maatschappelijk - Gebruik voor IKC - toegestaan'),
          afstand_tot_terras_m: afstandLocatie(f, 'Horeca - Gebruik voor terras - vergunningplicht'),
        }),
      },
      {
        artikel: '71', output: 'omliggende_bedrijven_voldaan', geldtOp: (f) => enkel(f).includes('71'),
        params: (f) => bekend({
          afstand_tot_vishandel_m: afstandLocatie(f, 'Bedrijf - Gebruik voor vishandel - toegestaan'),
          afstand_tot_bedrijf_categorie_2_m: afstandLocatie(f, 'Bedrijf - Bedrijfsactiviteiten in milieucategorie 2 - toegestaan'),
        }),
      },
    ],
    gebruik: AMBACHT_49,
    bouw: {
      artikel: '54', output: 'gebouw_voldoet_aan_artikel_54',
      params: (f, bp) => ({
        aantal_bouwlagen_bouwplan: bp.lagen,
        soort_gebouw: bp.soortGebouw,
        hoogteaccent_bouwlagen_ter_plaatse: Number(aanduidingMet(f, /hoogteaccent (\d+) bouwlagen/)?.[1] ?? 0),
      }),
    },
    bestemmingen: null, // hieronder: locatie 54, dan 55
  },
};

// Deelgebiedplannen die Woongebieden 2018 ter plaatse herzien: van elke bestemming alleen lid 1
// (scripts/regels/gen_deelgebieden.py). [artikel, naam zoals in de GML, slug van de uitvoer].
function alleenOmschrijving(regeling, kort, bestemmingen, extra = {}, bouwregels = {}, aanvullend = {}) {
  return {
    regeling,
    kolom: 'tijdelijk',
    bestemming: `de bestemmingsomschrijving van ${bestemmingen.map(([, naam]) => naam).join(', ')}`,
    bestemmingen: bestemmingen.map(([nr, naam, slug]) => ({
      geldtOp: (f) => enkel(f).includes(nr),
      bestemming: `${naam} (artikel ${nr}.1, alleen de bestemmingsomschrijving)`,
      gebruik: {
        artikel: `${nr}.1`, output: `gebruik_past_in_bestemming_${slug}`,
        params: (f, bp) => ({ gebruik_bouwplan: 'wonen', ...(extra[nr]?.(f, bp) ?? {}) }),
      },
      bouw: bouwregels[nr] ?? null,
      // De overige artikelen van het plan die op deze bestemming meetellen (gesloten lijst).
      aanvullend: aanvullend[nr] ?? [],
      // Alle bestemmingen waar wonen past, hebben bouwregels; elders beslist lid 1 al.
      bouwOpen: null,
    })),
  };
}

// Bouwregels voor hoofdgebouwen waar wonen past (gen_deelgebieden.py). Feiten uit de GML op het punt.
const maatMax = (f) => maat(f, 'maximum bouwhoogte (m)');
const BOUW_FRANSE_GAT = {
  artikel: '5.2', output: 'hoofdgebouw_voldoet_aan_bouwregels',
  params: (f, bp) => bekend({
    deelgebied_ter_plaatse: f.functieaanduidingen.map((a) => a.naam).find((n) => n.startsWith('specifieke vorm van wonen - ')) ?? 'geen',
    hoofdgebouw_binnen_bouwvlak: f.binnenBouwvlak,
    aantal_woningen_bouwplan: bp.aantalWoningen,
    woningtype_bouwplan: bp.woningtype,
    aanduiding_gestapeld: heeftAanduiding(f, 'gestapeld'),
    bouwhoogte_bouwplan_m: bp.bouwhoogte,
    goothoogte_bouwplan_m: bp.goothoogte,
    maatvoering_maximum_bouwhoogte_ter_plaatse: maatMax(f) !== undefined,
    maximum_bouwhoogte_ter_plaatse_m: maatMax(f),
  }),
};
const BOUW_BRANDMEESTERS = {
  artikel: '4.2', output: 'hoofdgebouw_voldoet_aan_bouwregels',
  params: (f, bp) => bekend({
    hoofdgebouw_binnen_bouwvlak: f.binnenBouwvlak,
    maximum_bouwhoogte_ter_plaatse_m: maatMax(f),
    bouwhoogte_bouwplan_m: bp.bouwhoogte,
    woningtype_aanduiding_ter_plaatse: WONINGTYPES.find((t) => heeftAanduiding(f, t)) ?? 'geen',
    woningtype_bouwplan: bp.woningtype,
    maximum_aantal_wooneenheden_ter_plaatse: maat(f, 'maximum aantal wooneenheden'),
    aantal_woningen_bouwplan: bp.aantalWoningen,
  }),
};
const BOUW_GOEIE_SPOOR = {
  artikel: '6.2', output: 'hoofdgebouw_voldoet_aan_bouwregels',
  params: (f, bp) => {
    const we = maat(f, 'maximum aantal wooneenheden');
    return bekend({
      hoofdgebouw_binnen_bouwvlak: f.binnenBouwvlak,
      woningtype_bouwplan: bp.woningtype,
      aanduiding_gestapeld: heeftAanduiding(f, 'gestapeld'),
      aantal_woningen_bouwplan: bp.aantalWoningen,
      aanduiding_maximum_aantal_wooneenheden: we !== undefined,
      maximum_aantal_wooneenheden_ter_plaatse: we,
      maximum_bouwhoogte_ter_plaatse_m: maatMax(f),
      bouwhoogte_bouwplan_m: bp.bouwhoogte,
    });
  },
};
// 't Goeie Spoor, bestemming Wonen: alle andere artikelen die meetellen voor een woongebouw.
// Gebruiksregels 6.3.2 tot en met 6.3.4, algemene regels 9 tot en met 12.1,
// voorwaardelijke verplichtingen 15.1 tot en met 15.4. Gegevens die de kaart niet kent, komen
// uit Gegevens invullen (aannames.js).
const altijd = () => true;
const geen = () => ({});
const wonen = () => ({ gebruik_bouwplan: 'wonen' });
const heeftGebied = (f, naam) => f.gebiedsaanduidingen.some((g) => g.naam.toLowerCase() === naam);
const OVERIG_GOEIE_SPOOR = [
  { artikel: '6.3.2', output: 'gebruik_niet_strijdig_met_bestemming_wonen', geldtOp: altijd, params: wonen },
  { artikel: '6.3.3', output: 'geluid_voldaan', geldtOp: altijd, params: geen },
  { artikel: '6.3.4', output: 'trillingen_voldaan', geldtOp: altijd, params: geen },
  { artikel: '9', output: 'anti_dubbeltelregel_voldaan', geldtOp: altijd, params: geen },
  { artikel: '10.1', output: 'ondergronds_bouwen_voldaan', geldtOp: altijd, params: (f) => ({ aanduiding_onderdoorgang: heeftAanduiding(f, 'onderdoorgang') }) },
  { artikel: '11.1', output: 'gebruik_niet_algemeen_strijdig', geldtOp: altijd, params: wonen },
  { artikel: '12.1', output: 'boringsvrije_zone_voldaan', geldtOp: (f) => heeftGebied(f, 'milieuzone - boringsvrije zone'), params: () => ({ aanduiding_boringsvrije_zone: true }) },
  { artikel: '15.1', output: 'parkeren_voldaan', geldtOp: altijd, params: geen },
  { artikel: '15.2', output: 'inrichtingsplan_voldaan', geldtOp: altijd, params: geen },
  { artikel: '15.3', output: 'puntensysteem_15_3_voldaan', geldtOp: altijd, params: geen },
  { artikel: '15.4', output: 'broedplaatsen_voldaan', geldtOp: altijd, params: geen },
];
const BOUW_PIONIER = {
  artikel: '6.2', output: 'hoofdgebouw_voldoet_aan_bouwregels',
  params: (f, bp) => bekend({
    hoofdgebouw_binnen_bouwvlak: f.binnenBouwvlak,
    maatvoering_maximum_bouwhoogte_ter_plaatse: maatMax(f) !== undefined,
    maximum_bouwhoogte_ter_plaatse_m: maatMax(f),
    bouwhoogte_bouwplan_m: bp.bouwhoogte,
    aantal_bouwlagen_bouwplan: bp.lagen,
    aanduiding_avt: heeftAanduiding(f, 'specifieke bouwaanduiding - aaneengebouwd/twee-aaneen/vrijstaand'),
    woningtype_bouwplan: bp.woningtype,
    dakhelling_bouwplan_graden: 0, // plat dak, een aanname over het bouwplan
  }),
};

Object.assign(PLANNEN, {
  // Artikelnummers volgens de regeltekst; waar de GML een ander nummer noemt, is de tekst leidend.
  'NL.IMRO.0345.TAMRitmeester-vg01': {
    regeling: 'tam_omgevingsplan_ritmeesterlocatie',
    kolom: 'nieuw',
    label: 'geldend',
    geldtOp: (f) => enkel(f).includes('14'),
    bestemming: 'Hoofdgebouw bouwen (artikel 14), Wonen toegestaan (artikel 9)',
    toepassing: { artikel: '2', output: 'tijdelijk_deel_niet_van_toepassing', params: () => ({ punt_in_locatie_tam_omgevingsplan: true }) },
    gebruik: { artikel: '9', output: 'wonen_toegestaan', geldtOp: (f) => enkel(f).includes('9'), params: () => ({}) },
    bouw: {
      artikel: '14', output: 'hoofdgebouw_voldoet_aan_artikel_14',
      params: (f, bp) => bekend({
        aanduiding_aaneengebouwd: heeftAanduiding(f, 'aaneengebouwd'),
        hoofdgebouw_aaneen_gebouwd: bp.woningtype === 'aaneengebouwd',
        aanduiding_gestapeld: heeftAanduiding(f, 'gestapeld'),
        hoofdgebouw_gestapeld: bp.woningtype === 'gestapeld',
        maximum_bouwhoogte_ter_plaatse_m: maat(f, 'maximum bouwhoogte (m)'),
        bouwhoogte_bouwplan_m: bp.bouwhoogte,
        maximum_bebouwingspercentage_ter_plaatse: maat(f, 'maximum bebouwingspercentage (%)'),
      }),
    },
  },
  'NL.IMRO.0345.FranseGatZ1en2-vg01': alleenOmschrijving('bestemmingsplan_franse_gat_zuid_deel_1_en_2', 'Franse Gat Zuid 1 en 2',
    [['3', 'Verkeer - eigen terrein', 'verkeer_eigen_terrein'], ['4', 'Verkeer', 'verkeer'], ['5', 'Wonen', 'wonen']], {}, { 5: BOUW_FRANSE_GAT }),
  'NL.IMRO.0345.Brandmeesters-vg01': alleenOmschrijving('bestemmingsplan_brandmeesters', 'Brandmeesters',
    [['3', 'Maatschappelijk', 'maatschappelijk'], ['4', 'Wonen', 'wonen']], {}, { 4: BOUW_BRANDMEESTERS }),
  'NL.IMRO.0345.bpOvergangAmbacht-vg01': alleenOmschrijving('bestemmingsplan_overgangszone_ambacht', 'Overgangszone Ambacht',
    [['3', 'Bedrijf', 'bedrijf']]),
  'NL.IMRO.0345.tGoeieSpoor-vg01': alleenOmschrijving('bestemmingsplan_t_goeie_spoor', "'t Goeie Spoor",
    [['3', 'Groen', 'groen'], ['4', 'Parkeren - eigen terrein', 'parkeren_eigen_terrein'], ['5', 'Verkeer', 'verkeer'], ['6', 'Wonen', 'wonen']],
    // 6.1: alleen gestapelde wooneenheden, met eisen aan sociale woningen die het bouwplan niet noemt.
    { 6: (f, bp) => ({ woningtype_bouwplan: bp.woningtype }) }, { 6: BOUW_GOEIE_SPOOR }, { 6: OVERIG_GOEIE_SPOOR }),
  'NL.IMRO.0345.PionierkwartierF3-vg01': alleenOmschrijving('bestemmingsplan_pionierkwartier_fase_3', 'Pionierkwartier fase 3',
    [['3', 'Groen - 2', 'groen_2'], ['4', 'Gemengd - Uit te werken', 'gemengd_uit_te_werken'], ['5', 'Verkeer', 'verkeer'], ['6', 'Wonen', 'wonen']], {}, { 6: BOUW_PIONIER }),
});

{
  const wg = PLANNEN['NL.IMRO.0345.Woongeb2018-vg01'];
  const { regeling, kolom, bestemmingen, ...wonen } = wg;
  wg.bestemmingen = [{ ...wonen, bestemming: 'Wonen (artikel 13)' }, ...WG_ANDERE];
}

{
  // TAM Het Ambacht: locatie 'Gebouw bouwen' (artikel 54) of 'Hoofdgebouw bouwen' (artikel 55,
  // drie kavels met bestaande woningen). Plat dak: dakhelling 0, een aanname over het bouwplan.
  const am = PLANNEN['NL.IMRO.0345.TAMAmbacht-ow01'];
  const { regeling, kolom, label, variant, bestemmingen, nietGemodelleerd, ...a54 } = am;
  am.bestemmingen = [
    { ...a54, bestemming: 'Gebouw bouwen (artikel 54), Wonen vergunningplicht (artikel 49)' },
    {
      geldtOp: (f) => enkel(f).includes('55'),
      bestemming: 'Hoofdgebouw bouwen (artikel 55), Wonen vergunningplicht (artikel 49)',
      gebruik: AMBACHT_49,
      bouw: {
        artikel: '55', output: 'hoofdgebouw_voldoet_aan_artikel_55',
        params: (f, bp) => ({ goothoogte_bouwplan_m: bp.goothoogte, dakhelling_bouwplan_graden: 0 }),
      },
    },
  ];
}

// De overige plannen en bestemmingen (OVERIGE, uit de bron).
// Lid 1 per bestemming; bij een uit te werken bestemming ook het voorlopige bouwverbod (lid 3).
// Bouwregels die nog ontbreken, staan alleen open waar wonen past: elders beslist lid 1 al.
const bouwverbod = (b) => ({
  artikel: `${b.nr}.3`, output: `bouwen_toegestaan_${b.slug}`,
  params: () => ({ uitwerkingsplan_van_kracht_ter_plaatse: false }), // anders was het uitwerkingsplan hier het plan ter plaatse
});
// Bouwregels voor hoofdgebouwen uit gen_overige.py: dezelfde parameters voor alle plannen.
const WONINGTYPE_AANDUIDINGEN = ['gestapeld', 'aaneengebouwd', 'twee-aaneen', 'vrijstaand'];
const bouwOverig = (b) => ({
  artikel: `${b.nr}.2`, output: `hoofdgebouw_voldoet_aan_bouwregels_${b.slug}`,
  params: (f, bp) => {
    const bh = maat(f, 'maximum bouwhoogte (m)');
    const we = maat(f, 'maximum aantal wooneenheden');
    return bekend({
      hoofdgebouw_binnen_bouwvlak: f.binnenBouwvlak,
      bouwhoogte_bouwplan_m: bp.bouwhoogte,
      goothoogte_bouwplan_m: bp.goothoogte,
      maatvoering_maximum_bouwhoogte_ter_plaatse: bh !== undefined,
      maximum_bouwhoogte_ter_plaatse_m: bh,
      maximum_goothoogte_ter_plaatse_m: maat(f, 'maximum goothoogte (m)'),
      woningtype_bouwplan: bp.woningtype,
      woningtype_aanduiding_ter_plaatse: WONINGTYPE_AANDUIDINGEN.find((t) => heeftAanduiding(f, t)) ?? 'geen',
      specifieke_bouwaanduiding_ter_plaatse: f.bouwaanduidingen.map((a) => a.naam).find((n) => n.startsWith('specifieke bouwaanduiding - ')) ?? 'geen',
      aantal_woningen_bouwplan: bp.aantalWoningen,
      aanduiding_maximum_aantal_wooneenheden: we !== undefined,
      maximum_aantal_wooneenheden_ter_plaatse: we,
      dakhelling_bouwplan_graden: 0, // plat dak, een aanname over het bouwplan
    });
  },
});

function uitOverige(p) {
  return p.bestemmingen.filter((b) => !b.alleen_bouwverbod).map((b) => ({
    geldtOp: (f) => enkel(f).includes(b.gml),
    bestemming: `${b.naam} (artikel ${b.nr}.1, alleen de bestemmingsomschrijving)`,
    gebruik: { artikel: `${b.nr}.1`, output: `gebruik_past_in_bestemming_${b.slug}`, params: () => ({ gebruik_bouwplan: 'wonen' }) },
    bouw: b.bouwverbod ? bouwverbod(b) : (b.bouwregels ? bouwOverig(b) : null),
    bouwOpen: b.wonen && !b.bouwverbod && !b.bouwregels ? `${p.kort}, bouwregels van ${b.naam} (artikel ${b.nr}.2)` : null,
  }));
}
for (const p of OVERIGE.plannen) {
  if (!p.aanvulling) {
    PLANNEN[p.id] = {
      regeling: p.regeling, kolom: 'tijdelijk',
      bestemming: `de bestemmingsomschrijving van ${p.bestemmingen.map((b) => b.naam).join(', ')}`,
      bestemmingen: uitOverige(p),
    };
    KORT_OVERIG[p.regeling] = p.kort;
    continue;
  }
  const def = PLANNEN[p.id];
  for (const b of p.bestemmingen.filter((x) => x.alleen_bouwverbod)) {
    const best = def.bestemmingen.find((x) => x.gebruik?.artikel === `${b.nr}.1`);
    Object.assign(best, { bouw: bouwverbod(b), bouwOpen: null });
  }
  const extra = uitOverige(p);
  if (extra.length) {
    const { regeling, kolom, label, variant, toepassing, nietGemodelleerd, bestemmingen, ...basis } = def;
    def.bestemmingen = [...(bestemmingen ?? [basis]), ...extra];
  }
}

/**
 * TAM-plannen, artikel 4: niet genoemde bouwactiviteiten zijn verboden. Ligt op het punt geen
 * enkele locatie voor het bouwen van een gebouw of hoofdgebouw (ook geen die nog niet in YAML
 * staat), dan geldt artikel 4. Dat is een lezing; zie het validatierapport.
 */
function metBouwverbod(id, bouwlocaties) {
  const def = PLANNEN[id];
  const { regeling, kolom, label, variant, toepassing, nietGemodelleerd, bestemmingen, ...basis } = def;
  const genoemd = (f) => bouwlocaties.some((a) => enkel(f).includes(a));
  def.bestemmingen = [...(bestemmingen ?? [basis]), {
    geldtOp: (f) => !genoemd(f),
    bestemming: 'geen locatie voor het bouwen van een gebouw: algemeen bouwverbod (artikel 4)',
    gebruik: null,
    bouw: {
      artikel: '4', output: 'bouwactiviteit_niet_verboden',
      params: (f) => ({ bouwen_hoofdgebouw_genoemd_ter_plaatse: genoemd(f), bestaand_bouwwerk: false }),
    },
  }];
}
{
  // TAM Franse Gat 3: locatie 'Hoofdgebouw bouwen' (13) of 'Gebouw bouwen' (12), beide met wonen (9).
  const fg = PLANNEN['NL.IMRO.0345.TAMFranseGat3-vg01'];
  const { regeling, kolom, label, variant, toepassing, nietGemodelleerd, bestemmingen, ...a13 } = fg;
  fg.bestemmingen = [a13, {
    geldtOp: (f) => enkel(f).includes('12'),
    bestemming: 'Gebouw bouwen (artikel 12), Wonen toegestaan (artikel 9)',
    gebruik: a13.gebruik,
    bouw: {
      artikel: '12', output: 'gebouw_voldoet_aan_artikel_12',
      params: (f, bp) => bekend({
        aanduiding_gestapeld: heeftAanduiding(f, 'gestapeld'),
        gebouw_gestapeld: bp.woningtype === 'gestapeld',
        maximum_bouwhoogte_ter_plaatse_m: maat(f, 'maximum bouwhoogte (m)'),
        bouwhoogte_bouwplan_m: bp.bouwhoogte,
      }),
    },
  }];
}
metBouwverbod('NL.IMRO.0345.TAMFranseGat3-vg01', ['12', '13']); // 12 Gebouw bouwen, 13 Hoofdgebouw bouwen
metBouwverbod('NL.IMRO.0345.TAMRitmeester-vg01', ['14']); // 14 Hoofdgebouw bouwen (nummer volgens de tekst)
metBouwverbod('NL.IMRO.0345.TAMAmbacht-ow01', ['54', '55']); // 54 Gebouw bouwen, 55 Hoofdgebouw bouwen

export const OMGEVINGSPLAN = 'omgevingsplan_veenendaal';
export const PARAPLU = 'parapluplan_2022';

// Lagen bovenop het basisplan (de stapel, aanvullend). Aannames over het bouwplan staan hier en in
// BouwplanInvoer: een woongebouw zonder bijbehorende bouwwerken of kelder, één huishouden per woning.
export const PARAPLU_ARTIKELEN = {
  3: { artikel: '3', output: 'bouwdiepte_voldaan', params: (f, bp) => ({ woningtype_bouwplan: bp.woningtype }) },
  4: { artikel: '4', output: 'voldoet_aan_artikel_4', params: () => ({ bouwplan_bevat_bijbehorende_bouwwerken: false, woning_voor_meer_dan_een_huishouden: false }) },
  10: { artikel: '10', output: 'voldoet_aan_artikel_10', params: () => ({ ondergronds_bouwwerk_buiten_gebouwen: false }) },
  11: { artikel: '11', output: 'voldoet_aan_artikel_11', params: () => ({}) },
};
export const ARCHEOLOGIE = {
  'Waarde - Archeologie - Zeer-Hoog': { artikel: '6', output: 'archeologie_zeer_hoog_voldaan' },
  'Waarde - Archeologie - Hoog': { artikel: '7', output: 'archeologie_hoog_voldaan' },
  'Waarde - Archeologie - Middelhoog': { artikel: '8', output: 'archeologie_middelhoog_voldaan' },
  'Waarde - Archeologie - Watergerelateerd': { artikel: '9', output: 'archeologie_watergerelateerd_voldaan' },
};
/** Facetplannen: een object zonder regels over bouwen staat op null. */
export const FACETTEN = {
  'Leiding - Hoogspanning ondergronds': {
    regeling: 'bestemmingsplan_ondergrondse_hoogspanning', artikel: '3', output: 'bouwen_in_leidingstrook_toegestaan',
    params: (f, bp) => ({ bouwwerk_is_gebouw: true, bouwhoogte_bouwplan_m: bp.bouwhoogte, ten_dienste_van_de_leiding: false }),
  },
  // Artikel 4 heeft geen eigen bouwregels; de veiligheidszone zit in het basisplan (Woongebieden 2018, 13.2.2 onder m).
  'Leiding - Hoogspanning bovengronds': null,
};

/** Korte namen voor in het paneel. */
export const KORT = {
  ...KORT_OVERIG,
  omgevingsplan_veenendaal: 'Omgevingsplan',
  parapluplan_2022: 'Parapluplan 2022',
  bestemmingsplan_woongebieden_2018: 'Woongebieden 2018',
  bestemmingsplan_stationskwartier_c_pelikaanstraat: 'Stationskwartier C',
  tam_omgevingsplan_franse_gat_deelgebied_3: 'TAM Franse Gat 3',
  tam_omgevingsplan_het_ambacht: 'TAM Het Ambacht (ontwerp)',
  bestemmingsplan_franse_gat_zuid_deel_1_en_2: 'Franse Gat Zuid 1 en 2',
  bestemmingsplan_brandmeesters: 'Brandmeesters',
  bestemmingsplan_overgangszone_ambacht: 'Overgangszone Ambacht',
  bestemmingsplan_t_goeie_spoor: "'t Goeie Spoor",
  bestemmingsplan_pionierkwartier_fase_3: 'Pionierkwartier fase 3',
  bestemmingsplan_ondergrondse_hoogspanning: 'Ondergrondse hoogspanning',
  tam_omgevingsplan_ritmeesterlocatie: 'TAM Ritmeester',
};
