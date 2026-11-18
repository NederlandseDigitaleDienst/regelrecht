/*
 * Bilingual landing content. One layout, two datasets.
 * Language is derived from the route path ("/en/..." → en, otherwise nl).
 *
 * The NL strings are the originals from the static landing page.
 * The EN strings are a translation kept in the same government register.
 */

import paper from '~/research/rules-as-executed.meta.json';
import jurixPaper from '~/research/citizen-explanation.meta.json';
import promisedPaper from '~/research/rules-as-promised.meta.json';
import { joinAuthors } from '~/lib/authors';

// The papers' titles, authors and dates come from the generated meta JSON
// next to their HTML, so a regenerated paper cannot drift from the tile that
// announces it.
const paperTitle = `${paper.title}: ${paper.subtitle}`;
const paperAuthors = paper.authors.join(' & ');
const jurixTitle = `${jurixPaper.title}: ${jurixPaper.subtitle}`;
const jurixAuthors = joinAuthors(jurixPaper.authors.map((a) => a.name), '&');
const promisedAuthors = joinAuthors(promisedPaper.authors.map((a) => a.name), '&');
function paperDate(locale: string, date = paper.date): string {
  return new Intl.DateTimeFormat(locale, {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  }).format(new Date(`${date}T00:00:00`));
}

export interface NavLink {
  label: string
  href: string
}

export interface LandingContent {
  meta: { title: string; description: string }
  nav: {
    brandTitle: string
    brandMinistry: string
    home: string
    what: string
    how: string
    tools: string
    example: string
    faq: string
    jobs: string
    signup: string
    docs: string
    research: string
    roadmap: string
  }
  hero: { title: string; intro: string }
  whatIsIt: { title: string; lede: string; cards: { h: string; p: string }[] }
  whyImportant: {
    title: string
    lede: string
    problemSolutions: {
      problemTitle: string
      problemText: string
      solutionTitle: string
      solutionText: string
    }[]
  }
  howItWorks: { title: string; lede: string; steps: { title: string; text: string }[] }
  tools: {
    title: string
    items: { title: string; text: string; link?: NavLink; meta?: string }[]
  }
  example: {
    title: string
    lede: string
    cases: {
      img: string
      alt: string
      h: string
      p: string
      bullets: string[]
      reverse?: boolean
      // Set where the screenshot shows a screen a reader can actually open.
      // The editor has none: it sits behind a login.
      link?: NavLink
    }[]
  }
  innovation: {
    title: string
    cards: { meta: string; h: string; p: string }[]
  }
  research: {
    title: string
    lede: string
    items: {
      title: string
      meta: string
      text: string
      href: string
      linkLabel: string
      /** A second button on the card, in the page's own language. */
      also?: { href: string; linkLabel: string }
    }[]
  }
  references: {
    title: string
    items: {
      title: string
      meta: string
      text: string
      href: string
      linkLabel: string
    }[]
  }
  // The roadmap teaser. `fases` carries only the prose per fase; the names,
  // subtitles and counts come from the roadmap's own config and content, so
  // the landing page cannot drift from the page it links to.
  roadmap: {
    title: string
    lede: string
    werkpakkettenLabel: (n: number) => string
    ctaLabel: string
    fases: Record<string, string>
  }
  faq: { title: string; items: { q: string; a: string; link?: NavLink }[] }
  jobs: {
    title: string
    lede: string
    vacancyTag: string
    contactsLabel: string
    closesLabel: { open: string; closed: string }
    items: {
      title: string
      organisation: string
      pitch: string
      meta: string[]
      closes: string
      ctaLabel: string
      ctaHref: string
      contacts: { label: string; href: string }[]
    }[]
  }
  feedback: { title: string; body: string; cta: string; ctaHref: string }
  footer: {
    blurb: string
    linksTitle: string
    contactTitle: string
    partOfTitle: string
    links: NavLink[]
    partOf: { label: string; href?: string }[]
  }
  signup: {
    pageTitle: string
    metaDescription: string
    noscript: string
    radioYes: string
    radioNo: string
    emailLabel: string
    nameLabel: string
    orgLabel: string
    orgPlaceholder: string
    roleLabel: string
    rolePlaceholder: string
    required: string
    requiredSr: string
    submit: string
    submitting: string
    companyHoneypot: string
    errEmailEmpty: string
    errEmailInvalid: string
    errName: string
    successTitle: string
    successBody: string
    successReset: string
    errorTitle: string
    errorBody: string
    errorReset: string
  }
}

const SIGNUP_NL_PATH = '/aanmelden'
const SIGNUP_EN_PATH = '/en/signup'
const GUIDE_PATH = '/guide/what-is-regelrecht'
const GITHUB = 'https://github.com/NederlandseDigitaleDienst/regelrecht'

