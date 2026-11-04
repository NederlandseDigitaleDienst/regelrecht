/**
 * One operation node of the law format's machine_readable logic, with only the
 * fields this module reads. Operands (`subject`, `value`, `when`, ...) are a
 * literal, a `$variable` reference or a nested operation, and the YAML is not
 * schema-checked here, so they stay `unknown` until narrowed.
 */
export interface OperationNode {
  operation: string;
  title?: string;
  values?: unknown[];
  conditions?: unknown[];
  cases?: { when?: unknown; then?: unknown }[];
  subject?: unknown;
  value?: unknown;
  when?: unknown;
  then?: unknown;
  else?: unknown;
  default?: unknown;
  from?: unknown;
  to?: unknown;
  date?: unknown;
  in?: string;
  date_of_birth?: unknown;
  reference_date?: unknown;
  collection?: unknown;
  body?: unknown;
  filter?: unknown;
  as?: string;
  combine?: string;
}

/** An execution action, or a bare operation node, as buildOperationTree accepts it. */
type ActionLike = Partial<OperationNode> & { output?: string };

/** One row of the flattened operation tree. */
export interface OperationTreeRow {
  number: string;
  title: string;
  subtitle: string | undefined;
  operation: string;
  values: unknown[];
  node: OperationNode;
}

/** A variable an operand can refer to. */
export interface AvailableVariable {
  name: string;
  ref: string;
  category: string;
}

/** The parts of an article that collectAvailableVariables reads. */
interface ArticleLike {
  machine_readable?: {
    definitions?: Record<string, unknown> | null;
    execution?: {
      input?: { name: string }[] | null;
      parameters?: { name: string }[] | null;
      actions?: { output?: string }[] | null;
    } | null;
  } | null;
}

export const OPERATION_LABELS: Record<string, string> = {
  // Rekenkundig
  ADD: 'optellen',
  SUBTRACT: 'aftrekken',
  MULTIPLY: 'vermenigvuldigen',
  DIVIDE: 'delen',
  MIN: 'minimum',
  MAX: 'maximum',
  // Vergelijking
  EQUALS: 'gelijk aan',
  GREATER_THAN: 'groter dan',
  GREATER_THAN_OR_EQUAL: 'groter dan of gelijk',
  LESS_THAN: 'kleiner dan',
  LESS_THAN_OR_EQUAL: 'kleiner dan of gelijk',
  IN: 'in lijst',
  // Logisch
  AND: 'en',
  OR: 'of',
  NOT: 'niet',
  // Conditioneel
  IF: 'als/dan',
  SWITCH: 'keuze',
  // Datum
  AGE: 'leeftijd',
  DATE_ADD: 'datum optellen',
  DATE: 'datum',
  DAY_OF_WEEK: 'dag van de week',
  DATE_DIFF: 'datumverschil',
  DATE_PART: 'datumonderdeel',
  START_OF: 'begin van',
  // Verzameling
  LIST: 'lijst',
  FOREACH: 'voor elk',
  FOLD: 'afspelen',
};

// DATE_DIFF units are English in the YAML (schema enum); subtitles read in Dutch.
const DATE_DIFF_UNIT_LABELS: Record<string, string> = { days: 'dagen', months: 'maanden', years: 'jaren' };
// DATE_PART and START_OF select a component instead of counting units, so their
// `in` is singular where DATE_DIFF's is plural.
const DATE_PART_LABELS: Record<string, string> = { year: 'jaar', month: 'maand', day: 'dagnummer' };
const START_OF_LABELS: Record<string, string> = { year: 'het jaar', month: 'de maand' };

