/**
 * The explanation page (UitlegPagina.vue), as data: one application for a
 * parking card for the disabled at the fictional municipality of Wilgendaal,
 * from the law to the chronicle, step by step. Each step has a short heading,
 * two or three sentences, and one or more panels of YAML.
 *
 * A panel quotes the corpus: its lines are lines of the file it names under
 * `source`, in order, without comments; `# …` marks lines left out.
 * tests/uitleg.test.js checks each one there, so a change in the corpus that
 * this page no longer matches makes the test fail. In a sentence, `name` is
 * code, as in the deck (slides.js).
 */

const BABW = 'corpus/regulation/nl/amvb/besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw/2026-01-01.yaml';
const BELEID =
  'corpus/regulation/nl/uitvoeringsbeleid/fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal/2026-01-01.yaml';
const CEL = 'corpus/cells/gemeente_wilgendaal/cel.yaml';
const REDUCTIE =
  'corpus/cells/gemeente_wilgendaal/reductions/reductie_geldige_gehandicaptenparkeerkaart/2026-01-01.yaml';
const AWB = 'corpus/cells/reductions/reductie_algemene_wet_bestuursrecht/2026-01-01.yaml';
const GGD_CEL = 'corpus/cells/ggd_wilgendaal/cel.yaml';
const GGD_REDUCTIE = 'corpus/cells/ggd_wilgendaal/reductions/reductie_geneeskundig_advies/2026-01-01.yaml';

export const intro = {
  overline: 'Uitleg',
  title: 'Hoe het in elkaar zit',
  lead:
    'Eén aanvraag voor een gehandicaptenparkeerkaart bij de fictieve gemeente Wilgendaal, van de wet tot de kroniek. ' +
    'Bij elke stap staat de YAML die de schakel legt, letterlijk uit het corpus.',
};

