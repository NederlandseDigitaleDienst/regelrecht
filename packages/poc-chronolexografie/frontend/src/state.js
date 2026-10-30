/**
 * De stand van het spel zoals de app hem toont: de gekozen casus, de
 * speeldatum, de chronolexogrammen van de kroniek van haar cel, de kronieken
 * van de andere cellen en wat de wet in de formulieren vraagt. De kroniek zelf
 * leeft in de cel van dit tabblad, als WASM (api.js); `state` is de kopie die
 * de app na elke stap ophaalt. Eén speeldatum voor alle stappen, omdat de
 * kroniek één klok heeft die alleen vooruit loopt: een stap op een eerdere dag
 * dan de vorige weigert de cel.
 *
 * Wat in welke stap zit (aanvraag, besluit, bezwaar, beslissing op bezwaar)
 * leest dit bestand uit de kroniek af aan `kind`, `basis` en `refers_to`,
 * zoals de cel ze vastlegt (RFC-046, RFC-047, RFC-050). Onder welk artikel
 * het besluit valt en onder welk de beslissing op bezwaar, zegt de casus
 * (`api.casus`); het spel leidt het af uit de wet.
 */
import { reactive } from 'vue';
import { api } from './api.js';
import { beroepenTegen, ontvangstenBij, uitspraakOp } from './beroep.js';

export const state = reactive({
  // De gekozen casus, zoals `api.casus` haar geeft; null op het keuzescherm.
  casus: null,
  datum: '',
  grammen: [],
  // De kronieken van de andere cellen van de casus (RFC-052), elk met
  // `cel`, `cel_naam` en `grammen`; leeg bij een casus zonder.
  andere: [],
  // De kroniek van de rechtbank (RFC-053): die van de andere cel die
  // `casus.rechter` noemt; leeg bij een casus zonder rechter.
  rechter: [],
  vormen: null,
  fout: '',
  // Het chronolexogram dat de kroniek bij openen kiest (een link uit het logboek).
  inKroniek: null,
  // The name of the scenario the visitor began (`casus.scenarios[].naam`),
  // which the guide beside the page follows; null in free play.
  scenario: null,
  // The cell's refusals of steps since that scenario began, in order. A
  // refusal records nothing in a chronicle, yet a guide can end on one
  // (scenarioGuides.js); a reset that shortens the chronicle forgets them.
  refusals: [],
  // How often a scenario began in this tab: a restart starts the forms
  // anew, even at the same step (scenarioInvoer.js).
  begonnen: 0,
});

/** Het artikel waaronder de casus op een aanvraag besluit, `<wet>#<artikel>`. */
export const besluitBasis = () => state.casus?.besluit;

/** Het artikel van de beslissing op bezwaar (Awb 7:11). */
export const bezwaarBasis = () => state.casus?.beslissing_op_bezwaar;

/** Of een besluit het gevraagde toewijst, volgens de uitvoer die de casus noemt. */
export const toegewezen = (g) => g?.outputs?.[state.casus?.uitkomst];

// Het spel van dit tabblad, bewaard zodat een herladen tabblad verder gaat
// waar het was. Elk tabblad speelt zijn eigen spel, in het geheugen van de
// browser; sessionStorage hoort bij het tabblad en overleeft een herlaad,
// zodat twee tabbladen elkaars spel niet overschrijven. Het bewaren is een
// gemak: zonder opslag (een privévenster, een volle opslag) speel je gewoon
// door en begint een herlaad op het keuzescherm.
const SLEUTEL = 'poc-spel';
async function bewaar() {
  const bewaard = await api.bewaren();
  try {
    // Met de speeldatum: de kroniek loopt alleen vooruit, dus een herlaad
    // die terugzet naar de eerste dag van de casus zou elke stap weigeren.
    sessionStorage.setItem(
      SLEUTEL,
      JSON.stringify({
        casus: state.casus?.id,
        spel: bewaard,
        datum: state.datum,
        scenario: state.scenario,
        refusals: state.refusals,
      }),
    );
  } catch {
    // Geen opslag of een volle: dan ook geen ouder spel laten staan, anders
    // gaat een herlaad stil verder bij een eerdere stap.
    vergeet();
  }
}
const vergeet = () => {
  try {
    sessionStorage.removeItem(SLEUTEL);
  } catch {
    // Geen opslag: dan is er ook niets te vergeten.
  }
};
const bewaard = () => {
  try {
    return sessionStorage.getItem(SLEUTEL);
  } catch {
    return null;
  }
};

