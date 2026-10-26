import { describe, expect, it } from 'vitest';
import {
  collectKeyValues,
  keyFieldsFor,
  lawShape,
  materialiseAll,
  materialiseRecord,
  tablesFromProfiles,
} from './materialize.js';
import type { Binding, LawShape } from './materialize.js';


/**
 * Bindings for one test, written with only the fields that test needs (so
 * `service` may be left out); the materialiser reads them as `Binding`s.
 */
const bindingsOf = <K extends string>(b: Record<K, Partial<Binding>>): Record<K, Binding> => b as Record<K, Binding>;

const LAW = {
  $id: 'test_wet',
  articles: [
    {
      number: '1',
      machine_readable: {
        execution: {
          parameters: [{ name: 'bsn', type: 'string' }, { name: 'aanvraag_bedrag', type: 'amount' }],
          input: [
            { name: 'geboortedatum', type: 'date', source: {} },
            { name: 'inkomen', type: 'amount', source: {} },
            { name: 'kinderen', type: 'array', source: {} },
            { name: 'partner_bsn', type: 'string', source: {} },
            { name: 'partner_inkomen', type: 'amount', source: {} },
            { name: 'eigen_verklaring', type: 'boolean', source: {} },
            { name: 'huur', type: 'amount', source: {} },
            { name: 'adres', type: 'object', source: {} },
          ],
        },
      },
    },
  ],
};

const BINDINGS = {
  // No `absent`: a missing person is unknown to the register.
  geboortedatum: {
    kind: 'table',
    service: 'RvIG',
    table: 'personen',
    field: 'geboortedatum',
    select_on: [{ name: 'bsn', value: '$bsn' }],
  },
  // The tax register is authoritative: no row is no such income.
  inkomen: {
    kind: 'table',
    service: 'BELASTINGDIENST',
    table: 'inkomen',
    field: 'bedrag',
    select_on: [{ name: 'bsn', value: '$bsn' }],
    absent: 0,
  },
  kinderen: {
    kind: 'table',
    service: 'RvIG',
    table: 'kinderen',
    field: 'kind_bsn',
    select_on: [{ name: 'ouder_bsn', value: '$bsn' }],
  },
  // The BRP is authoritative for relationships: no row is no partner.
  partner_bsn: {
    kind: 'table',
    service: 'RvIG',
    table: 'relaties',
    field: 'partner_bsn',
    select_on: [{ name: 'bsn', value: '$bsn' }],
    absent: null,
  },
  // Selects on another input of the same law, resolved after `partner_bsn`.
  partner_inkomen: {
    kind: 'table',
    service: 'BELASTINGDIENST',
    table: 'inkomen',
    field: 'bedrag',
    select_on: [{ name: 'bsn', value: '$partner_bsn' }],
    absent: 0,
  },
  eigen_verklaring: { kind: 'claim', service: 'TOESLAGEN' },
  // The rent register says there is none for a homeowner.
  huur: {
    kind: 'table',
    service: 'TOESLAGEN',
    table: 'huur',
    field: 'bedrag',
    select_on: [{ name: 'bsn', value: '$bsn' }],
    absent: null,
  },
  adres: {
    kind: 'table',
    service: 'RvIG',
    table: 'verblijfplaats',
    fields: ['straat', 'huisnummer', 'postcode'],
    select_on: [{ name: 'bsn', value: '$bsn' }],
    absent: null,
  },
} satisfies Record<string, Binding>;

const TABLES: Record<string, Record<string, unknown>[]> = {
  'RvIG personen': [{ bsn: '100000001', geboortedatum: '1990-01-01' }, { bsn: '100000002' }],
  'BELASTINGDIENST inkomen': [{ bsn: '100000001', bedrag: 2500000 }, { bsn: '100000003', bedrag: 4000000 }],
  'RvIG relaties': [{ bsn: '100000001', partner_bsn: null }, { bsn: '100000002', partner_bsn: '100000003' }],
  'RvIG kinderen': [
    { ouder_bsn: '100000001', kind_bsn: '200000001' },
    { ouder_bsn: '100000001', kind_bsn: '200000002' },
    { ouder_bsn: '100000009', kind_bsn: '200000009' },
  ],
  'RvIG verblijfplaats': [{ bsn: '100000001', straat: 'Kalverstraat', huisnummer: '1' }],
};
const rowsFor = (service: string, table: string) => TABLES[`${service} ${table}`] ?? [];

