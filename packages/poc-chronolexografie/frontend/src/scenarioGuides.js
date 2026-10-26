/**
 * The scenarios of the cases, step by step, for the guide beside the page
 * (ScenarioGuide.vue). A guide belongs to a scenario of a case YAML
 * (`casussen/<id>.yaml`, `scenarios[].naam`) by its name; what the visitor
 * sees at the end stays the `eind` of that scenario. The steps here replace
 * the `stappen` of the YAML on the screen; a case from a source (NAPP) has no
 * guide and keeps those. tests/scenarioGuides.test.js keeps the names in step
 * with the YAML.
 *
 * A step: `title`, the `tab` it happens on, the play date it wants (`date`,
 * optional), what to do (`todo`), what the visitor sees once it is done
 * (`seen`, not on the last step), when it is done (`done`, a test on
 * scenarioProgress.counts()) and optionally what the scenario expects of it
 * (`expect`: a test and the text of the warning when it fails).
 *
 * The texts are hand-written and name the buttons and fields as the
 * components label them; change them together.
 */
import { refused } from './scenarioProgress.js';

// What the scenario expects of a decision, as warning text.
const expectation = (check, text) => ({ check, text });
// The n-th decision on an application grants, or rejects.
const granted = (n = 1) =>
  expectation((k) => k.decided[n - 1] === true, 'hier hoort het besluit de aanvraag toe te wijzen');
const rejected = (n = 1) =>
  expectation((k) => k.decided[n - 1] === false, 'hier hoort het besluit de aanvraag af te wijzen');
const upheld = (n = 1) =>
  expectation((k) => k.gegrond >= n, 'hier hoort het bezwaar gegrond te zijn');
const dismissed = (n = 1) =>
  expectation((k) => k.ongegrond >= n, 'hier hoort het bezwaar ongegrond te zijn');
const inadmissible = (n = 1) =>
  expectation((k) => k.nietOntvankelijk >= n, 'hier hoort het bezwaar niet-ontvankelijk te zijn');

// The steps every scenario is built from; a scenario overrides the texts.
const apply = ({ n = 1, ...o } = {}) => ({
  title: n === 1 ? 'Aanvraag indienen' : 'Nog een aanvraag indienen',
  tab: 'aanvrager',
  todo: 'Klik op Aanvraag indienen.',
  seen:
    n === 1
      ? 'De aanvraag staat onder Mijn aanvragen op In behandeling. De velden komen uit de wet (het artikel en Awb 4:2); de dag van ontvangst is de speeldatum.'
      : 'De tweede aanvraag staat onder Mijn aanvragen op In behandeling.',
  done: (k) => k.aanvragen >= n,
  ...o,
});

// `outcome` is what the scenario expects of this decision: 'granted' or
// 'rejected'.
const decide = ({ n = 1, outcome, ...o } = {}) => ({
  title: n === 1 ? 'Besluit nemen' : 'Besluiten op de tweede aanvraag',
  tab: 'bestuursorgaan',
  expect: outcome && { granted, rejected }[outcome](n),
  todo: 'Klik onder Open aanvragen op Besluit nemen.',
  seen: 'Het besluit staat onder Besluiten. Bij Aanvrager staat de aanvraag niet meer op In behandeling.',
  done: (k) => k.decided.length >= n,
  ...o,
});

const object = ({ n = 1, ...o } = {}) => ({
  title: n === 1 ? 'Bezwaar maken' : 'Nog een bezwaar maken',
  tab: 'aanvrager',
  todo: 'Klik bij het besluit op Bezwaar maken en dan op Bezwaar indienen.',
  seen: 'Het bezwaar staat in het dossier en verwijst naar het besluit. De cel nam het bestreden besluit en de termijn uit dat besluit. De aanvraag verandert niet.',
  done: (k) => k.bezwaren >= n,
  ...o,
});

const decideObjection = ({ n = 1, ...o } = {}) => ({
  title: 'Beslissen op bezwaar',
  tab: 'bestuursorgaan',
  todo: 'Klik onder Open bezwaren op Ontvankelijkheid toetsen, laat de bevindingen staan en klik op Beslissen op bezwaar.',
  seen: 'De beslissing staat onder Beslissingen op bezwaar.',
  done: (k) => k.beslissingen >= n,
  ...o,
});

