/**
 * Wat een wet uit de besloten zaken van een andere wet leest.
 *
 * Los van graph/lawGraph.ts en zonder afhankelijkheden: de corpuslader zet dit
 * bij elke wet, en zou anders de graafbibliotheek in de hoofdbundel trekken.
 */

import type { ExecutionLike, LawDocLike } from './format';

/** A law entry, as far as this module reads it. */
interface CaseRefLaw {
  id: string;
  doc?: LawDocLike | null;
}

/** One input's binding in `bindings.yaml`, only the fields read here. */
interface CaseBindingLike {
  kind?: string;
  field?: string;
  service?: string;
  select_on?: Array<{ name?: string; value?: string }> | null;
}

/** An input another law's decided cases supply: a reference to that law's output. */
export interface CaseRef {
  name: string;
  regulation: string;
  output: string;
}

function executions(law: CaseRefLaw): ExecutionLike[] {
  return (law.doc?.articles ?? []).map((a) => a.machine_readable?.execution).filter((ex): ex is ExecutionLike => Boolean(ex));
}

/**
 * The inputs a law reads from another law's decided cases (`kind: cases` with
 * `field` in bindings.yaml), as references to that law's output of the same
 * name. Precario reads the granted area from the terrace permit that way; in
 * the law itself that input has `source: {}`, so without this the graph drew
 * no line between the two.
 */
export function caseRefsFor(
  law: CaseRefLaw,
  bindings: Record<string, Record<string, CaseBindingLike> | undefined> | null | undefined,
  lawByPath: (lawPath: string, service?: string) => CaseRefLaw | null,
): CaseRef[] {
  const out: CaseRef[] = [];
  for (const [name, b] of Object.entries(bindings?.[law.id] ?? {})) {
    if (b.kind !== 'cases' || !b.field) continue;
    const path = (b.select_on ?? []).find((s) => s.name === 'law')?.value;
    const supplier = path ? lawByPath(path, b.service) : null;
    // Alleen een veld dat de andere wet ook uitrekent; `approved` is een
    // feit van de zaak, geen uitkomst van de wet.
    const produces = supplier && executions(supplier).some((ex) => (ex.output ?? []).some((o) => o.name === b.field));
    if (produces) out.push({ name, regulation: supplier.id, output: b.field });
  }
  return out;
}
