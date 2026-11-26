// What the wizards build, the CRDs accept: each object's spec is checked
// against the openAPIV3Schema in crds/, so a field or value the wizard sends
// that the CRD does not know fails here and not on the cluster.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { load } from 'js-yaml';
import { ACCENT_COLORS, buildRegelHet, partijReference } from '../console/regelhets-body.js';
import {
  ORGANISATIE_EXISTING,
  ORGANISATIE_NEW,
  buildOrganisatie,
  organisatieReference,
} from '../console/organisaties-body.js';

function loadSchema(file) {
  const crd = load(readFileSync(new URL(`../crds/${file}`, import.meta.url), 'utf8'));
  return crd.spec.versions[0].schema.openAPIV3Schema;
}

// The x-kubernetes-* keywords are not evaluated: the CEL rules on the
// RegelHet spec are mirrored by the test on them below, and the keys of a
// list map are not checked for uniqueness.
const SUBSET = new Set([
  'x-kubernetes-list-type', 'x-kubernetes-list-map-keys', 'x-kubernetes-validations',
  'type', 'properties', 'required', 'enum', 'pattern', 'maxLength', 'minLength', 'maxItems',
  'items', 'additionalProperties', 'description', 'default', 'format',
]);

// Every keyword anywhere in a schema, also under a field no wizard state
// builds: a keyword outside the subset fails loudly instead of being
// skipped.
function unknownKeywords(schema, path = 'spec') {
  const found = Object.keys(schema)
    .filter((key) => !SUBSET.has(key))
    .map((key) => `${path}: the validator does not know the schema keyword ${key}`);
  for (const [name, sub] of Object.entries(schema.properties ?? {})) found.push(...unknownKeywords(sub, `${path}.${name}`));
  if (schema.items) found.push(...unknownKeywords(schema.items, `${path}[]`));
  if (schema.additionalProperties && typeof schema.additionalProperties === 'object') {
    found.push(...unknownKeywords(schema.additionalProperties, `${path}.*`));
  }
  return found;
}

function validate(schema, value, path = 'spec') {
  const errors = [];
  const type = schema.type;
  if (type === 'object') {
    if (value === null || typeof value !== 'object' || Array.isArray(value)) return [`${path}: not an object`];
    for (const name of schema.required ?? []) {
      if (!(name in value)) errors.push(`${path}.${name}: required`);
    }
    for (const [name, sub] of Object.entries(value)) {
      const known = schema.properties?.[name];
      if (known) errors.push(...validate(known, sub, `${path}.${name}`));
      else if (schema.additionalProperties === false) errors.push(`${path}.${name}: not in the CRD`);
      else if (schema.additionalProperties && typeof schema.additionalProperties === 'object') {
        errors.push(...validate(schema.additionalProperties, sub, `${path}.${name}`));
      }
    }
    return errors;
  }
  if (type === 'array') {
    if (!Array.isArray(value)) return [`${path}: not an array`];
    if (schema.maxItems !== undefined && value.length > schema.maxItems) errors.push(`${path}: more than ${schema.maxItems} items`);
    value.forEach((item, i) => errors.push(...validate(schema.items ?? {}, item, `${path}[${i}]`)));
    return errors;
  }
  if (type === 'string') {
    if (typeof value !== 'string') return [`${path}: not a string`];
    if (schema.enum && !schema.enum.includes(value)) errors.push(`${path}: ${JSON.stringify(value)} not in enum`);
    if (schema.maxLength !== undefined && value.length > schema.maxLength) errors.push(`${path}: longer than ${schema.maxLength}`);
    if (schema.minLength !== undefined && value.length < schema.minLength) errors.push(`${path}: shorter than ${schema.minLength}`);
    if (schema.pattern && !new RegExp(schema.pattern).test(value)) errors.push(`${path}: ${JSON.stringify(value)} does not match ${schema.pattern}`);
    return errors;
  }
  if (type === 'boolean') return typeof value === 'boolean' ? [] : [`${path}: not a boolean`];
  if (type === 'integer') return Number.isInteger(value) ? [] : [`${path}: not an integer`];
  return [`${path}: the validator does not know the type ${type}`];
}

