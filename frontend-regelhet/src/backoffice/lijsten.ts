// The lists the work queue sorts its rows into (WerkvoorraadView.vue). Te
// doen holds everything whose next step is on the executor's side, Wacht op
// anderen what waits on someone else, such as an external party's answer to a
// request or an act the system takes itself. Na toekenning shows the grants
// whose acts are not all done, how far each is, whoever takes the next act.
// Alle aanvragen holds every submission that came in, decided or not, the
// newest first: the place to find one again.

/** The lists, in the order the sidebar shows them. */
export const LIJSTEN = [
  {
    naam: 'te-doen',
    tekst: 'Te doen',
    icoon: 'to-do',
    uitleg: 'Alles waarvan de volgende stap hier ligt, wat het langst wacht bovenaan.',
    // The view says this one itself, with what still waits on someone else.
    leeg: { tekst: 'U bent helemaal klaar', uitleg: '' },
  },
  {
    naam: 'na-toekenning',
    tekst: 'Na toekenning',
    icoon: 'checked-text-document',
    uitleg: 'De toekenningen waarvan nog niet elke stap is gezet.',
    leeg: {
      tekst: 'Geen toekenning onderweg',
      uitleg: 'Een toekenning staat hier tot elke stap erna is gezet.',
    },
  },
  {
    naam: 'wacht',
    tekst: 'Wacht op anderen',
    icoon: 'time',
    uitleg: 'Wat wacht op het antwoord van een andere partij, of op een stap die het systeem zelf zet.',
    leeg: {
      tekst: 'Er wacht niets op een ander',
      uitleg: 'Alles wat open staat, ligt bij u.',
    },
  },
  {
    naam: 'alle',
    tekst: 'Alle aanvragen',
    icoon: 'inbox',
    uitleg: 'Elke indiening die is binnengekomen, besloten of niet, de nieuwste bovenaan.',
    leeg: {
      tekst: 'Nog niets binnengekomen',
      uitleg: 'Een indiening die binnenkomt, staat hier.',
    },
  },
] as const;

export type Lijst = (typeof LIJSTEN)[number]['naam'];

/** The list a query value names; Te doen when it names none. */
export function lijstVan(waarde: unknown): Lijst {
  return LIJSTEN.find((l) => l.naam === waarde)?.naam ?? 'te-doen';
}

/**
 * The lists a submission is in: every one is in Alle aanvragen, and one
 * without a decision is also to do, unless it waits on the answer to a request.
 */
export function lijstenVanIndiening(open: boolean, wachtOpAntwoord: boolean): Lijst[] {
  if (!open) return ['alle'];
  return wachtOpAntwoord ? ['wacht', 'alle'] : ['te-doen', 'alle'];
}

/**
 * The lists a grant whose acts are not all done is in: to do unless the
 * system takes its next act.
 */
export function lijstenVanToekenning(doorHetSysteem: boolean): Lijst[] {
  return doorHetSysteem ? ['na-toekenning', 'wacht'] : ['te-doen', 'na-toekenning'];
}

/** The rows that waited longest first, by the day they wait since; equal days by key, so the order holds between loads. */
export function oudsteEerst<T extends { sinds: string; sleutel: string }>(rijen: T[]): T[] {
  return [...rijen].sort((a, b) => a.sinds.localeCompare(b.sinds) || a.sleutel.localeCompare(b.sleutel));
}

/** The rows that came in last first, by the day they came in; equal days by key, so the order holds between loads. */
export function nieuwsteEerst<T extends { ontvangen: string; sleutel: string }>(rijen: T[]): T[] {
  return [...rijen].sort((a, b) => b.ontvangen.localeCompare(a.ontvangen) || a.sleutel.localeCompare(b.sleutel));
}
