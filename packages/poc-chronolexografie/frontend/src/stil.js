// De begrippen die de cel implementeert maar voor dit geval niet weet
// (`stil`, RFC-051), zoals de voorvertoning ze meldt: daarover oordeelt het
// bestuursorgaan zelf. Wat eenmaal gevraagd is, blijft staan zolang de
// speeldatum en de kroniek dezelfde zijn: of de cel het weet, hangt af van
// het geval, van wat op die dag geldt en van wat zij vastlegde, niet van de
// bevindingen. Een voorvertoning vindt alleen het eerste begrip dat het
// bestuursorgaan nog niet beoordeelde; zo verschijnen ze een voor een.
// Verandert de dag of de kroniek, dan vraagt de voorvertoning opnieuw, en
// gaat een oordeel dat niet meer gevraagd wordt weg uit `oordelen` in plaats
// van onzichtbaar mee te gaan.
import { reactive, watch } from 'vue';
import { hoofdletter, state } from './state.js';

export function stilBegrippen(...oordelen) {
  const stil = reactive({});
  watch(
    () => [state.datum, state.grammen.length],
    () => {
      for (const [id, velden] of Object.entries(stil)) {
        for (const v of velden) for (const o of oordelen) if (o[id]) delete o[id][v.naam];
        delete stil[id];
      }
    },
  );
  function meldStil(id, velden) {
    const lijst = (stil[id] ??= []);
    for (const v of velden) if (!lijst.some((x) => x.naam === v.naam)) lijst.push(v);
  }
  return { stil, meldStil };
}

// Wie het laat afweten: een artikel dat het oordeel voor deze aanvraag aan
// het bestuursorgaan laat (`beleid`), zoals het beleid zonder advies van de
// GGD bij de parkeerkaart of de hardheidsclausule van de parkeerverordening,
// of de cel uit haar kroniek.
export const nietGeweten = (v) =>
  v.beleid
    ? `${v.basis} · Dit artikel laat het oordeel voor deze aanvraag aan ${state.casus.rol} over.`
    : `${v.basis} · ${hoofdletter(state.casus.cel_naam)} weet dit niet voor deze aanvraag; ${state.casus.rol} oordeelt.`;
