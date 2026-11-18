/**
 * Simple view: Dutch law-making process as GitFlow.
 *
 * Branch model:
 *   col 0 - main (Corpus Juris, the law in force)
 *   col 1 - develop (Wetgevingskalender, proposals in procedure)
 *   col 2 - fork: wetsvoorstel (ministry / fraction workspace)
 *   col 3 - topic branch within the wetsvoorstel fork
 *   col 4 - fork: advisory (Raad van State / assessing bodies)
 */

import type { Branch, Stage, Connection } from './types';

export const branches: Branch[] = [
  {
    id: 'main',
    label: 'Corpus Juris',
    gitLabel: 'main',
    color: 'var(--color-branch-main)',
    col: 0,
    startRow: 0,
    endRow: 30,
  },
  {
    id: 'develop',
    label: 'Wetgevingskalender',
    gitLabel: 'develop',
    color: 'var(--color-branch-develop)',
    col: 1,
    startRow: 2,
    endRow: 26,
  },
  {
    id: 'wetsvoorstel',
    label: 'Wetsvoorstel',
    gitLabel: 'fork/wetsvoorstel',
    color: 'var(--color-branch-wetsvoorstel)',
    col: 2,
    startRow: 4,
    endRow: 15,
  },
  {
    id: 'internal',
    label: 'Interne afstemming',
    gitLabel: 'topic/*',
    color: 'var(--color-branch-internal)',
    col: 3,
    startRow: 6,
    endRow: 10,
  },
  {
    id: 'advisory',
    label: 'Raad van State',
    gitLabel: 'fork/rvs-advies',
    color: 'var(--color-branch-advisory)',
    col: 4,
    startRow: 18,
    endRow: 22,
  },
];

