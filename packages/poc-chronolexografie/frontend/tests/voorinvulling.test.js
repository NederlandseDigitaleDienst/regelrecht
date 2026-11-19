import { describe, expect, it } from 'vitest';
import {
  beginWaarden,
  bronInvoer,
  herkomstTekst,
  metSysteem,
  opHetFormulier,
  systeemBronnen,
  systeemTekst,
  systeemVult,
  voorgevuldeWaarden,
} from '@regelrecht/frontend-shared/chronolex/voorinvulling';
import { BESTUUR, RIJEN } from './fixtureTabel.js';

const celNaam = (cel) => ({ voorbeeld_register: 'Voorbeeldregister' })[cel] ?? `cel ${cel}`;

describe('bronInvoer', () => {
  it('gives a source every field with a value, a prefilled one too', () => {
    // The seats read the designation, which is prefilled itself.
    const invoer = bronInvoer({ nummer: '1', naam_opgegeven: 'X', leeg: undefined });
    expect(invoer).toEqual({ nummer: '1', naam_opgegeven: 'X' });
  });
});

describe('beginWaarden', () => {
  const voorgevuld = { naam: { waarde: 'Uit de bron', herkomst: [] } };

  it('takes the source over the example, and the step over the source', () => {
    const scenario = { naam: 'Uit het voorbeeld', jaar: 2028 };
    expect(beginWaarden(scenario, {}, voorgevuld)).toEqual({ naam: 'Uit de bron', jaar: 2028 });
    const stap = { ...scenario, naam: 'Uit de stap' };
    expect(beginWaarden(stap, { naam: 'Uit de stap' }, voorgevuld)).toEqual({ naam: 'Uit de stap', jaar: 2028 });
  });

  it('starts from the sources alone in free play', () => {
    expect(beginWaarden(null, null, voorgevuld)).toEqual({ naam: 'Uit de bron' });
  });
});

describe('voorgevuldeWaarden', () => {
  it('takes the value of each prefilled field the sources gave', () => {
    const voorgevuld = {
      naam_opgegeven: { bron: 'r#2#naam', waarde: 'Vereniging Voorbeeld', herkomst: [] },
      zetels: { bron: 'r#3#zetels', waarde: null, herkomst: [], fout: 'nummer ontbreekt' },
    };
    expect(voorgevuldeWaarden(voorgevuld)).toEqual({ naam_opgegeven: 'Vereniging Voorbeeld' });
    expect(voorgevuldeWaarden(undefined)).toEqual({});
    // A table from a chronicle holds a decimal cell as its number.
    const tabel = { bestuur: { waarde: [{ aantal: { __decimal: '1.5' } }], herkomst: [] } };
    expect(voorgevuldeWaarden(tabel)).toEqual({ bestuur: [{ aantal: 1.5 }] });
  });
});

// An unknown value (RFC-036), as the engine gives an output that reads a
// parameter the form did not pass (kern.rs `voorinvullen`).
const ONBEKEND = { __unknown: true, missing: [{ kind: 'not_passed', law: 'voorbeeld_inschrijving', name: 'kenmerk' }] };

describe('an unknown value from a source', () => {
  const voorgevuld = {
    bron: 'voorbeeld_inschrijving#5#leden',
    waarde: ONBEKEND,
    herkomst: [{ door: 'voorbeeld_register', antwoord: 'naam_bij_nummer' }],
  };

  it('fills no field, a table neither', () => {
    expect(voorgevuldeWaarden({ aanhef: voorgevuld, leden: voorgevuld })).toEqual({});
  });

  it('says the source gives an unknown value, and what it misses', () => {
    expect(herkomstTekst(voorgevuld, undefined, celNaam)).toBe(
      'Niet vooringevuld: Voorbeeldregister geeft onbekend (ontbreekt: kenmerk).',
    );
    expect(herkomstTekst(voorgevuld, undefined, celNaam, BESTUUR.kolommen)).toBe(
      'Niet vooringevuld: Voorbeeldregister geeft onbekend (ontbreekt: kenmerk).',
    );
  });

  it('names the source when no cell answered', () => {
    expect(herkomstTekst({ ...voorgevuld, herkomst: [] }, undefined, celNaam)).toBe(
      'Niet vooringevuld: de bron geeft onbekend (ontbreekt: kenmerk).',
    );
  });
});

