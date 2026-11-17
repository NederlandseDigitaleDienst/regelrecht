/**
 * The guide beside the page (ScenarioGuide.vue), built from a scenario of the
 * case YAML (`casussen/<id>.yaml`, `scenarios[]`). The YAML holds the whole
 * scenario: per step its kind (`stap`), play date, the values the forms take
 * (`invoer`), what the step comes to (`uitkomst`) and its text. This file
 * only turns that into what the panel shows: the tab of each kind, a title
 * and text where the step gives none, when a step is done and what the
 * scenario expects of it. Nothing here knows a case by name: a case from
 * a source gets its guide the same way as one of the PoC's own.
 *
 * A step may also be a line of text. It has no check, so it cannot be a
 * step of its own that the chronicles mark as done: it goes along as an
 * instruction (`notes`) with the next action, or, after the last action,
 * with the end of the scenario (`after`). A step of a kind this file does
 * not know is an error: the guide would skip a step the visitor
 * has to take. A scenario without a single action has no guide; the
 * tab Scenario's shows its lines instead. Who to log in as, for a case with
 * a login, the panel reads from the form's choices (inloggen.js). A case
 * with a login step (inlogstap.js) has steps `inloggen` instead: done once
 * the service recorded one more authentication, or refused.
 *
 * A built step: `title`, the `tab` it happens on, the play date it wants
 * (`date`, optional), what happens in the case (`story`, from `verhaal`),
 * what to do in one sentence (`todo`), the values the
 * form takes from the scenario (`values`, one line each), what the visitor
 * sees once it is done (`seen`), when it is done (`done`, a test on
 * scenarioProgress.counts()), optionally what the scenario expects of it
 * (`expect`: a test and the text of the warning when it fails), the
 * instructions before it (`notes`), the step of the YAML (`stap`) and,
 * counted from 1, what it acts on (`doel`), and the answers it expects of a
 * cell (`antwoorden`) with the play date to ask them on (`antwoordDatum`).
 *
 * The texts name the buttons and fields as the components label them;
 * change them together.
 */
import { LOGIN_REFUSED } from './sessie.js';
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

/**
 * Per kind of step: its tab, its title and text when the step gives none,
 * when it is done, given `n`, the how-many-th of its kind it is (for a
 * decision: the application it decides on), and the refusal a step of this
 * kind can expect (`weigering`, as the cell words it): a decision that
 * misses a judgement, a decision on objection against a decision already
 * replaced. tests/scenarios/ checks the same.
 */
