/*
 * The text of the plain-language page about the position paper "Rules as
 * Executed": /research/rules-as-executed/uitgelegd and /explained.
 *
 * Written for someone who has never heard of the paper, at language level B1:
 * short sentences, everyday words, one story from start to end. Dutch is the
 * source; the English is its translation. Edit both together (AGENTS.md treats
 * a key in one dataset only as a bug).
 *
 * The page is an argument, not a procedure, so its parts carry a title and no
 * number. It opens with the problem the paper starts from, then says why that
 * problem is a constitutional one, and only then comes to the proposal. One
 * example, two housemates applying for healthcare allowance, opens the problem
 * and returns in the proposal. The part on power uses the toeslagenaffaire
 * instead, because the constitutional point needs a case where it went wrong;
 * the other parts have an example of their own.
 *
 * Every claim here explains the paper as published on 31 August 2026 and has
 * to be traceable to it. The claim-by-claim mapping, with the sentence of the
 * paper that carries each one, is in paper-explained.claims.md next to this
 * file. A simpler sentence may leave detail out; it may not say more than the
 * paper does.
 */

export type Lang = 'nl' | 'en';

export interface Cell {
  /** 'yes' | 'part' | 'no', drawn as a tag. */
  v: 'yes' | 'part' | 'no';
  note?: string;
}

export interface ActorRow {
  actor: string;
  now: [Cell, Cell, Cell];
  proposal: [Cell, Cell, Cell];
}

export interface OverrideVariant {
  id: string;
  label: string;
  short: string;
  rows: [string, string][];
  note: string;
}

export interface Option {
  id: string;
  label: string;
  short: string;
  outcome: string;
  tag: string;
  tone: 'success' | 'critical';
}

export interface FlowStep {
  label: string;
  text: string;
}

/** The running example, picked up again in a later part. */
export interface CaseNote {
  label: string;
  text: string;
}

export interface Content {
  meta: { title: string; description: string };
  header: {
    title: string;
    subtitle: string;
    lede: string;
    version: (date: string) => string;
    veterans: string;
    tocLabel: string;
    pdf: string;
    paper: string;
    discuss: string;
  };
  readInPaper: string;
  more: string;
  close: string;
  thisPaper: string;
  tableLink: string;
  figureLink: string;
  yes: string;
  part: string;
  no: string;
  now: string;
  proposal: string;
  /** The same, short enough for a two-way switch on a 320 px screen. */
  proposalShort: string;

  /** The problem the paper starts from: the law is published, the rules the computer executes are not. */
  problem: {
    title: string;
    /** The problem in two sentences, set large, before anything else. */
    core: string;
    p1: string;
    caseLabel: string;
    caseIntro: string;
    options: Option[];
    caseNote: string;
    caseLink: { href: string; text: string };
    demandLabel: string;
    demand: string;
    more: string[];
  };
  /** The constitutional argument: the balance between the powers. */
  power: {
    title: string;
    intro: string[];
    case: CaseNote;
    demandsLabel: string;
    /** What the rule of law asks, and where digital execution falls short of it. */
    demands: [string, string][];
    p3: string;
    matrixLabel: string;
    rowHeader: string;
    columns: [string, string, string];
    more: string[];
  };
  /** The proposal: publish the rules, and be bound to them. */
  binding: {
    title: string;
    intro: string[];
    case: CaseNote;
    /** Why the binding asks for a format of its own. */
    formLabel: string;
    form: [string, string][];
    /** Encoding a law once: how the format is used, not a reason for it, so it sits apart from `form`. */
    reuse: string;
    /** Lead-in for the full encoding of the example, in the sheet. */
    codeLead: string;
    flowLabel: string;
    flowNow: FlowStep[];
    flowNowNote: string;
    flowProposal: FlowStep[];
    flowProposalNote: string;
    more: string[];
    exampleLabel: string;
    exampleIntro: string;
    exampleStatute: string;
    mapRows: [string, string, string][];
    exampleNote: string;
  };
  /** The receipt, computed in the browser. */
  receipt: {
    title: string;
    p1: string;
    p2: string;
    demoIntro: string;
    decision: string;
    fields: { law: string; date: string; inputs: string; amount: string; digest: string };
    inputsSummary: string;
    inForce: string;
    check: string;
    checking: string;
    checkDigest: string;
    checkAmount: string;
    ok: string;
    mismatch: string;
    loading: string;
    failed: string;
    noScript: string;
    revealA: string;
    revealB: (published: string, local: string, diff: string) => string;
    limitsLabel: string;
    limits: string[];
    more: string[];
  };
  /** Back in time: which rules applied on a given date. */
  time: {
    title: string;
    intro: string[];
    caseLabel: string;
    caseIntro: string;
    /** The version of the rulework, and the percentage it carries. */
    caseRows: [string, string][];
    caseNote: string;
    p3: string;
    more: string[];
  };
  /** A person may depart from the outcome. */
  default: {
    title: string;
    p1: string;
    variantsLabel: string;
    variants: OverrideVariant[];
    more: string[];
  };
  /** What this does not fix. */
  limits: {
    title: string;
    intro: string[];
    /** What the proposal writes out: where there is room, where not, and that it was used. */
    explicit: [string, string][];
    explicitNote: string;
    /** The rigidity that was a choice, with the paper's own examples. */
    avoidable: string;
    p2: string;
    openLabel: string;
    open: string[];
    more: string[];
  };
  /** For people who have worked on rules as code for years. */
  veterans: {
    title: string;
    p1: string;
    p2: string;
    tableLabel: string;
    rowHeader: string;
    columns: [string, string, string];
    /** What each column of the comparison means, with the paper's own term. */
    columnHelp: [string, string, string];
    /** Under the table: why only this paper has three yeses. */
    tableNote: string;
    p3: string;
    more: string[];
    reasons: string[];
    moreAfter: string[];
  };
}

