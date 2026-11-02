/**
 * The short page (KortPagina.vue), as data: what RegelRecht is and what
 * chronolexografie adds to it, on one screen. Two short lists, a picture of
 * the two layers (KortSchema.vue), and one case whose steps carry the numbers
 * of the arrows in the picture. The button at the bottom opens the
 * explanation page (uitleg.js) for the long version.
 *
 * The words follow the deck (slides.js) and the explanation page: cel,
 * kroniek, vastlegging, reductie, open begrip, beleid. The steps of the case
 * follow the files of the case (`casus`); tests/kort.test.js checks that each
 * name in code is in them. In a sentence, `name` is code, as in the deck.
 */

/** The case the steps follow, which the button under them opens. */
export const casus = 'gehandicaptenparkeerkaart';

export const intro = {
  overline: 'Chronolexografie',
  title: 'In het kort',
  lead:
    'RegelRecht legt wetten vast als regels die een computer kan uitvoeren. ' +
    'Met chronolexografie telt bij een besluit mee wat een organisatie eerder vastlegde.',
};

/**
 * The picture: two bands, each with its boxes, and the arrows between them.
 * Coordinates are in the viewBox of KortSchema.vue. An arrow with `nummer`
 * carries a badge at `badge`, the number of its step in the case below.
 */
export const schema = {
  titel: 'Schema van RegelRecht en chronolexografie',
  breedte: 990,
  hoogte: 640,
  lagen: [
    { id: 'rr', label: 'RegelRecht', y: 0, hoogte: 250 },
    { id: 'cl', label: 'Chronolexografie', y: 270, hoogte: 370 },
  ],
  // The policy is a regelwerk of RegelRecht (its layer UITVOERINGSBELEID) that
  // chronolexografie binds to the answer of a cell, so it stands on the line
  // between the bands, in the style of neither.
  vakken: [
    { laag: 'rr', x: 420, y: 30, h: 54, titel: 'Schema', sub: 'schema/latest' },
    { laag: 'rr', x: 20, y: 120, titel: 'Wettekst', sub: 'BWB · CVDR' },
    { laag: 'rr', x: 220, y: 120, titel: 'Harvester, enrich', sub: 'harvester, pipeline' },
    { laag: 'rr', x: 420, y: 120, titel: 'Corpus', sub: 'regelwerken (YAML)' },
    { laag: 'rr', x: 620, y: 120, titel: 'Engine', sub: 'Rust · WASM · trace' },
    { laag: 'rr', x: 820, y: 120, titel: 'Editor, demo', sub: 'frontend · frontend-demo' },
    { laag: 'grens', x: 620, y: 228, titel: 'Uitvoeringsbeleid', sub: 'source.reduction' },
    { laag: 'cl', x: 220, y: 360, titel: 'Indiener', sub: 'aanvraag · bezwaar' },
    { laag: 'cl', x: 420, y: 360, titel: 'Cel', sub: 'cel.yaml · packages/cel' },
    { laag: 'cl', x: 220, y: 540, titel: 'Andere cel', sub: 'GGD · KvK · rechtbank' },
    { laag: 'cl', x: 420, y: 540, titel: 'Kroniek', sub: 'alleen aanvullen' },
    { laag: 'cl', x: 620, y: 540, titel: 'Reductie', sub: 'REDUCTIE · Awb-reductie' },
  ],
  pijlen: [
    { laag: 'rr', d: 'M170 152 H218' },
    { laag: 'rr', d: 'M370 152 H418' },
    { laag: 'rr', d: 'M570 152 H618' },
    { laag: 'rr', d: 'M770 152 H818' },
    { laag: 'rr', d: 'M495 84 V118', label: { x: 503, y: 106, tekst: 'valideert' } },
    { laag: 'cl', d: 'M370 392 H418', nummer: 1, badge: [394, 392] },
    { laag: 'cl', d: 'M470 424 V538', nummer: 2, badge: [470, 481] },
    { laag: 'cl', d: 'M370 572 H418', nummer: 3, badge: [394, 572] },
    { laag: 'cl', d: 'M520 360 L630 186', nummer: 4, badge: [575, 273] },
    { laag: 'cl', d: 'M680 184 V226', nummer: 5, badge: [680, 205] },
    { laag: 'cl', d: 'M640 292 L562 358', nummer: 6, badge: [601, 325] },
    { laag: 'cl', d: 'M545 424 L650 538', nummer: 7, badge: [597, 481] },
    { laag: 'cl', d: 'M620 572 H572', nummer: 8, badge: [596, 572] },
    { laag: 'cl', d: 'M735 540 V294', nummer: 9, badge: [735, 417] },
    { laag: 'cl', d: 'M735 228 V186', nummer: 9, badge: [735, 207] },
  ],
  bijschrift:
    'Boven staat RegelRecht: de wettekst gaat via de harvester naar het corpus, het schema valideert dat corpus en de engine voert het uit. ' +
    'Onder staat wat chronolexografie toevoegt. Een cel legt aanvragen en besluiten vast in haar kroniek, en een reductie leest die kroniek terug. ' +
    'Het uitvoeringsbeleid staat op de grens. Het is een regelwerk in het corpus van RegelRecht. ' +
    'Chronolexografie laat het een open begrip koppelen aan het antwoord van een cel.',
};