/**
 * Bij het openen: ga verder met het spel dat dit tabblad bewaarde, als het
 * adres de casus `casusId` van dat spel noemt (navigation.js). Op het
 * startscherm of bij een andere casus blijft het spel bewaard, maar gaat de
 * app er niet heen: anders sprong het startscherm, na een demo vanaf de
 * slotdia van het dek, bij elk volgend bezoek door naar die casus. Neemt de
 * cel het niet terug (de wet of de casus veranderde sinds), dan staat dat in
 * de banner en blijft het keuzescherm staan.
 *
 * Returns 'resumed', 'none' (nothing saved, or a game the cell refused and
 * that is gone now) or 'kept' (the cell did not load; the game stays saved
 * for the next reload, and nothing may overwrite it).
 */
export async function hervatten(casusId) {
  const spelTotNu = bewaard();
  if (!spelTotNu) return 'none';
  try {
    const { casus, spel: bewaardSpel, datum, scenario, refusals } = JSON.parse(spelTotNu);
    if (!casusId || casus !== casusId) return 'none';
    await openen(await api.herstellen(bewaardSpel), datum, { scenario, refusals });
    return 'resumed';
  } catch (e) {
    // Laadde de cel niet (het netwerk, een deploy), dan blijft het spel
    // bewaard voor de volgende herlaad; alleen een spel dat de cel weigert,
    // gaat weg.
    const kept = CELL_NOT_LOADED.test(e.message);
    if (!kept) vergeet();
    state.casus = null;
    state.fout = e.message;
    return kept ? 'kept' : 'none';
  }
}

/** Kies een casus: het spel begint haar cel opnieuw. */
export async function kiezen(id) {
  state.fout = '';
  try {
    await openen(await api.kies(id));
    return true;
  } catch (e) {
    state.fout = e.message;
    return false;
  }
}

async function openen(casus, datum = casus.speeldatum, { scenario = null, refusals = [] } = {}) {
  state.scenario = scenario;
  state.refusals = refusals;
  state.vormen = null;
  state.grammen = [];
  state.rechter = [];
  state.casus = casus;
  state.datum = datum;
  await verversen();
}

/** Terug naar het keuzescherm; een herlaad begint daar dan ook. */
export function naarKeuze() {
  vergeet();
  state.casus = null;
  state.scenario = null;
  state.fout = '';
}

/**
 * Begin scenario `naam`: the chronicle goes back to what the cell records
 * before the first scenario, the play date to the first day of the case, and
 * the guide follows that scenario. Returns whether the cell took the reset.
 */
export async function beginScenario(naam) {
  const ok = await stap(() => api.reset());
  if (!ok) return false;
  state.datum = state.casus.speeldatum;
  state.scenario = naam;
  state.refusals = [];
  state.begonnen += 1;
  await bewaar();
  return true;
}

/** Haal de kroniek en de vormen op, voor de speeldatum, en bewaar het spel. */
export async function verversen() {
  if (!state.casus) return;
  try {
    const [kroniek, vormen] = await Promise.all([api.kroniek(), api.vormen(state.datum)]);
    if (kroniek.grammen.length < state.grammen.length) state.refusals = [];
    state.grammen = kroniek.grammen;
    state.andere = kroniek.andere ?? [];
    state.rechter = state.andere.find((a) => a.cel === state.casus.rechter?.cel)?.grammen ?? [];
    state.vormen = vormen;
    await bewaar();
  } catch (e) {
    state.fout = e.message;
  }
}

// Errors api.js throws when the cell itself is down, not a refusal.
const CELL_NOT_LOADED = /^the cell did not load/;
const CELL_DOWN = /^the cell (did not load|stopped)/;

/**
 * Doe een stap: `actie` is een functie die de API aanroept. Een weigering
 * van de cel komt in `state.fout`; daarna is de kroniek vers. Geeft het
 * antwoord terug, of `null` bij een fout.
 */
export async function stap(actie) {
  state.fout = '';
  try {
    const antwoord = await actie();
    await verversen();
    return antwoord;
  } catch (e) {
    state.fout = e.message;
    // In a scenario, a refusal of the cell is kept for the guide, and saved,
    // so a reload still knows the step. Free play has no guide to keep it
    // for. A cell that did not load or stopped (api.js) refused nothing.
    if (state.scenario && !CELL_DOWN.test(e.message)) state.refusals.push(e.message);
    await bewaar().catch(() => {});
    return null;
  }
}

export const verwijstNaar = (id) => state.grammen.filter((g) => g.refers_to === id);

export const aanvragen = () => state.grammen.filter((g) => g.kind === 'AANVRAAG');

/** Het besluit op een aanvraag, als het er is. */
export const besluitOp = (id) =>
  verwijstNaar(id).find((g) => g.basis === besluitBasis() && g.kind !== 'BEZWAAR');