const appeal = (o = {}) => ({
  title: 'Beroep instellen',
  tab: 'aanvrager',
  date: '2026-12-01',
  todo: 'Klik bij de beslissing op bezwaar op Beroep instellen en dan op Beroepschrift indienen.',
  seen: 'Het beroep staat in de kroniek van Rechtbank Wilgendaal, met het afschrift dat het bestuursorgaan van zijn beslissing maakte. Rechtbank en bestuursorgaan hebben elk een eigen kroniek; beide staan onder Kroniek.',
  done: (k) => k.beroepen >= 1,
  ...o,
});

const judge = (o = {}) => ({
  title: 'Uitspraak doen',
  tab: 'andere',
  date: '2027-01-11',
  todo: 'Klik onder Ingekomen beroepen op Uitspraak doen.',
  seen: 'De uitspraak staat onder Uitspraken, in de kroniek van de rechtbank. Het bestuursorgaan weet er nog niets van.',
  done: (k) => k.uitspraken >= 1,
  ...o,
});

const send = (o = {}) => ({
  title: 'Toezenden',
  tab: 'andere',
  todo: 'Klik bij de uitspraak op Toezenden.',
  seen: 'Het bestuursorgaan legt de ontvangst vast in zijn eigen kroniek (Awb 8:79).',
  done: (k) => k.ontvangsten >= 1,
  ...o,
});

// Vakantieverlof: one play date, 14 October 2026, unless a step says
// otherwise.
const OFF = 'Zet "specifieke aard beroep ouder" uit en klik op Besluit nemen.';

