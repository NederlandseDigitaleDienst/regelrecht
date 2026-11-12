import type { RunningJob, Task } from './taskCategories';

// Structural view of an article as both the saved law and a proposal carry it;
// only the fields compared here.
export interface ReviewArticle {
  number: string | number;
  text?: string | null;
  machine_readable?: unknown;
}

export interface ReviewTarget {
  name: 'werkdocumenten-traject' | 'editor-traject';
  params: Record<string, string>;
  query: { task: string | undefined };
}

/**
 * reviewTarget - de router-target voor de "Beoordelen"-knop van een
 * job_review-taak. Vertakt op `payload.kind`:
 *  - `kind === 'document'`: de werkdocumenten-route (router.ts, route
 *    `werkdocumenten-traject`: `trajecten/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/
 *    werkdocumenten/:docPath(.*)?`), met `docPath` op `payload.target_path` -
 *    het werkdocument bestaat op de branch meestal nog niet, dus deze route
 *    is ook de deep-link naar een nog-niet-bestaand document.
 *  - anders (wet-review): de traject-scoped editor-route (route
 *    `editor-traject`: `editor/:trajectRef([a-z0-9-]+-[0-9a-f]{8})/:lawId?/
 *    :articleNumber?`), met `payload.law_id`.
 * Beide dragen de taak-id als `?task=`-query zodat de bestemming de taak kan
 * koppelen aan wat er geopend wordt.
 *
 * Puur en los van de router/sheet zodat de route-opbouw te testen is zonder
 * een component te mounten. Geeft `null` terug wanneer de taak niet genoeg
 * payload-velden heeft voor een van beide routes (bijv. een corrupte of
 * onvolledige taak) - de aanroeper toont dan geen/een disabled knop in
 * plaats van te crashen of naar een kapotte route te navigeren.
 */
/**
 * sameTraject - horen twee traject-refs (`{slug}-{8hex}`) bij hetzelfde
 * traject?
 *
 * Het slug-deel is cosmetisch: het wordt afgeleid uit de naam van dát moment,
 * dus hernoemen verandert het terwijl het traject hetzelfde blijft. Alleen de
 * laatste 8 hex-tekens zijn de identiteit - de backend zoekt er ook uitsluitend
 * daarop op (`resolve_traject_ref`: "renaming a traject does not break existing
 * URLs"). Een taak draagt de ref van het moment van aanvragen, dus een
 * letterlijke stringvergelijking laat een klaarstaand voorstel na een
 * hernoeming stilletjes verdwijnen.
 *
 * Een ref zonder herkenbare 8-hex-staart (leeg, null, afgekapt) hoort nergens
 * bij: dan `false`, niet "matcht met alles".
 */
export function sameTraject(a: unknown, b: unknown): boolean {
  const key = trajectKey(a);
  return key !== null && key === trajectKey(b);
}

function trajectKey(ref: unknown): string | null {
  if (typeof ref !== 'string') return null;
  const match = /-([0-9a-f]{8})$/i.exec(ref);
  // Group 1 always participates when the regex matches.
  return match ? match[1]!.toLowerCase() : null;
}

/**
 * Whether a task says which traject it belongs to, through `traject_id` or a
 * `payload.traject_ref` with an id part. An action that runs in a traject
 * (retrying an enrichment) needs this: a task that names no traject is shown
 * everywhere, so running it "here" could be the wrong traject.
 */
export function taskNamesTraject(task: Task): boolean {
  if (typeof task.traject_id === 'string' && task.traject_id.length >= 8) return true;
  return trajectKey(task.payload?.traject_ref) !== null;
}

/**
 * Whether a task belongs to the traject `trajectRef`. `/api/tasks` returns the
 * whole account, so a view inside one traject has to narrow it down: the same
 * law enriched in two trajects is two separate pieces of work.
 *
 * `traject_id` decides when the task has it: the server sets it on every
 * traject task, and a ref's 8-hex suffix is the first 8 characters of that id
 * (`traject_ref` in trajects.rs). `payload.traject_ref` is the fallback,
 * because not every task kind fills it.
 *
 * A task that names no traject at all stays visible: it belongs to no other
 * traject either, and hiding it would leave it where nobody can dismiss it.
 * That is the opposite of `sameTraject`, where a missing ref matches nothing;
 * both are intended.
 */
export function taskInTraject(task: Task, trajectRef: unknown): boolean {
  const key = trajectKey(trajectRef);
  if (key === null) return false;
  if (typeof task.traject_id === 'string' && task.traject_id.length >= 8) {
    return task.traject_id.slice(0, 8).toLowerCase() === key;
  }
  const own = trajectKey(task.payload?.traject_ref);
  return own === null || own === key;
}

