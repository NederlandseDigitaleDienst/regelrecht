// The kern of the chronolexografie poc, compiled to WebAssembly
// (packages/poc-chronolexografie/src/wasm.rs), on the regelwerken of one
// RegelHet: the app computes here, on this device, what the executor would
// decide.
//
// 1. Load: the source of the RegelHet (`GET /fsc4b/regelwerken`) as the one
//    source of an empty bundle.
// 2. Replay: the dossier records the signed data rests on, per cell, as
//    the chronicles of the case's cells (`herstellen`, which checks each
//    chronicle as one read from a file). Decisions come along, which
//    recording a fact (`vastleggen`) could not do.
// 3. Compare: the kern computes the same answers the executor signed
//    (`brongegevens`); every answer that differs is shown.
// 4. Predict: the application is submitted here (`indienen`) and decided on
//    trial (`voorBeslissen`). A fact that only comes after an application,
//    such as the GGD's advice, does not exist yet: the app tries each thing
//    it can say, and the prediction says what follows from each.
//
// What the citizen fills in reaches only this kern until they apply.
import { leesbaar } from '@regelrecht/frontend-shared/chronolex/waarden';
import type { Veld } from '../../gedeeld/types';
import type { Antwoord, Gegevens, NaAanvraag, Record_ } from './gegevens';
import { canoniek } from './gegevens';

/** The methods of the kern's `Poc` this app calls; each answers JSON text and throws an `Error` when the cell refuses. */
export interface PocWasm {
  casussen(): string;
  kies(casus: string): string;
  vormen(datum: string): string;
  indienen(artikel: string, velden: string, datum: string, op?: string): string;
  vastleggen(cel: string, feit: string, velden: string, datum: string, verwijstNaar?: string): string;
  voorBeslissen(op: string, oordeel: string, datum: string, na?: string): string;
  brongegevens(inlogveld: string, bsn: string, datum: string): string;
  bewaren(): string;
  herstellen(bewaard: string): string;
  free?(): void;
}

/** Makes a `Poc` on a bundle and its sources, both JSON text: the WASM constructor, or a fake one in a test. */
export type MaakPoc = (bundel: string, bronnen: string) => PocWasm;

/** The name of the source in the kern; the server loads its case under the same one (packages/regelhet/src/case.rs). */
const BRON = 'regelhet';

/** One answer side by side: what the executor signed and what this device computed. */
export interface Vergelijking {
  naam: string;
  ondertekend: unknown;
  lokaal: unknown;
  /** Whether both say the same; a silent cell (`null`) is no `false`. */
  gelijk: boolean;
}

/** Every answer side by side, and whether both sides read the same versions of the regelwerken. */
export interface Vergelijkingen {
  antwoorden: Vergelijking[];
  regelwerkenGelijk: boolean;
  /** Whether neither side gave any answer, so there was nothing to check. */
  leeg: boolean;
}

export type Uitkomst = 'toegekend' | 'afgewezen' | 'onbekend';

/** One branch of a prediction: what a cell would have to establish, and what follows. */
export interface Tak {
  /** The cell that establishes the fact, by its name in the case. */
  celNaam: string;
  /** What the step is called while it waits, such as "Geneeskundig advies uitbrengen". */
  taak: string;
  /** What the cell establishes in this branch, readable; empty when it establishes none of it. */
  vastgesteld: string[];
  /** The branch of every other thing the cell can establish, all with this outcome. */
  anders?: boolean;
  uitkomst: Uitkomst;
  /** Why the kern could not decide, when it could not. */
  reden?: string;
}

/** What the decision on the application would be. */
export interface Voorspelling {
  /** Without anything that comes after the application: the outcome as things stand. */
  nu: { uitkomst: Uitkomst; reden?: string };
  /**
   * Per fact after the application, what its cell must establish for each
   * outcome: the least that grants, and what follows otherwise.
   */
  takken: Tak[];
  /** Whether every branch comes out the same, so the outcome does not depend on what is still to come. */
  zeker: boolean;
}

/** A field of a fact, as the kern gives the forms (`vormen`). */
interface Handeling {
  feit: string;
  velden: Veld[];
}
interface CelHandelingen {
  cel: string;
  cel_naam?: string;
  handelingen: Handeling[];
}

/** Why the kern stopped: a panic aborts the WebAssembly instance, after which every call fails. */
class Gestopt extends Error {}

export class Rekenkern {
  private gestopt: string | null = null;