export function collectAvailableVariables(article: ArticleLike | null | undefined): AvailableVariable[] {
  // Engine built-in context variables - always available regardless of
  // article content. `referencedate` is the calculation date injected by
  // the engine from the scenario's "Given the calculation date is ..."
  const vars: AvailableVariable[] = [
    { name: 'referencedate', ref: '$referencedate', category: 'Context' },
  ];

  if (!article?.machine_readable) return vars;
  const mr = article.machine_readable;

  if (mr.definitions) {
    for (const name of Object.keys(mr.definitions)) {
      vars.push({ name, ref: `$${name}`, category: 'Definitie' });
    }
  }

  const execution = mr.execution;
  if (!execution) return vars;

  if (execution.input) {
    for (const input of execution.input) {
      vars.push({ name: input.name, ref: `$${input.name}`, category: 'Input' });
    }
  }

  if (execution.parameters) {
    for (const param of execution.parameters) {
      vars.push({ name: param.name, ref: `$${param.name}`, category: 'Parameter' });
    }
  }

  if (execution.actions) {
    for (const action of execution.actions) {
      if (action.output) {
        vars.push({ name: action.output, ref: `$${action.output}`, category: 'Actie' });
      }
    }
  }

  return vars;
}

export function buildOperationTree(action: ActionLike | null | undefined): OperationTreeRow[] {
  if (!action) return [];

  let rootNode: OperationNode;
  if (action.operation) {
    // A truthy `operation` is what makes a node an operation (isOperationNode).
    rootNode = action as OperationNode;
  } else if (isOperationNode(action.value)) { // same check as before: object with a truthy `operation`
    rootNode = action.value;
  } else {
    return [];
  }

  // Onderwerp/Waarde rows keep YAML form verbatim - identifiers must match
  // their `$VAR` references. But the Titel row is a human-readable LABEL,
  // not a snippet of YAML; if the user later adds a `title:` field on the
  // operation, it'd never have `$` or underscores, and a derived title
  // shouldn't either. So derivedTitle humanises (strips `$`, swaps `_` for
  // spaces, lowercases all-caps idents).
  //
  // Hierarchy:
  //   - User-supplied `node.title`  → title row
  //   - Else derivedTitle(node)     → title row (and no subtitle, since the
  //                                    derived expression IS the title)
  //   - When user-title exists      → derivedTitle as supporting text below,
  //                                    so the technical detail is still
  //                                    visible underneath the user's label
  const rootTitle = action.title ?? action.output ?? 'operatie';
  const tree: OperationTreeRow[] = [];

  function traverse(node: OperationNode, prefix: string, isRoot: boolean) {
    const derived = derivedTitle(node);
    const userTitle = node.title;
    const title = isRoot ? rootTitle : (userTitle ?? derived);
    // Show derived as subtitle whenever it adds info beyond the title:
    // - root: always (rootTitle = output name, derived = expression)
    // - non-root: only when user supplied a title that masks the derived form
    const subtitle = isRoot
      ? derived
      : (userTitle ? derived : undefined);

    tree.push({
      number: prefix,
      title,
      subtitle,
      operation: node.operation,
      values: node.values || [],
      node,
    });

    let childIndex = 1;
    for (const child of getChildOperations(node)) {
      traverse(child, `${prefix}.${childIndex}`, false);
      childIndex++;
    }
  }

  traverse(rootNode, '1', true);
  return tree;
}

function getChildOperations(node: OperationNode): OperationNode[] {
  const children: OperationNode[] = [];

  if (Array.isArray(node.values)) {
    for (const v of node.values) {
      if (isOperationNode(v)) children.push(v);
    }
  }

  if (Array.isArray(node.conditions)) {
    for (const c of node.conditions) {
      if (isOperationNode(c)) children.push(c);
    }
  }

  if (isOperationNode(node.when)) children.push(node.when);
  if (isOperationNode(node.value)) children.push(node.value);
  if (isOperationNode(node.then)) children.push(node.then);
  if (isOperationNode(node.else)) children.push(node.else);

  // DATE_DIFF holds its operands in from/to (the `in` unit is never an operation)
  if (isOperationNode(node.from)) children.push(node.from);
  if (isOperationNode(node.to)) children.push(node.to);

  // The date operations (DATE_ADD, DAY_OF_WEEK, DATE_PART, START_OF) hold their
  // operand in `date`, and the canonical "eerste dag van de volgende maand"
  // nests a START_OF right there. Without this the nested operation is invisible
  // in the tree.
  if (isOperationNode(node.date)) children.push(node.date);
  // FOREACH holds its operands in collection/body/filter (`as` is a name and
  // `combine` an enum, neither is ever an operation)
  if (isOperationNode(node.collection)) children.push(node.collection);
  if (isOperationNode(node.body)) children.push(node.body);
  if (isOperationNode(node.filter)) children.push(node.filter);
  // FOLD also holds one in `initial` (RFC-056); `as` and `state` are names.
  if (isOperationNode(node.initial)) children.push(node.initial);

  if (Array.isArray(node.cases)) {
    for (const c of node.cases) {
      if (isOperationNode(c.when)) children.push(c.when);
      if (isOperationNode(c.then)) children.push(c.then);
    }
  }
  if (isOperationNode(node.default)) children.push(node.default);

  return children;
}

