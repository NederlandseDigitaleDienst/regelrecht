// Shared helpers for the RegelHet plugin pages. Plain ES module so pages can
// `import` from it. Taken from the _shared.js of Fundament's CloudNativePG
// plugin (Apache-2.0), with createRoute added; the messages are Dutch.
//
// The plugin CSP has no 'unsafe-inline', which rules out onclick= and inline
// style alike: events go through addEventListener, styling through the
// <nldd-*> components and the .plugin-* classes in plugin-sdk.css.

import { ACCENT_COLORS } from './regelhets-body.js';

// Loads the plugin SDK v1. The iframe runs on the plugin-proxy origin, which
// also serves the SDK, so these bare paths satisfy script-src 'self'. The /v1/
// segment tracks protocolVersion: a breaking change ships as /v2/.
export function loadSdk() {
  const link = document.createElement('link');
  link.rel = 'stylesheet';
  link.href = '/plugins/sdk/v1/plugin-sdk.css';
  document.head.appendChild(link);

  return new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = '/plugins/sdk/v1/plugin-sdk.js';
    script.onload = () => resolve(window.fundament);
    script.onerror = () => reject(new Error('failed to load plugin-sdk.js'));
    document.head.appendChild(script);
  });
}

export function escapeHtml(value) {
  if (value === null || value === undefined) return '';
  return String(value)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#39;');
}

// Posts a navigate message to the parent, which resolves it relative to the
// iframe's current route, so this sends only the resource identity and works
// from a create view too. parentOrigin falls back to '*' before init.
export function navigateToDetail(name, namespace) {
  window.parent.postMessage(
    { type: 'plugin:navigate', name, namespace },
    window.fundament?.parentOrigin ?? '*',
  );
}

// Opens the create route of this kind, from a list page.
export function createRoute() {
  window.parent.postMessage({ type: 'plugin:create' }, window.fundament?.parentOrigin ?? '*');
}

// Returns to the resource-kind list. Only meaningful from a create or detail
// view; the host ignores it on a list.
export function navigateBack() {
  window.parent.postMessage(
    { type: 'plugin:navigate-back' },
    window.fundament?.parentOrigin ?? '*',
  );
}

// Renders [key, value] pairs as an nldd-list, label left and value right: the
// same rows the Console's generated detail page draws. Both halves are escaped
// here, so pass raw values: pre-escaped ones render as "&amp;".
export function renderKeyValueList(pairs, accessibleLabel) {
  const rows = pairs
    .map(
      ([k, v]) => `
        <nldd-list-item>
          <nldd-text-cell text="${escapeHtml(k)}"></nldd-text-cell>
          <nldd-spacer-cell size="12"></nldd-spacer-cell>
          <nldd-text-cell width="fit-content" horizontal-alignment="right" text="${escapeHtml(v)}"></nldd-text-cell>
        </nldd-list-item>`,
    )
    .join('');
  return `<nldd-list appearance="simple" accessible-label="${escapeHtml(accessibleLabel)}">${rows}</nldd-list>`;
}

// Loads the NLDD Design System bundle the Console serves next to the SDK, which
// registers every <nldd-*> element. Resolves once both the stylesheet and the
// script are in, so no component renders unstyled.
export function loadNlddDesignSystem() {
  // The design system reads light/dark from :root[data-scheme]; mirror the
  // .light/.dark class the SDK sets on <body> on init and on every theme change.
  const syncScheme = () => {
    const dark = document.body.classList.contains('dark');
    document.documentElement.setAttribute('data-scheme', dark ? 'dark' : 'light');
  };
  syncScheme();
  new MutationObserver(syncScheme).observe(document.body, {
    attributes: true,
    attributeFilter: ['class'],
  });

  const settled = (el, what) =>
    new Promise((resolve, reject) => {
      el.addEventListener('load', () => resolve(), { once: true });
      el.addEventListener('error', () => reject(new Error(`failed to load ${what}`)), { once: true });
    });

  const link = document.createElement('link');
  link.rel = 'stylesheet';
  link.href = '/plugins/sdk/v1/nldd-design-system.css';
  const css = settled(link, 'nldd-design-system.css');
  document.head.appendChild(link);

  const script = document.createElement('script');
  script.src = '/plugins/sdk/v1/nldd-design-system.js';
  const js = settled(script, 'nldd-design-system.js');
  document.head.appendChild(script);

  return Promise.all([css, js]);
}

// The element that carries `invalid` for a field: the <nldd-dropdown> around a
// <select>, or the control itself.
function fieldControl(el) {
  return el.closest('nldd-dropdown') ?? el;
}

// Shows message in the field's nldd-validation-item, the one rule-less item in
// its nldd-validation-list, and marks the field invalid.
function setFieldError(el, message) {
  const control = fieldControl(el);
  const item = control.closest('nldd-form-field')?.querySelector('nldd-validation-item');
  if (item) {
    item.textContent = message;
    control.setAttribute('unmet', item.id);
  }
  control.setAttribute('invalid', '');
}