export const steps = [
  {
    overline: 'Wet',
    title: 'BABW art. 49',
    text: [
      'Artikel 49 van het BABW regelt de gehandicaptenparkeerkaart. Het zegt wat een aanvraag bevat en welke uitkomst het besluit vastlegt.',
      'Het college verstrekt de kaart alleen als het zijn bevoegdheid gebruikt. Wanneer het dat doet, zegt de wet niet: dat is het open begrip `bevoegdheid_tot_verstrekken_gebruikt`.',
    ],
    panels: [
      {
        label: 'De aanvraag en het besluit',
        source: BABW,
        code: `produces:
  - type: AANVRAAG
    requests: besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49
    parameters:
      - bsn_aanvrager
      - kaart_als_bestuurder
      - kaart_als_passagier
  - type: BESCHIKKING
    decision_type: TOEKENNING
    outputs:
      - gehandicaptenparkeerkaart_verstrekt
      - kaart_vermeldt_bestuurder
      - kaart_vermeldt_passagier
    # …`,
      },
      {
        label: 'De uitkomst hangt af van het open begrip',
        source: BABW,
        code: `- output: gehandicaptenparkeerkaart_verstrekt
  value:
    operation: AND
    conditions:
      # …
      - $bevoegdheid_tot_verstrekken_gebruikt
      - $gehandicaptenparkeerkaart_mag_worden_afgegeven`,
      },
    ],
  },
  {
    overline: 'Beleid',
    title: 'Beleid vult de bevoegdheid',
    text: [
      'Het fictieve beleid van Wilgendaal vult dat open begrip in artikel 1. Het college gebruikt zijn bevoegdheid niet als de aanvrager al een geldige kaart van het college heeft.',
      'Of dat zo is, laat ook dit artikel open, als `aanvrager_heeft_geldige_gehandicaptenparkeerkaart`.',
    ],
    panels: [
      {
        label: 'Beleid artikel 1: welk begrip het vult, en wat het openlaat',
        source: BELEID,
        code: `open_terms:
  - id: aanvrager_heeft_geldige_gehandicaptenparkeerkaart
    # …
implements:
  - law: besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw
    article: '49'
    open_term: bevoegdheid_tot_verstrekken_gebruikt`,
      },
      {
        label: 'Geen kaart als er al een geldige is',
        source: BELEID,
        code: `- output: bevoegdheid_tot_verstrekken_gebruikt
  value:
    operation: NOT
    value: $aanvrager_heeft_geldige_gehandicaptenparkeerkaart`,
      },
    ],
  },
  {
    overline: 'Beleid',
    title: 'Beleid vraagt de cel',
    text: [
      'Artikel 2 van het beleid vult dat begrip met een antwoord van de cel van de gemeente. Het vraagt dat antwoord met `source.reduction`.',
      'Geeft de cel geen antwoord, dan stelt het college het zelf vast.',
    ],
    panels: [
      {
        label: 'Beleid artikel 2: welk begrip het vult',
        source: BELEID,
        code: `implements:
  - law: fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal
    article: '1'
    open_term: aanvrager_heeft_geldige_gehandicaptenparkeerkaart`,
      },
      {
        label: 'Het antwoord van de cel als invoer',
        source: BELEID,
        code: `- name: antwoord_geldige_kaart
  type: boolean
  nullable: true
  source:
    reduction: aanvrager_heeft_geldige_gehandicaptenparkeerkaart
    parameters:
      bsn_aanvrager: $bsn_aanvrager
      besluit_datum: $besluit_datum`,
      },
      {
        label: 'Zonder antwoord beslist het college',
        source: BELEID,
        code: `- output: aanvrager_heeft_geldige_gehandicaptenparkeerkaart
  value:
    operation: IF
    cases:
      - when:
          operation: EQUALS
          subject: $antwoord_geldige_kaart
          value: null
        then: $aanvrager_heeft_geldige_gehandicaptenparkeerkaart_volgens_college
    default: $antwoord_geldige_kaart`,
      },
    ],
  },
  {
    overline: 'Cel',
    title: 'Cel geeft het antwoord',
    text: [
      'De cel is het systeem van de gemeente, met haar eigen kroniek van aanvragen en besluiten.',
      'In haar `cel.yaml` staat welke reductie dit antwoord geeft.',
    ],
    panels: [
      {
        label: 'cel.yaml van de gemeente Wilgendaal',
        source: CEL,
        code: `answers:
  aanvrager_heeft_geldige_gehandicaptenparkeerkaart: reductie_geldige_gehandicaptenparkeerkaart
  aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt: reductie_geldige_gehandicaptenparkeerkaart
  # …`,
      },
    ],
  },
  {
    overline: 'Reductie',
    title: 'Reductie zoekt geldige kaart',
    text: [
      'De reductie leest de kroniek van de gemeente terug. Ze vraagt eerst de reductie van de Awb welke besluiten onder BABW art. 49 nog gelden, en geeft daarvoor `basis` mee.',
      'Onder die besluiten zoekt ze een kaart voor deze aanvrager die op de dag van het besluit nog geldig is.',
    ],
    panels: [
      {
        label: 'De geldende besluiten onder BABW art. 49',
        source: REDUCTIE,
        code: `- name: geldende_besluiten
  type: array
  source:
    regulation: reductie_algemene_wet_bestuursrecht
    output: geldende_besluiten
    parameters:
      basis: besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw#49`,
      },
      {
        label: 'Een verstrekte kaart voor deze aanvrager',
        source: REDUCTIE,
        code: `operation: FOREACH
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
combine: OR`,
      },
    ],
  },
  {
    overline: 'Reductie zonder cel',
    title: 'Awb-reductie: wat nog geldt',
    text: [
      'Deze reductie hoort bij geen cel en werkt voor elk artikel. Ze leest de besluiten onder het artikel dat `basis` noemt, en de beslissingen op bezwaar.',
      'Een besluit dat bij bezwaar is herroepen (Awb 7:11) of door de rechter vernietigd (Awb 8:72), telt niet mee; een besluit dat ervoor in de plaats kwam wel.',
    ],
    panels: [
      {
        label: 'Uit de kroniek: de besluiten onder basis',
        source: AWB,
        code: `- name: besluiten
  type: array
  source:
    decisions: $basis
- name: beslissingen_op_bezwaar
  type: array
  source:
    decisions: algemene_wet_bestuursrecht#7:11`,
      },
      {
        label: 'Wat geldt',
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
    title: 'Advies van de GGD',
    text: [
      'Of de aanvrager voor een kaart in aanmerking komt, hangt af van een geneeskundig advies. Artikel 7 van het beleid neemt dat advies als antwoord van de cel van de GGD.',
      'Dat antwoord komt uit een reductie van de GGD, die de vastgelegde adviezen uit haar eigen kroniek leest.',
    ],
    panels: [
      {
        label: 'Beleid artikel 7 vraagt de GGD',
        source: BELEID,
        code: `- name: antwoord_langdurige_loopbeperking
  type: boolean
  nullable: true
  source:
    reduction: advies_langdurige_loopbeperking
    parameters:
      bsn_aanvrager: $bsn_aanvrager
      dagtekening_aanvraag: $dagtekening_aanvraag
      datum_afgifte: $datum_afgifte`,
      },
      {
        label: 'cel.yaml van de GGD Wilgendaal',
        source: GGD_CEL,
        code: `answers:
  # …
  advies_langdurige_loopbeperking: reductie_geneeskundig_advies
  # …`,
      },
      {
        label: 'De reductie leest de adviezen',
        source: GGD_REDUCTIE,
        code: `- name: adviezen
  type: array
  source:
    facts: fictief_beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#5#geneeskundig_advies_uitgebracht`,
      },
    ],
  },
];
