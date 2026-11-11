/**
 * The parts of a field declaration (an execution input, parameter or output in
 * the law format) that these maps read. Only the fields read here; the YAML is
 * not schema-checked, so each is optional.
 */
export interface FieldDeclaration {
  name?: string;
  type?: string;
  type_spec?: { unit?: string | null } | null;
  // Only `true` counts; the YAML is unchecked, so anything else may be here.
  nullable?: unknown;
  source?: unknown;
}

/** The parts of an article (as useOpenLaw() returns it) that these maps read. */
interface ArticleLike {
  number?: string | number;
  machine_readable?: {
    execution?: {
      input?: readonly FieldDeclaration[] | null;
      parameters?: readonly FieldDeclaration[] | null;
      output?: readonly FieldDeclaration[] | null;
    } | null;
  } | null;
}

/** What the form knows about a field: its datatype, amount unit and nullability. */
export interface FieldMeta {
  type: string;
  unit: string | null;
  nullable: boolean;
}

/** What a result view knows about an output: its datatype and amount unit. */
export interface OutputMeta {
  type: string | null;
  unit: string | null;
}

/**
 * Builds a mapping from output/input/parameter names to article numbers.
 *
 * @param {Array} articles - Articles array from useOpenLaw()
 * @returns {{ outputToArticle: Map<string, string>, inputToArticle: Map<string, string>, paramToArticle: Map<string, string> }}
 */
export function buildArticleMap(articles: readonly ArticleLike[] | null | undefined) {
  const outputToArticle = new Map<string | undefined, string>();
  const inputToArticle = new Map<string | undefined, string>();
  const paramToArticle = new Map<string | undefined, string>();

  for (const article of articles || []) {
    const exec = article.machine_readable?.execution;
    if (!exec) continue;
    const num = String(article.number);
    if (Array.isArray(exec.output)) {
      for (const o of exec.output) outputToArticle.set(o.name, num);
    }
    if (Array.isArray(exec.input)) {
      for (const i of exec.input) inputToArticle.set(i.name, num);
    }
    if (Array.isArray(exec.parameters)) {
      for (const p of exec.parameters) paramToArticle.set(p.name, num);
    }
  }

  return { outputToArticle, inputToArticle, paramToArticle };
}

/**
 * The form-relevant part of a field declaration (parameter or input): its
 * datatype, the unit of an amount, and whether `null` is one of its values
 * (`nullable`, schema v0.5.8, RFC-036). `nullable` is a boolean with schema
 * default false, so a declaration without the key is a field that is never
 * absent; the form then offers a value or a blank cell, not `null`.
 */
function fieldMeta(field: FieldDeclaration & { type: string }): FieldMeta {
  return {
    type: field.type,
    unit: field.type_spec?.unit ?? null,
    nullable: field.nullable === true,
  };
}

/**
 * Builds a name -> datatype map for scenario parameter inputs, so each input
 * can render the control matching its declared type (boolean -> switch,
 * amount -> currency field, etc.). Merges execution.input and
 * execution.parameters. Captures `type_spec.unit` so the amount branch can
 * convert eurocents<->euros, and `nullable` so the form only accepts a
 * stated absence where the law allows one.
 *
 * Precedence on a name collision: the parameter's declaration wins, for the
 * whole of the meta (type, unit and nullable alike). This map serves the
 * parameter rows of the scenario form (`Given parameter "x" is ...`), and
 * the engine checks a top-level parameter against the *parameter's*
 * declaration (`required_parameter_for_nobody` in service.rs): a `null` is
 * accepted or refused by the parameter's `nullable`, whatever a same-named
 * input says. So a nullable input shadowed by a non-nullable parameter has
 * `null` refused here, on purpose, because the engine refuses it too. The
 * columns of a data-source table are the other way round: they feed inputs,
 * and `buildExternalFieldTypeMap` reads inputs only, untouched by any
 * same-named parameter.
 *
 * @param {Array} articles - Articles array from useOpenLaw()
 * @returns {Map<string, { type: string, unit: (string|null), nullable: boolean }>}
 */
export function buildTypeMap(articles: readonly ArticleLike[] | null | undefined): Map<string, FieldMeta> {
  const typeMap = new Map<string, FieldMeta>();
  const add = (field: FieldDeclaration | null | undefined) => {
    if (field?.name && field.type) {
      typeMap.set(field.name, fieldMeta({ ...field, type: field.type }));
    }
  };

  // Two passes across all articles: collect every input first, then let
  // parameters override on name collision (a scenario `Given parameter`
  // targets an execution parameter most directly). A single per-article pass
  // would let a later article's input clobber an earlier article's parameter.
  for (const article of articles || []) {
    for (const i of article.machine_readable?.execution?.input || []) add(i);
  }
  for (const article of articles || []) {
    for (const p of article.machine_readable?.execution?.parameters || []) add(p);
  }

  return typeMap;
}

/**
 * Builds an outputName -> { type, unit } map from `execution.output`, so result
 * views can format a value by its declared unit instead of guessing from the
 * name. First declaration wins on a name collision, matching the backend's
 * `collect_law_outputs` dedup.
 *
 * @param {Array} articles - Articles array from useOpenLaw()
 * @returns {Map<string, { type: (string|null), unit: (string|null) }>}
 */
export function buildOutputTypeMap(articles: readonly ArticleLike[] | null | undefined): Map<string, OutputMeta> {
  const map = new Map<string, OutputMeta>();
  for (const article of articles || []) {
    for (const o of article.machine_readable?.execution?.output || []) {
      if (o?.name && !map.has(o.name)) {
        map.set(o.name, { type: o.type ?? null, unit: o.type_spec?.unit ?? null });
      }
    }
  }
  return map;
}

/**
 * Builds a fieldName -> { type, unit } map for EXTERNAL data-source fields -
 * inputs whose `source` is empty (no `regulation` and no `output`), i.e. raw
 * data the engine resolves by field name (e.g. insurance.verdragsinschrijving).
 * Spans a set of law docs (the current law + its loaded dependencies), because
 * data-source fields are declared by the leaf laws, not the law under test.
 * Last doc wins on a name collision. Drives typed cells in DataSourceTable.
 * Parameters play no part: a table column feeds an input, and the engine
 * checks the value against the input's declaration, so a same-named
 * parameter (nullable or not) leaves the column's meta alone. The parameter
 * rows of the form have the opposite rule, see `buildTypeMap`.
 *
 * @param {Array<{articles?: Array}>} lawDocs - parsed law documents
 * @returns {Map<string, { type: string, unit: (string|null), nullable: boolean }>}
 */
export function buildExternalFieldTypeMap(
  lawDocs: readonly ({ articles?: readonly ArticleLike[] | null } | null | undefined)[] | null | undefined,
): Map<string, FieldMeta> {
  const map = new Map<string, FieldMeta>();
  for (const doc of lawDocs || []) {
    for (const article of doc?.articles || []) {
      for (const f of article.machine_readable?.execution?.input || []) {
        const src = f.source;
        // External data-source fields are declared with an empty `source: {}`
        // (a `regulation` means cross-law, an `output` means internal). Match
        // exactly that - an empty object - rather than merely "no regulation and
        // no output", so a malformed source carrying other keys isn't
        // misclassified as external. (versionsCache YAML is not schema-checked.)
        const isExternal = src && typeof src === 'object' && Object.keys(src).length === 0;
        if (isExternal && f.name && f.type) {
          map.set(f.name, fieldMeta({ ...f, type: f.type }));
        }
      }
    }
  }
  return map;
}
