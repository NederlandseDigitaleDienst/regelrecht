import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { artikelNaam, genomenBij, genummerd, ingevuldDoor, opsomming, stapTekst, toon, uitslag, wetNaam } from '../src/logboek.js';
import { rol, state } from '../src/state.js';

// De casus zoals `api.casus` haar geeft, voor zover het logboek haar leest.
beforeEach(() => {
  state.casus = {
    besluit: 'leerplichtwet_1969#13a',
    beslissing_op_bezwaar: 'algemene_wet_bestuursrecht#7:11',
    uitkomst: 'verlof_verleend',
    cel: 'obs_de_klimop',
    cel_naam: 'OBS De Klimop',
    regelwerken: [
      { id: 'leerplichtwet_1969', naam: null },
      { id: 'algemene_wet_bestuursrecht', naam: 'Algemene wet bestuursrecht' },
      { id: 'fictief_beleid_met_naam', naam: 'Beleid van de gemeente' },
    ],
  };
});

// Ids verzonnen; de vormen zoals `api.logboek` ze geeft (kern::Stap).
const bezwaar = { id: '019a0000-0000-7000-8000-00000000bbbb', kind: 'BEZWAAR' };
const grammen = [bezwaar];

const celAntwoord = {
  soort: 'begrip',
  naam: 'verlof_al_verleend_in_schooljaar',
  wet: 'leerplichtwet_1969',
  artikel: '13a',
  waarde: false,
  ontbreekt: [],
  keer: 0,
  // Het beleid van het hoofd koppelt het begrip aan het antwoord van de cel
  // (FilledBy::Answer, RFC-048).
  ingevuld_door: {
    answer: {
      law_id: 'fictief_beleid_vakantieverlof_obs_de_klimop',
      article: '2',
      answers: [
        {
          answerer: 'obs_de_klimop',
          answer: 'verlof_al_verleend_in_schooljaar',
          law_id: 'reductie_verlof_al_verleend_in_schooljaar',
          article: '1',
          valid_from: '2026-08-01',
          records_read: 5,
          last_record: bezwaar.id,
        },
      ],
    },
  },
};

const keten = [
  {
    soort: 'artikel',
    wet: 'algemene_wet_bestuursrecht',
    artikel: '7:11',
    stappen: [
      {
        soort: 'uitkomst',
        naam: 'heroverweging_geeft_aanleiding',
        waarde: true,
        ontbreekt: [],
        stappen: [
          {
            soort: 'artikel',
            wet: 'leerplichtwet_1969',
            artikel: '13a',
            stappen: [celAntwoord, { ...celAntwoord, keer: 1 }],
          },
        ],
      },
      { soort: 'hook', wet: 'algemene_wet_bestuursrecht', artikel: '6:7', stappen: [] },
    ],
  },
];

describe('genummerd', () => {
  it('nummert de keten per niveau', () => {
    const [art] = genummerd(keten);
    expect(art.nummer).toBe('1');
    const [heroverweging, hook] = art.kinderen;
    expect(hook.nummer).toBe('1.2');
    const art13a = heroverweging.kinderen[0];
    expect(art13a.nummer).toBe('1.1.1');
    expect(art13a.kinderen.map((k) => [k.nummer, k.stap.keer])).toEqual([
      ['1.1.1.1', 0],
      ['1.1.1.2', 1],
    ]);
  });
});

describe('ingevuldDoor', () => {
  it('noemt het beleidsartikel, het antwoord, de cel en de reductie, en hoe ver zij las', () => {
    expect(ingevuldDoor(celAntwoord.ingevuld_door, grammen)).toBe(
      'ingevuld door beleid fictief_beleid_vakantieverlof_obs_de_klimop art. 2, antwoord verlof_al_verleend_in_schooljaar van OBS De Klimop (reductie reductie_verlof_al_verleend_in_schooljaar, 5 chronolexogrammen t/m bezwaar 0000bbbb)',
    );
  });

  it('noemt een koppeling waarop geen cel antwoordde', () => {
    const stil = { ...celAntwoord, waarde: null, ingevuld_door: { answer: { ...celAntwoord.ingevuld_door.answer, answers: [] } } };
    expect(ingevuldDoor(stil.ingevuld_door, grammen)).toBe(
      'ingevuld door beleid fictief_beleid_vakantieverlof_obs_de_klimop art. 2, geen antwoord van een cel',
    );
  });

  it('noemt een bevinding van het bestuursorgaan en een regeling', () => {
    const bevinding = { ...celAntwoord, naam: 'specifieke_aard_beroep_ouder', waarde: true, ingevuld_door: 'finding' };
    expect(ingevuldDoor(bevinding.ingevuld_door, grammen)).toBe('bevinding van het bestuursorgaan');
    const regeling = {
      ...celAntwoord,
      ingevuld_door: { regulation: { law_id: 'leerplichtwet_1969', article: '1' } },
    };
    expect(ingevuldDoor(regeling.ingevuld_door, grammen)).toBe(
      'ingevuld door Leerplichtwet 1969 artikel 1',
    );
  });
});

