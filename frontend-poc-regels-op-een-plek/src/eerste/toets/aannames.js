// Gegevens die de kaart niet kent en die een regel wel vraagt: oordelen (welstand, puntensysteem),
// onderzoeken (bodem, archeologie, geluid) en maten van het bouwplan of de bestaande situatie.
// Zonder deze gegevens geeft de engine een onthouding. In Simuleren kun je ze per gegeven
// invullen, om te zien wat er dan uitkomt. Wat je invult is een aanname, geen feit.
//
// De lijst is gemaakt door de toets over het hele zoekgebied te draaien en alle ontbrekende
// gegevens te verzamelen (5 lagen gestapeld), aangevuld met de vervolgvragen die
// pas opduiken als je een eerdere vraag beantwoordt (bodem, kruispunt). Niet in de lijst: wat
// de toets zelf afleidt (de uitkomst van de bouw-, gebruiks- en aanvullende regels) en wat van de
// kaart komt (maatvoering). Parkeren in de bouwregels komt uit paraplu-artikel 11 waar dat geldt;
// elders (Brandmeesters 10.2) is het een gegeven dat je invult.
//
// Een veld kan meer namen hebben: hetzelfde gegeven heet in twee YAML-bestanden anders.
// `gunstig` is de waarde die de regel laat slagen, voor de knop "Alles gunstig", gelezen uit de
// YAML (de grens staat in de uitleg). Een functie: de waarde hangt af van het bouwplan. null:
// niet invullen, omdat een andere route in dezelfde regel al volstaat of de grens op de kaart staat.
import { reactive } from 'vue';