/**
 * Whether a running job was requested in the traject `trajectRef`. Same rule
 * as `taskInTraject`: a job without a recognisable traject stays visible.
 */
export function jobInTraject(job: RunningJob, trajectRef: unknown): boolean {
  const key = trajectKey(trajectRef);
  if (key === null) return false;
  const own = trajectKey(job.traject_ref);
  return own === null || own === key;
}

export function reviewTarget(task: Task | null | undefined): ReviewTarget | null {
  const trajectRef = task?.payload?.traject_ref;
  if (!trajectRef) return null;
  if (task?.payload?.kind === 'document') {
    const targetPath = task.payload.target_path;
    if (!targetPath) return null;
    return {
      name: 'werkdocumenten-traject',
      params: { trajectRef, docPath: targetPath },
      query: { task: task.id },
    };
  }
  const lawId = task?.payload?.law_id;
  if (!lawId) return null;
  // Wijst de taak een artikel aan, neem het dan meteen in de route op. De
  // editor zou er zelf ook naartoe springen zodra het voorstel is geladen,
  // maar dan zie je eerst het verkeerde artikel voorbijkomen.
  const article = task?.payload?.article;
  return {
    name: 'editor-traject',
    params: article == null ? { trajectRef, lawId } : { trajectRef, lawId, articleNumber: String(article) },
    query: { task: task.id },
  };
}

function articleDiffers(current: ReviewArticle | undefined, proposedArticle: ReviewArticle): boolean {
  const sameText = (current?.text ?? '') === (proposedArticle.text ?? '');
  const sameMr =
    JSON.stringify(current?.machine_readable ?? null) ===
    JSON.stringify(proposedArticle.machine_readable ?? null);
  return !(sameText && sameMr);
}

/**
 * proposalDivergence - vergelijkt de artikelen van de huidige (opgeslagen)
 * wet met de voorgestelde artikelen uit een job_review-taak.
 *
 * Levert het EERSTE artikel dat inhoudelijk afwijkt en dat de editor kan
 * seeden (`target`, `null` als niets seedbaar afwijkt), plus `hiddenChanges`:
 * waar of niet er wijzigingen zijn die de (single-article-scoped) editor
 * niet zichtbaar kan maken. Dat geldt voor:
 *  - een tweede (of latere) afwijkend artikel naast `target`;
 *  - a proposed article the current law does not have (v1 can only seed an
 *    EXISTING article as an unsaved edit; see applyProposedContent in
 *    useProposalReview.ts);
 *  - an article of the current law that is MISSING from the proposal (a
 *    deletion) - saving commits that deletion like the rest of the proposal,
 *    so the status line has to point at it even though there is no proposed
 *    article to seed or show.
 *
 * @param {string|null} [articleNumber] Wijst de taak één artikel aan
 *   (`payload.article`), dan gaat het hier alléén daarover: dat artikel wordt
 *   het target en `hiddenChanges` blijft onwaar. Wat de verrijking elders in
 *   de wet voorstelt, hangt aan de taak van dát artikel en is hier geen
 *   verborgen wijziging maar simpelweg andermans werk.
 */
export function proposalDivergence<A extends ReviewArticle>(
  currentArticles: readonly ReviewArticle[] | null | undefined,
  proposedArticles: readonly A[] | null | undefined,
  articleNumber: string | number | null = null,
): { target: A | null; hiddenChanges: boolean } {
  const current: readonly ReviewArticle[] = Array.isArray(currentArticles) ? currentArticles : [];
  const proposed: readonly A[] = Array.isArray(proposedArticles) ? proposedArticles : [];

  if (articleNumber != null) {
    const wanted = String(articleNumber);
    const pa = proposed.find((a) => String(a.number) === wanted);
    const match = current.find((a) => String(a.number) === wanted);
    // Without a counterpart in the saved law there is nothing to seed: the
    // editor cannot show an article the law does not have as a single
    // change. Then there is no target, and the status line says so.
    const target = pa && match && articleDiffers(match, pa) ? pa : null;
    return { target, hiddenChanges: false };
  }

  let target: A | null = null;
  let hiddenChanges = false;
  for (const pa of proposed) {
    const match = current.find((a) => String(a.number) === String(pa.number));
    if (!match) {
      hiddenChanges = true;
      continue;
    }
    if (!articleDiffers(match, pa)) continue;
    if (!target) {
      target = pa;
    } else {
      hiddenChanges = true;
    }
  }

  const proposedNumbers = new Set(proposed.map((pa) => String(pa.number)));
  if (current.some((a) => !proposedNumbers.has(String(a.number)))) {
    hiddenChanges = true;
  }

  return { target, hiddenChanges };
}
