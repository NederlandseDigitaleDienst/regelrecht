/**
 * The introductory deck, as data. PresentationDeck.vue knows how to render
 * each kind; this file holds the story.
 *
 * Kinds: `title` (title, subtitle, footer), `statement` (a few large lines),
 * `section` (a title with plain lines), `bullets` (title, lead, bullets,
 * optionally a `figure`: a diagram from Diagram.vue by name, or a YAML/JSON
 * fragment as `code`), and `closing` (title, lines, and as `button` the text
 * of the button that opens the explanation page). In a line, `**bold**`
 * is bold and `` `name` `` is code.
 *
 * The concept slides put the position paper
 * (https://chronolexografie.nl/position-paper/) in plain words of our own,
 * one idea per bullet. Where RegelRecht departs from the paper or adds to it,
 * the bullet starts with "In RegelRecht".
 *
 * The names of reductions, open terms and outputs are copied from the corpus;
 * tests/presentation.test.js checks each one there. A YAML fragment names
 * its `source`, the file it is cut from; the test finds its lines there in
 * order. Lines left out are marked with `# …`.
 */

export const KINDS = Object.freeze(['title', 'statement', 'section', 'bullets', 'closing']);

/** The names the slides use, which tests/presentation.test.js finds in the corpus. */
export const CORPUS_NAMES = Object.freeze([
  'reductie_geldige_gehandicaptenparkeerkaart',
  'reductie_algemene_wet_bestuursrecht',
  'beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal',
  'geldende_besluiten',
  'aanvrager_heeft_geldige_gehandicaptenparkeerkaart',
  'aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt',
  'bevoegdheid_tot_verstrekken_gebruikt',
  'gehandicaptenparkeerkaart_verstrekt',
  'kaart_geldig_tot',
  'gemeente_wilgendaal',
]);

const GRAM = `{
  "kind": "BESCHIKKING",
  "basis": "besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49",
  "recording_actor": "gemeente_wilgendaal",
  "recorded_on": "2026-10-20",
  "refers_to": "019a…",
  "fields": { "bsn_aanvrager": "999990020", "kaart_als_bestuurder": true, … },
  "outputs": {
    "gehandicaptenparkeerkaart_verstrekt": true,
    "kaart_geldig_tot": "2031-10-19",
    …
  }
}`;

const CEL = `# corpus/cells/gemeente_wilgendaal/cel.yaml
name: Het college van burgemeester en wethouders van de gemeente Wilgendaal
category: college van burgemeester en wethouders
answers:
  aanvrager_heeft_geldige_gehandicaptenparkeerkaart: reductie_geldige_gehandicaptenparkeerkaart
  aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt: reductie_geldige_gehandicaptenparkeerkaart`;

const FOREACH = `# reductie_geldige_gehandicaptenparkeerkaart, artikel 1
operation: FOREACH
collection: $geldende_besluiten
as: besluit
filter:
  operation: AND
  conditions:
    - operation: EQUALS
      subject: $besluit.gehandicaptenparkeerkaart_verstrekt
      value: true
    - operation: EQUALS
      subject: $besluit.bsn_aanvrager
      value: $bsn_aanvrager
    # …
body: true
combine: OR`;

const INPUT = `# reductie_geldige_gehandicaptenparkeerkaart, artikel 1
input:
  - name: geldende_besluiten
    type: array
    source:
      regulation: reductie_algemene_wet_bestuursrecht
      output: geldende_besluiten
      parameters:
        basis: besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49`;

