// De stapel op een punt: welke plannen er liggen, welk plan ter plaatse de regels geeft,
// welke lagen daarbovenop komen en welke opzij gaan, met per keuze de reden.
//
// De gelaagdheidsregel staat in plannen.json in de bron, onder `regel`. Stappen:
//   3  vastgesteld TAM-plan op het punt: geen bestemmingsplannen, alleen de bruidschat
//   4  anders: het nieuwste basis-, wijzigings- of uitwerkingsplan is het basisplan
//   5  daarbovenop: paraplu-artikelen uit de tabellen, archeologie, Ondergrondse hoogspanning
// Een ontwerp-TAM geldt niet en komt apart als "beoogd".
//
// Dit is een selectie van plannen, geen toets: rekenen doet de RegelRecht-engine.
import { objectenOpPunt } from './geo.js';

const BASIS = new Set(['basisplan', 'wijzigingsplan', 'uitwerkingsplan']);

/** Feiten op het punt uit één plan: bestemming, bouwvlak, maatvoering, aanduidingen. */
export function feitenUitPlan(fc, p) {
  const op = objectenOpPunt(fc, p).map((f) => f.properties);
  const van = (soort) => op.filter((o) => o.soort === soort);
  const feiten = {
    enkelbestemming: van('enkelbestemming'),
    dubbelbestemmingen: van('dubbelbestemming'),
    binnenBouwvlak: van('bouwvlak').length > 0,
    maatvoering: van('maatvoering'),
    bouwaanduidingen: van('bouwaanduiding'),
    functieaanduidingen: van('functieaanduiding'),
    gebiedsaanduidingen: van('gebiedsaanduiding'),
    lettertekenaanduidingen: van('lettertekenaanduiding'),
  };
  // Voor afstanden tot andere locaties van hetzelfde plan (bijvoorbeeld TAM Het Ambacht 70 en 71).
  Object.defineProperty(feiten, 'punt', { value: p, enumerable: false });
  Object.defineProperty(feiten, 'laag', { value: fc, enumerable: false });
  return feiten;
}

function inPlangebied(fc, p) {
  return objectenOpPunt(fc, p).some((f) => f.properties.soort === 'plangebied');
}

function reden(gekozen) {
  if (gekozen.rol === 'wijzigingsplan') return `hier gewijzigd door wijzigingsplan ${gekozen.naam} (${gekozen.datum})`;
  if (gekozen.rol === 'uitwerkingsplan') return `hier uitgewerkt door uitwerkingsplan ${gekozen.naam} (${gekozen.datum})`;
  return `herzien door ${gekozen.naam} (${gekozen.datum})`;
}

/**
 * @param {[number, number]} p     punt [lon, lat]
 * @param {object} index           plannen.json
 * @param {Map<string, object>} lagen  plan-id naar FeatureCollection (met bbox, zie geo.indexeer)
 */
