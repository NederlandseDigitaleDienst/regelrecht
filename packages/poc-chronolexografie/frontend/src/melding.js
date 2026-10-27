/**
 * Een weigering van de cel in het Nederlands. De cel meldt in het Engels wat
 * ze weigert en waarom; de banner toont deze korte zin en houdt de melding
 * zelf achter "Details". Hier staan de weigeringen die een bezoeker met de
 * formulieren kan veroorzaken; de rest krijgt de algemene zin. De gevallen in
 * tests/melding.test.js zijn kopieën van de meldingen in packages/cel
 * (cell.rs, chronicle.rs): wie daar een melding herschrijft, past ze mee aan.
 */
import { hoofdletter, kort, leesbaar, state } from './state.js';

const vertalingen = [
  // Een besluit wordt maar één keer vervangen (de cel, bij een tweede
  // beslissing op bezwaar die hetzelfde besluit zou vervangen).
  [
    /in the place of '([^']+)', which chronolexogram '([^']+)' already replaces/,
    (besluit, vervanger) =>
      `Besluit ${kort(besluit)} is na een eerder bezwaar al vervangen door besluit ${kort(vervanger)}. Een besluit kan maar één keer worden vervangen. Er is niets vastgelegd.`,
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
  // Een oordeel dat een artikel voor deze aanvraag aan het bestuursorgaan
  // laat, met een eigen open begrip: de parkeerkaart zonder advies van de
  // GGD, een vakantie die de cel niet kent, de hardheidsclausule bij het
  // maximum (kern::voorvertonen).
  [
    /the policy leaves '([^']+)' to the authority for this case/,
    (begrip) =>
      `De regels laten "${leesbaar(begrip)}" voor deze aanvraag aan ${state.casus?.rol ?? 'het bestuursorgaan'} over. Geef die bevinding.`,
  ],
  // Een bevinding die de wet per geval aan het bestuursorgaan laat, en die
  // het nog niet gaf (de engine).
  [
    /Required open term '([^']+)' on \S+ is decided per case by .*no judgement was given/,
    (begrip) =>
      `${hoofdletter(state.casus?.rol ?? 'het bestuursorgaan')} heeft "${leesbaar(begrip)}" nog niet beoordeeld, en het besluit heeft die bevinding nodig.`,
  ],
  [/the application '[^']+' is already decided on/, () => 'Hierop is al besloten.'],
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
  [
    /lacks '([^']+)', which the decision of \S+ needs \(Awb 4:5\)/,
    (veld) =>
      `De aanvraag mist "${leesbaar(veld)}", en het besluit heeft dat nodig (Awb 4:5). Vul het aan in een nieuwe aanvraag.`,
  ],
  [
    /asks '([^']+)' of the deciding authority/,
    (veld) =>
      `Het bestuursorgaan heeft "${leesbaar(veld)}" nog niet beoordeeld, en het besluit heeft die bevinding nodig.`,
  ],
  [
    /'([^']+)' is .*, which does not fit a/,
    (veld) => `De waarde van "${leesbaar(veld)}" past niet bij wat de wet vraagt.`,
  ],
  [
    /the law does not ask '([^']+)' in the (\S+) of/,
    (veld, soort) =>
      `De wet vraagt niet om "${leesbaar(veld)}" bij ${soort === 'BEZWAAR' ? 'het bezwaar' : 'de aanvraag'}.`,
  ],
];

/** De Nederlandse zin bij `fout`, of null als er geen vertaling voor is. */
export function vertaling(fout) {
  for (const [patroon, zin] of vertalingen) {
    const gevonden = String(fout).match(patroon);
    if (gevonden) return zin(...gevonden.slice(1));
  }
  return null;
}

/**
 * De Nederlandse zin bij de melding `fout` van de cel, de engine of de
 * browser. Die melden in het Engels.
 */
export function melding(fout) {
  return vertaling(fout) ?? 'Wat er misging, staat onder Details, in het Engels.';
}
