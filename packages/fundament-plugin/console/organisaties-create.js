// The wizard of an Organisatie (runWizard in _shared.js): its name and logo,
// its colour, its address, and a last look before it is made.
import {
  loadSdk,
  loadNlddDesignSystem,
  navigateToDetail,
  renderKeyValueList,
  runWizard,
  field,
  textField,
  accentDropdown,
  namespaceControl,
} from './_shared.js';
import { addresses, hostnameError, nameFromHostname, namespaceError } from './regelhets-body.js';
import { ORGANISATIE_RESOURCE, buildOrganisatie, displayNameError, logoError } from './organisaties-body.js';

await Promise.all([loadSdk(), loadNlddDesignSystem()]);
const ctx = await fundament.init;

const state = {
  displayName: '',
  logo: '',
  accentColor: '',
  hostname: '',
  namespace: ctx.namespaces?.[0] ?? '',
};

const steps = [
  {
    title: 'Organisatie',
    intro: 'Welke naam en welk logo zien medewerkers en burgers?',
    fields: () => `
      ${field('Naam', textField('displayName', state.displayName, 'required placeholder="Gemeente Wilgendaal"'), { errorId: 'e-name' })}
      ${field('Logo', textField('logo', state.logo, 'type="url" placeholder="https://"'), { errorId: 'e-logo', hint: 'Het https-adres van het logo; leeg toont alleen de naam.' })}`,
    checks: (f) => [
      [f.querySelector('[name="displayName"]'), displayNameError],
      [f.querySelector('[name="logo"]'), logoError],
    ],
    keep: (v) => Object.assign(state, { displayName: v('displayName'), logo: v('logo') }),
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
    intro: 'Onder welke naam is de organisatie te bereiken, en in welke namespace komt ze?',
    fields: () => `
      ${field('Hostnaam', textField('hostname', state.hostname, 'required placeholder="wilgendaal"'), {
        errorId: 'e-host',
        hint: 'Burgers gebruiken <naam>.<basisdomein>, medewerkers <naam>-medewerkers.<basisdomein>. De naam is ook de naam van de organisatie.',
      })}
      ${field('Namespace', namespaceControl(ctx, state.namespace), { errorId: 'e-ns', hint: 'Procedures in elke namespace van dit project kunnen haar kiezen.' })}`,
    checks: (f) => [
      [f.querySelector('[name="hostname"]'), hostnameError],
      [f.querySelector('[name="namespace"]'), namespaceError],
    ],
    keep: (v) => Object.assign(state, { hostname: v('hostname'), namespace: v('namespace') }),
  },
  {
    title: 'Controleren',
    intro: 'Dit wordt de organisatie.',
    fields: () => {
      const a = addresses(state.hostname);
      return `
        ${renderKeyValueList(
          [
            ['Naam', state.displayName],
            ['Objectnaam', nameFromHostname(state.hostname)],
            ['Namespace', ctx.namespaceDisplayNames?.[state.namespace] ?? state.namespace],
            ['Logo', state.logo || 'geen'],
            ['Accentkleur', state.accentColor || 'standaard (donkerblauw)'],
            ['Inloggen', 'testomgeving'],
            ['Burgers', a.burger],
            ['Medewerkers', a.medewerkers],
          ],
          'Samenvatting',
        )}
        <nldd-spacer size="16"></nldd-spacer>
        <nldd-banner variant="neutral" text="Procedures kiezen zelf hun organisatie"
                     supporting-text="Een procedure komt achter deze voorkant in de stap Organisatie van haar wizard. Ze houdt haar eigen adressen en haar eigen data."></nldd-banner>`;
    },
    checks: () => [],
    keep: () => {},
  },
];

async function create() {
  const organisatie = buildOrganisatie(state);
  const { namespace } = organisatie.metadata;
  let made;
  try {
    made = await fundament.k8s.create({ ...ORGANISATIE_RESOURCE, namespace }, organisatie);
  } catch (err) {
    if (err?.code === 'forbidden') throw new Error('alleen een beheerder van het project mag een organisatie maken');
    throw err;
  }
  navigateToDetail(made.metadata.name, namespace);
}

runWizard({
  content: document.getElementById('content'),
  stepBar: document.getElementById('steps'),
  steps,
  finishText: 'Organisatie aanmaken',
  failPrefix: 'De organisatie is niet aangemaakt',
  finish: create,
});
