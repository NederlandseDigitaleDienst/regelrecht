/**
 * taskCategories - de indeling van de takenlijst, gedeeld door het
 * categorie-panel (dat telt) en de main-lijst (die filtert). Eén bron, zodat
 * een badge nooit een ander aantal claimt dan de lijst erachter toont.
 *
 * Twee assen:
 *  - `alle` / `prioriteit` / `wachten`: alles op je bord, wat er nú actie van
 *    vraagt, en waar je alleen op wacht. `alle` is letterlijk alles: taken én
 *    lopende jobs. Die jobs zijn technisch geen taak (andere tabel, andere
 *    vorm, geen acties - zie `running` in useTasks), maar dat onderscheid is
 *    van ons, niet van de gebruiker: een lopende conversie ligt net zo goed op
 *    je bord. Ze weglaten maakte "Alle taken" een leugen.
 *  - Contexten: filters over datzelfde werk. `werkdocumenten` is er één, en elke
 *    wet waar iets voor openstaat is er zelf ook één (`wet` + lawId) - plat
 *    naast elkaar, er is geen overkoepelende "Wetten"-verzameling. Lopende jobs
 *    tellen hier mee: een conversie ván een werkdocument gaat over dat
 *    werkdocument, en een verrijking ván een wet over die wet. Een wet kan dus
 *    een context zijn puur omdat er iets voor loopt.
 */

export const ALLE = 'alle';
export const PRIORITEIT = 'prioriteit';
export const WACHTEN = 'wachten';
export const WERKDOCUMENTEN = 'werkdocumenten';
// Eén wet als context; draagt altijd een lawId (route: /taken/wet/{lawId}).
export const WET = 'wet';

/** A context filter: the documents bucket or a single law. */
export type TaskContext = typeof WERKDOCUMENTEN | typeof WET;

// Structural view of a task row from `/api/tasks` (owned by useTasks); only
// the fields this module and its siblings (taskTitle, taskReview) read.
export interface TaskPayload {
  kind?: string | null;
  law_id?: string | null;
  target_path?: string | null;
  traject_ref?: string | null;
  article?: string | number | null;
}

export interface Task {
  id?: string;
  task_type?: string;
  /** The traject the task belongs to, set by the server for every traject task. */
  traject_id?: string | null;
  title?: string | null;
  payload?: TaskPayload | null;
}

// Structural view of a running job (RunningTaskJob in tasks.rs).
export interface RunningJob {
  job_id?: string;
  job_type?: string;
  law_id?: string | null;
  traject_ref?: string | null;
  target_path?: string | null;
}

export interface LawContext {
  lawId: string;
  count: number;
  name: string;
}

/**
 * Vraagt deze taak nú actie? Dit is de enige plek die dat bepaalt - het panel,
 * de lijst én de badge in de primary sidebar hangen er allemaal aan.
 *
 * Vandaag is het antwoord precies "de job is mislukt": zo'n taak zit vast tot
 * iemand ingrijpt, en dat is het enige dat we kunnen wéten. De andere helft van
 * de bedoeling - taken met een deadline die vandaag verstrijkt - kan nog niet:
 * de tasks-tabel heeft geen enkel deadline- of prioriteitsveld, alleen
 * `created_at` (zie 0028_tasks.sql). Komt dat er, dan groeit die check hier
 * aan en verandert er verder niets aan de UI.
 */
export function isPrioriteit(task: Task | null | undefined): boolean {
  return task?.task_type === 'job_failed';
}

/**
 * De context van een taak, afgeleid uit de payload die de worker schrijft.
 *
 * Let op de asymmetrie in die payloads: een document-REVIEW draagt
 * `kind: "document"` (worker.rs:1543), maar een mislukte CONVERSIE draagt dat
 * veld niet (worker.rs:1366) - die herken je alleen aan `target_path` zonder
 * `law_id`. Vandaar de drietrapscheck, in deze volgorde: het expliciete `kind`
 * wint, daarna `law_id` (enrich-review én enrich-fout), en `target_path` vangt
 * de conversiefout op.
 */
export function taskContext(task: Task | null | undefined): TaskContext | null {
  const payload = task?.payload;
  if (!payload) return null;
  if (payload.kind === 'document') return WERKDOCUMENTEN;
  if (payload.law_id) return WET;
  if (payload.target_path) return WERKDOCUMENTEN;
  return null;
}

/** De wet waar een taak over gaat, of null voor een werkdocument-taak. */
export function taskLawId(task: Task | null | undefined): string | null {
  return taskContext(task) === WET ? (task?.payload?.law_id ?? null) : null;
}

/**
 * De context van een lopende job. Andere vorm dan een taak (jobs komen uit de
 * jobs-tabel, zie RunningTaskJob in tasks.rs), dus een eigen functie - maar
 * dezelfde contexten, zodat "waar gaat dit over" één antwoord houdt.
 *
 * De val zit in `law_id`: een document_convert-job draagt daar een synthetische
 * `doc:{traject_ref}/{target_path}`-sleutel (corpus_handlers.rs:2943), géén
 * wet. Vandaar dat het job_type beslist en niet de aanwezigheid van law_id -
 * anders zou elke lopende conversie als "wet" tellen, met een `doc:`-string als
 * naam in het panel.
 */