export const casusBlok = {
  overline: 'Een casus',
  title: 'De gehandicaptenparkeerkaart in Wilgendaal',
  lead:
    'Wilgendaal is een fictieve gemeente. Haar beleid is verzonnen, als eigen keuze in de modellering. ' +
    'De nummers horen bij de pijlen in het schema.',
  stappen: [
    'Iemand vraagt de gemeente om een gehandicaptenparkeerkaart. Artikel 49 van het BABW zegt welke gegevens bij die aanvraag horen. De regel van de Awb voor elke aanvraag (artikel 4:2) loopt mee.',
    'De cel van de gemeente legt de aanvraag vast in haar kroniek.',
    'De GGD legt vast dat zij een geneeskundig advies heeft uitgebracht. De gemeente legt in haar eigen kroniek vast dat zij het advies ontving.',
    'De gemeente laat de engine artikel 49 uitvoeren.',
    'Hoe het college zijn bevoegdheid gebruikt, laat artikel 49 open. Het beleid van de gemeente vult dat open begrip in: wie al een geldige kaart van de gemeente heeft, krijgt geen tweede.',
    "Of de aanvrager al zo'n kaart heeft, vraagt het beleid met `source.reduction` aan de cel van de gemeente.",
    'In haar `cel.yaml` staat welke reductie die vraag beantwoordt: `reductie_geldige_gehandicaptenparkeerkaart`.',
    'De reductie vraagt de Awb-reductie welke besluiten na bezwaar en beroep nog gelden. Daarna kijkt zij of daar een nog geldige kaart voor deze aanvrager bij zit. Het advies van de GGD lezen twee andere reducties uit dezelfde kroniek terug: of er een onderzoek was, en wat het advies vond.',
    'Het antwoord gaat via het beleid terug naar de engine. Die beslist of de aanvrager de kaart krijgt en tot wanneer. Dat besluit komt als nieuwe vastlegging in de kroniek, net als bij stap 2. Vraagt dezelfde persoon binnen die tijd opnieuw, dan volgt een weigering.',
  ],
  knop: 'Gehandicaptenparkeerkaart naspelen',
};

export const regelrecht = {
  title: 'Wat RegelRecht is',
  bullets: [
    'RegelRecht legt een wet of regeling één keer vast als regels die een computer kan uitvoeren, in YAML.',
    'Per artikel staan de wettekst en de uitvoerbare regels naast elkaar.',
    'Eén engine voert die regels uit, op een datum die je kiest. Bij elke uitkomst laat hij zien welke artikelen hij gebruikte.',
    'Laat een artikel iets open, dan vult een andere regel dat open begrip in, zoals een lagere regeling of uitvoeringsbeleid.',
    'Een artikel kan een uitkomst uit een andere wet gebruiken. De engine volgt die verwijzing zelf.',
    'De regels zijn een interpretatie van de wet. Bij verschil gaat de wet voor.',
  ],
};

export const chronolexografie = {
  title: 'Hoe chronolexografie daarop uitbreidt',
  bullets: [
    'Elke organisatie heeft een cel. Die legt aanvragen, besluiten, bezwaren en beroepen vast in haar eigen kroniek.',
    'In de kroniek komt alleen iets bij. Een herroepen besluit blijft staan.',
    'Een reductie is een rekenregel die de kroniek terugleest. Zo beantwoordt een cel een vraag over wat zij eerder vastlegde.',
    'Met chronolexografie kan uitvoeringsbeleid een open begrip koppelen aan het antwoord van een cel. Eén vraag gaat naar één cel, en de engine neemt de antwoorden samen.',
    'Zo telt bij een besluit mee wat de organisatie eerder besloot en wat daarvan na bezwaar of beroep nog geldt. Ook telt mee wat een andere organisatie vastlegde, zoals een advies van de GGD.',
    'Elk feit staat bij wie het vaststelde. Daardoor kun je een besluit later naspelen, met de wet en de kroniek van die dag.',
  ],
};

/** The button at the bottom, which opens the explanation page. */
export const verder = 'Lees de uitgebreide uitleg';