const vakantieverlof = [
  {
    name: 'Aanvraag wordt verleend',
    summary: 'Een ouder vraagt verlof in februari, en het hoofd verleent het.',
    steps: [
      apply({
        todo: 'Klik op Aanvraag indienen. De voorbeeldwaarden vragen verlof van 1 tot en met 5 februari 2027.',
      }),
      decide({
        todo: 'Laat "specifieke aard beroep ouder" aan staan en klik op Besluit nemen. Of het verlof buiten de schoolvakanties valt, vraagt het formulier niet: dat antwoordt de school zelf.',
        outcome: 'granted',
      }),
    ],
  },
  {
    name: 'Afwijzing, bezwaar en heroverweging: gegrond',
    summary:
      'Het hoofd weigert het verlof, maar na het bezwaar van de ouder krijgt de jongere bij heroverweging alsnog verlof.',
    steps: [
      apply(),
      decide({
        todo: 'Zet "specifieke aard beroep ouder" uit: het hoofd vindt het beroep niet bijzonder genoeg. Klik op Besluit nemen.',
        seen: 'Onder Besluiten staat Afgewezen; bij Aanvrager staat de aanvraag op Afgewezen.',
        outcome: 'rejected',
      }),
      object({
        date: '2026-11-01',
        todo: 'Klik bij het besluit op Bezwaar maken. De gronden noemen een verklaring van de werkgever. Klik op Bezwaar indienen.',
      }),
      decideObjection({
        date: '2026-11-15',
        todo: 'Klik onder Open bezwaren op Ontvankelijkheid toetsen: het bezwaar is ontvankelijk. Kies bij "specifieke aard beroep ouder" Ja en klik op Beslissen op bezwaar.',
        expect: upheld(),
      }),
    ],
  },
  {
    name: 'Afwijzing, bezwaar en heroverweging: ongegrond',
    summary:
      'Een tweede verlof in hetzelfde schooljaar wordt geweigerd, en het bezwaar daartegen houdt geen stand.',
    steps: [
      apply(),
      decide({
        todo: 'Klik op Besluit nemen.',
        seen: 'Het verlof is verleend. Bij de tweede aanvraag leest de school dit besluit terug.',
        outcome: 'granted',
      }),
      apply({
        n: 2,
        todo: 'Vul als eerste verlofdag 10 mei 2027 en als laatste verlofdag 14 mei 2027 in. Klik op Aanvraag indienen.',
      }),
      decide({
        n: 2,
        todo: 'Klik bij de tweede aanvraag op Besluit nemen.',
        seen: 'Het verlof is geweigerd: de school antwoordt uit haar eigen kroniek dat de jongere dit schooljaar al verlof kreeg. Klik onder Besluiten op Waarom? voor het spoor.',
        outcome: 'rejected',
      }),
      object({
        date: '2026-11-01',
        todo: 'Zoek de tweede aanvraag. Klik bij haar besluit op Bezwaar maken en dan op Bezwaar indienen.',
      }),
      decideObjection({
        date: '2026-11-15',
        todo: 'Klik onder Open bezwaren op Ontvankelijkheid toetsen. Laat de bevinding staan en klik op Beslissen op bezwaar.',
        expect: dismissed(),
      }),
    ],
  },
  {
    name: 'Bezwaar te laat: niet-ontvankelijk',
    summary: 'De ouder maakt pas na de bezwaartermijn bezwaar, en dat wordt niet heroverwogen.',
    steps: [
      apply(),
      decide({ todo: OFF, outcome: 'rejected' }),
      object({
        date: '2026-12-01',
        todo: 'De bezwaartermijn van zes weken is voorbij. Klik bij het besluit op Bezwaar maken en dan op Bezwaar indienen.',
        seen: 'De cel legt het bezwaar vast, ook al is het te laat. Of het op tijd is, blijkt pas bij Ontvankelijkheid toetsen (Awb 7:11).',
      }),
      decideObjection({
        todo: 'Klik onder Open bezwaren op Ontvankelijkheid toetsen: het bezwaar kwam te laat. Klik op Beslissen op bezwaar.',
        expect: inadmissible(),
      }),
    ],
  },
  {
    name: 'Tweede bezwaar tegen een vervangen besluit',
    summary: 'De ouder maakt twee keer bezwaar tegen hetzelfde besluit, en de cel vervangt een besluit maar één keer.',
    steps: [
      apply(),
      decide({ todo: 'Klik op Besluit nemen. Het verlof wordt verleend.', outcome: 'granted' }),
      object({ date: '2026-11-01' }),
      object({
        n: 2,
        date: '2026-11-02',
        todo: 'Klik nog een keer bij hetzelfde besluit op Bezwaar maken en dan op Bezwaar indienen.',
        seen: 'Er staan twee bezwaren tegen hetzelfde besluit open, onder Bestuursorgaan.',
      }),
      decideObjection({
        title: 'Beslissen op het eerste bezwaar',
        date: '2026-11-15',
        todo: 'Klik bij het eerste bezwaar op Ontvankelijkheid toetsen. Kies bij "specifieke aard beroep ouder" Nee en klik op Beslissen op bezwaar.',
        seen: 'Gegrond: een weigering komt in de plaats van het besluit. Het tweede bezwaar staat nog open.',
        expect: upheld(),
      }),
      {
        title: 'Beslissen op het tweede bezwaar',
        tab: 'bestuursorgaan',
        date: '2026-11-16',
        todo: 'Klik bij het tweede bezwaar op Ontvankelijkheid toetsen. Kies weer Nee bij "specifieke aard beroep ouder" en klik op Beslissen op bezwaar.',
        done: refused(/already replaces/),
      },
    ],
  },
  {
    name: 'Een herroepen verlof telt niet meer mee',
    summary: 'Wordt het eerste verlof herroepen, dan telt het niet meer bij het tweede.',
    steps: [
      apply({
        todo: 'Klik op Aanvraag indienen. Dit is verlof A, van 1 tot en met 5 februari 2027.',
      }),
      decide({ todo: 'Klik op Besluit nemen. Verlof A wordt verleend.', outcome: 'granted' }),
      apply({
        n: 2,
        todo: 'Vul als eerste verlofdag 10 mei 2027 en als laatste verlofdag 14 mei 2027 in en klik op Aanvraag indienen. Dit is verlof B.',
      }),
      decide({
        n: 2,
        todo: 'Klik op Besluit nemen.',
        seen: 'Verlof B is geweigerd: de school las A in haar kroniek, dus de jongere kreeg dit schooljaar al verlof.',
        outcome: 'rejected',
      }),
      object({
        date: '2026-11-01',
        todo: 'Zoek de eerste aanvraag (A). Klik bij haar besluit op Bezwaar maken en dan op Bezwaar indienen.',
      }),
      decideObjection({
        date: '2026-11-15',
        todo: 'Klik onder Open bezwaren op Ontvankelijkheid toetsen. Kies bij "specifieke aard beroep ouder" Nee en klik op Beslissen op bezwaar.',
        seen: 'Gegrond: A is herroepen, en een weigering komt in de plaats.',
        expect: upheld(),
      }),
      object({
        n: 2,
        date: '2026-11-20',
        title: 'Bezwaar maken tegen B',
        todo: 'Zoek de tweede aanvraag (B). Klik bij haar besluit op Bezwaar maken en dan op Bezwaar indienen.',
      }),
      decideObjection({
        n: 2,
        title: 'Beslissen op het bezwaar tegen B',
        date: '2026-12-01',
        todo: 'Klik onder Open bezwaren op Ontvankelijkheid toetsen, laat de bevinding staan en klik op Beslissen op bezwaar.',
        expect: upheld(2),
      }),
    ],
  },
  {
    name: 'Verlof in de kerstvakantie: geweigerd zonder oordeel van het hoofd',
    summary: 'Verlof in de kerstvakantie wordt geweigerd, omdat het vakantierooster van de school laat zien dat die dagen in een schoolvakantie vallen.',
    steps: [
      apply({
        todo: 'Vul als eerste verlofdag 21 december 2026 en als laatste verlofdag 23 december 2026 in. Klik op Aanvraag indienen.',
      }),
      decide({
        todo: 'Het hoofd geeft alleen "specifieke aard beroep ouder". Laat die aan staan en klik op Besluit nemen.',
        outcome: 'rejected',
      }),
    ],
  },
  {
    name: 'Verlof in een schooljaar dat de school nog niet kent: het hoofd oordeelt',
    summary: 'Staat het schooljaar nog niet in het vakantierooster, dan vraagt het formulier het hoofd of het verlof buiten de schoolvakanties valt.',
    steps: [
      apply({
        todo: 'Vul als eerste verlofdag 20 december 2027 en als laatste verlofdag 22 december 2027 in. Klik op Aanvraag indienen.',
      }),
      decide({
        todo: 'Het vakantierooster van de school reikt nog niet tot schooljaar 2027–2028. De voorvertoning meldt dat. Het formulier vraagt het hoofd nu of het verlof buiten de schoolvakanties valt. Kies bij "verlof valt buiten schoolvakanties" Nee en klik op Besluit nemen.',
        outcome: 'rejected',
      }),
    ],
  },
  {
    name: 'Bezwaar ongegrond, beroep gegrond, nieuw besluit op bezwaar',
    summary:
      'Na een ongegrond bezwaar gaat de ouder in beroep; de rechter vernietigt en het hoofd beslist opnieuw.',
    steps: [
      apply(),
      decide({ todo: OFF, outcome: 'rejected' }),
      object({ date: '2026-11-01' }),
      decideObjection({
        date: '2026-11-15',
        todo: 'Klik onder Open bezwaren op Ontvankelijkheid toetsen. Laat de bevinding staan en klik op Beslissen op bezwaar.',
        seen: 'Ongegrond: de weigering blijft. Bij Aanvrager kun je nu in beroep gaan.',
        expect: dismissed(),
      }),
      appeal(),
      judge({
        todo: 'Klik onder Ingekomen beroepen op Uitspraak doen. Volgens de voorbeeldbevindingen schendt de beslissing artikel 7:12, eerste lid, Awb: de motivering is niet deugdelijk. De rechtsgevolgen blijven niet in stand. De school heeft tot en met 25 januari 2027 voor een nieuwe beslissing.',
      }),
      send({
        todo: 'Klik bij de uitspraak op Toezenden aan OBS De Klimop.',
        seen: 'De school legt de ontvangst vast in haar eigen kroniek (Awb 8:79). Onder Bestuursorgaan staat nu Na een uitspraak.',
      }),
      {
        title: 'Opnieuw beslissen',
        tab: 'bestuursorgaan',
        date: '2027-01-18',
        todo: 'Klik onder Na een uitspraak op Heroverwegen, kies bij "specifieke aard beroep ouder" Ja en klik op Opnieuw beslissen op bezwaar.',
        done: (k) => k.redecisions >= 1,
        expect: upheld(),
      },
    ],
  },
];

