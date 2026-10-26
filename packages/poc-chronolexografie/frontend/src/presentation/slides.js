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
 * The names of reductions, open terms and outputs are copied from the corpus;
 * tests/presentation.test.js checks each one there. The fragments are cut
 * from the corpus YAML and only shortened, marked with `# …`.
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
    subtitle: 'Rechtsfeiten vastleggen waar ze ontstaan',
    footer: 'Proof of concept in RegelRecht',
  },
  {
    kind: 'bullets',
    overline: 'Wat is het',
    title: 'Een kroniek per organisatie',
    lead: 'Een organisatie legt elk rechtsfeit dat zij vaststelt vast als gedateerd stuk in haar eigen kroniek. Latere vragen beantwoordt zij uit die kroniek.',
    bullets: [
      'Het stuk herhaalt de wet niet. Wat er in een veld staat en waarom, zegt de wet die op die dag gold.',
      'Niets wordt overschreven. Een wijziging is een nieuw stuk, en het oude blijft staan.',
    ],
  },
  {
    kind: 'bullets',
    overline: 'Waarom',
    title: 'Wetten vragen naar het verleden',
    lead: 'Kreeg de aanvrager eerder een kaart? Geldt dat besluit nog, na bezwaar en beroep?',
    bullets: [
      'Dat antwoord ligt nu verspreid over systemen die elk hun eigen toestand bijhouden. Wie alleen de laatste stand bewaart, kan later niet meer zeggen waarom.',
      'Een besluit moet jaren later nog uit te leggen zijn, met de wet en de gegevens van toen. Ook als een bezwaar of een uitspraak achteraf veranderde wat geldt.',
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
    lead: 'Een gedateerd stuk in de kroniek van wie het rechtsfeit vaststelt.',
    bullets: [
      'Een indiening (aanvraag, bezwaar of beroep), een besluit, of een **executogram**: een ander feit dat een artikel vereist, zoals de ontvangst van een afschrift.',
      'Het stuk noemt het artikel waarop het berust, de dag, de velden en de uitvoer.',
    ],
    figure: { code: GRAM, label: 'Een besluit in de kroniek van Wilgendaal, ingekort' },
  },
  {
    kind: 'bullets',
    overline: 'Begrip',
    title: 'Kroniek',
    lead: 'De reeks chronolexogrammen van één cel, op datum. Er komt alleen iets bij: een herroepen besluit blijft staan, en wat er nog geldt, lees je af aan wat erna kwam.',
    bullets: ['Een stap op een eerdere dag dan de vorige weigert de cel, want de kroniek heeft één klok.'],
    figure: { diagram: 'kroniek', label: 'De kroniek van de gemeente Wilgendaal na twee aanvragen' },
  },
  {
    kind: 'bullets',
    overline: 'Begrip',
    title: 'Cel',
    lead: 'Een organisatie met een eigen kroniek, zoals een gemeente, de GGD of een rechtbank.',
    bullets: [
      'Ze legt vast en beslist via de engine. De code van de cel noemt geen artikel: alles wat zij controleert, komt uit de wet.',
      'In RegelRecht is een cel een map met `cel.yaml` en haar reducties. In `cel.yaml` staat wie zij is en welke antwoorden zij geeft.',
    ],
    figure: { code: CEL, label: 'De cel van de gemeente Wilgendaal' },
  },
  {
    kind: 'bullets',
    overline: 'Begrip',
    title: 'Reductie',
    lead: 'Een benoemd antwoord uit de eigen kroniek, geschreven als regelwerk.',
    bullets: [
      'Alleen een reductie leest een kroniek: de besluiten onder een artikel, of de feiten die de cel vastlegde. Ze staat in de map van haar cel; waar ze staat, zegt van wie ze is.',
      'De wet vraagt de cel niets. Het uitvoeringsbeleid van wie beslist, koppelt een open begrip aan het antwoord van de cel.',
    ],
    figure: { code: BINDING, label: 'Het beleid van Wilgendaal koppelt een open begrip aan een antwoord' },
  },
  {
    kind: 'bullets',
    overline: 'Begrip',
    title: 'Synthese',
    lead: 'Eén open begrip, met antwoorden van meer cellen.',
    bullets: [
      'Het begrip zegt wie bevoegd is en hoe de antwoorden één waarde worden (`combine`). De engine vraagt elke bevoegde cel van de sfeer en combineert in het proces dat vraagt; een cel reduceert alleen haar eigen kroniek.',
    ],
    figure: { diagram: 'synthese', label: 'Drie cellen beantwoorden één open begrip' },
  },
  {
    kind: 'bullets',
    overline: 'Wat volgt',
    title: 'Uitleg zonder opgeslagen redenen',
    lead: 'De cel bewaart geen redenen. Ze neemt het besluit opnieuw, met de wet en de kroniek van die dag.',
    bullets: [
      'Komt er een andere uitkomst uit, dan faalt de uitleg. Zo blijkt het als de kroniek of de wet niet meer klopt met wat er besloten is.',
      'Een antwoord uit een reductie is in dat spoor één stap. Wie wil weten hoe de cel eraan kwam, laat haar de reductie opnieuw draaien.',
    ],
  },
  {
    kind: 'bullets',
    overline: 'Wat volgt',
    title: 'Afschriften tussen cellen',
    lead: 'Een cel leest nooit de kroniek van een ander. De wet stuurt stukken.',
    bullets: [
      'Bij het beroepschrift gaat een afschrift van het bestreden besluit mee (Awb 6:5, tweede lid).',
      'De rechtbank zendt partijen een afschrift van de uitspraak (Awb 8:79, eerste lid). De gemeente legt de ontvangst vast in haar eigen kroniek.',
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
    lead: 'Artikel 49 van het BABW zegt dat het college een kaart kan verstrekken. Hoe het die bevoegdheid gebruikt, laat het open: `bevoegdheid_tot_verstrekken_gebruikt`.',
    bullets: [
      'Het beleid van Wilgendaal vult dat in (artikel 1): geen kaart aan wie op de dag van het besluit al een geldige kaart van het college heeft. Dat is een nieuw open begrip, `aanvrager_heeft_geldige_gehandicaptenparkeerkaart`.',
      'Artikel 2 van het beleid koppelt dat begrip aan het antwoord van de eigen cel.',
    ],
  },
  {
    kind: 'bullets',
    overline: 'De keten',
    title: 'Van de wet naar de kroniek',
    lead: 'De engine voert artikel 49 uit en volgt de open begrippen tot het antwoord dat de cel uit haar kroniek haalt.',
    bullets: [],
    figure: { diagram: 'keten', label: 'De keten van artikel 49 tot de kroniek van Wilgendaal' },
  },
  {
    kind: 'bullets',
    overline: 'De Awb-reductie',
    title: 'Welke besluiten gelden nog?',
    lead: '`reductie_algemene_wet_bestuursrecht` krijgt een artikel mee en geeft van alle besluiten daaronder de `geldende_besluiten`.',
    bullets: [
      'Een besluit dat bij bezwaar is herroepen (Awb 7:11, tweede lid), telt niet meer.',
      'Vernietigde de bestuursrechter een besluit met zijn rechtsgevolgen (Awb 8:72), dan telt het ook niet. Wat een vernietigde beslissing op bezwaar in de plaats nam, vervalt mee.',
      'Een besluit dat de beslissing op bezwaar of de rechter in de plaats stelde, telt wel.',
      'Deze reductie hoort bij geen cel en heeft geen eigen kroniek. Ze staat naast de cellen en leest de kroniek van de cel die haar aanroept.',
    ],
  },
  {
    kind: 'bullets',
    overline: 'De gemeente',
    title: 'Wat de gemeente zelf doet',
    lead: 'Eén FOREACH over `geldende_besluiten`. Van bezwaar of beroep weet de reductie van de gemeente niets.',
    bullets: [
      'Een besluit telt als het de kaart verstrekte aan dezelfde aanvrager, en de kaart op de dag van het nieuwe besluit nog geldig is.',
    ],
    figure: { code: OWN, label: 'Uit de reductie van de gemeente, ingekort' },
  },
  {
    kind: 'bullets',
    overline: 'Synthese',
    title: 'Eerder een kaart gekregen?',
    lead: 'Wie eerder een kaart kreeg, hoeft soms niet opnieuw gekeurd te worden (Regeling gehandicaptenparkeerkaart, artikel 2, tweede lid). Wilgendaal kent alleen haar eigen kaarten.',
    bullets: [
      'Bevoegd voor `aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt` is elke verstrekkende instantie: het college van een gemeente, en het CBR.',
      'Wilgendaal vraagt het zichzelf, de gemeente Oostmeer en het CBR. Elk antwoordt uit de eigen kroniek, en `combine: OR` maakt er één waarde van.',
    ],
    figure: { diagram: 'synthese-kaart', label: 'Drie verstrekkende instanties beantwoorden één begrip' },
  },
  {
    kind: 'closing',
    overline: 'Nu live',
    title: 'Speel het na',
    lines: ["Drie scenario's van de gehandicaptenparkeerkaart, elk met een gids naast de pagina."],
  },
];