export const slides = [
  {
    kind: 'title',
    title: 'Chronolexografie',
    subtitle: 'Adequaat digitaal vastleggen van de rechtstoestand',
    footer: 'Proof of concept in RegelRecht',
  },
  {
    kind: 'bullets',
    overline: 'Wat is het',
    title: 'Een kroniek per organisatie',
    bullets: [
      'Elke organisatie legt haar eigen feiten vast, bij zichzelf.',
      'Zo staat elk feit op één plek: bij wie het vaststelde.',
      'Een feit wordt nooit gewist, ook niet bij een correctie.',
      'Zo kun je altijd terugzien hoe een rechtstoestand ontstond.',
      'In RegelRecht zegt de wet van die dag welke velden een vastlegging heeft.',
    ],
  },
  {
    kind: 'bullets',
    overline: 'Waarom',
    title: 'Wetten vragen naar het verleden',
    lead: 'Heeft dit bedrijf nog recht op een parkeervergunning, gezien het aantal medewerkers en de vergunningen die het al heeft?',
    bullets: [
      'Dat antwoord ligt nu verspreid over systemen.',
      'Die bewaren vaak alleen de laatste stand.',
      'Een besluit moet later nog uit te leggen zijn, met de wet en gegevens van toen.',
      'Ook als een bezwaar of uitspraak achteraf veranderde wat geldt.',
    ],
  },
  {
    kind: 'section',
    overline: 'Deel 1',
    title: 'De begrippen',
  },
  {
    kind: 'bullets',
    overline: 'Begrip',
    title: 'Chronolexogram',
    bullets: [
      'Een chronolexogram is één vastlegging: één handeling of besluit, op één moment.',
      'Een **lexogram** legt vast dat wet- of regelgeving verandert, of gaat veranderen.',
      'Een **decretogram** legt vast dat iets is besloten.',
      'Een **executogram** legt vast dat iets is gedaan, zoals een uitbetaling.',
      'In RegelRecht legt de cel ook aanvragen, bezwaren en beroepen vast.',
    ],
    figure: { code: GRAM, label: 'Een besluit in de kroniek van Wilgendaal, ingekort' },
  },
  {
    kind: 'bullets',
    overline: 'Begrip',
    title: 'Kroniek',
    bullets: [
      'Een kroniek is de rij vastleggingen in een cel, op volgorde.',
      'Elke kroniek heeft een eigen tijdlijn.',
      'Een cel kan meer kronieken hebben, om feiten te groeperen.',
      'Er komt alleen iets bij: een herroepen besluit blijft staan.',
      'Zo kun je elk besluit later precies naspelen, met de wet en de kroniek van die dag.',
    ],
    figure: { diagram: 'kroniek', label: 'De kroniek van de gemeente Wilgendaal na twee aanvragen' },
  },
  {
    kind: 'bullets',
    overline: 'Begrip',
    title: 'Cel',
    bullets: [
      'Een cel is de plek waar vastleggingen ontstaan en bewaard worden.',
      'Alleen de cel zelf voegt er iets aan toe.',
      'In RegelRecht heeft elke organisatie een cel: een gemeente, de Gemeentelijke Gezondheidsdienst (GGD), een rechtbank.',
      'De cel legt vast en beslist via de engine; wat ze controleert, komt uit de wet.',
      'Het is een map met `cel.yaml`. Daarin staat wie de cel is en welke antwoorden ze geeft.',
    ],
    figure: { code: CEL, source: 'corpus/cells/gemeente_wilgendaal/cel.yaml', label: 'De cel van de gemeente Wilgendaal' },
  },
  {
    kind: 'bullets',
    overline: 'Begrip',
    title: 'Reductie',
    bullets: [
      'Reductie is feiten van eerder opnieuw gebruiken.',
      'Dat gebeurt altijd in de cel waar die feiten staan.',
      'Hoe een cel reduceert, bepaalt de cel zelf.',
      'RegelRecht biedt één manier: een reductie als regelwerk met een benoemd antwoord.',
      'In het fragment loopt de cel haar besluiten langs en kijkt of er een kaart bij zit voor deze aanvrager.',
    ],
    figure: {
      code: FOREACH,
      source: 'corpus/cells/gemeente_wilgendaal/reductions/reductie_geldige_gehandicaptenparkeerkaart/2026-01-01.yaml',
      label: 'Uit de reductie van de gemeente Wilgendaal, ingekort',
    },
  },
  {
    kind: 'bullets',
    overline: 'Begrip',
    title: 'Synthese',
    bullets: [
      'Een artikel neemt de antwoorden van verschillende cellen, op verschillende vragen, samen tot één uitkomst.',
      'Elke cel beantwoordt haar eigen vraag uit haar eigen kroniek, en blijft zo baas over haar eigen feiten.',
      'De synthese gebeurt bij wie vraagt, niet in een cel.',
      'In RegelRecht doet de engine dit als hij een artikel uitvoert met antwoorden van meer cellen.',
    ],
    figure: { diagram: 'synthese', label: 'Een artikel neemt de antwoorden van twee cellen op twee vragen samen' },
  },
  {
    kind: 'section',
    overline: 'Deel 2',
    title: 'Praktijk: de gehandicaptenparkeerkaart',
    lines: [
      'Een aanvraag bij de fictieve gemeente Wilgendaal, onder artikel 49 van het Besluit administratieve bepalingen inzake het wegverkeer (BABW).',
      'Het beleid van Wilgendaal is verzonnen, als eigen keuze in de modellering.',
    ],
  },
  {
    kind: 'bullets',
    overline: 'De vraag',
    title: 'Heeft de aanvrager al een gehandicaptenparkeerkaart van de gemeente?',
    lead: 'Volgens artikel 49 van het BABW kan het college een kaart verstrekken. Hoe het die bevoegdheid gebruikt, laat de wet open.',
    bullets: [
      'Wilgendaal geeft geen tweede kaart aan wie al een geldige kaart van de gemeente heeft.',
      'Zo staat het in het beleid van de gemeente (artikel 1). Het vult de ruimte in die BABW artikel 49 laat.',
      "Of de aanvrager al zo'n kaart heeft, is een nieuw open begrip.",
    ],
  },
  {
    kind: 'bullets',
    overline: 'De keten',
    title: 'Van de wet naar de kroniek',
    lead: 'Wie vraagt wat aan wie, van het artikel tot de kroniek, en hoe het antwoord terugkomt.',
    bullets: [],
    figure: { diagram: 'keten', label: 'De keten van artikel 49 tot de kroniek van Wilgendaal' },
  },
  {
    kind: 'bullets',
    overline: 'De Awb-reductie',
    title: 'Eén Awb-reductie voor elke wet',
    lead: 'De reductie van de Awb past de regels van de Awb toe op de besluiten van elke wet.',
    bullets: [
      'Wat na bezwaar en beroep nog geldt, staat zo op één plek.',
      'Elke reductie van een uitvoerder roept haar aan met de grondslag van haar eigen besluiten (`basis`).',
      'De uitvoerder schrijft alleen zijn eigen vraag. Herroepen en vernietigde besluiten zijn er al uit.',
    ],
    figure: { diagram: 'awb', label: 'Twee reducties van Wilgendaal die de Awb-reductie gebruiken' },
  },
  {
    kind: 'bullets',
    overline: 'De gemeente',
    title: 'Wat de gemeente zelf doet',
    lead: 'De reductie van de gemeente loopt alleen door de `geldende_besluiten`.',
    bullets: [
      'In het fragment haalt ze die bij de Awb-reductie, voor de besluiten onder artikel 49.',
      'Daarna telt een besluit als het een kaart verstrekte aan dezelfde aanvrager, en die kaart nog geldig is.',
      'Gaf de rechter zelf een kaart, dan zwijgt ze. De einddatum staat niet in haar kroniek.',
    ],
    figure: {
      code: INPUT,
      source: 'corpus/cells/gemeente_wilgendaal/reductions/reductie_geldige_gehandicaptenparkeerkaart/2026-01-01.yaml',
      label: 'De invoer van de reductie van de gemeente',
    },
  },
  {
    kind: 'closing',
    overline: 'Verder',
    title: 'Hoe het in elkaar zit',
    lines: ['De uitlegpagina laat met de bedrijfsparkeervergunning zien hoe regel, cel en reducties samenhangen.'],
    button: 'Naar de uitleg',
  },
];
