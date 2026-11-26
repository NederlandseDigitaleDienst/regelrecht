// What the console pages build and check for an Organisatie, apart from the
// DOM, so node:test can run it (console-test/). The hostname follows the rules
// of a RegelHet, so those checks come from regelhets-body.js.
import { addresses, nameFromHostname } from './regelhets-body.js';

export const ORGANISATIE_RESOURCE = {
  group: 'regelhet.regelrecht.nl',
  version: 'v1alpha1',
  resource: 'organisaties',
};

// The three answers to "does this RegelHet belong to an organisatie?" in the
// wizard of a RegelHet.
export const ORGANISATIE_NONE = 'none';
export const ORGANISATIE_NEW = 'new';
export const ORGANISATIE_EXISTING = 'existing';

const DISPLAY_NAME_MAX = 100;

export function displayNameError(value) {
  if (!value) return 'Vul de naam in die mensen zien, zoals Gemeente Wilgendaal.';
  return value.length > DISPLAY_NAME_MAX ? `Ten hoogste ${DISPLAY_NAME_MAX} tekens.` : null;
}

export function logoError(value) {
  if (!value) return null;
  let url;
  try {
    url = new URL(value);
  } catch {
    return 'Een adres dat met https:// begint, of leeg.';
  }
  if (url.protocol !== 'https:' || !url.host || url.username || url.password || /\s/.test(value)) {
    return 'Een adres dat met https:// begint, zonder inloggegevens, of leeg.';
  }
  return null;
}

// The Organisatie object for the choices of a wizard.
export function buildOrganisatie({ namespace, hostname, displayName, logo, accentColor }) {
  const spec = { displayName, hostname };
  if (logo) spec.logo = logo;
  if (accentColor) spec.theme = { accentColor };
  return {
    apiVersion: `${ORGANISATIE_RESOURCE.group}/${ORGANISATIE_RESOURCE.version}`,
    kind: 'Organisatie',
    metadata: { name: nameFromHostname(hostname), namespace },
    spec,
  };
}

// The value of an organisatie in a dropdown, and back: <namespace>/<name>.
export function organisatieKey(item) {
  return `${item.metadata.namespace}/${item.metadata.name}`;
}

export function parseOrganisatieKey(key) {
  const [namespace, name] = String(key).split('/');
  return namespace && name ? { namespace, name } : null;
}

// What spec.organisatie of a RegelHet in `namespace` says for a choice of
// the wizard: nothing for none, the new one by name, or the chosen one, with
// its namespace only when it is another than the regelhet's own.
export function organisatieReference({ choice, existing, newHostname }, namespace) {
  if (choice === ORGANISATIE_NEW) return { name: nameFromHostname(newHostname) };
  if (choice !== ORGANISATIE_EXISTING) return undefined;
  const ref = parseOrganisatieKey(existing);
  if (!ref) return undefined;
  return ref.namespace === namespace ? { name: ref.name } : ref;
}

// Whether two hostnames give a shared address, which the plugin would refuse
// (resources.go, hosts.overlaps).
export function hostnamesClash(a, b) {
  if (!a || !b) return false;
  const left = Object.values(addresses(a)).slice(0, 2);
  const right = Object.values(addresses(b)).slice(0, 2);
  return left.some((x) => right.includes(x));
}

// The organisaties a user may pick: those in the namespaces of the project,
// sorted by display name.
export function sortOrganisaties(items) {
  return [...items].sort((a, b) =>
    (a.spec?.displayName ?? a.metadata.name).localeCompare(b.spec?.displayName ?? b.metadata.name),
  );
}
