/*
 * Excerpts of the schema conformance fixtures, for the examples on
 * /reference/schema-release-notes.
 *
 * A release note shows how a construct is written, and an example typed into
 * the page would be checked by nothing. The fixtures under
 * packages/engine/tests/conformance/ are checked on every run of
 * `just conformance`: a `valid/` fixture must pass the schema and survive the
 * model's round trip, an `invalid/` one must be refused by the schema it names.
 * So the page shows lines out of a fixture, read when the site is built.
 *
 * A fixture marks the lines it lends out with a pair of comments:
 *
 *     # release-note-begin: <region>
 *     ...
 *     # release-note-end: <region>
 *
 * Regions may nest or overlap; marker lines of other regions inside an excerpt
 * are dropped. Everything here throws instead of returning less: a missing,
 * doubled or empty region fails the build, because a release note that quietly
 * renders no example is worse than one that does not build.
 */
import { read } from './corpus';

const CONFORMANCE = 'packages/engine/tests/conformance';
const MARKER = /^\s*# release-note-(begin|end): (\S+)\s*$/;
const BLOB = 'https://github.com/NederlandseDigitaleDienst/regelrecht/blob/main';

export interface FixtureExcerpt {
  /** The lines of the region, dedented so the outermost key starts at column 0. */
  yaml: string;
  /** Repository path of the fixture. */
  path: string;
  /** The fixture on GitHub, at main. */
  url: string;
}

/** The lines `fixture` marks as `region`; `fixture` is relative to the conformance directory. */
export function fixtureExcerpt(fixture: string, region: string): FixtureExcerpt {
  const path = `${CONFORMANCE}/${fixture}`;
  const lines = read(path).split('\n');

  const at = (kind: 'begin' | 'end') =>
    lines.flatMap((line, i) => {
      const m = MARKER.exec(line);
      return m && m[1] === kind && m[2] === region ? [i] : [];
    });
  const begins = at('begin');
  const ends = at('end');
  if (begins.length !== 1 || ends.length !== 1) {
    throw new Error(
      `${path}: expected one release-note-begin and one release-note-end for "${region}", ` +
        `found ${begins.length} and ${ends.length}`,
    );
  }
  const [begin, end] = [begins[0]!, ends[0]!];
  if (end <= begin) throw new Error(`${path}: region "${region}" ends before it begins`);

  const body = lines.slice(begin + 1, end).filter((line) => !MARKER.test(line));
  const filled = body.filter((line) => line.trim() !== '');
  if (filled.length === 0) throw new Error(`${path}: region "${region}" is empty`);

  const indent = Math.min(...filled.map((line) => line.length - line.trimStart().length));
  const yaml = body
    .map((line) => line.slice(indent))
    .join('\n')
    .trimEnd();

  return { yaml, path, url: `${BLOB}/${path}` };
}
