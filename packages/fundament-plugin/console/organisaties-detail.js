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
import { phaseBadge } from './regelhets-body.js';
import { ORGANISATIE_RESOURCE } from './organisaties-body.js';

await Promise.all([loadSdk(), loadNlddDesignSystem()]);
const ctx = await fundament.init;
const content = document.getElementById('content');
const { name, namespace } = ctx.resource ?? {};

// The RegelHet instances on the list of the front, as the plugin last saw them.
function processenHtml(processen) {
  if (!processen.length) {
    return `<nldd-banner variant="neutral" size="sm" text="Nog geen procedure staat achter deze organisatie."
              supporting-text="Kies haar in de stap Organisatie van een nieuwe procedure."></nldd-banner>`;
  }
  const rows = processen
    .map((p) => {
      const badge = phaseBadge({ status: { phase: p.phase } });
      const where = ctx.namespaceDisplayNames?.[p.namespace] ?? p.namespace;
      return `
        <nldd-list-item accessible-label="${escapeHtml(`${p.name}, ${badge.text}`)}">
          <nldd-text-cell text="${escapeHtml(p.name)}" supporting-text="${escapeHtml(where)}"></nldd-text-cell>
          <nldd-spacer-cell size="12"></nldd-spacer-cell>
          <nldd-badge text="${escapeHtml(badge.text)}" color="${badge.color}"></nldd-badge>
        </nldd-list-item>`;
    })
    .join('');
  return `<nldd-list appearance="simple" accessible-label="Procedures">${rows}</nldd-list>`;
}

function show(item) {
  const badge = phaseBadge(item);
  const spec = item.spec ?? {};
  const conditions = conditionPairs(item);
  content.innerHTML = `
    <nldd-title size="2" heading-level="1" text="${escapeHtml(spec.displayName || name)}"
                supporting-text="${escapeHtml(`${name}, ${ctx.namespaceDisplayNames?.[namespace] ?? namespace}`)}">
      <nldd-badge slot="end" text="${escapeHtml(badge.text)}" color="${badge.color}"></nldd-badge>
    </nldd-title>
    ${item.status?.message ? `<nldd-spacer size="12"></nldd-spacer><nldd-banner variant="${badge.color === 'success' ? 'neutral' : badge.color}" size="sm" text="${escapeHtml(item.status.message)}"></nldd-banner>` : ''}
    <nldd-spacer size="16"></nldd-spacer>
    <nldd-list appearance="simple" accessible-label="Adressen">
      ${linkRow('Burgers', item.status?.url)}
      ${linkRow('Medewerkers', item.status?.backofficeUrl)}
      ${linkRow('Inloggen (Keycloak)', item.status?.loginUrl)}
    </nldd-list>
    <nldd-spacer size="16"></nldd-spacer>
    <nldd-title size="5" heading-level="2" text="Procedures"></nldd-title>
    ${processenHtml(item.status?.processen ?? [])}
    <nldd-spacer size="16"></nldd-spacer>
    <nldd-title size="5" heading-level="2" text="Instellingen"></nldd-title>
    ${renderKeyValueList(
      [
        ['Naam', spec.displayName ?? ''],
        ['Logo', spec.logo || 'geen'],
        ['Accentkleur', spec.theme?.accentColor || 'standaard (donkerblauw)'],
        ['Versie', spec.version || 'laatste build van main'],
        ['Inloggen', 'testomgeving'],
        ['Hostnaam', spec.hostname ?? ''],
      ],
      'Instellingen',
    )}
    ${conditions.length ? `<nldd-spacer size="16"></nldd-spacer><nldd-title size="5" heading-level="2" text="Toestand"></nldd-title>${renderKeyValueList(conditions, 'Toestand')}` : ''}
    <nldd-spacer size="24"></nldd-spacer>
    ${deleteSectionHtml(
      name,
      'Verwijderen haalt de voorkant weg. De procedures erachter blijven draaien op hun eigen adressen, met hun data; ze staan daarna niet meer achter een organisatie.',
      'Organisatie verwijderen',
    )}`;
  wireDelete(content, { resource: ORGANISATIE_RESOURCE, name, namespace });
}

try {
  show(await fundament.k8s.get({ ...ORGANISATIE_RESOURCE, name, namespace }));
} catch (err) {
  const banner = document.createElement('nldd-banner');
  banner.setAttribute('variant', 'critical');
  banner.setAttribute('text', `Deze organisatie is niet te lezen: ${err?.message ?? err}`);
  content.replaceChildren(banner);
}
