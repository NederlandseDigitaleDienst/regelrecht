/**
 * Per-traject persistence for the editor's open law tabs.
 *
 * The editor's tab bar is scoped to the active traject: each traject remembers
 * its own set of open tabs and which one was active, so switching trajects
 * swaps the bar to that traject's set instead of carrying over tabs that point
 * at laws the new traject doesn't have.
 *
 * Keys follow the per-entity `regelrecht-<feature>:<id>` convention used
 * elsewhere (cf. `useDraftNotes.js`), here `<id>` is the traject ref. All
 * access is wrapped in try/catch with a safe default so full/disabled storage
 * never breaks the editor - tab persistence is best-effort.
 *
 * A tab is an article (`{ kind: 'article', lawId, articleNumber }`) or a
 * scenario feature file (`{ kind: 'scenario', lawId, filename }`). Tabs saved
 * before scenario tabs existed carry no `kind` and read as articles.
 *
 * Reads are sanitised (see `sanitizeTabs`): earlier builds of the per-traject
 * feature briefly wrote one traject's law under another traject's key, and
 * nothing validated the stored content, so that pollution outlived the write
 * fix. Sanitising on read (drop malformed entries, normalise `articleNumber`
 * to a string, de-duplicate, cap at `MAX_TABS`) heals such storage the moment
 * it is loaded.
 */

const TABS_STORAGE_PREFIX = 'regelrecht-open-tabs:';
const ACTIVE_TAB_STORAGE_PREFIX = 'regelrecht-active-tab:';

// Cap on how many law tabs a traject keeps open; the oldest are dropped past
// this. Lives here (rather than in EditorView) so both the sanitising read and
// the in-memory store (`useOpenTabs`) enforce the same bound.
export const MAX_TABS = 20;

/** An article tab. These keys are the persisted format; do not rename them. */
export interface ArticleTab {
  kind: 'article';
  lawId: string;
  articleNumber: string;
}

/** A tab for one feature file of a regelwerk's scenarios. Persisted as is. */
export interface ScenarioTab {
  kind: 'scenario';
  lawId: string;
  filename: string;
}

/** A stored tab. */
export type OpenTab = ArticleTab | ScenarioTab;

/** A tab's identity: two tabs with the same key are the same tab. */
export function tabKey(tab: OpenTab): string {
  return tab.kind === 'scenario' ? `${tab.lawId}::scenario::${tab.filename}` : `${tab.lawId}:${tab.articleNumber}`;
}

type TrajectRef = string | null | undefined;

function tabsKey(trajectRef: TrajectRef): string {
  return `${TABS_STORAGE_PREFIX}${trajectRef ?? ''}`;
}

function activeTabKey(trajectRef: TrajectRef): string {
  return `${ACTIVE_TAB_STORAGE_PREFIX}${trajectRef ?? ''}`;
}

/**
 * Coerce one stored value into a valid tab, or `null`. Every tab needs a
 * non-empty string `lawId`. A scenario tab needs a non-empty `filename`; any
 * other value is an article tab, which needs an `articleNumber`, normalised
 * to a string so a legacy numeric value and a string value never read as two
 * different tabs.
 */
export function sanitizeTab(raw: unknown): OpenTab | null {
  if (!raw || typeof raw !== 'object') return null;
  const { kind, lawId, articleNumber, filename } = raw as {
    kind?: unknown;
    lawId?: unknown;
    articleNumber?: unknown;
    filename?: unknown;
  };
  if (typeof lawId !== 'string' || lawId === '') return null;
  if (kind === 'scenario') {
    if (typeof filename !== 'string' || filename === '') return null;
    return { kind: 'scenario', lawId, filename };
  }
  if (articleNumber == null || articleNumber === '') return null;
  return { kind: 'article', lawId, articleNumber: String(articleNumber) };
}

/**
 * Sanitise a parsed tab array: drop malformed entries, normalise
 * `articleNumber` to a string, de-duplicate by `tabKey` (first occurrence
 * wins) and cap at `MAX_TABS` (keeping the newest).
 */
export function sanitizeTabs(parsed: unknown): OpenTab[] {
  if (!Array.isArray(parsed)) return [];
  const seen = new Set<string>();
  const clean: OpenTab[] = [];
  for (const raw of parsed) {
    const tab = sanitizeTab(raw);
    if (!tab) continue;
    const key = tabKey(tab);
    if (seen.has(key)) continue;
    seen.add(key);
    clean.push(tab);
  }
  return clean.length > MAX_TABS ? clean.slice(-MAX_TABS) : clean;
}

/** The saved open tabs for a traject, or `[]` when there are none / on error. */
export function loadSavedTabs(trajectRef: TrajectRef): OpenTab[] {
  try {
    const saved = localStorage.getItem(tabsKey(trajectRef));
    return sanitizeTabs(saved ? JSON.parse(saved) : []);
  } catch {
    return [];
  }
}

/** Persist a traject's open tabs (best-effort). */
export function saveTabs(trajectRef: TrajectRef, tabs: readonly OpenTab[]): void {
  try {
    localStorage.setItem(tabsKey(trajectRef), JSON.stringify(tabs));
  } catch {
    /* quota/full or disabled - tabs are best-effort */
  }
}

/** The saved active tab for a traject, or `null` when none / on error. */
export function loadSavedActiveTab(trajectRef: TrajectRef): OpenTab | null {
  try {
    const saved = localStorage.getItem(activeTabKey(trajectRef));
    return sanitizeTab(saved ? JSON.parse(saved) : null);
  } catch {
    return null;
  }
}

/** Persist (or clear, when `tab` is falsy) a traject's active tab. */
export function saveActiveTab(trajectRef: TrajectRef, tab: OpenTab | null | undefined): void {
  try {
    if (!tab) localStorage.removeItem(activeTabKey(trajectRef));
    else localStorage.setItem(activeTabKey(trajectRef), JSON.stringify(tab));
  } catch {
    /* quota/full or disabled - tabs are best-effort */
  }
}
