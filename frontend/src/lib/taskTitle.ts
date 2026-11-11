/**
 * taskTitle - de regel die een taak in de lijst toont.
 *
 * Bewust NIET `task.title` van de server. De worker stelt die string samen met
 * alleen het law_id in de hand (worker.rs:1495: `format!("Verrijking
 * beoordelen: {}", payload.law_id)`), dus daar staat onvermijdelijk
 * "wet_op_de_zorgtoeslag" in plaats van "Wet op de zorgtoeslag" - de
 * weergavenamen leven in het corpus, dat alleen de frontend bevraagt. Wie de
 * naam kent moet de zin schrijven, dus doen we het hier.
 *
 * Toon: gebiedende wijs voor wat jij moet doen ("Beoordeel …"), mededelend voor
 * wat er buiten je om gebeurde ("… is mislukt") of waar je op wacht - een
 * mislukking is geen opdracht.
 *
 * `task.title` blijft de terugval voor een payload-vorm die we niet kennen; dan
 * is de kale servertekst nog altijd beter dan een lege regel.
 */
import { taskContext, WERKDOCUMENTEN, WET } from './taskCategories';
import type { RunningJob, Task } from './taskCategories';

// Werkdocumenten dragen een pad (`mvt/concept.md`); de bestandsnaam is het
// handvat dat de gebruiker herkent, net als in de werkdocumentenlijst.
function fileName(path: string | null | undefined, fallback = 'werkdocument'): string {
  return path?.split('/').pop() || fallback;
}

/**
 * @param {object} task
 * @param {(lawId: string) => string} lawName Weergavenaam voor een law_id
 *   (displayName van de corpuslezer; valt zelf terug op humanizeLawId).
 */
export function taskTitle(
  task: Task | null | undefined,
  lawName: (lawId: string) => string = (id) => id,
): string {
  const failed = task?.task_type === 'job_failed';
  const context = taskContext(task);

  if (context === WET) {
    // taskContext only answers WET for a task whose payload carries a law_id.
    const payload = task!.payload!;
    const naam = lawName(payload.law_id!);
    // Een mislukking geldt de hele verrijking, dus die noemt nooit een artikel.
    if (failed) return `Verrijking van ${naam} is mislukt`;
    // De worker maakt één taak per gewijzigd artikel en zet het nummer in de
    // payload. Zonder nummer gaat de taak over de hele wet: een nieuwe wet uit
    // documentconversie, of een voorstel dat de worker niet kon opsplitsen.
    const artikel = payload.article;
    return artikel
      ? `Beoordeel artikel ${artikel} van ${naam}`
      : `Beoordeel verrijking van ${naam}`;
  }
  if (context === WERKDOCUMENTEN) {
    // taskContext only answers WERKDOCUMENTEN for a task with a payload.
    const naam = fileName(task!.payload!.target_path);
    return failed ? `Conversie van ${naam} is mislukt` : `Beoordeel werkdocument ${naam}`;
  }
  return task?.title ?? '';
}

/**
 * De regel voor een lopende job. Spiegelt de categorie "Wachten op": dit is
 * niets om te doen, alleen om te weten.
 *
 * Een document_convert-job draagt een synthetische `doc:`-sleutel als law_id,
 * dus die leest zijn naam uit target_path.
 */
export function runningTitle(
  job: RunningJob | null | undefined,
  // Called only with a law_id; a job without one leaves the law out.
  lawName: (lawId: string | null | undefined) => string = (id) => String(id),
): string {
  if (job?.job_type === 'document_convert') {
    return `Wachten op conversie van ${fileName(job.target_path)}`;
  }
  // Een wet maken uit een geüpload document: target_path draagt de geüploade
  // bestandsnaam (COALESCE in list_running_task_jobs_for_account).
  if (job?.job_type === 'law_convert') {
    return `Wachten op conversie van ${fileName(job.target_path, 'document')} naar een regelwerk`;
  }
  // Without a law_id (or a name for it) the title leaves the law out rather
  // than print "undefined" or end on a dangling "van".
  const naam = job?.law_id ? lawName(job.law_id) : '';
  const van = naam ? ` van ${naam}` : '';
  // Een wet ophalen in een traject: law_id draagt het BWB-id van de op te halen
  // wet, die nog niet in het corpus staat, dus lawName valt hier meestal terug.
  if (job?.job_type === 'traject_harvest') {
    return `Wachten op wet ophalen${van}`;
  }
  return `Wachten op verrijking${van}`;
}
