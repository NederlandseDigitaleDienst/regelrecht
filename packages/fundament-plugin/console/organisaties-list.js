import { loadSdk, loadNlddDesignSystem, escapeHtml, navigateToDetail, createRoute } from './_shared.js';
import { phaseBadge } from './wetsprocessen-body.js';
import { ORGANISATIE_RESOURCE, sortOrganisaties } from './organisaties-body.js';

await Promise.all([loadSdk(), loadNlddDesignSystem()]);
const ctx = await fundament.init;
const content = document.getElementById('content');
document.getElementById('new-btn').addEventListener('click', () => createRoute());

// As on the list of wetsprocessen: the union over the namespaces of the
// project, or what the user may see on the cluster outside one.
async function listAll() {
  const namespaces = Array.isArray(ctx.namespaces) && ctx.namespaces.length > 0 ? ctx.namespaces : [undefined];
  const lists = await Promise.all(
    namespaces.map((namespace) => fundament.k8s.list({ ...ORGANISATIE_RESOURCE, namespace })),
  );
  return sortOrganisaties(lists.flatMap((l) => l.items ?? []));
}

function rowHtml(item, displayNames) {
  const { name, namespace } = item.metadata;
  const badge = phaseBadge(item);
  const where = displayNames?.[namespace] ?? namespace;
  const title = item.spec?.displayName || name;
  const count = item.status?.processen?.length ?? 0;
  const processen = count === 1 ? '1 procedure' : `${count} procedures`;
  return `
    <nldd-list-item button data-name="${escapeHtml(name)}" data-namespace="${escapeHtml(namespace)}"
                    accessible-label="${escapeHtml(`${title}, ${badge.text}`)}">
      <nldd-text-cell text="${escapeHtml(title)}" supporting-text="${escapeHtml(item.status?.url || where)}"></nldd-text-cell>
      <nldd-text-cell width="fit-content" text="${escapeHtml(processen)}" color="secondary"></nldd-text-cell>
      <nldd-spacer-cell size="12"></nldd-spacer-cell>
      <nldd-badge text="${escapeHtml(badge.text)}" color="${badge.color}"></nldd-badge>
    </nldd-list-item>`;
}

try {
  const items = await listAll();
  if (items.length === 0) {
    content.innerHTML = `
      <nldd-banner variant="neutral" text="Er is nog geen organisatie in dit project."
                   supporting-text="Met Nieuwe organisatie kies je een naam, een logo en een adres. Een procedure kiest daarna haar organisatie."></nldd-banner>`;
  } else {
    content.innerHTML = `<nldd-list appearance="box-base" accessible-label="Organisaties">${items
      .map((item) => rowHtml(item, ctx.namespaceDisplayNames))
      .join('')}</nldd-list>`;
    content.querySelectorAll('nldd-list-item').forEach((row) => {
      row.addEventListener('click', () => navigateToDetail(row.dataset.name, row.dataset.namespace));
    });
  }
} catch (err) {
  const banner = document.createElement('nldd-banner');
  banner.setAttribute('variant', 'critical');
  banner.setAttribute('text', `De organisaties zijn niet te lezen: ${err?.message ?? err}`);
  content.replaceChildren(banner);
}
