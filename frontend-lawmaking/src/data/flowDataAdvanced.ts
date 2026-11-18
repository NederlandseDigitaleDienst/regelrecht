/**
 * Advanced flow data: the full Dutch legislative process.
 *
 * Branch model (GitFlow):
 *   col 0: main (Corpus Juris - the law in force)
 *   col 1: develop (Wetgevingskalender - proposals in procedure)
 *   col 2: ministry fork (wetsvoorstel - the ministry's own workspace)
 *   col 3: internal sub-branch (policy + legal team)
 *   col 4-8: parallel advisory/check forks (assessments, consultation)
 *   col 3-7: amendment branches (each amendment its own branch)
 *   col 3: novelle (repair fork)
 */

import type { Branch, Phase, Stage, Connection } from './types';

export const branches: Branch[] = [
  {
    id: 'main',
    label: 'Corpus Juris',
    gitLabel: 'main',
    color: 'var(--color-branch-main)',
    col: 0,
  },
  {
    id: 'develop',
    label: 'Wetgevingskalender',
    gitLabel: 'develop',
    color: 'var(--color-branch-develop)',
    col: 1,
  },
  {
    id: 'ministry',
    label: 'Ministerie',
    gitLabel: 'fork/wetsvoorstel',
    color: 'var(--color-branch-wetsvoorstel)',
    col: 2,
  },
  {
    id: 'internal',
    label: 'Intern team',
    gitLabel: 'topic/*',
    color: 'var(--color-branch-internal)',
    col: 3,
  },
  {
    id: 'uh-toets-branch',
    label: 'U&H toets',
    gitLabel: 'fork/uh-toets',
    color: 'var(--color-branch-advisory)',
    col: 4,
  },
  {
    id: 'regeldruk-branch',
    label: 'Regeldruk',
    gitLabel: 'fork/regeldruk',
    color: 'var(--color-branch-advisory)',
    col: 5,
  },
  {
    id: 'financieel-branch',
    label: 'Financieel',
    gitLabel: 'fork/financieel',
    color: 'var(--color-branch-advisory)',
    col: 6,
  },
  {
    id: 'privacy-branch',
    label: 'Privacy',
    gitLabel: 'fork/dpia',
    color: 'var(--color-branch-advisory)',
    col: 7,
  },
  {
    id: 'consultatie-branch',
    label: 'Consultatie',
    gitLabel: 'fork/consultatie',
    color: 'var(--color-branch-advisory)',
    col: 4,
  },
  {
    id: 'adviesorganen-branch',
    label: 'Adviesorganen',
    gitLabel: 'fork/advies',
    color: 'var(--color-branch-advisory)',
    col: 5,
  },
  {
    id: 'rvs-branch',
    label: 'Raad van State',
    gitLabel: 'fork/rvs',
    color: 'var(--color-branch-advisory)',
    col: 4,
  },
  {
    id: 'initiatief-branch',
    label: 'Initiatiefwet',
    gitLabel: 'external-contributor/*',
    color: 'var(--color-review)',
    col: 3,
  },
  {
    id: 'amend-a-branch',
    label: 'Amd. A',
    gitLabel: 'patch/amd-a',
    color: 'var(--color-review)',
    col: 3,
  },
  {
    id: 'amend-b-branch',
    label: 'Amd. B',
    gitLabel: 'patch/amd-b',
    color: 'var(--color-review)',
    col: 4,
  },
  {
    id: 'amend-c-branch',
    label: 'Amd. C',
    gitLabel: 'patch/amd-c',
    color: 'var(--color-review)',
    col: 5,
  },
  {
    id: 'sub-amend-branch',
    label: 'Sub-amd.',
    gitLabel: 'patch/sub-amd',
    color: 'var(--color-review)',
    col: 6,
  },
  {
    id: 'rejected-branch',
    label: 'Verworpen',
    gitLabel: 'patch/rejected',
    color: 'var(--color-branch-rejected)',
    col: 7,
  },
  {
    id: 'novelle-branch',
    label: 'Novelle',
    gitLabel: 'fix-PR/novelle',
    color: 'var(--color-branch-wetsvoorstel)',
    col: 3,
  },
];