const regelhetSchema = loadSchema('regelhet.regelrecht.nl_regelhets.yaml').properties.spec;
const organisatieSchema = loadSchema('regelhet.regelrecht.nl_organisaties.yaml').properties.spec;

const base = {
  namespace: 'tnt-voorbeeld--regelhet',
  hostname: 'zorgtoeslag',
  repoUrl: 'https://git.example.org/casus/a.git',
  ref: '',
  path: '',
};

const states = {
  public: base,
  'private with token': { ...base, privateRepo: true },
  stable: { ...base, stable: true },
  'stable with test accounts': { ...base, stable: true, testAccounts: true },
  'pinned version': { ...base, version: 'a'.repeat(40) },
  'accent colour': { ...base, accentColor: 'paars' },
  fsc4b: { ...base, fsc4b: true },
  'fsc4b with apps': { ...base, fsc4b: true, fsc4bOrigins: ['https://app.example.org', 'http://localhost:5173'] },
  'title and case': { ...base, title: 'Zorgtoeslag', casus: 'zorgtoeslag' },
  'party, same namespace': { ...base, partij: partijReference(`${base.namespace}/gemeente/ggd_voorbeeld`, base.namespace) },
  'party, executor elsewhere': { ...base, partij: partijReference('tnt-ander--regelhet/gemeente/ggd_voorbeeld', base.namespace) },
  // The wizard may hold every choice at once; the object keeps what goes together.
  'party with everything else chosen': {
    ...base,
    fsc4b: true,
    organisatie: { name: 'gemeente' },
    partij: partijReference(`${base.namespace}/gemeente/ggd_voorbeeld`, base.namespace),
  },
  'own domain': { ...base, hostname: 'zorg.example.nl', ref: 'v1.2/x', path: 'sub/bron.yaml' },
  'organisatie, new': {
    ...base,
    organisatie: organisatieReference({ choice: ORGANISATIE_NEW, newHostname: 'gemeente' }, base.namespace),
  },
  'organisatie, existing elsewhere': {
    ...base,
    organisatie: organisatieReference({ choice: ORGANISATIE_EXISTING, existing: 'tnt-ander--regelhet/gemeente' }, base.namespace),
  },
  everything: {
    ...base,
    privateRepo: true,
    version: 'b'.repeat(40),
    accentColor: 'robijnrood',
    title: 'Zorgtoeslag',
    casus: 'zorgtoeslag',
    fsc4b: true,
    fsc4bOrigins: ['https://app.example.org'],
    organisatie: { name: 'gemeente' },
  },
};

test('the validator knows every keyword of both CRDs', () => {
  assert.deepEqual(unknownKeywords(regelhetSchema), []);
  assert.deepEqual(unknownKeywords(organisatieSchema), []);
});

for (const [label, input] of Object.entries(states)) {
  test(`the CRD accepts a RegelHet: ${label}`, () => {
    assert.deepEqual(validate(regelhetSchema, buildRegelHet(input).spec), []);
  });
}

test('the CRD accepts every accent colour of the wizard', () => {
  for (const accentColor of ACCENT_COLORS) {
    assert.deepEqual(validate(regelhetSchema, buildRegelHet({ ...base, accentColor }).spec), [], accentColor);
  }
});

test('the CRD accepts an Organisatie', () => {
  const full = buildOrganisatie({
    namespace: base.namespace,
    hostname: 'gemeente',
    displayName: 'Gemeente Voorbeeld',
    logo: 'https://example.org/logo.svg',
    accentColor: 'groen',
  });
  assert.deepEqual(validate(organisatieSchema, full.spec), []);
  const bare = buildOrganisatie({ namespace: base.namespace, hostname: 'gemeente', displayName: 'Gemeente' });
  assert.deepEqual(validate(organisatieSchema, bare.spec), []);
});

