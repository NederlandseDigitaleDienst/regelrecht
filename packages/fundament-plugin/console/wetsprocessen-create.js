// The wizard: one step at a time, each a form of its own whose submit is
// "Volgende". The last step shows what will be made and makes it: the
// Wetsproces, and for a private repository the Secret with its token, owned
// by the Wetsproces so both go together.
import { loadSdk, loadNlddDesignSystem, escapeHtml, navigateToDetail, navigateBack, wireSubmit, renderKeyValueList } from './_shared.js';
import {
  ACCENT_COLORS,
  DEFAULT_PATH,
  DEFAULT_REF,
  SECRET_RESOURCE,
  WETSPROCES_RESOURCE,
  addresses,
  buildSecret,
  buildWetsproces,
  commitError,
  hostnameError,
  namespaceError,
  pathError,
  refError,
  repoUrlError,
  secretName,
  tokenError,
  usernameError,
  nameFromHostname,
  keycloakSecretName,
  fsc4bDiscoveryUrl,
} from './wetsprocessen-body.js';

await Promise.all([loadSdk(), loadNlddDesignSystem()]);
const ctx = await fundament.init;

const content = document.getElementById('content');
const stepBar = document.getElementById('steps');

const state = {
  repoUrl: '',
  ref: DEFAULT_REF,
  path: DEFAULT_PATH,
  privateRepo: false,
  stable: false,
  testAccounts: true,
  fsc4b: false,
  username: '',
  token: '',
  pinned: false,
  version: '',
  accentColor: '',
  hostname: '',
  namespace: ctx.namespaces?.[0] ?? '',
};

function field(label, control, { errorId, hint } = {}) {
  const errors = errorId
    ? `<nldd-validation-list><nldd-validation-item id="${errorId}"></nldd-validation-item></nldd-validation-list>`
    : '';
  const help = hint ? `<nldd-form-field-help-text>${escapeHtml(hint)}</nldd-form-field-help-text>` : '';
  return `<nldd-form-field label="${escapeHtml(label)}">${control}${errors}${help}</nldd-form-field>`;
}

function textField(name, value, extra = '') {
  return `<nldd-text-field name="${name}" value="${escapeHtml(value)}" no-spellcheck ${extra}></nldd-text-field>`;
}

function radios(name, label, options, chosen) {
  const fields = options
    .map(
      ([value, text]) =>
        `<nldd-radio-button-field value="${value}" label="${escapeHtml(text)}"${value === chosen ? ' checked' : ''}></nldd-radio-button-field>`,
    )
    .join('');
  return `<nldd-radio-button-group name="${name}" accessible-label="${escapeHtml(label)}">${fields}</nldd-radio-button-group>`;
}

function optionHtml(value, text, selected) {
  return `<option value="${escapeHtml(value)}"${selected ? ' selected' : ''}>${escapeHtml(text)}</option>`;
}

function chosenRadio(form, name) {
  const group = form.querySelector(`nldd-radio-button-group[name="${name}"]`);
  const checked = [...group.querySelectorAll('nldd-radio-button-field')].find((f) => f.checked);
  return checked?.value ?? '';
}