export const phases: Phase[] = [
  { id: 'A', label: 'A. Departementale voorbereiding', startStage: 'ministry-fork', endStage: 'ministry-merge-intern', color: 'var(--color-branch-wetsvoorstel)' },
  { id: 'B', label: 'B. Interdepartementaal & toetsen', startStage: 'interdept', endStage: 'voorportaal', color: 'var(--color-branch-advisory)' },
  { id: 'C', label: 'C. Externe consultatie', startStage: 'internetconsultatie', endStage: 'verwerking-reacties', color: 'var(--color-branch-advisory)' },
  { id: 'D', label: 'D. Kabinet', startStage: 'onderraad', endStage: 'develop-receive', color: 'var(--color-branch-develop)' },
  { id: 'E', label: 'E. Raad van State', startStage: 'rvs-fork', endStage: 'rvs-merge', color: 'var(--color-branch-advisory)' },
  { id: 'F', label: 'F. Tweede Kamer', startStage: 'koninklijke-boodschap', endStage: 'stemmingen', color: 'var(--color-branch-develop)' },
  { id: 'G', label: 'G. Eerste Kamer', startStage: 'ek-behandeling', endStage: 'ek-final', color: 'var(--color-branch-develop)' },
  { id: 'H', label: 'H. Bekrachtiging', startStage: 'koninklijk-besluit', endStage: 'corpus-updated', color: 'var(--color-branch-main)' },
];

