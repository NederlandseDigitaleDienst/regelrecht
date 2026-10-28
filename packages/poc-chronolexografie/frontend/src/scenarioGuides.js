/**
 * The guide beside the page (ScenarioGuide.vue), built from a scenario of the
 * case YAML (`casussen/<id>.yaml`, `scenarios[]`). The YAML holds the whole
 * scenario: per step its kind (`stap`), play date, the values the forms take
 * (`invoer`), what the step comes to (`uitkomst`) and its text. This file
 * only turns that into what the panel shows: the tab of each kind, a title
 * and text where the step gives none, when a step is done and what the
 * scenario expects of it. A case from a source (NAPP) gives its steps as
 * plain text and has no guide; the tab Scenario's shows those instead.
 *
 * A built step: `title`, the `tab` it happens on, the play date it wants
 * (`date`, optional), what to do (`todo`), what the visitor sees once it is
 * done (`seen`), when it is done (`done`, a test on
 * scenarioProgress.counts()), optionally what the scenario expects of it
 * (`expect`: a test and the text of the warning when it fails), the step of
 * the YAML (`stap`) and, counted from 1, what it acts on (`doel`).
 *
 * The texts name the buttons and fields as the components label them;
 * change them together.
 */
import { hoofdletter, jaNee, leesbaar, longDate } from './state.js';

// What the scenario expects of a step, as warning text.
const expectation = (check, text) => ({ check, text });

const granted = (n) =>
  expectation((k) => k.decided[n - 1] === true, 'hier hoort het besluit de aanvraag toe te wijzen');
const rejected = (n) =>
  expectation((k) => k.decided[n - 1] === false, 'hier hoort het besluit de aanvraag af te wijzen');
const upheld = (n) => expectation((k) => k.gegrond >= n, 'hier hoort het bezwaar gegrond te zijn');
const dismissed = (n) => expectation((k) => k.ongegrond >= n, 'hier hoort het bezwaar ongegrond te zijn');
const inadmissible = (n) =>
  expectation((k) => k.nietOntvankelijk >= n, 'hier hoort het bezwaar niet-ontvankelijk te zijn');

const SEEN_OBJECTION =
  'Het bezwaar staat in het dossier en verwijst naar het besluit. De cel nam het bestreden besluit en de termijn uit dat besluit. De aanvraag verandert niet.';

/**
 * Per kind of step: its tab, its title and text when the step gives none,
 * when it is done, given `n`, the how-many-th of its kind it is (for a
 * decision: the application it decides on), and the refusal a step of this
 * kind can expect (`weigering`, as the cell words it): a decision that
 * misses a judgement, a decision on objection against a decision already
 * replaced. tests/scenarios.rs checks the same.
 */