  private constructor(
    private readonly poc: PocWasm,
    readonly casus: { id: string; cel: string; cel_naam: string; uitkomst: string; andere_cellen: { cel: string; cel_naam: string }[] },
  ) {}

  /**
   * The kern on the source `bestanden` of a RegelHet, with the case `casus`
   * chosen (a source may hold more than one). A source that does not load,
   * or that lacks the case, fails here with what the kern said.
   */
  static laad(maak: MaakPoc, bestanden: Record<string, string>, casus: string): Rekenkern {
    const poc = maak('{}', JSON.stringify([{ naam: BRON, bestanden }]));
    const lijst = JSON.parse(poc.casussen()) as { casussen: { id: string }[]; bronnen: { fout: string | null; fouten: string[] }[] };
    const bron = lijst.bronnen[0];
    if (bron?.fout) throw new Error(`de regelwerken laden niet: ${bron.fout}`);
    if (bron?.fouten.length) throw new Error(`de casus laadt niet: ${bron.fouten.join('; ')}`);
    if (!lijst.casussen.some((c) => c.id === casus)) {
      throw new Error(`de regelwerken bevatten de casus ${casus} niet (wel: ${lijst.casussen.map((c) => c.id).join(', ') || 'geen'})`);
    }
    return new Rekenkern(poc, JSON.parse(poc.kies(casus)) as Rekenkern['casus']);
  }

  /** A step of the kern, as JSON. */
  private stap<T>(f: (p: PocWasm) => string): T {
    if (this.gestopt) throw new Gestopt(this.gestopt);
    try {
      return JSON.parse(f(this.poc)) as T;
    } catch (e) {
      if (e instanceof WebAssembly.RuntimeError) {
        this.gestopt = `de rekenkern is gestopt: ${e.message}`;
        throw new Gestopt(this.gestopt);
      }
      throw e;
    }
  }

  /**
   * The chronicles of the case's cells from `records`, the citizen's dossier
   * records, grouped per cell in the case's order and kept in their own
   * order. A record of a cell the case does not know is an error: the kern
   * could not tell what it is about.
   */
  naspelen(records: Record_[]): void {
    const cellen = [this.casus.cel, ...this.casus.andere_cellen.map((a) => a.cel)];
    const vreemd = records.find((r) => !cellen.includes(r.recording_actor));
    if (vreemd) throw new Error(`een record van ${vreemd.recording_actor}, een cel die deze casus niet kent`);
    const kronieken = cellen.map((cel) => records.filter((r) => r.recording_actor === cel));
    this.stap((p) => p.herstellen(JSON.stringify({ casus: this.casus.id, kronieken })));
  }

  /**
   * The answers the executor signed beside what this kern computes from the
   * replayed records on the same day, for the same BSN, with the same code
   * (`brongegevens` in packages/poc-chronolexografie/src/brongegevens.rs);
   * and whether both computed on the same versions of the regelwerken.
   * `inlogveld` is the field the login fills, which the answers take.
   */
  vergelijk(g: Gegevens, inlogveld: string): Vergelijkingen {
    const lokaal = this.stap<{ antwoorden: Record<string, Antwoord>; regelwerken: unknown[] }>((p) =>
      p.brongegevens(inlogveld, g.sub, g.peildatum),
    );
    const alle = [...new Set([...Object.keys(g.antwoorden), ...Object.keys(lokaal.antwoorden)])].sort();
    const antwoorden = alle.map((naam) => {
      const ondertekend = g.antwoorden[naam]?.waarde;
      const hier = lokaal.antwoorden[naam]?.waarde;
      const gelijk = naam in g.antwoorden && naam in lokaal.antwoorden && canoniek(ondertekend) === canoniek(hier);
      return { naam, ondertekend, lokaal: hier, gelijk };
    });
    const versies = (lijst: unknown[]) => lijst.map(canoniek).sort().join('\n');
    return { antwoorden, regelwerkenGelijk: versies(g.regelwerken) === versies(lokaal.regelwerken), leeg: alle.length === 0 };
  }

  /** The outcome of a trial decision on `op`, from the output the case names as its outcome. */
  private uitkomst(op: string, datum: string): { uitkomst: Uitkomst; reden?: string } {
    try {
      const proef = this.stap<{ uitvoer: Record<string, unknown> }>((p) => p.voorBeslissen(op, '{}', datum));
      const w = proef.uitvoer[this.casus.uitkomst];
      return { uitkomst: w === true ? 'toegekend' : w === false ? 'afgewezen' : 'onbekend' };
    } catch (e) {
      if (e instanceof Gestopt) throw e;
      return { uitkomst: 'onbekend', reden: e instanceof Error ? e.message : String(e) };
    }
  }