const KINDS = {
  aanvraag: {
    tab: 'aanvrager',
    title: (n) => (n === 1 ? 'Aanvraag indienen' : 'Nog een aanvraag indienen'),
    todo: 'Klik op Aanvraag indienen.',
    seen: (n) =>
      n === 1
        ? 'De aanvraag staat onder Mijn aanvragen op In behandeling.'
        : 'De tweede aanvraag staat onder Mijn aanvragen op In behandeling.',
    done: (n) => (k) => k.aanvragen >= n,
  },
  besluit: {
    tab: 'bestuursorgaan',
    title: (n) => (n === 1 ? 'Besluit nemen' : 'Besluiten op de tweede aanvraag'),
    todo: 'Klik onder Open aanvragen op Besluit nemen.',
    seen: () => 'Het besluit staat bij Aanvrager onder Mijn aanvragen.',
    done: (n) => (k) => k.decided.length >= n,
    weigering: /no judgement was given|asks '/,
  },
  bezwaar: {
    tab: 'aanvrager',
    title: (n) => (n === 1 ? 'Bezwaar maken' : 'Nog een bezwaar maken'),
    todo: 'Klik bij het besluit op Bezwaar maken en dan op Bezwaar indienen.',
    seen: () => 'Het bezwaar staat bij Bestuursorgaan onder Open bezwaren.',
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
    // The case's own cell records on the tab of the authority.
    eigen: {
      tab: 'bestuursorgaan',
      todo: 'Klik onder Vastleggen op Vastleggen.',
      seen: () => 'Het feit staat in de kroniek van het bestuursorgaan.',
    },
    // Done once the cell holds one more record of the fact than it knew
    // before this step, whatever the visitor changed in it.
    done: (n, stap, vooraf) => (k) => k.executogrammen(feitNaam(stap.feit)) >= vooraf + n,
  },
  beroep: {
    tab: 'aanvrager',
    title: () => 'Beroep instellen',
    todo: 'Klik bij de beslissing op bezwaar op Beroep instellen en dan op Beroepschrift indienen.',
    seen: () => 'Het beroep staat bij Andere cellen onder Ingekomen beroepen.',
    done: (n) => (k) => k.beroepen >= n,
  },
  uitspraak: {
    tab: 'andere',
    title: () => 'Uitspraak doen',
    todo: 'Klik onder Ingekomen beroepen op Uitspraak doen.',
    seen: () => 'De uitspraak staat onder Uitspraken.',
    done: (n) => (k) => k.uitspraken >= n,
  },
  toezenden: {
    tab: 'andere',
    title: () => 'Toezenden',
    todo: 'Klik bij de uitspraak op Toezenden.',
    seen: () => 'Het bestuursorgaan legt de ontvangst vast (Awb 8:79).',
    done: (n) => (k) => k.ontvangsten >= n,
  },
  // The login step of the case (InlogScherm.vue). Done once the service
  // holds one more record of the fact of the login than before the scenario
  // for each login that got in; `n` counts those. A refused login records
  // nothing, so it is done once the screen kept its refusal (state.js).
  inloggen: {
    tab: 'aanvrager',
    title: (n) => (n === 1 ? 'Inloggen' : 'Opnieuw inloggen'),
    todo:
      'Klik boven het formulier op Inloggen (is er al iemand ingelogd, dan eerst op Uitloggen), vul de gebruikersnaam en het wachtwoord in en klik op Inloggen.',
    seen: () => 'Boven het formulier staat wie er is ingelogd, met wat de erkende dienst verklaarde.',
    done: (n, stap, vooraf) => (k) => k.executogrammen(feitNaam(stap.feit)) >= vooraf + n,
    weigering: new RegExp(`^${LOGIN_REFUSED}`),
  },
  // Nothing to do but reach the step's play date: the guide then asks the
  // cell for the answers the step expects (antwoorden.js).
  // The guide itself shows the answers; the tab is where the case's own
  // cell is, whose answers a scenario mostly checks.
  toetsen: {
    tab: 'bestuursorgaan',
    title: () => 'Toetsen',
    todo: 'Zet de speeldatum op de dag van deze stap; de gids vraagt de cel dan om haar antwoorden.',
    seen: () => 'De gids toont de antwoorden van de cel.',
    // The case refuses a check step without a play date (src/casus.rs).
    done: (n, stap) => (k) => (k.datum ?? '') >= stap.datum,
  },
  // A decision the case's cell takes ex officio (AmbtshalveBesluiten.vue),
  // under the article the step names. Done once the cell holds as many
  // decisions under that article as the scenario took so far.
  ambtshalve: {
    tab: 'bestuursorgaan',
    title: () => 'Ambtshalve besluiten',
    todo: 'Klik onder Ambtshalve besluiten op Ambtshalve besluiten.',
    seen: () => 'Het besluit staat onder Ambtshalve besluiten.',
    done: (n, stap) => (k) => k.ambtshalve(stap.artikel) >= n,
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

// `beleid#5#geneeskundig_advies_uitgebracht` → `geneeskundig_advies_uitgebracht`.
export const feitNaam = (feit) => String(feit).split('#').at(-1);

/**
 * The cell a record step records in: the one it names, or without one the
 * case's own cell (src/casus.rs, `vastleggende_cel`).
 */
export const vastleggendeCel = (stap, casus) => stap.cel ?? casus?.cel ?? null;

/**
 * How many records of `feit` the cells of `casus` hold before a scenario, by
 * the name of the fact. Given a cell, only those of that cell, and only of
 * that very fact (`<regelwerk>#<article>#<fact>`).
 */
const vooraf = (casus, feit, cel = null) =>
  [{ cel: casus?.cel, begin: casus?.begin }, ...(casus?.andere_cellen ?? [])]
    .filter((c) => cel === null || c.cel === cel)
    .flatMap((c) => c.begin?.executogrammen ?? [])
    .filter((e) => (cel === null ? feitNaam(e.feit) === feitNaam(feit) : e.feit === feit)).length;

const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;
const waarde = (v) =>
  typeof v === 'boolean' ? hoofdletter(jaNee(v)) : ISO_DATE.test(String(v)) ? longDate(v) : String(v);

/**
 * The values the step fills in, from its `invoer`, one line each: the guide
 * lists them, the form holds them, and both read them from the YAML. Empty
 * when the step adds none to the case's example values.
 */
export function invoerRegels(invoer) {
  return Object.entries(invoer ?? {}).map(([naam, v]) => `${leesbaar(naam)}: ${waarde(v)}`);
}

/**
 * The guide of `scenario` (a scenario of a case, as `api.casus` gives it),
 * with `casus` for what its other cells record before a scenario. Null
 * when the scenario has no action.
 */
export function bouwGids(scenario, casus = null) {
  const geteld = {};
  const feiten = {};
  // Per recording cell and fact, how many records of it the scenario made.
  const inCel = {};
  // Per kind, the steps the cell decided on and the steps it refused.
  const besloten = {};
  const geweigerd = {};
  const uitkomsten = {};
  // The instructions not yet given to an action.
  let notes = [];
  // The play date so far: a step without one plays on the date before it.
  let datum = casus?.speeldatum ?? null;
  const steps = scenario.stappen.flatMap((stap) => {
    if (typeof stap !== 'object' || stap === null) {
      notes.push(String(stap));
      return [];
    }
    const kind = KINDS[stap.stap];
    if (!kind) throw new Error(`unknown step '${stap.stap}' in "${scenario.naam}"`);
    if (stap.datum) datum = stap.datum;
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
    let done;
    if (stap.stap === 'vastleggen') done = kind.done(keerFeit, stap, vooraf(casus, stap.feit));
    else if (stap.stap === 'inloggen') {
      // A refused login is done when one more got in than the steps before
      // expect: that is the login the step expected to be refused.
      const feit = casus?.inloggen?.feit;
      const ingelogd = (besloten.inloggen ?? 0) + (stap.uitkomst === 'geweigerd' ? 1 : 0);
      done = kind.done(ingelogd, { feit }, vooraf(casus, feit));
    } else if (stap.stap === 'ambtshalve') {
      const sleutel = `ambtshalve ${stap.artikel}`;
      done = kind.done((geteld[sleutel] = (geteld[sleutel] ?? 0) + 1), stap);
    } else done = kind.done(stap.stap === 'besluit' ? doel : n, stap);
    // A record step of the case's own cell happens on the authority's tab.
    const soort =
      stap.stap === 'vastleggen' && casus && vastleggendeCel(stap, casus) === casus.cel ? { ...kind, ...kind.eigen } : kind;
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
    const step = {
      title: stap.titel ?? soort.title(stap.stap === 'besluit' ? doel : n),
      tab: soort.tab,
      date: stap.datum ?? undefined,
      story: stap.verhaal ?? '',
      todo: stap.tekst ?? soort.todo,
      values: invoerRegels(stap.invoer),
      seen: stap.gezien ?? soort.seen(stap.stap === 'besluit' ? doel : n),
      notes,
      done,
      expect,
      stap,
      doel,
      // For a record step: the how-many-th record of its fact in its cell
      // it is in the scenario, and how many that cell held before (scenarioInvoer.js finds
      // the record a later step refers to with these).
      keer:
        stap.stap === 'vastleggen'
          ? (inCel[`${vastleggendeCel(stap, casus)} ${stap.feit}`] =
              (inCel[`${vastleggendeCel(stap, casus)} ${stap.feit}`] ?? 0) + 1)
          : 0,
      vooraf: stap.stap === 'vastleggen' ? vooraf(casus, stap.feit, vastleggendeCel(stap, casus)) : 0,
      // The answers the scenario expects of a cell once the step is done,
      // and the play date it asks them on (antwoorden.js).
      antwoorden: stap.antwoorden ?? [],
      antwoordDatum: datum,
    };
    notes = [];
    return [step];
  });
  if (!steps.length) return null;
  return { name: scenario.naam, summary: scenario.samenvatting ?? '', steps, after: notes };
}

// One guide per scenario object, so the panel and the forms share it.
const gebouwd = new WeakMap();

/**
 * The guide for scenario `naam` of `casus`, from the PoC's own cases or
 * from a source alike, or null: a scenario without a single action, and a
 * name the case does not know, have none. The tab Scenario's shows the
 * lines of such a scenario instead.
 */
export function guideFor(casus, naam) {
  if (!casus) return null;
  const scenario = casus.scenarios?.find((s) => s.naam === naam);
  if (!scenario?.stappen?.length) return null;
  if (!gebouwd.has(scenario)) gebouwd.set(scenario, bouwGids(scenario, casus));
  return gebouwd.get(scenario);
}