test('the validator rejects what the CRD rejects', () => {
  const spec = buildRegelHet(base).spec;
  assert.ok(validate(regelhetSchema, { ...spec, hostname: 'Hoofdletter' }).length > 0);
  assert.ok(validate(regelhetSchema, { ...spec, theme: { accentColor: 'nepkleur' } }).length > 0);
  assert.ok(validate(regelhetSchema, { hostname: 'a' }).length > 0);
});

test('the validator rejects more apps than the CRD takes', () => {
  const origins = Array.from({ length: 21 }, (_, i) => `https://app${i}.example.org`);
  const spec = buildRegelHet({ ...base, fsc4b: true, fsc4bOrigins: origins }).spec;
  assert.ok(validate(regelhetSchema, spec).some((e) => e.includes('more than')));
});

// The CEL rules of the RegelHet spec, each as the same test on an object.
// The order is that of the CRD, so a rule added, removed or changed there
// fails here until it is mirrored.
const stable = (spec) => spec.auth?.mode === 'stable';
const CEL = [
  ["!(has(self.partij) && has(self.auth) && has(self.auth.mode) && self.auth.mode == 'stable')", (spec) => !(spec.partij && stable(spec))],
  ['!(has(self.partij) && has(self.fsc4b) && has(self.fsc4b.enabled) && self.fsc4b.enabled)', (spec) => !(spec.partij && spec.fsc4b?.enabled)],
  ['!(has(self.partij) && has(self.organisatie))', (spec) => !(spec.partij && spec.organisatie)],
  [
    "!(has(self.fsc4b) && has(self.fsc4b.enabled) && self.fsc4b.enabled && has(self.auth) && has(self.auth.mode) && self.auth.mode == 'stable')",
    (spec) => !(spec.fsc4b?.enabled && stable(spec)),
  ],
  [
    '!(has(self.fsc4b) && has(self.fsc4b.origins) && size(self.fsc4b.origins) > 0 && !(has(self.fsc4b.enabled) && self.fsc4b.enabled))',
    (spec) => !(spec.fsc4b?.origins?.length > 0 && !spec.fsc4b?.enabled),
  ],
];

test('what the wizard builds passes the CEL rules of the RegelHet spec', () => {
  const rules = (regelhetSchema['x-kubernetes-validations'] ?? []).map((r) => r.rule);
  assert.deepEqual(rules, CEL.map(([rule]) => rule), 'the CEL rules of the CRD changed: mirror them in this test');
  const partij = partijReference(`${base.namespace}/gemeente/ggd_voorbeeld`, base.namespace);
  const every = {
    ...states,
    // Every choice the rules exclude, made together.
    'stable with everything': { ...base, stable: true, fsc4b: true, fsc4bOrigins: ['https://app.example.org'], partij, organisatie: { name: 'gemeente' } },
    'apps without fsc4b': { ...base, fsc4bOrigins: ['https://app.example.org'] },
  };
  for (const [label, input] of Object.entries(every)) {
    const spec = buildRegelHet(input).spec;
    assert.deepEqual(validate(regelhetSchema, spec), [], label);
    CEL.forEach(([rule, holds]) => assert.ok(holds(spec), `${label}: ${rule}`));
  }
});

test('the name the wizard gives an object fits the rule on it', () => {
  const root = loadSchema('regelhet.regelrecht.nl_regelhets.yaml');
  assert.deepEqual(
    (root['x-kubernetes-validations'] ?? []).map((r) => r.rule),
    ['size(self.metadata.name) <= 40'],
  );
  // A DNS label is at most 63 characters, so the wizard has to refuse a
  // longer first label itself (NAME_MAX in regelhets-body.js); what it builds for a
  // name within the rule passes.
  assert.ok(buildRegelHet({ ...base, hostname: 'a'.repeat(40) }).metadata.name.length <= 40);
});