const KINDS = {
  aanvraag: {
    tab: 'aanvrager',
    title: (n) => (n === 1 ? 'Aanvraag indienen' : 'Nog een aanvraag indienen'),
    todo: 'Klik op Aanvraag indienen.',
    seen: (n) =>
      n === 1
        ? 'De aanvraag staat onder Mijn aanvragen op In behandeling. De velden komen uit de wet (het artikel en Awb 4:2); de dag van ontvangst is de speeldatum.'
        : 'De tweede aanvraag staat onder Mijn aanvragen op In behandeling.',
    done: (n) => (k) => k.aanvragen >= n,
  },
  besluit: {
    tab: 'bestuursorgaan',
    title: (n) => (n === 1 ? 'Besluit nemen' : 'Besluiten op de tweede aanvraag'),
    todo: 'Klik onder Open aanvragen op Besluit nemen.',
    seen: () => 'Het besluit staat onder Besluiten. Bij Aanvrager staat de aanvraag niet meer op In behandeling.',
    done: (n) => (k) => k.decided.length >= n,
    weigering: /no judgement was given|asks '/,
  },
  bezwaar: {
    tab: 'aanvrager',
    title: (n) => (n === 1 ? 'Bezwaar maken' : 'Nog een bezwaar maken'),
    todo: 'Klik bij het besluit op Bezwaar maken en dan op Bezwaar indienen.',
    seen: () => SEEN_OBJECTION,
    done: (n) => (k) => k.bezwaren >= n,
  },
  beslissing_op_bezwaar: {
    tab: 'bestuursorgaan',
    title: () => 'Beslissen op bezwaar',
    todo: 'Klik onder Open bezwaren op Ontvankelijkheid toetsen, laat de bevindingen staan en klik op Beslissen op bezwaar.',
    seen: () => 'De beslissing staat onder Beslissingen op bezwaar.',
    done: (n) => (k) => k.beslissingen >= n,
    weigering: /already replaces/,
  },
  vastleggen: {
    tab: 'andere',
    title: () => 'Vastleggen',
    todo: 'Klik op Vastleggen.',
    seen: () => 'Het feit staat in de kroniek van de andere cel.',
    // Done once the cell holds one more record of the fact than it knew
    // before this step, whatever the visitor changed in it.
    done: (n, stap, vooraf) => (k) => k.executogrammen(feitNaam(stap.feit)) >= vooraf + n,
  },
  beroep: {
    tab: 'aanvrager',
    title: () => 'Beroep instellen',
    todo: 'Klik bij de beslissing op bezwaar op Beroep instellen en dan op Beroepschrift indienen.',
    seen: () =>
      'Het beroep staat in de kroniek van Rechtbank Wilgendaal, met het afschrift dat het bestuursorgaan van zijn beslissing maakte. Rechtbank en bestuursorgaan hebben elk een eigen kroniek; beide staan onder Kroniek.',
    done: (n) => (k) => k.beroepen >= n,
  },
  uitspraak: {
    tab: 'andere',
    title: () => 'Uitspraak doen',
    todo: 'Klik onder Ingekomen beroepen op Uitspraak doen.',
    seen: () =>
      'De uitspraak staat onder Uitspraken, in de kroniek van de rechtbank. Het bestuursorgaan weet er nog niets van.',
    done: (n) => (k) => k.uitspraken >= n,
  },
  toezenden: {
    tab: 'andere',
    title: () => 'Toezenden',
    todo: 'Klik bij de uitspraak op Toezenden.',
    seen: () => 'Het bestuursorgaan legt de ontvangst vast in zijn eigen kroniek (Awb 8:79).',
    done: (n) => (k) => k.ontvangsten >= n,
  },
  opnieuw_beslissen: {
    tab: 'bestuursorgaan',
    title: () => 'Opnieuw beslissen',
    todo: 'Klik onder Na een uitspraak op Heroverwegen en dan op Opnieuw beslissen op bezwaar.',
    seen: () => 'De nieuwe beslissing op bezwaar staat onder Na een uitspraak.',
    done: (n) => (k) => k.redecisions >= n,
    weigering: /already replaces/,
  },
};

// `regeling#3#geneeskundig_onderzoek_verricht` → `geneeskundig_onderzoek_verricht`.
const feitNaam = (feit) => String(feit).split('#').at(-1);

/** How many records of `feit` the other cells of `casus` hold before a scenario. */
const vooraf = (casus, feit) =>
  (casus?.andere_cellen ?? [])
    .flatMap((c) => c.begin?.executogrammen ?? [])
    .filter((e) => feitNaam(e.feit) === feitNaam(feit)).length;

const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;
const waarde = (v) =>
  typeof v === 'boolean' ? hoofdletter(jaNee(v)) : ISO_DATE.test(String(v)) ? longDate(v) : String(v);

/**
 * The sentence that names the values the step fills in, from its `invoer`:
 * the guide says them, the form holds them, and both read them from the
 * YAML. Empty when the step adds none to the case's example values.
 */
export function invoerZin(invoer) {
  const delen = Object.entries(invoer ?? {}).map(([naam, v]) => `"${leesbaar(naam)}" ${waarde(v)}`);
  if (!delen.length) return '';
  const lijst = delen.length === 1 ? delen[0] : `${delen.slice(0, -1).join(', ')} en ${delen.at(-1)}`;
  return `Het formulier staat ingevuld uit het scenario: ${lijst}.`;
}

/**
 * The guide of `scenario` (a scenario of a case, as `api.casus` gives it),
 * with `casus` for what its other cells record before a scenario.
 */
