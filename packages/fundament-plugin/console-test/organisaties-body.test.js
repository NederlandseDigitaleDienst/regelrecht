// What the wizards build for an organisatie: the Organisatie object, the
// spec.organisatie of a RegelHet for each answer of the step Organisatie,
// and the checks. Run with `node --test packages/fundament-plugin/console-test/*.test.js`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  ORGANISATIE_EXISTING,
  ORGANISATIE_NEW,
  ORGANISATIE_NONE,
  buildOrganisatie,
  displayNameError,
  hostnamesClash,
  logoError,
  organisatieKey,
  organisatieReference,
  parseOrganisatieKey,
  sortOrganisaties,
} from '../console/organisaties-body.js';
import { buildRegelHet } from '../console/regelhets-body.js';

const ns = 'tnt-voorbeeld--wilgendaal';

test('an organisatie leaves out what it does not set', () => {
  assert.deepEqual(buildOrganisatie({ namespace: ns, hostname: 'wilgendaal', displayName: 'Gemeente Wilgendaal' }), {
    apiVersion: 'regelhet.regelrecht.nl/v1alpha1',
    kind: 'Organisatie',
    metadata: { name: 'wilgendaal', namespace: ns },
    spec: { displayName: 'Gemeente Wilgendaal', hostname: 'wilgendaal' },
  });
  const full = buildOrganisatie({
    namespace: ns,
    hostname: 'wilgendaal.example.nl',
    displayName: 'Gemeente Wilgendaal',
    logo: 'https://example.org/logo.svg',
    accentColor: 'groen',
  });
  assert.equal(full.metadata.name, 'wilgendaal');
  assert.equal(full.spec.logo, 'https://example.org/logo.svg');
  assert.deepEqual(full.spec.theme, { accentColor: 'groen' });
});

const base = {
  namespace: 'tnt-voorbeeld--parkeerkaart',
  hostname: 'parkeerkaart',
  repoUrl: 'https://git.example.org/casus/a.git',
};

test('no organisatie leaves the regelhet as it was', () => {
  const ref = organisatieReference({ choice: ORGANISATIE_NONE, existing: `${ns}/wilgendaal` }, base.namespace);
  assert.equal(ref, undefined);
  assert.equal(buildRegelHet({ ...base, organisatie: ref }).spec.organisatie, undefined);
});

test('a new organisatie lands in the namespace of the regelhet and is named by its hostname', () => {
  const ref = organisatieReference({ choice: ORGANISATIE_NEW, newHostname: 'wilgendaal' }, base.namespace);
  assert.deepEqual(buildRegelHet({ ...base, organisatie: ref }).spec.organisatie, { name: 'wilgendaal' });
});

test('an existing organisatie names its namespace only when it is another', () => {
  const elsewhere = organisatieReference({ choice: ORGANISATIE_EXISTING, existing: `${ns}/wilgendaal` }, base.namespace);
  assert.deepEqual(elsewhere, { namespace: ns, name: 'wilgendaal' });
  const here = organisatieReference({ choice: ORGANISATIE_EXISTING, existing: `${ns}/wilgendaal` }, ns);
  assert.deepEqual(here, { name: 'wilgendaal' });
  assert.equal(organisatieReference({ choice: ORGANISATIE_EXISTING, existing: '' }, ns), undefined);
});

test('the key of an organisatie goes both ways', () => {
  const item = { metadata: { namespace: ns, name: 'wilgendaal' } };
  assert.deepEqual(parseOrganisatieKey(organisatieKey(item)), { namespace: ns, name: 'wilgendaal' });
  assert.equal(parseOrganisatieKey('zonder-slash'), null);
});

test('the checks refuse what the CRD and the plugin refuse', () => {
  assert.equal(displayNameError('Gemeente Wilgendaal'), null);
  assert.ok(displayNameError(''));
  assert.ok(displayNameError('x'.repeat(101)));
  assert.equal(logoError(''), null);
  assert.equal(logoError('https://example.org/logo.svg'), null);
  assert.ok(logoError('http://example.org/logo.svg'));
  assert.ok(logoError('https://u:p@example.org/logo.svg'));
  assert.ok(logoError('geen adres'));
});

test('two hostnames clash when they share an address', () => {
  assert.ok(hostnamesClash('wilgendaal', 'wilgendaal'));
  assert.ok(hostnamesClash('wilgendaal-medewerkers', 'wilgendaal'));
  assert.ok(!hostnamesClash('wilgendaal', 'parkeerkaart'));
  assert.ok(!hostnamesClash('', 'parkeerkaart'));
});

test('organisaties sort by the name people see', () => {
  const items = [
    { metadata: { name: 'b' }, spec: { displayName: 'Gemeente Zandvoort' } },
    { metadata: { name: 'a' }, spec: { displayName: 'Gemeente Wilgendaal' } },
  ];
  assert.deepEqual(sortOrganisaties(items).map((o) => o.metadata.name), ['a', 'b']);
});

test('the limits are the ones the CRD sets', () => {
  const crd = readFileSync(new URL('../crds/regelhet.regelrecht.nl_organisaties.yaml', import.meta.url), 'utf8');
  const block = crd.slice(crd.indexOf('displayName:'), crd.indexOf('type: string', crd.indexOf('displayName:')));
  assert.match(block, /maxLength: 100/);
});
