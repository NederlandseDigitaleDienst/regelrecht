/**
 * The explanation page (UitlegPagina.vue), as data: one application for a
 * business parking permit at the fictional municipality of Wilgendaal, from
 * the rule to the chronicle, step by step, written for a reader who does not
 * know the law format. Each step has a short heading, two or three
 * sentences, and one or more panels of YAML that fold.
 *
 * A step that rests on the text of an article starts with that text
 * (`quote`): from `text` of the article `artikel` in `source`, a paragraph
 * or a list of them (one per lid), with `…` where words are left out. That
 * the regulation is invented, the strip of the portal above every page
 * says, so the page does not say it again.
 *
 * A panel shows the whole file it names under `source`, as the demo shows a
 * law (YamlNode from frontend-shared): a tree from the root that folds, with
 * the nodes on the way to each path of `open` open and everything else
 * folded, so the reader can open the rest. A file can come back in another
 * step with other paths open. A path runs from the root of the file and is
 * labelled as the tree labels it (yamlExpand.js in frontend-shared): an item
 * of a list by its `output`, `name` or `number`, else by its index; `*`
 * stands for any one part. tests/uitleg.test.js checks
 * that every path is in the file, so a change in the corpus
 * that this page no longer matches makes the test fail. In a sentence,
 * `name` is code, as in the deck (slides.js).
 */

import { CORPUS } from '../corpus.js';

const VERORDENING =
  `${CORPUS}/regulation/nl/gemeentelijke_verordening/wilgendaal/parkeerverordening_gemeente_wilgendaal/2026-01-01.yaml`;
const BESLUIT =
  `${CORPUS}/regulation/nl/gemeentelijke_verordening/wilgendaal/uitvoeringsbesluit_parkeren_gemeente_wilgendaal/2026-01-01.yaml`;
const BELEID =
  `${CORPUS}/regulation/nl/uitvoeringsbeleid/beleid_bedrijfsparkeervergunning_gemeente_wilgendaal/2026-01-01.yaml`;
const CEL = `${CORPUS}/cells/gemeente_wilgendaal/cel.yaml`;
const REDUCTIE =
  `${CORPUS}/cells/gemeente_wilgendaal/reductions/reductie_geldige_bedrijfsparkeervergunningen/2026-01-01.yaml`;
const AWB = `${CORPUS}/cells/reductions/reductie_algemene_wet_bestuursrecht/2026-01-01.yaml`;
const KVK_CEL = `${CORPUS}/cells/kamer_van_koophandel/cel.yaml`;
const KVK_REDUCTIE =
  `${CORPUS}/cells/kamer_van_koophandel/reductions/reductie_vestiging_in_handelsregister/2026-01-01.yaml`;

/** The files the panels show, for the page that loads them (UitlegPagina.vue). */
export const bronnen = [VERORDENING, BESLUIT, BELEID, CEL, REDUCTIE, AWB, KVK_CEL, KVK_REDUCTIE];

/**
 * The paths of a panel that start open: its own, and the article each one
 * runs through. The tree opens an article only when a path names it as such
 * (yamlExpand.js in frontend-shared), as the demo opens one per click.
 */
export const openPaden = ({ open }) => {
  const artikelen = open.map((p) => p.match(/^articles\.[^.]+/)?.[0]).filter(Boolean);
  return [...new Set([...artikelen, ...open])];
};

/** The case the page follows, which the button at the bottom opens. */
export const casus = 'bedrijfsparkeervergunning';

