/*
 * The schema reference model: the real law schema, turned into something a
 * page can render.
 *
 * Single source of truth for what /reference/schema shows. It reads
 * src/data/schema-latest.json — the committed snapshot of schema/latest,
 * written by scripts/sync-schema.mjs, because `schema/` is not in the docs
 * Docker build context (COPY docs/ .) and cannot be imported across the repo
 * root. check-schema-version.mjs fails CI when the snapshot drifts, so what
 * this module reports is always the released schema.
 *
 * The shape of the job, and why it is not a generic JSON-Schema renderer:
 * a generic one prints the machinery (allOf/if/then, oneOf branches, $ref
 * URIs) and leaves the reader to reconstruct the meaning. The schema's real
 * content is that a WET needs a bwb_id, that there are fourteen operations,
 * that `source` is how one law calls another. So this module resolves refs,
 * turns the six conditional blocks into readable rules, and hands each field
 * the docs page that explains it.
 */

import schemaJson from '~/data/schema-latest.json';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/** A JSON Schema node, as much of draft-07 as this schema actually uses. */
export interface SchemaNode {
  type?: string | string[];
  description?: string;
  examples?: unknown[];
  enum?: string[];
  const?: string;
  default?: unknown;
  pattern?: string;
  format?: string;
  minimum?: number;
  maximum?: number;
  required?: string[];
  properties?: Record<string, SchemaNode>;
  items?: SchemaNode;
  oneOf?: SchemaNode[];
  allOf?: SchemaNode[];
  anyOf?: SchemaNode[];
  not?: SchemaNode;
  /** Conditional pair, used only by the top-level identifier rules. */
  if?: SchemaNode;
  then?: SchemaNode;
  additionalProperties?: boolean | SchemaNode;
  $ref?: string;
  $id?: string;
  title?: string;
  definitions?: Record<string, SchemaNode>;
}

/** One row in a field table. */
export interface Field {
  name: string;
  /** Rendered type, e.g. "string", "array of operation", "object". */
  type: string;
  required: boolean;
  /** Description with RFC references already linked; may contain HTML. */
  description: string;
  enum?: string[];
  default?: unknown;
  pattern?: string;
  /** Anchor of the definition this field points at, when it refs one. */
  refAnchor?: string;
  refName?: string;
  /** Where the prose docs explain this field. */
  concept?: ConceptLink;
  /** YAML examples, pre-rendered. */
  examples: string[];
  /** Build-time search haystack. */
  zoek: string;
  /**
   * Carried from the common fields rather than added by this kind. Lets a
   * section list everything a kind accepts while still showing which part is
   * particular to it.
   */
  inherited?: boolean;
}

export interface ConceptLink {
  href: string;
  label: string;
}

/** One of the fourteen operations. */
export interface Operation {
  /** Definition name, e.g. "arithmeticOperation". */
  defName: string;
  /** Heading + anchor slug, e.g. "arithmetic". */
  slug: string;
  /** Display title, e.g. "Arithmetic". */
  title: string;
  /** The `operation:` values this form accepts. */
  keywords: string[];
  description: string;
  fields: Field[];
  examples: string[];
  concept?: ConceptLink;
  zoek: string;
}

/** A conditional identifier rule, from the top-level allOf. */
export interface LayerRule {
  layers: string[];
  requires: string[];
  /** Keys an article may not carry under this layer (only `REDUCTIE`). */
  forbids: string[];
}

/** A documented enum, rendered as its own table. */
export interface EnumBlock {
  slug: string;
  title: string;
  path: string;
  description: string;
  values: string[];
  glossary?: ConceptLink;
  zoek: string;
}

// ---------------------------------------------------------------------------
// The schema
// ---------------------------------------------------------------------------

const schema = schemaJson as unknown as SchemaNode;
const defs = schema.definitions ?? {};

/** Version from the $id URL, e.g. "v0.6.0" — never hand-stated. */
export function schemaVersion(): string {
  const m = (schema.$id ?? '').match(/schema\/(v\d+\.\d+\.\d+)\/schema\.json/);
  if (!m) throw new Error('schema $id does not carry a version: ' + schema.$id);
  return m[1];
}

/** The immutable tag URL law files put in their `$schema` key. */
export function schemaUrl(): string {
  if (!schema.$id) throw new Error('schema has no $id');
  return schema.$id;
}

// ---------------------------------------------------------------------------
// Cross-references
//
// Which docs page explains a field. Hand-maintained because the mapping is a
// judgement — the schema cannot know that `source` is the subject of a whole
// concept page. Every anchor here is verified by scripts/check-links.mjs
// against the built HTML, so a renamed heading fails the build rather than
// rotting into a dead link.
// ---------------------------------------------------------------------------

