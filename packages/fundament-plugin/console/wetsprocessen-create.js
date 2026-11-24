// The wizard: one step at a time, each a form of its own whose submit is
// "Volgende" (runWizard in _shared.js). The last step shows what will be made
// and makes it: the Wetsproces, for a private repository the Secret with its
// token, owned by the Wetsproces so both go together, and a new Organisatie
// when one is asked for.
import {
  loadSdk,
  loadNlddDesignSystem,
  escapeHtml,
  navigateToDetail,
  renderKeyValueList,
  runWizard,
  field,
  textField,
  radios,
  optionHtml,
  chosenRadio,
  namespaceControl,
  accentDropdown,
} from './_shared.js';
import {
  ORGANISATIE_EXISTING,
  ORGANISATIE_NEW,
  ORGANISATIE_NONE,
  ORGANISATIE_RESOURCE,
  buildOrganisatie,
  displayNameError,
  hostnamesClash,
  logoError,
  organisatieKey,
  organisatieReference,
  sortOrganisaties,
} from './organisaties-body.js';
import {
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
  ownParties: false,
  username: '',
  token: '',
  pinned: false,
  version: '',
  accentColor: '',
  hostname: '',
  namespace: ctx.namespaces?.[0] ?? '',
  organisatie: ORGANISATIE_NONE,
  existingOrganisatie: '',
  orgDisplayName: '',
  orgHostname: '',
  orgLogo: '',
  orgAccentColor: '',
};

// The organisaties a wetsproces can join: those in the namespaces of the
// project. The plugin refuses one of another project, so the wizard offers
// none. Not readable is no reason to stop the wizard: it offers none then.
async function listOrganisaties() {
  const namespaces = Array.isArray(ctx.namespaces) && ctx.namespaces.length > 0 ? ctx.namespaces : [undefined];
  try {
    const lists = await Promise.all(
      namespaces.map((namespace) => fundament.k8s.list({ ...ORGANISATIE_RESOURCE, namespace })),
    );
    return sortOrganisaties(lists.flatMap((l) => l.items ?? []));
  } catch {
    return [];
  }
}
const organisaties = await listOrganisaties();
state.existingOrganisatie = organisaties[0] ? organisatieKey(organisaties[0]) : '';