describe('stapTekst', () => {
  it('zet bij een begrip alleen de naam en de waarde, zonder wie het invulde', () => {
    expect(stapTekst(celAntwoord)).toEqual({ tekst: 'verlof al verleend in schooljaar: nee' });
  });

  it('noemt een aanroep zonder artikel bij de uitvoer die hij vroeg', () => {
    const stap = { soort: 'artikel', wet: 'leerplichtwet_1969', artikel: '', uitvoer: 'verlofdagen', stappen: [] };
    expect(stapTekst(stap).tekst).toBe('verlofdagen uit Leerplichtwet 1969');
  });

  it('zegt bij een uitkomst die niet te bepalen was wat ontbrak', () => {
    const stap = { soort: 'uitkomst', naam: 'bezwaar_of_beroepschrift_tijdig_ingediend', waarde: null, ontbreekt: ['datum_ontvangst_bezwaar_of_beroepschrift'] };
    expect(stapTekst(stap)).toEqual({
      tekst: 'bezwaar of beroepschrift tijdig ingediend: niet bepaald',
      toelichting: 'ontbreekt: datum ontvangst bezwaar of beroepschrift',
    });
  });

  it('noemt een artikel en een hook bij hun naam', () => {
    expect(stapTekst(keten[0]).tekst).toBe('Awb artikel 7:11 wordt uitgevoerd');
    expect(stapTekst(keten[0].stappen[1]).tekst).toBe('Ook van toepassing: Awb artikel 6:7');
  });
});

describe('uitslag', () => {
  it('leest de uitslag af aan de uitvoer van het chronolexogram', () => {
    expect(uitslag({ kind: 'AANVRAAG', outputs: {} }).tekst).toBe('ontvangen');
    expect(uitslag({ kind: 'BESCHIKKING', basis: 'leerplichtwet_1969#13a', outputs: { verlof_verleend: false } })).toEqual({ tekst: 'afgewezen', kleur: 'critical' });
    // De uitvoer die toewijst, noemt de casus.
    state.casus.uitkomst = 'gehandicaptenparkeerkaart_verstrekt';
    expect(uitslag({ kind: 'BESCHIKKING', outputs: { gehandicaptenparkeerkaart_verstrekt: true } }).tekst).toBe('toegewezen');
    const op = (o) => uitslag({ kind: 'BESCHIKKING', basis: 'algemene_wet_bestuursrecht#7:11', outputs: o }).tekst;
    expect(op({ bezwaar_ontvankelijk: false })).toBe('niet-ontvankelijk');
    expect(op({ bezwaar_ontvankelijk: true, bestreden_besluit_herroepen: true })).toBe('gegrond');
    expect(op({ bezwaar_ontvankelijk: true, bestreden_besluit_herroepen: false })).toBe('ongegrond');
  });
});

describe('wat de cel uit zichzelf vastlegde', () => {
  const vestiging = { kind: 'EXECUTOGRAM', basis: 'regeling_vaststelling_schoolvakanties_20252030#2', stream: 'vestiging', outputs: {} };
  it('noemt de vestiging een executogram, vastgelegd', () => {
    expect(rol(vestiging)).toBe('Executogram');
    expect(uitslag(vestiging)).toEqual({ tekst: 'vastgelegd', kleur: 'neutral' });
  });
});