const CONCEPTS: Record<string, ConceptLink> = {
  source: { href: '/concepts/cross-law-references#how-it-works', label: 'Cross-Law References' },
  open_terms: {
    href: '/concepts/inversion-of-control#the-law-declares-an-open-term',
    label: 'Inversion of Control',
  },
  implements: {
    href: '/concepts/inversion-of-control#the-implementing-regulation-fills-it',
    label: 'Inversion of Control',
  },
  hooks: { href: '/concepts/hooks-and-reactive-execution#how-hooks-work', label: 'Hooks' },
  overrides: {
    href: '/concepts/hooks-and-reactive-execution#overrides-lex-specialis',
    label: 'Overrides (lex specialis)',
  },
  produces: {
    href: '/concepts/hooks-and-reactive-execution#the-produces-annotation',
    label: 'The produces annotation',
  },
  procedure: {
    href: '/concepts/hooks-and-reactive-execution#administrative-procedure-stages',
    label: 'Administrative procedure stages',
  },
  competent_authority: {
    href: '/concepts/competent-authority#the-two-forms',
    label: 'Competent Authority',
  },
  markings: {
    href: '/concepts/markings#how-they-are-flagged',
    label: 'Markings',
  },
  // Schema v0.5.x called this channel `untranslatables`. Kept so a law file on
  // an older schema version still links to the concept page.
  untranslatables: {
    href: '/concepts/markings#a-note-on-the-name',
    label: 'Markings (formerly untranslatables)',
  },
  valid_to: {
    href: '/concepts/temporal-and-dates#which-version-is-in-force',
    label: 'Temporal Validity',
  },
  valid_from: {
    href: '/concepts/temporal-and-dates#which-version-is-in-force',
    label: 'Temporal Validity',
  },
  type_spec: { href: '/concepts/values#amounts-and-units', label: 'Amounts and Units' },
  nullable: {
    href: '/concepts/values#absent-and-unknown-values',
    label: 'Absent and Unknown Values',
  },
  definitions: { href: '/concepts/law-format#definitions', label: 'Definitions' },
  actions: { href: '/concepts/law-format#operations', label: 'Operations' },
  articles: { href: '/concepts/law-format#articles', label: 'Law Format' },
};

/** Operation definition -> the concept page covering it. */
const OPERATION_CONCEPTS: Record<string, ConceptLink> = {
  foreachOperation: { href: '/concepts/collections#iterating', label: 'Collections' },
  listOperation: { href: '/concepts/collections#counting', label: 'Collections' },
  inOperation: { href: '/concepts/collections#counting', label: 'Collections' },
  roundingOperation: {
    href: '/concepts/values#rounding',
    label: 'Rounding',
  },
  ageOperation: {
    href: '/concepts/temporal-and-dates#comparing-and-subtracting-dates',
    label: 'Dates',
  },
  dateAddOperation: {
    href: '/concepts/temporal-and-dates#comparing-and-subtracting-dates',
    label: 'Dates',
  },
  dateDiffOperation: {
    href: '/concepts/temporal-and-dates#comparing-and-subtracting-dates',
    label: 'Dates',
  },
  dateConstructOperation: {
    href: '/concepts/temporal-and-dates#comparing-and-subtracting-dates',
    label: 'Dates',
  },
  dayOfWeekOperation: {
    href: '/concepts/temporal-and-dates#comparing-and-subtracting-dates',
    label: 'Dates',
  },
};

/**
 * Enum -> glossary section. The glossary has no per-term anchors (its terms
 * live in table cells), so these point at the section heading that defines
 * the vocabulary.
 */
const GLOSSARY: Record<string, ConceptLink> = {
  regulatory_layer: {
    href: '/reference/glossary#legal-hierarchy',
    label: 'Glossary: Legal Hierarchy',
  },
  legal_character: {
    href: '/reference/glossary#legal-character',
    label: 'Glossary: Legal Character',
  },
  decision_type: {
    href: '/reference/glossary#decision-type',
    label: 'Glossary: Decision Type',
  },
};

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

const HTML_ESCAPES: Record<string, string> = {
  '&': '&amp;',
  '<': '&lt;',
  '>': '&gt;',
  '"': '&quot;',
};