export const stages: Stage[] = [
  // === MAIN: Corpus Juris ===
  {
    id: 'corpus-start',
    branch: 'main',
    type: 'commit',
    gitLabel: 'HEAD (main)',
    lawLabel: 'Corpus Juris',
    subtitle: 'Geldend recht',
    description:
      'Het geheel van alle geldende Nederlandse wetgeving - de "main branch". ' +
      'Wat hier staat, is geldend recht.',
    col: 0, step: 0,
  },
  {
    id: 'main-other',
    branch: 'main',
    type: 'commit',
    gitLabel: 'commit (andere wet)',
    lawLabel: 'Andere wetswijziging',
    subtitle: 'Main gaat door',
    description:
      'Terwijl het wetsvoorstel in procedure is, gaan andere wetten gewoon door. ' +
      'Main verandert voortdurend - daarom zijn rebases nodig.',
    col: 0, step: 5,
  },

  // === DEVELOP: Wetgevingskalender ===
  {
    id: 'develop-start',
    branch: 'develop',
    type: 'branch',
    gitLabel: 'git checkout -b develop',
    lawLabel: 'Wetgevingskalender',
    subtitle: 'Voorstellen in procedure',
    description:
      'De develop branch bevat alle wetsvoorstellen die officieel in procedure zijn, ' +
      'maar nog geen onderdeel van het Corpus Juris.',
    col: 1, step: 1,
  },

  // === WETSVOORSTEL: Ministry fork ===
  {
    id: 'voorstel-fork',
    branch: 'wetsvoorstel',
    type: 'branch',
    gitLabel: 'git clone (fork)',
    lawLabel: 'Wetsvoorstel ingediend',
    subtitle: 'Fork naar eigen omgeving',
    description:
      'Een ministerie forkt het Corpus Juris naar een eigen omgeving. ' +
      'Het team werkt onafhankelijk, net als een fork op een eigen Git-server.',
    col: 2, step: 2,
  },
  {
    id: 'voorstel-beleid',
    branch: 'wetsvoorstel',
    type: 'commit',
    gitLabel: 'commit (beleidsnota)',
    lawLabel: 'Beleidsnota',
    subtitle: 'Wat wil het ministerie bereiken?',
    description:
      'De beleidsmedewerker schrijft een beleidsnota: wat is het probleem, ' +
      'wat moet de wet bereiken? Dit is de "product spec" bij de code.',
    col: 2, step: 3,
  },

  // === INTERNAL: Sub-branch within the fork ===
  {
    id: 'intern-start',
    branch: 'internal',
    type: 'branch',
    gitLabel: 'git checkout -b concept',
    lawLabel: 'Concepttekst',
    subtitle: 'Juridisch team start',
    description:
      'Binnen de fork maakt het juridisch team een sub-branch aan. ' +
      'Wetgevingsjuristen vertalen het beleid naar juridische tekst.',
    col: 3, step: 4,
  },
  {
    id: 'intern-draft',
    branch: 'internal',
    type: 'commit',
    gitLabel: 'commit',
    lawLabel: 'Eerste concept',
    subtitle: 'Juridische tekst',
    description:
      'Het eerste concept van de wettekst. Er volgen nog vele iteraties - ' +
      'net als bij code wordt er voortdurend herzien en verbeterd.',
    col: 3, step: 6,
  },
  {
    id: 'intern-review',
    branch: 'internal',
    type: 'commit',
    gitLabel: 'commit (review)',
    lawLabel: 'Interne review',
    subtitle: 'Directie Wetgeving + JenV',
    description:
      'Interne review door de Directie Wetgeving en de wetgevingstoets van JenV. ' +
      'Vergelijkbaar met code review binnen het team.',
    col: 3, step: 7,
  },
  {
    id: 'intern-rebase',
    branch: 'internal',
    type: 'commit',
    gitLabel: 'git rebase main',
    lawLabel: 'Rebase van main',
    subtitle: 'Bijwerken naar actueel recht',
    description:
      'Het Corpus Juris verandert voortdurend. Het voorstel moet ' +
      'regelmatig gerebased worden zodat het consistent blijft met geldend recht.',
    col: 3, step: 8,
  },
  {
    id: 'intern-final',
    branch: 'internal',
    type: 'commit',
    gitLabel: 'commit (final)',
    lawLabel: 'Akkoord DG/SG',
    subtitle: 'Ambtelijke goedkeuring',
    description:
      'De ambtelijke top (DG, SG) keurt het concept goed. ' +
      'Vergelijkbaar met required approvals voordat je mag mergen.',
    col: 3, step: 9,
  },

  // === Back to wetsvoorstel fork ===
  {
    id: 'intern-merge',
    branch: 'wetsvoorstel',
    type: 'merge',
    gitLabel: 'merge topic → fork',
    lawLabel: 'Concept gereed',
    subtitle: 'Interne branch gemerged',
    description:
      'De interne sub-branch wordt samengevoegd in de wetsvoorstel-fork. ' +
      'Het concept is klaar voor verdere uitwerking.',
    col: 2, step: 10,
  },
  {
    id: 'voorstel-mvt',
    branch: 'wetsvoorstel',
    type: 'commit',
    gitLabel: 'commit (MvT)',
    lawLabel: 'Memorie van Toelichting',
    subtitle: 'Uitleg bij het voorstel',
    description:
      'De Memorie van Toelichting legt uit waarom de wet er zo uitziet - ' +
      'vergelijkbaar met design docs en ADRs bij de code.',
    col: 2, step: 11,
  },
  {
    id: 'voorstel-interdepart',
    branch: 'wetsvoorstel',
    type: 'commit',
    gitLabel: 'commit (feedback)',
    lawLabel: 'Interdepartementaal overleg',
    subtitle: 'Andere ministeries reageren',
    description:
      'Het voorstel circuleert langs alle relevante ministeries. ' +
      'Hun feedback wordt verwerkt - cross-team review.',
    col: 2, step: 12,
  },
  {
    id: 'voorstel-consultatie',
    branch: 'wetsvoorstel',
    type: 'commit',
    gitLabel: 'commit (consultatie)',
    lawLabel: 'Internetconsultatie',
    subtitle: '4+ weken publiek commentaar',
    description:
      'Het voorstel gaat op internetconsultatie.nl - een publieke RFC. ' +
      'Burgers, NGOs en bedrijven geven feedback.',
    col: 2, step: 13,
  },
  {
    id: 'voorstel-push',
    branch: 'wetsvoorstel',
    type: 'commit',
    gitLabel: 'git push (PR)',
    lawLabel: 'Ministerraad akkoord',
    subtitle: 'Aangeboden aan Wetgevingskalender',
    description:
      'De Ministerraad keurt het voorstel goed en biedt het aan. ' +
      'Vergelijkbaar met een pull request van een fork naar de upstream develop branch.',
    col: 2, step: 14,
  },

  // === Back to develop ===
  {
    id: 'develop-receive',
    branch: 'develop',
    type: 'merge',
    gitLabel: 'merge fork → develop',
    lawLabel: 'Voorstel in procedure',
    subtitle: 'Opgenomen in kalender',
    description:
      'Het voorstel is opgenomen in de Wetgevingskalender. ' +
      'Vanaf nu doorloopt het de formele parlementaire procedure.',
    col: 1, step: 15,
  },

  // === ADVISORY: Raad van State fork ===
  {
    id: 'rvs-fork',
    branch: 'advisory',
    type: 'branch',
    gitLabel: 'git clone develop (fork)',
    lawLabel: 'RvS adviesaanvraag',
    subtitle: 'Fork naar Raad van State',
    description:
      'Het voorstel wordt via de Koning aan de Raad van State aangeboden. ' +
      'De RvS forkt het voorstel om onafhankelijk te toetsen.',
    col: 4, step: 16,
  },
  {
    id: 'rvs-toets',
    branch: 'advisory',
    type: 'commit',
    gitLabel: 'commit (toetsing)',
    lawLabel: 'Grondwettelijke toets',
    subtitle: 'Toets aan Grondwet',
    description:
      'De RvS toetst het voorstel aan de Grondwet, EU-recht, ' +
      'en bestaande wetgeving. Een diepgaande juridische review.',
    col: 4, step: 17,
  },
  {
    id: 'rvs-kwaliteit',
    branch: 'advisory',
    type: 'commit',
    gitLabel: 'commit (kwaliteit)',
    lawLabel: 'Wetgevingskwaliteit',
    subtitle: 'Duidelijkheid & consistentie',
    description:
      'Toets op wetgevingskwaliteit: is de tekst duidelijk, consistent, ' +
      'en uitvoerbaar? Vergelijkbaar met een code quality review.',
    col: 4, step: 18,
  },
  {
    id: 'rvs-advies',
    branch: 'advisory',
    type: 'commit',
    gitLabel: 'commit (advies + dictum)',
    lawLabel: 'RvS advies',
    subtitle: 'Formeel advies met dictum',
    description:
      'De Raad van State levert een formeel advies met dictum. ' +
      'Het kabinet moet hierop reageren met een Nader Rapport.',
    col: 4, step: 19,
  },
  {
    id: 'rvs-nader-rapport',
    branch: 'advisory',
    type: 'commit',
    gitLabel: 'commit (nader rapport)',
    lawLabel: 'Nader Rapport',
    subtitle: 'Kabinetsreactie op advies',
    description:
      'Het kabinet reageert op het RvS-advies en verwerkt de feedback. ' +
      'Vergelijkbaar met het adresseren van review comments.',
    col: 4, step: 20,
  },

  // === Back to develop: advisory merged ===
  {
    id: 'rvs-merge',
    branch: 'develop',
    type: 'merge',
    gitLabel: 'merge rvs → develop',
    lawLabel: 'Advies verwerkt',
    subtitle: 'Wijzigingen doorgevoerd',
    description:
      'Het advies van de Raad van State is verwerkt in het voorstel. ' +
      'De fork wordt gemerged terug naar develop.',
    col: 1, step: 21,
  },
  {
    id: 'tk-review',
    branch: 'develop',
    type: 'review',
    gitLabel: 'PR review (debat)',
    lawLabel: 'Tweede Kamer',
    subtitle: 'Behandeling & stemming',
    description:
      'De Tweede Kamer behandelt het voorstel - vergelijkbaar met een PR review. ' +
      'Kamerleden dienen amendementen in, debatteren, en stemmen.',
    col: 1, step: 22,
  },
  {
    id: 'tk-amendement',
    branch: 'develop',
    type: 'commit',
    gitLabel: 'commit (amendments)',
    lawLabel: 'Amendementen TK',
    subtitle: 'Wijzigingen door Kamer',
    description:
      'Amendementen van Kamerleden worden verwerkt - vergelijkbaar met ' +
      'reviewer-submitted patches die gemerged worden in de branch.',
    col: 1, step: 23,
  },
  {
    id: 'ek-review',
    branch: 'develop',
    type: 'review',
    gitLabel: 'approve / reject',
    lawLabel: 'Eerste Kamer',
    subtitle: 'Alleen aannemen of verwerpen',
    description:
      'De Eerste Kamer kan alleen aannemen of verwerpen, geen amendementen. ' +
      'Vergelijkbaar met een protected branch gate: approve or reject.',
    col: 1, step: 24,
  },

  // === MAIN: The King merges ===
  {
    id: 'koninklijk-besluit',
    branch: 'main',
    type: 'merge',
    gitLabel: 'merge develop → main',
    lawLabel: 'Koninklijk Besluit',
    subtitle: 'Bekrachtiging door de Koning',
    description:
      'De Koning is de enige maintainer van het Corpus Juris - alleen hij kan mergen ' +
      'naar main, met de Minister als co-author op elke commit. ' +
      'Publicatie in het Staatsblad en inwerkingtreding volgen automatisch.',
    tags: [
      { label: 'Staatsblad', color: 'var(--color-branch-advisory)' },
      { label: 'Inwerkingtreding', color: 'var(--color-branch-main)' },
    ],
    col: 0, step: 25,
  },
  {
    id: 'corpus-updated',
    branch: 'main',
    type: 'commit',
    gitLabel: 'HEAD',
    lawLabel: 'Corpus Juris',
    subtitle: 'Bijgewerkt',
    description:
      'Het Corpus Juris is bijgewerkt met de nieuwe wet. ' +
      'Main gaat door - klaar voor het volgende wetsvoorstel.',
    col: 0, step: 26,
  },
];

