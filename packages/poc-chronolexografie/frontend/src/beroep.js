/**
 * Het beroep bij de bestuursrechter (RFC-053): de stappen ervan en wat de app
 * in de twee kronieken leest. De rechtbank is een andere cel van de casus
 * (RFC-052) met een eigen kroniek (`state.rechter`). Het beroep verwijst niet
 * met `refers_to` naar het bestreden besluit maar draagt wat de rechtbank
 * ontving (`received`); de school legt de ontvangst van een uitspraak vast
 * als executogram dat met `refers_to` naar haar eigen besluit verwijst, en een
 * besluit dat opnieuw is genomen noemt die ontvangst als `after`. Onder welk
 * artikel het beroep valt en welk feit de ontvangst is
 * (`<wet>#<artikel>#<feit>`, Awb 8:79), zegt de casus (`api.casus`); het spel
 * leidt het af uit de wet.
 *
 * De bestuurlijke lus (Awb 8:51a): na een tussenuitspraak deelt het
 * bestuursorgaan de rechtbank mee of en hoe het het gebrek herstelde
 * (8:51b). De rechtbank legt die mededeling vast als executogram dat naar de
 * tussenuitspraak verwijst, met de nieuwe beslissing als het bestuursorgaan
 * die nam (`received`, Awb 6:19). De einduitspraak op hetzelfde beroep noemt
 * die mededeling als `after`. Welk feit de mededeling is, zegt de casus
 * (`mededeling`).
 */
import { api } from './api.js';
import { state } from './state.js';

export const beroepApi = {
  instellen: api.beroepInstellen,
  uitspraakDoen: api.uitspraakDoen,
  // Wat de uitspraak, of het opnieuw beslissen op bezwaar, zou worden, op een
  // kopie van de cel: er wordt niets vastgelegd (zoals api.voorvertoningBesluit).
  voorvertoningUitspraak: api.voorvertoningUitspraak,
  toezenden: api.toezenden,
  heroverweging: api.heroverwegingNaUitspraak,
  opnieuwBeslissen: api.opnieuwBeslissenOpBezwaar,
  voorvertoningNaUitspraak: api.voorvertoningNaUitspraak,
  meedelen: api.herstelMeedelen,
};

/** De beroepen bij de rechtbank tegen besluit `id` van de cel van de casus. */
export const beroepenTegen = (id) =>
  state.rechter.filter((g) => g.kind === 'BEROEP' && g.received?.id === id);

/** De uitspraak op een beroep, als die er is: de eerste. */
export const uitspraakOp = (beroep) =>
  state.rechter.find((g) => g.refers_to === beroep && g.kind === 'UITSPRAAK') ?? null;

/**
 * Elke uitspraak op een beroep, in volgorde: na een tussenuitspraak (Awb
 * 8:80a) volgt de einduitspraak op hetzelfde beroep.
 */
export const uitsprakenOp = (beroep) =>
  state.rechter.filter((g) => g.refers_to === beroep && g.kind === 'UITSPRAAK');

/** Of uitspraak `u` een tussenuitspraak is, zoals zij het zelf zegt. */
export const isTussenuitspraak = (u) => u?.outputs?.tussenuitspraak === true;

/**
 * Of `g` de mededeling van het bestuursorgaan na een tussenuitspraak is
 * (Awb 8:51b): een executogram van dat feit in de kroniek van de rechtbank.
 */
export const isMededeling = (g) =>
  g.kind === 'EXECUTOGRAM' && `${g.basis}#${g.fact}` === state.casus?.mededeling;

/** De mededeling die de rechtbank over uitspraak `id` ontving, als die er is. */
export const mededelingOp = (id) =>
  state.rechter.find((g) => isMededeling(g) && g.refers_to === id) ?? null;

/**
 * Waar een beroep staat na de laatste uitspraak erop: `open` zonder
 * uitspraak, `herstel` na een tussenuitspraak zonder mededeling, `einde`
 * na een tussenuitspraak met mededeling (de rechtbank doet einduitspraak),
 * en `klaar` na een uitspraak die geen tussenuitspraak is.
 */
export function standVan(beroep) {
  const laatste = uitsprakenOp(beroep).at(-1);
  if (!laatste) return 'open';
  if (!isTussenuitspraak(laatste)) return 'klaar';
  return mededelingOp(laatste.id) ? 'einde' : 'herstel';
}

/**
 * Of besluit `id` van de cel van de casus al bij de rechtbank ligt, omdat
 * het het bestreden besluit verving terwijl het beroep liep: dat beroep gaat
 * er dan ook over (Awb 6:19), dus een nieuw beroep staat er niet tegen open.
 */
export const onderLopendBeroep = (id) =>
  state.rechter.some((g) => isMededeling(g) && g.received?.id === id);

/** Of `g` de ontvangst van een uitspraak is: een executogram van dat feit. */
export const isOntvangst = (g) =>
  g.kind === 'EXECUTOGRAM' && `${g.basis}#${g.fact}` === state.casus?.ontvangst;

/** De ontvangsten van een uitspraak die naar besluit `id` verwijzen. */
export const ontvangstenBij = (id) =>
  state.grammen.filter((g) => isOntvangst(g) && g.refers_to === id);

/** De ontvangst van uitspraak `id` in de kroniek van de casus, als die er is. */
export const ontvangstVan = (id) =>
  state.grammen.find((g) => isOntvangst(g) && g.received?.id === id) ?? null;

/**
 * De ontvangsten waarop het bestuursorgaan nog opnieuw moet beslissen: de
 * rechter droeg een nieuw besluit op, en er is nog geen besluit dat
 * na die ontvangst is genomen.
 */
export const openOntvangsten = () =>
  state.grammen.filter(
    (g) =>
      isOntvangst(g) &&
      g.fields?.opdracht_nieuw_besluit === true &&
      !state.grammen.some((x) => x.after === g.id),
  );

/**
 * De ontvangen tussenuitspraken waarop het bestuursorgaan nog moet meedelen
 * of en hoe het het gebrek herstelt (Awb 8:51b).
 */
export const openTussenuitspraken = () =>
  state.grammen.filter(
    (g) => isOntvangst(g) && g.fields?.tussenuitspraak === true && !mededelingOp(g.received?.id),
  );

/** De beslissing die het bestuursorgaan nam na ontvangst `id`, als die er is. */
export const besluitNa = (id) => state.grammen.find((g) => g.after === id) ?? null;

/**
 * Of het bestuursorgaan op `datum` nog binnen de termijn van de rechter is
 * (Awb 8:72 lid 4 onder b): `null` zonder termijn. De cel dwingt de termijn
 * niet af; de app toont hem.
 */
export function binnenTermijn(ontvangst, datum, veld = 'termijn_nieuw_besluit_einddatum') {
  const termijn = ontvangst.fields?.[veld];
  return termijn ? datum <= termijn : null;
}