function clearFieldError(el) {
  const control = fieldControl(el);
  control.removeAttribute('invalid');
  control.removeAttribute('unmet');
}

// Wires the shared submit flow. checks are [control, check] pairs, where check
// returns an error message or null; each failure shows under its own field and
// the first failing field takes focus. A failing action shows in the critical
// errorBanner. On success the button keeps loading: the action navigates away.
export function wireSubmit(form, { button, errorBanner, failPrefix, checks, action }) {
  // A field is judged on submit, and stops being wrong the moment it is edited.
  for (const [control] of checks) {
    control.addEventListener('input', () => clearFieldError(control));
    control.addEventListener('change', () => clearFieldError(control));
  }

  let busy = false;
  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    // Enter in a text field submits too, so a click is not the only way in.
    if (busy) return;
    errorBanner.hidden = true;

    // The checks run inside the try: preventDefault() already suppressed the
    // native submit, so a check that throws would otherwise leave the form
    // silently dead, with no banner and no field error.
    try {
      let firstInvalid = null;
      for (const [control, check] of checks) {
        // A field the design system has not upgraded yet has no value.
        const message = check(String(control.value ?? '').trim());
        if (message) {
          setFieldError(control, message);
          firstInvalid ??= control;
        } else {
          clearFieldError(control);
        }
      }
      if (firstInvalid) {
        firstInvalid.focus();
        return;
      }

      busy = true;
      button.loading = true;
      await action();
    } catch (err) {
      errorBanner.setAttribute('text', `${failPrefix}: ${err?.message ?? err}`);
      errorBanner.hidden = false;
      busy = false;
      button.loading = false;
    }
  });
}

// A dropdown of the accent colours of the NLDD design system, empty for the
// screens' default, donkerblauw (frontend-regelhet/src/gedeeld/uiterlijk.ts).
export function accentDropdown(name, chosen) {
  const options = [optionHtml('', 'Standaard (donkerblauw)', !chosen)]
    .concat(ACCENT_COLORS.map((c) => optionHtml(c, c, c === chosen)))
    .join('');
  return `<nldd-dropdown><select name="${name}">${options}</select></nldd-dropdown>`;
}

// The building blocks of a wizard step. Values are escaped here.
export function field(label, control, { errorId, hint } = {}) {
  const errors = errorId
    ? `<nldd-validation-list><nldd-validation-item id="${errorId}"></nldd-validation-item></nldd-validation-list>`
    : '';
  const help = hint ? `<nldd-form-field-help-text>${escapeHtml(hint)}</nldd-form-field-help-text>` : '';
  return `<nldd-form-field label="${escapeHtml(label)}">${control}${errors}${help}</nldd-form-field>`;
}

export function textField(name, value, extra = '') {
  return `<nldd-text-field name="${name}" value="${escapeHtml(value)}" no-spellcheck ${extra}></nldd-text-field>`;
}

export function radios(name, label, options, chosen) {
  const fields = options
    .map(
      ([value, text]) =>
        `<nldd-radio-button-field value="${escapeHtml(value)}" label="${escapeHtml(text)}"${value === chosen ? ' checked' : ''}></nldd-radio-button-field>`,
    )
    .join('');
  return `<nldd-radio-button-group name="${name}" accessible-label="${escapeHtml(label)}">${fields}</nldd-radio-button-group>`;
}

export function optionHtml(value, text, selected) {
  return `<option value="${escapeHtml(value)}"${selected ? ' selected' : ''}>${escapeHtml(text)}</option>`;
}

export function chosenRadio(form, name) {
  const group = form.querySelector(`nldd-radio-button-group[name="${name}"]`);
  const checked = [...(group?.querySelectorAll('nldd-radio-button-field') ?? [])].find((f) => f.checked);
  return checked?.value ?? '';
}

// A dropdown of the namespaces of the project, or a text field outside one.
export function namespaceControl(ctx, chosen) {
  const namespaces = Array.isArray(ctx.namespaces) ? ctx.namespaces : [];
  if (!namespaces.length) return textField('namespace', chosen, 'required maxlength="63"');
  return `<nldd-dropdown><select name="namespace">${namespaces
    .map((n) => optionHtml(n, ctx.namespaceDisplayNames?.[n] ?? n, n === chosen))
    .join('')}</select></nldd-dropdown>`;
}