export function jobContext(job: RunningJob | null | undefined): TaskContext | null {
  if (!job) return null;
  if (job.job_type === 'document_convert') return WERKDOCUMENTEN;
  return job.law_id ? WET : null;
}

/** De wet waar een lopende job over gaat, of null voor een conversie. */
export function jobLawId(job: RunningJob | null | undefined): string | null {
  return jobContext(job) === WET ? (job?.law_id ?? null) : null;
}

/**
 * De taken voor een categorie. `wachten` zit hier niet bij: dat zijn jobs,
 * geen taken, en die komen uit `running`.
 *
 * `wet` zonder lawId levert alle wet-taken. Het panel linkt daar niet naartoe
 * (elke wet is zijn eigen ingang), maar een handmatig getypte /taken/wet blijft
 * zo een zinnige lijst in plaats van een lege.
 */
export function filterTasks<T extends Task>(
  tasks: readonly T[] | null | undefined,
  categorie: string | null | undefined,
  lawId: string | null = null,
): T[] {
  const list: T[] = Array.isArray(tasks) ? (tasks as T[]) : [];
  if (categorie === ALLE) return list;
  if (categorie === PRIORITEIT) return list.filter(isPrioriteit);
  if (categorie === WET && lawId) return list.filter((t) => taskLawId(t) === lawId);
  if (categorie === WERKDOCUMENTEN || categorie === WET) {
    return list.filter((t) => taskContext(t) === categorie);
  }
  return [];
}

/**
 * De lopende jobs voor een categorie - de tegenhanger van `filterTasks`, want
 * jobs hebben hun eigen vorm. Ze horen in `wachten` (waar ze de lijst zijn),
 * in `alle`, en in de context waar ze over gaan. Niet in `prioriteit`: er valt
 * niets te doen aan iets dat nog loopt.
 */
export function filterRunning<J extends RunningJob>(
  running: readonly J[] | null | undefined,
  categorie: string | null | undefined,
  lawId: string | null = null,
): J[] {
  const jobs: J[] = Array.isArray(running) ? (running as J[]) : [];
  if (categorie === WACHTEN || categorie === ALLE) return jobs;
  if (categorie === WET && lawId) return jobs.filter((j) => jobLawId(j) === lawId);
  if (categorie === WERKDOCUMENTEN || categorie === WET) {
    return jobs.filter((j) => jobContext(j) === categorie);
  }
  return [];
}

/**
 * De wetten waar iets voor openstaat, met hun aantal - de individuele
 * wet-contexten. Telt taken én lopende jobs, dus een wet kan hier staan puur
 * omdat er een verrijking voor loopt. Gesorteerd op naam zodat de lijst niet
 * herschikt zodra er iets bijkomt; `nameFor` levert de weergavenaam (payload en
 * job dragen allebei alleen het rauwe law_id).
 */
export function lawContexts(
  tasks: readonly Task[] | null | undefined,
  running: readonly RunningJob[] | null | undefined = [],
  nameFor: (lawId: string) => string = (id) => id,
): LawContext[] {
  const counts = new Map<string, number>();
  const bump = (lawId: string | null) => {
    if (!lawId) return;
    counts.set(lawId, (counts.get(lawId) ?? 0) + 1);
  };
  for (const task of Array.isArray(tasks) ? tasks : []) bump(taskLawId(task));
  for (const job of Array.isArray(running) ? running : []) bump(jobLawId(job));
  return [...counts.entries()]
    .map(([lawId, count]) => ({ lawId, count, name: nameFor(lawId) }))
    .sort((a, b) => a.name.localeCompare(b.name, 'nl'));
}

/**
 * Aantallen voor de vaste panel-ingangen. Wetten zitten hier niet bij: die
 * tellen per wet, via `lawContexts`.
 *
 * Elk aantal telt wat de bijbehorende lijst ook toont - dus `alle` en
 * `werkdocumenten` tellen hun lopende jobs mee, en `prioriteit` niet.
 */
export function categoryCounts(
  tasks: readonly Task[] | null | undefined,
  running: readonly RunningJob[] | null | undefined = [],
) {
  const list = Array.isArray(tasks) ? tasks : [];
  const jobs = Array.isArray(running) ? running : [];
  return {
    [ALLE]: list.length + jobs.length,
    [PRIORITEIT]: list.filter(isPrioriteit).length,
    [WACHTEN]: jobs.length,
    [WERKDOCUMENTEN]:
      list.filter((t) => taskContext(t) === WERKDOCUMENTEN).length +
      jobs.filter((j) => jobContext(j) === WERKDOCUMENTEN).length,
  };
}