export function bouwGids(scenario, casus = null) {
  const geteld = {};
  const feiten = {};
  // Per kind, the steps the cell decided on and the steps it refused.
  const besloten = {};
  const geweigerd = {};
  const uitkomsten = {};
  const steps = scenario.stappen.map((stap) => {
    const kind = KINDS[stap.stap];
    if (!kind) throw new Error(`unknown step '${stap.stap}' in "${scenario.naam}"`);
    const n = (geteld[stap.stap] = (geteld[stap.stap] ?? 0) + 1);
    // What the step acts on, counted from 1; a decision and a decision on
    // objection count the application and the objection it decides on.
    let doel = stap.op ?? n;
    if (stap.stap === 'bezwaar') doel = stap.op ?? geteld.aanvraag ?? 1;
    if (stap.stap === 'beroep') doel = stap.op ?? besloten.beslissing_op_bezwaar ?? 1;
    if (stap.uitkomst === 'geweigerd') geweigerd[stap.stap] = (geweigerd[stap.stap] ?? 0) + 1;
    else besloten[stap.stap] = (besloten[stap.stap] ?? 0) + 1;
    const keer = stap.uitkomst ? (uitkomsten[stap.uitkomst] = (uitkomsten[stap.uitkomst] ?? 0) + 1) : 0;
    const keerFeit = stap.feit ? (feiten[feitNaam(stap.feit)] = (feiten[feitNaam(stap.feit)] ?? 0) + 1) : 0;
    let done =
      stap.stap === 'vastleggen'
        ? kind.done(keerFeit, stap, vooraf(casus, stap.feit))
        : kind.done(stap.stap === 'besluit' ? doel : n, stap);
    let expect = null;
    switch (stap.uitkomst) {
      case 'toegewezen':
        expect = granted(doel);
        break;
      case 'afgewezen':
        expect = rejected(doel);
        break;
      case 'gegrond':
        expect = upheld(keer);
        break;
      case 'ongegrond':
        expect = dismissed(keer);
        break;
      case 'niet_ontvankelijk':
        expect = inadmissible(keer);
        break;
      case 'geweigerd': {
        // The cell refuses and records nothing: the step is done once it
        // refused as the step expects, and a decision taken after all
        // departs from the scenario.
        const genomen = done;
        const re = kind.weigering ?? /$^/;
        const keerGeweigerd = geweigerd[stap.stap];
        done = (k) => k.refusals.filter((m) => re.test(m)).length >= keerGeweigerd || genomen(k);
        expect =
          stap.stap === 'besluit'
            ? expectation((k) => !genomen(k), 'hier hoort de cel de stap te weigeren, zonder besluit')
            : expectation((k) => !genomen(k), 'hier hoort de cel de stap te weigeren');
        break;
      }
    }
    const todo = [invoerZin(stap.invoer), stap.tekst ?? kind.todo].filter(Boolean).join(' ');
    return {
      title: stap.titel ?? kind.title(stap.stap === 'besluit' ? doel : n),
      tab: kind.tab,
      date: stap.datum ?? undefined,
      todo,
      seen: stap.gezien ?? kind.seen(stap.stap === 'besluit' ? doel : n),
      done,
      expect,
      stap,
      doel,
    };
  });
  return { name: scenario.naam, summary: scenario.samenvatting ?? '', steps };
}

// One guide per scenario object, so the panel and the forms share it.
const gebouwd = new WeakMap();

/**
 * The guide for scenario `naam` of `casus`, or null: a case from a source
 * (`casus.bron`, such as NAPP) has none, and neither has a scenario whose
 * steps are plain text or a name the case does not know. Those show the
 * steps from the YAML instead.
 */
export function guideFor(casus, naam) {
  if (!casus || casus.bron) return null;
  const scenario = casus.scenarios?.find((s) => s.naam === naam);
  if (!scenario?.stappen?.length || scenario.stappen.some((s) => typeof s !== 'object')) return null;
  if (!gebouwd.has(scenario)) gebouwd.set(scenario, bouwGids(scenario, casus));
  return gebouwd.get(scenario);
}