describe('herkomstTekst', () => {
  const voorgevuld = {
    bron: 'r#2#naam_volgens_register',
    waarde: 'Vereniging Voorbeeld',
    herkomst: [{ door: 'voorbeeld_register', antwoord: 'naam_bij_nummer' }],
  };

  it('names the cell the value comes from', () => {
    expect(herkomstTekst(voorgevuld, 'Vereniging Voorbeeld', celNaam)).toBe(
      'Ingevuld uit Voorbeeldregister (naam bij nummer).',
    );
  });

  it('says the applicant changed it, next to what the source gives', () => {
    expect(herkomstTekst(voorgevuld, 'Iets anders', celNaam)).toBe(
      'Door u gewijzigd. Voorbeeldregister geeft: Vereniging Voorbeeld.',
    );
    expect(herkomstTekst(voorgevuld, undefined, celNaam)).toBe(
      'Door u leeggemaakt. Voorbeeldregister geeft: Vereniging Voorbeeld.',
    );
  });

  it('says why a source gave nothing', () => {
    expect(herkomstTekst({ ...voorgevuld, waarde: null, herkomst: [], fout: 'refused: no output' }, undefined, celNaam)).toBe(
      'Niet vooringevuld: refused: no output',
    );
  });

  it('shows the value the source gives as the app shows values', () => {
    expect(herkomstTekst({ ...voorgevuld, waarde: true }, false, celNaam)).toBe(
      'Door u gewijzigd. Voorbeeldregister geeft: ja.',
    );
  });

  it('says nothing when no cell gave a value', () => {
    expect(herkomstTekst({ ...voorgevuld, waarde: null, herkomst: [] }, undefined, celNaam)).toBe('');
    expect(herkomstTekst(undefined, 'x', celNaam)).toBe('');
  });
});

describe('herkomstTekst of a table', () => {
  const voorgevuld = {
    bron: 'voorbeeld_inschrijving#3#bestuur_volgens_register',
    waarde: RIJEN,
    herkomst: [{ door: 'voorbeeld_register', antwoord: 'bestuur_bij_nummer' }],
  };
  const tekst = (waarde) => herkomstTekst(voorgevuld, waarde, celNaam, BESTUUR.kolommen);
  const [een, twee] = RIJEN;

  it('names the cell when the table holds what the source gives, in any order', () => {
    const ingevuld = 'Ingevuld uit Voorbeeldregister (bestuur bij nummer).';
    expect(tekst(RIJEN.map((r) => ({ ...r })))).toBe(ingevuld);
    expect(tekst([twee, een])).toBe(ingevuld);
  });

  it('says what the source gives where the applicant changed the table', () => {
    expect(tekst([{ ...een, vergoeding: 15000 }, twee])).toBe(
      'Door u gewijzigd. Voorbeeldregister geeft bij Lid Een, voorzitter, 2026-01-01: vergoeding 12500.',
    );
    const nieuw = { naam: 'Lid Drie', functie: 'secretaris', sinds: '2026-03-01', vergoeding: null };
    expect(tekst([een, nieuw])).toBe(
      'Door u gewijzigd. Voorbeeldregister geeft ook Lid Twee, penningmeester, 2026-01-15; ' +
        'niet Lid Drie, secretaris, 2026-03-01.',
    );
  });

  it('names a changed row by its other cells when its key cells are empty', () => {
    const k = [
      { naam: 'code', type: 'string' },
      { naam: 'aantal', type: 'number' },
    ];
    const bron = { ...voorgevuld, waarde: [{ code: null, aantal: 1 }] };
    expect(herkomstTekst(bron, [{ aantal: 2 }], celNaam, k)).toBe(
      'Door u gewijzigd. Voorbeeldregister geeft bij 1: aantal 1.',
    );
  });

  it('names an added row by its other cells when its key cells are empty', () => {
    const k = [
      { naam: 'code', type: 'string' },
      { naam: 'aantal', type: 'number' },
    ];
    const bron = { ...voorgevuld, waarde: [{ code: 'A', aantal: 1 }] };
    expect(herkomstTekst(bron, [{ code: 'A', aantal: 1 }, { code: null, aantal: 3 }], celNaam, k)).toBe(
      'Door u gewijzigd. Voorbeeldregister geeft niet 3.',
    );
  });

  it('lists the rows of the source when the applicant emptied the table', () => {
    expect(tekst(undefined)).toBe(
      'Door u leeggemaakt. Voorbeeldregister geeft: Lid Een, voorzitter, 2026-01-01, 12500; ' +
        'Lid Twee, penningmeester, 2026-01-15.',
    );
  });
});