export const GROEPEN = [
  {
    id: 'welstand', kop: 'Welstand en bodem',
    bron: 'Omgevingsplan, artikel 22.29. Geldt bij elke bouwaanvraag.',
    velden: [
      { id: 'welstand', namen: ['niet_in_strijd_met_redelijke_eisen_van_welstand'], soort: 'ja-nee', gunstig: true,
        tekst: 'Past binnen redelijke eisen van welstand', uitleg: 'Oordeel van het college, per aanvraag.' },
      { id: 'welstandsvrij', namen: ['gebied_zonder_redelijke_eisen_van_welstand'], soort: 'ja-nee', gunstig: null,
        tekst: 'Gebied is welstandsvrij', uitleg: 'Aangewezen in het tijdelijk deel. Eén van de drie routes is genoeg.' },
      { id: 'welstandAfwijken', namen: ['bevoegd_gezag_verleent_in_afwijking_van_welstand'], soort: 'ja-nee', gunstig: null,
        tekst: 'College wijkt af van welstand', uitleg: 'Oordeel van het bevoegd gezag.' },
      { id: 'bodemgevoelig', namen: ['bodemgevoelig_gebouw_op_bodemgevoelige_locatie'], soort: 'ja-nee', gunstig: false,
        tekst: 'Bodemgevoelig gebouw op een bodemgevoelige locatie', uitleg: 'Een woning is bodemgevoelig. Of de locatie dat is, volgt uit bodemonderzoek.' },
      { id: 'bodemkwaliteit', namen: ['toelaatbare_kwaliteit_bodem_niet_overschreden'], soort: 'ja-nee', gunstig: null,
        tekst: 'Bodemkwaliteit blijft binnen de norm', uitleg: 'Telt alleen bij een bodemgevoelige locatie.' },
      { id: 'bodemmaatregel', namen: ['sanerende_of_beschermende_maatregel_aannemelijk'], soort: 'ja-nee', gunstig: null,
        tekst: 'Sanering of beschermende maatregel aannemelijk', uitleg: 'Telt alleen bij een bodemgevoelige locatie.' },
    ],
  },
  {
    id: 'parkeren', kop: 'Parkeren en laden',
    bron: 'Parapluplan, artikel 11. Geldt waar het bestemmingsplan in de tabel van het paraplu staat.',
    velden: [
      { id: 'parkeren', namen: ['voldoende_parkeergelegenheid_aangetoond'], soort: 'ja-nee', gunstig: true,
        tekst: 'Voldoende parkeerplaatsen aangetoond', uitleg: 'Volgens het parkeernormenbeleid.' },
      { id: 'parkerenBijzonder', namen: ['parkeren_bijzondere_omstandigheden_aangetoond'], soort: 'ja-nee', gunstig: null,
        tekst: 'Bijzondere omstandigheden (parkeren)', uitleg: 'Alternatieve route naast voldoende plaatsen.' },
      { id: 'parkerenAnders', namen: ['parkeren_op_andere_wijze_voorzien'], soort: 'ja-nee', gunstig: null,
        tekst: 'Parkeren op andere wijze voorzien', uitleg: 'Alternatieve route, met de effecten in beeld.' },
      { id: 'laden', namen: ['voldoende_laad_en_losruimte_aangetoond'], soort: 'ja-nee', gunstig: true,
        tekst: 'Voldoende laad- en losruimte aangetoond', uitleg: '' },
      { id: 'ladenBijzonder', namen: ['laden_lossen_bijzondere_omstandigheden_aangetoond'], soort: 'ja-nee', gunstig: null,
        tekst: 'Bijzondere omstandigheden (laden)', uitleg: '' },
      { id: 'ladenAnders', namen: ['laden_lossen_op_andere_wijze_voorzien'], soort: 'ja-nee', gunstig: null,
        tekst: 'Laden en lossen op andere wijze voorzien', uitleg: '' },
      { id: 'parkerenEigenPlan', namen: ['parkeren_voldaan'], soort: 'ja-nee', gunstig: true,
        tekst: 'Parkeren voldoet aan het eigen plan', uitleg: 'Waar paraplu-artikel 11 niet geldt, zoals Brandmeesters 10.2. Waar artikel 11 wel geldt, komt de uitkomst daarvandaan en telt dit niet.' },
    ],
  },
  {
    id: 'archeologie', kop: 'Archeologie',
    bron: 'Parapluplan, artikel 6 en 7. Alleen waar een archeologische waarde ligt.',
    velden: [
      { id: 'archeoRapport', namen: ['archeologisch_rapport_gunstig'], soort: 'ja-nee', gunstig: true,
        tekst: 'Archeologisch rapport gunstig', uitleg: 'Het rapport laat zien dat de waarden niet onevenredig worden geschaad.' },
      { id: 'oppervlakte', namen: ['bebouwde_oppervlakte_bouwplan_m2'], soort: 'getal', eenheid: 'm²', gunstig: null,
        tekst: 'Bebouwde oppervlakte bouwplan', uitleg: 'Tot 100 m² (zeer hoog) of meer, afhankelijk van de waarde, geen onderzoek. Bij "alles gunstig" leeg: het rapport volstaat.' },
      { id: 'graafdiepte', namen: ['graafdiepte_bouwplan_m'], soort: 'getal', eenheid: 'm', gunstig: null,
        tekst: 'Graaf- of heidiepte', uitleg: 'Tot 0,3 of 0,5 m geen onderzoek. Bij "alles gunstig" leeg: het rapport volstaat.' },
      { id: 'fundering', namen: ['vervangend_op_bestaande_fundering'], soort: 'ja-nee', gunstig: null,
        tekst: 'Vervangende bouw op bestaande fundering', uitleg: '' },
    ],
  },
  {
    id: 'bestaand', kop: 'Perceel en bestaande situatie',
    bron: 'Bouwregels van de bestemmingsplannen en TAM Ritmeester. Niet uit de kaart te halen.',
    velden: [
      { id: 'bestaandeWoningen', namen: ['bestaand_of_vergund_aantal_woningen'], soort: 'getal', eenheid: 'woningen', gunstig: (bp) => bp.aantalWoningen,
        tekst: 'Bestaand of vergund aantal woningen', uitleg: 'Op het perceel, nu. Zonder aanduiding mag het bouwplan niet meer woningen hebben. Gunstig: zoveel als het bouwplan.' },
      { id: 'afstandPerceel', namen: ['afstand_tot_zijdelingse_perceelsgrens_m', 'afstand_tot_zijdelingse_perceelgrens_m'], soort: 'getal', eenheid: 'm', gunstig: 2.5,
        tekst: 'Afstand tot de zijdelingse perceelsgrens', uitleg: 'Van het nieuwe gebouw. Woongebieden en Ritmeester: minstens 2,5 m; Franse Gat 3: 0 of minstens 1 m.' },
      { id: 'afstandPerceelBestaand', namen: ['bestaande_afstand_tot_zijdelingse_perceelsgrens_m'], soort: 'getal', eenheid: 'm', gunstig: null,
        tekst: 'Bestaande afstand tot de perceelsgrens', uitleg: 'Van het gebouw dat er nu staat. Bij "alles gunstig" leeg: 2,5 m afstand volstaat.' },
      { id: 'bouwgrens', namen: ['in_naar_de_weg_gekeerde_bouwgrens', 'in_bouwgrens_gebouwd'], soort: 'ja-nee', gunstig: true,
        tekst: 'Gebouwd in de naar de weg gekeerde bouwgrens', uitleg: '' },
      { id: 'bouwdiepte', namen: ['bouwdiepte_m'], soort: 'getal', eenheid: 'm', gunstig: 13,
        tekst: 'Bouwdiepte', uitleg: 'TAM Ritmeester 14.3: hoogstens 13 m.' },
      { id: 'bouwdiepteHoofdgebouw', namen: ['bouwdiepte_hoofdgebouw_m', 'diepte_hoofdgebouw_m'], soort: 'getal', eenheid: 'm', gunstig: 10,
        tekst: 'Bouwdiepte hoofdgebouw', uitleg: 'Parapluplan 3.2.1: hoogstens 10 m (aaneen) of 12 m (twee-aaneen, vrijstaand). TAM Franse Gat 3: hoogstens 12 m.' },
      { id: 'bestaandeBouwhoogte', namen: ['bestaande_bouwhoogte_m'], soort: 'getal', eenheid: 'm', gunstig: (bp) => Math.round(bp.lagen * bp.meterPerLaag * 100) / 100,
        tekst: 'Bestaande bouwhoogte', uitleg: 'Van het gebouw dat er nu staat. Woongebieden 2018, 13.2.2 onder f: grondgebonden hoogstens 10 m of de bestaande hoogte. Gunstig: zo hoog als het bouwplan.' },
      { id: 'bebouwingspercentage', namen: ['bebouwingspercentage_bouwplan'], soort: 'getal', eenheid: '%', gunstig: null,
        tekst: 'Bebouwingspercentage', uitleg: 'TAM Ritmeester 14.3: hoogstens het maximum op de kaart. Bij "alles gunstig" leeg.' },
    ],
  },
  {
    id: 'programma', kop: 'Woningbouwprogramma en beleid',
    bron: "Bestemmingsplannen met voorwaarden bij wonen, TAM Ritmeester, TAM Franse Gat, ontwerp-TAM Het Ambacht.",
    velden: [
      { id: 'puntensysteem', namen: ['puntensysteem_voldaan'], soort: 'ja-nee', gunstig: true,
        tekst: 'Voldoet aan het puntensysteem', uitleg: "Beleidsregel 'Puntensysteem Omgevingsvisie Veenendaal'." },
      { id: 'voorwaardenWonen', namen: ['voorwaarden_wonen_voldaan'], soort: 'ja-nee', gunstig: true,
        tekst: 'Voldoet aan de voorwaarden bij wonen', uitleg: 'Bijvoorbeeld een aandeel sociale woningen.' },
      { id: 'socialProcent', namen: ['aandeel_sociale_woningen_procent'], soort: 'getal', eenheid: '%', gunstig: 30,
        tekst: 'Aandeel sociale woningen in het bouwplan', uitleg: "'t Goeie Spoor en Franse Gat 3: minstens 30%." },
      { id: 'sociaal', namen: ['aantal_sociale_woningen_bouwplan'], soort: 'getal', eenheid: 'woningen', gunstig: (bp) => Math.ceil(bp.aantalWoningen * 0.3),
        tekst: 'Aantal sociale woningen', uitleg: 'Gunstig: 30% van het bouwplan, naar boven afgerond.' },
      { id: 'sociaalHuur', namen: ['aantal_sociale_huurwoningen_bouwplan'], soort: 'getal', eenheid: 'woningen', gunstig: (bp) => Math.ceil(bp.aantalWoningen * 0.3),
        tekst: 'Aantal sociale huurwoningen', uitleg: 'Minstens twee derde van de sociale woningen. Gunstig: allemaal huur.' },
      { id: 'werkingsgebied', namen: ['aantal_wooneenheden_werkingsgebied_na_bouwplan'], soort: 'getal', eenheid: 'woningen', gunstig: 175,
        tekst: 'Woningen in het werkingsgebied, na het bouwplan', uitleg: 'TAM Ritmeester 9: hoogstens 175.' },
      { id: 'werkingsgebiedHuur', namen: ['aandeel_sociale_huurwoningen_werkingsgebied_procent'], soort: 'getal', eenheid: '%', gunstig: 20,
        tekst: 'Aandeel sociale huur in het werkingsgebied', uitleg: 'TAM Ritmeester 9: minstens 20%.' },
      { id: 'werkingsgebiedKoop', namen: ['aandeel_sociale_koopwoningen_werkingsgebied_procent'], soort: 'getal', eenheid: '%', gunstig: 10,
        tekst: 'Aandeel sociale koop in het werkingsgebied', uitleg: 'TAM Ritmeester 9: minstens 10%.' },
    ],
  },
  {
    id: 'ambacht', kop: 'Het Ambacht (ontwerp-TAM)',
    bron: 'Alleen in de kolom nieuw, waar het ontwerp-TAM Het Ambacht ligt. Beoogd, geen geldend recht.',
    velden: [
      { id: 'ambachtTotaal', namen: ['aantal_wooneenheden_na_vergunning'], soort: 'getal', eenheid: 'woningen', gunstig: 1500,
        tekst: 'Wooneenheden in het gebied na de vergunning', uitleg: 'Artikel 49: hoogstens 2300; boven 1500 moet ook het kruispunt zijn aangepast. Gunstig: 1500.' },
      { id: 'ambachtBedrijven', namen: ['afstand_tot_bedrijven_bijlage_2_voldaan'], soort: 'ja-nee', gunstig: true,
        tekst: 'Afstand tot bedrijven (bijlage 2) gehaald', uitleg: '' },
      { id: 'ambachtBedrijfNietBeperkt', namen: ['bedrijf_niet_beperkt_aangetoond'], soort: 'ja-nee', gunstig: null,
        tekst: 'Bedrijf wordt niet beperkt (aangetoond)', uitleg: 'Route als de afstand niet wordt gehaald.' },
      { id: 'ambacht1500', namen: ['meer_dan_1500_wooneenheden_vergund'], soort: 'ja-nee', gunstig: false,
        tekst: 'Al meer dan 1500 wooneenheden vergund', uitleg: 'Dan moet eerst het kruispunt zijn aangepast.' },
      { id: 'ambachtKruispunt', namen: ['kruispunt_aangepast'], soort: 'ja-nee', gunstig: null,
        tekst: 'Kruispunt aangepast', uitleg: 'Telt alleen boven de 1500 wooneenheden.' },
      { id: 'ambachtPlint', namen: ['plint_voldaan'], soort: 'ja-nee', gunstig: true,
        tekst: 'Plint voldoet', uitleg: 'Artikel 54.2, onder i en j.' },
      { id: 'ambachtStadswoningen', namen: ['aandeel_stadswoningen_voldaan'], soort: 'ja-nee', gunstig: true,
        tekst: 'Aandeel stadswoningen voldoet', uitleg: 'Artikel 54.2, onder k tot en met m.' },
      { id: 'ambachtGeluid', namen: ['geluidbelasting_aanvaardbaar'], soort: 'ja-nee', gunstig: true,
        tekst: 'Geluidbelasting aanvaardbaar', uitleg: 'Akoestisch onderzoek, artikel 70.' },
      { id: 'ambachtCategorie2', namen: ['bedrijf_categorie_2_niet_beperkt'], soort: 'ja-nee', gunstig: true,
        tekst: 'Bedrijf in categorie 2 niet beperkt', uitleg: 'Artikel 71.' },
      { id: 'ambachtVishandel', namen: ['vishandel_niet_beperkt'], soort: 'ja-nee', gunstig: true,
        tekst: 'Vishandel niet beperkt', uitleg: 'Artikel 71.' },
    ],
  },
  {
    id: 'overig', kop: 'Overige oordelen',
    bron: 'Losse onderdelen van bestemmingsplannen en TAM Ritmeester.',
    velden: [
      { id: 'overigeEisen', namen: ['overige_eisen_voldaan', 'overige_eisen_6_2_2_voldaan'], soort: 'ja-nee', gunstig: true,
        tekst: 'Overige eisen voldaan', uitleg: 'Onderdelen die in YAML niet zijn uitgewerkt (Industrielaan, Stationskwartier C).' },
      { id: 'orientatie', namen: ['orientatie_en_voorgevel_voldaan'], soort: 'ja-nee', gunstig: true,
        tekst: 'Oriëntatie en voorgevel voldoen', uitleg: 'Pionierkwartier fase 3, 6.2.2.' },
      { id: 'gevellijn', namen: ['in_gevellijn_gebouwd'], soort: 'ja-nee', gunstig: true,
        tekst: 'Gebouwd in de gevellijn', uitleg: 'Franse Gat Zuid, deel 1 en 2.' },
      { id: 'diepte', namen: ['diepte_voldaan'], soort: 'ja-nee', gunstig: true,
        tekst: 'Diepte hoofdgebouw voldoet aan het deelgebied', uitleg: 'Franse Gat Zuid, deel 1 en 2.' },
      { id: 'hoogspanning', namen: ['hoogspanningsverbinding_blijvend_buiten_gebruik'], soort: 'ja-nee', gunstig: true,
        tekst: 'Hoogspanningsverbinding blijvend buiten gebruik', uitleg: 'Woongebieden 2018, 13.2.2 onder m. Telt in de veiligheidszone.' },
      { id: 'geluid', namen: ['geluid_voldaan'], soort: 'ja-nee', gunstig: true,
        tekst: 'Geluid wegverkeer voldoet', uitleg: 'Voorwaardelijke bepaling (Brandmeesters).' },
      { id: 'entrees', namen: ['entrees_voldaan'], soort: 'ja-nee', gunstig: true,
        tekst: 'Entrees voldoen', uitleg: 'TAM Ritmeester 14.3.' },
      { id: 'uiterlijk', namen: ['uiterlijk_aanzien_voldoende'], soort: 'ja-nee', gunstig: true,
        tekst: 'Uiterlijk aanzien voldoende', uitleg: "Beleidsregel 'Het verhaal van het Ritmeesterkwartier'." },
    ],
  },
  {
    id: 'goeiespoor', kop: "'t Goeie Spoor: gebruik, geluid, trillingen en voorwaarden",
    bron: "Bestemmingsplan 't Goeie Spoor, artikel 6.3, 9, 10, 12 en 15. De voorwaarden waaronder de vergunning wordt verleend.",
    velden: [
      { id: 'meerHuishoudens', namen: ['meer_dan_een_huishouden_per_woning'], soort: 'ja-nee', gunstig: false,
        tekst: 'Meer dan één huishouden per woning', uitleg: '6.3.2 onder a. Ook kamerbewoning.' },
      { id: 'geluidHogereWaarden', namen: ['voldoet_aan_geluidbelastingen_besluit_hogere_waarden'], soort: 'ja-nee', gunstig: true,
        tekst: 'Geluidbelasting binnen het Besluit hogere waarden', uitleg: '6.3.3 onder a. Of onder de 55 dB (spoor) en 48 dB (weg).' },
      { id: 'geluidRail', namen: ['geluidbelasting_railverkeer_db'], soort: 'getal', eenheid: 'dB', gunstig: null,
        tekst: 'Geluidbelasting railverkeer', uitleg: '6.3.3 onder a: ten hoogste 55 dB. Bij "alles gunstig" leeg: het Besluit hogere waarden volstaat.' },
      { id: 'geluidWeg', namen: ['geluidbelasting_wegverkeer_db'], soort: 'getal', eenheid: 'dB', gunstig: null,
        tekst: 'Geluidbelasting wegverkeer', uitleg: '6.3.3 onder a: ten hoogste 48 dB.' },
      { id: 'geluidVoorwaarden', namen: ['voorwaarden_besluit_hogere_waarden_voldaan'], soort: 'ja-nee', gunstig: true,
        tekst: 'Voorwaarden uit het Besluit hogere waarden voldaan', uitleg: '6.3.3 onder b.' },
      { id: 'ventilatie', namen: ['ventilatie_voldoet_aan_vereist_percentage'], soort: 'ja-nee', gunstig: null,
        tekst: 'Ventilatie voldoet aan het vereiste percentage',
        uitleg: '6.3.3 onder c. In het vastgestelde plan staat "XXX %": het percentage is niet ingevuld. Niemand kan dit vaststellen; "alles gunstig" vult het daarom niet in.' },
      { id: 'trillingenNieuw', namen: ['streefwaarden_trillingen_nieuwe_situatie_niet_overschreden'], soort: 'ja-nee', gunstig: true,
        tekst: 'Trillingen binnen de streefwaarden (nieuwe situatie)', uitleg: '6.3.4 onder a. Uit een dynamische berekening.' },
      { id: 'trillingenMaatregelen', namen: ['maatregelen_trillingen_getroffen'], soort: 'ja-nee', gunstig: null,
        tekst: 'Alle doelmatige maatregelen tegen trillingen getroffen', uitleg: '6.3.4 onder b, de alternatieve route.' },
      { id: 'trillingenBestaand', namen: ['streefwaarden_trillingen_bestaande_situatie_niet_overschreden'], soort: 'ja-nee', gunstig: null,
        tekst: 'Trillingen binnen de streefwaarden (bestaande situatie)', uitleg: '6.3.4 onder b, de alternatieve route.' },
      { id: 'dubbeltelling', namen: ['grond_eerder_in_aanmerking_genomen'], soort: 'ja-nee', gunstig: false,
        tekst: 'Grond al meegeteld bij een eerder bouwplan', uitleg: 'Artikel 9, anti-dubbeltelregel.' },
      { id: 'ondergronds', namen: ['ondergronds_bouwwerk_in_bouwplan'], soort: 'ja-nee', gunstig: false,
        tekst: 'Bouwplan heeft een ondergronds bouwwerk', uitleg: '10.1. Bijvoorbeeld een kelder of parkeergarage.' },
      { id: 'ondergrondsOnder', namen: ['ondergronds_uitsluitend_onder_hoofdgebouw'], soort: 'ja-nee', gunstig: null,
        tekst: 'Ondergronds bouwwerk geheel onder het hoofdgebouw', uitleg: '10.1. Telt alleen met een ondergronds bouwwerk.' },
      { id: 'middenspanning', namen: ['middenspanningsleiding_aanwezig'], soort: 'ja-nee', gunstig: null,
        tekst: 'Middenspanningsleiding nog aanwezig', uitleg: "10.1. Telt alleen ter plaatse van 'onderdoorgang'." },
      { id: 'boring', namen: ['grondroering_dieper_dan_30_m'], soort: 'ja-nee', gunstig: false,
        tekst: 'Funderingswerken dieper dan 30 meter', uitleg: '12.1, boringsvrije zone.' },
      { id: 'boringProvincie', namen: ['toestemming_provincie_verkregen'], soort: 'ja-nee', gunstig: null,
        tekst: 'Toestemming van de provincie voor dieper dan 30 meter', uitleg: '12.1.' },
      { id: 'inrichtingsplan', namen: ['inrichtingsplan_goedgekeurd'], soort: 'ja-nee', gunstig: true,
        tekst: 'Inrichtingsplan goedgekeurd door de gemeente', uitleg: '15.2. Met groene dakterrassen.' },
      { id: 'broedseizoen', namen: ['bouw_buiten_broedseizoen'], soort: 'ja-nee', gunstig: true,
        tekst: 'Bouwwerkzaamheden buiten het broedseizoen', uitleg: '15.4.' },
      { id: 'broedRapport', namen: ['ecologisch_rapport_goedgekeurd'], soort: 'ja-nee', gunstig: null,
        tekst: 'Goedgekeurd ecologisch rapport', uitleg: '15.4, de alternatieve route.' },
    ],
  },
];