export const connections: Connection[] = [
  // main → develop (branch-off with vertical distance)
  { from: 'corpus-start', to: 'develop-start', type: 'branch-off' },
  // develop → wetsvoorstel fork
  { from: 'develop-start', to: 'voorstel-fork', type: 'branch-off' },
  // wetsvoorstel flow
  { from: 'voorstel-fork', to: 'voorstel-beleid', type: 'straight' },
  // wetsvoorstel → internal sub-branch
  { from: 'voorstel-beleid', to: 'intern-start', type: 'branch-off' },
  // internal sub-branch flow
  { from: 'intern-start', to: 'intern-draft', type: 'straight' },
  { from: 'intern-draft', to: 'intern-review', type: 'straight' },
  { from: 'intern-review', to: 'intern-rebase', type: 'straight' },
  { from: 'intern-rebase', to: 'intern-final', type: 'straight' },
  // internal merges back into wetsvoorstel
  { from: 'intern-final', to: 'intern-merge', type: 'merge-in' },
  // wetsvoorstel continues
  { from: 'intern-merge', to: 'voorstel-mvt', type: 'straight' },
  { from: 'voorstel-mvt', to: 'voorstel-interdepart', type: 'straight' },
  { from: 'voorstel-interdepart', to: 'voorstel-consultatie', type: 'straight' },
  { from: 'voorstel-consultatie', to: 'voorstel-push', type: 'straight' },
  // wetsvoorstel merges into develop
  { from: 'voorstel-push', to: 'develop-receive', type: 'merge-in' },
  // develop → RvS advisory fork
  { from: 'develop-receive', to: 'rvs-fork', type: 'branch-off' },
  // advisory flow
  { from: 'rvs-fork', to: 'rvs-toets', type: 'straight' },
  { from: 'rvs-toets', to: 'rvs-kwaliteit', type: 'straight' },
  { from: 'rvs-kwaliteit', to: 'rvs-advies', type: 'straight' },
  { from: 'rvs-advies', to: 'rvs-nader-rapport', type: 'straight' },
  // advisory merges back into develop
  { from: 'rvs-nader-rapport', to: 'rvs-merge', type: 'merge-in' },
  // develop continues through parliamentary process
  { from: 'rvs-merge', to: 'tk-review', type: 'straight' },
  { from: 'tk-review', to: 'tk-amendement', type: 'straight' },
  { from: 'tk-amendement', to: 'ek-review', type: 'straight' },
  // develop merges into main (the King merges!)
  { from: 'ek-review', to: 'koninklijk-besluit', type: 'merge-in' },
  // main continues
  { from: 'koninklijk-besluit', to: 'corpus-updated', type: 'straight' },
  // main continues (dashed) while branches are active
  { from: 'corpus-start', to: 'koninklijk-besluit', type: 'main-continues' },
  // develop continues (dashed) while forks are active
  { from: 'develop-start', to: 'develop-receive', type: 'main-continues' },
  { from: 'develop-receive', to: 'rvs-merge', type: 'main-continues' },
];
