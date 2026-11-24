import {
  loadSdk,
  loadNlddDesignSystem,
  escapeHtml,
  renderKeyValueList,
  linkRow,
  conditionPairs,
  deleteSectionHtml,
  wireDelete,
} from './_shared.js';
import { REGELHET_RESOURCE, phaseBadge, partijText } from './regelhets-body.js';

await Promise.all([loadSdk(), loadNlddDesignSystem()]);
const ctx = await fundament.init;
const content = document.getElementById('content');
const { name, namespace } = ctx.resource ?? {};

// Where the passwords of stable mode are: the name of the Secret and its
// keys, and how to read one, never a value. The page cannot read the Secret
// and does not try.
function keycloakHtml(item) {
  const secret = item.status.keycloakSecret;
  const accounts = item.spec?.auth?.testAccounts
    ? `<li>de testaccounts van de casus: sleutel <code>test-password</code>, één wachtwoord voor allemaal</li>`
    : '';
  return `
    <nldd-spacer size="12"></nldd-spacer>
    <nldd-rich-text>
      <p>De wachtwoorden staan in het Secret <code>${escapeHtml(secret)}</code> in deze namespace. Het cluster heeft ze zelf gemaakt; ze staan nergens anders.</p>
      <ul>
        <li>de beheerder van Keycloak, gebruiker <code>admin</code>: sleutel <code>admin-password</code></li>
        ${accounts}
      </ul>
      <p>Lezen kan wie in deze namespace Secrets mag lezen, bijvoorbeeld met
      <code>kubectl get secret ${escapeHtml(secret)} -o jsonpath='{.data.admin-password}' | base64 -d</code>.</p>
    </nldd-rich-text>`;
}

// Whether the token Secret is still there. The pods never need it: only a
// new copy of the casus does, after a change of the repository, branch or
// manifest.
function tokenText(state) {
  if (state === 'Present') return 'aanwezig';
  if (state === 'Absent') return 'verwijderd; een nieuwe versie van de casus vraagt een nieuw Secret met een token';
  return 'onbekend';
}

function show(item) {
  const badge = phaseBadge(item);
  const spec = item.spec ?? {};
  const manifest = spec.manifest ?? {};
  const conditions = conditionPairs(item);
  content.innerHTML = `
    <nldd-title size="2" heading-level="1" text="${escapeHtml(name)}"
                supporting-text="${escapeHtml(ctx.namespaceDisplayNames?.[namespace] ?? namespace)}">
      <nldd-badge slot="end" text="${escapeHtml(badge.text)}" color="${badge.color}"></nldd-badge>
    </nldd-title>
    ${item.status?.message ? `<nldd-spacer size="12"></nldd-spacer><nldd-banner variant="${badge.color === 'success' ? 'neutral' : badge.color}" size="sm" text="${escapeHtml(item.status.message)}"></nldd-banner>` : ''}
    <nldd-spacer size="16"></nldd-spacer>
    <nldd-list appearance="simple" accessible-label="Adressen">
      ${linkRow('Burgers', item.status?.url)}
      ${linkRow(spec.partij ? 'Medewerkers van de partij' : 'Medewerkers', item.status?.backofficeUrl)}
      ${linkRow('Inloggen (Keycloak)', item.status?.loginUrl)}
      ${linkRow('FSC voor Burgers (discovery)', item.status?.fsc4bDiscoveryUrl)}
      ${(item.status?.parties ?? []).map((p) => linkRow(p.name || p.cell, p.url)).join('')}
    </nldd-list>
    ${item.status?.keycloakSecret ? keycloakHtml(item) : ''}
    <nldd-spacer size="16"></nldd-spacer>
    <nldd-title size="5" heading-level="2" text="Instellingen"></nldd-title>
    ${renderKeyValueList(
      [
        ['Repository', manifest.repoUrl ?? ''],
        ['Branch of tag', manifest.ref ?? 'main'],
        ['Manifest', manifest.path ?? ''],
        ['Casus', spec.casus || 'de enige casus van het manifest'],
        ['Toegang', manifest.credentialsSecretRef ? `privé, met Secret ${manifest.credentialsSecretRef.name}` : 'openbaar'],
        ['Kopie van de casus', item.status?.snapshot?.commit ? `commit ${item.status.snapshot.commit}` : 'nog niet gemaakt'],
        ...(manifest.credentialsSecretRef ? [['Token', tokenText(item.status?.token)]] : []),
        ['Versie', spec.version || 'laatste build van main'],
        ['Titel', spec.title || 'uit het casusbestand'],
        ['Accentkleur', spec.theme?.accentColor || 'standaard'],
        [
          'Inloggen',
          spec.auth?.mode === 'stable'
            ? `stabiel, via Keycloak${spec.auth.testAccounts ? ', met testaccounts' : ''}`
            : 'testomgeving',
        ],
        ...(spec.partij
          ? [['Partij van', partijStatusText(spec.partij, item)]]
          : [
              ['FSC voor Burgers', spec.fsc4b?.enabled ? 'aan, met het demo-stelsel' : 'uit'],
              ['Externe partijen', partiesText(item)],
              ['Organisatie', organisatieText(spec.organisatie, item)],
            ]),
        ['Hostnaam', spec.hostname ?? ''],
      ],
      'Instellingen',
    )}
    ${conditions.length ? `<nldd-spacer size="16"></nldd-spacer><nldd-title size="5" heading-level="2" text="Toestand"></nldd-title>${renderKeyValueList(conditions, 'Toestand')}` : ''}
    <nldd-spacer size="24"></nldd-spacer>
    ${deleteSectionHtml(
      name,
      'Verwijderen haalt de procedure weg met alles wat erbij hoort: de schermen, de database met alle kronieken en het volume eronder. Dat is niet terug te draaien, en er is geen back-up.',
      'Procedure en data verwijderen',
    )}`;
  wireDelete(content, { resource: REGELHET_RESOURCE, name, namespace });
}