function isOperationNode(v: unknown): v is OperationNode {
  // Boolean() because a type guard returns a boolean; every caller only tests
  // the result for truthiness, as it did with the raw `operation` value.
  return v != null && typeof v === 'object' && Boolean((v as { operation?: unknown }).operation);
}

// Subtitle generation - produces a compact, infix-style description of what
// an operation node does. Examples:
//   EQUALS  → "$x = 0"
//   AND (3) → "$a en $b en $c"
//   AND (5) → "5 condities"
//   ADD (4) → "som van 4 waarden"
//   IF      → "1 conditie + standaard"
// Threshold of 3: ≤3 operands renders as full infix, >3 collapses to a count
// summary so a long arithmetic chain doesn't blow out the row.

const COMPARISON_SYMBOLS: Record<string, string> = {
  EQUALS: '=',
  NOT_EQUALS: '≠',
  GREATER_THAN: '>',
  LESS_THAN: '<',
  GREATER_THAN_OR_EQUAL: '≥',
  LESS_THAN_OR_EQUAL: '≤',
};
const ARITHMETIC_SYMBOLS: Record<string, string> = { ADD: '+', SUBTRACT: '−', MULTIPLY: '×', DIVIDE: '÷' };
const ARITHMETIC_NOUNS: Record<string, string> = { ADD: 'som', SUBTRACT: 'aftrekking', MULTIPLY: 'product', DIVIDE: 'deling' };
const LOGICAL_CONNECTORS: Record<string, string> = { AND: 'en', OR: 'of' };
const COMPACT_THRESHOLD = 3;

