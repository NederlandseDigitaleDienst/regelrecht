// Vaste kleuren en klassen van de kaartdemo. Leaflet tekent buiten de NLDD-componenten, dus de
// kaart krijgt hier zijn rijkshuisstijlkleuren. Statuskleuren altijd met tekst in de legenda.

/** Uitkomst van een toets. Dezelfde vier (vijf met de scan) in elke demo. */
export const KLASSEN = [
  { klasse: 'toegestaan', tekst: 'Toegestaan', kleur: '#0ca30c' },
  { klasse: 'onthouding', tekst: 'Onthouding: er ontbreekt een gegeven', kleur: '#fab219' },
  { klasse: 'bron', tekst: 'Onthouding: regels alleen als scan', kleur: '#8c6d1f' },
  { klasse: 'niet', tekst: 'Niet toegestaan', kleur: '#d03b3b' },
  { klasse: 'open', tekst: 'Rekent hier niet: geen plan of geen regels', kleur: '#8a8a86' },
];
export const KLEUR = Object.fromEntries(KLASSEN.map((k) => [k.klasse, k.kleur]));

/** Uitkomst naar NLDD-tag en -banner. */
export const TAG = { toegestaan: 'success', niet: 'critical', 'niet toegestaan': 'critical', onthouding: 'warning', bron: 'warning', open: 'neutral' };
export const TEKST = { toegestaan: 'toegestaan', niet: 'niet toegestaan', 'niet toegestaan': 'niet toegestaan', onthouding: 'onthouding', bron: 'alleen scan', open: 'rekent hier niet' };
export const BANNER = {
  toegestaan: { variant: 'success', tekst: 'Toegestaan' },
  niet: { variant: 'critical', tekst: 'Niet toegestaan' },
  onthouding: { variant: 'warning', tekst: 'Onthouding' },
  bron: { variant: 'warning', tekst: 'Onthouding: bron alleen als scan' },
  open: { variant: 'neutral', tekst: 'Rekent hier niet' },
};
export const banner = (k) => BANNER[k] ?? { variant: 'neutral', tekst: 'Geen uitkomst' };

/** Lijnkleur per soort plan. Ontwerpen gestippeld. */
export const KLEUR_PLAN = {
  basisplan: '#01689b', wijzigingsplan: '#01689b', uitwerkingsplan: '#01689b', // lintblauw
  facet: '#d52b1e', // rood
  tam: '#275937', 'tam-ontwerp': '#275937', // donkergroen
};
export const ZOEKGEBIED = '#154273'; // donkerblauw, ook de testpunten
export const ACCENT = '#e17000'; // oranje: de bestemming op het gekozen punt
export const PUNT = '#ffb612'; // geel: het gekozen punt, met zwarte rand
export const TWEEDE_REEKS = '#42145f'; // violet: scenario's naast de testpunten

/** Maximale bouwhoogte, bij 3,2 m per laag. */
export const HOOGTE = [
  { tot: 9.6, tekst: 'tot 9,6 m (3 lagen)', kleur: '#86b6ef' },
  { tot: 12.8, tekst: '9,6 tot 12,8 m (4 lagen)', kleur: '#5598e7' },
  { tot: 16, tekst: '12,8 tot 16 m (5 lagen)', kleur: '#2a78d6' },
  { tot: 22.4, tekst: '16 tot 22,4 m (6 of 7 lagen)', kleur: '#1c5cab' },
  { tot: Infinity, tekst: 'meer dan 22,4 m (8 lagen of meer)', kleur: '#104281' },
];

/** Hoofdgroepen van bestemmingen (IMRO) in vier groepen. */
export const BESTEMMING = [
  { groep: 'wonen', tekst: 'Wonen', kleur: '#2a78d6', hoofdgroepen: ['wonen'] },
  { groep: 'gemengd', tekst: 'Gemengd of centrum', kleur: '#1baf7a', hoofdgroepen: ['gemengd', 'centrum'] },
  { groep: 'functie', tekst: 'Andere functie: maatschappelijk, bedrijf, horeca, sport en dergelijke', kleur: '#6250d6',
    hoofdgroepen: ['maatschappelijk', 'dienstverlening', 'horeca', 'kantoor', 'detailhandel', 'sport', 'bedrijf', 'bedrijventerrein', 'cultuur en ontspanning'] },
  { groep: 'open', tekst: 'Groen, water, verkeer', kleur: '#8a8a86', hoofdgroepen: ['groen', 'water', 'verkeer', 'tuin', 'natuur', 'agrarisch'] },
];

/** Twee kolommen naast elkaar: rand boven de kolom. */
export const KOLOM = { oud: '#8a8a86', nieuw: '#01689b' };
