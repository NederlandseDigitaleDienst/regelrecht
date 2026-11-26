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

export const DEFAULT_REF = 'main';
// A manifest repository has its bron.yaml at the root.
export const DEFAULT_PATH = 'bron.yaml';

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
// The title in the bar of the screens: as the CRD and the server allow it,
// one line of at most 80 characters (spec.title).
export const TITLE_MAX = 80;
const CONTROL = /[\u0000-\u001f\u007f]/;

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
    return 'Een pad binnen de repository, zoals bron.yaml.';
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

// A case file under casussen/, by its name without .yaml (spec.casus).
const CASUS = /^[a-z0-9][a-z0-9_-]*$/;
export const CASUS_MAX = 100;

export function casusError(value) {
  if (!value) return null;
  if (value.endsWith('.yaml')) return 'De naam zonder .yaml, zoals bedrijfsparkeervergunning.';
  if (!CASUS.test(value)) return 'De naam van het casusbestand: kleine letters, cijfers, _ en -.';
  return value.length > CASUS_MAX ? `Ten hoogste ${CASUS_MAX} tekens.` : null;
}

// An origin a browser app may call FSC voor Burgers from
// (spec.fsc4b.origins): https with a host and an optional port, or http on
// the developer's own machine; no path. The CRD holds the same pattern.
const FSC4B_ORIGIN = /^(https:\/\/[a-z0-9]([a-z0-9.-]*[a-z0-9])?(:[0-9]{1,5})?|http:\/\/(localhost|127\.0\.0\.1)(:[0-9]{1,5})?)$/;
export const FSC4B_ORIGINS_MAX = 20;

/** The origins in `value`: one per line or separated by commas or spaces, without empty ones. */
export function fsc4bOrigins(value) {
  return value
    .split(/[\s,]+/)
    .map((o) => o.trim())
    .filter(Boolean);
}

export function fsc4bOriginsError(value) {
  const origins = fsc4bOrigins(value);
  const fout = origins.find((o) => !FSC4B_ORIGIN.test(o));
  if (fout) return `${fout} is geen herkomst: https://host of https://host:poort, of http op localhost, zonder pad.`;
  return origins.length > FSC4B_ORIGINS_MAX ? `Ten hoogste ${FSC4B_ORIGINS_MAX} adressen.` : null;
}

export function titleError(value) {
  if (!value) return null;
  if (CONTROL.test(value)) return 'Eén regel tekst.';
  return [...value].length > TITLE_MAX ? `Ten hoogste ${TITLE_MAX} tekens.` : null;
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
  if (hostname.includes('.')) {
    const [first, ...rest] = hostname.split('.');
    return { burger: hostname, medewerkers: `medewerkers.${hostname}`, login: `${first}-login.${rest.join('.')}` };
  }
  return {
    burger: `${hostname}.${domain}`,
    medewerkers: `${hostname}-medewerkers.${domain}`,
    login: `${hostname}-login.${domain}`,
  };
}

// The Secret the chart generates for the Keycloak of stable mode
// (charts/regelhet, templates/keycloak.yaml): the admin password and the
// password of the test accounts. The pages name it, and never read it.
export function keycloakSecretName(name) {
  return `${name}-keycloak`;
}

export function secretName(name) {
  return `${name}-repo`;
}

// The two answers to "does this procedure run as a party of another?" in the
// wizard.
export const ROLE_EXECUTOR = 'uitvoerder';
export const ROLE_PARTY = 'partij';

