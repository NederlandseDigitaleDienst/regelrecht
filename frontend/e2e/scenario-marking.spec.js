/**
 * A scenario that reaches an article with a marking nobody accepted. The
 * engine does not run such an article; the result says so in words, with the
 * marking in its parts and the way to the article one click away.
 *
 * Requires the WASM engine to be built (frontend/public/wasm/pkg).
 */
import { test, expect } from '@playwright/test';
import { loadFixture, mockAuthedEditor, clearOpenTabsStorage, TEST_TRAJECT_REF } from './helpers.js';
import { mockCorpusApi } from './helpers-corpus.js';

const FILE = 'marking.feature';
const SCENARIO = `Feature: An article with a marking

  Scenario: The marked article is not run
    Given the calculation date is "2025-01-01"
    Given parameter "bsn" is "999993653"
    When I evaluate "heeft_recht_op_zorgtoeslag" of "wet_op_de_zorgtoeslag"
    Then the execution succeeds
`;

// Article 2 gets a marking that was not accepted.
const withMarking = (yamlText) => yamlText.replace(
  '    machine_readable:\n      definitions:\n        drempelinkomen_alleenstaande:',
  `    machine_readable:
      markings:
        - about: het verschil tussen de standaardpremie en de normpremie
          reason: Een verzonnen reden voor deze test.
          resolution: operation
          resolved_by: Een bewerking die nog niet bestaat
          target: []
          legal_text_excerpt: dat verschil
          accepted: false
      definitions:
        drempelinkomen_alleenstaande:`,
);

test('a scenario on an article with an unaccepted marking says what to do', async ({ page }) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await clearOpenTabsStorage(page);
  const content = withMarking(loadFixture('zorgtoeslag-full.yaml'));
  expect(content).toContain('markings:');
  const corpus = new Map([['wet_op_de_zorgtoeslag', { content, path: '', pubDate: '2024-12-20' }]]);
  await mockCorpusApi(page, corpus, { id: 'wet_op_de_zorgtoeslag', scenarioFilename: FILE }, SCENARIO);
  await mockAuthedEditor(page);
  await page.goto(`/trajecten/${TEST_TRAJECT_REF}/editor/wet_op_de_zorgtoeslag/scenarios/${FILE}`);

  const result = page.locator('[data-testid="scenario-result-pane"]');
  const error = result.locator('[data-testid="execution-error"]');
  await expect(error).toBeVisible({ timeout: 30_000 });
  await expect(error).toHaveAttribute('text', 'Artikel 2 van Wet Op De Zorgtoeslag heeft een markering die nog niet is geaccepteerd');
  await expect(error).toHaveAttribute('supporting-text', /Beoordeel de markering en accepteer haar/);
  // The form beside it says the same in a line, not the engine's text.
  await expect(page.locator('[data-testid="scenario-run-error"]')).toHaveAttribute('text', /heeft een markering die nog niet is geaccepteerd/);
  // The trace up to the failure is still there, under the message.
  await expect(result.locator('nldd-code-viewer')).toContainText('Execution failed');

  await result.locator('[data-testid="execution-error-details"]').click();
  const sheet = page.locator('[data-testid="error-details-sheet"]');
  const parts = sheet.locator('[data-testid="error-details-marking-part"] nldd-text-cell');
  await expect(parts).toHaveCount(3);
  await expect(parts.nth(0)).toHaveAttribute('overline', 'Waar het om gaat');
  await expect(parts.nth(0)).toHaveAttribute('text', 'het verschil tussen de standaardpremie en de normpremie');
  await expect(parts.nth(2)).toHaveAttribute('text', 'Een nieuwe bewerking in de engine: Een bewerking die nog niet bestaat');

  await sheet.locator('[data-testid="error-details-finding"]').click();
  await expect(page).toHaveURL(/\/editor\/wet_op_de_zorgtoeslag\/2$/);
});