export const intro = {
  overline: 'Uitleg',
  title: 'Hoe het in elkaar zit',
  lead:
    'Een bedrijf vraagt de gemeente Wilgendaal om een parkeervergunning. Deze pagina volgt die aanvraag van de regel tot de kroniek, het overzicht waarin de gemeente al haar besluiten bewaart. ' +
    'Waar een stap op een artikel rust, staat eerst de tekst van dat artikel. Daarna volgt de YAML die het uitvoerbaar maakt, uit de bestanden zelf. De delen waar het om gaat, staan open. De rest klap je zelf open.',
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
        label: 'Artikel 2: wat het bedrijf vraagt, wat het besluit vastlegt en wanneer het de vergunning krijgt',
        source: VERORDENING,
        open: [
          'articles.2.machine_readable.execution.produces.0.parameters',
          'articles.2.machine_readable.execution.produces.1.outputs',
          'articles.2.machine_readable.execution.actions.bedrijfsparkeervergunning_verleend.value.conditions.1.conditions.0',
        ],
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
        label: 'Artikel 3: het nieuwe begrip en de berekening van het maximum',
        source: BESLUIT,
        open: [
          'articles.3.machine_readable.open_terms.1',
          'articles.3.machine_readable.implements.0',
          'articles.3.machine_readable.execution.actions.maximum_bedrijfsparkeervergunningen.value.cases.*.when.conditions.*.value',
          'articles.3.machine_readable.execution.actions.maximum_bedrijfsparkeervergunningen_bereikt.value',
        ],
      },
    ],
  },
  {
    overline: 'Beleid',
    title: 'Gemeente vraagt de cel',
    quote: {
      source: BELEID,
      artikel: '2',
      label: 'Beleid bedrijfsparkeervergunning Wilgendaal, artikel 2',
      tekst:
        'Hoeveel geldige bedrijfsparkeervergunningen het bedrijf op de dag van het besluit heeft (artikel 3 van het Uitvoeringsbesluit Parkeren Wilgendaal 2026), ontleent het college aan het antwoord van zijn eigen cel over zijn eerdere besluiten. Geeft de cel geen antwoord, dan stelt het college het zelf vast.',
    },
    text: [
      'Het uitvoeringsbesluit zegt welke regels gelden, het beleid van het college hoe het ze uitvoert. Het aantal vraagt de gemeente aan haar cel, het systeem dat haar kroniek bijhoudt.',
      'In de YAML is dat `source.reduction`, waarmee de cel het antwoord uitrekent: een reductie, een rekenregel die de kroniek terugleest.',
    ],
    panels: [
      {
        label: 'Artikel 2 van het beleid: de vraag aan de cel, en het college als de cel niet antwoordt',
        source: BELEID,
        open: [
          'articles.2.machine_readable.implements.0',
          'articles.2.machine_readable.execution.input.antwoord_aantal_geldig.source.parameters',
          'articles.2.machine_readable.execution.actions.aantal_geldige_bedrijfsparkeervergunningen.value.cases.0.when',
        ],
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
        open: ['answers'],
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
        label: 'De reductie: de besluiten die nog gelden, en de telling van de vergunningen van dit bedrijf',
        source: REDUCTIE,
        open: [
          'articles.1.machine_readable.execution.input.geldende_besluiten.source.parameters',
          'articles.1.machine_readable.execution.actions.aantal_geldige_bedrijfsparkeervergunningen.value.filter.conditions.*.subject',
          'articles.1.machine_readable.execution.actions.aantal_geldige_bedrijfsparkeervergunningen.value.filter.conditions.*.value',
        ],
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
        label: 'Artikel 1 leest de besluiten, het bezwaar en de uitspraken uit de kroniek; artikel 2 zegt wat geldt',
        source: AWB,
        open: [
          'articles.1.machine_readable.execution.input.*.source',
          'articles.2.machine_readable.execution.actions.geldende_besluiten.value.values',
        ],
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
      'Of het bedrijf in een vergunninggebied zit, blijkt uit het handelsregister. Daarom vraagt artikel 1 van het beleid de postcode van de vestiging en het aantal mensen dat er werkt aan de cel van de Kamer van Koophandel.',
      'Die cel leest haar eigen inschrijvingen terug.',
    ],
    panels: [
      {
        label: 'Beleid artikel 1: de vraag aan de KvK',
        source: BELEID,
        open: ['articles.1.machine_readable.execution.input.antwoord_postcode.source.parameters'],
      },
      {
        label: 'cel.yaml van de Kamer van Koophandel',
        source: KVK_CEL,
        open: ['answers'],
      },
      {
        label: 'De reductie leest de inschrijvingen',
        source: KVK_REDUCTIE,
        open: ['articles.1.machine_readable.execution.input.inschrijvingen.source'],
      },
    ],
  },
];