// Each step: its title in the step bar, its fields, the checks on submit and
// what it keeps in the state. `checks` takes the form and returns
// [control, check] pairs, as wireSubmit wants them.
const steps = [
  {
    title: 'Casus',
    intro: 'Waar staat de casus? Het manifest (bron.yaml) noemt het casusbestand en de artikelen; die staan in dezelfde repository.',
    fields: () => `
      ${field('Repository', textField('repoUrl', state.repoUrl, 'type="url" required placeholder="https://code.digitaledienst.overheid.nl/regelrecht/wetsproces-parkeerkaart"'), { errorId: 'e-repo', hint: 'Het https-adres om de repository te klonen.' })}
      ${field('Branch, tag of commit', textField('ref', state.ref), { errorId: 'e-ref', hint: 'Leeg is main.' })}
      ${field('Pad naar het manifest', textField('path', state.path), { errorId: 'e-path', hint: 'In een manifestrepository staat bron.yaml bovenin. In de repository fieldlab-2026 zelf is het packages/wetsproces/bron.yaml.' })}`,
    checks: (f) => [
      [f.querySelector('[name="repoUrl"]'), repoUrlError],
      [f.querySelector('[name="ref"]'), refError],
      [f.querySelector('[name="path"]'), pathError],
    ],
    keep: (v) => Object.assign(state, { repoUrl: v('repoUrl'), ref: v('ref'), path: v('path') }),
  },
  {
    title: 'Toegang',
    intro: 'Wie mag de casus lezen, en hoe loggen mensen in?',
    fields: () => `
      <nldd-title size="5" heading-level="3" text="De repository"></nldd-title>
      ${radios('access', 'Toegang tot de repository', [['public', 'Openbaar'], ['private', 'Privé, met een token']], state.privateRepo ? 'private' : 'public')}
      <div id="credentials"${state.privateRepo ? '' : ' hidden'}>
        <nldd-spacer size="16"></nldd-spacer>
        ${field('Gebruikersnaam', textField('username', state.username, 'autocomplete="off"'), { errorId: 'e-user' })}
        ${field('Token', `<nldd-password-field name="token" value="${escapeHtml(state.token)}" autocomplete="off"></nldd-password-field>`, { errorId: 'e-token', hint: 'Een token dat de repository alleen mag lezen. Het komt in een Secret in de namespace van het wetsproces.' })}
      </div>
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-title size="5" heading-level="3" text="Inloggen"></nldd-title>
      ${radios('login', 'Inloggen', [['test', 'Testomgeving: iedereen kiest zelf een testpersoon, en een beheerder zet de datum'], ['stable', 'Stabiel: inloggen via een eigen Keycloak, met de echte datum']], state.stable ? 'stable' : 'test')}
      <div id="stable"${state.stable ? '' : ' hidden'}>
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-rich-text>
          <p>Het wetsproces krijgt een eigen Keycloak op een derde adres, &lt;naam&gt;-login. Het wachtwoord van de beheerder van die Keycloak maakt het cluster zelf aan, in een Secret in de namespace; de detailpagina noemt de naam van dat Secret.</p>
        </nldd-rich-text>
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-checkbox-field name="testAccounts" label="Met de testaccounts van de casus, die allemaal één gegenereerd wachtwoord delen"${state.testAccounts ? ' checked' : ''}></nldd-checkbox-field>
      </div>
      <div id="fsc4b"${state.stable ? ' hidden' : ''}>
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-checkbox-field name="fsc4b" label="Ook FSC voor Burgers: een app mag namens een testburger aanvragen indienen en het dossier lezen, met een test-Activatiedienst die niemand controleert"${state.fsc4b ? ' checked' : ''}></nldd-checkbox-field>
      </div>`,
    wire: (f) => {
      f.querySelector('nldd-radio-button-group[name="access"]').addEventListener('change', () => {
        f.querySelector('#credentials').hidden = chosenRadio(f, 'access') !== 'private';
      });
      f.querySelector('nldd-radio-button-group[name="login"]').addEventListener('change', () => {
        f.querySelector('#stable').hidden = chosenRadio(f, 'login') !== 'stable';
        f.querySelector('#fsc4b').hidden = chosenRadio(f, 'login') === 'stable';
      });
    },
    checks: (f) => {
      if (chosenRadio(f, 'access') !== 'private') return [];
      return [
        [f.querySelector('[name="username"]'), usernameError],
        [f.querySelector('[name="token"]'), tokenError],
      ];
    },
    keep: (v, f) =>
      Object.assign(state, {
        privateRepo: chosenRadio(f, 'access') === 'private',
        username: v('username'),
        token: v('token'),
        stable: chosenRadio(f, 'login') === 'stable',
        testAccounts: Boolean(f.querySelector('nldd-checkbox-field[name="testAccounts"]')?.checked),
        fsc4b: chosenRadio(f, 'login') !== 'stable' && Boolean(f.querySelector('nldd-checkbox-field[name="fsc4b"]')?.checked),
      }),
  },
  {
    title: 'Versie',
    intro: 'Welke versie van het wetsprocessysteem draait de casus?',
    fields: () => `
      ${radios('version', 'Versie', [['latest', 'Laatste versie (main), die meegaat met elke nieuwe build'], ['pinned', 'Een vaste commit']], state.pinned ? 'pinned' : 'latest')}
      <div id="commit"${state.pinned ? '' : ' hidden'}>
        <nldd-spacer size="16"></nldd-spacer>
        ${field('Commit', textField('commit', state.version, 'maxlength="40"'), { errorId: 'e-commit', hint: 'De volledige hash van een commit van regelrecht waarvoor een image is gebouwd.' })}
      </div>`,
    wire: (f) => {
      f.querySelector('nldd-radio-button-group').addEventListener('change', () => {
        f.querySelector('#commit').hidden = chosenRadio(f, 'version') !== 'pinned';
      });
    },
    checks: (f) => (chosenRadio(f, 'version') === 'pinned' ? [[f.querySelector('[name="commit"]'), commitError]] : []),
    keep: (v, f) => {
      const pinned = chosenRadio(f, 'version') === 'pinned';
      Object.assign(state, { pinned, version: pinned ? v('commit') : '' });
    },
  },
  {
    title: 'Uiterlijk',
    intro: 'De accentkleur van de schermen, een van de kleuren van het NLDD-designsysteem.',
    fields: () => {
      const options = [optionHtml('', 'Standaard', !state.accentColor)]
        .concat(ACCENT_COLORS.map((c) => optionHtml(c, c, c === state.accentColor)))
        .join('');
      return field('Accentkleur', `<nldd-dropdown><select name="accentColor">${options}</select></nldd-dropdown>`);
    },
    checks: () => [],
    keep: (v) => Object.assign(state, { accentColor: v('accentColor') }),
  },
  {
    title: 'Adres',
    intro: 'Onder welke naam is het wetsproces te bereiken, en in welke namespace komt het?',
    fields: () => {
      const namespaces = Array.isArray(ctx.namespaces) ? ctx.namespaces : [];
      const ns = namespaces.length
        ? `<nldd-dropdown><select name="namespace">${namespaces
            .map((n) => optionHtml(n, ctx.namespaceDisplayNames?.[n] ?? n, n === state.namespace))
            .join('')}</select></nldd-dropdown>`
        : textField('namespace', state.namespace, 'required maxlength="63"');
      return `
        ${field('Hostnaam', textField('hostname', state.hostname, 'required placeholder="zorgtoeslag"'), {
          errorId: 'e-host',
          hint: 'Burgers gebruiken <naam>.<basisdomein>, medewerkers <naam>-medewerkers.<basisdomein>. Een eigen domein mag ook, met punten erin. De naam is ook de naam van het wetsproces.',
        })}
        ${field('Namespace', ns, { errorId: 'e-ns' })}`;
    },
    checks: (f) => [
      [f.querySelector('[name="hostname"]'), hostnameError],
      [f.querySelector('[name="namespace"]'), namespaceError],
    ],
    keep: (v) => Object.assign(state, { hostname: v('hostname'), namespace: v('namespace') }),
  },
  {
    title: 'Controleren',
    intro: 'Dit wordt het wetsproces.',
    fields: () => {
      const a = addresses(state.hostname);
      return `
        ${renderKeyValueList(
          [
            ['Naam', nameFromHostname(state.hostname)],
            ['Namespace', ctx.namespaceDisplayNames?.[state.namespace] ?? state.namespace],
            ['Repository', state.repoUrl],
            ['Branch of tag', state.ref || DEFAULT_REF],
            ['Manifest', state.path || DEFAULT_PATH],
            ['Toegang', state.privateRepo ? `privé, token in Secret ${secretName(nameFromHostname(state.hostname))}` : 'openbaar'],
            ['Versie', state.version || 'laatste versie (main)'],
            ['Accentkleur', state.accentColor || 'standaard'],
            [
              'Inloggen',
              state.stable
                ? `stabiel, via Keycloak${state.testAccounts ? ', met testaccounts' : ''}`
                : 'testomgeving',
            ],
            ['FSC voor Burgers', state.fsc4b ? `aan, met het demo-stelsel: ${fsc4bDiscoveryUrl(state.hostname)}` : 'uit'],
            ['Burgers', a.burger],
            ['Medewerkers', a.medewerkers],
            ...(state.stable
              ? [
                  ['Inloggen via', a.login],
                  ['Secret met de wachtwoorden', keycloakSecretName(nameFromHostname(state.hostname))],
                ]
              : []),
          ],
          'Samenvatting',
        )}
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-banner variant="warning" text="Verwijderen haalt ook de data weg"
                     supporting-text="Het wetsproces krijgt een eigen database${state.stable ? ', en zijn Keycloak een tweede' : ''}. Wie het later verwijdert, verwijdert die database${state.stable ? 's' : ''} en de volumes eronder mee, met alle kronieken${state.stable ? ' en accounts' : ''}. Er is geen back-up."></nldd-banner>`;
    },
    checks: () => [],
    keep: () => {},
  },
];

