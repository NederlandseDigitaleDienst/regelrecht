/**
 * The theory page (TheoriePagina.vue), as data: what a visitor who opens the
 * app for the first time needs to know to read its screens. One sentence or
 * two per term, and two pictures: the chronicle of one cell as a timeline,
 * and two cells with what passes between them.
 *
 * The words are those of the screens and the deck (slides.js): cel, kroniek,
 * vastlegging, chronolexogram, bevinding, reductie, heroverweging. The
 * example follows one scenario of one case (`casus`, `scenario`);
 * tests/theorie.test.js checks its steps and days against the case file.
 */

/** The case and the scenario the example follows. */
export const casus = 'gehandicaptenparkeerkaart';
export const scenario = 'Bezwaar, en een nieuw advies';

export const intro = {
  overline: 'Theorie',
  title: 'Wat je hier ziet',
  lead:
    'Je speelt een aanvraag, het besluit erop en het bezwaar ertegen na. ' +
    'Je ziet hoe een uitvoerder vastlegt wat er gebeurde en dat bij een volgend besluit terugleest. ' +
    'Die manier van vastleggen heet chronolexografie. ' +
    'Het voorbeeld op deze pagina is de gehandicaptenparkeerkaart in Wilgendaal, een fictieve gemeente.',
};

/**
 * The chronicle of the municipality in the scenario. `stap` is the step of
 * the scenario that records the entry; `besluit` marks a decision, which the
 * picture draws with a filled marker.
 */
export const kroniek = {
  title: 'De cel en haar kroniek',
  text: [
    'Een cel is een organisatie die haar eigen kroniek bijhoudt, zoals een gemeente, een school of de GGD.',
    'De kroniek is de rij vastleggingen van een cel, op volgorde: de aanvraag, de feiten, de besluiten. Een vastlegging heet een chronolexogram.',
    'Er komt alleen iets bij. Er wordt nooit iets in gewijzigd of uit verwijderd.',
  ],
  bijschrift: 'De kroniek van de gemeente Wilgendaal bij één aanvraag. Een besluit heeft een gevuld nummer.',
  vastleggingen: [
    { stap: 'aanvraag', dag: '2026-10-13', soort: 'Aanvraag', tekst: 'Iemand vraagt een gehandicaptenparkeerkaart.' },
    { stap: 'vastleggen', dag: '2026-10-16', soort: 'Feit', tekst: 'Het advies van de GGD is ontvangen. Het vindt geen criterium.' },
    { stap: 'besluit', dag: '2026-10-20', soort: 'Besluit', tekst: 'De kaart is geweigerd.', besluit: true },
    { stap: 'bezwaar', dag: '2026-11-01', soort: 'Bezwaar', tekst: 'De aanvrager maakt bezwaar tegen de weigering.' },
    { stap: 'vastleggen', dag: '2026-11-10', soort: 'Feit', tekst: 'Een nieuw advies van de GGD is ontvangen. Het vindt een langdurige loopbeperking.' },
    {
      stap: 'beslissing_op_bezwaar',
      dag: '2026-11-20',
      soort: 'Beslissing op bezwaar',
      tekst: 'Het bezwaar is gegrond. De weigering is herroepen en de kaart is verstrekt.',
      besluit: true,
    },
  ],
};

export const besluit = {
  title: 'Het besluit volgt uit de wet en de kroniek',
  text: [
    'De wet is vastgelegd als regels die een computer kan uitvoeren. De engine voert het artikel uit met de aanvraag en met wat in de kroniek staat.',
    'Waar de wet het oordeel aan het bestuursorgaan laat, geeft het bestuursorgaan een bevinding. In deze verkenning doe je dat zelf, op het tabblad Bestuursorgaan.',
  ],
};

/**
 * Two cells and what passes between them: each with the entry it records in
 * its own chronicle, on the day of the first advice in the scenario.
 */
export const cellen = {
  title: 'Een andere cel legt zelf vast',
  text: [
    'De GGD is een andere cel, met een eigen kroniek. Zij legt daarin vast dat zij een advies uitbracht.',
    'De gemeente legt in haar eigen kroniek vast dat zij het advies ontving. Alleen de cel zelf voegt iets toe aan haar kroniek.',
    'Bij beroep gaat het ook zo: de rechtbank zendt een afschrift van de uitspraak, en de gemeente legt de ontvangst vast.',
  ],
  bijschrift: 'Twee cellen. De GGD brengt het advies uit, de gemeente ontvangt het. Elk legt dat vast in de eigen kroniek.',
  dag: '2026-10-16',
  van: { naam: 'GGD Wilgendaal', vastlegging: 'Geneeskundig advies uitgebracht' },
  pijl: 'advies',
  naar: { naam: 'Gemeente Wilgendaal', vastlegging: 'Geneeskundig advies ontvangen' },
};

export const begrippen = [
  {
    title: 'Reductie',
    text: [
      'Bij een besluit hoort soms een vraag over eerder: heeft de aanvrager al een geldige kaart van de gemeente?',
      'De cel beantwoordt die vraag met een reductie: een rekenregel die haar eigen kroniek terugleest.',
    ],
  },
  {
    title: 'Bezwaar',
    text: [
      'Is het bezwaar ontvankelijk, dan heroverweegt het bestuursorgaan: het artikel wordt opnieuw toegepast op dezelfde aanvraag. Dat gebeurt op de dag van de beslissing op bezwaar, met de kroniek van die dag.',
      'In het voorbeeld staat er dan een nieuw advies in de kroniek (vastlegging 5). De uitkomst verschilt, dus de beslissing op bezwaar herroept de weigering. De weigering blijft in de kroniek staan.',
    ],
  },
  {
    title: 'Waarom',
    text: [
      'Bij elk besluit staat waarom het zo uitviel: welk artikel is toegepast en welke vastlegging uit de kroniek is gelezen.',
      'De cel neemt het besluit daarvoor opnieuw, met de wet en de kroniek van die dag. Je vindt het op het tabblad Logboek.',
    ],
  },
];

/** The tabs of a case, with what the visitor does on each. */
export const tabbladen = {
  title: 'De tabbladen',
  rijen: [
    { naam: "Scenario's", tekst: 'Kies een verloop en speel het stap voor stap na.' },
    { naam: 'Aanvrager', tekst: 'Dien een aanvraag of een bezwaar in.' },
    { naam: 'Bestuursorgaan', tekst: 'Beslis op een aanvraag of een bezwaar.' },
    { naam: 'Andere cellen', tekst: 'Leg vast wat een andere cel vaststelt, zoals het advies van de GGD.' },
    { naam: 'Kroniek', tekst: 'Bekijk elk chronolexogram, op volgorde.' },
    { naam: 'Logboek', tekst: 'Bekijk elke handeling, en bij een besluit waarom het zo uitviel.' },
  ],
};

const MAANDEN = ['jan', 'feb', 'mrt', 'apr', 'mei', 'jun', 'jul', 'aug', 'sep', 'okt', 'nov', 'dec'];

/** `2026-10-13` → `13 okt 2026`, as the deck writes a day. */
export function dagTekst(dag) {
  const [jaar, maand, d] = dag.split('-').map(Number);
  return `${d} ${MAANDEN[maand - 1]} ${jaar}`;
}
