/**
 * Real law: Wet open overheid (Woo) - kamerstuk 33328 + novelle 35112
 *
 * 9+ year journey from initiative bill (initiatiefwet) to entry into force.
 * Dates are real. Row spacing is semi-proportional to time.
 *
 * Layout:
 *   col 0: main (Corpus Juris / Wob)
 *   col 1: initiatiefwet branch (33328 - the proposal)
 *   col 2: parliament (TK/EK treatment)
 *   col 3: CI checks / reviews (RvS, impact analysis)
 *   col 4: amendments (33328)
 *   col 5: novelle branch (35112)
 *   col 6: amendments on novelle (35112)
 */

import type { Branch, Phase, TimelineMarker, Stage, Connection } from './types';

export const branches: Branch[] = [
  {
    id: 'main',
    label: 'Corpus Juris',
    gitLabel: 'main (Wob)',
    color: 'var(--color-branch-main)',
    col: 0,
    startRow: 0,
    endRow: 42,
  },
  {
    id: 'initiative',
    label: '33328',
    gitLabel: 'feature/woo-33328',
    color: 'var(--color-branch-feature)',
    col: 1,
    startRow: 1,
    endRow: 20,
  },
  {
    id: 'parliament',
    label: 'Parlement',
    gitLabel: 'PR #33328',
    color: 'var(--color-branch-feature)',
    col: 2,
    startRow: 13,
    endRow: 37,
  },
  {
    id: 'novelle',
    label: 'Novelle',
    gitLabel: 'fix-PR #35112',
    color: 'var(--color-deploy)',
    col: 3,
  },
  {
    id: 'amend-21-branch',
    label: 'Amd. 21',
    gitLabel: 'patch/33328-21',
    color: 'var(--color-review)',
    col: 3,
  },
  {
    id: 'amend-22-branch',
    label: 'Amd. 22',
    gitLabel: 'patch/33328-22',
    color: 'var(--color-review)',
    col: 4,
  },
  {
    id: 'amend-28-branch',
    label: 'Amd. 28',
    gitLabel: 'patch/33328-28',
    color: 'var(--color-review)',
    col: 5,
  },
  {
    id: 'amend-34-branch',
    label: 'Amd. 34',
    gitLabel: 'patch/33328-34',
    color: 'var(--color-review)',
    col: 6,
  },
  {
    id: 'rejected-33328',
    label: 'Verworpen',
    gitLabel: 'patch/33328-rejected',
    color: 'var(--color-branch-rejected)',
    col: 7,
  },
  {
    id: 'novelle-amend-accepted-branch',
    label: 'Amd. novelle',
    gitLabel: 'patches/35112-accepted',
    color: 'var(--color-review)',
    col: 4,
  },
  {
    id: 'rejected-35112',
    label: 'Verworpen',
    gitLabel: 'patch/35112-rejected',
    color: 'var(--color-branch-rejected)',
    col: 5,
  },
];

export const phases: Phase[] = [
  { id: 'init', label: 'Indiening & voorbereiding', startRow: 1, endRow: 6, color: 'var(--color-branch-feature)' },
  { id: 'rvs', label: 'Raad van State', startRow: 7, endRow: 9, color: 'var(--color-ci)' },
  { id: 'committee', label: 'Commissiebehandeling', startRow: 10, endRow: 12, color: 'var(--color-review)' },
  { id: 'tk', label: 'Tweede Kamer', startRow: 13, endRow: 20, color: 'var(--color-review)' },
  { id: 'stall', label: 'ABDTOPConsult (vertraging)', startRow: 21, endRow: 26, color: 'var(--color-ci)' },
  { id: 'novelle-phase', label: 'Novelle 35112', startRow: 27, endRow: 35, color: 'var(--color-deploy)' },
  { id: 'ek', label: 'Eerste Kamer (33328 + 35112)', startRow: 36, endRow: 38, color: 'var(--color-review)' },
  { id: 'pub', label: 'Bekrachtiging & inwerkingtreding', startRow: 39, endRow: 42, color: 'var(--color-deploy)' },
];

