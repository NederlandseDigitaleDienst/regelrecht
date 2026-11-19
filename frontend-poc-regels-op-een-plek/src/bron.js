// De gegevens van deze verkenning: de regels in YAML, de kaart en de vooraf gerekende uitkomsten.
// Ze staan niet in deze repository. Het poc-portaal haalt ze op uit een afgeschermde bron en
// serveert ze achter het wachtwoord van de poc op <basis>bron/spoorzone, als één JSON-object:
// { naam, versie, bestanden: { <pad>: <tekst> }, melding? } (packages/poc-portal/src/bron.rs).
// Lokaal serveert de dev-server dezelfde route uit de map in POC_BRON_SPOORZONE
// (scripts/bron-dev.js).
//
// Het portaal geeft alleen YAML-bestanden door. Een bestand dat geen YAML is, staat in de bron met
// .yaml achter de naam; JSON en GeoJSON zijn geldige YAML. bestand('demo.json') vindt dus ook
// demo.json.yaml.
import { b } from './eerste/basePad.js';

export const BRON = 'spoorzone';

let bron = null;

/** Haal de bron één keer op. Gooit een Error met de reden als dat niet lukt. */
export async function laadBron(haal = fetch) {
  if (bron) return bron;
  const r = await haal(b(`bron/${BRON}`));
  if (!r.ok) {
    const reden = (await r.text().catch(() => '')).trim();
    throw new Error(reden || `de bron gaf status ${r.status}`);
  }
  let b2;
  try {
    b2 = await r.json();
  } catch {
    throw new Error('de bron gaf geen JSON');
  }
  if (!b2?.bestanden) throw new Error('de bron heeft geen bestanden');
  bron = b2;
  return bron;
}

/**
 * De bron als hij geladen is: met `versie` (de commit van de bron) en, als het portaal de bron
 * niet opnieuw kon ophalen, `melding` (dan is dit de laatste goede kopie).
 */
export const geladenBron = () => bron;

/** Zet de bron rechtstreeks, voor tests. */
export function zetBron(b) {
  bron = b;
}

/** De tekst van een bestand uit de bron, op zijn pad zoals de app het kent. */
export function bestand(pad) {
  if (!bron) throw new Error('de bron is nog niet geladen');
  const p = String(pad).replace(/^\.?\//, '');
  const tekst = bron.bestanden[p] ?? bron.bestanden[`${p}.yaml`];
  if (tekst === undefined) throw new Error(`niet in de bron: ${p}`);
  return tekst;
}

/** Een JSON-bestand uit de bron. */
export const json = (pad) => JSON.parse(bestand(pad));
