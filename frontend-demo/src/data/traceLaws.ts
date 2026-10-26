/**
 * Which law each line of the engine's text trace belongs to.
 *
 * The presenter walks a trace law by law (zorgtoeslagwet, then the Zvw, then
 * the penitentiaire beginselenwet), and in two thousand lines of box drawing
 * the place where one law hands over to the next is hard to find. The engine
 * marks that place itself: `Evaluating rules for <law>` opens a law, and
 * `Reference: <law>#<output>` opens the law it reads from. Everything indented
 * below such a line runs inside that law, until the tree comes back out.
 *
 * Indentation is the column of the line's connector (`├ └ ╟ ╙`); a line that
 * has none (the root header) sits outside every scope. The format comes from
 * `render_node` in `packages/engine/src/trace.rs`.
 */

const CONNECTOR = /[├└╟╙]/;
// A law id can have path segments: `algemene_ouderdomswet/leeftijdsbepaling`.
const OPENS = /(?:Evaluating rules for|Reference:) ([A-Za-z0-9_/]+)/;
/** The root header, `zorgtoeslagwet (2024-02-01 {bsn: …} hoogte_toeslag)`. */
const HEADER = /^([A-Za-z0-9_/]+) \(/;

/** One line of the trace, with the law it runs in and whether that law starts here. */
export interface TraceLine {
  text: string;
  law: string | null;
  start: boolean;
}

/**
 * The trace split by law. `sections` are the indices of the lines where a
 * different law takes over, in order; `laws` lists each law once, in the order
 * it first appears.
 */
export interface TraceLaws {
  lines: TraceLine[];
  sections: number[];
  laws: string[];
}

/**
 * @param isLaw  a `Reference:` to something that is
 *   not a law in the corpus (the engine writes `untranslatable#…` for a marked
 *   construct) opens no scope
 */
export function traceLaws(text: string | null | undefined, isLaw: (id: string) => boolean = () => true): TraceLaws {
  const stack: Array<{ law: string; col: number }> = [];
  const lines: TraceLine[] = [];
  const sections: number[] = [];
  const laws: string[] = [];
  const raw = String(text ?? '').split('\n');
  raw.forEach((line, i) => {
    const hit = line.match(CONNECTOR);
    // A match without the `g` flag always carries its index.
    const col = hit ? hit.index! : -1;
    // Out of every scope this line is not inside.
    if (col >= 0) while (stack.length && stack.at(-1)!.col >= col) stack.pop(); // non-empty: checked in the condition
    const outer = stack.at(-1)?.law ?? null;
    const found = col >= 0 ? line.match(OPENS)?.[1] : !stack.length && line.match(HEADER)?.[1];
    const opened = found && isLaw(found) ? found : null;
    let law = outer;
    let start = false;
    if (opened) {
      stack.push({ law: opened, col });
      law = opened;
      // A law that reads a second output of itself is not a new stop.
      start = opened !== outer;
    }
    if (start) {
      sections.push(i);
      // `start` is only set together with `law = opened`, which is a string.
      if (!laws.includes(law!)) laws.push(law!);
    }
    lines.push({ text: line, law, start });
  });
  return { lines, sections, laws };
}