// The executor of spec.partij, and whether the plugin linked the procedure
// with it (condition PartiesLinked).
function partijStatusText(partij, item) {
  const where = partijText(partij, namespace);
  const linked = (item.status?.conditions ?? []).find((c) => c.type === 'PartiesLinked');
  if (!linked) return where;
  return linked.status === 'True' ? `${where}, gekoppeld` : `${where}, niet gekoppeld: ${linked.message}`;
}

// The parties of an executor: those that run as a procedure of their own,
// and those its case asks something of that are entered in the admin panel.
function partiesText(item) {
  const linked = item.status?.parties ?? [];
  const own = new Set(linked.map((p) => p.cell));
  const rest = (item.status?.askedParties ?? []).filter((a) => !own.has(a.cell)).map((a) => a.name || a.cell);
  const parts = linked.map((p) => {
    const where = p.procedure ? ` (procedure ${p.procedure.namespace}/${p.procedure.name})` : '';
    return `${p.name || p.cell} als eigen procedure${where}`;
  });
  if (rest.length) parts.push(`${rest.join(', ')} via het beheerpaneel`);
  return parts.length ? parts.join('; ') : 'geen, of nog niet bekend';
}

// The organisatie of spec.organisatie, and whether the plugin put the
// RegelHet on its list (condition OrganisatieGevonden).
function organisatieText(ref, item) {
  if (!ref) return 'geen, losse procedure';
  const where = `${ref.namespace || namespace}/${ref.name}`;
  const found = (item.status?.conditions ?? []).find((c) => c.type === 'OrganisatieGevonden');
  if (!found) return where;
  return found.status === 'True' ? `${where}, aangemeld` : `${where}, niet aangemeld: ${found.message}`;
}

try {
  show(await fundament.k8s.get({ ...REGELHET_RESOURCE, name, namespace }));
} catch (err) {
  const banner = document.createElement('nldd-banner');
  banner.setAttribute('variant', 'critical');
  banner.setAttribute('text', `Deze procedure is niet te lezen: ${err?.message ?? err}`);
  content.replaceChildren(banner);
}