describe('lawShape', () => {
  it('reads parameters and input types across articles', () => {
    const shape = lawShape(LAW);
    expect(shape.id).toBe('test_wet');
    expect(shape.parameters).toEqual(['bsn', 'aanvraag_bedrag']);
    expect(shape.inputTypes.inkomen).toBe('amount');
    expect(shape.inputTypes.kinderen).toBe('array');
  });
});

describe('keyFieldsFor', () => {
  it('keys on the parameter the bindings select on', () => {
    expect(keyFieldsFor(lawShape(LAW), BINDINGS)).toEqual(['bsn']);
  });

  it('falls back to the first parameter without any selecting binding', () => {
    expect(keyFieldsFor(lawShape(LAW), { eigen_verklaring: BINDINGS.eigen_verklaring })).toEqual(['bsn']);
  });

  it('keys on the identity only, never on a form parameter a binding selects on', () => {
    const shape: LawShape = { id: 'x', parameters: ['kvk_nummer', 'seizoen', 'terras_locatie'], inputTypes: {} };
    const bindings = bindingsOf({
      a: { kind: 'table', select_on: [{ name: 'kvk_nummer', value: '$kvk_nummer' }] },
      b: { kind: 'table', select_on: [{ name: 'seizoen', value: '$seizoen' }, { name: 'locatie', value: '$terras_locatie' }] },
      c: { kind: 'table', select_on: [{ name: 'seizoen', value: '$seizoen' }] },
    });
    expect(keyFieldsFor(shape, bindings)).toEqual(['kvk_nummer']);
    const noIdentity: LawShape = { id: 'y', parameters: ['adres', 'jaar'], inputTypes: {} };
    expect(keyFieldsFor(noIdentity, bindingsOf({ a: { kind: 'table', select_on: [{ name: 'adres', value: '$adres' }] } }))).toEqual(['adres']);
  });

  it('prefers bsn on a tie', () => {
    const shape: LawShape = { id: 'x', parameters: ['kvk_nummer', 'bsn'], inputTypes: {} };
    const bindings = bindingsOf({
      a: { kind: 'table', select_on: [{ name: 'kvk_nummer', value: '$kvk_nummer' }] },
      b: { kind: 'table', select_on: [{ name: 'bsn', value: '$bsn' }] },
    });
    expect(keyFieldsFor(shape, bindings)[0]).toBe('bsn');
  });
});