// What the UI spec has the system fill (src/ui_spec.rs): a value it knows, a
// source the cell computes, or the login.
describe('fields the system fills', () => {
  const systeem = {
    dagtekening: { waarde: '2028-03-06' },
    kanaal: { waarde: 'portaal' },
    elektronisch: { bron: 'r#6#langs_portaal' },
    nummer: {},
  };
  const voorgevuld = {
    naam: { bron: 'r#5h#naam', waarde: 'Uit de bron', herkomst: [] },
    elektronisch: { bron: 'r#6#langs_portaal', waarde: true, herkomst: [{ door: 'voorbeeld_register' }] },
  };

  it('asks the sources of the system with the prefill', () => {
    expect(systeemBronnen(systeem)).toEqual({ elektronisch: 'r#6#langs_portaal' });
  });

  it('fills over the scenario and the applicant, and leaves the login to the login', () => {
    const waarden = { dagtekening: '2020-01-01', kanaal: 'papier', naam: 'Eigen naam', nummer: '90123456' };
    expect(metSysteem(waarden, systeem, voorgevuld, new Set(['nummer']))).toEqual({
      dagtekening: '2028-03-06',
      kanaal: 'portaal',
      elektronisch: true,
      naam: 'Eigen naam',
      nummer: '90123456',
    });
  });

  it('leaves a field the login gives nothing empty', () => {
    const uit = metSysteem({ nummer: 'uit het voorbeeld' }, systeem, {}, new Set());
    expect('nummer' in uit && uit.nummer === undefined).toBe(true);
  });

  it('leaves a field empty when its source gives nothing', () => {
    const uit = metSysteem({ elektronisch: false }, systeem, {});
    expect('elektronisch' in uit && uit.elektronisch === undefined).toBe(true);
  });

  it('fills nothing for an application without a session, as one on paper', () => {
    const inloggen = { veld: 'nummer', inlogstap: { cel: 'dienst' } };
    expect(systeemVult(inloggen, null, systeem)).toEqual({});
    expect(systeemVult(inloggen, { claims: {} }, systeem)).toBe(systeem);
    // A case without a login step has no session to wait for.
    expect(systeemVult({ veld: 'nummer' }, null, systeem)).toBe(systeem);
  });

  it('says where the value comes from', () => {
    expect(systeemTekst(systeem.dagtekening, undefined, celNaam)).toBe('Ingevuld door het systeem.');
    expect(systeemTekst(systeem.elektronisch, voorgevuld.elektronisch, celNaam)).toMatch(/^Ingevuld uit Voorbeeldregister/);
    expect(systeemTekst(systeem.nummer, undefined, celNaam)).toBe('');
  });
});

describe('opHetFormulier', () => {
  const velden = [{ naam: 'naam' }, { naam: 'nummer' }, { naam: 'kanaal' }, { naam: 'termijn' }];

  it('shows neither what the login fills nor what the UI spec hides', () => {
    const getoond = opHetFormulier(velden, ['kanaal', 'termijn'], new Set(['nummer']));
    expect(getoond.map((v) => v.naam)).toEqual(['naam']);
  });

  it('submits a hidden field all the same', () => {
    // Hidden is about the form, not the application: its value stays in it.
    const waarden = metSysteem({ termijn: '2028-06-30' }, { kanaal: { waarde: 'portaal' } }, {});
    expect(waarden).toEqual({ termijn: '2028-06-30', kanaal: 'portaal' });
  });
});
