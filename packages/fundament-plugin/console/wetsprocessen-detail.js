import { loadSdk, loadNlddDesignSystem, escapeHtml, renderKeyValueList, navigateBack } from './_shared.js';
import { WETSPROCES_RESOURCE, phaseBadge } from './wetsprocessen-body.js';

await Promise.all([loadSdk(), loadNlddDesignSystem()]);
const ctx = await fundament.init;
const content = document.getElementById('content');
const { name, namespace } = ctx.resource ?? {};

function linkRow(label, url) {
  if (!url) return '';
  return `
    <nldd-list-item>
      <nldd-text-cell text="${escapeHtml(label)}"></nldd-text-cell>
      <nldd-spacer-cell size="12"></nldd-spacer-cell>
      <nldd-link href="${escapeHtml(url)}" target="_blank" text="${escapeHtml(url)}"></nldd-link>
    </nldd-list-item>`;
}

function show(item) {
  const badge = phaseBadge(item);
  const spec = item.spec ?? {};
  const manifest = spec.manifest ?? {};
  const conditions = (item.status?.conditions ?? []).map((c) => [
    c.type,
    `${c.status === 'True' ? 'ja' : 'nee'}${c.message ? `: ${c.message}` : ''}`,
  ]);
  content.innerHTML = `
    <nldd-title size="2" heading-level="1" text="${escapeHtml(name)}"
                supporting-text="${escapeHtml(ctx.namespaceDisplayNames?.[namespace] ?? namespace)}">
      <nldd-badge slot="end" text="${escapeHtml(badge.text)}" color="${badge.color}"></nldd-badge>
    </nldd-title>
    ${item.status?.message ? `<nldd-spacer size="12"></nldd-spacer><nldd-banner variant="${badge.color === 'success' ? 'neutral' : badge.color}" size="sm" text="${escapeHtml(item.status.message)}"></nldd-banner>` : ''}
    <nldd-spacer size="16"></nldd-spacer>
    <nldd-list appearance="simple" accessible-label="Adressen">
      ${linkRow('Burgers', item.status?.url)}
      ${linkRow('Medewerkers', item.status?.backofficeUrl)}
    </nldd-list>
    <nldd-spacer size="16"></nldd-spacer>
    <nldd-title size="5" heading-level="2" text="Instellingen"></nldd-title>
    ${renderKeyValueList(
      [
        ['Repository', manifest.repoUrl ?? ''],
        ['Branch of tag', manifest.ref ?? 'main'],
        ['Manifest', manifest.path ?? ''],
        ['Toegang', manifest.credentialsSecretRef ? `privé, met Secret ${manifest.credentialsSecretRef.name}` : 'openbaar'],
        ['Versie', spec.version || 'laatste build van main'],
        ['Accentkleur', spec.theme?.accentColor || 'standaard'],
        ['Hostnaam', spec.hostname ?? ''],
      ],
      'Instellingen',
    )}
    ${conditions.length ? `<nldd-spacer size="16"></nldd-spacer><nldd-title size="5" heading-level="2" text="Toestand"></nldd-title>${renderKeyValueList(conditions, 'Toestand')}` : ''}
    <nldd-spacer size="24"></nldd-spacer>
    <nldd-title size="5" heading-level="2" text="Verwijderen"></nldd-title>
    <nldd-rich-text>
      <p>Verwijderen haalt het wetsproces weg met alles wat erbij hoort: de schermen, de database met alle
      kronieken en het volume eronder. Dat is niet terug te draaien, en er is geen back-up.</p>
    </nldd-rich-text>
    <nldd-spacer size="12"></nldd-spacer>
    <nldd-form>
      <form id="delete-form" novalidate>
        <nldd-banner id="delete-error" variant="critical" hidden></nldd-banner>
        <nldd-form-field label="Typ **${escapeHtml(name)}** om te bevestigen">
          <nldd-text-field id="confirm" name="confirm" no-spellcheck></nldd-text-field>
        </nldd-form-field>
        <nldd-form-actions>
          <nldd-button id="delete-btn" type="submit" appearance="destructive" text="Wetsproces en data verwijderen"></nldd-button>
        </nldd-form-actions>
      </form>
    </nldd-form>`;

  const form = document.getElementById('delete-form');
  const banner = document.getElementById('delete-error');
  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    banner.hidden = true;
    if (String(document.getElementById('confirm').value ?? '').trim() !== name) {
      banner.setAttribute('text', `Typ ${name} om het verwijderen te bevestigen.`);
      banner.hidden = false;
      return;
    }
    const button = document.getElementById('delete-btn');
    button.loading = true;
    try {
      await fundament.k8s.delete({ ...WETSPROCES_RESOURCE, name, namespace });
      navigateBack();
    } catch (err) {
      banner.setAttribute(
        'text',
        err?.code === 'forbidden'
          ? 'Verwijderen mag alleen een beheerder van het project.'
          : `Verwijderen lukte niet: ${err?.message ?? err}`,
      );
      banner.hidden = false;
      button.loading = false;
    }
  });
}

try {
  show(await fundament.k8s.get({ ...WETSPROCES_RESOURCE, name, namespace }));
} catch (err) {
  const banner = document.createElement('nldd-banner');
  banner.setAttribute('variant', 'critical');
  banner.setAttribute('text', `Dit wetsproces is niet te lezen: ${err?.message ?? err}`);
  content.replaceChildren(banner);
}