function escapeHtml(s: string): string {
  return s.replace(/[&<>"]/g, (c) => HTML_ESCAPES[c]);
}

/**
 * Link RFC references and `backtick code` inside a description.
 *
 * The descriptions already name their RFCs in prose ("(RFC-016)", "See
 * RFC-019."), so the reference is there; only the link is missing. The
 * site-wide rehype plugin that does this (lib/rehype-rfc-links.ts) runs on
 * RFC pages only — it returns early when the source file is not under
 * content/rfcs — and widening it would change all 52 docs pages. So the
 * linking happens here, for this page alone.
 *
 * Escaping runs first, so a description can never inject markup.
 */
export function renderDescription(text: string): string {
  let out = escapeHtml(text);
  out = out.replace(
    /\bRFC-(\d{3})\b/g,
    (_m, num) => `<a href="/rfcs/rfc-${num}">RFC-${num}</a>`,
  );
  out = out.replace(/`([^`]+)`/g, (_m, code) => `<code>${code}</code>`);
  return out;
}

/** Follow a local $ref. Returns null for anything that is not `#/definitions/x`. */
function resolveRef(ref: string): { name: string; node: SchemaNode } | null {
  const m = ref.match(/^#\/definitions\/(.+)$/);
  if (!m) return null;
  const node = defs[m[1]];
  return node ? { name: m[1], node } : null;
}

/**
 * A human type label for a node.
 *
 * Refs render as the definition's name rather than being inlined: inlining
 * `operationValue` would recurse forever (it points back at `operation`), and
 * a named link is what a reader wants anyway.
 */
function typeLabel(node: SchemaNode): string {
  if (node.$ref) {
    const target = resolveRef(node.$ref);
    return target ? defTitle(target.name) : 'object';
  }
  if (node.const) return `"${node.const}"`;
  if (node.enum) return 'string';
  if (node.oneOf) {
    const parts = node.oneOf.map(typeLabel);
    return [...new Set(parts)].join(' or ');
  }
  if (node.allOf) {
    const named = node.allOf.find((s) => s.$ref);
    if (named) return typeLabel(named);
    return 'object';
  }
  if (node.type === 'array') {
    const inner = node.items ? typeLabel(node.items) : 'value';
    return `array of ${inner}`;
  }
  if (Array.isArray(node.type)) return node.type.join(' or ');
  return node.type ?? 'any';
}

/** "arithmeticOperation" -> "Arithmetic operation" for prose. */
function defTitle(name: string): string {
  const spaced = name.replace(/([A-Z])/g, ' $1').toLowerCase().trim();
  return spaced.charAt(0).toUpperCase() + spaced.slice(1);
}

/**
 * Anchor for a definition, matching the heading the page renders.
 *
 * Most definitions get a heading named after them. Three do not, because the
 * page introduces them under a name a reader would look for instead: the
 * article payload is "The machine_readable section", and the operation keyword
 * list is the Operations section itself. Mapping them here keeps every
 * generated link pointing at a heading that exists — check-links.mjs verifies
 * that against the built HTML, so a wrong entry fails the build.
 */
const ANCHOR_OVERRIDES: Record<string, string> = {
  machineReadableSection: 'machine-readable',
  operationType: 'operations',
  operation: 'operations',
};

function defAnchor(name: string): string {
  return (
    ANCHOR_OVERRIDES[name] ?? name.replace(/([A-Z])/g, '-$1').toLowerCase()
  );
}

/**
 * Render a JSON value as the YAML a law file would carry.
 *
 * Small on purpose: the values in `examples` are law snippets (scalars, maps,
 * lists), never the full generality of YAML. Strings stay unquoted unless they
 * need quoting, because `$bedrag` and `WET` read better bare and that is how
 * the corpus writes them.
 */
export function toYaml(value: unknown): string {
  return renderBlock(value).join('\n');
}

/** Indent every line of an already-rendered block by one level. */
function indentBlock(lines: string[]): string[] {
  return lines.map((line) => '  ' + line);
}

/**
 * Render as a list of unindented lines; the caller adds the indentation.
 *
 * Indentation is applied in exactly one place (indentBlock, at the point of
 * nesting) rather than being passed down and re-applied on the way out. The
 * earlier version did both — it took an `indent` argument *and* re-indented
 * the returned string under a `- ` — which double-indented every object
 * inside a list and produced YAML that would not parse.
 */
function renderBlock(value: unknown): string[] {
  if (isScalar(value)) return [renderScalar(value)];

  if (Array.isArray(value)) {
    if (value.length === 0) return ['[]'];
    return value.flatMap((item) => {
      if (isScalar(item)) return [`- ${renderScalar(item)}`];
      const body = renderBlock(item);
      // The first line rides on the dash; the rest line up under it.
      return [`- ${body[0]}`, ...indentBlock(body.slice(1))];
    });
  }

  const entries = Object.entries(value as Record<string, unknown>);
  if (entries.length === 0) return ['{}'];
  return entries.flatMap(([key, v]) => {
    if (isScalar(v)) return [`${key}: ${renderScalar(v)}`];
    if (Array.isArray(v) && v.length === 0) return [`${key}: []`];
    if (!Array.isArray(v) && Object.keys(v as object).length === 0) {
      return [`${key}: {}`];
    }
    const body = renderBlock(v);
    // A list under a key sits at the key's own indentation, which is how the
    // corpus writes it; a map nests one level in.
    return [`${key}:`, ...(Array.isArray(v) ? body : indentBlock(body))];
  });
}

function isScalar(v: unknown): boolean {
  return v === null || typeof v !== 'object';
}

function renderScalar(value: unknown): string {
  if (value === null) return 'null';
  if (typeof value === 'boolean' || typeof value === 'number') return String(value);

  const s = String(value);
  // Quote only when bare would be ambiguous: a leading indicator character,
  // something YAML would read as another type, or stray whitespace. Dates are
  // quoted too — bare 2025-01-01 parses as a date, not the string the schema
  // holds, and law files write them quoted for that reason.
  const needsQuote =
    s === '' ||
    /^[-?:,[\]{}#&*!|>'"%@`]/.test(s) ||
    /:\s|\s#/.test(s) ||
    s !== s.trim() ||
    /^(true|false|null|yes|no|on|off|~)$/i.test(s) ||
    /^-?\d+(\.\d+)?$/.test(s) ||
    /^\d{4}-\d{2}-\d{2}/.test(s) ||
    // Sexagesimal: bare 6:7 is the number 367 in YAML 1.1, not the string.
    // Article references are written exactly like this ("Awb 6:7"), so this
    // is the case that actually shows up rather than a theoretical one.
    /^-?\d+(:\d+)+$/.test(s);
  return needsQuote ? `'${s.replace(/'/g, "''")}'` : s;
}

function renderExamples(node: SchemaNode): string[] {
  if (!node.examples?.length) return [];
  return node.examples.map((ex) => toYaml(ex));
}

// ---------------------------------------------------------------------------
// Search haystack
//
// Built here, at build time, and written into a data-zoek attribute; the
// browser only compares strings. Same split as the roadmap filter, and for the
// same reason: the normaliser lives in its own module (lib/schema-zoek.ts)
// because a client script cannot import this one — it reads a JSON module
// through Vite and pulls in the whole schema.
// ---------------------------------------------------------------------------

import { normaliseerZoekterm } from './schema-zoek';

function haystack(...parts: (string | string[] | undefined)[]): string {
  const flat = parts
    .flatMap((p) => (Array.isArray(p) ? p : [p]))
    .filter((p): p is string => Boolean(p));
  return normaliseerZoekterm(flat.join(' '));
}

// ---------------------------------------------------------------------------
// Field extraction
// ---------------------------------------------------------------------------

/**
 * Field names that get a section of their own, so their type can link to it.
 *
 * Derived from the schema rather than listed: every inline object the page
 * expands is a property of `machineReadableSection`, of `baseField`, or of
 * the `inputField` branch, and those are exactly the three sets
 * machineReadableSubStructures() and fieldSubStructures() walk. Listing them
 * by hand would mean a new nested section silently losing its inbound link.
 */
const INLINE_SECTIONS: Set<string> = (() => {
  const names = new Set<string>();
  const collect = (owner: SchemaNode) => {
    for (const [key, node] of Object.entries(owner.properties ?? {})) {
      const inline =
        (node.type === 'array' && node.items?.properties) ||
        (node.type === 'object' && node.properties);
      if (inline) names.add(key);
    }
  };
  collect(defs.machineReadableSection ?? {});
  collect(defs.baseField ?? {});
  const inputOwn = defs.inputField?.allOf?.find((s) => !s.$ref);
  if (inputOwn) collect(inputOwn);
  // The inline objects that get a section elsewhere on the page.
  collect(schema);
  collect(schema.properties?.articles?.items ?? {});
  collect(defs.machineReadableSection?.properties?.execution ?? {});
  collect(defs.action ?? {});
  collect(defs.machineReadableSection?.properties?.hooks?.items ?? {});
  collect(defs.ifOperation ?? {});
  // Free-form maps: any key, one value shape. There is no field list to
  // render, so these get no section and their type stays unlinked rather than
  // pointing at a heading that does not exist.
  names.delete('parameters');
  names.delete('definitions');
  collect(schema.properties?.procedure?.items ?? {});
  collect(defs.machineReadableSection?.properties?.enables?.items ?? {});
  // `defaults`/`default` hold an actions list already documented under Action,
  // and `match` is a free-form map of matching criteria.
  names.delete('defaults');
  names.delete('default');
  names.delete('match');
  return names;
})();

function toField(
  name: string,
  node: SchemaNode,
  required: string[],
  conceptKey = name,
): Field {
  // A $ref on an array sits on `items`, not on the field. Without this an
  // `actions` field renders as the bare text "array of Action" while the
  // section documenting Action is one click away and unlinked.
  const direct = node.$ref ? resolveRef(node.$ref) : null;
  const viaItems =
    !direct && node.type === 'array' && node.items?.$ref
      ? resolveRef(node.items.$ref)
      : null;
  const ref = direct ?? viaItems;

  /*
   * An inline object has no $ref to follow, so typeLabel can only call it
   * "object". Where the page gives that object its own section, link the type
   * to it: `source` said "object" in dead text with its five fields sitting
   * one screen below. Keyed by the field name, which is what every one of
   * these sections is anchored on.
   */
  const inlineAnchor = INLINE_SECTIONS.has(name) ? name.replace(/_/g, '-') : undefined;
  // A $ref sibling may carry its own description; prefer it, then the target's.
  // Only a *direct* ref lends its description: the element type's text
  // describes one element, not the array the field actually is.
  const description = node.description ?? direct?.node.description ?? '';
  const enumValues = node.enum ?? (node.const ? [node.const] : undefined);

  return {
    name,
    type: typeLabel(node),
    required: required.includes(name),
    description: renderDescription(description),
    enum: enumValues,
    default: node.default,
    pattern: node.pattern,
    refAnchor: ref ? defAnchor(ref.name) : inlineAnchor,
    refName: ref ? defTitle(ref.name) : undefined,
    concept: CONCEPTS[conceptKey],
    examples: renderExamples(node),
    zoek: haystack(name, description, enumValues, typeLabel(node)),
  };
}

function fieldsOf(node: SchemaNode): Field[] {
  const props = node.properties ?? {};
  const required = node.required ?? [];
  return Object.entries(props).map(([name, child]) => toField(name, child, required));
}

// ---------------------------------------------------------------------------
// Public model
// ---------------------------------------------------------------------------

/** Top-level fields of a law file. */
export function topLevelFields(): Field[] {
  return fieldsOf(schema).map((f) =>
    // Its own section, not the legalBasis definition: the two share a name
    // and not a shape, so linking to the definition would show a reader
    // fields this one does not have.
    f.name === 'legal_basis' ? { ...f, refAnchor: 'top-legal-basis' } : f,
  );
}

/** A nested structure that has no `definitions` entry of its own. */
export interface SubStructure {
  slug: string;
  title: string;
  /** Where it sits, e.g. "machine_readable.open_terms[]". */
  path: string;
  description: string;
  fields: Field[];
  zoek: string;
}

/**
 * Expand the fields whose shape is written inline rather than as a named
 * definition.
 *
 * `articles`, `open_terms`, `implements`, `hooks` and the rest are arrays of
 * anonymous objects. `typeLabel` can only call those "array of object", which
 * tells a reader nothing and links nowhere — and that is where most of a law
 * actually lives. So each one gets its own sub-table.
 *
 * Driven off the schema rather than a hand-written list: a new inline-object
 * field appears here on its own once the schema has it.
 */
/**
 * The two forms of `execution.produces` (schema v0.8.1): the object most
 * laws write, and the list whose items each carry a `type`, under
 * `definitions.producedItem`. Before v0.8.1 `produces` was only the object.
 */
function producesForms(): { object: SchemaNode; list: SchemaNode | null } {
  const produces: SchemaNode =
    defs.machineReadableSection?.properties?.execution?.properties?.produces ?? {};
  const object = produces.oneOf?.find((n) => n.type === 'object') ?? produces;
  const list = produces.oneOf?.find((n) => n.type === 'array') ?? null;
  return { object, list };
}

function expandInline(
  owner: SchemaNode,
  pathPrefix: string,
  only?: string[],
): SubStructure[] {
  const props = owner.properties ?? {};
  return Object.entries(props)
    .filter(([name]) => !only || only.includes(name))
    .map(([name, node]) => {
      // Either an array of inline objects, or an inline object itself.
      const shape =
        node.type === 'array' && node.items?.properties
          ? node.items
          : node.type === 'object' && node.properties
            ? node
            : null;
      if (!shape) return null;
      const isArray = node.type === 'array';
      return {
        slug: name.replace(/_/g, '-'),
        title: name,
        path: `${pathPrefix}${name}${isArray ? '[]' : ''}`,
        description: renderDescription(node.description ?? ''),
        fields: fieldsOf(shape),
        zoek: haystack(name, node.description, Object.keys(shape.properties ?? {})),
      };
    })
    .filter((s): s is SubStructure => s !== null);
}

/**
 * The nested objects a field can carry, each expanded into its own table.
 *
 * `source`, `type_spec` and `temporal` are inline objects on the field
 * definitions, so the field table can only call them "object". That hides
 * twelve documented properties, and `source` in particular is the entire
 * cross-law mechanism: which regulation, which output, which parameters.
 */
export function fieldSubStructures(): SubStructure[] {
  const base = defs.baseField ?? {};
  const input = defs.inputField?.allOf?.find((s) => !s.$ref) ?? {};
  return [
    ...expandInline(input, 'input[].', ['source']),
    ...expandInline(base, 'field.', ['type_spec', 'temporal']),
  ];
}

/**
 * Inline objects elsewhere in the schema that are worth their own table.
 *
 * Anything whose type renders as "object" but whose fields are never shown is
 * dead text: the reader is told there is structure and not what it is. These
 * are the ones with a real field list, each hung under the section it appears
 * in. Two objects are deliberately absent — `source.parameters` and
 * `machine_readable.definitions` are free-form maps (any key, one value
 * shape), so there is no field list to render and nothing to link to.
 */
export function otherSubStructures(): Record<string, SubStructure[]> {
  const mrs = defs.machineReadableSection ?? {};
  const exec = mrs.properties?.execution ?? {};
  const article = schema.properties?.articles?.items ?? {};

  return {
    'law-file': [
      ...expandInline(schema, '', ['preamble', 'procedure']),
    ],
    articles: expandInline(article, 'articles[].', ['placement', 'references']),
    procedure: expandInline(
      schema.properties?.procedure?.items ?? {},
      'procedure[].',
      ['stages'],
    ),
    enables: expandInline(
      mrs.properties?.enables?.items ?? {},
      'enables[].',
      ['for', 'interface'],
    ),
    // `produces` is one object or a list of items (schema v0.8.1): a table
    // for each form.
    execution: [
      ...expandInline(
        { properties: { produces: { ...producesForms().object, description: exec.properties?.produces?.description } } },
        'execution.',
        ['produces'],
      ),
      ...(producesForms().list
        ? expandInline(
            {
              properties: {
                produces: {
                  type: 'array',
                  items: defs.producedItem ?? {},
                  description: producesForms().list?.description,
                },
              },
            },
            'execution.',
            ['produces'],
          ).map((sub) => ({ ...sub, slug: 'produces-list', title: 'produces (list)' }))
        : []),
    ],
    action: expandInline(defs.action ?? {}, 'action.', ['resolve']),
    hooks: expandInline(
      mrs.properties?.hooks?.items ?? {},
      'hooks[].',
      ['applies_to'],
    ),
    if: expandInline(defs.ifOperation ?? {}, 'IF.', ['cases']),
  };
}

/**
 * The sub-structures belonging to one field kind, keyed by its slug.
 *
 * `source` is a property of an input, and `type_spec`/`temporal` of every
 * field. Rendered as a flat run of h3s they read as siblings of Parameter,
 * Input and Output — `source` appeared under Output, which is where it does
 * not belong. This hands each one to the section that owns it.
 */
export function subStructuresFor(kindSlug: string): SubStructure[] {
  const all = fieldSubStructures();
  if (kindSlug === 'input-field') return all.filter((s) => s.title === 'source');
  if (kindSlug === 'base-field') {
    return all.filter((s) => s.title === 'type_spec' || s.title === 'temporal');
  }
  return [];
}

/** The shape of one article: where the text and the translation live. */
export function articleFields(): Field[] {
  const items = schema.properties?.articles?.items;
  return items ? fieldsOf(items) : [];
}

/**
 * The top-level `legal_basis`, which is NOT the `legalBasis` definition.
 *
 * It carries `law_id`, `article` and `description`, where the definition used
 * on fields and actions carries `law`, `bwb_id`, `article`, `paragraph` and
 * four more. Two shapes under one name, so the page documents both rather
 * than linking one to the other and showing the wrong fields.
 */
export function topLevelLegalBasisFields(): Field[] {
  const items = schema.properties?.legal_basis?.items;
  return items ? fieldsOf(items) : [];
}

/**
 * Inline structures under `machine_readable`, each worth its own table.
 *
 * `execution` is excluded: it is an inline object too, but it is the heart of
 * the section and the page gives it a hand-written introduction of its own.
 * Letting the generic expansion also emit it would produce two `#execution`
 * headings and a duplicate anchor.
 */
export function machineReadableSubStructures(): SubStructure[] {
  return expandInline(defs.machineReadableSection ?? {}, 'machine_readable.').filter(
    (s) => s.title !== 'execution',
  );
}

/** `definitions.action`: what one step of a calculation looks like. */
export function actionFields(): Field[] {
  return fieldsOf(defs.action ?? {});
}

/**
 * The conditional layer rules, read out of the top-level `allOf`.
 *
 * In the schema these are if/then blocks keyed on `regulatory_layer`. Six
 * require an identifier. The one for `REDUCTIE` is structural: it requires no
 * identifier but forbids keys on every article's `machine_readable` (a
 * `not`/`anyOf` of `required`), and its `else` keeps `source.decisions` and
 * `source.facts` to that layer. Two more yield no row (RFC-048), both keyed on
 * `UITVOERINGSBELEID`: one has only an `else`, which keeps `source.reduction`
 * to that layer, and one nests its rules (an article that takes an answer
 * fills an open term; the policy then needs a named root
 * `competent_authority`) one level deeper. The component states those in
 * prose. Rendering them as JSON Schema would be
 * unreadable; the content is a plain table of "this kind of instrument needs
 * this identifier" or "may not carry these keys", so that is what this
 * returns.
 */
export function layerRules(): LayerRule[] {
  return (schema.allOf ?? [])
    .map((clause) => {
      const cond = clause.if?.properties?.regulatory_layer as SchemaNode | undefined;
      if (!cond) return null;
      const layers = cond.enum ?? (cond.const ? [cond.const] : []);
      if (!layers.length) return null;
      const requires = clause.then?.required ?? [];
      const forbids = forbiddenKeys(clause.then);
      if (!requires.length && !forbids.length) return null;
      return { layers, requires, forbids };
    })
    .filter((r): r is LayerRule => r !== null);
}

/**
 * The keys a `then` forbids on an article's `machine_readable`, from its
 * `not: { anyOf: [{ required: [...] }] }`. A branch that requires a key of a
 * nested object (`execution` with `produces`) reads as the dotted path.
 */
function forbiddenKeys(then: SchemaNode | undefined): string[] {
  const mr = then?.properties?.articles?.items?.properties?.machine_readable;
  return (mr?.not?.anyOf ?? []).flatMap((branch) =>
    (branch.required ?? []).flatMap((key) => {
      const nested = branch.properties?.[key]?.required;
      return nested?.length ? nested.map((n) => `${key}.${n}`) : [key];
    }),
  );
}

/** The `machine_readable` block: its own fields, and `execution`'s. */
export function machineReadableFields(): Field[] {
  return fieldsOf(defs.machineReadableSection ?? {});
}

export function executionFields(): Field[] {
  const exec = defs.machineReadableSection?.properties?.execution;
  return exec ? fieldsOf(exec) : [];
}

/**
 * The field definitions: the shared base plus the three specialisations.
 *
 * parameterField/inputField/outputField are each `allOf: [baseField, {...}]`,
 * so their own table shows only what they add on top of the base.
 */
export function fieldKinds(): { name: string; slug: string; description: string; fields: Field[] }[] {
  const out: { name: string; slug: string; description: string; fields: Field[] }[] = [];

  const base = defs.baseField;
  if (base) {
    out.push({
      name: 'Common to every field',
      slug: 'base-field',
      description:
        'Every parameter, input and output carries these. The three kinds below repeat them, so each section is the complete list for that kind.',
      fields: fieldsOf(base),
    });
  }

  for (const key of ['parameterField', 'inputField', 'outputField'] as const) {
    const node = defs[key];
    if (!node?.allOf) continue;
    /*
     * The branch without a $ref holds what this kind adds to baseField. The
     * section shows the base fields *and* the additions, so it is the whole
     * list for that kind rather than a delta the reader has to apply by
     * scrolling back. `outputField` adds nothing, and without this its
     * section was a heading over a sentence explaining an absent table.
     */
    const own = node.allOf.find((s) => !s.$ref);
    const added = own ? fieldsOf(own) : [];
    const inherited = base ? fieldsOf(base).map((f) => ({ ...f, inherited: true })) : [];
    const fields = [...inherited, ...added];
    const label = key.replace('Field', '');
    // A kind that adds nothing still gets a section — leaving it out would
    // read as though `output` did not exist — and says what it *is* rather
    // than apologising for an absent table. Keyed off the field count, so it
    // stays right if a later schema version adds a field here.
    const KIND_INTRO: Record<string, string> = {
      parameter:
        'A value the caller passes in, rather than one the law looks up or '
        + 'derives. Adds `required`, which defaults to true; an omitted '
        + '`required: false` parameter resolves to an unknown value rather '
        + 'than an error (RFC-036).',
      input:
        'A value the law looks up elsewhere: another law\'s output, or a fact '
        + 'from a register. Adds `source`, which is required and says where '
        + 'the value comes from (RFC-007).',
      output:
        'A value the article yields. Adds nothing to the common fields, and '
        + 'that is the point: every output is callable from another law '
        + 'through `source`, which asks for it by `name` (RFC-007). Callable '
        + 'is not the same as presentable, and what a portal offers a citizen '
        + 'is derived from `produces`, not from a flag on the output '
        + '(RFC-038).',
    };
    out.push({
      name: label.charAt(0).toUpperCase() + label.slice(1),
      slug: defAnchor(key),
      // Through renderDescription like every other description on the page,
      // so `backticks` become code and an RFC reference becomes a link.
      description: renderDescription(node.description ?? KIND_INTRO[label] ?? ''),
      fields,
    });
  }
  return out;
}

/**
 * The fourteen operations, in the order `definitions.operation` lists them.
 *
 * That order is the schema's own dispatch order, so the page follows it rather
 * than imposing an alphabetical one.
 */
export function operations(): Operation[] {
  const union = defs.operation?.oneOf ?? [];
  return union
    .map((branch) => {
      const target = branch.$ref ? resolveRef(branch.$ref) : null;
      if (!target) return null;
      const { name, node } = target;

      const opProp = node.properties?.operation;
      const keywords = opProp?.enum ?? (opProp?.const ? [opProp.const] : []);

      // The discriminator is the heading; it would be noise in the table.
      const fields = fieldsOf(node).filter((f) => f.name !== 'operation');
      const description = node.description ?? '';
      const slug = name.replace(/Operation$/, '').replace(/([A-Z])/g, '-$1').toLowerCase();

      /*
       * A form that accepts exactly one keyword is named by that keyword.
       * `FOREACH` is what an author writes and what a reader searches for;
       * "Foreach" is a prettified version of the definition name that exists
       * nowhere in a law file. It also stops the heading and the tag under it
       * saying the same word twice, which they did for nine of the fourteen.
       *
       * The multi-keyword forms keep their descriptive title, because there
       * the heading genuinely says something the tags do not: "Arithmetic"
       * over ADD/SUBTRACT/MULTIPLY/DIVIDE/MIN/MAX.
       */
      const single = keywords.length === 1;

      return {
        defName: name,
        slug,
        title: single ? keywords[0] : defTitle(name).replace(/ operation$/, ''),
        /* Shown as tags only when they add to the heading. */
        keywords: single ? [] : keywords,
        description: renderDescription(description),
        fields,
        examples: renderExamples(node),
        concept: OPERATION_CONCEPTS[name],
        zoek: haystack(name, keywords, description, fields.map((f) => f.name)),
      };
    })
    .filter((o): o is Operation => o !== null);
}

/**
 * The enums worth their own table: the vocabulary a law author picks from.
 *
 * Located by path rather than by scanning for every `enum` in the document —
 * a scan would also surface internal ones (the if/then branches, the operation
 * discriminators) that are machinery, not vocabulary.
 */
export function enumBlocks(): EnumBlock[] {
  const wanted: { path: string; title: string; node?: SchemaNode; key: string }[] = [
    {
      path: 'regulatory_layer',
      title: 'Regulatory layer',
      node: schema.properties?.regulatory_layer,
      key: 'regulatory_layer',
    },
    {
      path: 'machine_readable.execution.produces.legal_character',
      title: 'Legal character',
      node: producesForms().object.properties?.legal_character,
      key: 'legal_character',
    },
    {
      path: 'machine_readable.execution.produces.decision_type',
      title: 'Decision type',
      node: producesForms().object.properties?.decision_type,
      key: 'decision_type',
    },
    {
      path: 'type / type_spec.unit',
      title: 'Value types and units',
      node: defs.baseField?.properties?.type,
      key: 'type',
    },
  ];

  const blocks = wanted
    .filter((w) => w.node?.enum)
    .map((w) => ({
      slug: w.key.replace(/_/g, '-'),
      title: w.title,
      path: w.path,
      description: renderDescription(w.node!.description ?? ''),
      values: w.node!.enum!,
      glossary: GLOSSARY[w.key],
      zoek: haystack(w.title, w.path, w.node!.description, w.node!.enum),
    }));

  // Units ride along with the type table: they are the other half of how a
  // value declares what it is.
  const unit = defs.baseField?.properties?.type_spec?.properties?.unit;
  if (unit?.enum) {
    blocks.push({
      slug: 'unit',
      title: 'Units',
      path: 'type_spec.unit',
      description: renderDescription(unit.description ?? ''),
      values: unit.enum,
      glossary: undefined,
      zoek: haystack('unit', 'type_spec.unit', unit.description, unit.enum),
    });
  }
  return blocks;
}

/**
 * `legalBasis`: the fields of the citation every field and action may carry.
 *
 * It has its own section because the operand tables link to it — and because a
 * reader chasing "how does a value point back at the law text" should land on
 * a list of fields rather than on a `$ref`.
 */
export function legalBasisFields(): Field[] {
  return fieldsOf(defs.legalBasis ?? {});
}

/**
 * A string definition (`articleReference`, `variableReference`): no field
 * table, only the description and the pattern. Each has its own section
 * because a field links to it (`established_by`, `parameters_from`).
 */
export function stringDefinition(
  name: 'articleReference' | 'variableReference',
): { description: string; pattern: string } {
  const node = defs[name] ?? {};
  return {
    description: renderDescription(node.description ?? ''),
    pattern: node.pattern ?? '',
  };
}

/**
 * What may appear wherever an operation takes a value.
 *
 * In the schema this is `operationValue`, a bare `oneOf` with no description:
 * a variable reference, a literal, or a nested operation. That recursion is
 * what makes operations composable, so it is worth stating in prose; rendering
 * the `oneOf` itself would say nothing.
 */
export function operandForms(): { label: string; detail: string }[] {
  return [
    {
      label: 'A variable reference',
      detail:
        'A name beginning with $, resolved from the parameters, inputs, outputs '
        + 'and definitions in scope, such as $inkomen.',
    },
    {
      label: 'A literal',
      detail: 'A number, a boolean, null, or a string that does not begin with $.',
    },
    {
      label: 'Another operation',
      detail:
        'Any of the forms below, nested. This is what lets one action express a '
        + 'whole calculation rather than a single step.',
    },
  ];
}

/**
 * The headings SchemaReference renders, in document order.
 *
 * The page outline is built from what Astro's `render()` returns, and that
 * only sees headings written in the markdown — everything this component
 * generates is invisible to it, so the sidebar listed four entries for a page
 * with forty-five. Exporting them here lets the route splice them into the
 * outline at the point the component sits.
 *
 * Every entry is derived from the same function the component renders from, so
 * a section added there appears here without a second edit. What is hand-kept
 * is the ORDER, and the depth-2/3 entries whose headings the component writes
 * out literally. DocsOutline shows depth 2-3 only, so a depth-4 entry landing
 * in the wrong position costs nothing today; a new depth-3 section would need
 * a line here.
 */
export function referenceHeadings(): { depth: number; slug: string; text: string }[] {
  const out: { depth: number; slug: string; text: string }[] = [];
  const h = (depth: number, slug: string, text: string) => out.push({ depth, slug, text });

  h(2, 'law-file', 'The rulework');
  h(3, 'identifiers-per-layer', 'Identifiers per regulatory layer');
  h(4, 'top-legal-basis', 'legal_basis');
  h(3, 'articles', 'Articles');

  h(2, 'machine-readable', 'The machine_readable section');
  for (const sub of machineReadableSubStructures()) h(3, sub.slug, sub.title);
  h(3, 'execution', 'execution');
  h(3, 'action', 'Action');

  h(2, 'fields', 'Fields');
  for (const kind of fieldKinds()) {
    h(3, kind.slug, kind.name);
    // Depth 4: a nested object sits under the kind that owns it, and
    // DocsOutline keeps only depth 2-3, so these stay out of the sidebar
    // rather than flattening the hierarchy it is meant to show.
    for (const sub of subStructuresFor(kind.slug)) h(4, sub.slug, sub.title);
  }
  h(3, 'legal-basis', 'Legal basis');
  h(3, 'article-reference', 'Article reference');
  h(3, 'variable-reference', 'Variable reference');

  // Everything otherSubStructures() renders, at the depth the component uses.
  // Listed from the same function rather than by hand: the earlier version
  // enumerated these and had already drifted ten headings behind the page,
  // which stayed invisible only because DocsOutline drops depth 4.
  for (const subs of Object.values(otherSubStructures())) {
    for (const sub of subs) h(4, sub.slug, sub.title);
  }

  h(2, 'operations', 'Operations');
  h(3, 'operation-value', 'Operand value');
  for (const op of operations()) h(3, op.slug, op.title);

  h(2, 'vocabulary', 'Vocabulary');
  for (const block of enumBlocks()) h(3, block.slug, block.title);

  return out;
}

/** Counts for the filter's live region and the page intro. */
export function schemaStats(): { definitions: number; operations: number; topLevel: number } {
  return {
    definitions: Object.keys(defs).length,
    operations: operations().length,
    topLevel: Object.keys(schema.properties ?? {}).length,
  };
}