export const VELDEN = GROEPEN.flatMap((g) => g.velden.map((v) => ({ ...v, groep: g.id })));

/** Ingevulde aannames, per veld-id: true, false of een getal. Niet ingevuld = onbekend. */
export const aannames = reactive({});

/** Uitkomsten die de toets zelf afleidt: nooit zelf invullen, en geen "wat ontbreekt". */
export const AFGELEID = new Set([
  'bouwregels_ter_plaatse_voldaan', 'gebruiksregels_ter_plaatse_voldaan', 'aanvullende_regels_ter_plaatse_voldaan',
]);

/** Het veld bij een parameternaam, of undefined als je het niet kunt invullen (kaartfeit). */
export const veldVoor = (naam) => VELDEN.find((v) => v.namen.includes(naam));

/** Ingevulde aannames als parameters voor de engine. */
export function naarParams(a = aannames) {
  const p = {};
  for (const v of VELDEN) {
    const w = a[v.id];
    if (typeof w === 'boolean' || (typeof w === 'number' && Number.isFinite(w))) for (const n of v.namen) p[n] = w;
  }
  return p;
}

/**
 * Alles invullen wat een regel laat slagen, ja-nee en getallen. Getallen die van het bouwplan
 * afhangen, volgen het bouwplan van dit moment.
 */
export function allesGunstig(a = aannames, bp = { lagen: 5, meterPerLaag: 3.2, aantalWoningen: 20 }) {
  for (const v of VELDEN) {
    if (v.gunstig === null) continue;
    a[v.id] = typeof v.gunstig === 'function' ? v.gunstig(bp) : v.gunstig;
  }
}

export function allesWissen(a = aannames) {
  for (const k of Object.keys(a)) delete a[k];
}

export const aantalIngevuld = (a = aannames) => VELDEN.filter((v) => a[v.id] !== undefined).length;