export const stages: Stage[] = [
  // === MAIN ===
  {
    id: 'corpus-start',
    branch: 'main',
    type: 'commit',
    gitLabel: 'HEAD (main)',
    lawLabel: 'Corpus Juris',
    subtitle: 'Geldend recht',
    description: 'Het geheel van alle geldende Nederlandse wetgeving.',
    col: 0, step: 0,
  },

  // === DEVELOP ===
  {
    id: 'develop-start',
    branch: 'develop',
    type: 'branch',
    gitLabel: 'git checkout -b develop',
    lawLabel: 'Wetgevingskalender',
    subtitle: 'Voorstellen in procedure',
    description: 'De develop branch bevat alle wetsvoorstellen die officieel in procedure zijn.',
    col: 1, step: 1,
  },

  // === PHASE A: Ministry fork ===
  {
    id: 'ministry-fork',
    branch: 'ministry',
    type: 'branch',
    gitLabel: 'git clone (fork)',
    lawLabel: 'Beleidsidee',
    subtitle: 'Ministerie forkt naar eigen omgeving',
    description:
      'Een wetgevend traject begint met een aanleiding: regeerakkoord, EU-richtlijn, ' +
      'rechterlijke uitspraak, of maatschappelijk probleem. Het ministerie forkt het Corpus Juris.',
    col: 2, step: 2,
  },
  {
    id: 'intern-start',
    branch: 'internal',
    type: 'branch',
    gitLabel: 'git checkout -b concept',
    lawLabel: 'Beleidsnota + conceptwet',
    subtitle: 'Parallel: beleid en recht',
    description:
      'Beleidsmedewerkers en wetgevingsjuristen werken parallel. ' +
      'Beleidsnota (wat?) en juridische tekst (hoe?) worden gelijktijdig ontwikkeld.',
    col: 3, step: 3,
  },
  {
    id: 'intern-mvt',
    branch: 'internal',
    type: 'commit',
    gitLabel: 'commit (MvT)',
    lawLabel: 'Memorie van Toelichting',
    subtitle: 'Onderbouwing & artikelsgewijze toelichting',
    description:
      'De MvT beschrijft het doel, de achtergrond, en de artikelsgewijze toelichting.',
    col: 3, step: 4,
  },
  {
    id: 'intern-toets',
    branch: 'internal',
    type: 'commit',
    gitLabel: 'commit (review)',
    lawLabel: 'Departementale toets',
    subtitle: 'Directie Wetgeving / JenV',
    description:
      'Toets aan de Aanwijzingen voor de regelgeving. Bij JenV ook een wetgevingstoets ' +
      'op rechtsstatelijke kwaliteit.',
    col: 3, step: 5,
  },
  {
    id: 'intern-signoff',
    branch: 'internal',
    type: 'commit',
    gitLabel: 'approve (chain)',
    lawLabel: 'Akkoord DG/SG',
    subtitle: 'Ambtelijke goedkeuringsketen',
    description:
      'Goedkeuringsketen: wetgevingsjurist → afdelingshoofd → directeur → DG → SG → minister.',
    col: 3, step: 6,
  },
  {
    id: 'ministry-merge-intern',
    branch: 'ministry',
    type: 'merge',
    gitLabel: 'merge topic → fork',
    lawLabel: 'Intern concept gereed',
    subtitle: 'Beleid + recht samengevoegd',
    description: 'De interne branches worden samengevoegd in de ministry fork.',
    col: 2, step: 7,
  },

  // === PHASE B: Interdepartmental + parallel checks ===
  {
    id: 'interdept',
    branch: 'ministry',
    type: 'commit',
    gitLabel: 'commit (cross-team)',
    lawLabel: 'Interdepartementaal overleg',
    subtitle: 'IOWJZ / IHW afstemming',
    description:
      'Het concept circuleert langs alle betrokken ministeries. Feedback wordt verwerkt.',
    col: 2, step: 8,
  },
  // Parallel advisory forks
  {
    id: 'uh-toets',
    branch: 'uh-toets-branch',
    type: 'commit',
    gitLabel: 'fork → CI: integration test',
    lawLabel: 'Uitvoerbaarheidstoets',
    subtitle: 'UWV / Belastingdienst / SVB',
    description: 'Uitvoeringsorganisaties toetsen of de wet uitvoerbaar en handhaafbaar is.',
    col: 4, step: 9,
  },
  {
    id: 'regeldruk',
    branch: 'regeldruk-branch',
    type: 'commit',
    gitLabel: 'fork → CI: performance test',
    lawLabel: 'Regeldruktoets (ATR)',
    subtitle: 'Administratieve lasten',
    description: 'Het Adviescollege Toetsing Regeldruk beoordeelt de regeldruk.',
    col: 5, step: 9,
  },
  {
    id: 'financieel',
    branch: 'financieel-branch',
    type: 'commit',
    gitLabel: 'fork → CI: budget check',
    lawLabel: 'Financiële toets',
    subtitle: 'Ministerie van Financiën',
    description: 'Beoordeling van de budgettaire gevolgen.',
    col: 6, step: 9,
  },
  {
    id: 'privacy-toets',
    branch: 'privacy-branch',
    type: 'commit',
    gitLabel: 'fork → CI: security scan',
    lawLabel: 'DPIA / Privacy toets',
    subtitle: 'Autoriteit Persoonsgegevens',
    description: 'Data Protection Impact Assessment.',
    col: 7, step: 9,
  },
  {
    id: 'voorportaal',
    branch: 'ministry',
    type: 'merge',
    gitLabel: 'merge all checks → fork',
    lawLabel: 'Ambtelijk voorportaal',
    subtitle: 'Resultaten samenbrengen',
    description:
      'Senior ambtenaren bespreken het voorstel met alle toetsresultaten. ' +
      'Alle parallelle checks zijn groen.',
    col: 2, step: 10,
  },

  // === PHASE C: External consultation (parallel forks) ===
  {
    id: 'internetconsultatie',
    branch: 'consultatie-branch',
    type: 'commit',
    gitLabel: 'fork → public RFC',
    lawLabel: 'Internetconsultatie',
    subtitle: '4+ weken publiek commentaar',
    description:
      'Het concept wordt gepubliceerd op internetconsultatie.nl. ' +
      'Burgers, bedrijven, NGOs en experts reageren.',
    col: 4, step: 11,
  },
  {
    id: 'adviesorganen',
    branch: 'adviesorganen-branch',
    type: 'commit',
    gitLabel: 'fork → domain experts',
    lawLabel: 'Adviesorganen',
    subtitle: 'SER, RvdR, AP, etc.',
    description: 'Gespecialiseerde organen geven advies. Loopt parallel met internetconsultatie.',
    col: 5, step: 11,
  },
  {
    id: 'verwerking-reacties',
    branch: 'ministry',
    type: 'merge',
    gitLabel: 'merge consultatie → fork',
    lawLabel: 'Verwerking reacties',
    subtitle: 'Aanpassingen n.a.v. consultatie',
    description: 'Het ministerie verwerkt alle reacties uit consultatie en adviezen.',
    col: 2, step: 12,
  },

  // === PHASE D: Cabinet ===
  {
    id: 'onderraad',
    branch: 'ministry',
    type: 'commit',
    gitLabel: 'commit (politiek)',
    lawLabel: 'Onderraad',
    subtitle: 'Relevante ministers',
    description: 'De relevante ministeriële onderraad bespreekt het voorstel.',
    col: 2, step: 13,
  },
  {
    id: 'ministerraad',
    branch: 'ministry',
    type: 'commit',
    gitLabel: 'approve (cabinet)',
    lawLabel: 'Ministerraad',
    subtitle: 'Kabinetsbesluit (elke vrijdag)',
    description: 'De voltallige ministerraad keurt het voorstel goed.',
    col: 2, step: 14,
  },
  {
    id: 'ministry-push',
    branch: 'ministry',
    type: 'commit',
    gitLabel: 'git push (PR)',
    lawLabel: 'Voorstel aangeboden',
    subtitle: 'Aangeboden aan Wetgevingskalender',
    description: 'Het voorstel wordt aangeboden aan de Wetgevingskalender.',
    col: 2, step: 15,
  },
  // Ministry fork merges into develop
  {
    id: 'develop-receive',
    branch: 'develop',
    type: 'merge',
    gitLabel: 'merge fork → develop',
    lawLabel: 'Voorstel in procedure',
    subtitle: 'Opgenomen in kalender',
    description: 'Het voorstel is opgenomen in de Wetgevingskalender.',
    col: 1, step: 16,
  },

  // === PHASE E: Raad van State (advisory fork) ===
  {
    id: 'rvs-fork',
    branch: 'rvs-branch',
    type: 'branch',
    gitLabel: 'fork → senior review',
    lawLabel: 'Adviesaanvraag Raad van State',
    subtitle: 'Via Koninklijke Boodschap',
    description: 'Het voorstel wordt via de Koning aan de Raad van State aangeboden.',
    col: 4, step: 17,
  },
  {
    id: 'rvs-toets',
    branch: 'rvs-branch',
    type: 'commit',
    gitLabel: 'commit (toetsing)',
    lawLabel: 'Constitutionele toets',
    subtitle: 'Grondwet, EU-recht, consistentie',
    description: 'De RvS toetst op grondwettelijkheid, juridische kwaliteit en wetgevingstechniek.',
    col: 4, step: 18,
  },
  {
    id: 'rvs-advies',
    branch: 'rvs-branch',
    type: 'commit',
    gitLabel: 'commit (advies + dictum)',
    lawLabel: 'RvS advies',
    subtitle: 'Formeel advies met dictum',
    description: 'De Raad van State levert een formeel advies met dictum.',
    col: 4, step: 19,
  },
  {
    id: 'nader-rapport',
    branch: 'rvs-branch',
    type: 'commit',
    gitLabel: 'commit (nader rapport)',
    lawLabel: 'Nader rapport',
    subtitle: 'Kabinetsreactie op advies',
    description: 'De regering reageert op het advies en past het voorstel aan.',
    col: 4, step: 20,
  },
  {
    id: 'rvs-merge',
    branch: 'develop',
    type: 'merge',
    gitLabel: 'merge rvs → develop',
    lawLabel: 'Advies verwerkt',
    subtitle: 'Wijzigingen doorgevoerd',
    description: 'Het RvS-advies is verwerkt. De fork wordt gemerged terug naar develop.',
    col: 1, step: 21,
  },

  // === Rebase moment ===
  {
    id: 'rebase',
    branch: 'develop',
    type: 'commit',
    gitLabel: 'git rebase main',
    lawLabel: 'Actualisering wettekst',
    subtitle: 'Verwerking tussentijdse wetswijzigingen',
    description:
      'Tijdens de jaren van voorbereiding is het Corpus Juris veranderd. ' +
      'Verwijzingen worden bijgewerkt, samenloopbepalingen toegevoegd.',
    col: 1, step: 22,
  },

  // === Koninklijke Boodschap - submission to the TK ===
  {
    id: 'koninklijke-boodschap',
    branch: 'develop',
    type: 'commit',
    gitLabel: 'open PR',
    lawLabel: 'Koninklijke Boodschap',
    subtitle: 'Indiening bij Tweede Kamer',
    description: 'Het wetsvoorstel wordt via een Koninklijke Boodschap ingediend bij de Tweede Kamer.',
    col: 1, step: 23,
  },

  // === Initiatiefwetsvoorstel (external contributor, parallel) ===
  {
    id: 'initiatief-voorstel',
    branch: 'initiatief-branch',
    type: 'branch',
    gitLabel: 'external contributor PR',
    lawLabel: 'Initiatiefwetsvoorstel',
    subtitle: 'Voorstel vanuit de Kamer',
    description:
      'Elk Tweede Kamerlid kan zelf een wetsvoorstel indienen - zonder regering. ' +
      'Vergelijkbaar met een external contributor die een PR opent.',
    col: 3, step: 23,
  },
  {
    id: 'initiatief-rvs',
    branch: 'initiatief-branch',
    type: 'commit',
    gitLabel: 'CI (via TK)',
    lawLabel: 'RvS advies (via Kamer)',
    subtitle: 'TK vraagt advies',
    description: 'Bij een initiatiefwetsvoorstel vraagt de Tweede Kamer advies aan de Raad van State.',
    col: 3, step: 24,
  },
  {
    id: 'initiatief-merge',
    branch: 'initiatief-branch',
    type: 'commit',
    gitLabel: 'merge into PR',
    lawLabel: 'Behandeling als regulier',
    subtitle: 'Zelfde procedure vanaf hier',
    description: 'Na het RvS advies volgt dezelfde parlementaire procedure.',
    col: 3, step: 25,
  },

  // === PHASE F: Tweede Kamer ===
  {
    id: 'tk-verslag',
    branch: 'develop',
    type: 'commit',
    gitLabel: 'PR review comments',
    lawLabel: 'Verslag',
    subtitle: 'Schriftelijke vragen commissie',
    description: 'De vaste Kamercommissie stelt schriftelijke vragen. Kan meerdere rondes duren.',
    col: 1, step: 26,
  },
  {
    id: 'tk-nota-nav',
    branch: 'develop',
    type: 'commit',
    gitLabel: 'respond to review',
    lawLabel: 'Nota n.a.v. verslag',
    subtitle: 'Beantwoording vragen',
    description: 'De regering beantwoordt alle vragen uit het verslag.',
    col: 1, step: 27,
  },
  {
    id: 'nvw-1',
    branch: 'develop',
    type: 'commit',
    gitLabel: 'push commits',
    lawLabel: 'Nota van wijziging',
    subtitle: 'Regering wijzigt eigen voorstel',
    description: 'De regering kan het eigen voorstel wijzigen op basis van politieke feedback.',
    col: 1, step: 28,
  },
  {
    id: 'wetgevingsoverleg',
    branch: 'develop',
    type: 'commit',
    gitLabel: 'line-by-line review',
    lawLabel: 'Wetgevingsoverleg',
    subtitle: 'Artikelsgewijs in commissie',
    description: 'Formele commissievergadering: artikelsgewijze bespreking.',
    col: 1, step: 29,
  },
  {
    id: 'plenair-debat',
    branch: 'develop',
    type: 'commit',
    gitLabel: 'PR discussion',
    lawLabel: 'Plenair debat',
    subtitle: 'Voltallige Kamer',
    description: 'Plenair debat: woordvoerders spreken, minister reageert. Amendementen worden ingediend.',
    col: 1, step: 30,
  },

  // === Amendments (each its own branch, parallel) ===
  {
    id: 'amendement-a',
    branch: 'amend-a-branch',
    type: 'commit',
    gitLabel: 'patch (Van der Berg)',
    lawLabel: 'Amendement A',
    subtitle: 'Art. 3 lid 2 wijzigen - aangenomen',
    description:
      'Elk Kamerlid kan amendementen indienen - reviewer-submitted patches. ' +
      'Dit amendement wijzigt artikel 3 lid 2.',
    col: 3, step: 31,
  },
  {
    id: 'amendement-b',
    branch: 'amend-b-branch',
    type: 'commit',
    gitLabel: 'patch (Jansen) - overgenomen',
    lawLabel: 'Amendement B',
    subtitle: 'Nieuw art. 5a - cherry-picked',
    description:
      'De minister neemt dit amendement over (cherry-pick). ' +
      'Het wordt direct onderdeel van het voorstel - geen stemming nodig.',
    col: 4, step: 31,
  },
  {
    id: 'amendement-c',
    branch: 'amend-c-branch',
    type: 'commit',
    gitLabel: 'patch (De Vries) - conflicterend',
    lawLabel: 'Amendement C',
    subtitle: 'Conflicteert met A - aangenomen',
    description:
      'Dit amendement wijzigt hetzelfde artikellid als A - een merge conflict. ' +
      'Bureau Wetgeving bepaalt de stemvolgorde: verste strekking eerst.',
    col: 5, step: 31,
  },
  {
    id: 'subamendement',
    branch: 'sub-amend-branch',
    type: 'commit',
    gitLabel: 'patch/sub (op A)',
    lawLabel: 'Subamendement op A',
    subtitle: 'Max 1 niveau diep - eerst gestemd',
    description:
      'Een subamendement: wijziging op het amendement. Maximaal één niveau diep. ' +
      'Wordt altijd eerst in stemming gebracht.',
    col: 6, step: 31,
  },
  {
    id: 'amendement-rejected',
    branch: 'rejected-branch',
    type: 'commit',
    gitLabel: 'patch (Smit) - verworpen',
    lawLabel: 'Amendement verworpen',
    subtitle: 'Dead end - geen merge',
    description: 'Dit amendement wordt verworpen bij stemming. De branch stopt hier.',
    col: 7, step: 31,
  },

  // === Voting list + vote ===
  {
    id: 'stemmingslijst',
    branch: 'develop',
    type: 'commit',
    gitLabel: 'CI: conflict detection',
    lawLabel: 'Stemmingslijst',
    subtitle: 'Bureau Wetgeving ordent',
    description:
      'Bureau Wetgeving stelt de stemmingslijst op: subamendementen eerst, ' +
      'dan amendementen (verste strekking eerst). De merge queue.',
    col: 1, step: 32,
  },
  {
    id: 'stemmingen',
    branch: 'develop',
    type: 'merge',
    gitLabel: 'merge queue - stemming',
    lawLabel: 'Stemmingen Tweede Kamer',
    subtitle: 'Per artikel, dan geheel',
    description:
      'Stemming: subamendementen eerst, dan amendementen, per artikel, dan het hele voorstel. ' +
      'Gewone meerderheid vereist.',
    col: 1, step: 33,
  },

  // === PHASE G: Eerste Kamer ===
  {
    id: 'ek-behandeling',
    branch: 'develop',
    type: 'commit',
    gitLabel: 'final reviewer',
    lawLabel: 'Eerste Kamer behandeling',
    subtitle: 'Voorlopig verslag + debat',
    description:
      'Schriftelijke voorbereiding gevolgd door plenair debat. ' +
      'De Eerste Kamer heeft GEEN recht van amendement.',
    col: 1, step: 34,
  },
  {
    id: 'ek-toezegging',
    branch: 'develop',
    type: 'commit',
    gitLabel: 'create issue (follow-up)',
    lawLabel: 'Toezegging minister',
    subtitle: 'Belofte voor later',
    description:
      'In plaats van te amenderen, vraagt de EK toezeggingen: ' +
      'beloftes om bezwaren later op te lossen. Follow-up issues.',
    col: 1, step: 35,
  },
  {
    id: 'ek-stemming',
    branch: 'develop',
    type: 'commit',
    gitLabel: 'approve / reject',
    lawLabel: 'Stemming Eerste Kamer',
    subtitle: 'Aannemen of verwerpen',
    description: 'Aannemen of verwerpen - zonder wijziging. Protected branch: alleen approve/reject.',
    col: 1, step: 36,
  },

  // === Novelle (loops back, parallel with EK) ===
  {
    id: 'novelle-start',
    branch: 'novelle-branch',
    type: 'branch',
    gitLabel: 'dependent fix-PR',
    lawLabel: 'Novelle',
    subtitle: 'Reparatie-fork terug via TK',
    description:
      'Als de EK bezwaren heeft: een novelle. Een apart wetsvoorstel dat het origineel repareert. ' +
      'Doorloopt het volledige TK-traject opnieuw. Beide worden gelijktijdig aangenomen.',
    col: 3, step: 36,
  },
  {
    id: 'novelle-tk',
    branch: 'novelle-branch',
    type: 'commit',
    gitLabel: 'PR review (TK opnieuw)',
    lawLabel: 'Novelle door Tweede Kamer',
    subtitle: 'Volledige behandeling',
    description: 'De novelle doorloopt het volledige Tweede Kamer-traject.',
    col: 3, step: 37,
  },
  {
    id: 'novelle-merge',
    branch: 'novelle-branch',
    type: 'commit',
    gitLabel: 'fix-PR merged',
    lawLabel: 'Novelle aangenomen TK',
    subtitle: 'Terug naar EK',
    description: 'De novelle is aangenomen door de TK en gaat terug naar de EK.',
    col: 3, step: 38,
  },

  // EK votes on both
  {
    id: 'ek-final',
    branch: 'develop',
    type: 'merge',
    gitLabel: 'approve (beide)',
    lawLabel: 'EK stemt over beide',
    subtitle: 'Wet + novelle gelijktijdig',
    description: 'De Eerste Kamer stemt gelijktijdig over het wetsvoorstel en de novelle.',
    col: 1, step: 39,
  },

  // === PHASE H: The King merges ===
  {
    id: 'koninklijk-besluit',
    branch: 'main',
    type: 'merge',
    gitLabel: 'merge develop → main',
    lawLabel: 'Koninklijk Besluit',
    subtitle: 'Bekrachtiging door de Koning',
    description:
      'De Koning is de enige maintainer van het Corpus Juris - alleen hij kan mergen ' +
      'naar main, met de Minister als co-author op elke commit.',
    tags: [
      { label: 'Staatsblad', color: 'var(--color-branch-advisory)' },
      { label: 'Inwerkingtreding', color: 'var(--color-branch-main)' },
    ],
    col: 0, step: 40,
  },
  {
    id: 'corpus-updated',
    branch: 'main',
    type: 'commit',
    gitLabel: 'HEAD',
    lawLabel: 'Corpus Juris',
    subtitle: 'Bijgewerkt',
    description: 'Het Corpus Juris is bijgewerkt. Main gaat door.',
    col: 0, step: 41,
  },
];

