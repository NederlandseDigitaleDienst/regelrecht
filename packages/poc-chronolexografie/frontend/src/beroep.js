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
};

/** De beroepen bij de rechtbank tegen besluit `id` van de cel van de casus. */
export const beroepenTegen = (id) =>
  state.rechter.filter((g) => g.kind === 'BEROEP' && g.received?.id === id);

/** De uitspraak op een beroep, als die er is. */
export const uitspraakOp = (beroep) =>
  state.rechter.find((g) => g.refers_to === beroep && g.kind === 'UITSPRAAK') ?? null;

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
 * Of het bestuursorgaan op `datum` nog binnen de termijn van de rechter is
 * (Awb 8:72 lid 4 onder b): `null` zonder termijn. De cel dwingt de termijn
 * niet af; de app toont hem.
 */
export function binnenTermijn(ontvangst, datum) {
  const termijn = ontvangst.fields?.termijn_nieuw_besluit_einddatum;
  return termijn ? datum <= termijn : null;
}