/** Timeline markers shown on the left axis */
export const timelineMarkers: TimelineMarker[] = [
  { label: '2012' },
  { label: '2013' },
  { label: '2014' },
  { label: '2015' },
  { label: '2016' },
  { label: '2016' },
  { label: '2017' },
  { label: '2018' },
  { label: '2019' },
  { label: '2020' },
  { label: '2021' },
  { label: '2021' },
  { label: '2022' },
];

export const stages: Stage[] = [
  // === Corpus Juris (Wob is current law) ===
  {
    id: 'wob',
    branch: 'main',
    type: 'commit',
    gitLabel: 'main (HEAD)',
    lawLabel: 'Wet openbaarheid van bestuur',
    subtitle: 'Wob - geldend recht sinds 1991',
    date: '',
    description:
      'De Wob is het geldende transparantierecht. Burgers moeten zelf informatie opvragen (passief). ' +
      'Critici vinden de wet verouderd: te ruime weigeringsgronden, geen actieve openbaarmaking.',
    col: 0, step: 0,
  },

  // === 2012: Submission ===
  {
    id: 'peters-submit',
    branch: 'initiative',
    type: 'branch',
    gitLabel: 'external contributor PR',
    lawLabel: 'Indiening Peters (GL)',
    subtitle: 'Initiatiefwetsvoorstel',
    date: '5 jul 2012',
    description:
      'Mariko Peters (GroenLinks) dient eigenhandig een initiatiefwetsvoorstel in: ' +
      'de "Nieuwe Wet openbaarheid van bestuur". Een lone external contributor die een PR opent ' +
      'tegen het hele systeem.',
    col: 1, step: 1,
  },
  {
    id: 'mvt',
    branch: 'initiative',
    type: 'commit',
    gitLabel: 'commit (MvT)',
    lawLabel: 'Memorie van Toelichting',
    subtitle: 'Onderbouwing wetsvoorstel',
    date: '10 jul 2012',
    description:
      'De memorie van toelichting beschrijft het doel: van passieve naar actieve openbaarmaking, ' +
      'een Informatiecommissaris, uitbreiding naar semipublieke sector.',
    col: 1, step: 2,
  },
  {
    id: 'rvs-sent',
    branch: 'initiative',
    type: 'commit',
    gitLabel: 'request review',
    lawLabel: 'Verzonden naar RvS',
    subtitle: 'Adviesaanvraag',
    date: '4 okt 2012',
    description:
      'Het voorstel wordt naar de Raad van State gestuurd voor advies.',
    col: 1, step: 3,
  },
  {
    id: 'handover-1',
    branch: 'initiative',
    type: 'commit',
    gitLabel: 'git remote set-url (maintainer change)',
    lawLabel: 'Peters → Voortman',
    subtitle: 'Eerste overname verdediging',
    date: '22 nov 2012',
    description:
      'Peters verlaat de Kamer. Linda Voortman (GL) neemt de verdediging over. ' +
      'De eerste van zes overdrachten - alsof de maintainer van een open-source project vertrekt ' +
      'en iemand anders het overneemt.',
    col: 1, step: 4,
  },
  {
    id: 'govt-response',
    branch: 'initiative',
    type: 'review',
    gitLabel: 'maintainer comment',
    lawLabel: 'Reactie regering (Plasterk)',
    subtitle: '"We wachten af"',
    date: '15 aug 2013',
    description:
      'Minister Plasterk (PvdA, BZK): de regering dient geen eigen alternatief in maar ' +
      '"wacht de behandeling af". Passief-agressieve houding van de project owner.',
    col: 1, step: 5,
  },

  // === 2013: Raad van State ===
  {
    id: 'rvs-advies',
    branch: 'initiative',
    type: 'ci-check',
    gitLabel: 'CI: senior review',
    lawLabel: 'Advies Raad van State',
    subtitle: 'Substantiële herziening nodig',
    date: '12 dec 2013',
    description:
      'De RvS levert advies na 14 maanden. Het advies leidt tot een substantiële herziening ' +
      'van het voorstel. Tegelijkertijd sluit Gerard Schouw (D66) aan als co-verdediger.',
    col: 3, step: 6,
  },
  {
    id: 'schouw-joins',
    branch: 'initiative',
    type: 'commit',
    gitLabel: 'add co-author',
    lawLabel: 'Schouw (D66) sluit aan',
    subtitle: 'Tweede co-verdediger',
    date: '12 dec 2013',
    description:
      'Gerard Schouw (D66) wordt tweede verdediger. Het voorstel krijgt een breder draagvlak. ' +
      'Vergelijkbaar met een co-author die aan de PR wordt toegevoegd.',
    col: 1, step: 6,
  },
  {
    id: 'revised-bill',
    branch: 'initiative',
    type: 'commit',
    gitLabel: 'force push (revised)',
    lawLabel: 'Gewijzigd voorstel + nader rapport',
    subtitle: 'Verwerking RvS feedback',
    date: '12 dec 2013',
    description:
      'Het voorstel wordt substantieel herzien op basis van het RvS advies. ' +
      'Vergelijkbaar met een force push na ingrijpende review feedback.',
    col: 1, step: 7,
  },

  // === 2014: Committee Phase ===
  {
    id: 'verslag',
    branch: 'initiative',
    type: 'review',
    gitLabel: 'PR review comments',
    lawLabel: 'Verslag commissie BiZa',
    subtitle: 'Schriftelijke vragen',
    date: '5 feb 2014',
    description:
      'De vaste commissie Binnenlandse Zaken stelt schriftelijke vragen.',
    col: 1, step: 8,
  },
  {
    id: 'nota-nav-1',
    branch: 'initiative',
    type: 'commit',
    gitLabel: 'respond + push (1e NvW)',
    lawLabel: 'Nota n.a.v. verslag + 1e NvW',
    subtitle: 'Beantwoording + aanpassingen',
    date: '13 mei 2014',
    description:
      'Initiatiefnemers beantwoorden de vragen en dienen de eerste nota van wijziging in.',
    col: 1, step: 9,
  },
  {
    id: 'nvw-2',
    branch: 'initiative',
    type: 'commit',
    gitLabel: 'push (2e NvW)',
    lawLabel: '2e nota van wijziging',
    subtitle: 'Verdere aanpassingen',
    date: '9 okt 2014',
    description:
      'Tweede nota van wijziging: scope uitgebreid naar "publieke taak"-instellingen, ' +
      'hergebruikregels toegevoegd.',
    col: 1, step: 10,
  },

  // === 2015: Quiet year - sponsorship transfers ===
  {
    id: 'handovers-2015',
    branch: 'initiative',
    type: 'commit',
    gitLabel: 'maintainer changes (×2)',
    lawLabel: 'Overdrachten D66',
    subtitle: 'Schouw → Verhoeven → Van Weyenberg',
    date: '2015',
    description:
      'In 2015 wisselt de D66-verdediger twee keer: Schouw → Verhoeven (aug) → Van Weyenberg (okt). ' +
      'Het voorstel overleeft drie maintainer-wisselingen in één jaar.',
    col: 1, step: 11,
  },

  // === 2016 March-April: Tweede Kamer (the burst) ===
  {
    id: 'nvw-3',
    branch: 'parliament',
    type: 'commit',
    gitLabel: 'push (3e NvW)',
    lawLabel: '3e nota van wijziging',
    subtitle: 'Voorbereiding plenair',
    date: '24 mrt 2016',
    description:
      'Derde nota van wijziging: hergebruik-hoofdstuk verwijderd (apart geregeld), ' +
      'responstermijn van 2 naar 4 weken, Informatiecommissaris 4 jaar uitgesteld.',
    col: 2, step: 12,
  },
  {
    id: 'plenair-1',
    branch: 'parliament',
    type: 'review',
    gitLabel: 'PR discussion (1e termijn)',
    lawLabel: 'Plenair debat eerste termijn',
    subtitle: 'Behandeling gestart',
    date: '31 mrt 2016',
    description:
      'Het plenaire debat begint. De behandeling wordt over meerdere dagen verspreid.',
    col: 2, step: 13,
  },

  // Amendments on 33328 (parallel, filed 7-13 april 2016)
  {
    id: 'amend-21',
    branch: 'amend-21-branch',
    type: 'commit',
    gitLabel: 'patch (Oosenbrug/Fokke)',
    lawLabel: 'Amd. 21: Klachtprocedure',
    subtitle: 'PvdA - aangenomen',
    date: '7 apr 2016',
    description:
      'Amendement Oosenbrug/Fokke (PvdA): herintroductie klachtprocedure bij niet-tijdig beslissen.',
    col: 3, step: 14,
  },
  {
    id: 'amend-22',
    branch: 'amend-22-branch',
    type: 'commit',
    gitLabel: 'patch (Segers/Veldman)',
    lawLabel: 'Amd. 22: Wob-jurisprudentie',
    subtitle: 'CU/VVD - aangenomen',
    date: '12 apr 2016',
    description:
      'Amendement Segers/Veldman (CU/VVD): aansluiting bij bestaande Wob-jurisprudentie.',
    col: 4, step: 14,
  },
  {
    id: 'amend-28',
    branch: 'amend-28-branch',
    type: 'commit',
    gitLabel: 'patch (Segers/Oosenbrug)',
    lawLabel: 'Amd. 28: Kamerlid-info',
    subtitle: 'CU/PvdA - aangenomen',
    date: '13 apr 2016',
    description:
      'Amendement Segers/Oosenbrug: informatie aan individuele Kamerleden beschermd.',
    col: 5, step: 14,
  },
  {
    id: 'amend-34',
    branch: 'amend-34-branch',
    type: 'commit',
    gitLabel: 'patch (Veldman/Bisschop)',
    lawLabel: 'Amd. 34: Commissaris uitstellen',
    subtitle: 'VVD/SGP - aangenomen',
    date: '13 apr 2016',
    description:
      'Amendement Veldman/Bisschop: Informatiecommissaris pas na evaluatie. ' +
      'Gewijzigde versie van eerder ingediend amendement.',
    col: 6, step: 14,
  },
  {
    id: 'amend-rejected',
    branch: 'rejected-33328',
    type: 'commit',
    gitLabel: 'patches REJECTED (×3)',
    lawLabel: '3 amendementen verworpen',
    subtitle: 'VVD/SGP - scope + bedrijfsgegevens',
    date: '13 apr 2016',
    description:
      'Drie amendementen van Veldman/Bisschop (VVD/SGP) verworpen: reikwijdte beperken (23), ' +
      'beleidsopvattingen uitsluiten (24), bedrijfsgegevens absoluut maken (33). ' +
      'NB: amendement 33 (bedrijfsgegevens) wordt later via de novelle alsnog aangenomen!',
    col: 7, step: 14,
  },

  {
    id: 'nvw-4',
    branch: 'parliament',
    type: 'commit',
    gitLabel: 'push (4e NvW)',
    lawLabel: '4e nota van wijziging',
    subtitle: 'Laatste technische aanpassingen',
    date: '13 apr 2016',
    description:
      'Vierde en laatste nota van wijziging: technische correcties, verwijzingen bijgewerkt.',
    col: 2, step: 14,
  },
  {
    id: 'plenair-2',
    branch: 'parliament',
    type: 'review',
    gitLabel: 'PR discussion (re/dupliek)',
    lawLabel: 'Plenair debat afronden',
    subtitle: 'Re- en dupliek',
    date: '13 apr 2016',
    description:
      'Het plenaire debat wordt afgerond met re- en dupliek.',
    col: 2, step: 16,
  },

  // TK vote
  {
    id: 'tk-vote',
    branch: 'parliament',
    type: 'merge',
    gitLabel: 'merge approved (94-56)',
    lawLabel: 'Stemming Tweede Kamer',
    subtitle: '94 voor, 56 tegen',
    date: '19 apr 2016',
    description:
      'AANGENOMEN. Voor: SP, PvdD, PvdA, GL, D66, 50PLUS, CU, PVV. ' +
      'Tegen: VVD, CDA, SGP. De VVD stemt tegen - dit verandert later.',
    col: 2, step: 17,
  },

  // === 2016-2017: The Stall (impact analysis) ===
  {
    id: 'ek-expert',
    branch: 'parliament',
    type: 'review',
    gitLabel: 'expert review requested',
    lawLabel: 'Deskundigenbijeenkomst EK',
    subtitle: 'Experts gehoord',
    date: '7 jun 2016',
    description:
      'De Eerste Kamer houdt een deskundigenbijeenkomst. Een ongebruikelijke stap - ' +
      'de EK wil eerst experts horen voordat ze het voorstel inhoudelijk behandelt.',
    col: 2, step: 18,
  },
  {
    id: 'impact-commissioned',
    branch: 'parliament',
    type: 'ci-check',
    gitLabel: 'CI: external audit ordered',
    lawLabel: 'ABDTOPConsult opdracht',
    subtitle: 'Minister Blok (VVD) bestelt onderzoek',
    date: '1 sep 2016',
    description:
      'Minister Blok (VVD) geeft ABDTOPConsult opdracht de uitvoerbaarheid te onderzoeken. ' +
      'Critici noemen dit een vertragingstactiek: de regering probeert het voorstel te torpederen ' +
      'door aan te tonen dat het "onuitvoerbaar" is.',
    col: 3, step: 19,
  },
  {
    id: 'impact-1',
    branch: 'parliament',
    type: 'ci-check',
    gitLabel: 'CI: FAIL - "unexecutable"',
    lawLabel: 'Impactanalyse deel 1',
    subtitle: '"Onuitvoerbaar, zeer hoge kosten"',
    date: '15 dec 2016',
    description:
      'Deel 1 van de impactanalyse: rijksoverheid, uitvoeringsorganisaties, politie. ' +
      'Conclusie: "het wetsvoorstel is in de huidige vorm onuitvoerbaar en brengt zeer ' +
      'hoge uitvoeringskosten met zich mee." De CI pipeline faalt.',
    col: 3, step: 20,
  },
  {
    id: 'impact-2',
    branch: 'parliament',
    type: 'ci-check',
    gitLabel: 'CI: FAIL - part 2',
    lawLabel: 'Impactanalyse deel 2',
    subtitle: 'Gemeenten, provincies, waterschappen',
    date: '13 jun 2017',
    description:
      'Deel 2 bevestigt de problemen voor decentrale overheden.',
    col: 3, step: 21,
  },
  {
    id: 'handover-snels',
    branch: 'parliament',
    type: 'commit',
    gitLabel: 'maintainer change (×2)',
    lawLabel: 'Voortman → Snels, VW → Sneller',
    subtitle: 'Nieuwe verdedigers na verkiezingen 2017',
    date: '2017',
    description:
      'Na de verkiezingen van maart 2017 wisselen beide verdedigers: ' +
      'Voortman → Bart Snels (GL), Van Weyenberg → Joost Sneller (D66). ' +
      'Het voorstel overleeft opnieuw een volledige maintainer-wissel.',
    col: 2, step: 22,
  },
  {
    id: 'negotiations',
    branch: 'parliament',
    type: 'review',
    gitLabel: 'stalled - negotiating fix',
    lawLabel: 'Onderhandelingen novelle',
    subtitle: 'Initiatiefnemers en regering overleggen',
    date: '2017-2018',
    description:
      'De behandeling in de Eerste Kamer wordt opgeschort. Initiatiefnemers en regering ' +
      'onderhandelen over een compromis: een novelle die de bezwaren wegneemt. ' +
      'De PR staat on hold terwijl een fix-PR wordt voorbereid.',
    col: 2, step: 23,
  },

  // === 2019: Novelle ===
  {
    id: 'novelle-submit',
    branch: 'novelle',
    type: 'branch',
    gitLabel: 'dependent fix-PR #35112',
    lawLabel: 'Novelle ingediend',
    subtitle: 'Snels (GL) & Van Weyenberg (D66)',
    date: '2 jan 2019',
    description:
      'De novelle (35112) wordt ingediend: een apart wetsvoorstel dat het origineel wijzigt. ' +
      'Een dependent fix-PR die eerst door de hele TK-pipeline moet voordat de EK ' +
      'het origineel kan behandelen.',
    col: 3, step: 24,
  },
  {
    id: 'novelle-rvs',
    branch: 'novelle',
    type: 'ci-check',
    gitLabel: 'CI: review novelle',
    lawLabel: 'RvS advies novelle',
    subtitle: 'Aanbevelingen verwerkt',
    date: '12 apr 2019',
    description:
      'De Raad van State adviseert over de novelle. Aanbevelingen worden verwerkt.',
    col: 3, step: 25,
  },
  {
    id: 'novelle-revised',
    branch: 'novelle',
    type: 'commit',
    gitLabel: 'push (revised)',
    lawLabel: 'Herzien voorstel novelle',
    subtitle: 'RvS feedback verwerkt',
    date: '30 jun 2020',
    description:
      'Het novelle-voorstel wordt herzien op basis van het RvS advies.',
    col: 3, step: 26,
  },
  {
    id: 'novelle-committee',
    branch: 'novelle',
    type: 'review',
    gitLabel: 'committee review',
    lawLabel: 'Commissiebehandeling novelle',
    subtitle: 'Verslag + nota n.a.v. verslag',
    date: '22 sep 2020',
    description:
      'De commissie behandelt de novelle: verslag en beantwoording.',
    col: 3, step: 27,
  },

  // Novelle amendments (parallel, jan 2021)
  {
    id: 'novelle-amend-accepted',
    branch: 'novelle-amend-accepted-branch',
    type: 'commit',
    gitLabel: 'patches accepted (×6)',
    lawLabel: '6 amendementen aangenomen',
    subtitle: 'CDA, D66, GL, SGP, VVD',
    date: '11-14 jan 2021',
    description:
      'Zes amendementen aangenomen op de novelle, waaronder: ' +
      'Van der Molen (CDA): geanonimiseerde beleidsopvattingen openbaar (n.a.v. toeslagenaffaire). ' +
      'Snoeren/Bisschop (VVD/SGP): bedrijfsgegevens als absolute weigeringsgrond - ' +
      'dezelfde inhoud als het in 2016 verworpen amendement 33328-33! Politieke verschuiving.',
    col: 4, step: 28,
  },
  {
    id: 'novelle-amend-rejected',
    branch: 'rejected-35112',
    type: 'commit',
    gitLabel: 'patch rejected (×1)',
    lawLabel: '1 amendement verworpen',
    subtitle: 'Sneller/Buitenweg (D66/GL)',
    date: '14 jan 2021',
    description:
      'Eén amendement verworpen: verplichting om alle documenten bij besluit mee te sturen.',
    col: 5, step: 28,
  },

  // Novelle TK vote
  {
    id: 'novelle-tk-vote',
    branch: 'novelle',
    type: 'merge',
    gitLabel: 'fix-PR merged (130-20)',
    lawLabel: 'Stemming TK novelle',
    subtitle: '130 voor, 20 tegen - VVD nu VOOR',
    date: '26 jan 2021',
    description:
      'De novelle wordt aangenomen met overweldigende meerderheid. ' +
      'Cruciaal: de VVD stemt nu VOOR (was tegen in 2016). ' +
      'De toeslagenaffaire heeft het politieke landschap fundamenteel verschoven. ' +
      'Alleen PVV stemt tegen.',
    col: 3, step: 29,
  },

  // === Eerste Kamer: combined treatment ===
  {
    id: 'ek-written',
    branch: 'parliament',
    type: 'review',
    gitLabel: 'final review (written)',
    lawLabel: 'EK schriftelijke voorbereiding',
    subtitle: '3 rondes vragen + antwoorden',
    date: 'apr-sep 2021',
    description:
      'De Eerste Kamer behandelt beide voorstellen (33328 + 35112) gezamenlijk. ' +
      'Drie schriftelijke rondes: voorlopig verslag, memorie van antwoord, ' +
      'nader voorlopig verslag, nadere memorie van antwoord, verslag, nota.',
    col: 2, step: 30,
  },
  {
    id: 'ek-debate',
    branch: 'parliament',
    type: 'review',
    gitLabel: 'final review meeting',
    lawLabel: 'Plenaire behandeling EK',
    subtitle: 'Gezamenlijk debat 33328 + 35112',
    date: '28 sep 2021',
    description:
      'Plenair debat over beide voorstellen tegelijk. Drie moties ingediend, ' +
      'één ingetrokken. De Eerste Kamer kan niet amenderen - alleen aannemen of verwerpen.',
    col: 2, step: 31,
  },
  {
    id: 'ek-vote',
    branch: 'parliament',
    type: 'merge',
    gitLabel: 'both PRs approved',
    lawLabel: 'Stemming Eerste Kamer',
    subtitle: 'BEIDE aangenomen - CDA/CU tegen',
    date: '5 okt 2021',
    description:
      'Beide voorstellen aangenomen bij zitten en opstaan. ' +
      'Tegen 33328: SGP, CDA, CU. Tegen 35112: CDA, CU. ' +
      'Bijzonder: SGP stemt TEGEN 33328 maar VOOR 35112.',
    col: 2, step: 32,
  },

  // === Publication & entry into force ===
  {
    id: 'royal-assent',
    branch: 'main',
    type: 'merge',
    gitLabel: 'merge to main',
    lawLabel: 'Bekrachtiging door de Koning',
    subtitle: 'Ondertekend',
    date: '25 okt 2021',
    description:
      'De Koning ondertekent beide wetten. De feature branch wordt gemerged naar main. ' +
      'Publicatie in Staatsblad (Stb. 2021, 499+500) op 27 okt 2021. ' +
      'Inwerkingtreding per 1 mei 2022 - de Wob wordt ingetrokken.',
    tags: [
      { label: 'Stb. 2021, 499+500', color: 'var(--color-branch-advisory)' },
      { label: '1 mei 2022 - go live', color: 'var(--color-branch-main)' },
    ],
    col: 0, step: 33,
  },
  {
    id: 'corpus-updated',
    branch: 'main',
    type: 'commit',
    gitLabel: 'HEAD',
    lawLabel: 'Corpus Juris',
    subtitle: 'Woo is geldend recht',
    date: '',
    description:
      'Het Corpus Juris is bijgewerkt. De Wob is vervangen door de Woo.',
    col: 0, step: 36,
  },
];