// Gehandicaptenparkeerkaart: the application on 13 October 2026, the
// decision a week later.
const pDecide = (o) => decide({ date: '2026-10-20', ...o });
const withBsn = (number, note) => `Verander het BSN in ${number} en klik op Aanvraag indienen. ${note}`;
const ASK_CRITERIA =
  'Er is geen advies, dus het formulier vraagt de vier criteria één voor één aan het college. Kies Ja bij "langdurige loopbeperking" en Nee bij de andere drie.';
const CRITERIA = `${ASK_CRITERIA} Zet "geneeskundig onderzoek achterwege gelaten" en "bekend dat aanvrager nog steeds voldoet" aan. Klik op Besluit nemen.`;
const NO_ADVICE = '999990068';

const gehandicaptenparkeerkaart = [
  {
    name: 'De kaart wordt verstrekt op het advies van de GGD',
    summary: 'De GGD adviseert positief, en het college verstrekt de kaart op dat advies.',
    steps: [
      apply({
        todo: 'Klik op Aanvraag indienen. De voorbeeldwaarden vragen een kaart als bestuurder.',
        seen: 'De aanvraag staat onder Mijn aanvragen op In behandeling. De GGD onderzocht deze aanvrager op 9 oktober; dat onderzoek en het advies staan in de kroniek van de GGD, onder Andere cellen.',
      }),
      pDecide({
        todo: 'Het formulier vraagt de vier criteria niet: het advies van de GGD geeft ze. Klik op Besluit nemen.',
        outcome: 'granted',
      }),
    ],
  },
  {
    name: 'Het advies vindt geen criterium',
    summary: 'Het advies van de GGD vindt geen enkel criterium, en de kaart wordt geweigerd.',
    steps: [
      apply({ todo: withBsn(NO_ADVICE, 'De GGD onderzocht deze aanvrager op 9 oktober en vond geen enkel criterium.') }),
      pDecide({ todo: 'Klik op Besluit nemen.', outcome: 'rejected' }),
    ],
  },
  {
    name: 'Een tweede kaart naast een geldige',
    summary: 'Wie al een geldige kaart heeft, krijgt er geen tweede bij.',
    steps: [
      apply(),
      pDecide({
        todo: 'Klik op Besluit nemen.',
        seen: 'De kaart is verstrekt, geldig tot en met 19 oktober 2031.',
        outcome: 'granted',
      }),
      apply({ n: 2, date: '2027-03-01', todo: 'Klik nog een keer op Aanvraag indienen.' }),
      pDecide({
        n: 2,
        date: undefined,
        todo: 'Klik op Besluit nemen. Het beleid van de gemeente geeft geen tweede kaart naast een geldige.',
        outcome: 'rejected',
      }),
    ],
  },
  {
    name: 'Ongegrond bezwaar tegen een verstrekking',
    summary: 'De aanvrager maakt bezwaar tegen de verstrekte kaart, en de heroverweging geeft dezelfde uitkomst.',
    steps: [
      apply(),
      pDecide({
        todo: 'Klik op Besluit nemen.',
        seen: 'De kaart is verstrekt, geldig tot en met 19 oktober 2031.',
        outcome: 'granted',
      }),
      object({ date: '2026-11-01' }),
      decideObjection({
        date: '2026-11-20',
        todo: 'Klik onder Open bezwaren op Ontvankelijkheid toetsen. Laat alle bevindingen staan en klik op Beslissen op bezwaar.',
        expect: dismissed(),
      }),
    ],
  },
  {
    name: 'Gegrond bezwaar na een nieuw advies',
    summary: 'Na het bezwaar onderzoekt de GGD opnieuw, en op dat nieuwe advies komt er alsnog een kaart.',
    steps: [
      apply({ todo: withBsn(NO_ADVICE, 'Het advies van de GGD vindt geen criterium.') }),
      pDecide({
        todo: 'Klik op Besluit nemen.',
        seen: 'De kaart is geweigerd: het advies vindt geen criterium.',
        outcome: 'rejected',
      }),
      object({ date: '2026-11-01' }),
      {
        title: 'Nieuw advies van de GGD',
        tab: 'andere',
        date: '2026-11-10',
        todo: `De GGD brengt een nieuw advies uit. Vul bij GGD Wilgendaal onder "Vastleggen: Geneeskundig advies uitgebracht" BSN ${NO_ADVICE} en datum onderzoek 10 november 2026 in. Zet alleen "langdurige loopbeperking" aan en klik op Vastleggen.`,
        seen: 'Het nieuwe advies staat in de kroniek van de GGD; de gemeente leest het bij de heroverweging.',
        done: (k) =>
          k.executogrammen('geneeskundig_advies_uitgebracht', { bsn_aanvrager: NO_ADVICE }) >= 2,
      },
      decideObjection({
        date: '2026-11-20',
        todo: 'Klik onder Open bezwaren op Ontvankelijkheid toetsen en dan op Beslissen op bezwaar.',
        expect: upheld(),
      }),
    ],
  },
  {
    name: 'Zonder advies oordeelt het college, en zonder onderzoek nog geen kaart',
    summary: 'Zonder advies oordeelt het college zelf, maar zonder onderzoek komt er nog geen kaart.',
    steps: [
      apply({ todo: withBsn('999990032', 'De GGD heeft deze aanvrager niet onderzocht.') }),
      {
        title: 'Besluit nemen zonder oordeel over het onderzoek',
        tab: 'bestuursorgaan',
        date: '2026-10-20',
        todo: `${ASK_CRITERIA} Laat "geneeskundig onderzoek heeft plaatsgehad" op Nog niet beoordeeld en klik op Besluit nemen.`,
        done: (k) => refused(/geneeskundig_onderzoek_heeft_plaatsgehad/)(k) || k.decided.length >= 1,
        expect: expectation((k) => k.decided.length === 0, 'hier hoort de cel de stap te weigeren, zonder besluit'),
      },
    ],
  },
  {
    name: 'Een nieuwe kaart zonder nieuw onderzoek',
    summary: 'Wie eerder een kaart kreeg, krijgt een nieuwe zonder nieuw onderzoek.',
    steps: [
      apply(),
      pDecide({
        todo: 'Klik op Besluit nemen.',
        seen: 'De kaart is verstrekt, geldig tot en met 19 oktober 2031.',
        outcome: 'granted',
      }),
      apply({
        n: 2,
        date: '2031-10-20',
        todo: 'Zet de dagtekening op 20 oktober 2031 en klik op Aanvraag indienen. Het onderzoek en het advies van 2026 tellen niet voor deze aanvraag.',
      }),
      pDecide({ n: 2, date: undefined, todo: CRITERIA, outcome: 'granted' }),
    ],
  },
  {
    name: 'Een kaart van een andere gemeente telt mee',
    summary: 'Een eerdere kaart van een andere gemeente telt mee, dus het onderzoek mag achterwege blijven.',
    steps: [
      apply({
        todo: withBsn('999990044', 'Deze aanvrager kreeg in januari 2026 een kaart van de (fictieve) gemeente Oostmeer en woont nu in Wilgendaal.'),
      }),
      pDecide({ todo: CRITERIA, outcome: 'granted' }),
    ],
  },
  {
    name: 'Een kaart van het CBR telt ook',
    summary: 'Een eerdere kaart van het CBR telt net zo mee.',
    steps: [
      apply({ todo: withBsn('999990056', 'Het CBR gaf deze aanvrager in februari 2026 een kaart.') }),
      pDecide({ todo: CRITERIA, outcome: 'granted' }),
    ],
  },
  {
    name: 'Twee uitgevers, één antwoord',
    summary: 'Twee cellen zeggen dat er eerder een kaart was, en de wet maakt er één antwoord van.',
    steps: [
      apply({
        todo: withBsn('999990044', 'Deze aanvrager kreeg in januari 2026 een kaart van de gemeente Oostmeer.'),
      }),
      pDecide({ todo: CRITERIA, outcome: 'granted' }),
      apply({
        n: 2,
        date: '2031-10-20',
        todo: 'De kaart van Wilgendaal is verlopen. Verander het BSN in 999990044, zet de dagtekening op 20 oktober 2031 en klik op Aanvraag indienen.',
      }),
      pDecide({ n: 2, date: undefined, todo: CRITERIA, outcome: 'granted' }),
    ],
  },
  {
    name: 'Beroep ongegrond',
    summary: 'Na een ongegrond bezwaar gaat de aanvrager in beroep, en de rechter geeft de gemeente gelijk.',
    steps: [
      apply({ todo: withBsn(NO_ADVICE, 'Het advies van de GGD vindt geen criterium.') }),
      pDecide({ todo: 'Klik op Besluit nemen.', outcome: 'rejected' }),
      object({ date: '2026-11-01' }),
      decideObjection({
        date: '2026-11-20',
        todo: 'Klik onder Open bezwaren op Ontvankelijkheid toetsen. Laat alle bevindingen staan en klik op Beslissen op bezwaar.',
        seen: 'Ongegrond: de weigering blijft. Bij Aanvrager kun je nu in beroep gaan.',
        expect: dismissed(),
      }),
      appeal(),
      judge({
        todo: 'Klik onder Ingekomen beroepen op Uitspraak doen. De voorbeeldbevindingen zeggen dat de gemeente geen rechtsregel schond.',
      }),
      send({ todo: 'Klik bij de uitspraak op Toezenden aan gemeente Wilgendaal.' }),
    ],
  },
];

/** The guides per case id. */
export const guides = { vakantieverlof, gehandicaptenparkeerkaart };

/**
 * The guide for scenario `naam` of `casus`, or null: a case from a source
 * (`casus.bron`, such as NAPP) has none, and neither has a scenario that no
 * guide names. Those show the steps from the YAML instead.
 */
export function guideFor(casus, naam) {
  if (!casus || casus.bron) return null;
  return guides[casus.id]?.find((g) => g.name === naam) ?? null;
}