const nl: Content = {
  meta: {
    title: 'Rules as Executed, uitgelegd · RegelRecht',
    description:
      'Het paper Rules as Executed in gewone taal: waarom het de rechtsstaat raakt dat buiten de uitvoering niemand kan zien welke regels een computer uitvoert, en wat het paper voorstelt.',
  },
  header: {
    title: 'Rules as Executed, uitgelegd',
    subtitle: 'Het paper in gewone taal',
    lede:
      'Steeds vaker rekent een computer uit wat je krijgt of moet betalen, bijvoorbeeld bij de zorgtoeslag. De wet waar dat op rust, kan iedereen lezen. Maar de computer werkt niet met de wettekst. Hij werkt met regels die de uitvoerende organisatie uit de wet heeft afgeleid, en die zijn niet gepubliceerd. Alleen die organisatie kan zien welke regels de computer uitvoert. Het parlement, de rechter en jij kunnen dat niet. Daar begint het paper Rules as Executed. Deze pagina legt uit waarom dat de rechtsstaat raakt, en wat het paper voorstelt.',
    version: (date) =>
      `Dit is een uitleg bij het paper zoals het op ${date} verscheen. Wil je het precies weten, lees dan het paper zelf.`,
    veterans: 'Werk je al jaren met regels als code? Lees dan wat hier nieuw is, en waarom dat een eigen vorm vraagt.',
    tocLabel: 'Op deze pagina',
    pdf: 'Pdf op Zenodo',
    paper: 'Lees het paper',
    discuss: 'Praat mee over het paper',
  },
  readInPaper: 'In het paper',
  more: 'Meer uit het paper',
  close: 'Sluiten',
  thisPaper: 'Dit paper',
  tableLink: 'Dit overzicht in het paper',
  figureLink: 'Dit voorbeeld in het paper',
  yes: 'ja',
  part: 'deels',
  no: 'nee',
  now: 'Nu',
  proposal: 'Met het voorstel',
  proposalShort: 'Met voorstel',

  problem: {
    title: 'Het probleem: de regels die de computer uitvoert, zijn niet openbaar',
    core:
      'De wet is gepubliceerd. De regels die de computer uitvoert, zijn dat niet. Daardoor kan niemand buiten de uitvoerende organisatie controleren of een besluit klopt met de wet.',
    p1:
      'Een wet zegt wie ergens recht op heeft. Een computer heeft meer nodig. Hij moet weten welke gegevens hij gebruikt, hoe hij een begrip uit de wet invult en wat hij doet als een geval net anders ligt. Die keuzes maakt de organisatie die de wet uitvoert. Sommige keuzes schrijft ze op in regels die je kunt opzoeken. Veel keuzes liggen alleen vast in werkinstructies voor ambtenaren en in de software zelf.',
    caseLabel: 'Een voorbeeld: wie is je toeslagpartner?',
    caseIntro:
      'Je huurt samen met een vriend een huis. Jullie staan al twee jaar op hetzelfde adres ingeschreven, maar jullie zijn geen stel. Je vraagt zorgtoeslag aan. Telt je huisgenoot als je toeslagpartner?',
    options: [
      {
        id: 'adres',
        label: 'Het systeem kijkt alleen naar het adres',
        short: 'Adres',
        outcome:
          'Jullie staan langer dan zes maanden op hetzelfde adres. Het systeem ziet je huisgenoot als toeslagpartner en telt het inkomen van je huisgenoot mee. Je krijgt minder of geen zorgtoeslag.',
        tag: 'minder of geen toeslag',
        tone: 'critical',
      },
      {
        id: 'huishouden',
        label: 'Het systeem kijkt ook naar het huishouden',
        short: 'Huishouden',
        outcome:
          'Jullie wonen op hetzelfde adres, maar voeren geen gezamenlijke huishouding. Je huisgenoot is dan geen toeslagpartner. Je krijgt zorgtoeslag op je eigen inkomen.',
        tag: 'toeslag op eigen inkomen',
        tone: 'success',
      },
    ],
    caseNote:
      'De wet zegt onder meer dat een huisgenoot je partner is als jullie langer dan zes maanden op hetzelfde adres staan en een gezamenlijke huishouding voeren. Het adres staat in een registratie, het huishouden niet. Hoe een systeem dat tweede deel invult, bepaalt of je toeslag krijgt, en dat lees je niet in de wet. Het paper noemt juist deze afleiding van een partner uit registraties als voorbeeld. De twee systemen hierboven zijn verzonnen. Hoe het echte systeem het doet, kun je nu niet nagaan, en dat is precies het probleem. Dit voorbeeld komt verderop op deze pagina terug.',
    caseLink: { href: 'https://wetten.overheid.nl/BWBR0018472/2025-01-01#Artikel3', text: 'Artikel 3 van de Algemene wet inkomensafhankelijke regelingen' },
    demandLabel: 'Wat het paper daarom vraagt',
    demand:
      'De wet zoals de computer hem uitvoert, moet net zo openbaar zijn als de wet zoals de wetgever hem vaststelde, en net zo goed te controleren. Je moet kunnen controleren dat je besluit met precies die regels is uitgerekend.',
    more: [
      'Een besluit hangt vaak niet alleen af van de wet zelf. Het hangt ook af van hoe de organisatie die wet uitlegt. Een deel van die uitleg staat in beleidsregels, en die moeten openbaar zijn. Een groot deel staat nergens dan in de organisatie zelf.',
      'Het speelt niet bij één organisatie, en ook niet alleen in Nederland. In Australië beschuldigde een systeem dat inkomens middelde honderdduizenden mensen met een uitkering ten onrechte van een schuld (Robodebt). In het Verenigd Koninkrijk leidde een gebrekkig computersysteem ertoe dat zo’n duizend postkantoorhouders ten onrechte werden vervolgd (het Horizon-schandaal). Het paper noemt ze naast de toeslagenaffaire, als voorbeelden van hetzelfde patroon: systemen met grote gevolgen voor mensen, die van buiten niet te controleren zijn.',
    ],
  },

  power: {
    title: 'Waarom dit over macht gaat',
    intro: [
      'In Nederland is de macht verdeeld, maar niet strikt gescheiden. Wetten maken, wetten uitvoeren en rechtspreken zijn verdeeld over instellingen die elkaar controleren en begrenzen. Zo houden ze elkaar in evenwicht. De uitvoerende macht is daarin al lang de sterkste.',
      'Sinds de uitvoering in software zit, is dat evenwicht verder scheefgegroeid. Alleen de uitvoerende macht kan nog zien hoe de wet echt wordt toegepast. Het paper noemt dat een monopolie. Het parlement stemt over de wet, maar ziet niet hoe die in de praktijk uitpakt. De rechter krijgt alleen de uitkomst te zien. Wie het besluit krijgt, kan niet nagaan of het klopt. En ook de uitvoerende organisatie overziet het niet altijd: software groeit jaren door, en soms is ze van een leverancier en mag de overheid niet in de code kijken.',
    ],
    case: {
      label: 'Wat de toeslagenaffaire liet zien',
      text:
        'De parlementaire enquête stelde vast dat alle drie de machten ‘blind voor mens en recht’ waren geweest. Dat de uitvoering niet te zien was, is volgens het paper een deel van wat er misging. De Tweede Kamer kon haar eigen wetten niet zien in de vorm waarin ze werden uitgevoerd. Kamerleden moesten hun recht op inlichtingen gebruiken om losse dossiers uit te zoeken, omdat de regels zelf niet in te zien waren. De rechter beoordeelde besluiten zonder de regels te kunnen inzien waarmee ze waren uitgerekend. En de uitvoering had vaak zelf geen overzicht over haar eigen systemen.',
    },
    demandsLabel: 'Wat de rechtsstaat vraagt',
    demands: [
      [
        'Een wet moet bekend zijn gemaakt',
        'Volgens de Grondwet geldt een wet pas nadat hij is bekendgemaakt (artikel 88). Voor regels die de regering zelf maakt, geldt hetzelfde (artikel 89). Alleen de tekst publiceren is volgens het paper niet genoeg, als onduidelijk blijft hoe de wet wordt uitgevoerd.',
      ],
      [
        'Een minister legt verantwoording af',
        'Ministers zijn verantwoordelijk voor alles wat het bestuur doet, en moeten de Kamer de inlichtingen geven waar Kamerleden om vragen (artikel 42 en 68 van de Grondwet). Dat lukt niet voor een regel die niemand kan lezen.',
      ],
      [
        'Je moet kunnen voorspellen wat de overheid doet',
        'Dat heet rechtszekerheid. Nu kun je vaak alleen een aanvraag doen en afwachten wat de organisatie beslist die als enige de regels kent. Vooraf uitrekenen of achteraf controleren kan niet.',
      ],
      [
        'En nu ook: je moet het kunnen controleren',
        'De rechtsstaat vraagt van oudsher dat je regels kunt kennen, begrijpen en voorspellen. Daarbij ging men ervan uit dat je kon nagaan hoe een ambtenaar redeneerde. Nu computers de wet uitvoeren, is dat niet meer genoeg. Daarom komt er volgens het paper een eis bij: het moet onafhankelijk te controleren zijn welke regels echt worden uitgevoerd.',
      ],
    ],
    p3:
      'Het voorstel moet het evenwicht herstellen. Het geeft elke macht wat ze nodig heeft om haar eigen taak te doen. Dat werkt alleen als de andere machten ook echt met de gepubliceerde regels aan de slag kunnen.',
    matrixLabel: 'Wie kan wat?',
    rowHeader: 'Wie',
    columns: ['Leest de regels', 'Rekent één besluit na', 'Rekent veel gevallen door'],
    more: [
      'Het paper haalt een oproep uit de rechtswetenschap aan: ‘trias politica by design’. De scheiding van machten moet in de systemen van de overheid zelf worden ingebouwd, en niet achteraf met extra toezicht worden gerepareerd.',
      'De rechter heeft die geslotenheid niet aanvaard. Een van de hoogste bestuursrechters, de Afdeling bestuursrechtspraak van de Raad van State, oordeelde dat de overheid de keuzes, gegevens en aannames in een rekenmodel volledig openbaar moet maken als een besluit erop rust. Met een gepubliceerd regelwerk kan de rechter de uitkomst zelf narekenen. De rechter houdt het laatste woord over wat de wet betekent, en is nooit aan het regelwerk gebonden.',
      'Met het voorstel kun je je eigen besluit narekenen, maar niet dat van je buren. Dat is met opzet. Als iedereen elk besluit kon narekenen, zou je openheid kopen door privacy op te geven, schrijft het paper.',
      'De Tweede Kamer heeft geen gegevens van mensen nodig. Ze krijgt de regels zelf, en kan zien wat een wetswijziging zou doen voordat ze erover stemt. Kamerleden hoeven die regels niet zelf te lezen. Het paper denkt aan de ondersteuning van de Kamer, die dat voor alle fracties kan doen. In Frankrijk rekent het team LexImpact van de Assemblée nationale op die manier voorstellen voor Kamerleden door.',
      'Ook wie een wet voorbereidt, kan eerder zien wat die doet. Een beleidsmaker kan een nieuwe regel doorrekenen voordat die wordt ingediend, en zien of dezelfde uitkomst ook met minder gegevens over mensen te bereiken is. Een wetgevingsjurist kan de rekenvoorbeelden uit de memorie van toelichting als toets gebruiken: het regelwerk geeft dezelfde uitkomst, of niet. Dat werkt alleen als die voorbeelden worden bijgehouden, want na een wetswijziging of indexering kloppen ze vaak niet meer.',
      'De overheid heeft nu andere controles. Het Algoritmeregister noemt systemen, maar laat de regels zelf zelden zien. Commissies en toetsen zoals een DPIA beoordelen vooral beschrijvingen, gemaakt door wie de systemen bouwde. Met gepubliceerde regels kunnen ze de regels zelf beoordelen.',
      'De twee schrijvers van het paper zijn ingenieurs en geen juristen. Dat zeggen ze er zelf bij, en ze vragen juristen om te verbeteren wat niet klopt.',
    ],
  },

  binding: {
    title: 'Het voorstel: publiceer de regels, en beslis ermee',
    intro: [
      'Het paper stelt twee dingen voor, die bij elkaar horen. Het eerste: publiceer de regels die de computer uitvoert, zodat iedereen ze kan lezen, net als de wet. Het tweede: de organisatie moet beslissen met precies die gepubliceerde versie, en met geen andere. De organisatie is dan gebonden aan wat ze publiceert. De regels die je kunt lezen, zijn de regels waarmee je besluit is uitgerekend.',
      'Dat tweede deel is de kern. Regels zo opschrijven dat een computer ermee kan rekenen, kan al tientallen jaren. Regels publiceren gebeurt ook al. Maar tot nu toe garandeert niets dat de gepubliceerde regel ook de regel is waarmee het besluit wordt uitgerekend. Dat gat wil het paper dichten.',
      'Zo’n gepubliceerde regeling heet een regelwerk (het paper zelf spreekt nog van een executable specification). Een regelwerk is geen model dat naast de software wordt bijgehouden. Het is wat de computer uitvoert.',
      'Dat stelt eisen aan de vorm waarin de regels zijn opgeschreven. Daarom komt het paper met een eigen vorm, en gebruikt het geen bestaande taal voor regels.',
    ],
    case: {
      label: 'Het voorbeeld van de huisgenoot',
      text:
        'In het regelwerk staat dan hoe de organisatie ‘een gezamenlijke huishouding’ invult. Kijkt het systeem alleen naar het adres, dan kan iedereen dat lezen: de Kamer, de rechter, je adviseur en jijzelf. Vind je dat dit niet klopt met de wet, dan weet je waartegen je bezwaar maakt.',
    },
    formLabel: 'Waarom een eigen vorm',
    form: [
      [
        'Het regelwerk volgt de wet, artikel voor artikel',
        'Het beschrijft de wet in artikelen, voorwaarden en verwijzingen, zoals de wetgever hem schreef. Elk artikel krijgt zijn eigen stuk regelwerk. Het is de bedoeling dat een jurist zo per artikel kan nagaan of het klopt met de wet. Of dat zonder hulp lukt, moet volgens het paper nog blijken. Dat is ook het verschil met de broncode van een systeem publiceren. Daarin zitten de keuzes verstopt tussen techniek die met de wet niets te maken heeft, en wetgevers en rechters kunnen die code niet lezen.',
      ],
      [
        'Elke berekening geeft een antwoord',
        'De vorm kan rekenen, vergelijken en datums toetsen, en veel meer niet. Daardoor stopt elke berekening. Narekenen geeft dus altijd een uitkomst. Dat moet ook, wil narekenen een recht zijn.',
      ],
      [
        'Geen vertaalstap tussen publicatie en uitvoering',
        'Wat gepubliceerd is, is precies wat de computer uitvoert. Bij regels in een taal die op gewone taal lijkt, zoals RegelSpraak, is dat anders. Die moeten eerst door een programma worden omgezet, en in dat programma kunnen de gepubliceerde en de uitgevoerde regel weer uit elkaar lopen.',
      ],
    ],
    reuse:
      'Zo kan een wet die bijna overal geldt, zoals de Algemene wet bestuursrecht, één keer worden opgeschreven. Elke organisatie kan dan hetzelfde regelwerk gebruiken, in plaats van de regels zelf opnieuw te bouwen. Verwijst een wet naar een andere wet, dan staat die verwijzing in het regelwerk, en wordt de versie gebruikt die op de datum gold.',
    codeLead: 'Zo staat het parkeerartikel in de vorm uit het paper:',
    flowLabel: 'Van wet naar besluit',
    flowNow: [
      { label: 'De wet', text: 'Iedereen kan hem lezen' },
      { label: 'Beleidsregels', text: 'Deels te vinden' },
      { label: 'Werkinstructies', text: 'Niet te zien' },
      { label: 'De software', text: 'Niet te zien' },
      { label: 'Het besluit', text: 'Je ziet niet met welke regels is gerekend' },
    ],
    flowNowNote:
      'Elke laag is een vertaling van de vorige, en mensen houden ze met de hand gelijk. Niets garandeert dat de laatste nog klopt met de eerste.',
    flowProposal: [
      { label: 'De wet', text: 'Iedereen kan hem lezen' },
      { label: 'Het regelwerk', text: 'Gepubliceerd, en de enige versie die de computer gebruikt' },
      { label: 'Het besluit', text: 'Met een bonnetje erbij' },
    ],
    flowProposalNote: 'De keuzes die het besluit bepalen, staan nu op één plek, en iedereen kan ze lezen.',
    more: [
      'Wie de vorm niet zelf wil lezen, kan hetzelfde regelwerk bekijken in een scherm dat de verwijzingen aanklikbaar maakt en het met testgevallen laat rekenen.',
      'Het paper kiest ook niet voor linked data, een bekende manier om wetten machineleesbaar te maken. Die aanpak zoekt één algemeen schema van begrippen voor alle wetten. Volgens het paper is dat voor wetten over zorg, belasting en vreemdelingen tegelijk niet haalbaar. Het houdt het bij één wet per regelwerk, met verwijzingen ertussen.',
      'Een regelwerk is een uitleg van de wet, niet de wet zelf. Als het botst met de wet, gaat de wet voor en moet het regelwerk worden aangepast.',
      'De uitleg blijft het werk van de organisatie die de wet uitvoert. Die keuzes werden al gemaakt, schrijft het paper, alleen zag niemand buiten de organisatie ze. Na publicatie zijn ze zichtbaar, en kun je iemand erop aanspreken.',
      'Een regelwerk dat wel is gepubliceerd maar niet wordt gebruikt bij het beslissen, is volgens het paper geen oplossing. Het laat een uitleg zien waarmee geen enkel besluit wordt genomen. Is er voor een wet nog geen regelwerk, dan kan de uitvoering niet wachten. De organisatie publiceert dan in beleidsregels hoe ze de wet toepast.',
    ],
    exampleLabel: 'Hoe een regelwerk eruitziet',
    exampleIntro: 'Naar een verzonnen voorbeeld uit het paper: een artikel uit een gemeentelijke parkeerregeling.',
    exampleStatute:
      'Een bewoner komt in aanmerking voor een parkeervergunning als zijn voertuig weinig uitstoot. Voor een emissievrij voertuig geldt een lager tarief.',
    mapRows: [
      ['Een bewoner', 'is_resident', 'uit de basisregistratie'],
      ['als zijn voertuig weinig uitstoot', 'is_low_emission', 'uit het kentekenregister'],
      ['komt in aanmerking voor een parkeervergunning', 'eligible = AND(is_resident, is_low_emission)', 'dit artikel rekent het uit'],
      [
        'Voor een emissievrij voertuig geldt een lager tarief',
        'permit_fee = IF is_zero_emission THEN reduced_fee ELSE standard_fee',
        'uit de tarieventabel',
      ],
    ],
    exampleNote: 'Elk stukje van het artikel krijgt zijn eigen regel, en bij elke waarde staat waar die vandaan komt.',
  },

  receipt: {
    title: 'Het bonnetje: zo controleer je het',
    p1:
      'Dat een organisatie gebonden is aan wat ze publiceert, heeft pas waarde als iemand anders dat kan controleren. Daarom hoort er met het voorstel bij elk besluit een bonnetje. Daarop staat welke versie van de regels de organisatie zegt te hebben gebruikt, met welke gegevens ze rekende en wat eruit kwam. De versie staat er als een soort vingerafdruk op: een code die verandert zodra er ook maar iets in de regels anders is.',
    p2:
      'Met dat bonnetje kun je het besluit zelf narekenen. Je neemt de gepubliceerde regels, stopt dezelfde gegevens erin en kijkt of hetzelfde bedrag eruit komt. Dezelfde regels met dezelfde gegevens geven altijd dezelfde uitkomst.',
    demoIntro:
      'Probeer het maar. Hieronder staan twee besluiten over de zorgtoeslag van dezelfde persoon, met testgegevens. Je browser heeft ze net uitgerekend. Klik bij allebei op Reken na.',
    decision: 'Besluit',
    fields: {
      law: 'Regels',
      date: 'Rekendatum',
      inputs: 'Gegevens',
      amount: 'Zorgtoeslag volgens het besluit',
      digest: 'Vingerafdruk van de regels',
    },
    inputsSummary: '{n} gegevens uit de registraties, dezelfde voor beide besluiten',
    inForce: 'geldig vanaf {date}',
    check: 'Reken na',
    checking: 'Bezig met narekenen…',
    checkDigest: 'Dezelfde regels als gepubliceerd',
    checkAmount: 'Hetzelfde bedrag als bij narekenen',
    ok: 'klopt',
    mismatch: 'klopt niet',
    loading: 'De regels worden opgehaald…',
    failed: 'Narekenen lukte niet. Laad de pagina opnieuw.',
    noScript: 'Hiervoor is JavaScript nodig: het narekenen gebeurt in je eigen browser.',
    revealA:
      'Beide controles kloppen. De regels op het bonnetje zijn de gepubliceerde, en narekenen geeft hetzelfde bedrag.',
    revealB: (published, local, diff) =>
      `Dit klopt niet. Het systeem achter dit besluit gebruikte een eigen kopie van de regels. Daarin stond één percentage nog op de waarde van vorig jaar: ${local} in plaats van ${published}. Die kopie is nooit gepubliceerd. Het scheelt ${diff}, en je ziet het op twee manieren: de vingerafdruk is anders, en narekenen geeft een ander bedrag.`,
    limitsLabel: 'Wat het bonnetje niet laat zien',
    limits: [
      'Het bonnetje laat zien met welke gegevens is gerekend, maar bewijst niet dat die gegevens kloppen. Je kunt ze wel naast wat je zelf weet leggen, zoals je eigen inkomen. Daarnaast wil het paper elk gegeven, waar dat kan, koppelen aan de bron die erover gaat, zoals een basisregistratie.',
      'Of je nu al recht hebt op zo’n bonnetje, is nog onduidelijk. Het paper stelt voor dat je het altijd krijgt, samen met het besluit.',
    ],
    more: [
      'Beide besluiten zijn echt uitgerekend, in je browser, met de regels die deze site gebruikt. Voor besluit B past deze pagina zelf een kopie van de regels aan. Een echt bonnetje is ook ondertekend door de organisatie. Deze pagina ondertekent niets.',
      'Op een echt bonnetje staat ook welk programma de berekening deed. Rekent het programma waarmee jij narekent anders, dan kan er ook een verschil uitkomen. Hoe je zeker weet dat twee programma’s hetzelfde rekenen, is volgens het paper nog een open vraag.',
      'Publiceren en het bonnetje hebben pas gevolgen als ze in de wet staan. Het paper vindt dat een besluit zonder bonnetje dan zelf een gebrek heeft. Welk gebrek precies, laat het aan juristen.',
      'De voorbeelden op deze pagina rekenen met de huidige versie van de software. Die is na het paper verder ontwikkeld, dus een bedrag kan afwijken van wat dezelfde software in augustus 2026 gaf.',
    ],
  },

  time: {
    title: 'Terug in de tijd: welke regels golden toen?',
    intro: [
      'Wetten veranderen. Ze worden aangepast, vervangen of ingetrokken. De vraag ‘wat zegt de wet?’ heeft daarom pas een antwoord als je er een datum bij noemt.',
      'Een besluit van jaren geleden moet je kunnen narekenen met de regels zoals ze toen waren. Daarom vraagt het paper dat elke versie van een regelwerk bewaard blijft, en dat wat op een datum gold, blijft wat het was.',
      'Volgens het paper kun je wetten bijhouden zoals programmeurs hun software bijhouden, met versiebeheer zoals Git. Elke wijziging blijft bewaard, en je kunt twee versies precies naast elkaar leggen en zien wat er veranderde. Het wetsvoorstel, het advies van de Raad van State en de amendementen van de Kamer zijn de tussenstappen. De publicatie in het Staatsblad is de officiële nieuwe versie. Een regelwerk is gewone tekst, dus ook dat wordt zo bijgehouden. Bij een wetswijziging zie je dan precies wat er veranderde, in de wet en in de regels die de computer uitvoert.',
    ],
    caseLabel: 'Een voorbeeld: één percentage, twee jaren',
    caseIntro:
      'In de berekening van de zorgtoeslag zit een percentage van je inkomen. In de regelwerken die deze site gebruikt, verschilt het per jaar.',
    caseRows: [
      ['Regelwerk geldig vanaf 1 januari 2024', '13,67%'],
      ['Regelwerk geldig vanaf 1 januari 2025', '13,7%'],
    ],
    caseNote:
      'Een besluit over 2024 reken je na met het eerste regelwerk, een besluit over 2025 met het tweede. Dit is ook wat er misging bij besluit B hierboven: dat systeem rekende voor 2025 nog met het percentage van 2024. Omdat elke versie gepubliceerd is en een eigen vingerafdruk heeft, valt zo’n fout op.',
    p3:
      'Een regelwerk staat ook niet op zichzelf. De zorgtoeslag gebruikt begrippen uit andere wetten, en die veranderen op andere momenten. Bij het narekenen hoort bij elke wet de versie die op die datum gold. Daarom staat op het bonnetje de vingerafdruk van alle regelwerken die zijn gebruikt, en de datum waarvoor is gerekend.',
    more: [
      'Soms verandert een wet met terugwerkende kracht. Dan moet in het regelwerk staan dat vanaf een bepaalde datum het effect van de oude versie wordt teruggedraaid. Het paper noemt dat tijdreizen (time travel).',
      'Het paper haalt twee vragen uit elkaar. Wat schreef de wet voor op een datum? En met welke regel rekende het systeem op die datum? Rekent een systeem nog met de versie van 2020 terwijl de wet in 2022 veranderde, dan kun je dat zien.',
      'Niet elke wijziging komt uit het Staatsblad. Een Europese verordening of een uitspraak van de rechter kan een regel ook veranderen. Die horen dus ook in de geschiedenis van een regelwerk.',
      'Wie het register met alle versies bijhoudt, en wat voorkomt dat het achteraf wordt herschreven, is volgens het paper een open vraag.',
    ],
  },

  default: {
    title: 'Afwijken mag, waar de wet het toestaat',
    p1:
      'Gebonden zijn aan de gepubliceerde regels betekent niet dat de computer het laatste woord heeft. De computer geeft een standaarduitkomst. Geeft de wet ruimte om daarvan af te wijken, dan mag dat. Bijvoorbeeld als de uitkomst in een bepaald geval te hard uitvalt. Op het bonnetje staat dan wat er is veranderd, waarom en door wie.',
    variantsLabel: 'Drie manieren van afwijken',
    variants: [
      {
        id: 'lex',
        label: 'De wet zelf wijkt af',
        short: 'De wet',
        rows: [
          ['Wat', 'de termijn voor bezwaar'],
          ['Volgens de algemene regel', '6 weken (Algemene wet bestuursrecht)'],
          ['Wordt', '4 weken'],
          ['Waarom', 'de Vreemdelingenwet 2000 zegt dat, in artikel 69'],
        ],
        note: 'Een speciale wet wijkt af van de algemene. Dat staat al in de gepubliceerde regels.',
      },
      {
        id: 'orgaan',
        label: 'De burgemeester wijkt af in één geval',
        short: 'Eén geval',
        rows: [
          ['Wat', 'hoe lang een woning dicht moet'],
          ['Volgens de regel', '6 maanden'],
          ['Wordt', '3 maanden'],
          ['Waarom', 'zes maanden zou te zwaar zijn voor de kinderen in het gezin'],
          ['Op grond van', 'artikel 4:84 van de Algemene wet bestuursrecht'],
          ['Wie', 'de burgemeester'],
        ],
        note: 'Afwijken doet wie over het besluit gaat, hier de burgemeester. Dat kan alleen waar de wet die ruimte geeft. De burgemeester moet de grond noemen en uitleggen waarom het in dit geval nodig is. Doet een ambtenaar het werk, dan staat dat ook op het bonnetje.',
      },
      {
        id: 'batch',
        label: 'Een correctie voor een hele groep',
        short: 'Een groep',
        rows: [
          ['Wat', 'de uitkomst die de regels gaven'],
          ['Wordt', 'wat de rechter heeft bepaald'],
          ['Waarom', 'een uitspraak over een hele groep besluiten'],
          ['Wie', 'de organisatie, voor alle besluiten tegelijk'],
        ],
        note: 'Het bonnetje zegt dan ook dat een programma dit deed, en geen mens die het dossier las.',
      },
    ],
    more: [
      'Gebeurt zo’n afwijking steeds weer automatisch, bij iedereen met een bepaalde eigenschap, dan is de gepubliceerde regel eigenlijk vervangen. Het verschil met nu is dat een controleur het kan vinden, want het staat op elk bonnetje.',
      'De voorbeelden komen uit het paper. Het voorbeeld van de woning is verzonnen. De bezwaartermijn uit de Vreemdelingenwet is echt.',
    ],
  },

  limits: {
    title: 'Wat dit niet oplost',
    intro: [
      'Zolang mensen de wet uitvoerden, was er speelruimte die nergens op papier stond. Een ambtenaar hielp iemand verder terwijl geen regel dat toestond, en niemand keek ernaar. Dat mocht eigenlijk niet, hoe goed het ook was. Maar het geval liep er beter door af dan volgens de regel.',
      'Software heeft die speelruimte niet. Ze doet wat is vastgelegd en verder niets. Dat geldt nu al voor de systemen die de overheid gebruikt, en het heeft niets met publiceren te maken: een regelwerk dat niemand kan inzien, is even streng.',
      'Het voorstel brengt die ongeschreven ruimte niet terug, en dat kan ook niet, want een uitzondering die je opschrijft is gewoon weer een regel. Wel geeft het een deel van de ruimte terug, want de uitkomst van de computer is alleen een standaard. En het maakt zichtbaar wat nu nergens staat:',
    ],
    explicit: [
      [
        'Waar ruimte is',
        'Geeft de wet de overheid ruimte om af te wijken, dan staat dat in het gepubliceerde regelwerk. Ook de gronden die de Algemene wet bestuursrecht daarvoor geeft, staan erin.',
      ],
      ['Waar geen ruimte is', 'Geeft de wet die ruimte niet, dan biedt het regelwerk ook niets om mee af te wijken.'],
      [
        'Dat er is afgeweken',
        'Elke afwijking staat op het bonnetje: wat de regels uitrekenden, wat het werd, op welke grond en wie het deed. Ook als een programma het deed en geen mens.',
      ],
    ],
    explicitNote: 'Zo zie je hoe streng de regels zijn, en wie welke keuze maakte.',
    avoidable:
      'Een deel van de strengheid van bestaande systemen was een keuze, schrijft het paper. Het noemt als voorbeelden: alles terugvorderen of niets, geen mogelijkheid om af te wijken van de berekende uitkomst, en een drempel zonder hardheidsclausule erachter. Van dat deel neemt het voorstel zoveel weg als het kan. Zo wordt ook zichtbaar wat overblijft. Dat hoort bij regels zelf, en dat kan geen bouwer wegnemen.',
    p2: 'Het paper noemt zijn aanpak onvolmaakt, en een aantal vragen laat het open.',
    openLabel: 'Open vragen',
    open: [
      'Wat voor juridisch document een regelwerk eigenlijk is.',
      'Of je recht hebt op het bonnetje, en wat je dan mag zien over anderen, zoals het inkomen van je partner.',
      'Of juristen de regels zonder hulp goed kunnen controleren.',
      'Hoe je zeker weet dat twee programma’s hetzelfde uitrekenen.',
      'Wie wat moet bewijzen als een besluit niet klopt met de gepubliceerde regels.',
    ],
    more: [
      'Een bekende zorg is dat mensen regels gaan ontwijken als ze die kennen. Het paper maakt daarom onderscheid. Alleen de regels die een besluit uitrekenen worden openbaar. Hoe de overheid kiest wie ze controleert, hoort daar niet bij. Wie een adviseur kan betalen, kent de drempels nu ook al.',
      'Er was ook ruimte bij de controle. Niet elke regel werd bij iedereen nagelopen, omdat daar de tijd niet voor was. Een systeem dat iedereen steeds automatisch controleert, haalt die ruimte weg. Het paper zegt erbij dat het zelf hoort bij de beweging die die ruimte kleiner maakt.',
      'Een systeem dat je kunt controleren, krijgt misschien juist meer taken. Ook dat schrijft het paper.',
    ],
  },

  veterans: {
    title: 'Voor wie al jaren met regels als code werkt',
    p1:
      'Wetten als regels voor de computer opschrijven is niet nieuw. Denk aan RegelSpraak bij de Belastingdienst, FLINT bij de IND, OpenFisca en Catala in Frankrijk en Better Rules in Nieuw-Zeeland. Ze helpen specialisten om regels te schrijven en te laten rekenen.',
    p2:
      'Het paper vraagt iets wat geen van deze systemen als doel had. De gepubliceerde regels moeten gegarandeerd de regels zijn waarmee het besluit wordt uitgerekend, en wie het besluit krijgt moet dat kunnen nagaan. Die eis bepaalt ook de vorm. Daarom komt het paper met een eigen vorm, en gebruikt het geen bestaande taal. Het zegt er wel bij dat het overzicht hieronder geen ranglijst is.',
    tableLabel: 'Eerdere aanpakken vergeleken',
    rowHeader: 'Aanpak',
    columns: ['Rekent', 'Openbaar', 'Gebonden'],
    columnHelp: [
      'Een computer kan met de regels rekenen (in het paper: executable).',
      'Iedereen kan de regels lezen (published).',
      'Het is gegarandeerd dat de gepubliceerde regels ook de regels zijn waarmee het besluit wordt uitgerekend (bound to execution). Dit is waar het paper om vraagt.',
    ],
    tableNote:
      'Dat alleen dit paper drie keer ja heeft, is geen toeval. De laatste kolom is precies waar het paper over gaat, en de reden dat het geschreven is. De andere aanpakken zijn gebouwd om regels te schrijven en uit te voeren, en zijn daar goed in. Dit voorstel gaat over wie de regels achteraf wil lezen en controleren.',
    p3:
      'Een voorbeeld dichter bij huis is de Omgevingswet. Daar staan regels machineleesbaar online, en ze sturen de vergunningcheck in het Omgevingsloket. Toch zijn de wet, de regels in het loket en de software van de gemeente drie losse dingen, die mensen met de hand gelijk houden. Het voorstel maakt er één ding van.',
    more: [
      'Het paper kiest niet voor een gecontroleerde natuurlijke taal zoals RegelSpraak, en geeft daar drie redenen voor:',
    ],
    reasons: [
      'Meerdere programma’s moeten hetzelfde uitrekenen. Een tekst die op gewone taal lijkt, kunnen twee programma’s verschillend lezen zonder dat een van beide aantoonbaar fout zit.',
      'Bij een wetswijziging moet precies te zien zijn wat er veranderde. In gewone taal kan een andere formulering hetzelfde lijken en toch iets anders doen.',
      'Het paper zoekt het probleem niet bij wie de regels opschrijft, maar bij wie ze daarna moet kunnen controleren. Dat is de Kamer die wil weten of haar wet goed is omgezet, de rechter die wil nagaan hoe een besluit tot stand kwam, en jij, als je wilt weten of op jouw besluit de gepubliceerde regel is toegepast. RegelSpraak is gemaakt voor wie de regel opstelt. Dit voorstel is gemaakt voor die lezers.',
    ],
    moreAfter: [
      'Een laag in gewone taal die naar dit formaat vertaalt, sluit het paper niet uit. Het zegt alleen dat de vorm die gepubliceerd en ondertekend wordt, de gestructureerde moet zijn. En het zegt erbij dat het deze keuze met argumenten verdedigt, niet met metingen.',
      'In de huidige software vertalen taalmodellen wetten naar dit formaat, artikel voor artikel. Ze rekenen nooit zelf. Een vast programma rekent het besluit uit, met regels die mensen hebben nagekeken en die de organisatie heeft overgenomen.',
    ],
  },
};