// The parties a new procedure can run as: per procedure in `items` that can
// be an executor (no party itself, test mode), each party its case asks
// something of (status.askedParties) that no procedure runs as or names in
// its spec.partij yet. Its value is <namespace>/<name>/<cel>; the label
// names the party and the procedure, and accentColor is the executor's.
export function partyChoices(items) {
  const list = items ?? [];
  const claimed = new Set(
    list
      .filter((o) => o?.spec?.partij)
      .map((o) => {
        const u = o.spec.partij.uitvoerder ?? {};
        return `${u.namespace || o.metadata.namespace}/${u.name}/${o.spec.partij.cel}`;
      }),
  );
  const choices = [];
  for (const item of list) {
    if (item?.spec?.partij || item?.spec?.auth?.mode === 'stable') continue;
    const procedure = `${item.metadata.namespace}/${item.metadata.name}`;
    const taken = new Set((item.status?.parties ?? []).map((p) => p.cell));
    for (const asked of item.status?.askedParties ?? []) {
      const value = `${procedure}/${asked.cell}`;
      if (taken.has(asked.cell) || claimed.has(value)) continue;
      choices.push({
        value,
        label: `${asked.name || asked.cell} voor ${item.spec?.title || item.metadata.name} (${procedure})`,
        accentColor: item.spec?.theme?.accentColor ?? '',
      });
    }
  }
  return choices.sort((a, b) => a.label.localeCompare(b.label, 'nl'));
}

// The accent colour of a party that chose none, beside an executor whose
// accent is `executor`: red, or ruby red beside a red executor, so the two
// are told apart at a glance. The plugin does the same (parties.go,
// partyAccent).
export function partyAccentColor(executor) {
  return executor === 'rood' ? 'robijnrood' : 'rood';
}

// What spec.partij of a procedure in `namespace` says for a choice of
// partyChoices: the cel and the executor, with its namespace only when it is
// another than the procedure's own.
export function partijReference(choice, namespace) {
  const [ns, name, cel] = String(choice ?? '').split('/');
  if (!ns || !name || !cel) return undefined;
  return { cel, uitvoerder: ns === namespace ? { name } : { name, namespace: ns } };
}

// How a page names spec.partij of a procedure in `namespace`.
export function partijText(partij, namespace) {
  const u = partij.uitvoerder ?? {};
  return `${partij.cel} voor ${u.namespace || namespace}/${u.name}`;
}

// The RegelHet object for the choices of the wizard.
export function buildRegelHet({
  namespace,
  hostname,
  repoUrl,
  ref,
  path,
  privateRepo,
  version,
  accentColor,
  title = '',
  casus = '',
  stable = false,
  testAccounts = false,
  fsc4b = false,
  fsc4bOrigins: origins = [],
  partij,
  organisatie,
}) {
  const name = nameFromHostname(hostname);
  const manifest = { repoUrl, ref: ref || DEFAULT_REF, path: path || DEFAULT_PATH };
  if (privateRepo) manifest.credentialsSecretRef = { name: secretName(name) };
  const spec = { manifest, hostname };
  if (version) spec.version = version;
  // Without a choice the server runs the manifest's only case.
  if (casus) spec.casus = casus;
  if (accentColor) spec.theme = { accentColor };
  // Without a title the screens carry the case's own.
  if (title.trim()) spec.title = title.trim();
  // Test mode is the CRD's default, so a test RegelHet says nothing.
  if (stable) spec.auth = { mode: 'stable', testAccounts };
  // A party (partijReference) runs one server for its staff, linked with
  // its executor with keys made at start: test mode only, without a
  // citizens' side for FSC voor Burgers or the front of an organisatie.
  if (partij && !stable) {
    spec.partij = partij;
  } else {
    // FSC voor Burgers runs on the demo Stelsel, test mode only; off is the
    // CRD's default, so an object without it says nothing.
    // The browser apps that may call it from their own address, such as the
    // citizen's app (frontend-regelhet, burgerapp), only with it on.
    if (fsc4b && !stable) spec.fsc4b = origins.length ? { enabled: true, origins } : { enabled: true };
    // The Organisatie whose front lists it (organisaties-body.js,
    // organisatieReference); without one it runs on its own.
    if (organisatie) spec.organisatie = organisatie;
  }
  return {
    apiVersion: `${REGELHET_RESOURCE.group}/${REGELHET_RESOURCE.version}`,
    kind: 'RegelHet',
    metadata: { name, namespace },
    spec,
  };
}

// The Secret with the credentials of a private repository, owned by the
// RegelHet `owner` so it goes when the RegelHet goes.
// The annotation that lets the plugin delete the Secret once it has copied
// the case (api/v1alpha1, DeleteAfterSnapshotAnnotation): the token is needed
// for that one copy only.
export const DELETE_AFTER_SNAPSHOT = 'regelhet.regelrecht.nl/delete-after-snapshot';

