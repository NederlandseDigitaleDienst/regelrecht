/**
 * Gherkin parser - wraps @cucumber/gherkin to produce a simplified AST.
 *
 * Returns { feature, scenarios[] } where each scenario has steps[]
 * and optional dataTables.
 */
import * as Gherkin from '@cucumber/gherkin';
import * as Messages from '@cucumber/messages';

/** A step in the runner's simplified form. */
export interface Step {
  keyword: string;
  text: string;
  dataTable?: string[][];
  docString?: string;
}

export interface Scenario {
  name: string;
  tags: string[];
  steps: Step[];
}

export interface ParsedFeature {
  feature: string;
  background: Step[] | null;
  scenarios: Scenario[];
}

/** Parse a .feature text into a structured object. */
export function parseFeature(featureText: string): ParsedFeature {
  const uuidFn = Messages.IdGenerator.uuid();
  const builder = new Gherkin.AstBuilder(uuidFn);
  const matcher = new Gherkin.GherkinClassicTokenMatcher();
  const parser = new Gherkin.Parser(builder, matcher);

  const gherkinDocument = parser.parse(featureText);
  const feature = gherkinDocument.feature;

  if (!feature) {
    return { feature: '', background: null, scenarios: [] };
  }

  let background: Step[] | null = null;
  const scenarios: Scenario[] = [];

  for (const child of feature.children) {
    if (child.background) {
      background = child.background.steps.map(mapStep);
    }
    if (child.scenario) {
      scenarios.push({
        name: child.scenario.name,
        tags: child.scenario.tags.map((t) => t.name),
        steps: child.scenario.steps.map(mapStep),
      });
    }
  }

  return {
    feature: feature.name,
    background,
    scenarios,
  };
}

/**
 * Map a Gherkin step AST node to our simplified step format.
 */
function mapStep(step: Messages.Step): Step {
  const result: Step = {
    keyword: step.keyword.trim(),
    text: step.text,
  };

  if (step.dataTable) {
    result.dataTable = step.dataTable.rows.map((row) =>
      row.cells.map((cell) => cell.value),
    );
  }

  if (step.docString) {
    result.docString = step.docString.content;
  }

  return result;
}