// Runs a wizard in content, with its steps in stepBar: one step at a time,
// each a form of its own whose submit is "Volgende", and on the last step
// finish(). A step has a title, an intro, fields() returning its HTML, an
// optional wire(form), checks(form) returning [control, check] pairs as
// wireSubmit wants them, and keep(value, form), which stores what was
// entered. Steps with skip() returning true are passed over.
export function runWizard({ content, stepBar, steps, finishText, failPrefix, finish }) {
  let current = 0;
  const visible = () => steps.filter((s) => !s.skip?.());

  function renderStepBar(shown, step) {
    const at = shown.indexOf(step);
    stepBar.innerHTML = shown
      .map((s, i) => {
        const status = i < at ? 'past' : i === at ? 'current' : 'future';
        return `<nldd-step-bar-item text="${escapeHtml(s.title)}" status="${status}"></nldd-step-bar-item>`;
      })
      .join('');
  }

  function render() {
    const shown = visible();
    const step = shown[current];
    const last = current === shown.length - 1;
    renderStepBar(shown, step);
    content.innerHTML = `
      <nldd-title size="4" heading-level="2" text="${escapeHtml(step.title)}" supporting-text="${escapeHtml(step.intro)}"></nldd-title>
      <nldd-spacer size="16"></nldd-spacer>
      <nldd-form>
        <form id="step-form" novalidate>
          <nldd-banner id="error-banner" variant="critical" hidden></nldd-banner>
          ${step.fields()}
          <nldd-form-actions>
            <nldd-button-group orientation="horizontal">
              <nldd-button id="next-btn" type="submit" appearance="primary" text="${escapeHtml(last ? finishText : 'Volgende')}"></nldd-button>
              <nldd-button id="back-btn" type="button" appearance="secondary" text="${current === 0 ? 'Annuleren' : 'Vorige'}"></nldd-button>
            </nldd-button-group>
          </nldd-form-actions>
        </form>
      </nldd-form>`;
    const form = content.querySelector('#step-form');
    const value = (name) => String(form.querySelector(`[name="${name}"]`)?.value ?? '').trim();
    step.wire?.(form);
    content.querySelector('#back-btn').addEventListener('click', () => {
      if (current === 0) {
        navigateBack();
        return;
      }
      current -= 1;
      render();
    });
    wireSubmit(form, {
      button: content.querySelector('#next-btn'),
      errorBanner: content.querySelector('#error-banner'),
      failPrefix: last ? failPrefix : 'Dat lukte niet',
      checks: step.checks(form),
      action: async () => {
        step.keep(value, form);
        if (last) {
          await finish();
          return;
        }
        // keep() may change which steps show; stay on the step after this one.
        current = visible().indexOf(step) + 1;
        render();
      },
    });
  }

  render();
}

// One row of an nldd-list with a label and a link that opens in a new tab;
// nothing without a URL.
export function linkRow(label, url) {
  if (!url) return '';
  return `
    <nldd-list-item>
      <nldd-text-cell text="${escapeHtml(label)}"></nldd-text-cell>
      <nldd-spacer-cell size="12"></nldd-spacer-cell>
      <nldd-link href="${escapeHtml(url)}" target="_blank" text="${escapeHtml(url)}"></nldd-link>
    </nldd-list-item>`;
}

// The conditions of a status as [type, text] pairs for renderKeyValueList.
export function conditionPairs(item) {
  return (item.status?.conditions ?? []).map((c) => [
    c.type,
    `${c.status === 'True' ? 'ja' : 'nee'}${c.message ? `: ${c.message}` : ''}`,
  ]);
}

// The section that deletes the object of a detail page once its name is
// typed: its explanation, the form, and the delete itself.
export function deleteSectionHtml(name, explanation, buttonText) {
  return `
    <nldd-title size="5" heading-level="2" text="Verwijderen"></nldd-title>
    <nldd-rich-text><p>${escapeHtml(explanation)}</p></nldd-rich-text>
    <nldd-spacer size="12"></nldd-spacer>
    <nldd-form>
      <form id="delete-form" novalidate>
        <nldd-banner id="delete-error" variant="critical" hidden></nldd-banner>
        <nldd-form-field label="Typ **${escapeHtml(name)}** om te bevestigen">
          <nldd-text-field id="confirm" name="confirm" no-spellcheck></nldd-text-field>
        </nldd-form-field>
        <nldd-form-actions>
          <nldd-button id="delete-btn" type="submit" appearance="destructive" text="${escapeHtml(buttonText)}"></nldd-button>
        </nldd-form-actions>
      </form>
    </nldd-form>`;
}

// Wires the form of deleteSectionHtml to delete { ...resource, name, namespace }.
export function wireDelete(root, { resource, name, namespace }) {
  const form = root.querySelector('#delete-form');
  const banner = root.querySelector('#delete-error');
  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    banner.hidden = true;
    if (String(root.querySelector('#confirm').value ?? '').trim() !== name) {
      banner.setAttribute('text', `Typ ${name} om het verwijderen te bevestigen.`);
      banner.hidden = false;
      return;
    }
    const button = root.querySelector('#delete-btn');
    button.loading = true;
    try {
      await fundament.k8s.delete({ ...resource, name, namespace });
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
