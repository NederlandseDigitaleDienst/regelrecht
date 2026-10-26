/**
 * The introductory deck, as data. PresentationDeck.vue knows how to render
 * each kind; this file holds the story.
 *
 * Kinds: `title` (title, subtitle, footer), `statement` (a few large lines),
 * `section` (a title with plain lines), `bullets` (title, lead, bullets,
 * optionally a `figure`: a diagram from Diagram.vue by name, or a YAML/JSON
 * fragment as `code`), and `closing` (title, lines, and the demo scenarios as
 * buttons). In a line, `**bold**` is bold and `` `name` `` is code.
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
  'fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal',
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

const BINDING = `# fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal, artikel 2
implements:
  - law: fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal
    article: '1'
    open_term: aanvrager_heeft_geldige_gehandicaptenparkeerkaart
execution:
  input:
    - name: antwoord_geldige_kaart
      type: boolean
      nullable: true
      source:
        reduction: aanvrager_heeft_geldige_gehandicaptenparkeerkaart
        parameters:
          bsn_aanvrager: $bsn_aanvrager
          besluit_datum: $besluit_datum`;

const OWN = `# reductie_geldige_gehandicaptenparkeerkaart, artikel 1
input:
  - name: geldende_besluiten
    type: array
    source:
      regulation: reductie_algemene_wet_bestuursrecht
      output: geldende_besluiten
      parameters:
        basis: besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49
# …
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
    # … en kaart_geldig_tot niet vóór besluit_datum
body: true
combine: OR`;

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
    lead: 'Kreeg de aanvrager eerder een kaart? Geldt dat besluit nog?',
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
      'Het ontstaat bij de organisatie die het feit vaststelt, en blijft daar.',
      'In RegelRecht is het een aanvraag, bezwaar, beroep, besluit of **executogram**.',
      'Een executogram legt vast dat iets is gedaan.',
      'Bijvoorbeeld: de GGD heeft onderzocht, of de uitspraak is ontvangen.',
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
      'In RegelRecht heeft elke organisatie een cel: een gemeente, de GGD, een rechtbank.',
      'De cel legt vast en beslist via de engine; wat ze controleert, komt uit de wet.',
      'Het is een map met `cel.yaml`: wie de cel is en welke antwoorden ze geeft.',
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
      'Beleid koppelt een open begrip in de wet aan dat antwoord.',
    ],
    figure: {
      code: BINDING,
      source: 'corpus/regulation/nl/uitvoeringsbeleid/fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal/2026-01-01.yaml',
      label: 'Het beleid van Wilgendaal koppelt een open begrip aan een antwoord',
    },
  },
  {
    kind: 'bullets',
    overline: 'Begrip',
    title: 'Synthese',
    bullets: [
      'Een wetsartikel neemt antwoorden van meer cellen samen tot één uitkomst.',
      'Dat gebeurt bij wie beslist: de afnemer.',
      'Zo blijft elke cel baas over haar eigen feiten.',
      'Elke cel reduceert alleen haar eigen kroniek.',
      'In RegelRecht doet de engine dit als hij een artikel uitvoert met antwoorden van meer cellen.',
    ],
    figure: { diagram: 'synthese', label: 'Een artikel neemt de antwoorden van twee cellen samen' },
  },
  {
    kind: 'bullets',
    overline: 'Wat volgt',
    title: 'Uitleg zonder opgeslagen redenen',
    lead: 'In RegelRecht bewaart de cel geen redenen.',
    bullets: [
      'Ze neemt het besluit opnieuw, met de wet en de kroniek van die dag.',
      'Komt er iets anders uit, dan faalt de uitleg.',
      'Een antwoord uit een reductie is daarin één stap.',
      'Die reductie kan de cel ook opnieuw draaien.',
    ],
  },
  {
    kind: 'bullets',
    overline: 'Wat volgt',
    title: 'Afschriften tussen cellen',
    lead: 'In RegelRecht krijgt een cel van een andere cel alleen een afschrift.',
    bullets: [
      'De paper laat een cel altijd bij de bron kijken. Hier volgen we de wet, die afschriften voorschrijft.',
      'Bij een beroepschrift gaat een afschrift van het besluit mee (Awb 6:5).',
      'De rechtbank stuurt partijen een afschrift van de uitspraak (Awb 8:79).',
      'De gemeente legt de ontvangst vast in haar eigen kroniek.',
    ],
    figure: { diagram: 'afschrift', label: 'Afschriften tussen de gemeente en de rechtbank' },
  },
  {
    kind: 'section',
    overline: 'Deel 2',
    title: 'Praktijk: de gehandicaptenparkeerkaart',
    lines: [
      'Een aanvraag bij de fictieve gemeente Wilgendaal, onder artikel 49 van het BABW.',
      'Het beleid van Wilgendaal is verzonnen, als eigen keuze in de modellering.',
    ],
  },
  {
    kind: 'bullets',
    overline: 'De vraag',
    title: 'Heeft de aanvrager al een geldige kaart?',
    lead: 'Artikel 49 van het BABW: het college kan een kaart verstrekken.',
    bullets: [
      'Hoe het die bevoegdheid gebruikt, laat de wet open: `bevoegdheid_tot_verstrekken_gebruikt`.',
      'Het beleid van Wilgendaal (artikel 1): geen kaart naast een geldige kaart.',
      'Dat is een nieuw open begrip: `aanvrager_heeft_geldige_gehandicaptenparkeerkaart`.',
      'Artikel 2 van het beleid koppelt het aan het antwoord van de eigen cel.',
    ],
  },
  {
    kind: 'bullets',
    overline: 'De keten',
    title: 'Van de wet naar de kroniek',
    lead: 'De engine volgt de open begrippen tot het antwoord uit de kroniek.',
    bullets: [],
    figure: { diagram: 'keten', label: 'De keten van artikel 49 tot de kroniek van Wilgendaal' },
  },
  {
    kind: 'bullets',
    overline: 'De Awb-reductie',
    title: 'Welke besluiten gelden nog?',
    lead: '`reductie_algemene_wet_bestuursrecht` geeft van alle besluiten onder één artikel de `geldende_besluiten`.',
    bullets: [
      'Herroepen bij bezwaar (Awb 7:11): telt niet meer.',
      'Vernietigd door de rechter (Awb 8:72): telt niet meer.',
      'Wat een vernietigde beslissing op bezwaar in de plaats nam, vervalt mee.',
      'In de plaats gesteld bij bezwaar of door de rechter: telt wel.',
      'Deze reductie hoort bij geen cel. Ze leest de kroniek van de cel waarvoor ze draait.',
    ],
  },
  {
    kind: 'bullets',
    overline: 'De gemeente',
    title: 'Wat de gemeente zelf doet',
    lead: 'Een FOREACH over `geldende_besluiten`.',
    bullets: [
      'Telt: een verstrekte kaart, zelfde aanvrager, nog geldig op de dag van het besluit.',
      'Herroepen of vernietigd ziet ze niet: dat deed de Awb-reductie al.',
      'Gaf de rechter zelf een kaart, dan zwijgt ze. De einddatum staat niet in haar kroniek.',
    ],
    figure: {
      code: OWN,
      source: 'corpus/cells/gemeente_wilgendaal/reductions/reductie_geldige_gehandicaptenparkeerkaart/2026-01-01.yaml',
      label: 'Uit de reductie van de gemeente, ingekort',
    },
  },
  {
    kind: 'bullets',
    overline: 'Synthese',
    title: 'Eén besluit, antwoorden van twee cellen',
    lead: 'Het besluit over de kaart (BABW artikel 49) neemt twee antwoorden samen.',
    bullets: [
      'De cel van de GGD antwoordt uit haar kroniek: was er een onderzoek, en wat zegt het advies?',
      'De cel van Wilgendaal antwoordt uit haar eigen kroniek: heeft de aanvrager al een geldige kaart?',
      'Het beleid van Wilgendaal koppelt elk antwoord aan een open begrip.',
      'Artikel 49 maakt er één uitkomst van: de kaart wordt verstrekt of geweigerd.',
    ],
    figure: { diagram: 'synthese-kaart', label: 'Het besluit over de kaart neemt de antwoorden van de GGD en Wilgendaal samen' },
  },
  {
    kind: 'closing',
    overline: 'Nu live',
    title: 'Speel het na',
    lines: ["Twee scenario's van de gehandicaptenparkeerkaart, elk met een gids naast de pagina."],
  },
];
