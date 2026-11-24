/**
 * Een weigering van de cel in het Nederlands. De cel meldt in het Engels wat
 * ze weigert en waarom; de banner toont deze korte zin en houdt de melding
 * zelf achter "Details". Hier staan de weigeringen die een bezoeker met de
 * formulieren kan veroorzaken; de rest krijgt de algemene zin. De gevallen in
 * tests/melding.test.js zijn kopieën van de meldingen in packages/cel
 * (cell.rs, chronicle.rs): wie daar een melding herschrijft, past ze mee aan.
 */
import { weigeringInWoorden } from '@regelrecht/frontend-shared/chronolex/weigering';
import { kort, leesbaar, state } from './state.js';

const vertalingen = [
  // Een besluit wordt maar één keer vervangen (de cel, bij een tweede
  // beslissing op bezwaar die hetzelfde besluit zou vervangen).
  [
    /in the place of '([^']+)', which the decision '([^']+)' already replaces/,
    (besluit, beslissing) =>
      `Besluit ${kort(besluit)} is al vervangen door de beslissing op bezwaar ${kort(beslissing)}. Een besluit kan maar één keer worden vervangen. Er is niets vastgelegd.`,
  ],
  // De kroniek loopt alleen vooruit; dezelfde dag mag.
  [
    /before the last chronolexogram of the chronicle \('[^']+', (\d{4}-\d{2}-\d{2})\)/,
    (laatste) =>
      `De speeldatum ligt vóór de laatste stap in de kroniek (${laatste}). De kroniek loopt alleen vooruit: kies die dag of een latere.`,
  ],
  // De cel beantwoordt een open begrip uit haar kroniek, maar op een
  // speeldatum vóór de dag dat haar reductie geldt, heeft ze er geen.
  [
    /open term '([^']+)' of \S+ is filled by a reduction in \S+, but none is in force on (\d{4}-\d{2}-\d{2})/,
    (begrip, dag) =>
      `De cel beantwoordt "${leesbaar(begrip)}" uit haar kroniek, maar op ${dag} geldt haar reductie nog niet. Kies een latere speeldatum.`,
  ],
  // Uitleg gevraagd over een begrip dat het besluit de cel niet vroeg.
  [
    /the decision '([^']+)' did not ask the cell of '[^']+' for '([^']+)'/,
    (besluit, begrip) =>
      `Besluit ${kort(besluit)} vroeg de cel niet om "${leesbaar(begrip)}", dus er is geen antwoord om uit te leggen.`,
  ],
  [/the application '[^']+' is already decided on/, () => 'Hierop is al besloten.'],
  // Een spel dat dit tabblad bewaarde voordat een beslissing op bezwaar het
  // nieuwe besluit zelf ging houden (de cel, Cell::restore). Het spel is al
  // weggegooid (state.js, hervatten); de cel zet het niet om.
  [
    /the chronicle of '[^']+' is in an older form/,
    () => 'Dit spel is opgeslagen in een oudere vorm en kan niet verder. Begin opnieuw met een casus.',
  ],
  // Hetzelfde afschrift van een uitspraak voor de tweede keer toegezonden
  // (de cel, Cell::record_receipt).
  [
    /the copy of '([^']+)' of '[^']+' is already received/,
    (uitspraak) => `Het afschrift van uitspraak ${kort(uitspraak)} is al ontvangen. Er is niets vastgelegd.`,
  ],
  // Een stap voordat er een casus gekozen is.
  [/no case is chosen/, () => 'Er is geen casus gekozen. Kies een casus.'],
  // De cel laadt niet in deze browser (api.js): de WASM of de bundel kwam
  // niet binnen, of de bundel mist wat een casus nodig heeft.
  [
    /the cell did not load/,
    () => 'De cel kon in deze browser niet laden. Herlaad de pagina; wat er misging, staat onder Details, in het Engels.',
  ],
  // De cel liep vast (een panic in de WASM, api.js); daarna lukt geen stap
  // meer tot een herlaad.
  [
    /the cell stopped/,
    () => 'De cel is vastgelopen. Herlaad de pagina: je gaat verder na de laatste stap die lukte.',
  ],
];

/** De Nederlandse zin bij `fout`, of null als er geen vertaling voor is. */
export function vertaling(fout) {
  for (const [patroon, zin] of vertalingen) {
    const gevonden = String(fout).match(patroon);
    if (gevonden) return zin(...gevonden.slice(1));
  }
  // What a form can cause, shared with RegelHet.
  return weigeringInWoorden(fout, { veld: leesbaar, rol: state.casus?.rol ?? 'het bestuursorgaan' });
}

/**
 * De Nederlandse zin bij de melding `fout` van de cel, de engine of de
 * browser. Die melden in het Engels.
 */
export function melding(fout) {
  return vertaling(fout) ?? 'Wat er misging, staat onder Details, in het Engels.';
}
