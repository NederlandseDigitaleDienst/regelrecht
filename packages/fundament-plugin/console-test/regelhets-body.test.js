// What the wizard builds: the RegelHet and the Secret of a private
// repository, and the checks that say what is wrong before the API server
// refuses it. Run with `node --test packages/fundament-plugin/console-test/*.test.js`.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  ACCENT_COLORS,
  addresses,
  buildSecret,
  buildRegelHet,
  fsc4bDiscoveryUrl,
  keycloakSecretName,
  commitError,
  hostnameError,
  pathError,
  phaseBadge,
  refError,
  repoUrlError,
  titleError,
  casusError,
  partyChoices,
  partijReference,
  partijText,
  partyAccentColor,
} from '../console/regelhets-body.js';

const base = {
  namespace: 'tnt-voorbeeld--regelhet',
  hostname: 'zorgtoeslag',
  repoUrl: 'https://git.example.org/casus/a.git',
  ref: '',
  path: '',
  privateRepo: false,
  version: '',
  accentColor: '',
};

test('a public casus on the latest version leaves out what the plugin defaults', () => {
  const w = buildRegelHet(base);
  assert.deepEqual(w, {
    apiVersion: 'regelhet.regelrecht.nl/v1alpha1',
    kind: 'RegelHet',
    metadata: { name: 'zorgtoeslag', namespace: 'tnt-voorbeeld--regelhet' },
    spec: {
      manifest: { repoUrl: 'https://git.example.org/casus/a.git', ref: 'main', path: 'bron.yaml' },
      hostname: 'zorgtoeslag',
    },
  });
});

test('a private casus names its Secret, and the Secret is owned by the RegelHet', () => {
  const w = buildRegelHet({ ...base, privateRepo: true, version: 'a'.repeat(40), accentColor: 'paars' });
  assert.deepEqual(w.spec.manifest.credentialsSecretRef, { name: 'zorgtoeslag-repo' });
  assert.equal(w.spec.version, 'a'.repeat(40));
  assert.deepEqual(w.spec.theme, { accentColor: 'paars' });

  const made = { ...w, metadata: { ...w.metadata, uid: 'uid-1' } };
  const secret = buildSecret(made, { username: 'lezer', token: 't0k3n' });
  assert.equal(secret.metadata.name, 'zorgtoeslag-repo');
  assert.equal(secret.metadata.namespace, 'tnt-voorbeeld--regelhet');
  assert.deepEqual(secret.metadata.ownerReferences, [
    { apiVersion: 'regelhet.regelrecht.nl/v1alpha1', kind: 'RegelHet', name: 'zorgtoeslag', uid: 'uid-1' },
  ]);
  assert.deepEqual(secret.stringData, { username: 'lezer', token: 't0k3n' });
  // The token is for the one copy of the casus: the plugin deletes it after.
  assert.deepEqual(secret.metadata.annotations, { 'regelhet.regelrecht.nl/delete-after-snapshot': 'true' });
});

test('an own domain names the object after its first label', () => {
  const w = buildRegelHet({ ...base, hostname: 'zorg.example.nl' });
  assert.equal(w.metadata.name, 'zorg');
  assert.deepEqual(addresses('zorg.example.nl'), {
    burger: 'zorg.example.nl',
    medewerkers: 'medewerkers.zorg.example.nl',
    login: 'zorg-login.example.nl',
  });
  assert.deepEqual(addresses('zorgtoeslag', 'c.example.org'), {
    burger: 'zorgtoeslag.c.example.org',
    medewerkers: 'zorgtoeslag-medewerkers.c.example.org',
    login: 'zorgtoeslag-login.c.example.org',
  });
});

test('the checks refuse what the CRD and the plugin refuse', () => {
  assert.equal(repoUrlError('https://git.example.org/a.git'), null);
  assert.ok(repoUrlError('http://git.example.org/a.git'));
  assert.ok(repoUrlError('https://user:pw@git.example.org/a.git'));
  assert.equal(refError(''), null);
  assert.ok(refError('refs/heads/main'));
  assert.ok(refError('-main'));
  assert.equal(pathError('packages/regelhet/bron.yaml'), null);
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
  const crd = readFileSync(new URL('../crds/regelhet.regelrecht.nl_regelhets.yaml', import.meta.url), 'utf8');
  const block = crd.slice(crd.indexOf('accentColor:'), crd.indexOf('type: string', crd.indexOf('accentColor:')));
  const allowed = [...block.matchAll(/^\s+- (\S+)$/gm)].map((m) => m[1]).filter((v) => v !== '""');
  assert.deepEqual(allowed, ACCENT_COLORS);
});