  /**
   * What the decision on an application under `artikel` with `velden` would
   * be on `datum`, for the citizen with the login field `inlogveld` set to
   * `bsn`, given the facts `bijAanvraag` that come after it. Nothing stays
   * in the kern: it is put back as it was, whatever happens.
   */
  voorspel(invoer: { artikel: string; velden: Record<string, unknown>; datum: string; bijAanvraag: NaAanvraag[]; inlogveld: string; bsn: string }): Voorspelling {
    const { artikel, velden, datum } = invoer;
    const begin = JSON.stringify(this.stap<unknown>((p) => p.bewaren()));
    let gelukt = false;
    try {
      const vormen = this.stap<{ eigen: CelHandelingen; andere_cellen: CelHandelingen[] }>((p) => p.vormen(datum));
      const ingediend = this.stap<{ id: string }>((p) => p.indienen(artikel, JSON.stringify(velden), datum));
      const naIndienen = JSON.stringify(this.stap<unknown>((p) => p.bewaren()));
      const nu = this.uitkomst(ingediend.id, datum);
      const takken: Tak[] = [];
      for (const na of invoer.bijAanvraag) {
        takken.push(...this.takken(na, [vormen.eigen, ...vormen.andere_cellen], naIndienen, ingediend.id, invoer));
      }
      const uitkomsten = new Set(takken.length ? takken.map((t) => t.uitkomst) : [nu.uitkomst]);
      const voorspelling = { nu, takken, zeker: uitkomsten.size === 1 && !uitkomsten.has('onbekend') };
      gelukt = true;
      return voorspelling;
    } finally {
      // Put the kern back. When the prediction itself failed, its error is
      // the one to report, not a second one from putting it back.
      try {
        this.stap((p) => p.herstellen(begin));
      } catch (e) {
        if (gelukt || e instanceof Gestopt) throw e;
      }
    }
  }

  /**
   * The branches of the fact `na`: every combination of what its cell can
   * establish is recorded on a copy and decided on, and the outcomes are
   * summed up as the least that grants and what follows otherwise
   * (`samenvatten`).
   */
  private takken(na: NaAanvraag, cellen: CelHandelingen[], naIndienen: string, op: string, invoer: { datum: string; inlogveld: string; bsn: string }): Tak[] {
    const cel = cellen.find((c) => c.handelingen?.some((h) => h.feit === na.feit));
    const handeling = cel?.handelingen.find((h) => h.feit === na.feit);
    const celNaam = (cel && (cel.cel_naam ?? this.celNaam(cel.cel))) ?? 'de uitvoerder';
    const onbekend = (reden: string): Tak[] => [{ celNaam, taak: na.taak, vastgesteld: [], uitkomst: 'onbekend', reden }];
    if (!cel || !handeling) return onbekend(`de casus kent geen cel die ${na.feit} vastlegt`);
    const mogelijk = hypothesen(handeling.velden, invoer);
    if ('reden' in mogelijk) return onbekend(mogelijk.reden.replace('{cel}', celNaam));
    const uitkomsten = mogelijk.hypothesen.map(({ velden, vastgesteld }) => {
      this.stap((p) => p.herstellen(naIndienen));
      try {
        this.stap((p) => p.vastleggen(cel.cel, na.feit, JSON.stringify(velden), invoer.datum));
      } catch (e) {
        if (e instanceof Gestopt) throw e;
        return { vastgesteld, uitkomst: 'onbekend' as const, reden: e instanceof Error ? e.message : String(e) };
      }
      return { vastgesteld, ...this.uitkomst(op, invoer.datum) };
    });
    return samenvatten(uitkomsten).map((t) => ({ celNaam, taak: na.taak, ...t }));
  }

  private celNaam(cel: string): string {
    return cel === this.casus.cel ? this.casus.cel_naam : (this.casus.andere_cellen.find((a) => a.cel === cel)?.cel_naam ?? cel);
  }

  /** Free the WebAssembly memory of the kern. */
  sluit(): void {
    this.poc.free?.();
  }
}

/** The most yes-or-no fields of one fact the app tries in every combination: 2^4 trial decisions. */
export const MAX_JANEE = 4;

