/**
 * The explanation page (UitlegPagina.vue), as data: one application for a
 * business parking permit at the fictional municipality of Wilgendaal, from
 * the rule to the chronicle, step by step, written for a reader who does not
 * know the law format. Each step has a short heading, two or three
 * sentences, and one or more panels of YAML.
 *
 * A step that rests on the text of an article starts with that text
 * (`quote`): from `text` of the article `artikel` in `source`, a paragraph
 * or a list of them (one per lid), with `…` where words are left out. That
 * the regulation is invented, the strip of the portal above every page
 * says, so the page does not say it again.
 *
 * A panel quotes the corpus: its lines are lines of the file it names under
 * `source`, in order, without comments; `# …` marks lines left out.
 * tests/uitleg.test.js checks the panels and the quotes there, so a change in
 * the corpus that this page no longer matches makes the test fail. In a
 * sentence, `name` is code, as in the deck (slides.js).
 */

const VERORDENING =
  'corpus/regulation/nl/gemeentelijke_verordening/wilgendaal/fictieve_parkeerverordening_gemeente_wilgendaal/2026-01-01.yaml';
const BESLUIT =
  'corpus/regulation/nl/uitvoeringsbeleid/fictief_uitvoeringsbesluit_parkeren_gemeente_wilgendaal/2026-01-01.yaml';
const CEL = 'corpus/cells/gemeente_wilgendaal/cel.yaml';
const REDUCTIE =
  'corpus/cells/gemeente_wilgendaal/reductions/reductie_geldige_bedrijfsparkeervergunningen/2026-01-01.yaml';
const AWB = 'corpus/cells/reductions/reductie_algemene_wet_bestuursrecht/2026-01-01.yaml';
const KVK_CEL = 'corpus/cells/kamer_van_koophandel/cel.yaml';
const KVK_REDUCTIE =
  'corpus/cells/kamer_van_koophandel/reductions/reductie_vestiging_in_handelsregister/2026-01-01.yaml';

/** The case the page follows, which the button at the bottom opens. */
export const casus = 'bedrijfsparkeervergunning';

export const intro = {
  overline: 'Uitleg',
  title: 'Hoe het in elkaar zit',
  lead:
    'Een bedrijf vraagt de gemeente Wilgendaal om een parkeervergunning. Deze pagina volgt die aanvraag van de regel tot de kroniek, het overzicht waarin de gemeente al haar besluiten bewaart. ' +
    'Waar een stap op een artikel rust, staat eerst de tekst van dat artikel. Daarna volgt de YAML die het uitvoerbaar maakt, letterlijk uit de bestanden.',
};