const en: Content = {
  meta: {
    title: 'Rules as Executed, explained · RegelRecht',
    description:
      'The paper Rules as Executed in plain language: why it touches the rule of law that nobody outside the executing organization can see which rules a computer executes, and what the paper proposes.',
  },
  header: {
    title: 'Rules as Executed, explained',
    subtitle: 'The paper in plain language',
    lede:
      'More and more often a computer works out what you get or have to pay, for instance with the Dutch healthcare allowance. Anyone can read the law behind it. But the computer does not work with the text of the law. It works with rules the executing organization derived from the law, and those are not published. Only that organization can see which rules the computer executes. Parliament, the courts and you cannot. That is where the paper Rules as Executed starts. This page explains why that touches the rule of law, and what the paper proposes.',
    version: (date) =>
      `This explains the paper as published on ${date}. For the exact argument, read the paper itself.`,
    veterans: 'Been working on rules as code for years? Read what is new here, and why it needs a format of its own.',
    tocLabel: 'On this page',
    pdf: 'PDF on Zenodo',
    paper: 'Read the paper',
    discuss: 'Discuss the paper',
  },
  readInPaper: 'In the paper',
  more: 'More from the paper',
  close: 'Close',
  thisPaper: 'This paper',
  tableLink: 'This comparison in the paper',
  figureLink: 'This example in the paper',
  yes: 'yes',
  part: 'partly',
  no: 'no',
  now: 'Now',
  proposal: 'With the proposal',
  proposalShort: 'With proposal',

  problem: {
    title: 'The problem: the rules the computer executes are not public',
    core:
      'The law is published. The rules the computer executes are not. So nobody outside the executing organization can check whether a decision follows from the law.',
    p1:
      'A law says who is entitled to something. A computer needs more than that. It has to know which data to use, how to fill in a term from the law and what to do when a case is slightly different. The organization that carries out the law makes those choices. Some of them it writes down in rules you can look up. Many are recorded only in work instructions for officials and in the software itself.',
    caseLabel: 'An example: who is your allowance partner?',
    caseIntro:
      'You rent a house together with a friend. You have both been registered at the same address for two years, but you are not a couple. You apply for healthcare allowance. Does your housemate count as your allowance partner?',
    options: [
      {
        id: 'adres',
        label: 'The system only looks at the address',
        short: 'Address',
        outcome:
          'You have been at the same address for more than six months. The system treats your housemate as your allowance partner and counts your housemate’s income too. You get less allowance, or none.',
        tag: 'less or no allowance',
        tone: 'critical',
      },
      {
        id: 'huishouden',
        label: 'The system also looks at the household',
        short: 'Household',
        outcome:
          'You live at the same address, but you do not run a joint household. Your housemate is then not your allowance partner. You get allowance on your own income.',
        tag: 'allowance on own income',
        tone: 'success',
      },
    ],
    caseNote:
      'Among other things, the law says a housemate is your partner if you have been at the same address for more than six months and run a joint household. The address is in a register, the household is not. How a system fills in that second part decides whether you get the allowance, and you will not read it in the law. The paper names exactly this derivation of a partner from registers as an example. The two systems above are made up. How the real system does it, you cannot check today, and that is exactly the problem. This example comes back further down the page.',
    caseLink: { href: 'https://wetten.overheid.nl/BWBR0018472/2025-01-01#Artikel3', text: 'Article 3 of the Dutch General Act on Income-Related Schemes (in Dutch)' },
    demandLabel: 'What the paper therefore asks',
    demand:
      'The law as the computer executes it must be as public as the law as the legislature enacted it, and just as open to scrutiny. You must be able to check that your decision was worked out with exactly those rules.',
    more: [
      'A decision often depends on more than the law itself. It also depends on how the organization reads that law. Part of that reading is in policy rules, and those have to be public. A large part exists nowhere but inside the organization.',
      'This is not about one organization, and not only about the Netherlands. In Australia, a system that averaged incomes wrongly accused hundreds of thousands of welfare recipients of debt (Robodebt). In the United Kingdom, a faulty computer system led to the wrongful prosecution of roughly 1,000 sub-postmasters (the Horizon scandal). The paper names them next to the Dutch childcare benefits scandal, as examples of the same pattern: systems with serious consequences for people, which cannot be checked from outside.',
    ],
  },

  power: {
    title: 'Why this is about power',
    intro: [
      'In the Netherlands, power is divided, but not strictly separated. Making laws, carrying them out and judging are spread over institutions that check and limit each other. That keeps them in balance. The executive has long been the strongest of them.',
      'Since execution moved into software, that balance has tilted further. Only the executive can still see how the law is really applied. The paper calls that a monopoly. Parliament votes on the law but does not see how it works out in practice. A court only gets to see the outcome. Whoever receives the decision cannot check whether it is right. And even the executing organization does not always have the full picture: software grows for years, and sometimes it belongs to a supplier and the government may not look at the code.',
    ],
    case: {
      label: 'What the childcare benefits scandal showed',
      text:
        'The parliamentary inquiry found that all three powers had been ‘blind to people and justice’. That execution could not be seen is, according to the paper, part of what went wrong. Parliament could not see its own laws in the form in which they were carried out. Members had to use their right to information to work through individual files, because the rules themselves could not be inspected. Courts reviewed decisions without being able to inspect the rules they were worked out with. And the executive often had no overview of its own systems.',
    },
    demandsLabel: 'What the rule of law asks',
    demands: [
      [
        'A law must have been made known',
        'Under the Dutch Constitution a law only applies after it has been published (Article 88). The same holds for rules the government makes itself (Article 89). According to the paper, publishing the text alone is not enough if it stays unclear how the law is carried out.',
      ],
      [
        'A minister answers for it',
        'Ministers are responsible for everything the administration does, and owe the Chamber the information its members ask for (Articles 42 and 68 of the Constitution). That cannot be done for a rule nobody can read.',
      ],
      [
        'You must be able to predict what government will do',
        'That is legal certainty. Today you can often only apply and wait for the decision of the one organization that knows the rules. Working it out beforehand or checking it afterwards is not possible.',
      ],
      [
        'And now also: you must be able to check it',
        'The rule of law has long asked that you can know, understand and predict the rules. That assumed you could examine how an official reasoned. Now that computers carry out the law, that is no longer enough. So the paper adds a demand: it must be possible to check independently which rules are really executed.',
      ],
    ],
    p3:
      'The proposal is meant to restore the balance. It gives each power what it needs to do its own job. That only works if the other powers can really work with the published rules.',
    matrixLabel: 'Who can do what?',
    rowHeader: 'Who',
    columns: ['Reads the rules', 'Checks one decision', 'Runs many cases'],
    more: [
      'The paper cites a call from legal scholarship: ‘trias politica by design’. The separation of powers has to be built into the government’s systems themselves, not repaired afterwards with extra oversight.',
      'The courts have not accepted that opacity. One of the highest Dutch administrative courts held that when a decision rests on a calculation model, the government must fully disclose the choices, data and assumptions in it. With a published rulework a court can re-run the outcome itself. The court keeps the last word on what the law means, and is never bound by the rulework.',
      'With the proposal you can re-run your own decision, but not your neighbor’s. That is on purpose. If everyone could re-run every decision, you would buy openness by giving up privacy, the paper says.',
      'Parliament does not need anyone’s personal data. It gets the rules themselves, and can see what a change to the law would do before voting on it. Members of parliament do not have to read those rules themselves. The paper has in mind the support staff of the House, who could do it for every parliamentary group. In France, the LexImpact team at the Assemblée nationale already works out proposals for deputies this way.',
      'Whoever prepares a law can also see earlier what it does. A policymaker can run a new rule before it is submitted, and see whether the same outcome can be reached with less data about people. A legislative drafter can use the worked examples in the explanatory memorandum as a test: the rulework gives the same outcome, or it does not. That only works if those examples are kept up to date, because after an amendment or indexation they often no longer match.',
      'Government now has other checks. The Dutch Algorithm Register lists systems but rarely shows the rules themselves. Committees and assessments such as a DPIA mostly review descriptions, written by the people who built the systems. With published rules they can assess the rules themselves.',
      'The two authors of the paper are engineers, not lawyers. They say so themselves, and ask lawyers to correct what is wrong.',
    ],
  },

  binding: {
    title: 'The proposal: publish the rules, and decide with them',
    intro: [
      'The paper proposes two things that belong together. The first: publish the rules the computer executes, so that anyone can read them, just like the law. The second: the organization has to decide with exactly that published version, and with no other. The organization is then bound to what it publishes. The rules you can read are the rules your decision was worked out with.',
      'That second part is the core. Writing rules down so that a computer can calculate with them has been possible for decades. Publishing rules happens too. But so far nothing guarantees that the published rule is also the rule the decision was worked out with. That is the gap the paper wants to close.',
      'A regulation published this way is called a rulework (the paper itself still says executable specification). A rulework is not a model kept alongside the software. It is what the computer executes.',
      'That puts demands on the form the rules are written in. So the paper comes with a form of its own, and does not use an existing rule language.',
    ],
    case: {
      label: 'The housemate example',
      text:
        'The rulework then says how the organization fills in ‘a joint household’. If the system only looks at the address, anyone can read that: Parliament, the court, your adviser and you. If you think it does not match the law, you know what you are objecting to.',
    },
    formLabel: 'Why a format of its own',
    form: [
      [
        'The rulework follows the law, article by article',
        'It describes the law in articles, conditions and references, the way the legislature wrote it. Each article gets its own piece of rulework. The intent is that a lawyer can check article by article whether it matches the law. Whether that works without help still has to be shown, according to the paper. It is also the difference from publishing a system’s source code. There the choices are buried among technology that has nothing to do with the law, and legislators and judges cannot read that code.',
      ],
      [
        'Every calculation gives an answer',
        'The form can calculate, compare and check dates, and not much more. That way every calculation ends. So re-running always gives an outcome. It has to, if re-running is to be a right.',
      ],
      [
        'No translation step between publication and execution',
        'What is published is exactly what the computer executes. With rules in a language that resembles ordinary language, such as RegelSpraak, that is different. They first have to be converted by a program, and in that program the published rule and the executed rule can drift apart again.',
      ],
    ],
    reuse:
      'That way a law that applies almost everywhere, such as the General Administrative Law Act, can be written down once. Every organization can then use the same rulework, instead of building the rules again itself. When a law refers to another law, that reference is in the rulework, and the version in force on the date is used.',
    codeLead: 'This is the parking article in the format from the paper:',
    flowLabel: 'From law to decision',
    flowNow: [
      { label: 'The law', text: 'Anyone can read it' },
      { label: 'Policy rules', text: 'Partly findable' },
      { label: 'Work instructions', text: 'Cannot be seen' },
      { label: 'The software', text: 'Cannot be seen' },
      { label: 'The decision', text: 'You cannot see which rules were used' },
    ],
    flowNowNote:
      'Each layer translates the one before, and people keep them in line by hand. Nothing guarantees that the last one still matches the first.',
    flowProposal: [
      { label: 'The law', text: 'Anyone can read it' },
      { label: 'The rulework', text: 'Published, and the only version the computer uses' },
      { label: 'The decision', text: 'With a receipt attached' },
    ],
    flowProposalNote: 'The choices that determine the outcome are now in one place, and anyone can read them.',
    more: [
      'Whoever does not want to read the form directly can view the same rulework in a screen that makes the references clickable and runs it against test cases.',
      'The paper does not choose linked data either, a well-known way to make laws machine-readable. That approach looks for one general scheme of concepts for all laws. According to the paper, that is not feasible for laws on healthcare, tax and immigration at once. It keeps to one law per rulework, with references between them.',
      'A rulework is a reading of the law, not the law itself. If it clashes with the law, the law wins and the rulework has to be fixed.',
      'The reading remains the work of the organization that carries out the law. Those choices were already being made, the paper says, only nobody outside the organization could see them. Once published, they are visible, and someone can be held to them.',
      'A rulework that is published but not used in deciding is no solution, according to the paper. It shows a reading that no decision is made with. Where a law has no rulework yet, execution cannot wait. The organization then publishes in policy rules how it applies the law.',
    ],
    exampleLabel: 'What a rulework looks like',
    exampleIntro: 'After a made-up example in the paper: an article from a city parking permit act.',
    exampleStatute:
      'A resident is eligible for a parking permit if their vehicle has low emissions. The fee is reduced for zero-emission vehicles.',
    mapRows: [
      ['A resident', 'is_resident', 'from the population register'],
      ['if their vehicle has low emissions', 'is_low_emission', 'from the vehicle register'],
      ['is eligible for a parking permit', 'eligible = AND(is_resident, is_low_emission)', 'worked out by this article'],
      [
        'The fee is reduced for zero-emission vehicles',
        'permit_fee = IF is_zero_emission THEN reduced_fee ELSE standard_fee',
        'from the fee schedule',
      ],
    ],
    exampleNote: 'Each part of the article gets its own rule, and every value says where it comes from.',
  },

  receipt: {
    title: 'The receipt: how you check it',
    p1:
      'That an organization is bound to what it publishes only has value if someone else can check it. That is why, with the proposal, every decision comes with a receipt. It says which version of the rules the organization says it used, which data it worked with and what came out. The version is on it as a kind of fingerprint: a code that changes as soon as anything in the rules is different.',
    p2:
      'With that receipt you can re-run the decision yourself. You take the published rules, put in the same data and see whether the same amount comes out. The same rules with the same data always give the same result.',
    demoIntro:
      'Try it. Below are two decisions on the healthcare allowance of the same person, with test data. Your browser has just worked them out. Click Check it on both.',
    decision: 'Decision',
    fields: {
      law: 'Rules',
      date: 'Calculation date',
      inputs: 'Data',
      amount: 'Allowance according to the decision',
      digest: 'Fingerprint of the rules',
    },
    inputsSummary: '{n} values from the registers, the same for both decisions',
    inForce: 'in force from {date}',
    check: 'Check it',
    checking: 'Checking…',
    checkDigest: 'The same rules as published',
    checkAmount: 'The same amount when re-run',
    ok: 'matches',
    mismatch: 'does not match',
    loading: 'Fetching the rules…',
    failed: 'The check failed. Reload the page.',
    noScript: 'This needs JavaScript: the check runs in your own browser.',
    revealA: 'Both checks pass. The rules on the receipt are the published ones, and re-running gives the same amount.',
    revealB: (published, local, diff) =>
      `This does not match. The system behind this decision used its own copy of the rules. In it, one percentage still had last year’s value: ${local} instead of ${published}. That copy was never published. The difference is ${diff}, and it shows in two ways: the fingerprint is different, and re-running gives a different amount.`,
    limitsLabel: 'What the receipt does not show',
    limits: [
      'The receipt shows which data were used, but does not prove that those data are correct. You can put them next to what you know yourself, such as your own income. The paper also wants to tie each value, where possible, to the source responsible for it, such as a base register.',
      'Whether you already have a right to such a receipt is unclear. The paper proposes that you always get it, together with the decision.',
    ],
    more: [
      'Both decisions were really worked out, in your browser, with the rules this site uses. For decision B this page alters a copy of the rules itself. A real receipt is also signed by the organization. This page signs nothing.',
      'A real receipt also says which program did the calculation. If the program you re-run with calculates differently, a difference can come out as well. How to be sure that two programs calculate the same thing is still an open question, according to the paper.',
      'Publishing and the receipt only have consequences once they are in the law. The paper holds that a decision without a receipt then has a defect itself. Which defect exactly, it leaves to lawyers.',
      'The examples on this page use the current version of the software. It has moved on since the paper, so an amount can differ from what the same software gave in August 2026.',
    ],
  },

  time: {
    title: 'Back in time: which rules applied then?',
    intro: [
      'Laws change. They are amended, replaced or repealed. So the question ‘what does the law say?’ only has an answer once you name a date.',
      'A decision from years ago must be something you can re-run with the rules as they were then. That is why the paper asks that every version of a rulework is kept, and that what applied on a date stays what it was.',
      'According to the paper, laws can be kept the way programmers keep their software, with version control such as Git. Every change is kept, and you can put two versions side by side and see exactly what changed. The bill, the advice of the Council of State and the amendments of Parliament are the steps in between. Publication in the Staatsblad, the Dutch bulletin of acts, is the official new version. A rulework is plain text, so it is kept the same way. When a law changes, you then see exactly what changed, in the law and in the rules the computer executes.',
    ],
    caseLabel: 'An example: one percentage, two years',
    caseIntro:
      'The calculation of the healthcare allowance contains a percentage of your income. In the ruleworks this site uses, it differs per year.',
    caseRows: [
      ['Rulework in force from 1 January 2024', '13.67%'],
      ['Rulework in force from 1 January 2025', '13.7%'],
    ],
    caseNote:
      'You re-run a decision about 2024 with the first rulework, and a decision about 2025 with the second. This is also what went wrong with decision B above: that system still calculated 2025 with the percentage of 2024. Because every version is published and has its own fingerprint, a mistake like that stands out.',
    p3:
      'A rulework does not stand alone either. The healthcare allowance uses terms from other laws, and those change at other moments. Re-running takes, for each law, the version that applied on that date. That is why the receipt carries the fingerprint of every rulework that was used, and the date the calculation was for.',
    more: [
      'Sometimes a law changes with retroactive effect. The rulework then has to say that from a certain date the effect of the old version is reversed. The paper calls that time travel.',
      'The paper separates two questions. What did the law require on a date? And which rule did the system calculate with on that date? If a system still calculates with the 2020 version while the law changed in 2022, that can be seen.',
      'Not every change comes from the Staatsblad. A European regulation or a court ruling can change a rule too. So those belong in the history of a rulework as well.',
      'Who keeps the register with all versions, and what keeps it from being rewritten afterwards, is an open question according to the paper.',
    ],
  },

  default: {
    title: 'Departing is allowed where the law allows it',
    p1:
      'Being bound to the published rules does not mean the computer has the last word. The computer gives a default outcome. If the law leaves room to depart from it, that is allowed. For instance when the outcome is too harsh in a particular case. The receipt then says what was changed, why and by whom.',
    variantsLabel: 'Three ways to depart',
    variants: [
      {
        id: 'lex',
        label: 'The law itself departs',
        short: 'The law',
        rows: [
          ['What', 'the time limit for an objection'],
          ['Under the general rule', '6 weeks (General Administrative Law Act)'],
          ['Becomes', '4 weeks'],
          ['Why', 'the Vreemdelingenwet 2000 says so, in Article 69'],
        ],
        note: 'A specific law departs from the general one. That is already in the published rules.',
      },
      {
        id: 'orgaan',
        label: 'The mayor departs in one case',
        short: 'One case',
        rows: [
          ['What', 'how long a house has to be closed'],
          ['Under the rule', '6 months'],
          ['Becomes', '3 months'],
          ['Why', 'six months would be too hard on the children in the family'],
          ['Legal ground', 'Article 4:84 of the General Administrative Law Act'],
          ['Who', 'the mayor'],
        ],
        note: 'Whoever is responsible for the decision departs from it, here the mayor. That is only possible where the law gives room for it. The mayor has to name the ground and explain why it is needed in this case. If an official does the work, that is on the receipt too.',
      },
      {
        id: 'batch',
        label: 'A correction for a whole group',
        short: 'A group',
        rows: [
          ['What', 'the outcome the rules gave'],
          ['Becomes', 'what the court decided'],
          ['Why', 'a ruling about a whole group of decisions'],
          ['Who', 'the organization, for all decisions at once'],
        ],
        note: 'The receipt then also says that a program did this, and not a person who read the file.',
      },
    ],
    more: [
      'If such a departure happens automatically every time, for everyone with a certain feature, the published rule has in fact been replaced. The difference with today is that an auditor can find it, because it is on every receipt.',
      'The examples come from the paper. The house example is made up. The objection period in the Vreemdelingenwet is real.',
    ],
  },

  limits: {
    title: 'What this does not fix',
    intro: [
      'As long as people carried out the law, there was room to move that was written down nowhere. An official helped someone along although no rule allowed it, and nobody looked. That was not actually allowed, however good it was. But the case came out better than the rule would have made it.',
      'Software has no such room. It does what has been recorded and nothing else. That is already true of the systems the government uses today, and it has nothing to do with publishing: a rulework nobody can see is just as strict.',
      'The proposal does not bring that unwritten room back, and it cannot, because an exception you write down is simply another rule. It does give part of the room back, because the computer’s outcome is only a default. And it makes visible what is written down nowhere today:',
    ],
    explicit: [
      [
        'Where there is room',
        'If the law gives the authority room to depart, that is in the published rulework. So are the grounds the General Administrative Law Act gives for it.',
      ],
      ['Where there is none', 'If the law gives no such room, the rulework offers nothing to depart with either.'],
      [
        'That a departure happened',
        'Every departure is on the receipt: what the rules computed, what it became, on what ground and who did it. Also when a program did it rather than a person.',
      ],
    ],
    explicitNote: 'That way you can see how strict the rules are, and who made which choice.',
    avoidable:
      'Part of the strictness of existing systems was a choice, the paper says. Its examples: recovering everything or nothing, no way to depart from the computed outcome, and a threshold with no hardship clause behind it. The proposal takes away as much of that part as it can. That also shows what is left. That belongs to rules as such, and no builder can take it away.',
    p2: 'The paper calls its approach imperfect, and it leaves a number of questions open.',
    openLabel: 'Open questions',
    open: [
      'What kind of legal document a rulework actually is.',
      'Whether you have a right to the receipt, and what you may then see about others, such as your partner’s income.',
      'Whether lawyers can check the rules properly without help.',
      'How to be sure that two programs calculate the same thing.',
      'Who has to prove what when a decision does not match the published rules.',
    ],
    more: [
      'A familiar worry is that people will dodge rules once they know them. That is why the paper draws a line. Only the rules that work out a decision become public. How government chooses whom to check is not part of that. Anyone who can afford an adviser already knows the thresholds today.',
      'There was also room in enforcement. Not every rule was checked for everyone, because there was no time to. A system that keeps checking everyone automatically takes that room away. The paper adds that it belongs itself to the movement that makes that room smaller.',
      'A system you can check may be given more tasks. The paper says that too.',
    ],
  },

  veterans: {
    title: 'For those who have worked on rules as code for years',
    p1:
      'Writing laws down as rules for a computer is not new. Think of RegelSpraak at the Dutch Tax Administration, FLINT at the Dutch immigration service, OpenFisca and Catala in France and Better Rules in New Zealand. They help specialists write rules and run them.',
    p2:
      'The paper asks for something none of these systems set out to do. The published rules must be guaranteed to be the rules the decision was worked out with, and whoever gets the decision must be able to check that. That demand also shapes the form. So the paper comes with a form of its own, and does not use an existing language. It does add that the comparison below is not a ranking.',
    tableLabel: 'Earlier approaches compared',
    rowHeader: 'Approach',
    columns: ['Runs', 'Public', 'Bound'],
    columnHelp: [
      'A computer can calculate with the rules (in the paper: executable).',
      'Anyone can read the rules (published).',
      'It is guaranteed that the published rules are the rules the decision was worked out with (bound to execution). This is what the paper asks for.',
    ],
    tableNote:
      'That only this paper has three yeses is no coincidence. The last column is exactly what the paper is about, and the reason it was written. The other approaches were built to write rules and run them, and they are good at that. This proposal is about whoever wants to read and check the rules afterwards.',
    p3:
      'An example closer to home is the Dutch Environment and Planning Act (Omgevingswet). There, rules are online in machine-readable form, and they drive the permit check in the national portal. Still, the law, the rules in the portal and the municipality’s software are three separate things that people keep in line by hand. The proposal makes them one.',
    more: ['The paper does not choose a controlled natural language like RegelSpraak, and gives three reasons:'],
    reasons: [
      'Several programs have to calculate the same thing. Two programs can read text that looks like ordinary language differently, without either being demonstrably wrong.',
      'When a law changes, it must be exactly clear what changed. In ordinary language a different wording can look the same and still do something else.',
      'The paper does not place the problem with whoever writes the rules down, but with whoever has to be able to check them afterwards. That is Parliament, wanting to know whether its law was converted faithfully, the court, wanting to trace how a decision came about, and you, if you want to know whether the published rule was applied to your decision. RegelSpraak is made for whoever writes the rule. This proposal is made for those readers.',
    ],
    moreAfter: [
      'The paper does not rule out a layer in ordinary language that translates into this format. It only says that the form that is published and signed has to be the structured one. And it adds that it defends this choice with arguments, not with measurements.',
      'In the current software, language models translate laws into this format, article by article. They never calculate anything themselves. A decision is worked out by a fixed program, with rules that people have reviewed and that the organization has adopted.',
    ],
  },
};