describe('materialiseRecord', () => {
  const shape = lawShape(LAW);

  it('projects the selected row per input and remembers the owning service', () => {
    const { record, sources } = materialiseRecord(shape, BINDINGS, { bsn: '100000001' }, rowsFor);
    expect(record.geboortedatum).toBe('1990-01-01');
    expect(record.inkomen).toBe(2500000);
    expect(sources.inkomen).toBe('BELASTINGDIENST');
  });

  it('collects every matching row for an array input; no rows is an empty list', () => {
    expect(materialiseRecord(shape, BINDINGS, { bsn: '100000001' }, rowsFor).record.kinderen).toEqual(['200000001', '200000002']);
    expect(materialiseRecord(shape, BINDINGS, { bsn: '999' }, rowsFor).record.kinderen).toEqual([]);
  });

  it('follows the binding for a missing row: omit (unknown), null (absent) or 0 (counts nothing)', () => {
    const { record, sources } = materialiseRecord(shape, BINDINGS, { bsn: '999' }, rowsFor);
    // No `absent`: the register does not know this person, so the key is left out.
    expect('geboortedatum' in record).toBe(false);
    expect('geboortedatum' in sources).toBe(false);
    // absent: 0 and absent: null are written as such.
    expect(record.inkomen).toBe(0);
    expect(record.partner_bsn).toBeNull();
    expect(record.huur).toBeNull();
    expect(record.adres).toBeNull();
    expect(Object.values(record)).not.toContain(undefined);
  });

  it('treats absent: unknown the same as no absent key', () => {
    const bindings = bindingsOf({ geboortedatum: { ...BINDINGS.geboortedatum, absent: 'unknown' } });
    expect('geboortedatum' in materialiseRecord(shape, bindings, { bsn: '999' }, rowsFor).record).toBe(false);
  });

  it('never fills from the type: an amount without a row and without absent is unknown', () => {
    const bindings = bindingsOf({ inkomen: { ...BINDINGS.inkomen, absent: undefined } });
    delete bindings.inkomen.absent;
    expect('inkomen' in materialiseRecord(shape, bindings, { bsn: '999' }, rowsFor).record).toBe(false);
  });

  it('lets a matched row without the column follow absent, keeps an explicit null', () => {
    // 100000002 has a personen row without geboortedatum; no `absent`: unknown.
    const { record } = materialiseRecord(shape, BINDINGS, { bsn: '100000002' }, rowsFor);
    expect('geboortedatum' in record).toBe(false);
    // A detention row without a status column, in a register authoritative for absence: null.
    const bindings = bindingsOf({ huur: { ...BINDINGS.huur, table: 'personen', field: 'status' } });
    expect(materialiseRecord(shape, bindings, { bsn: '100000001' }, rowsFor).record.huur).toBeNull();
    expect(materialiseRecord(shape, { huur: { ...bindings.huur, absent: 0 } }, { bsn: '100000001' }, rowsFor).record.huur).toBe(0);
    // 100000001's relaties row says partner_bsn is null: the data states an absence.
    expect(materialiseRecord(shape, BINDINGS, { bsn: '100000001' }, rowsFor).record.partner_bsn).toBeNull();
  });

  it('bundles fields into an object with null for a listed column the row lacks', () => {
    const { record } = materialiseRecord(shape, BINDINGS, { bsn: '100000001' }, rowsFor);
    expect(record.adres).toEqual({ straat: 'Kalverstraat', huisnummer: '1', postcode: null });
  });

  it('makes a lookup on an absent selector null, whatever absent says', () => {
    // 100000001 has no partner: the partner's income is not unknown, there is nobody to look up.
    const { record } = materialiseRecord(shape, BINDINGS, { bsn: '100000001' }, rowsFor);
    expect(record.partner_inkomen).toBeNull();
    // 100000002 has a partner with an income row.
    expect(materialiseRecord(shape, BINDINGS, { bsn: '100000002' }, rowsFor).record.partner_inkomen).toBe(4000000);
  });

  it('leaves a claim-only input out until the citizen supplies it', () => {
    const { record, sources } = materialiseRecord(shape, BINDINGS, { bsn: '100000001' }, rowsFor);
    expect('eigen_verklaring' in record).toBe(false);
    expect('eigen_verklaring' in sources).toBe(false);
  });

  it('leaves an input out whose selector the sources cannot resolve', () => {
    const bindings = bindingsOf({
      partner_inkomen: { ...BINDINGS.partner_inkomen, select_on: [{ name: 'bsn', value: '$onbekend' }] },
    });
    expect('partner_inkomen' in materialiseRecord(shape, bindings, { bsn: '5' }, rowsFor).record).toBe(false);
  });

  it('asks the resolver for a cross-law input used as selector', () => {
    const bindings = bindingsOf({
      partner_inkomen: { ...BINDINGS.partner_inkomen, select_on: [{ name: 'bsn', value: '$partner_bsn' }] },
    });
    const resolveRef = (lawId: string, name: string) => (lawId === 'test_wet' && name === 'partner_bsn' ? '100000001' : undefined);
    const { record } = materialiseRecord(shape, bindings, { bsn: '5' }, rowsFor, { resolveRef });
    expect(record.partner_inkomen).toBe(2500000);
  });

  it('leaves events, laws and reference_data out: the demo has no source for them', () => {
    const bindings = bindingsOf({ huur: { kind: 'events', service: 'JenV', table: 'events', fields: ['case_id'], select_on: [] } });
    expect(materialiseRecord(shape, bindings, { bsn: '1' }, rowsFor).record).toEqual({});
  });

  it('reads kind: cases from the demo case store', () => {
    const bindings = bindingsOf({
      kinderen: {
        kind: 'cases',
        service: 'GEMEENTE',
        select_on: [
          { name: 'status', value: 'DECIDED' },
          { name: 'bsn', value: '$bsn' },
        ],
      },
    });
    const cases = [
      { status: 'DECIDED', bsn: '100000001', id: 'a' },
      { status: 'SUBMITTED', bsn: '100000001', id: 'b' },
      { status: 'DECIDED', bsn: '2', id: 'c' },
    ];
    const { record } = materialiseRecord(shape, bindings, { bsn: '100000001' }, rowsFor, { cases });
    expect(record.kinderen.map((c: { id: string }) => c.id)).toEqual(['a']);
  });

  it('reads one value from the latest matching case when the binding names a field', () => {
    // The permit the municipality granted and the area it granted it for.
    const bindings = bindingsOf({
      oppervlakte: { kind: 'cases', service: 'GEMEENTE', field: 'terras_oppervlakte', absent: 0, select_on: [{ name: 'status', value: 'DECIDED' }, { name: 'bsn', value: '$bsn' }] },
    });
    // Newest first, as the demo store keeps them (`state.cases.unshift`).
    const cases = [
      { status: 'SUBMITTED', bsn: '100000001', terras_oppervlakte: 90 },
      { status: 'DECIDED', bsn: '100000001', terras_oppervlakte: 60 },
      { status: 'DECIDED', bsn: '100000001', terras_oppervlakte: 40 },
    ];
    expect(materialiseRecord(shape, bindings, { bsn: '100000001' }, rowsFor, { cases }).record.oppervlakte).toBe(60);
    // No granted case: what `absent` says, as for a register without a row.
    expect(materialiseRecord(shape, bindings, { bsn: '2' }, rowsFor, { cases }).record.oppervlakte).toBe(0);
  });
});

