/**
 * useTaskReview - review-modus voor een job_review-taak in de editor.
 *
 * Een verrijking levert één taak per gewijzigd artikel. Je oordeelt per
 * artikel, maar de verrijking is de eenheid die geschreven wordt: pas als
 * elk onderdeel een oordeel heeft, gaat alles in één keer naar de backend,
 * die er één commit van maakt. Tot dat moment verandert er niets aan de wet.
 *
 * Bij ?task=<id>: haal de taakdetail op, vind het law-YAML-resultaat en lever
 * (a) de voorgestelde content, (b) de andere onderdelen van dezelfde
 * verrijking, en (c) de acties om een oordeel vast te leggen en de verrijking
 * te verwerken.
 *
 * Er is geen staleness-vlag meer. Die vergeleek `payload.source_etag` met de
 * actuele ETag en sloeg daardoor aan zodra een zusje van dezelfde verrijking
 * was goedgekeurd - een waarschuwing over je eigen werk. Met één schrijfmoment
 * hoort er ook één controle te zijn, en die zit nu op het `If-Match` van het
 * verwerken.
 */
import { ref, computed } from 'vue';
import { useTaskActions } from './useTasks';
import type { EnrichDecision, JobTaskSummary, TaskDetail } from './useTasks';
import { useEnrichVerdictsStore, type Verdict } from '../stores/enrichVerdicts';

export type { Verdict, JobVerdicts } from '../stores/enrichVerdicts';