export const content: Record<Lang, Content> = { nl, en };

/**
 * Who can do what, now and with the proposal (the part on power). The cells are claims
 * too, and each is in the claims file.
 */
export function actors(lang: Lang): ActorRow[] {
  const n = (v: Cell['v'], note?: string): Cell => ({ v, note });
  const t = lang === 'nl';
  return [
    {
      actor: t ? 'Wie de wet uitvoert' : 'Whoever carries out the law',
      now: [n('part', t ? 'niet altijd met het hele overzicht' : 'not always with the full picture'), n('part'), n('part')],
      proposal: [n('yes'), n('yes'), n('yes')],
    },
    {
      actor: t ? 'Jij, over je eigen besluit' : 'You, about your own decision',
      now: [n('no'), n('no', t ? 'je ziet alleen de uitkomst' : 'you only see the outcome'), n('no')],
      proposal: [n('yes'), n('yes', t ? 'je eigen besluit' : 'your own decision'), n('no')],
    },
    {
      actor: t ? 'Bezwaar en rechter' : 'Objection and court',
      now: [n('part', t ? 'moet het zelf uitzoeken' : 'has to work it out alone'), n('no'), n('no')],
      proposal: [n('yes'), n('yes', t ? 'het besluit dat voorligt' : 'the decision in front of them'), n('no')],
    },
    {
      actor: t ? 'Controleurs met toegang tot de gegevens' : 'Auditors with access to the data',
      now: [n('no', t ? 'zien vooral beschrijvingen' : 'mostly see descriptions'), n('no'), n('no')],
      proposal: [n('yes'), n('yes'), n('yes', t ? 'de echte besluiten' : 'real cases')],
    },
    {
      actor: t ? 'De Tweede Kamer en iedereen' : 'Parliament and everyone else',
      now: [n('no'), n('no'), n('no')],
      proposal: [
        n('yes', t ? 'de hele regel' : 'the whole rule'),
        n('no', t ? 'geen gegevens van mensen' : 'no personal data'),
        n('yes', t ? 'op cijfers over de bevolking, zonder echte gevallen' : 'on population figures, no real cases'),
      ],
    },
    {
      actor: t ? 'Wie een wet voorbereidt' : 'Whoever prepares a law',
      now: [n('part', t ? 'de wet wel, de software niet' : 'the law, not the software'), n('no'), n('no')],
      proposal: [
        n('yes'),
        n('no', t ? 'geen gegevens van mensen' : 'no personal data'),
        n('yes', t ? 'een voorstel, voordat het wet wordt' : 'a bill, before it is law'),
      ],
    },
  ];
}