describe('materialiseAll', () => {
  it('groups records per law, service and key field, with keys only for stated values', () => {
    const out = materialiseAll({ test_wet: LAW }, { test_wet: BINDINGS }, rowsFor, { bsn: ['100000001', '999'] });
    const rvig = out.find((s) => s.service === 'RvIG');
    expect(rvig).toMatchObject({ law: 'test_wet', keyField: 'bsn' });
    expect(rvig?.records).toHaveLength(2);
    expect(rvig?.records[0]).toMatchObject({ bsn: '100000001', geboortedatum: '1990-01-01' });
    // 999 is unknown to the BRP: its record states the absences (partner, adres) and nothing else.
    expect(rvig?.records[1]).toEqual({ bsn: '999', partner_bsn: null, adres: null, kinderen: [] });
    // A claim-only service registers nothing.
    expect(out.map((s) => s.service).sort()).toEqual(['BELASTINGDIENST', 'RvIG', 'TOESLAGEN']);
    expect(out.find((s) => s.service === 'TOESLAGEN')?.records).toEqual([{ bsn: '100000001', huur: null }, { bsn: '999', huur: null }]);
    for (const source of out) for (const r of source.records) expect(Object.values(r)).not.toContain(undefined);
  });

  it('lets paramsFor supply per-key parameters that bindings select on', () => {
    const law = {
      $id: 'terras',
      articles: [{ machine_readable: { execution: { parameters: [{ name: 'kvk_nummer' }, { name: 'terras_locatie' }], input: [{ name: 'beschikbaar', type: 'number', source: {} }] } } }],
    };
    const bindings = {
      terras: bindingsOf({
        beschikbaar: { kind: 'table', service: 'GEMEENTE', table: 'locaties', field: 'oppervlakte', select_on: [{ name: 'kvk_nummer', value: '$kvk_nummer' }, { name: 'locatie', value: '$terras_locatie' }] },
      }),
    };
    const rows = (service: string, table: string) => (table === 'locaties' ? [{ kvk_nummer: '1', locatie: 'voor', oppervlakte: 30 }, { kvk_nummer: '1', locatie: 'zij', oppervlakte: 12 }] : []);
    // Without the form answer no row matches and the space is unknown: nothing is registered.
    expect(materialiseAll({ terras: law }, bindings, rows, { kvk_nummer: ['1'] })).toEqual([]);
    const withForm = materialiseAll({ terras: law }, bindings, rows, { kvk_nummer: ['1'] }, { paramsFor: () => ({ terras_locatie: 'zij' }) });
    expect(withForm[0]?.records[0]?.beschikbaar).toBe(12);
  });

  it('skips laws that are not loaded and keys without values', () => {
    expect(materialiseAll({}, { test_wet: BINDINGS }, rowsFor, { bsn: ['1'] })).toEqual([]);
    expect(materialiseAll({ test_wet: LAW }, { test_wet: BINDINGS }, rowsFor, {})).toEqual([]);
  });
});

