// The lists the work queue sorts its rows into (WerkvoorraadView.vue). Te
// doen holds everything whose next step is on the executor's side, Wacht op
// anderen what waits on someone else, such as an external party's answer to a
// request or an act the system takes itself. Na toekenning shows the grants
// whose acts are not all done, how far each is, whoever takes the next act.

/** The lists, in the order the sidebar shows them. */
export const LIJSTEN = [
  {
    naam: 'te-doen',
    tekst: 'Te doen',
    icoon: 'to-do',
    uitleg: 'Alles waarvan de volgende stap hier ligt, wat het langst wacht bovenaan.',
  },
  {
    naam: 'na-toekenning',
    tekst: 'Na toekenning',
    icoon: 'checked-text-document',
    uitleg: 'De toekenningen waarvan nog niet elke stap is gezet.',
  },
  {
    naam: 'wacht',
    tekst: 'Wacht op anderen',
    icoon: 'time',
    uitleg: 'Wat wacht op het antwoord van een andere partij, of op een stap die het systeem zelf zet.',
  },
] as const;

export type Lijst = (typeof LIJSTEN)[number]['naam'];

/** The list a query value names; Te doen when it names none. */
export function lijstVan(waarde: unknown): Lijst {
  return LIJSTEN.find((l) => l.naam === waarde)?.naam ?? 'te-doen';
}

/** The list a submission without a decision is in: to do, unless it waits on the answer to a request. */
export function lijstenVanIndiening(wachtOpAntwoord: boolean): Lijst[] {
  return wachtOpAntwoord ? ['wacht'] : ['te-doen'];
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
