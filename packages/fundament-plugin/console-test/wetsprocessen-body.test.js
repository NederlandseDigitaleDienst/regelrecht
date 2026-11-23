// What the wizard builds: the Wetsproces and the Secret of a private
// repository, and the checks that say what is wrong before the API server
// refuses it. Run with `node --test packages/fundament-plugin/console-test/`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  ACCENT_COLORS,
  addresses,
  buildSecret,
  buildWetsproces,
  commitError,
  hostnameError,
  pathError,
  phaseBadge,
  refError,
  repoUrlError,
} from '../console/wetsprocessen-body.js';

const base = {
  namespace: 'tnt-voorbeeld--wetsproces',
  hostname: 'zorgtoeslag',
  repoUrl: 'https://git.example.org/casus/a.git',
  ref: '',
  path: '',
  privateRepo: false,
  version: '',
  accentColor: '',
};

test('a public casus on the latest version leaves out what the plugin defaults', () => {
  const w = buildWetsproces(base);
  assert.deepEqual(w, {
    apiVersion: 'wetsproces.regelrecht.nl/v1alpha1',
    kind: 'Wetsproces',
    metadata: { name: 'zorgtoeslag', namespace: 'tnt-voorbeeld--wetsproces' },
    spec: {
      manifest: { repoUrl: 'https://git.example.org/casus/a.git', ref: 'main', path: 'packages/wetsproces/bron.yaml' },
      hostname: 'zorgtoeslag',
    },
  });
});

test('a private casus names its Secret, and the Secret is owned by the Wetsproces', () => {
  const w = buildWetsproces({ ...base, privateRepo: true, version: 'a'.repeat(40), accentColor: 'paars' });
  assert.deepEqual(w.spec.manifest.credentialsSecretRef, { name: 'zorgtoeslag-repo' });
  assert.equal(w.spec.version, 'a'.repeat(40));
  assert.deepEqual(w.spec.theme, { accentColor: 'paars' });

  const made = { ...w, metadata: { ...w.metadata, uid: 'uid-1' } };
  const secret = buildSecret(made, { username: 'lezer', token: 't0k3n' });
  assert.equal(secret.metadata.name, 'zorgtoeslag-repo');
  assert.equal(secret.metadata.namespace, 'tnt-voorbeeld--wetsproces');
  assert.deepEqual(secret.metadata.ownerReferences, [
    { apiVersion: 'wetsproces.regelrecht.nl/v1alpha1', kind: 'Wetsproces', name: 'zorgtoeslag', uid: 'uid-1' },
  ]);
  assert.deepEqual(secret.stringData, { username: 'lezer', token: 't0k3n' });
});

test('an own domain names the object after its first label', () => {
  const w = buildWetsproces({ ...base, hostname: 'zorg.example.nl' });
  assert.equal(w.metadata.name, 'zorg');
  assert.deepEqual(addresses('zorg.example.nl'), { burger: 'zorg.example.nl', medewerkers: 'medewerkers.zorg.example.nl' });
  assert.deepEqual(addresses('zorgtoeslag', 'c.example.org'), {
    burger: 'zorgtoeslag.c.example.org',
    medewerkers: 'zorgtoeslag-medewerkers.c.example.org',
  });
});

test('the checks refuse what the CRD and the plugin refuse', () => {
  assert.equal(repoUrlError('https://git.example.org/a.git'), null);
  assert.ok(repoUrlError('http://git.example.org/a.git'));
  assert.ok(repoUrlError('https://user:pw@git.example.org/a.git'));
  assert.equal(refError(''), null);
  assert.ok(refError('refs/heads/main'));
  assert.ok(refError('-main'));
  assert.equal(pathError('packages/wetsproces/bron.yaml'), null);
  assert.ok(pathError('../bron.yaml'));
  assert.equal(commitError('0123456789abcdef0123456789abcdef01234567'), null);
  assert.ok(commitError('0123456'));
  assert.equal(hostnameError('zorgtoeslag'), null);
  assert.equal(hostnameError('zorg.example.nl'), null);
  assert.ok(hostnameError('Zorgtoeslag'));
  assert.ok(hostnameError('a'.repeat(51)), 'with -medewerkers a label passes 63');
  assert.ok(hostnameError(''));
});

test('the accent colours are the ones the CRD allows', () => {
  const crd = readFileSync(new URL('../crds/wetsproces.regelrecht.nl_wetsprocessen.yaml', import.meta.url), 'utf8');
  const block = crd.slice(crd.indexOf('accentColor:'), crd.indexOf('type: string', crd.indexOf('accentColor:')));
  const allowed = [...block.matchAll(/^\s+- (\S+)$/gm)].map((m) => m[1]).filter((v) => v !== '""');
  assert.deepEqual(allowed, ACCENT_COLORS);
});

test('the badge follows the phase', () => {
  assert.equal(phaseBadge({ status: { phase: 'Ready' } }).color, 'success');
  assert.equal(phaseBadge({ status: { phase: 'Failed' } }).color, 'critical');
  assert.equal(phaseBadge({}).text, 'Bezig');
});
