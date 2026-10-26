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
 * Text between „ and ” is quoted word for word from the position paper
 * (https://chronolexografie.nl/position-paper/); a slide that quotes says so
 * in `cite`. Everything else is RegelRecht's own explanation or choice, and
 * where that could be mistaken for the paper it starts with "In RegelRecht".
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
    lead: '„Elke actor houdt eigen feiten bij in een eigen cel, wat zorgt voor unieke, canonieke en contextuele vastlegging.”',
    bullets: [
      '„Feiten worden nooit verwijderd, ook niet bij correcties. Dit maakt “tijdreizen” mogelijk: volledige traceerbaarheid van hoe elke rechtstoestand is ontstaan.”',
      'In RegelRecht herhaalt een vastgelegd stuk de wet niet. Wat er in een veld staat en waarom, zegt de wet die op die dag gold.',
    ],
    cite: 'Tussen „ en ”: de position paper Chronolexografie (chronolexografie.nl).',
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
    lead: '„Een chronolexogram is simpelweg de benaming voor een elementair en op zichzelf consistent ‘element van vastlegging’.” „Elk chronolexogram wordt in een specifiek domein gecreëerd (tot feit gemaakt) en daar ook opgeslagen.”',
    bullets: [
      'In RegelRecht is een chronolexogram een indiening (aanvraag, bezwaar of beroep), een besluit, of een **executogram**. Een executogram is hier elk ander feit dat een artikel vereist, zoals de ontvangst van een afschrift.',
      'Het noemt het artikel waarop het berust, de dag, de velden en de uitvoer.',
    ],
    cite: 'Tussen „ en ”: de position paper Chronolexografie (chronolexografie.nl).',
    figure: { code: GRAM, label: 'Een besluit in de kroniek van Wilgendaal, ingekort' },
  },
  {
    kind: 'bullets',
    overline: 'Begrip',
    title: 'Kroniek',
    lead: '„Elke chronolexokroniek heeft een eigen tijdsas. Dat wil zeggen: chronolexogrammen binnen een chronolexokroniek kennen een gedefinieerde ordening, maar die ordening bestaat niet over kronieken heen.”',
    bullets: [
      'In RegelRecht heeft een cel één kroniek, en er komt alleen iets bij: een herroepen besluit blijft staan, en wat er nog geldt, lees je af aan wat erna kwam. Een stap op een eerdere dag dan de vorige weigert de cel.',
    ],
    cite: 'Tussen „ en ”: de position paper Chronolexografie (chronolexografie.nl).',
    figure: { diagram: 'kroniek', label: 'De kroniek van de gemeente Wilgendaal na twee aanvragen' },
  },
  {
    kind: 'bullets',
    overline: 'Begrip',
    title: 'Cel',
    lead: 'Een cel is „een ruimte waarin chronolexogrammen worden gemaakt, bewaard en verwerkt. Op geen enkele wijze is bewerking hiervan buiten de cel mogelijk.”',
    cite: 'Tussen „ en ”: de position paper Chronolexografie (chronolexografie.nl).',
    bullets: [
      'In RegelRecht heeft elke organisatie een eigen cel, zoals een gemeente, de GGD of een rechtbank. De cel legt vast en beslist via de engine. De code van de cel noemt geen artikel: alles wat zij controleert, komt uit de wet.',
      "Zo'n cel is een map met `cel.yaml` en haar reducties. In `cel.yaml` staat wie zij is en welke antwoorden zij geeft.",
    ],
    figure: { code: CEL, source: 'corpus/cells/gemeente_wilgendaal/cel.yaml', label: 'De cel van de gemeente Wilgendaal' },
  },
  {
    kind: 'bullets',
    overline: 'Begrip',
    title: 'Reductie',
    lead: '„de (inherent destructieve) activiteit van het hergebruik van feiten op een later moment in de tijd.” „Reductie vindt altijd plaats ín de cel waar de betreffende chronolexogrammen zijn vastgelegd”.',
    cite: 'Tussen „ en ”: de position paper Chronolexografie (chronolexografie.nl).',
    bullets: [
      'In RegelRecht is een reductie een regelwerk dat een benoemd antwoord geeft. Alleen een reductie leest een kroniek: de besluiten onder een artikel, of de feiten die de cel vastlegde. Een reductie staat in de map van haar cel, op één na: die van de Awb hoort bij geen cel.',
      'De wet vraagt de cel niets. Het uitvoeringsbeleid van wie beslist, koppelt een open begrip aan het antwoord van de cel.',
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
    lead: '„het betrekken van meerdere lexostatussen uit meerdere cellen en het combineren, vergelijken en beoordelen daarvan.” „Synthese gebeurt bij de afnemer, niet bij de bron”.',
    cite: 'Tussen „ en ”: de position paper Chronolexografie (chronolexografie.nl).',
    bullets: [
      'In RegelRecht gaat een synthese over één open begrip. Het begrip zegt wie bevoegd is en hoe de antwoorden één waarde worden (`combine`). De engine vraagt het aan elke bevoegde cel die bij het besluit hoort, en voegt de antwoorden samen. Elke cel kijkt alleen in haar eigen kroniek.',
    ],
    figure: { diagram: 'synthese', label: 'Drie cellen beantwoorden één open begrip' },
  },
  {
    kind: 'bullets',
    overline: 'Wat volgt',
    title: 'Uitleg zonder opgeslagen redenen',
    lead: 'In RegelRecht bewaart de cel geen redenen. Ze neemt het besluit opnieuw, met de wet en de kroniek van die dag.',
    bullets: [
      'Komt er een andere uitkomst uit, dan faalt de uitleg. Zo blijkt het als de kroniek of de wet niet meer klopt met wat er besloten is.',
      'In de uitleg van het besluit is een antwoord uit een reductie één stap. Wie wil weten hoe de cel eraan kwam, laat haar de reductie opnieuw draaien.',
    ],
  },
  {
    kind: 'bullets',
    overline: 'Wat volgt',
    title: 'Afschriften tussen cellen',
    lead: 'In RegelRecht leest een cel nooit de kroniek van een ander. Wat er tussen cellen gaat, is een afschrift dat de wet voorschrijft.',
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
      'Deze reductie hoort bij geen cel en heeft geen eigen kroniek. Ze staat naast de cellen en leest de kroniek van de cel die de aanroepende reductie draait.',
    ],
  },
  {
    kind: 'bullets',
    overline: 'De gemeente',
    title: 'Wat de gemeente zelf doet',
    lead: 'Een FOREACH over `geldende_besluiten`. Of een besluit herroepen of vernietigd is, ziet de reductie van de gemeente niet: dat zoekt de Awb-reductie uit.',
    bullets: [
      'Een besluit telt als het de kaart verstrekte aan dezelfde aanvrager, en de kaart op de dag van het nieuwe besluit nog geldig is.',
      'Verstrekte de rechter zelf een kaart, dan zwijgt de reductie. Hoe lang die kaart geldt, staat niet in de kroniek van de gemeente.',
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
    title: 'Eerder een kaart gekregen?',
    lead: 'Wie eerder een kaart kreeg, hoeft soms niet opnieuw gekeurd te worden (Regeling gehandicaptenparkeerkaart, artikel 2, tweede lid). Wilgendaal kent alleen haar eigen kaarten.',
    bullets: [
      'Bevoegd voor `aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt` is elke verstrekkende instantie: het college van een gemeente, en het CBR.',
      'Bij een besluit van Wilgendaal vraagt de engine het aan Wilgendaal zelf, de gemeente Oostmeer en het CBR. Elk antwoordt uit de eigen kroniek, en `combine: OR` maakt er één waarde van.',
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