export function describeSubtitle(node: OperationNode): string {
  const op = node.operation;

  if (COMPARISON_SYMBOLS[op]) {
    return `${formatArgName(node.subject)} ${COMPARISON_SYMBOLS[op]} ${formatArgName(node.value)}`;
  }

  if (op === 'NOT_NULL') return `${formatArgName(node.subject)} is ingevuld`;

  if (op === 'IN' || op === 'NOT_IN') {
    const verb = op === 'IN' ? 'in' : 'niet in';
    // Two YAML shapes:
    //   subject + value  → list lives elsewhere, value is a ref/literal
    //   subject + values → inline list as array
    if (node.value !== undefined) {
      return `${formatArgName(node.subject)} ${verb} ${formatArgName(node.value)}`;
    }
    const items = Array.isArray(node.values) ? node.values : [];
    if (items.length > COMPACT_THRESHOLD) return `${formatArgName(node.subject)} ${verb} ${items.length} waarden`;
    return `${formatArgName(node.subject)} ${verb} [${items.map(formatArgName).join(', ')}]`;
  }

  if (LOGICAL_CONNECTORS[op]) {
    const conditions = Array.isArray(node.conditions) ? node.conditions : [];
    if (conditions.length > COMPACT_THRESHOLD) return `${conditions.length} condities`;
    const labels = conditions.map(nameableOperand);
    if (labels.some(l => l === null)) return `${conditions.length} condities`;
    return labels.join(` ${LOGICAL_CONNECTORS[op]} `);
  }

  if (op === 'NOT') return `niet ${formatArgName(node.value)}`;

  if (ARITHMETIC_SYMBOLS[op]) {
    const values = Array.isArray(node.values) ? node.values : [];
    if (values.length > COMPACT_THRESHOLD) return `${ARITHMETIC_NOUNS[op]} van ${values.length} waarden`;
    const labels = values.map(nameableOperand);
    if (labels.some(l => l === null)) return `${ARITHMETIC_NOUNS[op]} van ${values.length} waarden`;
    return labels.join(` ${ARITHMETIC_SYMBOLS[op]} `);
  }

  if (op === 'MIN' || op === 'MAX') {
    const fn = op === 'MIN' ? 'min' : 'max';
    const values = Array.isArray(node.values) ? node.values : [];
    if (values.length > COMPACT_THRESHOLD) return `${fn} van ${values.length} waarden`;
    const labels = values.map(nameableOperand);
    if (labels.some(l => l === null)) return `${fn} van ${values.length} waarden`;
    return `${fn}(${labels.join(', ')})`;
  }

  if (op === 'CONCAT') {
    const values = Array.isArray(node.values) ? node.values : [];
    if (values.length > COMPACT_THRESHOLD) return `samenvoegen van ${values.length} waarden`;
    const labels = values.map(nameableOperand);
    if (labels.some(l => l === null)) return `samenvoegen van ${values.length} waarden`;
    return labels.join(' + ');
  }

  // IF/SWITCH have no single subject and multiple branches - any one-line
  // summary leaves out important parts of the operation. The structural
  // count ("1 conditie + standaard") was technically accurate but confusing
  // because it doesn't tell the reader what the operation actually returns.
  // Better to be honest: just label it as the operation type and let the
  // user add a `node.title` when they want a meaningful name.
  if (op === 'IF' || op === 'SWITCH') {
    return `${OPERATION_LABELS[op]} operatie`;
  }

  if (op === 'AGE') {
    return `leeftijd ${formatArgName(node.date_of_birth)} op ${formatArgName(node.reference_date)}`;
  }

  // The operand of DATE_ADD and DAY_OF_WEEK is `date`; `subject`/`value` are
  // kept as a fallback for hand-written nodes that predate that field name.
  if (op === 'DATE_ADD') return `${formatArgName(node.date ?? node.subject ?? node.value)} + offset`;
  if (op === 'DATE') return formatArgName(node.value ?? node.subject);
  if (op === 'DAY_OF_WEEK') return `dag van ${formatArgName(node.date ?? node.subject ?? node.value)}`;

  if (op === 'DATE_PART') {
    // `?? ''` only for the index type: '' finds no label, as undefined did.
    const part = DATE_PART_LABELS[node.in ?? ''] || formatArgName(node.in);
    return `${part} uit ${formatArgName(node.date)}`;
  }

  if (op === 'START_OF') {
    const unit = START_OF_LABELS[node.in ?? ''] || formatArgName(node.in);
    return `begin van ${unit} van ${formatArgName(node.date)}`;
  }

  if (op === 'DATE_DIFF') {
    const unit = DATE_DIFF_UNIT_LABELS[node.in ?? ''] || formatArgName(node.in);
    return `${formatArgName(node.from)} tot ${formatArgName(node.to)} in ${unit}`;
  }

  if (op === 'LIST') {
    const values = Array.isArray(node.values) ? node.values : [];
    if (values.length > COMPACT_THRESHOLD) return `lijst van ${values.length} items`;
    return `[${values.map(formatArgName).join(', ')}]`;
  }

  // FOREACH: name what it iterates and how it reduces. The body is usually a
  // nested operation and shows up as a child node, so repeating it here would
  // say the same thing twice.
  if (op === 'FOREACH') {
    const binding = node.as || 'item';
    const over = `voor elke ${binding} in ${formatArgName(node.collection)}`;
    const combine = node.combine ? OPERATION_LABELS[node.combine] || node.combine : null;
    const filtered = node.filter ? ', gefilterd' : '';
    return combine ? `${over}${filtered}, ${combine}` : `${over}${filtered}`;
  }

  return OPERATION_LABELS[op] || op;
}

