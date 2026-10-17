/**
 * De stand van het spel: de kroniek van de cel, wat de wet vraagt en de
 * speeldatum. Eén speeldatum voor alle stappen, omdat de kroniek één klok
 * heeft die alleen vooruit loopt: een stap op een eerdere dag dan de vorige
 * weigert de cel.
 *
 * Wat in welke stap zit (aanvraag, besluit, bezwaar, beslissing op bezwaar)
 * leest dit bestand uit de kroniek af aan `kind`, `grondslag` en `refers_to`,
 * zoals de cel ze vastlegt (RFC-046, RFC-047, RFC-050).
 */
import { reactive } from 'vue';
import { api } from './api.js';

export const BESLUIT_13A = 'leerplichtwet_1969#13a';
export const BESLISSING_OP_BEZWAAR = 'algemene_wet_bestuursrecht#7:11';

export const spel = reactive({
  datum: '2026-10-14',
  grammen: [],
  vormen: null,
  fout: '',
});

/** Haal de kroniek en de vormen op, voor de speeldatum. */
export async function verversen() {
  try {
    const [kroniek, vormen] = await Promise.all([api.kroniek(), api.vormen(spel.datum)]);
    spel.grammen = kroniek.grammen;
    spel.vormen = vormen;
  } catch (e) {
    spel.fout = e.message;
  }
}

/**
 * Doe een stap: `actie` is een functie die de API aanroept. Een weigering
 * van de cel komt in `spel.fout`; daarna is de kroniek vers. Geeft het
 * antwoord terug, of `null` bij een fout.
 */
export async function stap(actie) {
  spel.fout = '';
  try {
    const antwoord = await actie();
    await verversen();
    return antwoord;
  } catch (e) {
    spel.fout = e.message;
    return null;
  }
}

export const verwijstNaar = (id) => spel.grammen.filter((g) => g.refers_to === id);

export const aanvragen = () => spel.grammen.filter((g) => g.kind === 'AANVRAAG');

/** Het besluit onder 13a op een aanvraag of een bezwaar, als het er is. */
export const besluitOp = (id) =>
  verwijstNaar(id).find((g) => g.grondslag === BESLUIT_13A && g.kind !== 'BEZWAAR');

export const bezwarenTegen = (id) => verwijstNaar(id).filter((g) => g.kind === 'BEZWAAR');

/** De beslissing op een bezwaar (Awb 7:11), als die er is. */
export const beslissingOp = (bezwaar) =>
  verwijstNaar(bezwaar).find((g) => g.grondslag === BESLISSING_OP_BEZWAAR);

export const herroepen = (besluit) =>
  bezwarenTegen(besluit.id).some(
    (bz) => beslissingOp(bz.id)?.outputs?.bestreden_besluit_herroepen === true,
  );

/**
 * Het dossier van een aanvraag, in de volgorde waarin het ontstond: de
 * aanvraag, het besluit erop, en per bezwaar de beslissing en het besluit
 * dat in de plaats van het herroepene kwam.
 */
export function dossier(aanvraag) {
  const stappen = [{ gram: aanvraag, rol: 'Aanvraag' }];
  let besluit = besluitOp(aanvraag.id);
  while (besluit) {
    stappen.push({ gram: besluit, rol: stappen.length === 1 ? 'Besluit' : 'Nieuw besluit' });
    let volgende = null;
    for (const bezwaar of bezwarenTegen(besluit.id)) {
      stappen.push({ gram: bezwaar, rol: 'Bezwaar' });
      const beslissing = beslissingOp(bezwaar.id);
      if (beslissing) stappen.push({ gram: beslissing, rol: 'Beslissing op bezwaar' });
      volgende = besluitOp(bezwaar.id) ?? volgende;
    }
    besluit = volgende;
  }
  return stappen;
}

/** Het besluit onder 13a dat nu voor een aanvraag geldt. */
export function geldendBesluit(aanvraag) {
  const besluiten = dossier(aanvraag)
    .map((s) => s.gram)
    .filter((g) => g.grondslag === BESLUIT_13A && g.refers_to);
  return besluiten.filter((b) => !herroepen(b)).at(-1) ?? null;
}

/** Een naam uit de wet leesbaar: `laatste_verlofdag` → `laatste verlofdag`. */
export const leesbaar = (naam) => String(naam).replaceAll('_', ' ');

export const kort = (id) => String(id).slice(-8);

/** Een besluit (geen aanvraag of bezwaar): het verwijst naar waarop het is genomen. */
export const isBesluit = (g) => g.refers_to && g.kind !== 'AANVRAAG' && g.kind !== 'BEZWAAR';

/** Een decimaal staat in een chronolexogram als `{"__decimal": "…"}`. */
const isDecimaal = (v) =>
  v !== null && typeof v === 'object' && Object.keys(v).length === 1 && '__decimal' in v;

/** Een waarde leesbaar: een decimaal als het getal, de rest als JSON. */
export const waarde = (v) => (isDecimaal(v) ? v.__decimal : JSON.stringify(v));

/** JSON zoals de kroniek het bewaart, met elk decimaal als het getal zelf. */
export function alsJson(v) {
  const merk = '\u0000';
  const tekst = JSON.stringify(
    v,
    (_k, x) => (isDecimaal(x) ? `${merk}${x.__decimal}${merk}` : x),
    2,
  );
  return tekst.replace(/"\\u0000(.*?)\\u0000"/g, '$1');
}