let current = 0;

function renderStepBar() {
  stepBar.innerHTML = steps
    .map((s, i) => {
      const status = i < current ? 'past' : i === current ? 'current' : 'future';
      return `<nldd-step-bar-item text="${escapeHtml(s.title)}" status="${status}"></nldd-step-bar-item>`;
    })
    .join('');
}

async function create() {
  const wetsproces = buildWetsproces(state);
  const { namespace } = wetsproces.metadata;
  let made;
  try {
    made = await fundament.k8s.create({ ...WETSPROCES_RESOURCE, namespace }, wetsproces);
  } catch (err) {
    if (err?.code === 'forbidden') throw new Error('alleen een beheerder van het project mag een wetsproces maken');
    throw err;
  }
  if (state.privateRepo) {
    try {
      await fundament.k8s.create(
        { ...SECRET_RESOURCE, namespace },
        buildSecret(made, { username: state.username, token: state.token }),
      );
    } catch (err) {
      // Without its Secret the wetsproces cannot clone the casus; take it
      // away again rather than leave one behind that waits forever.
      await fundament.k8s
        .delete({ ...WETSPROCES_RESOURCE, name: made.metadata.name, namespace })
        .catch(() => {});
      throw new Error(`het token is niet bewaard (${err?.message ?? err}); er is niets aangemaakt`);
    }
  }
  navigateToDetail(made.metadata.name, namespace);
}