export const connections: Connection[] = [
  // Branch off from main
  { from: 'wob', to: 'peters-submit', type: 'branch-off' },

  // Initiative development
  { from: 'peters-submit', to: 'mvt', type: 'straight' },
  { from: 'mvt', to: 'rvs-sent', type: 'straight' },
  { from: 'rvs-sent', to: 'handover-1', type: 'straight' },
  { from: 'handover-1', to: 'govt-response', type: 'straight' },

  // RvS advice
  { from: 'govt-response', to: 'rvs-advies', type: 'ci-fork' },
  { from: 'rvs-advies', to: 'schouw-joins', type: 'ci-return' },
  { from: 'govt-response', to: 'schouw-joins', type: 'straight' },
  { from: 'schouw-joins', to: 'revised-bill', type: 'straight' },

  // Committee phase
  { from: 'revised-bill', to: 'verslag', type: 'straight' },
  { from: 'verslag', to: 'nota-nav-1', type: 'straight' },
  { from: 'nota-nav-1', to: 'nvw-2', type: 'straight' },
  { from: 'nvw-2', to: 'handovers-2015', type: 'straight' },

  // Transition to parliament
  { from: 'handovers-2015', to: 'nvw-3', type: 'branch-off' },

  // TK treatment
  { from: 'nvw-3', to: 'plenair-1', type: 'straight' },
  { from: 'plenair-1', to: 'nvw-4', type: 'straight' },

  // Amendments: each forks from plenair-1, accepted merge back, rejected dead-end
  { from: 'plenair-1', to: 'amend-21', type: 'branch-off' },
  { from: 'plenair-1', to: 'amend-22', type: 'branch-off' },
  { from: 'plenair-1', to: 'amend-28', type: 'branch-off' },
  { from: 'plenair-1', to: 'amend-34', type: 'branch-off' },
  { from: 'plenair-1', to: 'amend-rejected', type: 'branch-off' },
  // Accepted amendments merge back
  { from: 'amend-21', to: 'plenair-2', type: 'merge-in' },
  { from: 'amend-22', to: 'plenair-2', type: 'merge-in' },
  { from: 'amend-28', to: 'plenair-2', type: 'merge-in' },
  { from: 'amend-34', to: 'plenair-2', type: 'merge-in' },
  // amend-rejected: dead end - no merge back (rejected)
  { from: 'nvw-4', to: 'plenair-2', type: 'straight' },

  { from: 'plenair-2', to: 'tk-vote', type: 'straight' },

  // Post-TK: the stall
  { from: 'tk-vote', to: 'ek-expert', type: 'straight' },
  { from: 'ek-expert', to: 'impact-commissioned', type: 'ci-fork' },
  { from: 'impact-commissioned', to: 'impact-1', type: 'straight' },
  { from: 'impact-1', to: 'impact-2', type: 'straight' },
  { from: 'ek-expert', to: 'handover-snels', type: 'straight' },
  { from: 'impact-2', to: 'handover-snels', type: 'ci-return' },
  { from: 'handover-snels', to: 'negotiations', type: 'straight' },

  // Novelle branches off
  { from: 'negotiations', to: 'novelle-submit', type: 'branch-off' },
  { from: 'novelle-submit', to: 'novelle-rvs', type: 'straight' },
  { from: 'novelle-rvs', to: 'novelle-revised', type: 'straight' },
  { from: 'novelle-revised', to: 'novelle-committee', type: 'straight' },

  // Novelle amendments: each forks, accepted merge back, rejected dead-end
  { from: 'novelle-committee', to: 'novelle-amend-accepted', type: 'branch-off' },
  { from: 'novelle-committee', to: 'novelle-amend-rejected', type: 'branch-off' },
  { from: 'novelle-amend-accepted', to: 'novelle-tk-vote', type: 'merge-in' },
  // novelle-amend-rejected: dead end - no merge back (rejected)
  { from: 'novelle-committee', to: 'novelle-tk-vote', type: 'straight' },

  // Both go to EK
  { from: 'negotiations', to: 'ek-written', type: 'straight' },
  { from: 'novelle-tk-vote', to: 'ek-written', type: 'merge-in' },

  { from: 'ek-written', to: 'ek-debate', type: 'straight' },
  { from: 'ek-debate', to: 'ek-vote', type: 'straight' },

  // Merge to main
  { from: 'ek-vote', to: 'royal-assent', type: 'merge-in' },

  // Main continues
  { from: 'wob', to: 'royal-assent', type: 'main-continues' },
  { from: 'royal-assent', to: 'corpus-updated', type: 'straight' },
];