test('the badge follows the phase', () => {
  assert.equal(phaseBadge({ status: { phase: 'Ready' } }).color, 'success');
  assert.equal(phaseBadge({ status: { phase: 'Failed' } }).color, 'critical');
  assert.equal(phaseBadge({}).text, 'Bezig');
});

test('stable mode asks for the Keycloak of the chart; test mode says nothing', () => {
  assert.equal(buildRegelHet(base).spec.auth, undefined);
  const w = buildRegelHet({ ...base, stable: true, testAccounts: true });
  assert.deepEqual(w.spec.auth, { mode: 'stable', testAccounts: true });
  assert.deepEqual(buildRegelHet({ ...base, stable: true }).spec.auth, { mode: 'stable', testAccounts: false });
  assert.equal(keycloakSecretName('zorgtoeslag'), 'zorgtoeslag-keycloak');
});

test('FSC voor Burgers is in the spec only when chosen, and never in stable mode', () => {
  assert.equal(buildRegelHet(base).spec.fsc4b, undefined);
  assert.deepEqual(buildRegelHet({ ...base, fsc4b: true }).spec.fsc4b, { enabled: true });
  assert.equal(buildRegelHet({ ...base, fsc4b: true, stable: true }).spec.fsc4b, undefined);
});

test('the discovery URL of FSC voor Burgers is on the citizens\' host', () => {
  assert.equal(fsc4bDiscoveryUrl('a', 'cluster.example.org'), 'https://a.cluster.example.org/fsc4b/discovery');
  assert.equal(fsc4bDiscoveryUrl('zorg.example.nl'), 'https://zorg.example.nl/fsc4b/discovery');
});

// Two procedures of one project: a municipality whose case asks the GGD
// and the court something, the GGD already running for it as a procedure of
// its own, and one that is a party itself.
const procedures = [
  {
    metadata: { name: 'parkeerkaart', namespace: 'tnt-voorbeeld--regelhet' },
    spec: { title: 'Gehandicaptenparkeerkaart' },
    status: {
      askedParties: [
        { cell: 'ggd_wilgendaal', name: 'GGD Wilgendaal' },
        { cell: 'rechtbank', name: 'Rechtbank' },
      ],
      parties: [{ cell: 'rechtbank', url: 'https://r-medewerkers.example.org/' }],
    },
  },
  {
    metadata: { name: 'ggd', namespace: 'tnt-voorbeeld--ggd' },
    spec: { partij: { cel: 'lab', uitvoerder: { name: 'x' } } },
    status: { askedParties: [{ cell: 'lab' }] },
  },
  {
    metadata: { name: 'stabiel', namespace: 'tnt-voorbeeld--regelhet' },
    spec: { auth: { mode: 'stable' } },
    status: { askedParties: [{ cell: 'ggd_wilgendaal' }] },
  },
];

test('a procedure can run as a party the case of another asks something of, and no other runs as', () => {
  assert.deepEqual(partyChoices(procedures), [
    {
      value: 'tnt-voorbeeld--regelhet/parkeerkaart/ggd_wilgendaal',
      label: 'GGD Wilgendaal voor Gehandicaptenparkeerkaart (tnt-voorbeeld--regelhet/parkeerkaart)',
      accentColor: '',
    },
  ]);
  assert.deepEqual(partyChoices(undefined), []);
  // A procedure that names the cell already, linked or not, takes it.
  const claiming = {
    metadata: { name: 'ggd-wilgendaal', namespace: 'tnt-voorbeeld--ggd' },
    spec: { partij: { cel: 'ggd_wilgendaal', uitvoerder: { name: 'parkeerkaart', namespace: 'tnt-voorbeeld--regelhet' } } },
  };
  assert.deepEqual(partyChoices([...procedures, claiming]), []);
});