export const steps = [
  {
    overline: 'Verordening',
    title: 'Recht op een vergunning',
    quote: {
      source: VERORDENING,
      artikel: '2',
      label: 'Parkeerverordening Wilgendaal, artikel 2, eerste en tweede lid',
      tekst: [
        '1 Het college verleent op aanvraag een bedrijfsparkeervergunning aan een bedrijf dat in een vergunninggebied is gevestigd.',
        '2 Het college weigert de bedrijfsparkeervergunning als het bedrijf op de dag van het besluit al het maximum aantal geldige bedrijfsparkeervergunningen heeft.',
      ],
    },
    text: [
      'Een bedrijf krijgt de vergunning als het in een vergunninggebied zit en nog niet het maximum heeft. Wat een vergunninggebied is en hoe hoog het maximum ligt, zijn open begrippen die een andere regel invult.',
      'Met de hardheidsclausule, een uitzonderingsregel in artikel 6, kan het college het maximum loslaten, zoals scenario 4 laat zien.',
    ],
    panels: [
      {
        label: 'Wat het bedrijf vraagt, en wat het besluit vastlegt',
        source: VERORDENING,
        code: `produces:
  - type: AANVRAAG
    requests: fictieve_parkeerverordening_gemeente_wilgendaal#2
    parameters:
      - kvk_nummer
      - kenteken
  - type: BESCHIKKING
    decision_type: TOEKENNING
    outputs:
      - bedrijfsparkeervergunning_verleend
    # …`,
      },
      {
        label: 'De uitkomst: krijgt het bedrijf de vergunning?',
        source: VERORDENING,
        code: `- output: bedrijfsparkeervergunning_verleend
  value:
    operation: AND
    conditions:
      - $bedrijf_gevestigd_in_vergunninggebied
      - operation: OR
        conditions:
          - operation: NOT
            value: $maximum_bedrijfsparkeervergunningen_bereikt
          - $weigeringsgrond_buiten_toepassing_gelaten`,
      },
    ],
  },
  {
    overline: 'Uitvoeringsbesluit',
    title: 'Het maximum per bedrijf',
    quote: {
      source: BESLUIT,
      artikel: '3',
      label: 'Uitvoeringsbesluit parkeren Wilgendaal, artikel 3',
      tekst: [
        '1 Een bedrijf dat in de binnenstad is gevestigd, krijgt ten hoogste twee bedrijfsparkeervergunningen.',
        '2 Een bedrijf dat in de schil is gevestigd, krijgt er ten hoogste zoveel als het aantal personen dat volgens het handelsregister in het bedrijf werkzaam is.',
        '3 Het maximum is bereikt als het bedrijf op de dag van het besluit al zoveel geldige bedrijfsparkeervergunningen heeft.',
      ],
    },
    text: [
      'Dit artikel vult het maximum uit de verordening in: twee vergunningen in de binnenstad, en in de schil, de wijken rond de binnenstad, zoveel als er mensen in het bedrijf werken.',
      'Hoeveel geldige vergunningen het bedrijf al heeft, is een nieuw open begrip: `aantal_geldige_bedrijfsparkeervergunningen`.',
    ],
    panels: [
      {
        label: 'Artikel 3 laat een nieuw begrip open en vult het maximum',
        source: BESLUIT,
        code: `open_terms:
  # …
  - id: aantal_geldige_bedrijfsparkeervergunningen
    type: number
    required: true
    # …
implements:
  - law: fictieve_parkeerverordening_gemeente_wilgendaal
    article: '2'
    open_term: maximum_bedrijfsparkeervergunningen_bereikt`,
      },
      {
        label: 'Het maximum: twee in de binnenstad',
        source: BESLUIT,
        code: `- output: maximum_bedrijfsparkeervergunningen
  value:
    operation: IF
    cases:
      - when:
          operation: EQUALS
          subject: $vergunninggebied
          value: binnenstad
        then: 2
      # …
    default: 0`,
      },
      {
        label: 'Bereikt als het bedrijf er al zoveel heeft',
        source: BESLUIT,
        code: `- output: maximum_bedrijfsparkeervergunningen_bereikt
  value:
    operation: GREATER_THAN_OR_EQUAL
    subject: $aantal_geldige_bedrijfsparkeervergunningen
    value: $maximum_bedrijfsparkeervergunningen`,
      },
    ],
  },
  {
    overline: 'Uitvoeringsbesluit',
    title: 'Gemeente vraagt de cel',
    quote: {
      source: BESLUIT,
      artikel: '5',
      label: 'Uitvoeringsbesluit parkeren Wilgendaal, artikel 5',
      tekst:
        'Hoeveel geldige bedrijfsparkeervergunningen het bedrijf op de dag van het besluit heeft (artikel 3), ontleent het college aan het antwoord van zijn eigen cel over zijn eerdere besluiten. Geeft de cel geen antwoord, dan stelt het college het zelf vast.',
    },
    text: [
      'Het aantal vraagt de gemeente aan haar cel, het systeem dat haar kroniek bijhoudt.',
      'In de YAML is dat `source.reduction`. De cel rekent het antwoord uit met een reductie, een rekenregel die de kroniek terugleest.',
    ],
    panels: [
      {
        label: 'Artikel 5 vult het aantal uit artikel 3',
        source: BESLUIT,
        code: `implements:
  - law: fictief_uitvoeringsbesluit_parkeren_gemeente_wilgendaal
    article: '3'
    open_term: aantal_geldige_bedrijfsparkeervergunningen`,
      },
      {
        label: 'De vraag aan de cel',
        source: BESLUIT,
        code: `- name: antwoord_aantal_geldig
  type: number
  nullable: true
  source:
    reduction: aantal_geldige_bedrijfsparkeervergunningen
    parameters:
      kvk_nummer: $kvk_nummer
      besluit_datum: $besluit_datum`,
      },
      {
        label: 'Zonder antwoord van de cel beslist het college',
        source: BESLUIT,
        code: `- output: aantal_geldige_bedrijfsparkeervergunningen
  value:
    operation: IF
    cases:
      - when:
          operation: EQUALS
          subject: $antwoord_aantal_geldig
          value: null
        then: $aantal_geldige_bedrijfsparkeervergunningen_volgens_college
    default: $antwoord_aantal_geldig`,
      },
    ],
  },
  {
    overline: 'Cel',
    title: 'Cel geeft het antwoord',
    text: [
      'Elke cel weet welke vragen zij kan beantwoorden.',
      'In haar `cel.yaml` staat per vraag welke reductie het antwoord uitrekent.',
    ],
    panels: [
      {
        label: 'cel.yaml van de gemeente Wilgendaal',
        source: CEL,
        code: `answers:
  # …
  aantal_geldige_bedrijfsparkeervergunningen: reductie_geldige_bedrijfsparkeervergunningen`,
      },
    ],
  },
  {
    overline: 'Reductie',
    title: 'Reductie telt de vergunningen',
    text: [
      'Eerst vraagt de reductie aan de Awb-reductie uit stap 6 welke besluiten onder artikel 2 van de verordening nog gelden, met dat artikel in `basis`.',
      'Van elk besluit leest ze `bedrijfsparkeervergunning_verleend`, de uitkomst van toen. Zo telt ze de vergunningen van dit bedrijf die op de dag van het besluit nog gelden.',
    ],
    panels: [
      {
        label: 'De besluiten die nog gelden onder verordening artikel 2',
        source: REDUCTIE,
        code: `- name: geldende_besluiten
  type: array
  source:
    regulation: reductie_algemene_wet_bestuursrecht
    output: geldende_besluiten
    parameters:
      basis: fictieve_parkeerverordening_gemeente_wilgendaal#2`,
      },
      {
        label: 'Tel de eerdere besluiten die dit bedrijf een vergunning gaven',
        source: REDUCTIE,
        code: `operation: FOREACH
collection: $geldende_besluiten
as: besluit
filter:
  operation: AND
  conditions:
    - operation: EQUALS
      subject: $besluit.bedrijfsparkeervergunning_verleend
      value: true
    - operation: EQUALS
      subject: $besluit.kvk_nummer
      value: $kvk_nummer
    # …
body: 1
combine: ADD`,
      },
    ],
  },
  {
    overline: 'Reductie zonder cel',
    title: 'Awb-reductie: wat nog geldt',
    text: [
      'De Algemene wet bestuursrecht (Awb) heeft regels voor elk besluit van de overheid, en deze reductie past die toe op de kroniek van de cel die haar aanroept.',
      'Een besluit telt niet meer mee als de gemeente het na bezwaar zelf terugdraaide (herroepen, Awb 7:11) of als de rechter het vernietigde (Awb 8:72). Een besluit dat ervoor in de plaats kwam, telt wel.',
    ],
    panels: [
      {
        label: 'Uit de kroniek van de cel die haar aanroept: besluiten, bezwaar en uitspraken',
        source: AWB,
        code: `- name: besluiten
  type: array
  source:
    decisions: $basis
- name: beslissingen_op_bezwaar
  type: array
  source:
    decisions: algemene_wet_bestuursrecht#7:11
- name: uitspraken
  type: array
  source:
    facts: algemene_wet_bestuursrecht#8:79#afschrift_van_de_uitspraak`,
      },
      {
        label: 'Wat geldt: de besluiten die bleven staan, en die de rechter ervoor in de plaats stelde',
        source: AWB,
        code: `- output: geldende_besluiten
  value:
    operation: ADD
    values:
      - $besluiten_in_stand
      - $besluiten_in_de_plaats`,
      },
    ],
  },
  {
    overline: 'Zijtak',
    title: 'Vestiging bij de KvK',
    quote: {
      source: BESLUIT,
      artikel: '2',
      label: 'Uitvoeringsbesluit parkeren Wilgendaal, artikel 2',
      tekst:
        'Uit het handelsregister van de Kamer van Koophandel moet blijken dat het bedrijf in een vergunninggebied is gevestigd. Het bedrijf is gevestigd in het vergunninggebied waarin het bezoekadres van zijn vestiging ligt, zoals het handelsregister dat op de dag van het besluit vermeldt.',
    },
    text: [
      'Of het bedrijf in een vergunninggebied zit, blijkt uit het handelsregister. Daarom vraagt artikel 4 de postcode van de vestiging en het aantal mensen dat er werkt aan de cel van de Kamer van Koophandel.',
      'Die cel leest haar eigen inschrijvingen terug.',
    ],
    panels: [
      {
        label: 'Uitvoeringsbesluit artikel 4: de vraag aan de KvK',
        source: BESLUIT,
        code: `- name: antwoord_postcode
  type: string
  nullable: true
  source:
    reduction: postcode_vestiging
    parameters:
      kvk_nummer: $kvk_nummer
      peildatum: $besluit_datum`,
      },
      {
        label: 'cel.yaml van de Kamer van Koophandel',
        source: KVK_CEL,
        code: `answers:
  postcode_vestiging: reductie_vestiging_in_handelsregister
  # …`,
      },
      {
        label: 'De reductie leest de inschrijvingen',
        source: KVK_REDUCTIE,
        code: `- name: inschrijvingen
  type: array
  source:
    facts: fictief_beleid_handelsregister_kamer_van_koophandel#1#vestiging_ingeschreven`,
      },
    ],
  },
];