export function bepaalStapel(p, index, lagen) {
  const plannen = index.plannen.filter((pl) => inPlangebied(lagen.get(pl.id), p));
  const opPunt = plannen.map((pl) => pl.id);
  const stapel = {
    punt: p, opPunt, tijdelijk: 'geen', tam: null, basisplan: null,
    opzij: [], aanvullend: [], beoogd: null, meldingen: [],
  };
  const paraplu = plannen.find((pl) => pl.rol === 'paraplu');
  const facetten = plannen.filter((pl) => pl.rol === 'facet');

  // Beoogd: ontwerp-TAM, los van alles wat geldt.
  const ontwerp = plannen.find((pl) => pl.rol === 'tam-ontwerp');
  if (ontwerp) {
    const feiten = feitenUitPlan(lagen.get(ontwerp.id), p);
    const interferentie = ontwerp.uitzondering_vlak
      && feiten.enkelbestemming.some((e) => e.naam === ontwerp.uitzondering_vlak);
    stapel.beoogd = { plan: ontwerp, feiten, interferentie };
  }

  // Stap 3: vastgesteld TAM-plan.
  const tam = plannen.filter((pl) => pl.rol === 'tam').sort((a, b) => b.datum.localeCompare(a.datum))[0];
  if (tam) {
    stapel.tijdelijk = 'alleen-bruidschat';
    stapel.tam = { plan: tam, feiten: feitenUitPlan(lagen.get(tam.id), p) };
    for (const pl of plannen) {
      if (pl === tam || pl.rol === 'tam-ontwerp') continue;
      stapel.opzij.push({ plan: pl, reden: `buiten toepassing door artikel 2 van ${tam.naam}` });
    }
    return stapel;
  }

  // Stap 4: het nieuwste basisplan.
  const basis = plannen.filter((pl) => BASIS.has(pl.rol))
    .sort((a, b) => b.datum.localeCompare(a.datum) || a.id.localeCompare(b.id));
  if (basis.length) {
    const [gekozen, ...ouder] = basis;
    stapel.tijdelijk = 'basisplan';
    stapel.basisplan = { plan: gekozen, feiten: feitenUitPlan(lagen.get(gekozen.id), p) };
    if (gekozen.alleen_plangrens) {
      // Oud plan in IMROPCP2008: de GML kent alleen de plangrens.
      stapel.meldingen.push({
        soort: 'alleen-plangrens',
        tekst: `Van ${gekozen.naam} is alleen de plangrens digitaal. Bestemming, bouwvlak en `
          + 'maatvoering staan op de plankaart (pdf); de kaart kan ze op dit punt niet aflezen.',
      });
    }
    for (const pl of ouder) {
      if (pl.datum === gekozen.datum) {
        stapel.meldingen.push({
          soort: 'gelijke-datum',
          tekst: `${pl.naam} en ${gekozen.naam} zijn op dezelfde dag vastgesteld en overlappen hier. `
            + 'Welk plan voorgaat, volgt niet uit de datum. Beide staan in de stapel.',
        });
        stapel.opzij.push({ plan: pl, reden: 'zelfde datum als het basisplan; voorrang onbekend', twijfel: true });
      } else {
        stapel.opzij.push({ plan: pl, reden: reden(gekozen) });
      }
    }
  } else {
    const ontbrekend = (index.ontbrekend ?? []).find((o) => opPunt.includes(o.binnen));
    stapel.meldingen.push(ontbrekend
      ? { soort: 'bron-ontbreekt', tekst: `${ontbrekend.tekst} Grond: ${ontbrekend.grond}.` }
      : { soort: 'geen-plan', tekst: 'Op dit punt ligt geen basisplan uit de opgehaalde plannen. Mogelijk ontbreekt een plan in de bronnen.' });
  }

  // Stap 5: aanvullende lagen.
  if (paraplu) {
    const fc = lagen.get(paraplu.id);
    const artikelen = stapel.basisplan?.plan.paraplu ?? [];
    if (artikelen.length) {
      stapel.aanvullend.push({
        plan: paraplu, soort: 'paraplu-artikelen',
        artikelen: artikelen.map((nr) => ({ nr, ...index.paraplu.artikelen[nr] })),
        tekst: `vervangt deze artikelen van ${stapel.basisplan.plan.naam} (vervangingstabellen van het parapluplan)`,
      });
    }
    const archeo = feitenUitPlan(fc, p).dubbelbestemmingen;
    if (archeo.length) {
      const na = stapel.basisplan && stapel.basisplan.plan.datum > index.paraplu.vastgesteld;
      if (na) {
        stapel.meldingen.push({ soort: 'archeologie-open', tekst: index.paraplu.archeologie.open_na_vaststelling });
      }
      stapel.aanvullend.push({ plan: paraplu, soort: 'archeologie', objecten: archeo, twijfel: na });
    }
    if (!artikelen.length && !archeo.length) {
      stapel.opzij.push({
        plan: paraplu,
        reden: stapel.basisplan
          ? `${stapel.basisplan.plan.naam} staat niet in de tabellen van de paraplu en er ligt hier geen archeologische waarde`
          : 'geen basisplan en geen archeologische waarde op dit punt',
      });
    }
  }
  for (const facet of facetten) {
    const feiten = feitenUitPlan(lagen.get(facet.id), p);
    const objecten = [...feiten.dubbelbestemmingen, ...feiten.enkelbestemming];
    if (objecten.length) stapel.aanvullend.push({ plan: facet, soort: 'facet', objecten });
    else stapel.opzij.push({ plan: facet, reden: 'plangebied ligt hier, maar zonder strook of bestemming op dit punt' });
  }
  return stapel;
}