describe('tablesFromProfiles / collectKeyValues', () => {
  const profiles = {
    globalServices: { CBS: { levensverwachting: [{ jaar: 2025, waarde: 20.5 }] } },
    profiles: {
      '100000001': { sources: { RvIG: { personen: [{ bsn: '100000001' }] } } },
      '999999990': { sources: { RvIG: { personen: [{ bsn: '999999990' }] }, KVK: { inschrijvingen: [{ kvk_nummer: '85234567' }] } } },
    },
  };

  it('concatenates the rows of every persona per table', () => {
    const rowsFor = tablesFromProfiles(profiles);
    expect(rowsFor('RvIG', 'personen')).toHaveLength(2);
    expect(rowsFor('CBS', 'levensverwachting')).toHaveLength(1);
    expect(rowsFor('KVK', 'onbekend')).toEqual([]);
  });

  it('collects the distinct key values as strings', () => {
    const rowsFor = tablesFromProfiles(profiles);
    const all = [rowsFor('RvIG', 'personen'), rowsFor('KVK', 'inschrijvingen')];
    expect(collectKeyValues(all, ['bsn', 'kvk_nummer'])).toEqual({
      bsn: ['100000001', '999999990'],
      kvk_nummer: ['85234567'],
    });
  });
});

describe('een onbekende uitkomst in een besloten zaak', () => {
  it('blijft onbekend in plaats van als waarde door te gaan', () => {
    // No id: this test runs one record, without a law to resolve against.
    const shape = { parameters: ['kvk_nummer'], inputTypes: { vergunde_oppervlakte: 'number' } } as Omit<LawShape, 'id'> as LawShape;
    const bindings = bindingsOf({ vergunde_oppervlakte: { kind: 'cases', field: 'vergunde_oppervlakte', select_on: [{ name: 'kvk_nummer', value: '$kvk_nummer' }], service: 'GEMEENTE_ROTTERDAM', absent: 0 } });
    const cases = [{ kvk_nummer: '1', vergunde_oppervlakte: { __unknown: true, missing: [] } }];
    const { record } = materialiseRecord(shape, bindings, { kvk_nummer: '1' }, () => [], { cases });
    expect('vergunde_oppervlakte' in record).toBe(false);
    // Ter vergelijking: een bekende waarde gaat wel door.
    const known = materialiseRecord(shape, bindings, { kvk_nummer: '1' }, () => [], { cases: [{ kvk_nummer: '1', vergunde_oppervlakte: 30 }] });
    expect(known.record.vergunde_oppervlakte).toBe(30);
  });
});