function organisatieLabel(key) {
  const item = organisaties.find((o) => organisatieKey(o) === key);
  return item ? `${item.spec?.displayName ?? item.metadata.name} (${key})` : key;
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
        ${field('Token', `<nldd-password-field name="token" value="${escapeHtml(state.token)}" autocomplete="off"></nldd-password-field>`, { errorId: 'e-token', hint: 'Een token dat de repository alleen mag lezen. Het cluster gebruikt het één keer, om een kopie van de casus te maken, en verwijdert het daarna. Een nieuwe versie van de casus vraagt een nieuw token.' })}
      </div>
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-title size="5" heading-level="3" text="Inloggen"></nldd-title>
      ${radios('login', 'Inloggen', [['test', 'Testomgeving: iedereen kiest zelf een testpersoon, en een beheerder zet de datum'], ['stable', 'Stabiel: inloggen via een eigen Keycloak, met de echte datum']], state.stable ? 'stable' : 'test')}
      <div id="stable"${state.stable ? '' : ' hidden'}>
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-rich-text>
          <p>De procedure krijgt een eigen Keycloak op een derde adres, &lt;naam&gt;-login. Het wachtwoord van de beheerder van die Keycloak maakt het cluster zelf aan, in een Secret in de namespace; de detailpagina noemt de naam van dat Secret.</p>
        </nldd-rich-text>
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-checkbox-field name="testAccounts" label="Met de testaccounts van de casus, die allemaal één gegenereerd wachtwoord delen"${state.testAccounts ? ' checked' : ''}></nldd-checkbox-field>
      </div>
      <div id="fsc4b"${state.stable ? ' hidden' : ''}>
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-checkbox-field name="fsc4b" label="Ook FSC voor Burgers: een app mag namens een testburger aanvragen indienen en het dossier lezen, met een test-Activatiedienst die niemand controleert"${state.fsc4b ? ' checked' : ''}></nldd-checkbox-field>
      </div>
      <div id="ownParties"${state.stable ? ' hidden' : ''}>
        <nldd-spacer size="12"></nldd-spacer>
        <nldd-checkbox-field name="ownParties" label="Externe partijen op een eigen omgeving: een partij die volgens de regelgeving van de casus iets gevraagd wordt, zoals de GGD om een onderzoek, krijgt een eigen adres waar haar medewerkers inloggen. Vraagt de casus geen partij iets, dan komt er geen bij"${state.ownParties ? ' checked' : ''}></nldd-checkbox-field>
      </div>`,
    wire: (f) => {
      f.querySelector('nldd-radio-button-group[name="access"]').addEventListener('change', () => {
        f.querySelector('#credentials').hidden = chosenRadio(f, 'access') !== 'private';
      });
      f.querySelector('nldd-radio-button-group[name="login"]').addEventListener('change', () => {
        f.querySelector('#stable').hidden = chosenRadio(f, 'login') !== 'stable';
        f.querySelector('#fsc4b').hidden = chosenRadio(f, 'login') === 'stable';
        f.querySelector('#ownParties').hidden = chosenRadio(f, 'login') === 'stable';
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
        ownParties:
          chosenRadio(f, 'login') !== 'stable' && Boolean(f.querySelector('nldd-checkbox-field[name="ownParties"]')?.checked),
      }),
  },
  {
    title: 'Versie',
    intro: 'Welke versie van RegelHet draait de casus?',
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
    fields: () => field('Accentkleur', accentDropdown('accentColor', state.accentColor)),
    checks: () => [],
    keep: (v) => Object.assign(state, { accentColor: v('accentColor') }),
  },
  {
    title: 'Adres',
    intro: 'Onder welke naam is de procedure te bereiken, en in welke namespace komt ze?',
    fields: () => {
      const ns = namespaceControl(ctx, state.namespace);
      return `
        ${field('Hostnaam', textField('hostname', state.hostname, 'required placeholder="zorgtoeslag"'), {
          errorId: 'e-host',
          hint: 'Burgers gebruiken <naam>.<basisdomein>, medewerkers <naam>-medewerkers.<basisdomein>. Een eigen domein mag ook, met punten erin. De naam is ook de naam van de procedure.',
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
    title: 'Organisatie',
    // An organisatie runs in test mode only, and the plugin refuses a stable
    // wetsproces behind a front in test mode.
    skip: () => state.stable,
    intro: 'Staat de procedure achter de voorkant van een organisatie? Die geeft medewerkers en burgers één adres en één login voor al haar procedures. De procedure houdt ook haar eigen adressen.',
    fields: () => {
      const options = [
        [ORGANISATIE_NONE, 'Geen, een losse procedure'],
        [ORGANISATIE_NEW, 'Een nieuwe organisatie'],
      ];
      if (organisaties.length) options.push([ORGANISATIE_EXISTING, 'Een bestaande organisatie']);
      const chosen = state.organisatie === ORGANISATIE_EXISTING && !organisaties.length ? ORGANISATIE_NONE : state.organisatie;
      const existing = organisaties
        .map((o) => optionHtml(organisatieKey(o), organisatieLabel(organisatieKey(o)), organisatieKey(o) === state.existingOrganisatie))
        .join('');
      return `
        ${radios('organisatie', 'Organisatie', options, chosen)}
        <div id="org-new"${chosen === ORGANISATIE_NEW ? '' : ' hidden'}>
          <nldd-spacer size="16"></nldd-spacer>
          ${field('Naam', textField('orgDisplayName', state.orgDisplayName, 'placeholder="Gemeente Wilgendaal"'), { errorId: 'e-org-name', hint: 'De naam die medewerkers en burgers zien.' })}
          ${field('Hostnaam', textField('orgHostname', state.orgHostname, 'placeholder="wilgendaal"'), { errorId: 'e-org-host', hint: 'Burgers gebruiken <naam>.<basisdomein>, medewerkers <naam>-medewerkers.<basisdomein>. De organisatie komt in dezelfde namespace als de procedure.' })}
          ${field('Logo', textField('orgLogo', state.orgLogo, 'type="url" placeholder="https://"'), { errorId: 'e-org-logo', hint: 'Het https-adres van het logo; leeg toont alleen de naam.' })}
          ${field('Accentkleur', accentDropdown('orgAccentColor', state.orgAccentColor))}
        </div>
        <div id="org-existing"${chosen === ORGANISATIE_EXISTING ? '' : ' hidden'}>
          <nldd-spacer size="16"></nldd-spacer>
          ${field('Organisatie', `<nldd-dropdown><select name="existingOrganisatie">${existing}</select></nldd-dropdown>`, { hint: 'De organisaties in de namespaces van dit project.' })}
        </div>`;
    },
    wire: (f) => {
      f.querySelector('nldd-radio-button-group[name="organisatie"]').addEventListener('change', () => {
        f.querySelector('#org-new').hidden = chosenRadio(f, 'organisatie') !== ORGANISATIE_NEW;
        f.querySelector('#org-existing').hidden = chosenRadio(f, 'organisatie') !== ORGANISATIE_EXISTING;
      });
    },
    checks: (f) => {
      if (chosenRadio(f, 'organisatie') !== ORGANISATIE_NEW) return [];
      return [
        [f.querySelector('[name="orgDisplayName"]'), displayNameError],
        [
          f.querySelector('[name="orgHostname"]'),
          (v) =>
            hostnameError(v) ??
            (hostnamesClash(v, state.hostname) || nameFromHostname(v) === nameFromHostname(state.hostname)
              ? 'Een andere naam dan die van de procedure zelf.'
              : null),
        ],
        [f.querySelector('[name="orgLogo"]'), logoError],
      ];
    },
    keep: (v, f) =>
      Object.assign(state, {
        organisatie: chosenRadio(f, 'organisatie') || ORGANISATIE_NONE,
        existingOrganisatie: v('existingOrganisatie') || state.existingOrganisatie,
        orgDisplayName: v('orgDisplayName'),
        orgHostname: v('orgHostname'),
        orgLogo: v('orgLogo'),
        orgAccentColor: v('orgAccentColor'),
      }),
  },
  {
    title: 'Controleren',
    intro: 'Dit wordt de procedure.',
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
            ['Toegang', state.privateRepo ? `privé, token in Secret ${secretName(nameFromHostname(state.hostname))}, verwijderd na de kopie` : 'openbaar'],
            ['Versie', state.version || 'laatste versie (main)'],
            ['Accentkleur', state.accentColor || 'standaard'],
            [
              'Inloggen',
              state.stable
                ? `stabiel, via Keycloak${state.testAccounts ? ', met testaccounts' : ''}`
                : 'testomgeving',
            ],
            ['Organisatie', organisatieSummary()],
            ['FSC voor Burgers', state.fsc4b ? `aan, met het demo-stelsel: ${fsc4bDiscoveryUrl(state.hostname)}` : 'uit'],
            ['Externe partijen', state.ownParties ? 'op een eigen omgeving, waar de casus een partij iets vraagt' : 'via het beheerpaneel'],
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
                     supporting-text="De procedure krijgt een eigen database${state.stable ? ', en zijn Keycloak een tweede' : ''}. Wie het later verwijdert, verwijdert die database${state.stable ? 's' : ''} en de volumes eronder mee, met alle kronieken${state.stable ? ' en accounts' : ''}. Er is geen back-up."></nldd-banner>`;
    },
    checks: () => [],
    keep: () => {},
  },
];

function organisatieSummary() {
  switch (state.organisatie) {
    case ORGANISATIE_NEW:
      return `nieuw: ${state.orgDisplayName}, op ${addresses(state.orgHostname).burger} en ${addresses(state.orgHostname).medewerkers}`;
    case ORGANISATIE_EXISTING:
      return organisatieLabel(state.existingOrganisatie);
    default:
      return 'geen, losse procedure';
  }
}

// Makes the new Organisatie first, so the wetsproces finds it at once; when
// the wetsproces is not made after all, it goes again.
async function create() {
  const { namespace } = state;
  // A stable wetsproces skips the step Organisatie; a choice made before it
  // went back to stable does not count.
  if (state.stable) state.organisatie = ORGANISATIE_NONE;
  let organisatie;
  if (state.organisatie === ORGANISATIE_NEW) {
    try {
      organisatie = await fundament.k8s.create(
        { ...ORGANISATIE_RESOURCE, namespace },
        buildOrganisatie({
          namespace,
          hostname: state.orgHostname,
          displayName: state.orgDisplayName,
          logo: state.orgLogo,
          accentColor: state.orgAccentColor,
        }),
      );
    } catch (err) {
      if (err?.code === 'forbidden') throw new Error('alleen een beheerder van het project mag een organisatie maken');
      throw new Error(`de organisatie is niet aangemaakt (${err?.message ?? err}); er is niets aangemaakt`);
    }
  }
  const dropOrganisatie = () =>
    organisatie
      ? fundament.k8s.delete({ ...ORGANISATIE_RESOURCE, name: organisatie.metadata.name, namespace }).catch(() => {})
      : Promise.resolve();

  const wetsproces = buildWetsproces({
    ...state,
    organisatie: organisatieReference(
      { choice: state.organisatie, existing: state.existingOrganisatie, newHostname: state.orgHostname },
      namespace,
    ),
  });
  let made;
  try {
    made = await fundament.k8s.create({ ...WETSPROCES_RESOURCE, namespace }, wetsproces);
  } catch (err) {
    await dropOrganisatie();
    if (err?.code === 'forbidden') throw new Error('alleen een beheerder van het project mag een procedure maken');
    throw err;
  }
  if (state.privateRepo) {
    try {
      await fundament.k8s.create(
        { ...SECRET_RESOURCE, namespace },
        buildSecret(made, { username: state.username, token: state.token }),
      );
    } catch (err) {
      // Without its Secret the wetsproces cannot copy the casus; take it
      // away again rather than leave one behind that waits forever.
      await fundament.k8s
        .delete({ ...WETSPROCES_RESOURCE, name: made.metadata.name, namespace })
        .catch(() => {});
      await dropOrganisatie();
      throw new Error(`het token is niet bewaard (${err?.message ?? err}); er is niets aangemaakt`);
    }
  }
  navigateToDetail(made.metadata.name, namespace);
}

runWizard({
  content,
  stepBar,
  steps,
  finishText: 'Procedure aanmaken',
  failPrefix: 'De procedure is niet aangemaakt',
  finish: create,
});
