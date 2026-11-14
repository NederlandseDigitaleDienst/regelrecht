/**
 * De stappen van de Awb-procedure zoals deze pagina ze toont (aanvraag,
 * besluit, bezwaar, beslissing op bezwaar, beroep, uitspraak, opnieuw
 * beslissen), gespeeld via de generieke stappen van het spel (api.js):
 * indienen onder een artikel, beslissen op een indiening, en wat daarvoor en
 * daarna komt. Het spel noemt geen van die stappen; welke indieningen er zijn
 * en wie erop beslist, leidt het af uit de wet (`casus.indieningen`).
 *
 * Dit bestand is de enige plek in de app die een indiening op haar soort
 * (`BEZWAAR`, `BEROEP`) aan een Awb-stap koppelt, en die uitvoer van de Awb
 * bij naam leest (`bezwaar_ontvankelijk`, `nieuw_besluit`). De rest van de
 * app volgt de Awb nog als vaste keten (state.js, beroep.js); dat is het
 * werk dat hierna komt.
 */
import { api } from './api.js';
import { state } from './state.js';

/**
 * De indiening van soort `soort` die de casus kent, of null. Vestigen twee
 * artikelen een indiening van dezelfde soort, dan neemt de app de eerste:
 * de pagina heeft per Awb-stap één formulier. Het spel weigert dat niet,
 * want de wet mag het.
 */
const indiening = (casus, soort) => casus?.indieningen?.find((i) => i.soort === soort) ?? null;

/**
 * De casus zoals `api.casus` haar geeft, met de namen waaronder de app de
 * Awb-stappen leest: het artikel van de beslissing op bezwaar, dat van het
 * beroep, het feit van de ontvangst van een uitspraak en de rechtbank. Zonder
 * zo'n indiening is het null.
 */
export function metAwbNamen(casus) {
  const bezwaar = indiening(casus, 'BEZWAAR');
  const beroep = indiening(casus, 'BEROEP');
  return {
    ...casus,
    beslissing_op_bezwaar: bezwaar?.besluit ?? null,
    beroep: beroep?.artikel ?? null,
    ontvangst: beroep?.ontvangst ?? null,
    rechter: beroep ? { cel: beroep.cel, cel_naam: beroep.cel_naam } : null,
  };
}

/**
 * De formulieren zoals `api.vormen` ze per artikel geeft, met daarnaast die
 * van de Awb-stappen: per indiening het formulier van haar soort, en het
 * formulier van het besluit dat zij vraagt.
 */
export function vormenMetAwbNamen(vormen, casus) {
  const uit = {
    ...vormen,
    aanvraag: vormen.indieningen?.[casus.artikel],
    besluit: vormen.besluiten?.[casus.besluit],
  };
  for (const [soort, naam, beslissing] of [
    ['BEZWAAR', 'bezwaar', 'beslissing_op_bezwaar'],
    ['BEROEP', 'beroep', 'uitspraak'],
  ]) {
    const i = indiening(casus, soort);
    if (!i) continue;
    uit[naam] = vormen.indieningen?.[i.artikel];
    uit[beslissing] = vormen.besluiten?.[i.besluit];
  }
  return uit;
}

/** Een beslissing op bezwaar: het besluit, en het nieuwe besluit in de plaats van het herroepene. */
const opBezwaar = (b) => ({
  beslissing: b.besluit,
  nieuw_besluit: b.in_de_plaats?.nieuw_besluit ?? null,
  waarom: b.waarom,
});

/** Een uitspraak, met de bevindingen die geen artikel vroeg. */
const uitspraak = (b) => ({ uitspraak: b.besluit, niet_gevraagd: b.niet_gevraagd, waarom: b.waarom });

/** Een voorvertoning met haar uitkomst zoals `f` die geeft. */
const als = (f) => (v) => (v.voorlopig ? { ...v, voorlopig: f(v.voorlopig) } : v);

/**
 * Wat vóór een beslissing op bezwaar komt, onder de namen van de Awb, met
 * `reden`: waarom het bezwaar niet-ontvankelijk is, in de volgorde waarin
 * Awb 7:11 het toetst. Te laat en niet verschoonbaar (6:9, 6:11), gericht
 * tegen een besluit dat in bezwaar is genomen (7:1), of niet van een
 * belanghebbende (1:2, 8:1).
 */
