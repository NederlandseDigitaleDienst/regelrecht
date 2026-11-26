import { describe, it, expect } from 'vitest';
import { ApiError } from './apiFetch';
import {
  describeEngineLoadError,
  describeExecutionError,
  describeSaveError,
  rawErrorText,
  SCENARIO_WITHOUT_OUTPUT,
} from './userError';

describe('rawErrorText', () => {
  it('reads a bare string, as a WASM throw produces', () => {
    expect(rawErrorText('boom')).toBe('boom');
  });

  it('reads the message of an Error', () => {
    expect(rawErrorText(new Error('boom'))).toBe('boom');
  });

  it('falls back to String() for anything else', () => {
    expect(rawErrorText(42)).toBe('42');
  });
});

describe('describeEngineLoadError', () => {
  it('keeps the engine text as details and summarises in Dutch', () => {
    const raw = "Failed to load law: something the engine did not expect";
    const described = describeEngineLoadError(raw);
    expect(described.details).toBe(raw);
    expect(described.summary).toBe('Het regelwerk bevat een fout die eerst moet worden opgelost.');
  });

  it('points at the article a type-check finding names', () => {
    const described = describeEngineLoadError("article 2: 'huur' is not nullable, so comparing it with null cannot be true");
    expect(described.summary).toContain('Kijk bij artikel 2.');
  });

  it('points at the article when an output follows it', () => {
    const described = describeEngineLoadError(
      'Failed to load law: article 2a, output hoogte: operation X could not be read as an operation.',
    );
    expect(described.summary).toBe(
      'Het regelwerk bevat een fout die eerst moet worden opgelost. Kijk bij artikel 2a.',
    );
  });

  it('reads the articles off the findings of the type check', () => {
    const raw = [
      "Failed to load law: law 'kieswet' fails the type check (3 findings):",
      "  kieswet article B 1 output 'kiesgerechtigd': [N1] 'leeftijd' is not nullable",
      "  kieswet article B 1 action 2: [T2] cannot compare a number with a boolean",
      "  kieswet article B 5 input 'woonplaats': [N5] the source is nullable",
    ].join('\n');
    const described = describeEngineLoadError(raw);
    expect(described.summary).toBe('De engine vond 3 fouten in het regelwerk. Kijk bij de artikelen B 1 en B 5.');
    expect(described.details).toBe(raw);
  });

  it('keeps an article number with a space whole', () => {
    const described = describeEngineLoadError('Failed to load law: article B 1, output x: operation Y could not be read');
    expect(described.summary).toContain('Kijk bij artikel B 1.');
  });

  it('leaves out the article when the message is not a finding on it', () => {
    const described = describeEngineLoadError('Failed to load law: this law has no article 5');
    expect(described.summary).not.toContain('Kijk bij artikel');
  });

  it('says a regelwerk over the size limit is too large', () => {
    const described = describeEngineLoadError('YAML exceeds maximum size (1048576 bytes)');
    expect(described.summary).toBe('Het regelwerk is te groot om uit te voeren.');
  });

  it('recognises a YAML parse error', () => {
    const described = describeEngineLoadError('YAML parse error: mapping values are not allowed at line 3 column 7');
    expect(described.summary).toBe('Het regelwerk is niet geldig en kan niet worden gelezen.');
  });
});

describe('describeExecutionError', () => {
  it('names the regelwerk and date when no version is in force', () => {
    const described = describeExecutionError(
      "No version of law 'voorbeeldwet' in force on 2020-01-01 (not yet in force)",
    );
    expect(described.summary).toBe('Er is geen versie van voorbeeldwet die geldt op 2020-01-01.');
  });

  it('names a regelwerk that is not loaded', () => {
    const described = describeExecutionError("Law 'voorbeeldwet' not found. Load it first with loadLaw().");
    expect(described.summary).toBe('Het regelwerk voorbeeldwet is niet geladen.');
  });

  it("keeps the run's own sentence for a scenario without an output", () => {
    expect(describeExecutionError(SCENARIO_WITHOUT_OUTPUT)).toEqual({
      summary: SCENARIO_WITHOUT_OUTPUT,
      details: null,
    });
  });

  it('names the regelwerk the engine refused', () => {
    const raw = "Regelwerk 'voorbeeldwet' is niet geladen: Failed to load law: article 2: something";
    expect(describeExecutionError(raw)).toEqual({
      summary: 'Het regelwerk voorbeeldwet bevat een fout en is daarom niet geladen.',
      details: raw,
    });
  });

  it('uses a generic sentence for an unknown failure', () => {
    const described = describeExecutionError('Resolution error: nothing matched');
    expect(described.summary).toBe('Het scenario kon niet worden uitgevoerd.');
    expect(described.details).toBe('Resolution error: nothing matched');
  });
});