export function buildSecret(owner, { username, token }) {
  return {
    apiVersion: 'v1',
    kind: 'Secret',
    metadata: {
      name: secretName(owner.metadata.name),
      namespace: owner.metadata.namespace,
      labels: { 'regelhet.regelrecht.nl/name': owner.metadata.name },
      annotations: { [DELETE_AFTER_SNAPSHOT]: 'true' },
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

// What a failed procedure means and what to do about it, in the words of
// the wizard: the reasons of the Ready condition (controller.go,
// organisatie.go, parties.go, snapshot.go) and, for NotStarting and
// SnapshotFailed, the messages the pods and the copy of the case leave
// (pods.go, snapshot.sh, the server's own errors in
// packages/regelhet/src/{app,case,service}.rs, the README under "A RegelHet
// that does not start"). The first pattern
// that matches counts. Null for a failure this table does not know, so the
// page shows the plugin's own message as before.
const BY_MESSAGE = [
  [
    /Authentication failed|could not read Username|terminal prompts disabled|git server refused access/,
    'De repository met de casus weigert het token.',
    'Maak een nieuw token dat de repository mag lezen, en zet het in het Secret van deze procedure, of maak de procedure opnieuw aan.',
  ],
  [
    /couldn't find remote ref|has no branch, tag or commit/,
    'De branch, tag of commit van de casus bestaat niet.',
    'Controleer de ref bij Instellingen.',
  ],
  [
    /repository '.*' not found|Repository not found|is not there, or is private/,
    'De repository van de casus bestaat niet, of is privé zonder token.',
    'Controleer het adres van de repository. Bij een privé-repository hoort een token.',
  ],
  [
    /the manifest does not read|is not in commit|is no source manifest|which holds no YAML file|lies outside the repository|is no directory below the root/,
    'Het manifest of een map die het noemt staat niet in de repository, of klopt niet.',
    'Controleer het pad naar het manifest (meestal bron.yaml) en de mappen erin. Met regelrecht-regelhet check <manifest> zie je lokaal binnen een seconde wat er mis is.',
  ],
  [
    /a snapshot holds/,
    'De casus is te groot om te kopiëren.',
    'Laat het manifest minder mappen noemen.',
  ],
  [
    /the manifest makes \d+ cases|which the manifest does not make/,
    'Het manifest maakt meer dan één casus, en er is geen of een onbekende casus gekozen.',
    'Kies bij Instellingen welke casus deze procedure uitvoert. De melding hieronder noemt de casussen die het manifest maakt.',
  ],
  [
    /the source does not load|a case does not load|the manifest makes no case/,
    'De casus in de repository laadt niet.',
    'Een casusbestand, regelwerk of cel klopt niet, of het manifest maakt geen casus. Draai regelrecht-regelhet check <manifest> op de repository: die noemt het bestand.',
  ],
  [
    /the service:|the UI spec/,
    'De casus laadt, maar de server kan hem zo niet uitvoeren.',
    'Vaak past de UI-beschrijving of het inloggen van de casus niet bij de gekozen versie. Pas de casus aan of kies een andere versie; regelrecht-regelhet check <manifest> geeft de reden.',
  ],
  [
    /ImagePullBackOff|ErrImagePull|InvalidImageName/,
    'Er is geen image voor de gekozen versie.',
    'Kies een commit waarvoor een image is gebouwd, of laat de versie leeg voor de laatste versie.',
  ],
  [
    /CreateContainerConfigError/,
    'Een Secret dat de procedure nodig heeft ontbreekt.',
    'Bij een privé-repository: maak het Secret met username en token opnieuw aan in deze namespace.',
  ],
  [
    /CrashLoopBackOff|\(Error\)/,
    'De server van de procedure stopt steeds bij het starten.',
    'De laatste regel van de server staat hieronder. Een fout in de casus vind je met regelrecht-regelhet check <manifest>.',
  ],
];

const REASONS = {
  InvalidSpec: ['Een instelling van de procedure klopt niet.', 'De melding hieronder noemt het veld. Pas het aan in de procedure.'],
  HostnameInUse: [
    'Een andere procedure gebruikt dit adres al.',
    'Kies een andere hostnaam. Het oudste object houdt het adres.',
  ],
  NameInUse: [
    'Een organisatie in deze namespace heeft dezelfde naam.',
    'Geef de procedure of de organisatie een andere naam.',
  ],
  StableNeedsProductionCerts: [
    'Stabiel inloggen werkt niet met de testcertificaten van deze installatie.',
    'Laat de beheerder van de installatie Let’s Encrypt op production zetten, of kies inloggen in testmodus.',
  ],
  ChartNotInCommit: [
    'De gekozen commit is van voor de chart waarmee RegelHet op Fundament wordt geïnstalleerd.',
    'Kies een latere commit, of laat de versie leeg voor de laatste versie.',
  ],
  ReleaseStalled: [
    'Het installeren is gestopt na herhaalde fouten.',
    'De melding van Flux staat hieronder. Een gewijzigde procedure probeert het opnieuw.',
  ],
  OrganisatieModeMismatch: [
    'Een stabiele procedure kan niet achter een organisatie in testmodus.',
    'Kies een organisatie in dezelfde modus, of zet de procedure in testmodus.',
  ],
  ExecutorNotFound: [
    'De procedure waarvoor deze partij werkt bestaat niet (meer).',
    'Controleer bij Instellingen de naam en de namespace van de procedure van de uitvoerder.',
  ],
  ExecutorInOtherProject: [
    'De procedure van de uitvoerder staat in een ander project.',
    'Een partij en haar uitvoerder horen in hetzelfde project. Maak de partij aan in het project van de uitvoerder.',
  ],
  ExecutorNotUsable: [
    'De gekozen procedure kan geen uitvoerder voor deze partij zijn.',
    'De melding hieronder zegt waarom: zij draait zelf als partij, staat in stabiele modus, of is zelf geweigerd.',
  ],
  PartyNotAsked: [
    'De casus van de uitvoerder vraagt niets van deze partij.',
    'Kies een cel waar het beleid van de uitvoerder een verzoek aan doet. De melding hieronder noemt welke dat zijn.',
  ],
  PartyTaken: [
    'Een andere procedure draait al als deze partij voor dezelfde uitvoerder.',
    'Verwijder een van de twee, of kies een andere partij.',
  ],
  SnapshotFailed: [
    'De casus kon niet uit de repository worden gekopieerd.',
    'De melding hieronder zegt waarom. Na een wijziging van de repository, ref of het pad probeert de plugin het opnieuw.',
  ],
  TokenMissing: [
    'Het Secret met het token ontbreekt.',
    'Een nieuwe kopie van de casus heeft een token nodig, en de plugin verwijdert het Secret na elke kopie. Maak het Secret opnieuw aan in deze namespace.',
  ],
};

// Reasons that carry the cause in their message, read with BY_MESSAGE.
const READ_MESSAGE = new Set(['NotStarting', 'SnapshotFailed']);

export function failureAdvice(item) {
  const ready = (item?.status?.conditions ?? []).find((c) => c.type === 'Ready');
  const reason = ready?.reason;
  // A missing token keeps the procedure pending, but waits for a person.
  if (item?.status?.phase !== 'Failed' && reason !== 'TokenMissing') return null;
  const message = item.status.message ?? ready?.message ?? '';
  if (READ_MESSAGE.has(reason)) {
    const hit = BY_MESSAGE.find(([pattern]) => pattern.test(message));
    if (hit) return { text: hit[1], action: hit[2] };
    if (reason === 'NotStarting') return null;
  }
  const known = REASONS[reason];
  return known ? { text: known[0], action: known[1] } : null;
}

// Where an app finds the services of FSC voor Burgers: the discovery
// configuration on the citizens' host (resources.go, fsc4bDiscoveryPath).
export function fsc4bDiscoveryUrl(hostname, domain) {
  return `https://${addresses(hostname, domain).burger}/fsc4b/discovery`;
}
