// What the console pages build and check, apart from the DOM, so node:test
// can run it (console-test/). The rules repeat those of the CRD
// (api/v1alpha1) where a page can say what is wrong before the API server
// refuses it; the API server and the plugin check again.

export const REGELHET_RESOURCE = {
  group: 'regelhet.regelrecht.nl',
  version: 'v1alpha1',
  resource: 'regelhets',
};

export const SECRET_RESOURCE = { group: '', version: 'v1', resource: 'secrets' };

export const DEFAULT_REPO_URL = 'https://code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026.git';
export const DEFAULT_REF = 'main';
export const DEFAULT_PATH = 'packages/regelhet/bron.yaml';

// The themes of the NLDD design system, which the server takes as its
// accent colour (packages/regelhet/src/theme.rs). The CRD lists the same.
export const ACCENT_COLORS = [
  'bruin',
  'donkerblauw',
  'donkerbruin',
  'donkergeel',
  'donkergroen',
  'geel',
  'groen',
  'hemelblauw',
  'lichtblauw',
  'mintgroen',
  'mosgroen',
  'oranje',
  'paars',
  'robijnrood',
  'rood',
  'roze',
  'violet',
];

// The name of a RegelHet names its Helm release and the objects of the
// chart, so the CRD keeps it to 40 characters.
const NAME_MAX = 40;
const LABEL = /^[a-z0-9]([a-z0-9-]*[a-z0-9])?$/;
const HOSTNAME = /^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*$/;
const REF = /^[A-Za-z0-9._/][A-Za-z0-9._/-]*$/;
const COMMIT = /^[0-9a-f]{40}$/;

// Each check returns the message a field shows, or null.

export function repoUrlError(value) {
  let url;
  try {
    url = new URL(value);
  } catch {
    return 'Vul een adres in dat met https:// begint.';
  }
  if (url.protocol !== 'https:' || !url.host) return 'Vul een adres in dat met https:// begint.';
  if (url.username || url.password) return 'Zet geen inloggegevens in het adres; die vraagt de volgende stap.';
  return null;
}

export function refError(value) {
  if (!value) return null;
  if (!REF.test(value) || value.includes('..') || value.endsWith('.lock')) {
    return 'Een branch of tag, zoals main of v1.2.';
  }
  if (value.startsWith('refs/')) return 'Alleen de korte naam, zoals main.';
  return null;
}

export function pathError(value) {
  if (!value) return null;
  if (value.startsWith('/') || value.includes('..') || /\s/.test(value)) {
    return 'Een pad binnen de repository, zoals packages/regelhet/bron.yaml.';
  }
  return null;
}

export function usernameError(value) {
  return value ? null : 'Vul de gebruikersnaam in waar het token bij hoort.';
}

export function tokenError(value) {
  return value ? null : 'Vul een token in dat de repository mag lezen.';
}

export function commitError(value) {
  return COMMIT.test(value) ? null : 'De volledige commit-hash: 40 tekens, 0-9 en a-f.';
}

export function hostnameError(value) {
  if (!value) return 'Vul een naam in.';
  if (!HOSTNAME.test(value)) return 'Kleine letters, cijfers en streepjes; een eigen domein met punten ertussen.';
  if (value.split('.').some((label) => label.length > 63)) return 'Een deel tussen twee punten is ten hoogste 63 tekens.';
  if (!value.includes('.') && value.length > 50) {
    return 'Ten hoogste 50 tekens: de medewerkers krijgen er -medewerkers achter.';
  }
  return nameFromHostname(value).length > NAME_MAX ? `Het eerste deel is ten hoogste ${NAME_MAX} tekens.` : null;
}

export function namespaceError(value) {
  return LABEL.test(value) ? null : 'Kies een namespace.';
}

// The name of the object: the hostname, or its first label when it is a
// domain of its own.
export function nameFromHostname(hostname) {
  return hostname.split('.')[0];
}

// The addresses the plugin gives a hostname (resources.go, hostsOf), with
// `domain` standing for the base domain of the installation, which a page
// cannot read.
export function addresses(hostname, domain = '‹basisdomein›') {
  if (hostname.includes('.')) return { burger: hostname, medewerkers: `medewerkers.${hostname}` };
  return { burger: `${hostname}.${domain}`, medewerkers: `${hostname}-medewerkers.${domain}` };
}

export function secretName(name) {
  return `${name}-repo`;
}

// The RegelHet object for the choices of the wizard.
export function buildRegelHet({ namespace, hostname, repoUrl, ref, path, privateRepo, version, accentColor }) {
  const name = nameFromHostname(hostname);
  const manifest = { repoUrl, ref: ref || DEFAULT_REF, path: path || DEFAULT_PATH };
  if (privateRepo) manifest.credentialsSecretRef = { name: secretName(name) };
  const spec = { manifest, hostname };
  if (version) spec.version = version;
  if (accentColor) spec.theme = { accentColor };
  return {
    apiVersion: `${REGELHET_RESOURCE.group}/${REGELHET_RESOURCE.version}`,
    kind: 'RegelHet',
    metadata: { name, namespace },
    spec,
  };
}

// The Secret with the credentials of a private repository, owned by the
// RegelHet `owner` so it goes when the RegelHet goes.
export function buildSecret(owner, { username, token }) {
  return {
    apiVersion: 'v1',
    kind: 'Secret',
    metadata: {
      name: secretName(owner.metadata.name),
      namespace: owner.metadata.namespace,
      labels: { 'regelhet.regelrecht.nl/name': owner.metadata.name },
      ownerReferences: [
        {
          apiVersion: owner.apiVersion,
          kind: owner.kind,
          name: owner.metadata.name,
          uid: owner.metadata.uid,
        },
      ],
    },
    type: 'Opaque',
    stringData: { username, token },
  };
}

// What the status says, for a badge: its colour and its word.
export function phaseBadge(item) {
  switch (item?.status?.phase) {
    case 'Ready':
      return { color: 'success', text: 'Draait' };
    case 'Failed':
      return { color: 'critical', text: 'Mislukt' };
    default:
      return { color: 'warning', text: 'Bezig' };
  }
}
