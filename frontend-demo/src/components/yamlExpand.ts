/**
 * Welke knopen van de YAML-boom staan open als een wet net geopend is.
 *
 * De presentator klapt één artikel per keer open en vertelt erbij. Dat stelt
 * twee eisen die elkaar in de weg lijken te zitten: de boom moet kaal beginnen,
 * én één klik op een artikel moet meteen de juiste regels tonen in plaats van
 * de presentator nog vier niveaus dieper te laten klikken.
 *
 * Daarom is het artikel de enige handmatige stap, en ligt de weg naar een
 * geconfigureerd pad al open te wachten, tot en met dat pad zelf. Buren blijven
 * dicht omdat ze niet op die weg liggen, en wat onder het geconfigureerde pad
 * hangt blijft dicht omdat het pad daar ophoudt: dat laatste is wat een ondiep
 * pad als `…execution.actions` anders alsnog in één klap zou uitklappen.
 *
 * Soms moet ook een knoop onderweg dicht beginnen, omdat de presentator die
 * er bewust bij openklapt (`actions`, of één berekening daarin). Zo'n knoop
 * staat in `folded`: hij begint dicht, maar wat eronder klaarligt staat open
 * zodra hij wordt opengeklapt. Dat werkt omdat een kind pas bij het openen
 * wordt aangemaakt en dan zijn eigen voorbereide stand opzoekt.
 *
 * De paden komen uit `expanded_paths` en `folded_paths` in
 * `corpus/demo/demo-config.yaml`.
 */

/** `articles.2` — het enige knooppunt dat de presentator zelf openklapt. */
const ARTICLE = /^articles\.[^.]+$/;

/**
 * Ligt `path` op de weg naar `pattern`, of ís het `pattern`? `*` matcht elk
 * segment.
 *
 * Wat eronder ligt telt níét mee. Een pad dat ondiep eindigt — de
 * precariobelasting stopt bij `…execution.actions` — zou anders zijn hele
 * deelboom tot elke diepte openzetten, en dat is precies het uitklappen-in-één-
 * keer waar deze stand vanaf moest.
 */
export function onPath(pattern: string, path: string): boolean {
  const p = pattern.split('.');
  const q = path.split('.');
  if (q.length > p.length) return false;
  for (let i = 0; i < q.length; i += 1) {
    if (p[i] !== '*' && q[i] !== '*' && p[i] !== q[i]) return false;
  }
  return true;
}

/** Is `path` precies `pattern`? `*` matcht elk segment. */
export function matches(pattern: string, path: string): boolean {
  return pattern.split('.').length === path.split('.').length && onPath(pattern, path);
}

/** Label used in the child path: items of a list get their name/output. */
export function scalarHint(obj: unknown): string | number | undefined {
  if (!obj || typeof obj !== 'object' || Array.isArray(obj)) return undefined;
  const fields = obj as Record<string, unknown>;
  // Only a scalar names a node; an execution block's `output` is a list.
  return [fields.output, fields.name, fields.number].find((h): h is string | number => typeof h === 'string' || typeof h === 'number');
}

/** The path of `child`, found under `key` in the node at `parent`. */
export function childPath(parent: string, key: string | number, child: unknown, parentIsList: boolean): string {
  // Only a list item takes its name as label; a mapping key stays the key, so
  // `source: {output: x}` is addressed as `.source`, not `.x`.
  const hint = parentIsList ? scalarHint(child) : undefined;
  // Paths are dot-separated, so a dot inside a label (article "2.34") would
  // split it into two segments and no default or configured path would match.
  const label = (hint !== undefined ? String(hint) : String(key)).replaceAll('.', '_');
  return parent ? `${parent}.${label}` : label;
}

/** A node of the YAML tree, and the prepared state it opens in. */
export interface OpenQuery {
  path: string;
  depth: number;
  /** "Expand all" (true), "collapse all" (false), or the prepared state (null). */
  all: boolean | null;
  paths?: string[] | null;
  folded?: string[] | null;
}

export function initiallyOpen({ path, depth, all, paths, folded }: OpenQuery): boolean {
  // De knoppen "alles open" en "alles dicht" overrulen de voorbereide stand.
  if (all === true) return true;
  if (all === false) return false;
  if (depth < 1) return true;
  // Eerder stond elk artikel én zijn machine_readable-blok hier
  // onvoorwaardelijk open, waardoor de hele boom in één keer uitklapte en er
  // niets meer te onthullen viel.
  // Een wet die het artikel zelf als pad noemt (`articles.1`) opent het wel:
  // bij een wet met één artikel valt er niets te kiezen.
  if (ARTICLE.test(path)) return (paths ?? []).some((pattern) => matches(pattern, path));
  if (path === 'articles') return true;
  if ((folded ?? []).some((pattern) => matches(pattern, path))) return false;
  return (paths ?? []).some((pattern) => onPath(pattern, path));
}