export const content: Record<'nl' | 'en', LandingContent> = {
  nl: {
    meta: {
      title: 'RegelRecht: van wet naar digitale werking',
      description:
        'Een project van de Nederlandse Digitale Dienst dat verkent of wetgeving als uitvoerbare code geschreven kan worden.',
    },
    nav: {
      brandTitle: 'Nederlandse Digitale Dienst',
      brandMinistry: 'Ministerie van Economische Zaken en Klimaat',
      home: 'Home',
      what: 'Wat',
      how: 'Hoe',
      tools: 'Ecosysteem',
      example: 'Voorbeelden',
      faq: 'Vragen',
      jobs: 'Werken bij',
      signup: 'Aanmelden',
      docs: 'Documentatie',
      research: 'Onderzoek',
      roadmap: 'Roadmap',
    },
    hero: {
      title: 'Van wet naar digitale werking',
      intro:
        'RegelRecht verkent of wetgeving als uitvoerbare code geschreven kan worden, zodat verschillende organisaties dezelfde wet ook hetzelfde toepassen en burgers kunnen volgen hoe een besluit tot stand komt.',
    },
    whatIsIt: {
      title: 'Wetten uitvoeren zonder te programmeren',
      lede: 'De uitvoering van wetgeving kent verschillende uitdagingen: verschillende interpretaties, ondoorzichtige systemen en complex programmeerwerk dat vaak ver af staat van de oorspronkelijke wet. RegelRecht verkent of machine-uitvoerbare wetgeving een antwoord kan bieden: wetten die direct als uitvoerbare code geschreven worden, zonder tussenkomst van programmeurs.',
      cards: [
        {
          h: 'Van analoog recht naar code',
          p: 'Kunnen we traditionele wetgeving omzetten naar regelwerken, waarin de wettekst en de uitvoerbare interpretatie naast elkaar staan? We onderzoeken of dit de kloof tussen wetgever en uitvoering kan verkleinen.',
        },
        {
          h: 'Gedeelde, controleerbare uitvoering',
          p: 'Wat als de manier waarop een wet wordt uitgevoerd publiek leesbaar en machine-uitvoerbaar gepubliceerd wordt, naast de wettekst zelf? Verschillende partijen, van uitvoeringsorganisaties tot burgers, kunnen die uitvoering dan onafhankelijk controleren en draaien, en interpretatieverschillen worden zichtbaar in plaats van verborgen in code.',
        },
        {
          h: 'Volledige transparantie',
          p: 'Hoe maken we overheidsbesluiten inzichtelijker? We experimenteren met manieren waarop burgers kunnen zien welke regels gelden en hoe beslissingen tot stand komen.',
        },
      ],
    },
    whyImportant: {
      title: 'Het toepassen van wetten kent momenteel verschillende uitdagingen in onze rechtsstaat',
      lede: 'We onderzoeken of nieuwe technische benaderingen kunnen bijdragen aan oplossingen voor deze structurele vraagstukken.',
      problemSolutions: [
        {
          problemTitle: 'Verschillende interpretaties',
          problemText:
            'Dezelfde wet wordt door verschillende overheidsorganisaties anders geïnterpreteerd en toegepast, wat leidt tot inconsistenties en onrecht.',
          solutionTitle: 'Eenduidige toepassing',
          solutionText:
            'Zouden machine-uitvoerbare wetten interpretatieproblemen kunnen verminderen? We onderzoeken of dit kan leiden tot consistentere regeltoepassing.',
        },
        {
          problemTitle: 'Ondoorzichtige beslissingen',
          problemText:
            'Burgers krijgen besluiten zonder uitleg over hoe deze tot stand zijn gekomen. Overheid als black box.',
          solutionTitle: 'Volledige traceerbaarheid',
          solutionText:
            'Kunnen we elk besluit traceerbaar maken naar de exacte regeltoepassing? We verkennen mogelijkheden voor meer transparantie in overheidsbeslissingen.',
        },
        {
          problemTitle: 'Onuitvoerbare wetten',
          problemText:
            'Wetten worden vaak geschreven zonder volledig te testen of ze in de praktijk uitvoerbaar zijn. Dit kan leiden tot implementatieproblemen door inconsistenties, onduidelijkheden of praktische beperkingen.',
          solutionTitle: 'Uitvoerbaarheid testen',
          solutionText:
            'Zou machine-uitvoerbare wetgeving het mogelijk maken om wetten te testen? We onderzoeken of inconsistenties en conflicten vroegtijdig gedetecteerd kunnen worden.',
        },
      ],
    },
    howItWorks: {
      title: 'De overgang van analoog recht naar digitaal rechtsstelsel',
      lede: 'Hoe zou dat kunnen verlopen? We verkennen 7 mogelijke stappen en onderzoeken wat daardoor mogelijk zou kunnen worden:',
      steps: [
        {
          title: 'Analoog naar digitaal',
          text: 'Kunnen bestaande wetten systematisch worden omgezet van analoge tekst naar regelwerken? Een eerste stap om een digitale basis te onderzoeken.',
        },
        {
          title: 'Digitaal rechtsstelsel',
          text: 'Zouden nieuwe wetten vanaf het begin machine-uitvoerbaar kunnen worden geschreven? We verkennen hoe dat eruit zou kunnen zien en hoe we daarin kunnen ondersteunen.',
        },
        {
          title: 'Gestandaardiseerd ecosysteem',
          text: 'Een landelijke infrastructuur waar overheidssystemen dezelfde wettelijke definities gebruiken. Een gemeenschappelijke, gepubliceerde basis waar elke partij vanaf werkt en die ze onafhankelijk kunnen verifiëren.',
        },
        {
          title: 'Harmonisatie van wetgeving',
          text: 'Het wordt mogelijk om systematisch te werken aan harmonisatie van bestaande wetgeving. Conflicten en inconsistenties tussen bestaande regelsets kunnen automatisch worden gedetecteerd, waardoor harmonisatie een bewuste keuze wordt.',
        },
        {
          title: 'Uitvoerbaarheidstoets',
          text: 'Nieuwe wetten kunnen worden getest voordat ze ingaan. Het effect van nieuwe wetgeving op de consistentie van het rechtsstelsel kan tijdens het wetgevingsproces worden geanalyseerd.',
        },
        {
          title: 'Centrale publicatie',
          text: 'Machine-uitvoerbare wetgeving wordt centraal gepubliceerd voor iedereen. Execution engines worden ook beschikbaar gesteld zodat alle partijen dezelfde wetten op identieke wijze kunnen uitvoeren.',
        },
        {
          title: 'Transparante toepassing',
          text: 'Burgers en bedrijven kunnen de exacte werking van regels inzien en controleren. Volledige inzichtelijkheid in regeltoepassing.',
        },
      ],
    },
    tools: {
      title: 'Ecosysteem',
      items: [
        {
          title: 'Regelformaat',
          meta: 'YAML + JSON Schema',
          link: { label: 'Schemadocumentatie', href: '/reference/schema' },
          text: 'Elke regeling als regelwerk: YAML met de wettekst en de machine-uitvoerbare regels naast elkaar, één bestand per versie. Een versioned JSON Schema bewaakt de structuur.',
        },
        {
          title: 'BDD-scenario’s',
          meta: 'Gherkin + cucumber',
          link: { label: 'Scenario’s uitproberen', href: '/concepts/scenarios' },
          text: 'Verwachte uitkomsten worden vastgelegd als leesbare scenario’s. Juristen en programmeurs lezen dezelfde tests, en elke wijziging in de regels wordt direct gevalideerd. Waar mogelijk halen we die scenario’s rechtstreeks uit de memorie van toelichting.',
        },
        {
          title: 'Execution engine',
          meta: 'Rust + WebAssembly',
          link: { label: 'Documentatie', href: '/components/engine' },
          text: 'Een deterministische execution engine die de YAML-regels uitvoert. Werkt in Rust en compileert naar WebAssembly zodat dezelfde regels in de browser en op de server hetzelfde resultaat geven.',
        },
        {
          title: 'Analoog-recht-converter',
          meta: 'AI-ondersteund',
          text: 'Een LLM-gebaseerde tool die bestaand analoog recht zou kunnen omzetten naar machine-uitvoerbare regels. We verkennen hoe automatische transformatie eruit zou kunnen zien.',
        },
        {
          title: 'Editor',
          meta: 'Werk in uitvoering',
          link: { label: 'Documentatie', href: '/components/editor-api' },
          text: 'Een werkomgeving waarin juristen wetten machine-uitvoerbaar kunnen maken. We zijn aan het ontdekken hoe deze editor er precies uit moet zien.',
        },
        {
          title: 'Wettengraaf',
          meta: 'Relatie-analyse',
          text: 'Een visualisatie van de relaties tussen verschillende wetten, die zou kunnen tonen hoe wijzigingen in één wet doorwerken in het hele juridische landschap.',
        },
        {
          title: 'Corpus',
          meta: 'Git-gebaseerd',
          link: { label: 'Documentatie', href: '/components/corpus' },
          text: 'De bibliotheek van machine-uitvoerbare regels. Git verzorgt de versiegeschiedenis; een registry verbindt verschillende bronnen tot één geheel.',
        },
        {
          title: 'Simulatieomgeving',
          meta: 'Wat-als-analyse',
          link: {
            label: 'Live demo',
            href: 'https://demo.regelrecht.rijks.app/simulatie',
          },
          text: 'Een omgeving waarin de gevolgen van nieuwe wetgeving doorgerekend zouden kunnen worden, voordat ze in werking treden. Bedoeld om maatschappelijke impact en onbedoelde effecten zichtbaar te maken.',
        },
        {
          title: 'Publicatieplatform',
          meta: 'API + web',
          text: 'Een centrale plek voor publicatie en distributie van machine-uitvoerbare regels, met API-toegang voor overheidssystemen en private partijen.',
        },
      ],
    },
    example: {
      title: 'Een aantal denkrichtingen voor de praktijk',
      lede: 'Wat zouden de mogelijkheden van het RegelRecht-ecosysteem kunnen zijn? Een paar denkrichtingen voor transparante regeltoepassing, wetgevingstesting en de werkomgeving van de juridische experts zelf.',
      cases: [
        {
          img: '/portaal-screenshot.png',
          alt: 'Schermafbeelding van een persoonlijk regeldashboard: een lijst met toeslagen en uitkeringen waarbij per regel de herkomst in de wet wordt getoond.',
          h: 'Persoonlijk regeldashboard',
          p: 'Wat als burgers op één plek al hun toeslagen, uitkeringen en verplichtingen zouden kunnen zien? Elke regel zou dan traceerbaar kunnen zijn terug naar de machine-uitvoerbare wetgeving, met volledige transparantie over hoe besluiten tot stand komen.',
          bullets: [
            'Real-time regeltoepassing: zou directe feedback mogelijk maken?',
            'Volledige traceerbaarheid: kan een pad van wet naar persoonlijke situatie gelegd worden?',
            'Proactieve communicatie: kunnen burgers automatisch geïnformeerd worden bij regelwijzigingen?',
          ],
          link: {
            label: 'Bekijk in de demo',
            href: 'https://demo.regelrecht.rijks.app/portaal',
          },
        },
        {
          img: '/simulatie-screenshot.png',
          alt: 'Schermafbeelding van een simulatieomgeving waarin het effect van een wetswijziging op verschillende voorbeeldsituaties wordt doorgerekend.',
          h: 'Wetgeving simulatie & testing',
          p: 'Wat als beleidsmakers de gevolgen van nieuwe wetgeving zouden kunnen testen in een simulatieomgeving voordat deze wordt ingevoerd? Zou dit onbedoelde effecten kunnen voorkomen en de kwaliteit van wetgeving kunnen verbeteren?',
          bullets: [
            'Impactanalyse: zouden we de gevolgen van nieuwe regelgeving kunnen voorspellen?',
            'Harmonisatiecontrole: kunnen we conflicten met bestaande wetgeving detecteren?',
            'Scenariotesting: is het mogelijk verschillende beleidsopties te testen?',
            'Kwaliteitscontrole: kunnen inconsistenties vóór implementatie worden gedetecteerd?',
          ],
          reverse: true,
          link: {
            label: 'Bekijk in de demo',
            href: 'https://demo.regelrecht.rijks.app/simulatie',
          },
        },
        {
          img: '/editor-notities-screenshot.png',
          alt: 'Schermafbeelding van de RegelRecht-editor: de tekst van de Wet op de zorgtoeslag links, machine-leesbare definities en outputs in het midden, en scenario’s met verwachte uitkomsten rechts. Bij een geselecteerd begrip is een notitie-popup geopend.',
          h: 'Editor met notities en scenario’s',
          p: 'Wat als juristen, beleidsmakers en programmeurs in dezelfde omgeving aan wetgeving kunnen werken? Notities bij begrippen, machine-leesbare definities en testbare scenario’s, allemaal naast de oorspronkelijke wettekst.',
          bullets: [
            'Begrippen-annotatie: kunnen juristen direct toelichting toevoegen bij specifieke termen?',
            'Live scenario’s: zien we meteen of de regels nog kloppen na een wijziging?',
            'Meerdere wetten naast elkaar: kunnen we dwarsverbanden tussen wetten zichtbaar maken?',
          ],
        },
      ],
    },
    innovation: {
      title: 'RegelRecht draagt bij aan projecten uit het Innovatiebudget 2025',
      cards: [
        {
          meta: 'In samenwerking met VNG',
          h: 'Minder burgers in de knel door machineleesbare wetgeving',
          p: 'Hoe kunnen we voorkomen dat de stapeling van wet- en regelgeving wetten onuitvoerbaar maakt? Dit project verkent de ontwikkeling van een analysetool om wetsvoorstellen te toetsen op uitvoerbaarheid in samenhang met andere wetten.',
        },
        {
          meta: 'In samenwerking met Dienst Toeslagen',
          h: 'Modern rekenhart als bouwsteen voor de hele overheid',
          p: "Kunnen we een algemeen rekenhart voor de overheid ontwikkelen? Dit project verkent hoe zo'n systeem zou kunnen helpen bij het uitvoeren van complexe regelingen voor burgers en bedrijven, bijvoorbeeld bij het berekenen van toeslagen.",
        },
      ],
    },
    research: {
      title: 'Onderzoek',
      lede: 'De verkenning roept vragen op die verder reiken dan techniek: wat het voor de verhouding tussen de staatsmachten betekent als wetsuitvoering publiek en narekenbaar wordt, en welke juridische status een regelwerk heeft. Dat denkwerk schrijven we op voor een academisch publiek, zodat juristen en bestuurskundigen het kunnen toetsen en aanscherpen.',
      items: [
        {
          title: paperTitle,
          meta: `${paper.kind} • ${paperAuthors} • ${paperDate('nl-NL')}`,
          text: 'Het paper betoogt dat de overheid haar wetsuitvoering moet publiceren als machine-uitvoerbare specificatie, waarbij elk besluit vastlegt met welke versie het is genomen. Wie een besluit ontvangt, kan het narekenen, en het parlement kan de regel zelf analyseren.',
          href: '/research/rules-as-executed',
          linkLabel: 'position paper (Engels)',
          also: { href: '/research/rules-as-executed/uitgelegd', linkLabel: 'uitleg in gewone taal' },
        },
        {
          title: jurixTitle,
          meta: `Paper op ${jurixPaper.venue} • ${jurixAuthors} • ${paperDate('nl-NL', jurixPaper.date)}`,
          text: 'Een taalmodel kan een besluit van de engine aan een burger uitleggen, maar verzint er ook redenen bij. Het paper vergelijkt drie manieren om het model het spoor van de engine te geven, over vijf taalmodellen en drie wetten. Hoe strakker de structuur, hoe trouwer de uitleg aan het spoor; alleen geven burgers niet altijd de voorkeur aan de trouwste uitleg.',
          href: '/research/citizen-explanation',
          linkLabel: 'paper (Engels)',
        },
        {
          title: promisedPaper.title,
          meta: `Paper op ${promisedPaper.venue} • ${promisedAuthors} • ${paperDate('nl-NL', promisedPaper.date)}`,
          text: 'Tests voor een regelwerk halen hun verwachte uitkomst meestal bij wie het regelwerk schreef, en toetsen dus diens eigen lezing. Dit paper haalt ze uit de rekenvoorbeelden waarmee de regering het parlement laat zien wat een wet moet opleveren, en draait er vijftien tegen het corpus: elf geven het beloofde bedrag.',
          href: '/research/rules-as-promised',
          linkLabel: 'paper (Engels)',
        },
        {
          title: 'Rules as Executed: When machine-executable laws are published',
          meta: 'Tutorial op JURIX 2026 • Anne Schuth & Eelco Hotting • 8 december 2026',
          text: 'Negentig minuten met de demo voor een zaal vol onderzoekers in recht en informatica. We openen Nederlandse wetten naast het regelwerk dat ze uitvoerbaar maakt, draaien aan een drempel en kijken wie zijn toeslag verliest. Tegenspraak is de bedoeling.',
          href: '/research/jurix-2026-tutorial',
          linkLabel: 'over de tutorial (Engels)',
        },
      ],
    },
    references: {
      title: 'Belangrijke rapporten, onderzoeken en bronnen over de noodzaak van machine-uitvoerbare wetgeving',
      items: [
        {
          title: 'Factsheet digitale uitvoering van wetgeving',
          meta: 'Prof. Corien Prins (WRR) & Prof. Johan Wolswinkel (Tilburg University) • 23 januari 2025',
          text: 'Deze WRR-factsheet identificeert vijf aandachtspunten en toetsvragen voor parlementaire controle op digitale uitvoering van wetgeving. Het RegelRecht-project valt binnen het domein van deze factsheet en kan worden beoordeeld aan de hand van de voorgestelde criteria voor transparantie, traceerbaarheid en democratische controle.',
          href: 'https://www.wrr.nl/actueel/nieuws/2025/01/23/factsheet-digitale-uitvoering-van-wetgeving',
          linkLabel: 'WRR-factsheet',
        },
        {
          title:
            "Factsheet rechtsstatelijke risico's van de digitale uitvoering van wetten",
          meta: 'Dr. Mariette Lokin (OU/VU) & prof. mr. dr. Reijer Passchier (OU/Universiteit Leiden) • 29 november 2024',
          text: "Deze factsheet voor de Vaste commissie Digitale Zaken van de Tweede Kamer benoemt zes rechtsstatelijke risico's van digitale wetsuitvoering, waaronder ondoorzichtigheid en vertaalproblemen tussen wettekst en code, en bepleit traceerbaarheid van algoritmen naar hun juridische bron.",
          href: 'https://www.eerstekamer.nl/bijlage/20250129/wetenschappelijke_factsheet/document3/f=/vmkgn0uje7le.pdf',
          linkLabel: 'PDF',
        },
        {
          title: 'Informatiehuishouding, de postkoets met hulpmotor',
          meta: 'Arre Zuurmond (Regeringscommissaris) • 1 mei 2023',
          text: 'Zuurmond signaleert dat de huidige informatiehuishouding een bureaucratische, reactieve overheid ondersteunt die te sterk gebaseerd is op wantrouwen jegens burgers. Hij pleit voor een responsieve overheid met betere informatievoorziening.',
          href: 'https://www.rijksoverheid.nl/documenten/rapporten/2023/05/01/rapportage-regeringscommissaris-informatiehuishouding',
          linkLabel: 'rapport',
        },
        {
          title: 'Algoritmes getoetst',
          meta: 'Algemene Rekenkamer • 18 mei 2022',
          text: "De Rekenkamer testte 9 algoritmes bij verschillende overheidsorganisaties en constateerde dat 6 daarvan risico's hadden op het gebied van prestatiebeheer, bias, datalekken of onbevoegde toegang. Het rapport benadrukt de noodzaak van continue monitoring.",
          href: 'https://www.rekenkamer.nl/publicaties/rapporten/2022/05/18/algoritmes-getoetst',
          linkLabel: 'rapport',
        },
        {
          title: 'Aandacht voor algoritmes',
          meta: 'Algemene Rekenkamer • 26 januari 2021',
          text: 'Dit eerste systematische onderzoek naar algoritmegebruik bij de Nederlandse overheid constateerde dat algoritmes zich vooral richten op overheidsbehoeften, met beperkte aandacht voor ethische aspecten en burgerinzicht.',
          href: 'https://www.rekenkamer.nl/publicaties/rapporten/2021/01/26/aandacht-voor-algoritmes',
          linkLabel: 'rapport',
        },
        {
          title: 'Aanbevelingen wetgevingsproces en wetgevingskwaliteit',
          meta: 'Raad van State (Afdeling advisering) • 19 april 2021',
          text: 'De Raad van State benadrukt het belang van uitvoeringstoetsen en samenwerking tussen beleidsmakers, wetgevingsjuristen en uitvoeringsorganisaties in multidisciplinaire teams, en pleit voor betere toetsing op uitvoerbaarheid en doenvermogen.',
          href: 'https://www.raadvanstate.nl/actueel/nieuws/@125178/aanbevelingen-wetgevingsproces',
          linkLabel: 'aanbevelingen',
        },
        {
          title:
            'Gematigde groei: Staatscommissie Demografische Ontwikkelingen 2050',
          meta: 'Staatscommissie o.v.v. Richard van Zwol • 15 januari 2024',
          text: 'De staatscommissie signaleert dat demografische ontwikkelingen leiden tot druk op toegankelijkheid van overheidsdiensten zoals onderwijs, zorg en huisvesting.',
          href: 'https://www.rijksoverheid.nl/documenten/rapporten/2024/01/15/gematigde-groei-rapport-van-de-staatscommissie-demografische-ontwikkleingen-2050',
          linkLabel: 'rapport',
        },
        {
          title: 'Maak waar! De digitale overheid',
          meta: 'Studiegroep Informatiesamenleving en Overheid (o.v.v. Richard van Zwol) • 18 april 2017',
          text: 'De studiegroep concludeert dat digitalisering van de overheid een radicale mentaliteitsverandering vereist en dat digitale dienstverlening tot de kern van het primaire proces hoort.',
          href: 'https://kennisopenbaarbestuur.nl/documenten/rapporten/2017/04/18/maak-waar',
          linkLabel: 'rapport',
        },
        {
          title: 'Werk aan Uitvoering, Fase 2: Handelingsperspectieven',
          meta: 'Interdepartementaal (BZK, Financiën, OCW, SZW) • 3 juli 2020',
          text: 'Dit rapport analyseert problemen bij uitvoeringsorganisaties zoals de Belastingdienst, DUO en UWV: continuïteitsrisico’s, beperkte wendbaarheid bij beleidswijzigingen, en ontbrekende mogelijkheden voor maatwerk.',
          href: 'https://www.rijksoverheid.nl/documenten/rapporten/2020/07/03/werk-aan-uitvoering-fase-2-handelingsperspectieven-en-samenvatting-analyse',
          linkLabel: 'rapport',
        },
        {
          title:
            'Open op orde: generiek actieplan informatiehuishouding Rijksoverheid',
          meta: 'Ministerie van BZK • 6 april 2021',
          text: "Dit actieplan werd opgesteld als reactie op het rapport 'Ongekend onrecht' en richt zich op structurele verbetering van de informatiehuishouding binnen de gehele rijksoverheid.",
          href: 'https://www.eerstekamer.nl/overig/20210406/open_op_orde_generiek_actieplan/document3/f=/vlhqp2mq5pvc.pdf',
          linkLabel: 'PDF',
        },
      ],
    },
    roadmap: {
      title: 'Waar dit heen gaat',
      lede: 'Het werk is verdeeld over vijf fases, van het vastleggen van de basis tot het borgen ervan in de staande praktijk. Elk werkpakket benoemt wat er moet gebeuren, wie eraan werkt en welke vragen nog open staan. De eerste fases zijn uitgewerkt; de latere staan er wel, maar zijn nog niet in werkpakketten verdeeld.',
      // A fase with nothing in it yet is a real state of the roadmap, not an
      // omission: the later fases are named but not broken down. Saying that
      // beats printing "0 werkpakketten", which reads as an empty plan.
      werkpakkettenLabel: (n) =>
        n === 0
          ? 'Nog niet uitgewerkt'
          : n === 1
            ? '1 werkpakket'
            : `${n} werkpakketten`,
      ctaLabel: 'Bekijk de roadmap',
      fases: {
        wat: 'Vastleggen wat machine-leesbare wetgeving is, en wat ze moet kunnen.',
        'wat-fase-2':
          'Uitzoeken waar de aanpak houdt en waar ze breekt, met onderzoek per rechtsgebied.',
        hoe: 'De gereedschappen bouwen waarmee juristen en uitvoerders het werk zelf kunnen doen.',
        waar: 'Beleggen waar dit thuishoort: bij welke organisaties, onder welke afspraken.',
        garantie:
          'Zorgen dat het blijft kloppen als wetten veranderen en mensen wisselen.',
      },
    },
    faq: {
      title: 'Veelgestelde vragen',
      // Item order is tuned so the two-column masonry layout (see
      // LandingSections.astro) ends up with roughly equal column heights:
      // the long answers are spread across both columns instead of stacking
      // in the left one. The cards read column-by-column, not row-by-row, so
      // this is a visual ordering, not a content priority. Reordering, adding
      // or shortening an answer changes the balance — re-check the layout if
      // you touch this list.
      items: [
        {
          q: 'Wat gebeurt er met open normen?',
          a: 'Wetten bevatten bewust ruimte voor interpretatie: termen die "bij ministeriële regeling" worden ingevuld, of begrippen die een afweging aan de uitvoerder laten. Bij gewone automatisering verdwijnt die ruimte stilzwijgend in code: de keuze die een programmeur maakt wordt feitelijk recht, zonder publicatie of toetsing. RegelRecht maakt zo\'n keuze juist expliciet: de wet markeert een open term, een regeling vult hem in, en juristen kunnen aantekenen of een begrip volledig, deels of nog niet is ingevuld. Zo wordt zichtbaar waar de wet eindigt en de interpretatie begint. Echte menselijke beoordelingen in een besluitproces, zoals een hardheidsclausule of een individuele afweging door een ambtenaar, blijven gewoon menselijk werk; die proberen we niet weg te automatiseren.',
        },
        {
          q: 'Wat zou een digitaal rechtsstelsel kunnen betekenen?',
          a: 'Juridische regels worden dan geschreven als uitvoerbare code die computers direct kunnen draaien en toepassen, zonder tussenkomst van menselijke interpretatie of programmeurs. Is dat realiseerbaar? En hoe verhoudt het zich tot traditioneel analoog recht?',
        },
        {
          q: 'Hoe zou dit zich kunnen verhouden tot bestaande systemen?',
          a: 'Kan RegelRecht bestaande implementaties valideren en dienen als referentie voor nieuwe systemen? Bestaande systemen worden niet direct vervangen, maar controle en modernisering komen wel in beeld.',
        },
        {
          q: 'Zou RegelRecht juridisch bindend kunnen zijn?',
          a: 'RegelRecht is een technisch hulpmiddel. De juridische geldigheid blijft bij de oorspronkelijke wetgeving. De vraag is of het kan helpen bij consistente interpretatie en toepassing.',
        },
        {
          q: 'Betekent dit één centrale interpretatie?',
          a: 'Nee. RegelRecht verkent of de manier waarop een wet wordt uitgevoerd publiek gepubliceerd kan worden, niet of er één partij is die bepaalt wat de waarheid is. Het bevoegd gezag publiceert zijn lezing als gezaghebbende interpretatie, maar andere organisaties, juristen en burgers kunnen hun lezing daarnaast publiceren. De executie wordt bovendien niet aan één engine opgehangen: meerdere onafhankelijke implementaties moeten op dezelfde regels en data dezelfde uitkomst geven. Wat centraal wordt is de publicatie en controleerbaarheid, niet de interpretatie zelf.',
        },
        {
          q: 'Moet ik dan jullie Rust-engine in mijn landschap draaien?',
          a: 'Nee. RegelRecht legt het regelformaat en de bedoelde uitkomsten vast, niet de implementatie. De Rust-engine is de referentie-implementatie: hij laat zien wat de regels horen te doen en dient als ijkpunt waartegen andere implementaties zich kunnen meten. Een engine kan ook anders geïmplementeerd worden, in een andere taal of architectuur, zodat hij in jouw landschap past. Wat telt is dat elke implementatie op dezelfde regels en data dezelfde uitkomst geeft als de referentie.',
        },
        {
          q: 'Waarom een eigen regelformaat?',
          a: 'Het formaat is YAML met wettekst en machine-uitvoerbare regels naast elkaar. Een regeling in dat formaat heet een regelwerk. Een versioned JSON Schema bewaakt de structuur, BDD-scenario’s leggen de bedoelde uitkomsten vast. Zo kunnen juristen meelezen, ontwikkelaars meebouwen, en verschillende overheidssystemen dezelfde regels gebruiken.',
          link: { label: 'Lees RFC-011', href: '/rfcs/rfc-011' },
        },
        {
          q: 'Hoe draagt dit bij aan transparantie?',
          a: 'Door regels expliciet te maken in code kunnen burgers en organisaties exact zien hoe beslissingen tot stand komen, in plaats van te vertrouwen op ondoorzichtige systemen.',
        },
      ],
    },
    jobs: {
      title: 'Werk mee aan RegelRecht',
      lede: 'Het team rond RegelRecht groeit. Bouw mee aan een open infrastructuur waarin Nederlandse wetgeving machine-uitvoerbaar wordt, en zie je werk landen bij uitvoeringsorganisaties die er dagelijks beslissingen op nemen.',
      vacancyTag: 'Vacature',
      contactsLabel: 'Vragen? Neem contact op met',
      closesLabel: { open: 'Sluit', closed: 'Gesloten' },
      items: [
        {
          title: 'Software Engineer',
          organisation: 'Rijksorganisatie ODI · Ministerie van BZK',
          pitch:
            'Werk aan de Rust-engine en de tooling waarmee wetten machine-uitvoerbaar worden. Je adviseert opdrachtgevers binnen het Rijk, ontwerpt en programmeert, en werkt naast juristen die de regels in machineleesbare vorm gieten.',
          meta: ['Schaal 13', '€5.212 – €7.747', '32 – 36 uur', 'Den Haag'],
          closes: '2026-06-11',
          ctaLabel: 'Bekijk de vacature',
          ctaHref:
            'https://www.werkenvoornederland.nl/vacatures/software-engineer-BZK-2026-8544',
          contacts: [
            { label: 'Abram Klop (opgavemanager)', href: 'tel:+31650035732' },
            { label: 'Dian Hoppen (recruiter)', href: 'tel:+31650062738' },
          ],
        },
        {
          title: 'Redesign Analist',
          organisation: 'Rijksorganisatie ODI · Ministerie van BZK',
          pitch:
            'Analyseer wet- en regelgeving en vertaal die naar machine-uitvoerbare specificaties. Je ontleedt complexe uitvoeringsprocessen en werkt in multidisciplinaire teams samen met engineers die de regels in code omzetten.',
          meta: ['Schaal 12', '€4.691 – €6.907', '32 – 36 uur', 'Den Haag'],
          closes: '2026-06-24',
          ctaLabel: 'Bekijk de vacature',
          ctaHref:
            'https://www.werkenvoornederland.nl/vacatures/redesign-analist-BZK-2026-9806',
          contacts: [
            { label: 'Abram Klop (opgavemanager)', href: 'tel:+31650035732' },
            {
              label: 'Suzanne de Jager (recruiter)',
              href: 'tel:+31627295317',
            },
          ],
        },
      ],
    },
    feedback: {
      title: 'Denk mee over deze verkenning',
      body: 'Deze verkenning van machine-uitvoerbare wetgeving roept veel vragen op. Hoe zie jij de toekomst van de digitale overheid? Wat zijn je zorgen en verwachtingen bij deze ontwikkelingen? Jouw input helpt ons deze verkenning verder vorm te geven.',
      cta: 'Meld je aan of deel je gedachten',
      ctaHref: SIGNUP_NL_PATH,
    },
    footer: {
      blurb:
        'Een verkenning naar transparante, uitvoerbare wetgeving, en een van de drie projecten uit de startselectie van de Nederlandse Digitale Dienst.',
      linksTitle: 'Links',
      contactTitle: 'Contact',
      partOfTitle: 'Onderdeel van',
      links: [
        { label: 'GitHub-repository', href: GITHUB },
        { label: 'Hoe het werkt', href: '/#how-it-works' },
        { label: 'Op de hoogte blijven', href: SIGNUP_NL_PATH },
        { label: 'Roadmap', href: '/roadmap' },
        { label: 'Documentatie (Engels)', href: '/docs/' },
        { label: 'Onderzoek (Engels)', href: '/research/' },
      ],
      partOf: [
        {
          label: 'Nederlandse Digitale Dienst',
          href: 'https://digitaledienst.overheid.nl/',
        },
        { label: 'Ministerie van Economische Zaken en Klimaat' },
      ],
    },
    signup: {
      pageTitle: 'Op de hoogte blijven of meedenken',
      metaDescription: 'Laat je gegevens achter als je updates wilt ontvangen of wilt meedenken over de juridische validatie van RegelRecht.',
      noscript:
        'Dit formulier heeft JavaScript nodig. Stuur in plaats daarvan een e-mail naar regelrecht@minbzk.nl.',
      radioYes: 'Ja, ik wil meedenken en updates ontvangen',
      radioNo: 'Nee, alleen updates ontvangen',
      emailLabel: 'E-mailadres',
      nameLabel: 'Volledige naam',
      orgLabel: 'Organisatie',
      orgPlaceholder: 'Bijv. Ministerie van BZK, Gemeente Amsterdam',
      roleLabel: 'Functie',
      rolePlaceholder: 'Bijv. jurist, beleidsmedewerker, wetgevingsjurist',
      required: '*',
      requiredSr: '(verplicht)',
      submit: 'Meld me aan',
      submitting: 'Bezig met versturen…',
      companyHoneypot: 'Bedrijf (niet invullen)',
      errEmailEmpty: 'Vul je e-mailadres in. Hiermee sturen we je updates en kunnen we contact opnemen.',
      errEmailInvalid: 'Controleer je e-mailadres, er lijkt een @ of een punt te ontbreken.',
      errName: 'Vul je volledige naam in. Zodat we weten wie zich aanmeldt.',
      successTitle: 'Bedankt voor je aanmelding!',
      successBody:
        'We hebben je gegevens verstuurd. Je ontvangt bevestiging per e-mail zodra je aanmelding is verwerkt. Klopt er iets niet? Mail ons.',
      successReset: 'Nog iemand aanmelden',
      errorTitle: 'Er ging iets mis',
      errorBody:
        'Het versturen is niet gelukt. Probeer het opnieuw of stuur een e-mail naar regelrecht@minbzk.nl.',
      errorReset: 'Opnieuw proberen',
    },
  },

  en: {
    meta: {
      title: 'RegelRecht: from statute to digital execution',
      description:
        'A project of the Nederlandse Digitale Dienst exploring whether legislation can be written as executable code.',
    },
    nav: {
      brandTitle: 'Nederlandse Digitale Dienst',
      brandMinistry: 'Ministry of Economic Affairs and Climate Policy',
      home: 'Home',
      what: 'What',
      how: 'How',
      tools: 'Ecosystem',
      example: 'Examples',
      faq: 'Questions',
      jobs: 'Join us',
      signup: 'Sign up',
      docs: 'Documentation',
      research: 'Research',
      roadmap: 'Roadmap',
    },
    hero: {
      title: 'From statute to digital execution',
      intro:
        'RegelRecht explores whether legislation can be written as executable code, so that different organisations apply the same law the same way and citizens can follow how a decision is reached.',
    },
    whatIsIt: {
      title: 'Running laws without programming',
      lede: 'Executing legislation comes with several challenges: differing interpretations, opaque systems, and complex programming work that often sits far from the original law. RegelRecht explores whether machine-executable legislation can offer an answer: laws written directly as executable code, without programmers in between.',
      cards: [
        {
          h: 'From analogue law to code',
          p: 'Can we turn traditional legislation into ruleworks, in which the legal text and its executable interpretation sit side by side? We are investigating whether this can narrow the gap between legislator and execution.',
        },
        {
          h: 'Shared, verifiable execution',
          p: 'What if the way a law is executed were published in machine-executable form alongside the legal text itself? Different parties, from implementing organisations to citizens, could then independently verify and run that execution, and differences in interpretation would become visible instead of being hidden inside code.',
        },
        {
          h: 'Full transparency',
          p: 'How do we make government decisions more transparent? We are experimenting with ways for citizens to see which rules apply and how decisions are reached.',
        },
      ],
    },
    whyImportant: {
      title: 'Applying laws currently raises several challenges for the rule of law',
      lede: 'We are investigating whether new technical approaches can contribute to solutions for these structural questions.',
      problemSolutions: [
        {
          problemTitle: 'Differing interpretations',
          problemText:
            'The same law is interpreted and applied differently by different government organisations, leading to inconsistencies and injustice.',
          solutionTitle: 'Unambiguous application',
          solutionText:
            'Could machine-executable laws reduce interpretation problems? We are investigating whether this can lead to more consistent rule application.',
        },
        {
          problemTitle: 'Opaque decisions',
          problemText:
            'Citizens receive decisions with no explanation of how they were reached. Government as a black box.',
          solutionTitle: 'Full traceability',
          solutionText:
            'Can we make every decision traceable back to the exact rule that was applied? We are exploring options for more transparency in government decisions.',
        },
        {
          problemTitle: 'Unworkable laws',
          problemText:
            'Laws are often written without fully testing whether they are workable in practice. This can cause implementation problems through inconsistencies, ambiguities or practical constraints.',
          solutionTitle: 'Testing workability',
          solutionText:
            'Would machine-executable legislation make it possible to test laws? We are investigating whether inconsistencies and conflicts can be detected early.',
        },
      ],
    },
    howItWorks: {
      title: 'The transition from analog law to a digital legal system',
      lede: 'How might that unfold? We explore 7 possible steps and what each could make possible:',
      steps: [
        {
          title: 'Analogue to digital',
          text: 'Can existing laws be systematically converted from analogue text into ruleworks? A first step to explore a digital foundation.',
        },
        {
          title: 'Digital legal system',
          text: 'Could new laws be written machine-executable from the start? We explore what that could look like and how we can support it.',
        },
        {
          title: 'Standardised ecosystem',
          text: 'A national infrastructure where government systems use the same legal definitions. A shared, published baseline that every party works from and can independently verify.',
        },
        {
          title: 'Harmonising legislation',
          text: 'It becomes possible to work systematically on harmonising existing legislation. Conflicts and inconsistencies between existing rule sets can be detected automatically, making harmonisation a deliberate choice.',
        },
        {
          title: 'Workability assessment',
          text: 'New laws can be tested before they take effect. The effect of new legislation on the consistency of the legal system can be analysed during the legislative process.',
        },
        {
          title: 'Central publication',
          text: 'Machine-executable legislation is published centrally for everyone. Execution engines are made available too, so all parties can run the same laws in an identical way.',
        },
        {
          title: 'Transparent application',
          text: 'Citizens and businesses can inspect and verify exactly how rules work. Full transparency into rule application.',
        },
      ],
    },
    tools: {
      title: 'Ecosystem',
      items: [
        {
          title: 'Rule format',
          meta: 'YAML + JSON Schema',
          link: { label: 'Schema reference', href: '/reference/schema' },
          text: 'Every regulation as a rulework: YAML with the legal text and the machine-executable rules side by side, one file per version. A versioned JSON Schema guards the structure.',
        },
        {
          title: 'BDD scenarios',
          meta: 'Gherkin + cucumber',
          link: { label: 'Try a scenario', href: '/concepts/scenarios' },
          text: 'Expected outcomes are captured as readable scenarios. Legal experts and programmers read the same tests, and every change to the rules is validated immediately. Where possible we draw those scenarios straight from the explanatory memorandum.',
        },
        {
          title: 'Execution engine',
          meta: 'Rust + WebAssembly',
          link: { label: 'Documentation', href: '/components/engine' },
          text: 'A deterministic execution engine that runs the YAML rules. Written in Rust and compiled to WebAssembly so the same rules give the same result in the browser and on the server.',
        },
        {
          title: 'Analogue-law converter',
          meta: 'AI-assisted',
          text: 'An LLM-based tool that could turn existing analogue law into machine-executable rules. We are exploring what automatic transformation could look like.',
        },
        {
          title: 'Editor',
          meta: 'Work in progress',
          link: { label: 'Documentation', href: '/components/editor-api' },
          text: 'A working environment for legal experts to make laws machine-executable. We are still discovering what this editor should look like.',
        },
        {
          title: 'Law graph',
          meta: 'Relationship analysis',
          text: 'A visualisation of the relationships between different laws, that could show how changes to one law would ripple through the wider legal landscape.',
        },
        {
          title: 'Corpus',
          meta: 'Git-based',
          link: { label: 'Documentation', href: '/components/corpus' },
          text: 'The library of machine-executable rules. Git handles the version history; a registry ties different sources into a single whole.',
        },
        {
          title: 'Simulation environment',
          meta: 'What-if analysis',
          link: {
            label: 'Live demo',
            href: 'https://demo.regelrecht.rijks.app/simulatie',
          },
          text: 'An environment where the consequences of new legislation could be modelled before it takes effect, to surface societal impact and unintended effects.',
        },
        {
          title: 'Publication platform',
          meta: 'API + web',
          text: 'A central place for publication and distribution of machine-executable rules, with API access for government systems and private parties.',
        },
      ],
    },
    example: {
      title: 'A few directions for practice',
      lede: 'What could the RegelRecht ecosystem make possible? A handful of directions for transparent rule application, legislative testing, and the working environment of the legal experts themselves.',
      cases: [
        {
          img: '/portaal-en-screenshot.png',
          alt: 'Screenshot of a personal rules dashboard: a list of benefits and allowances where each rule shows its origin in the law.',
          h: 'Personal rules dashboard',
          p: 'What if citizens could see all their benefits, allowances and obligations in one place? Every rule could then be traceable back to the machine-executable legislation, with full transparency about how decisions are reached.',
          bullets: [
            'Real-time rule application: could it make immediate feedback possible?',
            'Full traceability: can a path be drawn from law to personal situation?',
            'Proactive communication: can citizens be informed automatically when rules change?',
          ],
          link: {
            label: 'See it in the demo',
            href: 'https://demo.regelrecht.rijks.app/portaal',
          },
        },
        {
          img: '/simulatie-screenshot.png',
          alt: 'Screenshot of a simulation environment that computes the effect of a legislative change across different example situations.',
          h: 'Legislative simulation & testing',
          p: 'What if policy makers could test the consequences of new legislation in a simulation environment before it is introduced? Could this prevent unintended effects and improve the quality of legislation?',
          bullets: [
            'Impact analysis: could we predict the consequences of new regulation?',
            'Harmonisation check: can we detect conflicts with existing legislation?',
            'Scenario testing: is it possible to test different policy options?',
            'Quality control: can inconsistencies be detected before implementation?',
          ],
          reverse: true,
          link: {
            label: 'See it in the demo',
            href: 'https://demo.regelrecht.rijks.app/simulatie',
          },
        },
        {
          img: '/editor-notities-screenshot.png',
          alt: 'Screenshot of the RegelRecht editor: the text of the Health-Care Allowance Act on the left, machine-readable definitions and outputs in the middle, and scenarios with expected outcomes on the right. A note popup is open on a selected term.',
          h: 'Editor with notes and scenarios',
          p: 'What if legal experts, policy makers and programmers could work on legislation in the same environment? Notes on terms, machine-readable definitions and runnable scenarios, all alongside the original legal text.',
          bullets: [
            'Term-level annotation: can lawyers add explanations directly to specific terms?',
            'Live scenarios: do we see immediately whether the rules still hold after a change?',
            'Multiple laws side by side: can we surface cross-references between laws?',
          ],
        },
      ],
    },
    innovation: {
      title: 'RegelRecht contributes to projects from the 2025 Innovation Budget',
      cards: [
        {
          meta: 'In collaboration with VNG',
          h: 'Fewer citizens in trouble through machine-readable legislation',
          p: 'How do we prevent the accumulation of laws and regulations from making laws unworkable? This project explores developing an analysis tool to test legislative proposals for workability in conjunction with other laws.',
        },
        {
          meta: 'In collaboration with Dienst Toeslagen',
          h: 'A modern calculation core as a building block for government',
          p: 'Can we develop a general calculation core for government? This project explores how such a system could help execute complex schemes for citizens and businesses, for example when calculating allowances.',
        },
      ],
    },
    research: {
      title: 'Research',
      lede: 'This exploration raises questions that reach beyond technology: what it means for the balance between the branches of government when law execution becomes public and checkable, and what legal status a rulework has. We write that up for an academic audience, so that legal and public-administration scholars can test and sharpen it.',
      items: [
        {
          title: paperTitle,
          meta: `${paper.kind} • ${paperAuthors} • ${paperDate('en-GB')}`,
          text: 'The paper argues that government should publish its law execution as machine-executable specifications, with every decision recording the version that produced it. The recipient of a decision can then recompute it, and Parliament can analyze the rule itself.',
          href: '/research/rules-as-executed',
          linkLabel: 'position paper',
          also: { href: '/research/rules-as-executed/explained', linkLabel: 'plain-language explainer' },
        },
        {
          title: jurixTitle,
          meta: `${jurixPaper.venue} paper • ${jurixAuthors} • ${paperDate('en-GB', jurixPaper.date)}`,
          text: "A language model can explain the engine's decision to a citizen, but it also invents reasons. The paper compares three ways of handing the model the engine's trace, across five language models and three Dutch laws. The more structured the input, the more faithful the explanation, yet citizens do not always prefer the most faithful explanation.",
          href: '/research/citizen-explanation',
          linkLabel: 'paper',
        },
        {
          title: promisedPaper.title,
          meta: `${promisedPaper.venue} paper • ${promisedAuthors} • ${paperDate('en-GB', promisedPaper.date)}`,
          text: "Tests for an encoded statute usually take their expected outcomes from whoever encoded it, so they check the encoder's own reading. This paper takes them from the worked examples with which the government shows parliament what a bill should yield, and runs fifteen against the corpus: eleven give the promised amount.",
          href: '/research/rules-as-promised',
          linkLabel: 'paper',
        },
        {
          title: 'Rules as Executed: When machine-executable laws are published',
          meta: 'JURIX 2026 tutorial • Anne Schuth & Eelco Hotting • 8 December 2026',
          text: 'Ninety minutes with the demo in front of a room of researchers in law and computer science. We open Dutch statutes next to their executable specification, turn a threshold and watch who loses an allowance. Being argued with is the point.',
          href: '/research/jurix-2026-tutorial',
          linkLabel: 'about the tutorial',
        },
      ],
    },
    references: {
      title: 'Important reports, studies, and sources on the need for machine-executable legislation',
      items: [
        {
          title: 'Factsheet on the digital execution of legislation',
          meta: 'Prof. Corien Prins (WRR) & Prof. Johan Wolswinkel (Tilburg University) • 23 January 2025',
          text: 'This WRR factsheet identifies five points of attention and review questions for parliamentary oversight of the digital execution of legislation. The RegelRecht project falls within the scope of this factsheet and can be assessed against the proposed criteria for transparency, traceability and democratic control.',
          href: 'https://www.wrr.nl/actueel/nieuws/2025/01/23/factsheet-digitale-uitvoering-van-wetgeving',
          linkLabel: 'WRR factsheet',
        },
        {
          title:
            'Factsheet on rule-of-law risks of the digital execution of laws',
          meta: 'Dr. Mariette Lokin (OU/VU) & Prof. Reijer Passchier (OU/Leiden University) • 29 November 2024',
          text: 'This factsheet for the House of Representatives’ Standing Committee on Digital Affairs names six rule-of-law risks of digital law execution, including opacity and translation problems between legal text and code, and argues for traceability of algorithms back to their legal source.',
          href: 'https://www.eerstekamer.nl/bijlage/20250129/wetenschappelijke_factsheet/document3/f=/vmkgn0uje7le.pdf',
          linkLabel: 'PDF',
        },
        {
          title: 'Information management, the stagecoach with an auxiliary motor',
          meta: 'Arre Zuurmond (Government Commissioner) • 1 May 2023',
          text: 'Zuurmond observes that current information management supports a bureaucratic, reactive government too strongly based on distrust of citizens. He argues for a responsive government with better information provision.',
          href: 'https://www.rijksoverheid.nl/documenten/rapporten/2023/05/01/rapportage-regeringscommissaris-informatiehuishouding',
          linkLabel: 'report',
        },
        {
          title: 'Algorithms tested',
          meta: 'Netherlands Court of Audit • 18 May 2022',
          text: 'The Court of Audit tested 9 algorithms at various government organisations and found that 6 of them carried risks around performance management, bias, data leaks or unauthorised access. The report stresses the need for continuous monitoring.',
          href: 'https://www.rekenkamer.nl/publicaties/rapporten/2022/05/18/algoritmes-getoetst',
          linkLabel: 'report',
        },
        {
          title: 'Attention to algorithms',
          meta: 'Netherlands Court of Audit • 26 January 2021',
          text: 'This first systematic study of algorithm use by the Dutch government found that algorithms focus mainly on government needs, with limited attention to ethical aspects and citizen insight.',
          href: 'https://www.rekenkamer.nl/publicaties/rapporten/2021/01/26/aandacht-voor-algoritmes',
          linkLabel: 'report',
        },
        {
          title: 'Recommendations on the legislative process and quality',
          meta: 'Council of State (Advisory Division) • 19 April 2021',
          text: 'The Council of State stresses the importance of implementation assessments and collaboration between policy makers, legislative lawyers and implementing organisations in multidisciplinary teams, and argues for better testing of workability and citizens’ ability to act.',
          href: 'https://www.raadvanstate.nl/actueel/nieuws/@125178/aanbevelingen-wetgevingsproces',
          linkLabel: 'recommendations',
        },
        {
          title:
            'Moderate growth: State Commission on Demographic Developments 2050',
          meta: 'State Commission chaired by Richard van Zwol • 15 January 2024',
          text: 'The State Commission observes that demographic developments put pressure on the accessibility of government services such as education, healthcare and housing.',
          href: 'https://www.rijksoverheid.nl/documenten/rapporten/2024/01/15/gematigde-groei-rapport-van-de-staatscommissie-demografische-ontwikkleingen-2050',
          linkLabel: 'report',
        },
        {
          title: 'Make it happen! The digital government',
          meta: 'Study Group on the Information Society and Government (chaired by Richard van Zwol) • 18 April 2017',
          text: 'The study group concludes that digitising government requires a radical change of mindset and that digital service delivery belongs at the core of the primary process.',
          href: 'https://kennisopenbaarbestuur.nl/documenten/rapporten/2017/04/18/maak-waar',
          linkLabel: 'report',
        },
        {
          title: 'Work on Implementation, Phase 2: Courses of action',
          meta: 'Interdepartmental (BZK, Finance, OCW, SZW) • 3 July 2020',
          text: 'This report analyses problems at implementing organisations such as the Tax Administration, DUO and UWV: continuity risks, limited agility when policy changes, and missing options for tailored solutions.',
          href: 'https://www.rijksoverheid.nl/documenten/rapporten/2020/07/03/werk-aan-uitvoering-fase-2-handelingsperspectieven-en-samenvatting-analyse',
          linkLabel: 'report',
        },
        {
          title:
            'Open and in order: generic action plan for information management',
          meta: 'Ministry of the Interior • 6 April 2021',
          text: "This action plan was drawn up in response to the 'Unprecedented injustice' report and focuses on structurally improving information management across central government.",
          href: 'https://www.eerstekamer.nl/overig/20210406/open_op_orde_generiek_actieplan/document3/f=/vlhqp2mq5pvc.pdf',
          linkLabel: 'PDF',
        },
      ],
    },
    roadmap: {
      title: 'Where this is going',
      lede: 'The work is split across five phases, from setting down the basics to embedding them in everyday practice. Each work package states what needs to happen, who is working on it and which questions are still open. The early phases are worked out; the later ones are named but not yet broken down. The roadmap itself is in Dutch.',
      werkpakkettenLabel: (n) =>
        n === 0
          ? 'Not yet broken down'
          : n === 1
            ? '1 work package'
            : `${n} work packages`,
      ctaLabel: 'View the roadmap (Dutch)',
      fases: {
        wat: 'Setting down what machine-readable legislation is, and what it has to be able to do.',
        'wat-fase-2':
          'Working out where the approach holds and where it breaks, with research per area of law.',
        hoe: 'Building the tools that let lawyers and implementing bodies do the work themselves.',
        waar: 'Establishing where this belongs: which organisations, under which agreements.',
        garantie:
          'Making sure it keeps holding up as laws change and people move on.',
      },
    },
    faq: {
      title: 'Frequently asked questions',
      // Item order is tuned for the two-column masonry layout (see
      // LandingSections.astro) so the columns end up roughly equal in height;
      // the long answers are spread across both columns. Cards read
      // column-by-column, not row-by-row, so this is a visual ordering, not a
      // content priority. Mirrors the Dutch list; re-check the layout if you
      // reorder, add or shorten an answer. Keep both lists in sync.
      items: [
        {
          q: 'What happens to open norms?',
          a: 'Laws deliberately leave room for interpretation: terms that are filled in "by ministerial regulation", or concepts that leave a judgement to the implementing body. In ordinary automation that room quietly disappears into code: the choice the programmer makes effectively becomes law, with no publication or scrutiny. RegelRecht turns that choice into something explicit instead: the law marks an open term, a regulation fills it in, and lawyers can record whether a concept is fully, partly or not yet filled in. That makes visible where the statute ends and interpretation begins. Genuinely human judgements inside a decision process, such as a hardship clause or a case-by-case assessment by an official, stay human work; we are not trying to automate those away.',
        },
        {
          q: 'What could a digital legal system mean?',
          a: 'Legal rules are written as executable code that computers can run and apply directly, without human interpretation or programmers in between. Is that achievable? And how does it relate to traditional analogue law?',
        },
        {
          q: 'How could this relate to existing systems?',
          a: 'Can RegelRecht validate existing implementations and serve as a reference for new systems? Existing systems are not directly replaced, but verification and modernisation come within reach.',
        },
        {
          q: 'Could RegelRecht be legally binding?',
          a: 'RegelRecht is a technical aid. Legal validity remains with the original legislation. The open question is whether it can help with consistent interpretation and application.',
        },
        {
          q: 'Does this mean one central interpretation?',
          a: 'No. RegelRecht explores whether the way a law is executed can be published, not whether one party decides what is true. The competent authority publishes its reading as the authoritative interpretation, but other organisations, lawyers and citizens can publish their reading alongside it. Execution is not tied to a single engine either: multiple independent implementations must produce the same outcome on the same rules and data. What becomes central is publication and verifiability, not interpretation itself.',
        },
        {
          q: 'Do I have to run your Rust engine in my own landscape?',
          a: 'No. RegelRecht fixes the rule format and the intended outcomes, not the implementation. The Rust engine is the reference implementation: it shows what the rules are meant to do and serves as the benchmark other implementations can measure themselves against. An engine can also be implemented differently, in another language or architecture, so that it fits your landscape. What matters is that every implementation produces the same outcome on the same rules and data as the reference.',
        },
        {
          q: 'Why a dedicated rule format?',
          a: 'The format is YAML, with legal text and machine-executable rules side by side. A regulation in that format is called a rulework. A versioned JSON Schema guards the structure, and BDD scenarios capture the intended outcomes. Legal experts can read along, developers can contribute, and different government systems can use the same rules.',
          link: { label: 'Read RFC-011', href: '/rfcs/rfc-011' },
        },
        {
          q: 'How does this contribute to transparency?',
          a: 'By making rules explicit in code, citizens and organisations can see exactly how decisions are reached, instead of relying on opaque systems.',
        },
      ],
    },
    jobs: {
      title: 'Join the RegelRecht team',
      lede: 'The team behind RegelRecht is growing. Help build an open infrastructure that turns Dutch legislation into something computers can execute, and see your work land at the public-sector organisations that act on it every day.',
      vacancyTag: 'Vacancy',
      contactsLabel: 'Questions? Get in touch with',
      closesLabel: { open: 'Closes', closed: 'Closed' },
      items: [
        {
          title: 'Software Engineer',
          organisation: 'Rijksorganisatie ODI · Ministry of the Interior',
          pitch:
            'Work on the Rust engine and the tooling that turns Dutch statutes into something computers can run. You advise teams across the Dutch government, design and write code, and work side by side with lawyers who translate rules into machine-readable form. Senior role, in Dutch (fluency required).',
          meta: ['Scale 13', '€5,212 – €7,747', '32 – 36 hours', 'The Hague'],
          closes: '2026-06-11',
          ctaLabel: 'View the vacancy (Dutch)',
          ctaHref:
            'https://www.werkenvoornederland.nl/vacatures/software-engineer-BZK-2026-8544',
          contacts: [
            { label: 'Abram Klop (opgavemanager)', href: 'tel:+31650035732' },
            { label: 'Dian Hoppen (recruiter)', href: 'tel:+31650062738' },
          ],
        },
        {
          title: 'Redesign Analyst',
          organisation: 'Rijksorganisatie ODI · Ministry of the Interior',
          pitch:
            'Analyse Dutch legislation and translate it into machine-executable specifications. You dissect complex government processes and work in multidisciplinary teams alongside the engineers who turn the rules into code. In Dutch (fluency required).',
          meta: ['Scale 12', '€4,691 – €6,907', '32 – 36 hours', 'The Hague'],
          closes: '2026-06-24',
          ctaLabel: 'View the vacancy (Dutch)',
          ctaHref:
            'https://www.werkenvoornederland.nl/vacatures/redesign-analist-BZK-2026-9806',
          contacts: [
            { label: 'Abram Klop (opgavemanager)', href: 'tel:+31650035732' },
            {
              label: 'Suzanne de Jager (recruiter)',
              href: 'tel:+31627295317',
            },
          ],
        },
      ],
    },
    feedback: {
      title: 'Help shape this exploration',
      body: 'This exploration of machine-executable legislation raises many questions. How do you see the future of digital government? What are your concerns and expectations around these developments? Your input helps us shape this exploration further.',
      cta: 'Sign up or share your thoughts',
      ctaHref: SIGNUP_EN_PATH,
    },
    footer: {
      blurb:
        'An exploration into transparent, executable legislation, and one of the three projects in the starting selection of the Nederlandse Digitale Dienst.',
      linksTitle: 'Links',
      contactTitle: 'Contact',
      partOfTitle: 'Part of',
      links: [
        { label: 'GitHub repository', href: GITHUB },
        { label: 'How it works', href: '/en/#how-it-works' },
        { label: 'Stay informed', href: SIGNUP_EN_PATH },
        { label: 'Roadmap (Dutch)', href: '/roadmap' },
        { label: 'Documentation', href: '/docs/' },
        { label: 'Research', href: '/research/' },
      ],
      partOf: [
        {
          label: 'Nederlandse Digitale Dienst',
          href: 'https://digitaledienst.overheid.nl/',
        },
        { label: 'Ministry of Economic Affairs and Climate Policy' },
      ],
    },
    signup: {
      pageTitle: 'Stay informed or help think along',
      metaDescription: 'Leave your details to receive updates, or to help think about the legal validation of RegelRecht.',
      noscript:
        'This form needs JavaScript. Please send an email to regelrecht@minbzk.nl instead.',
      radioYes: 'Yes, I want to help think along and receive updates',
      radioNo: 'No, only receive updates',
      emailLabel: 'Email address',
      nameLabel: 'Full name',
      orgLabel: 'Organisation',
      orgPlaceholder: 'E.g. Ministry of the Interior, City of Amsterdam',
      roleLabel: 'Role',
      rolePlaceholder: 'E.g. lawyer, policy officer, legislative lawyer',
      required: '*',
      requiredSr: '(required)',
      submit: 'Sign me up',
      submitting: 'Sending…',
      companyHoneypot: 'Company (do not fill in)',
      errEmailEmpty: 'Enter your email address. We use this to send you updates and to get in touch.',
      errEmailInvalid: 'Check your email address, an @ or a dot seems to be missing.',
      errName: 'Enter your full name. So we know who is signing up.',
      successTitle: 'Thank you for signing up!',
      successBody:
        'We have sent your details. You will receive confirmation by email once your registration is processed. Something not right? Email us.',
      successReset: 'Sign up someone else',
      errorTitle: 'Something went wrong',
      errorBody:
        'Sending failed. Please try again or send an email to regelrecht@minbzk.nl.',
      errorReset: 'Try again',
    },
  },
}

export function langFromPath(path: string): 'nl' | 'en' {
  return path.startsWith('/en/') || path === '/en' ? 'en' : 'nl'
}