/**
 * Of beslissing `b` het besluit `id` nam: een beslissing op bezwaar noemt
 * het besluit dat zij in de plaats nam bij id in haar uitvoer.
 */
export const neemt = (b, id) => Object.values(b.outputs ?? {}).includes(id);

/** De besluiten die een beslissing op bezwaar nam. */
export const genomenDoor = (beslissing) => state.grammen.filter((g) => neemt(beslissing, g.id));

export const bezwarenTegen = (id) => verwijstNaar(id).filter((g) => g.kind === 'BEZWAAR');

/** Elke beslissing op een bezwaar (Awb 7:11): meer dan één na een uitspraak. */
export const beslissingenOp = (bezwaar) =>
  verwijstNaar(bezwaar).filter((g) => g.basis === bezwaarBasis());

/** De laatste beslissing op een bezwaar, als die er is. */
export const beslissingOp = (bezwaar) => beslissingenOp(bezwaar).at(-1);

/**
 * De laatste beslissing op een bezwaar die nog geldt: een die de rechter met
 * haar rechtsgevolgen vernietigde (een ontvangen uitspraak zegt het), telt
 * niet meer, en dan geldt het bestreden besluit weer.
 */
export const geldendeBeslissingOp = (bezwaar) =>
  beslissingenOp(bezwaar)
    .filter((b) => !ontvangstenBij(b.id).some((o) => o.fields?.rechtsgevolgen_vernietigd === true))
    .at(-1);

export const herroepen = (besluit) =>
  bezwarenTegen(besluit.id).some(
    (bz) => geldendeBeslissingOp(bz.id)?.outputs?.bestreden_besluit_herroepen === true,
  );

/** Wat een chronolexogram is in het proces, zoals het dossier en het logboek het noemen. */
export function rol(g) {
  if (g.kind === 'AANVRAAG') return 'Aanvraag';
  if (g.kind === 'EXECUTOGRAM') return g.received ? 'Ontvangst uitspraak' : 'Executogram';
  if (g.kind === 'BEROEP') return 'Beroep';
  if (g.kind === 'UITSPRAAK') return 'Uitspraak';
  if (g.kind === 'BEZWAAR') return 'Bezwaar';
  if (g.basis === bezwaarBasis()) return 'Beslissing op bezwaar';
  const op = state.grammen.find((x) => x.id === g.refers_to);
  return op?.kind === 'BEZWAAR' ? 'Nieuw besluit' : 'Besluit';
}

/**
 * Het dossier van een aanvraag, in de volgorde waarin het ontstond: de
 * aanvraag, het besluit erop, en per bezwaar elke beslissing en het besluit
 * dat in de plaats van het herroepene kwam. Na een beslissing op bezwaar het
 * beroep en de uitspraak uit de kroniek van de rechtbank (`rechter: true`)
 * en de ontvangst van die uitspraak.
 */
export function dossier(aanvraag) {
  const stap = (gram, rechter = false) => ({ gram, rol: rol(gram), rechter });
  const stappen = [stap(aanvraag)];
  let besluit = besluitOp(aanvraag.id);
  while (besluit) {
    stappen.push(stap(besluit));
    let volgende = null;
    for (const bezwaar of bezwarenTegen(besluit.id)) {
      stappen.push(stap(bezwaar));
      for (const beslissing of beslissingenOp(bezwaar.id)) {
        stappen.push(stap(beslissing));
        for (const beroep of beroepenTegen(beslissing.id)) {
          stappen.push(stap(beroep, true));
          const uitspraak = uitspraakOp(beroep.id);
          if (uitspraak) stappen.push(stap(uitspraak, true));
        }
        for (const ontvangst of ontvangstenBij(beslissing.id)) stappen.push(stap(ontvangst));
      }
      // Het dossier volgt het besluit dat de laatste beslissing op het
      // bezwaar nam. Een besluit dat een eerdere, door de rechter
      // vernietigde beslissing nam, geldt niet meer en staat alleen in de
      // kroniek.
      const laatste = geldendeBeslissingOp(bezwaar.id);
      volgende = (laatste && genomenDoor(laatste)[0]) ?? volgende;
    }
    besluit = volgende;
  }
  return stappen;
}

/** Het besluit dat nu voor een aanvraag geldt. */
export function geldendBesluit(aanvraag) {
  const besluiten = dossier(aanvraag)
    .map((s) => s.gram)
    .filter((g) => g.basis === besluitBasis() && g.refers_to);
  return besluiten.filter((b) => !herroepen(b)).at(-1) ?? null;
}

/**
 * A date as the page writes it: `2026-10-14` → `14 oktober 2026`. A date
 * without a time is midnight UTC, so it is shown in UTC; west of Greenwich it
 * would otherwise become the day before.
 */