test('a party that chose no colour is told apart from its executor', () => {
  assert.equal(partyAccentColor(''), 'rood');
  assert.equal(partyAccentColor('paars'), 'rood');
  assert.equal(partyAccentColor('rood'), 'robijnrood');
});

test('a party names its executor, with its namespace only when it is another', () => {
  const choice = 'tnt-voorbeeld--regelhet/parkeerkaart/ggd_wilgendaal';
  assert.deepEqual(partijReference(choice, 'tnt-voorbeeld--regelhet'), {
    cel: 'ggd_wilgendaal',
    uitvoerder: { name: 'parkeerkaart' },
  });
  assert.deepEqual(partijReference(choice, 'tnt-voorbeeld--ggd'), {
    cel: 'ggd_wilgendaal',
    uitvoerder: { name: 'parkeerkaart', namespace: 'tnt-voorbeeld--regelhet' },
  });
  assert.equal(partijReference('', 'x'), undefined);
  assert.equal(
    partijText({ cel: 'ggd_wilgendaal', uitvoerder: { name: 'parkeerkaart' } }, 'tnt-voorbeeld--regelhet'),
    'ggd_wilgendaal voor tnt-voorbeeld--regelhet/parkeerkaart',
  );
});

test('a party is a test procedure for staff only: no FSC voor Burgers, no organisatie', () => {
  const partij = { cel: 'ggd_wilgendaal', uitvoerder: { name: 'parkeerkaart' } };
  const w = buildRegelHet({ ...base, hostname: 'ggd', fsc4b: true, organisatie: { name: 'o' }, partij });
  assert.deepEqual(w.spec.partij, partij);
  assert.equal(w.spec.fsc4b, undefined);
  assert.equal(w.spec.organisatie, undefined);
  assert.equal(buildRegelHet({ ...base, stable: true, partij }).spec.partij, undefined);
  assert.equal(buildRegelHet(base).spec.partij, undefined);
  assert.equal(buildRegelHet(base).spec.parties, undefined, 'ownInstances is gone');
});

test('the CRD knows spec.partij and no longer parties.ownInstances', () => {
  const crd = readFileSync(new URL('../crds/regelhet.regelrecht.nl_regelhets.yaml', import.meta.url), 'utf8');
  assert.ok(crd.includes('              partij:'));
  assert.ok(!crd.includes('ownInstances'));
});

test('a title is in the spec only when given, and the check refuses what the CRD refuses', () => {
  assert.equal(buildRegelHet(base).spec.title, undefined);
  assert.equal(buildRegelHet({ ...base, title: '  ' }).spec.title, undefined);
  assert.equal(buildRegelHet({ ...base, title: ' IIT Groningen ' }).spec.title, 'IIT Groningen');
  assert.equal(titleError(''), null);
  assert.equal(titleError('é'.repeat(80)), null);
  assert.ok(titleError('é'.repeat(81)));
  assert.ok(titleError('een\ntwee'));
  const crd = readFileSync(new URL('../crds/regelhet.regelrecht.nl_regelhets.yaml', import.meta.url), 'utf8');
  const block = crd.slice(crd.indexOf('              title:'), crd.indexOf('type: string', crd.indexOf('              title:')));
  assert.match(block, /maxLength: 80\n/);
});

test('a case is in the spec only when chosen, by the name of its case file', () => {
  assert.equal(buildRegelHet(base).spec.casus, undefined);
  assert.equal(buildRegelHet({ ...base, casus: 'bedrijfsparkeervergunning' }).spec.casus, 'bedrijfsparkeervergunning');
  assert.equal(casusError(''), null);
  assert.equal(casusError('individuele_inkomenstoeslag'), null);
  assert.ok(casusError('bedrijfsparkeervergunning.yaml'));
  assert.ok(casusError('casussen/a'));
  assert.ok(casusError('A'));
  assert.ok(casusError('a'.repeat(101)));
  const crd = readFileSync(new URL('../crds/regelhet.regelrecht.nl_regelhets.yaml', import.meta.url), 'utf8');
  assert.ok(crd.includes('pattern: ^([a-z0-9][a-z0-9_-]*)?$'), 'the wizard checks what the CRD checks');
});
