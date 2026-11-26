import { describe, it, expect, vi } from 'vitest';
import {
  MAX_SCENARIO_BYTES,
  checkImport,
  checkImports,
  filenameProblem,
  lawsNamed,
  parseCandidate,
  type ImportContext,
} from './scenarioImport';

const FITS = `Feature: Basis

  Background:
    Given law "wet_b" is loaded

  Scenario: Heeft recht
    Given the calculation date is "2025-01-01"
    Given parameter "inkomen" is 1
    When I evaluate "heeft_recht" of "wet_a"
    Then output "heeft_recht" is true
`;

const OTHER_TARGET = FITS.replace('of "wet_a"', 'of "wet_c"');
const NO_SCENARIOS = 'Feature: Leeg\n';
const BROKEN = 'Feature: Kapot\n  Scenario: a\n    Given x\n  Feature: tweede\n';
const UNKNOWN_STEP = FITS.replace('Then output "heeft_recht" is true', 'Then output "heeft_recht" is true\n    And the moon is full');
const NO_EXECUTION = `Feature: Alleen opzet

  Scenario: Zonder uitvoering
    Given the calculation date is "2025-01-01"
`;

function context(overrides: Partial<ImportContext> = {}): ImportContext {
  return {
    lawId: 'wet_a',
    existing: [],
    knownLaws: new Set(['wet_b', 'wet_c']),
    lawOutputs: new Set(['heeft_recht']),
    ...overrides,
  };
}

const check = (text: string, ctx: Partial<ImportContext> = {}, filename = 'nieuw.feature') =>
  checkImport({ filename, text }, context(ctx));

describe('checkImport', () => {
  it('accepts a file that targets the open regelwerk and loads only known ones', () => {
    const v = check(FITS);
    expect(v.status).toBe('ok');
    expect(v.reason).toBe('1 scenario, past bij dit regelwerk');
  });

  it('refuses a file that does not parse', () => {
    const v = check(BROKEN);
    expect(v.status).toBe('error');
    expect(v.reason).toMatch(/^Geen geldig Gherkin: /);
    // One line, not the parser's list of expected tokens.
    expect(v.reason).not.toContain('\n');
  });

  it('refuses a file without scenarios', () => {
    expect(check(NO_SCENARIOS)).toMatchObject({ status: 'error', reason: "Het bestand bevat geen scenario's" });
  });

  it('refuses a file that loads a regelwerk the corpus lacks', () => {
    const v = check(FITS, { knownLaws: new Set() });
    expect(v).toMatchObject({ status: 'error', reason: 'Niet in het corpus van dit traject: wet_b' });
  });

  it('refuses a file that runs another regelwerk', () => {
    const v = check(OTHER_TARGET);
    expect(v).toMatchObject({ status: 'error', reason: 'Rekent met een ander regelwerk: wet_c' });
  });

  it('refuses a name already in the folder in another case', () => {
    expect(check(FITS, { existing: ['NIEUW.feature'] }).status).toBe('error');
  });

  it('refuses a filename already in the folder', () => {
    const v = check(FITS, { existing: ['nieuw.feature'] });
    expect(v).toMatchObject({ status: 'error', reason: 'Er bestaat al een scenariobestand met deze naam' });
  });

  it('refuses the second file of one name in a selection', () => {
    const v = checkImport({ filename: 'nieuw.feature', text: FITS }, context(), ['Nieuw.feature']);
    expect(v).toMatchObject({ status: 'error', reason: 'Er is al een bestand met deze naam gekozen' });
  });

  it('refuses a file over the size limit', () => {
    const v = checkImport({ filename: 'groot.feature', text: FITS, size: MAX_SCENARIO_BYTES + 1 }, context());
    expect(v).toMatchObject({ status: 'error', reason: 'Het bestand is groter dan 1 MB' });
  });

  it('warns about an output the open regelwerk does not declare', () => {
    const v = check(FITS, { lawOutputs: new Set(['iets_anders']) });
    expect(v).toMatchObject({ status: 'warning', reason: '1 scenario; onbekende uitvoer: heeft_recht' });
  });

  it('skips the output check when the outputs could not be read', () => {
    expect(check(FITS, { lawOutputs: null }).status).toBe('ok');
  });

  it('warns about steps the editor does not recognise', () => {
    expect(check(UNKNOWN_STEP)).toMatchObject({ status: 'warning', reason: '1 scenario; 1 stap herkent de editor niet' });
  });

  it('warns about a file that executes nothing', () => {
    expect(check(NO_EXECUTION)).toMatchObject({ status: 'warning', reason: '1 scenario; geen stap rekent dit regelwerk uit' });
  });
});

describe('filenameProblem', () => {
  it('mirrors validate_scenario_filename', () => {
    expect(filenameProblem('a.feature')).toBeNull();
    expect(filenameProblem('a.txt')).toMatch(/\.feature/);
    expect(filenameProblem('a..b.feature')).not.toBeNull();
    expect(filenameProblem('a\\b.feature')).not.toBeNull();
  });
});

describe('lawsNamed', () => {
  it('lists the regelwerken the files name, without the open one', () => {
    const states = [FITS, OTHER_TARGET].map((text) => {
      const p = parseCandidate(text);
      return 'state' in p ? p.state : null;
    });
    expect(lawsNamed([...states, null], 'wet_a').sort()).toEqual(['wet_b', 'wet_c']);
  });
});

describe('checkImports', () => {
  it('asks the corpus once for the named regelwerken and once for the outputs', async () => {
    const corpus = {
      searchLaws: vi.fn(async () => ({ laws: [{ law_id: 'wet_b' }], total: 1 })),
      outputs: vi.fn(async () => [{ name: 'heeft_recht' }]),
    };
    const verdicts = await checkImports(
      [
        { filename: 'a.feature', text: FITS },
        { filename: 'a.feature', text: FITS },
        { filename: 'b.feature', text: OTHER_TARGET },
      ],
      { lawId: 'wet_a', existing: [] },
      corpus,
    );
    expect(verdicts.map((v) => v.status)).toEqual(['ok', 'error', 'error']);
    expect(corpus.searchLaws).toHaveBeenCalledTimes(1);
    expect(corpus.searchLaws).toHaveBeenCalledWith({ ids: ['wet_b', 'wet_c'], limit: 2 });
    expect(corpus.outputs).toHaveBeenCalledWith('wet_a');
  });

  it('does not look up the regelwerken of a file it already refused', async () => {
    const corpus = {
      searchLaws: vi.fn(async () => ({ laws: [], total: 0 })),
      outputs: vi.fn(async () => []),
    };
    const [v] = await checkImports([{ filename: 'a.txt', text: OTHER_TARGET }], { lawId: 'wet_a', existing: [] }, corpus);
    expect(v?.status).toBe('error');
    expect(corpus.searchLaws).not.toHaveBeenCalled();
  });

  it('rejects when the corpus cannot say which regelwerken it has', async () => {
    const corpus = {
      searchLaws: async () => {
        throw new Error('502');
      },
      outputs: async () => [],
    };
    await expect(
      checkImports([{ filename: 'a.feature', text: FITS }], { lawId: 'wet_a', existing: [] }, corpus),
    ).rejects.toThrow('502');
  });

  it('still checks when the outputs cannot be read', async () => {
    const corpus = {
      searchLaws: async () => ({ laws: [{ law_id: 'wet_b' }], total: 1 }),
      outputs: async () => {
        throw new Error('502');
      },
    };
    const [v] = await checkImports([{ filename: 'a.feature', text: FITS }], { lawId: 'wet_a', existing: [] }, corpus);
    expect(v?.status).toBe('ok');
  });
});