describe('describeSaveError', () => {
  it('moves a response body to the details, with the status', () => {
    const body = 'schema validation failed: /articles/1/machine_readable: unexpected key';
    const described = describeSaveError(new ApiError(body, { status: 400, body }));
    expect(described.summary).toBe('De server heeft het regelwerk geweigerd.');
    expect(described.details).toBe(`HTTP 400\n${body}`);
  });

  it('explains a missing permission', () => {
    const described = describeSaveError(new ApiError('read-only', { status: 403, body: 'read-only' }));
    expect(described.summary).toBe('Je hebt geen rechten om dit regelwerk op te slaan.');
  });

  it('explains a server failure', () => {
    const described = describeSaveError(new ApiError('Save failed: 502', { status: 502 }));
    expect(described.summary).toBe('De server kon het regelwerk niet opslaan. Probeer het later opnieuw.');
    expect(described.details).toBe('HTTP 502\nSave failed: 502');
  });

  it('explains a request that got no response', () => {
    const described = describeSaveError(new TypeError('Failed to fetch'));
    expect(described.summary).toContain('Geen verbinding met de server.');
    expect(described.details).toBe('Failed to fetch');
  });

  it('does not blame the connection for another TypeError', () => {
    const described = describeSaveError(new TypeError("Cannot read properties of undefined (reading 'x')"));
    expect(described.summary).toBe('Het regelwerk kon niet worden opgeslagen.');
    expect(described.details).toContain('Cannot read properties');
  });

  it("takes the editor's own message as the summary, without details", () => {
    const described = describeSaveError(new Error('Het regelwerk is intussen door iemand anders gewijzigd.'));
    expect(described).toEqual({
      summary: 'Het regelwerk is intussen door iemand anders gewijzigd.',
      details: null,
    });
  });
});

describe('describeEngineLoadError, with the engine\'s records', () => {
  const finding = (article: string, name: string, rule: string) => ({
    severity: 'error' as const,
    kind: 'type_check' as const,
    law_id: 'kieswet',
    article,
    place: { kind: 'output' as const, name },
    position: null,
    rule,
    message: `something about ${name}`,
  });

  it('counts the findings and names their articles, from the records', () => {
    const thrown = {
      // A sentence the patterns would read differently: the records win.
      error: 'Failed to load law: something the patterns do not know',
      diagnostics: [finding('B 1', 'a', 'N1'), finding('B 1', 'b', 'T2'), finding('B 5', 'c', 'N5')],
    };
    const described = describeEngineLoadError(thrown);
    expect(described.summary).toBe('De engine vond 3 fouten in het regelwerk. Kijk bij de artikelen B 1 en B 5.');
    expect(described.details).toBe(thrown.error);
    expect(described.diagnostics).toHaveLength(3);
  });

  it('says whose articles they are when the findings are in a regelwerk that depends on this one', () => {
    const elsewhere = (lawId: string, article: string) => ({ ...finding(article, 'a', 'T5'), kind: 'cross_law' as const, law_id: lawId });
    expect(describeEngineLoadError({ error: 'x', diagnostics: [elsewhere('wet_a', '2'), elsewhere('wet_a', '4')] }).summary)
      .toBe('Door dit regelwerk klopt Wet A niet meer. Kijk daar bij de artikelen 2 en 4.');
    // Two regelwerken can each have an article 2, so no article is named.
    expect(describeEngineLoadError({ error: 'x', diagnostics: [elsewhere('wet_a', '2'), elsewhere('wet_b', '2')] }).summary)
      .toBe('Door dit regelwerk klopt Wet A en Wet B niet meer.');
  });

  it('points at the line of a text that cannot be read', () => {
    const described = describeEngineLoadError({
      error: 'YAML parse error: did not find expected node at line 12 column 3',
      diagnostics: [{
        severity: 'error', kind: 'structure', law_id: null, article: null, place: null,
        position: { line: 12, column: 3 }, rule: null, message: 'did not find expected node',
      }],
    });
    expect(described.summary).toBe('Het regelwerk is niet geldig en kan niet worden gelezen. Kijk bij regel 12 van de YAML.');
  });

  it('falls back on the sentence for a record without a place', () => {
    const described = describeEngineLoadError({
      error: 'Failed to load law: article 2, output x: operation Y could not be read',
      diagnostics: [{
        severity: 'error', kind: 'load_check', law_id: null, article: null, place: null,
        position: null, rule: null, message: 'article 2, output x: operation Y could not be read',
      }],
    });
    expect(described.summary).toContain('Kijk bij artikel 2.');
  });
});