export const longDate = (iso) =>
  new Date(iso).toLocaleDateString('nl-NL', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
    timeZone: 'UTC',
  });

/** Een naam uit de wet leesbaar: `laatste_verlofdag` → `laatste verlofdag`. */
export const leesbaar = (naam) => String(naam).replaceAll('_', ' ');

/** `verlof verleend` → `Verlof verleend`. */
export const hoofdletter = (tekst) => tekst.charAt(0).toUpperCase() + tekst.slice(1);
/** Een uitkomst van eerder leesbaar: `true` → `ja`, `false` → `nee`. */
export const jaNee = (v) => (v === true ? 'ja' : v === false ? 'nee' : String(v));

/** Velden zonder de lege (`undefined`): die laat een indiening weg. */
export const zonderLeeg = (velden) =>
  Object.fromEntries(Object.entries(velden).filter(([, v]) => v !== undefined));

export const kort = (id) => String(id).slice(-8);

/**
 * Een aanvraag in één regel: de gegevens die het artikel van de casus zelf
 * vraagt (niet die van de Awb), en de dag van ontvangst.
 */
export function samenvatting(aanvraag) {
  const eigen = (state.vormen?.aanvraag.velden ?? []).filter(
    (v) => v.basis === besluitBasis() && v.naam in (aanvraag.fields ?? {}),
  );
  const delen = eigen.map((v) => `${leesbaar(v.naam)}: ${jaNee(aanvraag.fields[v.naam])}`);
  return [...delen, `ontvangen ${aanvraag.recorded_on}`].join(' · ');
}

/**
 * Een besluit (geen aanvraag, bezwaar of executogram): het verwijst naar
 * waarop het is genomen. Een ontvangst van een uitspraak verwijst ook, naar
 * het besluit waarover die gaat, en is geen besluit.
 */
export const isBesluit = (g) =>
  !!g.refers_to && !['AANVRAAG', 'BEZWAAR', 'EXECUTOGRAM'].includes(g.kind);

/** Een decimaal staat in een chronolexogram als `{"__decimal": "…"}`. */
const isDecimaal = (v) =>
  v !== null && typeof v === 'object' && Object.keys(v).length === 1 && '__decimal' in v;

/**
 * Een besluitwaarde (`{basis, fields, findings, outputs}`), zoals de
 * uitspraak die houdt als de rechter zelf in de zaak voorziet (Awb 8:72,
 * derde lid, onder b).
 */
export const isBesluitwaarde = (v) =>
  v !== null &&
  typeof v === 'object' &&
  typeof v.basis === 'string' &&
  ['fields', 'findings', 'outputs'].every((k) => v[k] !== null && typeof v[k] === 'object');

/**
 * Een waarde leesbaar: een decimaal als het getal, een besluitwaarde als
 * haar basis met haar uitvoer (een besluit daarin net zo, tussen
 * haakjes), de rest als JSON.
 */
export function waarde(v) {
  if (isDecimaal(v)) return v.__decimal;
  if (isBesluitwaarde(v)) {
    const uitvoer = Object.entries(v.outputs).map(([naam, w]) => {
      const tekst = waarde(w);
      return `${leesbaar(naam)}: ${isBesluitwaarde(w) ? `(${tekst})` : tekst}`;
    });
    return [`besluit onder ${v.basis}`, ...uitvoer].join('; ');
  }
  return JSON.stringify(v);
}

// Het id van een chronolexogram: een UUID.
const GRAM_ID = /^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/;

// Wat geen eigen waarde is: leeg, een verwijzing, de kopie van een besluit.
const isMetadata = (w) => w == null || isBesluitwaarde(w) || GRAM_ID.test(w);

/**
 * De hoofdvelden van een chronolexogram als `naam: waarde`: bij een besluit
 * de uitkomst van de casus (`casus.uitkomst`), bij de rest de velden die zijn
 * artikel hem zelf geeft (`vormen.eigen_velden`, niet wat een hook toevoegt)
 * en die geen metadata zijn. Blijven er meer dan twee over, dan geen: dan is
 * niet te zeggen welke de hoofdzaak zijn.
 */
export function hoofdvelden(gram) {
  const waarden = { ...gram.fields, ...gram.outputs };
  const uitkomst = state.casus?.uitkomst;
  const namen = uitkomst in (gram.outputs ?? {}) ? [uitkomst] : (state.vormen?.eigen_velden?.[gram.id] ?? []);
  const eigen = namen.filter((naam) => !isMetadata(waarden[naam]));
  if (eigen.length > 2) return [];
  return eigen.map((naam) => `${naam}: ${typeof waarden[naam] === 'string' ? waarden[naam] : waarde(waarden[naam])}`);
}

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