describe('genomenBij', () => {
  it('vindt de beslissing op bezwaar die een besluit als nieuw besluit nam', () => {
    const nieuw = '019a0000-0000-7000-8000-00000000dddd';
    const beslissing = { id: 'x', basis: 'algemene_wet_bestuursrecht#7:11', outputs: { nieuw_besluit: nieuw } };
    expect(genomenBij(nieuw, [beslissing])).toBe(beslissing);
    expect(genomenBij('ander', [beslissing])).toBeNull();
  });
});

describe('toon en artikelNaam', () => {
  it('maakt waarden en artikelen leesbaar', () => {
    expect(toon(true)).toBe('ja');
    expect(toon({ basis: 'leerplichtwet_1969#13a', fields: {} })).toBe('een besluit onder leerplichtwet_1969#13a');
    expect(toon({ __decimal: '1.50' })).toBe('1.50');
    expect(artikelNaam('fictief_beleid', '2')).toBe('Fictief beleid artikel 2');
    expect(wetNaam('fictief_beleid_met_naam')).toBe('Beleid van de gemeente');
    expect(wetNaam('besluit_administratieve_bepalingen_inzake_het_wegverkeer_babw')).toBe('BABW');
  });
});

describe('antwoorden van een andere cel (RFC-052)', () => {
  const KVK = 'kamer_van_koophandel';
  beforeEach(() => {
    state.casus.andere_cellen = [{ cel: KVK, cel_naam: 'Kamer van Koophandel' }];
    state.andere = [{ cel: KVK, grammen: [{ id: '019a0000-0000-7000-8000-00000000cccc', kind: 'EXECUTOGRAM' }] }];
  });
  afterEach(() => {
    delete state.casus.andere_cellen;
    state.andere = [];
  });

  const begrip = (ingevuld_door) => ({
    soort: 'begrip',
    naam: 'bedrijf_gevestigd_in_vergunninggebied',
    wet: 'parkeerverordening_gemeente_wilgendaal',
    artikel: '2',
    waarde: true,
    ontbreekt: [],
    keer: null,
    ingevuld_door,
  });
  const koppeling = (answers) => ({
    answer: { law_id: 'beleid_bedrijfsparkeervergunning_gemeente_wilgendaal', article: '1', answers },
  });
  const bron = (answer) => ({
    answerer: KVK,
    answer,
    article: '1',
    law_id: 'reductie_vestiging_in_handelsregister',
    valid_from: '2026-01-01',
    records_read: 1,
    last_record: null,
  });

  it('leest het laatste chronolexogram in de kroniek van de andere cel', () => {
    const t = ingevuldDoor(begrip(koppeling([{ ...bron('postcode_vestiging'), last_record: '019a0000-0000-7000-8000-00000000cccc' }])).ingevuld_door, []);
    expect(t).toContain('0000cccc');
  });

  it('noemt de andere cel bij haar naam', () => {
    const t = ingevuldDoor(begrip(koppeling([bron('postcode_vestiging')])).ingevuld_door, []);
    expect(t).toBe(
      'ingevuld door beleid beleid_bedrijfsparkeervergunning_gemeente_wilgendaal art. 1, antwoord postcode_vestiging van Kamer van Koophandel (reductie reductie_vestiging_in_handelsregister, niets uit de kroniek gelezen)',
    );
  });

  it('noemt elk antwoord dat het artikel nam, met de cel die het gaf', () => {
    const t = ingevuldDoor(begrip(koppeling([bron('postcode_vestiging'), bron('aantal_werkzame_personen')])).ingevuld_door, []);
    expect(t).toBe(
      'ingevuld door beleid beleid_bedrijfsparkeervergunning_gemeente_wilgendaal art. 1, antwoord postcode_vestiging van Kamer van Koophandel (reductie reductie_vestiging_in_handelsregister, niets uit de kroniek gelezen) en antwoord aantal_werkzame_personen van Kamer van Koophandel (reductie reductie_vestiging_in_handelsregister, niets uit de kroniek gelezen)',
    );
  });
});

describe('opsomming', () => {
  it('zet een komma tussen de eerste namen en "en" voor de laatste', () => {
    expect(opsomming(['A'])).toBe('A');
    expect(opsomming(['A', 'B'])).toBe('A en B');
    expect(opsomming(['A', 'B', 'C'])).toBe('A, B en C');
  });
});