export const connections: Connection[] = [
  // main → develop
  { from: 'corpus-start', to: 'develop-start', type: 'branch-off' },

  // develop → ministry fork
  { from: 'develop-start', to: 'ministry-fork', type: 'branch-off' },

  // Phase A: ministry internal
  { from: 'ministry-fork', to: 'intern-start', type: 'branch-off' },
  { from: 'intern-start', to: 'intern-mvt', type: 'straight' },
  { from: 'intern-mvt', to: 'intern-toets', type: 'straight' },
  { from: 'intern-toets', to: 'intern-signoff', type: 'straight' },
  { from: 'intern-signoff', to: 'ministry-merge-intern', type: 'merge-in' },

  // Phase B: interdepartmental + parallel check forks
  { from: 'ministry-merge-intern', to: 'interdept', type: 'straight' },
  { from: 'interdept', to: 'uh-toets', type: 'branch-off' },
  { from: 'interdept', to: 'regeldruk', type: 'branch-off' },
  { from: 'interdept', to: 'financieel', type: 'branch-off' },
  { from: 'interdept', to: 'privacy-toets', type: 'branch-off' },
  // All checks merge back
  { from: 'uh-toets', to: 'voorportaal', type: 'merge-in' },
  { from: 'regeldruk', to: 'voorportaal', type: 'merge-in' },
  { from: 'financieel', to: 'voorportaal', type: 'merge-in' },
  { from: 'privacy-toets', to: 'voorportaal', type: 'merge-in' },

  // Phase C: external consultation forks
  { from: 'voorportaal', to: 'internetconsultatie', type: 'branch-off' },
  { from: 'voorportaal', to: 'adviesorganen', type: 'branch-off' },
  { from: 'internetconsultatie', to: 'verwerking-reacties', type: 'merge-in' },
  { from: 'adviesorganen', to: 'verwerking-reacties', type: 'merge-in' },

  // Phase D: cabinet
  { from: 'verwerking-reacties', to: 'onderraad', type: 'straight' },
  { from: 'onderraad', to: 'ministerraad', type: 'straight' },
  { from: 'ministerraad', to: 'ministry-push', type: 'straight' },

  // Ministry fork → develop
  { from: 'ministry-push', to: 'develop-receive', type: 'merge-in' },

  // Phase E: RvS advisory fork
  { from: 'develop-receive', to: 'rvs-fork', type: 'branch-off' },
  { from: 'rvs-fork', to: 'rvs-toets', type: 'straight' },
  { from: 'rvs-toets', to: 'rvs-advies', type: 'straight' },
  { from: 'rvs-advies', to: 'nader-rapport', type: 'straight' },
  { from: 'nader-rapport', to: 'rvs-merge', type: 'merge-in' },

  // Rebase + Koninklijke Boodschap
  { from: 'rvs-merge', to: 'rebase', type: 'straight' },
  { from: 'rebase', to: 'koninklijke-boodschap', type: 'straight' },

  // Initiatiefwet (parallel, external contributor)
  { from: 'koninklijke-boodschap', to: 'initiatief-voorstel', type: 'branch-off' },
  { from: 'initiatief-voorstel', to: 'initiatief-rvs', type: 'straight' },
  { from: 'initiatief-rvs', to: 'initiatief-merge', type: 'straight' },

  // Phase F: Tweede Kamer
  { from: 'koninklijke-boodschap', to: 'tk-verslag', type: 'straight' },
  { from: 'tk-verslag', to: 'tk-nota-nav', type: 'straight' },
  { from: 'tk-nota-nav', to: 'nvw-1', type: 'straight' },
  { from: 'nvw-1', to: 'wetgevingsoverleg', type: 'straight' },
  { from: 'wetgevingsoverleg', to: 'plenair-debat', type: 'straight' },

  // Amendments fan out from the plenary debate
  { from: 'plenair-debat', to: 'amendement-a', type: 'branch-off' },
  { from: 'plenair-debat', to: 'amendement-b', type: 'branch-off' },
  { from: 'plenair-debat', to: 'amendement-c', type: 'branch-off' },
  { from: 'plenair-debat', to: 'subamendement', type: 'branch-off' },
  { from: 'plenair-debat', to: 'amendement-rejected', type: 'branch-off' },
  // Accepted amendments merge back to stemmingen
  { from: 'amendement-a', to: 'stemmingen', type: 'merge-in' },
  { from: 'amendement-b', to: 'stemmingen', type: 'merge-in' },
  { from: 'amendement-c', to: 'stemmingen', type: 'merge-in' },
  { from: 'subamendement', to: 'stemmingen', type: 'merge-in' },
  // amendement-rejected: dead end

  { from: 'plenair-debat', to: 'stemmingslijst', type: 'straight' },
  { from: 'stemmingslijst', to: 'stemmingen', type: 'straight' },

  // Phase G: Eerste Kamer
  { from: 'stemmingen', to: 'ek-behandeling', type: 'straight' },
  { from: 'ek-behandeling', to: 'ek-toezegging', type: 'straight' },
  { from: 'ek-toezegging', to: 'ek-stemming', type: 'straight' },

  // Novelle (parallel fork from EK)
  { from: 'ek-stemming', to: 'novelle-start', type: 'branch-off' },
  { from: 'novelle-start', to: 'novelle-tk', type: 'straight' },
  { from: 'novelle-tk', to: 'novelle-merge', type: 'straight' },
  { from: 'novelle-merge', to: 'ek-final', type: 'merge-in' },

  { from: 'ek-stemming', to: 'ek-final', type: 'straight' },

  // develop merges into main (the King merges!)
  { from: 'ek-final', to: 'koninklijk-besluit', type: 'merge-in' },
  { from: 'koninklijk-besluit', to: 'corpus-updated', type: 'straight' },

  // main continues (dashed) while branches are active
  { from: 'corpus-start', to: 'koninklijk-besluit', type: 'main-continues' },
  // develop continues (dashed) while forks are active
  { from: 'develop-start', to: 'develop-receive', type: 'main-continues' },
  { from: 'develop-receive', to: 'rvs-merge', type: 'main-continues' },
];