function formatArgName(v: unknown): string {
  if (v === null || v === undefined || v === '') return '(leeg)';
  if (typeof v === 'string') return v;
  if (typeof v === 'number') return String(v);
  if (typeof v === 'boolean') return String(v);
  if (isOperationNode(v)) {
    // Prefer the first variable inside the nested op - much more informative
    // than the operation label. So `AND( IN($a, ...), IN($b, ...) )` reads
    // as `$a en $b` instead of `(in lijst) en (in lijst)`. Falls back to
    // `(label)` only when the nested op holds no variable refs at all.
    const varName = findFirstVariable(v);
    if (varName) return varName;
    const label = OPERATION_LABELS[v.operation] || v.operation;
    return `(${label})`;
  }
  return '(leeg)';
}

/**
 * Operand renderer for compositional ops (AND/OR/ADD/SUBTRACT/MIN/MAX/etc.).
 * Returns a clean label when the operand can be cleanly named, or `null`
 * when it's a nameless nested op - `null` signals the caller to fall back
 * to the count-summary form rather than emit half-information like
 * `$a − (als/dan)` where the right side carries multiple operands but only
 * the wrapper type is shown.
 *
 * Nameable means: simple value (variable / literal / empty) OR a nested op
 * with an explicit user-supplied `node.title`. The user can opt complex
 * nested operations into the infix expression by adding a title.
 */
function nameableOperand(v: unknown): string | null {
  if (v === null || v === undefined || v === '') return '(leeg)';
  if (typeof v === 'string') return v;
  if (typeof v === 'number') return String(v);
  if (typeof v === 'boolean') return String(v);
  if (isOperationNode(v)) return v.title ?? null;
  return null;
}

/**
 * Human-readable label for an operation, derived from its expression. Strips
 * `$` from variable refs and turns underscores into spaces (and lowercases
 * all-caps idents) so the result reads like prose. Used as the Titel row in
 * OperationSettings, the row text in ActionSheet's parent-ops list, and the
 * (operatie) suffix on dropdown nested options. Falls through to the user-
 * supplied `node.title` (no derivation) when present - that's handled at
 * the buildOperationTree layer.
 *
 * Per-token regex (`$IDENT`) so each variable is humanised independently -
 * a string like `$PERCENTAGE_LID_5 + $aantal_dagen` becomes
 * `percentage lid 5 + aantal dagen` instead of inheriting the all-caps state
 * from one of its tokens.
 */
export function derivedTitle(node: OperationNode): string {
  return describeSubtitle(node).replace(
    /\$([A-Za-z_][A-Za-z0-9_]*)/g,
    (_: string, name: string) => humanizeIdentifier(name),
  );
}

function humanizeIdentifier(name: string): string {
  const spaced = name.replace(/_/g, ' ');
  return /[A-Z]/.test(spaced) && spaced === spaced.toUpperCase() ? spaced.toLowerCase() : spaced;
}


function getReadableName(v: unknown): string | null {
  if (typeof v === 'string') return v;
  if (typeof v === 'number') return String(v);
  if (typeof v === 'boolean') return String(v);
  if (isOperationNode(v)) {
    const varName = findFirstVariable(v);
    if (varName) return varName;
    return OPERATION_LABELS[v.operation] || v.operation;
  }
  return null;
}

function findFirstVariable(input: unknown): string | null {
  if (!input || typeof input !== 'object') return null;
  // Any object is walked, not only an operation node; these are the fields read.
  const node = input as { subject?: unknown; values?: unknown[]; conditions?: unknown[] };
  if (typeof node.subject === 'string' && node.subject.startsWith('$')) {
    return node.subject;
  }
  if (Array.isArray(node.values)) {
    for (const v of node.values) {
      if (typeof v === 'string' && v.startsWith('$')) return v;
      if (typeof v === 'object') {
        const found = findFirstVariable(v);
        if (found) return found;
      }
    }
  }
  if (Array.isArray(node.conditions)) {
    for (const c of node.conditions) {
      const found = findFirstVariable(c);
      if (found) return found;
    }
  }
  return null;
}