function render() {
  renderStepBar();
  const step = steps[current];
  const last = current === steps.length - 1;
  content.innerHTML = `
    <nldd-title size="4" heading-level="2" text="${escapeHtml(step.title)}" supporting-text="${escapeHtml(step.intro)}"></nldd-title>
    <nldd-spacer size="16"></nldd-spacer>
    <nldd-form>
      <form id="step-form" novalidate>
        <nldd-banner id="error-banner" variant="critical" hidden></nldd-banner>
        ${step.fields()}
        <nldd-form-actions>
          <nldd-button-group orientation="horizontal">
            <nldd-button id="next-btn" type="submit" appearance="primary" text="${last ? 'Wetsproces aanmaken' : 'Volgende'}"></nldd-button>
            <nldd-button id="back-btn" type="button" appearance="secondary" text="${current === 0 ? 'Annuleren' : 'Vorige'}"></nldd-button>
          </nldd-button-group>
        </nldd-form-actions>
      </form>
    </nldd-form>`;
  const form = document.getElementById('step-form');
  const value = (name) => String(form.querySelector(`[name="${name}"]`)?.value ?? '').trim();
  step.wire?.(form);
  document.getElementById('back-btn').addEventListener('click', () => {
    if (current === 0) {
      navigateBack();
      return;
    }
    current -= 1;
    render();
  });
  wireSubmit(form, {
    button: document.getElementById('next-btn'),
    errorBanner: document.getElementById('error-banner'),
    failPrefix: last ? 'Het wetsproces is niet aangemaakt' : 'Dat lukte niet',
    checks: step.checks(form),
    action: async () => {
      step.keep(value, form);
      if (last) {
        await create();
        return;
      }
      current += 1;
      render();
    },
  });
}

render();
