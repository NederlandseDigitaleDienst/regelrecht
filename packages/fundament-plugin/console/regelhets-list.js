import { loadSdk, loadNlddDesignSystem, escapeHtml, navigateToDetail, createRoute } from './_shared.js';
import { REGELHET_RESOURCE, phaseBadge } from './regelhets-body.js';

await Promise.all([loadSdk(), loadNlddDesignSystem()]);
const ctx = await fundament.init;
const content = document.getElementById('content');
document.getElementById('new-btn').addEventListener('click', () => createRoute());

// In a project the console passes its namespaces; the list is the union of
// the RegelHet instances in each. Without them (an organization page) it lists
// what the user may see on the cluster.
async function listAll() {
  const namespaces = Array.isArray(ctx.namespaces) && ctx.namespaces.length > 0 ? ctx.namespaces : [undefined];
  const lists = await Promise.all(
    namespaces.map((namespace) => fundament.k8s.list({ ...REGELHET_RESOURCE, namespace })),
  );
  return lists.flatMap((l) => l.items ?? []).sort((a, b) => a.metadata.name.localeCompare(b.metadata.name));
}

function rowHtml(item, displayNames) {
  const { name, namespace } = item.metadata;
  const badge = phaseBadge(item);
  const where = displayNames?.[namespace] ?? namespace;
  const url = item.status?.url ?? '';
  return `
    <nldd-list-item button data-name="${escapeHtml(name)}" data-namespace="${escapeHtml(namespace)}"
                    accessible-label="${escapeHtml(`${name}, ${badge.text}`)}">
      <nldd-text-cell text="${escapeHtml(name)}" supporting-text="${escapeHtml(url || where)}"></nldd-text-cell>
      <nldd-text-cell width="fit-content" text="${escapeHtml(where)}" color="secondary"></nldd-text-cell>
      <nldd-spacer-cell size="12"></nldd-spacer-cell>
      <nldd-badge text="${escapeHtml(badge.text)}" color="${badge.color}"></nldd-badge>
    </nldd-list-item>`;
}

try {
  const items = await listAll();
  if (items.length === 0) {
    content.innerHTML = `
      <nldd-banner variant="neutral" text="Er is nog geen procedure in dit project."
                   supporting-text="Met Nieuwe procedure kies je een casus, een versie en een adres."></nldd-banner>`;
  } else {
    content.innerHTML = `<nldd-list appearance="box-base" accessible-label="Procedures">${items
      .map((item) => rowHtml(item, ctx.namespaceDisplayNames))
      .join('')}</nldd-list>`;
    content.querySelectorAll('nldd-list-item').forEach((row) => {
      row.addEventListener('click', () => navigateToDetail(row.dataset.name, row.dataset.namespace));
    });
  }
} catch (err) {
  const banner = document.createElement('nldd-banner');
  banner.setAttribute('variant', 'critical');
  banner.setAttribute('text', `De procedures zijn niet te lezen: ${err?.message ?? err}`);
  content.replaceChildren(banner);
}