const heroverweging = (bezwaar) => (v) => {
  const ontvankelijk = v.uitvoer?.bezwaar_ontvankelijk === true;
  const tijdig = v.velden?.bezwaar_of_beroepschrift_tijdig_ingediend;
  let reden = null;
  if (!ontvankelijk) {
    if (tijdig === false && v.velden?.indiener_bezwaarschrift_niet_in_verzuim !== true) reden = 'te_laat';
    else if (v.verwezen_basis === artikelVan('BEZWAAR')) reden = 'genomen_in_bezwaar';
    else reden = 'geen_belanghebbende';
  }
  return {
    bezwaar,
    bestreden_besluit: v.verwezen,
    ontvankelijk,
    tijdig,
    reden,
    uitvoer: v.uitvoer,
    bevindingen: v.bevindingen,
  };
};

/**
 * De ontvangst van een uitspraak en waar die via `refers_to` naar
 * terugleidt: de beslissing op bezwaar, het bezwaar en het bestreden besluit.
 * Uit de kroniek zoals de cel haar nu houdt, niet uit de kopie in `state`.
 */
async function naUitspraak(ontvangst) {
  const { grammen } = await api.kroniek();
  const gram = (id) => grammen.find((g) => g.id === id);
  const receipt = gram(ontvangst);
  const beslissing = receipt && gram(receipt.refers_to);
  const bezwaar = beslissing && gram(beslissing.refers_to);
  const bestreden = bezwaar && gram(bezwaar.refers_to);
  if (!bestreden) throw new Error(`'${ontvangst}' leads to no decision on an objection`);
  return { receipt, beslissing, bezwaar, bestreden };
}

const artikelVan = (soort) => {
  const i = indiening(state.casus, soort);
  if (!i) throw new Error(`the case '${state.casus?.id}' has no submission ${soort}`);
  return i.artikel;
};

export const awb = {
  aanvragen: (velden, datum) => api.indienen(state.casus.artikel, velden, datum),
  besluiten: (op, bevindingen, datum) => api.beslissen(op, bevindingen, datum),
  voorvertoningBesluit: (op, bevindingen, datum) => api.voorvertoning(op, bevindingen, datum),
  bezwaren: (besluit, velden, datum) => api.indienen(artikelVan('BEZWAAR'), velden, datum, besluit),
  heroverweging: async (bezwaar, ontvankelijkheid, datum) =>
    heroverweging(bezwaar)(await api.voorBeslissen(bezwaar, ontvankelijkheid, datum)),
  beslissenOpBezwaar: async (op, bevindingen, datum) => opBezwaar(await api.beslissen(op, bevindingen, datum)),
  voorvertoningOpBezwaar: async (op, bevindingen, datum) =>
    als(opBezwaar)(await api.voorvertoning(op, bevindingen, datum)),
  beroepInstellen: (besluit, velden, datum) => api.indienen(artikelVan('BEROEP'), velden, datum, besluit),
  uitspraakDoen: async (op, bevindingen, datum) => uitspraak(await api.beslissen(op, bevindingen, datum)),
  voorvertoningUitspraak: async (op, bevindingen, datum) =>
    als(uitspraak)(await api.voorvertoning(op, bevindingen, datum)),
  toezenden: (uitspraakId, datum) => api.toezenden(uitspraakId, datum),
  // Het nieuwe besluit legt zijn eigen oordelen over de ontvankelijkheid
  // vast: `ontvankelijkheid` geeft ze, ook als ze gelijk blijven.
  heroverwegingNaUitspraak: async (ontvangst, ontvankelijkheid) => {
    const { receipt, beslissing, bezwaar, bestreden } = await naUitspraak(ontvangst);
    const v = await api.voorBeslissen(bezwaar.id, ontvankelijkheid, receipt.recorded_on, ontvangst);
    return {
      ontvangst,
      uitspraak: receipt.fields,
      beslissing: beslissing.id,
      bezwaar: bezwaar.id,
      bestreden_besluit: bestreden.id,
      bevindingen: v.bevindingen,
    };
  },
  opnieuwBeslissenOpBezwaar: async (ontvangst, bevindingen, datum) =>
    opBezwaar(await api.beslissen((await naUitspraak(ontvangst)).bezwaar.id, bevindingen, datum, ontvangst)),
  voorvertoningNaUitspraak: async (ontvangst, bevindingen, datum) =>
    als(opBezwaar)(
      await api.voorvertoning((await naUitspraak(ontvangst)).bezwaar.id, bevindingen, datum, ontvangst),
    ),
};