export function useTaskReview() {
  // useTaskActions(), not useTasks(): useTaskReview() is itself called
  // unconditionally in EditorView's setup(), so joining the polled
  // useTasks() here would start the 30s poll for every editor visitor
  // (including anonymous ones).
  const { fetchTask, resolveTask, fetchJobTasks, applyEnrichment } = useTaskActions();
  const verdicts = useEnrichVerdictsStore();
  const reviewTask = ref<TaskDetail | null>(null);
  const proposedContent = ref<string | null>(null);
  const loadError = ref<string | null>(null);
  // De onderdelen van deze verrijking: `[{ id, article, status, title }]`.
  // Eén element voor een taak zonder job (of wanneer het ophalen mislukt) -
  // dan is deze taak zelf de hele verrijking.
  const jobParts = ref<JobTaskSummary[]>([]);

  const jobId = computed(() => reviewTask.value?.job_id ?? null);
  // Waaronder de tussenstand wordt bewaard. Eén sleutel voor de hele
  // verrijking; valt terug op de taak zelf voor een taak zonder job, want dan
  // is die taak in zijn eentje de verrijking. Alles wat de oordelen leest of
  // schrijft gebruikt déze sleutel - een lezer die `jobId` neemt en een
  // schrijver die terugvalt, zouden elkaars oordelen niet zien.
  const verdictKey = computed(() => jobId.value ?? reviewTask.value?.id ?? null);
  const openParts = computed(() => jobParts.value.filter((p) => p.status === 'open'));
  /** Onderdelen die nog op een oordeel wachten. */
  const undecidedParts = computed(() => {
    const decided = verdicts.forJob(verdictKey.value);
    return openParts.value.filter((p) => !decided[p.id]);
  });
  /** Hoeveelste onderdeel je nu beoordeelt, 1-based (0 als het er niet bij zit). */
  const partIndex = computed(() => {
    const idx = openParts.value.findIndex((p) => p.id === reviewTask.value?.id);
    return idx < 0 ? 0 : idx + 1;
  });
  const partCount = computed(() => openParts.value.length);

  async function loadReview(taskId: string): Promise<void> {
    // Elke load begint schoon. Niet elke wissel van taak gaat langs `reset()`:
    // via de takenlijst springt `?task=` rechtstreeks van de ene taak naar de
    // andere, en dan zou de foutmelding van de vorige taak boven de nieuwe
    // blijven hangen.
    loadError.value = null;
    try {
      const detail = await fetchTask(taskId);
      if (detail.task_type !== 'job_review' || detail.status !== 'open') {
        loadError.value = 'Deze taak is al afgehandeld.';
        return;
      }
      const lawId = detail.payload?.law_id;
      // Het law-YAML-resultaat is het bestand met de wet zelf. De worker
      // staged ook `features/*.feature`-bestanden naast `laws/...`, en die
      // sorteren er soms vóór - dus eerst het exacte pad uit de payload
      // proberen, en pas als dat niets oplevert terugvallen op de eerste
      // niet-dot-prefixed file (sidecars als .enrichment.yaml sluiten we
      // in v1 nog steeds uit).
      const results = detail.results || [];
      const lawFile =
        results.find((f) => f.path === detail.payload?.yaml_path) ||
        // split() always yields at least one segment.
        results.find((f) => !f.path.split('/').pop()!.startsWith('.'));
      if (!lawFile || !lawId) {
        loadError.value = 'Geen resultaat gevonden bij deze taak.';
        return;
      }
      reviewTask.value = detail;
      proposedContent.value = lawFile.content;
      loadError.value = null;
      jobParts.value = await loadJobParts(detail);
    } catch (e) {
      loadError.value = 'Taak laden mislukt.';
    }
  }

  /**
   * De andere onderdelen van dezelfde verrijking. Mislukt dat (of hangt de
   * taak aan geen job), dan is deze taak in zijn eentje de verrijking: dan kun
   * je hem nog steeds beoordelen en verwerken.
   */
  async function loadJobParts(detail: TaskDetail): Promise<JobTaskSummary[]> {
    const fallback: JobTaskSummary[] = [
      {
        id: detail.id,
        article: detail.payload?.article == null ? null : String(detail.payload.article),
        status: 'open',
        title: detail.title,
      },
    ];
    if (!detail.job_id) return fallback;
    try {
      const json = await fetchJobTasks(detail.job_id);
      const parts = Array.isArray(json?.tasks) ? json.tasks : [];
      return parts.length > 0 ? parts : fallback;
    } catch {
      return fallback;
    }
  }

  /**
   * Leg het oordeel over het onderdeel dat nu open staat vast en zeg wat er
   * daarna moet gebeuren: het volgende onbeoordeelde onderdeel, of - als dit
   * de laatste was - dat de verrijking verwerkt kan worden.
   *
   * `content` is de inhoud die de gebruiker accordeert (het artikel zoals het
   * in de editor staat, of de hele wet bij een onderdeel zonder artikelnummer);
   * bij "niet overnemen" blijft hij weg.
   */
  function decide(
    action: Verdict['action'],
    content?: string | null,
  ): { done: boolean; next: JobTaskSummary | null } {
    const task = reviewTask.value;
    if (!task) return { done: false, next: null };
    verdicts.record(verdictKey.value, task.id, action, content ?? null);
    const next = undecidedParts.value.find((p) => p.id !== task.id) ?? null;
    return { done: !next, next };
  }

  /**
   * Verwerk de verrijking: de vastgelegde oordelen plus, wanneer
   * `rejectRemaining` waar is, "niet overnemen" voor alles waar nog geen
   * oordeel over is geveld. Dat laatste is de weg om een verrijking af te
   * ronden zonder alles één voor één langs te lopen.
   */
  async function processEnrichment(
    etag: string | null | undefined,
    { rejectRemaining = false }: { rejectRemaining?: boolean } = {},
  ) {
    const task = reviewTask.value;
    if (!task) return null;
    // With a task loaded the key falls back to at least its id.
    const key = verdictKey.value!;
    const decided = verdicts.forJob(key);
    const decisions = openParts.value
      .map((part): EnrichDecision | null => {
        const verdict = decided[part.id];
        if (verdict) {
          return verdict.action === 'approved'
            ? { task_id: part.id, action: 'approved', content: verdict.content ?? undefined }
            : { task_id: part.id, action: 'rejected' };
        }
        return rejectRemaining ? { task_id: part.id, action: 'rejected' } : null;
      })
      .filter((d): d is EnrichDecision => Boolean(d));
    // A task whose job is gone (tasks.job_id is set to NULL when the job is
    // deleted) has nothing the apply endpoint can find: it looks tasks up by
    // job id and would answer 404. Rejecting it still releases the task;
    // approving it cannot write anything, so it fails like a failed apply,
    // before anything is cleared, so the recorded verdict survives.
    if (!jobId.value) {
      const onlyRejected = decisions.length > 0 && decisions.every((d) => d.action === 'rejected');
      if (!onlyRejected) {
        throw new Error('Deze taak hoort bij geen verrijking meer en kan niet worden overgenomen. Verwerp haar om haar af te sluiten.');
      }
      await resolveTask(task.id, 'rejected');
      verdicts.clear(key);
      reset();
      return null;
    }
    const result = await applyEnrichment(jobId.value, decisions, etag);
    verdicts.clear(key);
    reset();
    return result;
  }

  /**
   * Los afhandelen van één taak, zonder de wet te schrijven. Alleen nog voor
   * het `law_create`-pad: daar bestaat de wet nog niet, dus die gaat via het
   * aanmaakpad (POST) en niet via het verwerken van een verrijking.
   */
  async function approveAfterSave(): Promise<void> {
    if (reviewTask.value) await resolveTask(reviewTask.value.id, 'approved');
    reset();
  }

  async function reject(): Promise<void> {
    if (reviewTask.value) await resolveTask(reviewTask.value.id, 'rejected');
    reset();
  }

  // Terug naar "geen review". Ook `loadError` gaat mee: een foutmelding van de
  // vorige taak hoort niet boven de volgende (of boven een artikel waar
  // helemaal geen taak op staat) te blijven hangen.
  function reset(): void {
    reviewTask.value = null;
    proposedContent.value = null;
    loadError.value = null;
    jobParts.value = [];
  }

  return {
    reviewTask,
    proposedContent,
    loadError,
    jobParts,
    openParts,
    undecidedParts,
    partIndex,
    partCount,
    loadReview,
    decide,
    processEnrichment,
    approveAfterSave,
    reject,
    reset,
  };
}