/**
 * What a cell could establish in a fact with the fields `velden`: every
 * combination of its yes-or-no fields. The login field gets the citizen's
 * number and a date the day of the prediction. A field of another kind the
 * app cannot guess, and more than `MAX_JANEE` yes-or-no fields it does not
 * try; then there is no hypothesis, and a reason (`{cel}` stands for the
 * cell).
 */
export function hypothesen(
  velden: Veld[],
  { datum, inlogveld, bsn }: { datum: string; inlogveld: string; bsn: string },
): { hypothesen: { velden: Record<string, unknown>; vastgesteld: string[] }[] } | { reden: string } {
  const vast: Record<string, unknown> = {};
  const janee: string[] = [];
  for (const v of velden) {
    if (v.type === 'boolean') janee.push(v.naam);
    else if (v.naam === inlogveld) vast[v.naam] = bsn;
    else if (v.type === 'date') vast[v.naam] = datum;
    else if (!v.mag_leeg) return { reden: `de app kan niet raden wat {cel} invult bij ${leesbaar(v.naam)}` };
  }
  if (janee.length > MAX_JANEE) return { reden: `{cel} kan te veel verschillende dingen vaststellen om ze allemaal te proberen` };
  const uit: { velden: Record<string, unknown>; vastgesteld: string[] }[] = [];
  for (let masker = 0; masker < 1 << janee.length; masker += 1) {
    const waar = janee.filter((_, i) => masker & (1 << i));
    uit.push({
      velden: { ...vast, ...Object.fromEntries(janee.map((n) => [n, waar.includes(n)])) },
      vastgesteld: waar.map((n) => leesbaar(n)),
    });
  }
  return { hypothesen: uit };
}

type Uitgerekend = { vastgesteld: string[]; uitkomst: Uitkomst; reden?: string };

const deelVan = (a: string[], b: string[]): boolean => a.every((x) => b.includes(x));

/**
 * The outcomes of every combination, summed up for a reader: each least
 * combination that grants (no smaller one grants too), and then what follows
 * from everything else, as one branch when it all comes out the same (when
 * nothing grants, that branch is every combination) and otherwise one branch
 * per combination. A combination that grants because a
 * smaller one does needs no line of its own.
 */
export function samenvatten(uitgerekend: Uitgerekend[]): Omit<Tak, 'celNaam' | 'taak'>[] {
  const toegekend = uitgerekend.filter((u) => u.uitkomst === 'toegekend');
  const minimaal = toegekend.filter((u) => !toegekend.some((v) => v !== u && v.vastgesteld.length < u.vastgesteld.length && deelVan(v.vastgesteld, u.vastgesteld)));
  const gedekt = (u: Uitgerekend) => u.uitkomst === 'toegekend' && minimaal.some((m) => deelVan(m.vastgesteld, u.vastgesteld));
  const rest = uitgerekend.filter((u) => !gedekt(u));
  const eerste = rest[0];
  if (eerste && rest.every((u) => u.uitkomst === eerste.uitkomst)) {
    return [...minimaal, { vastgesteld: [], anders: true, uitkomst: eerste.uitkomst, ...(eerste.reden ? { reden: eerste.reden } : {}) }];
  }
  return [...minimaal, ...rest];
}

/**
 * The kern from the WASM build the app serves under `basis` (`wasm/pkg/`,
 * copied there at build time by vite.config.ts). The JS glue of
 * wasm-bindgen comes in through a blob URL, as in the chronolexografie poc,
 * because Vite allows no import from the static files.
 */
export async function laadWasm(basis: string): Promise<MaakPoc> {
  const lijm = await fetch(`${basis}wasm/pkg/regelrecht_poc_chronolexografie.js`);
  if (!lijm.ok) throw new Error(`de rekenkern laadt niet (${lijm.status})`);
  const url = URL.createObjectURL(new Blob([await lijm.text()], { type: 'application/javascript' }));
  try {
    const wasm = (await import(/* @vite-ignore */ url)) as {
      default: (o: { module_or_path: string }) => Promise<unknown>;
      Poc: new (bundel: string, bronnen: string) => PocWasm;
    };
    await wasm.default({ module_or_path: `${basis}wasm/pkg/regelrecht_poc_chronolexografie_bg.wasm` });
    return (bundel, bronnen) => new wasm.Poc(bundel, bronnen);
  } finally {
    URL.revokeObjectURL(url);
  }
}
