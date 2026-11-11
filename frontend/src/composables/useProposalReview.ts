/**
 * useProposalReview - review mode in the editor: deciding on a generated
 * proposal (an enrichment, or a regelwerk from a document conversion) for the
 * regelwerk that is open.
 *
 * The task comes from the route (`?task=<id>`) and travels with navigation
 * inside the editor. While the open article is the one the task is about, the
 * proposal sits in its edits (`useOpenLaw.seedProposal`), the changes bar
 * shows the review, and approving or rejecting goes through this module.
 *
 * Een verrijking levert één taak per gewijzigd artikel. Je oordeelt per
 * artikel, maar de verrijking is de eenheid die geschreven wordt: pas als
 * elk onderdeel een oordeel heeft, gaat alles in één keer naar de backend,
 * die er één commit van maakt. Tot dat moment verandert er niets aan de wet.
 *
 * Er is geen staleness-vlag meer. Die vergeleek `payload.source_etag` met de
 * actuele ETag en sloeg daardoor aan zodra een zusje van dezelfde verrijking
 * was goedgekeurd - een waarschuwing over je eigen werk. Met één schrijfmoment
 * hoort er ook één controle te zijn, en die zit nu op het `If-Match` van het
 * verwerken.
 */
import { computed, ref, watch } from 'vue';
import { useRoute, useRouter, type RouteLocationRaw } from 'vue-router';
import * as yaml from 'js-yaml';
import { useTaskActions } from './useTasks';
import type { EnrichDecision, JobTaskSummary, TaskDetail } from './useTasks';
import { useEnrichVerdictsStore, type Verdict } from '../stores/enrichVerdicts';
import { proposalDivergence } from '../lib/taskReview';
import type { RequestError } from '../lib/apiFetch';
import type { LawArticle, LawDocument } from '../lib/lawFormat';
import type { OpenLaw } from './useOpenLaw';

/** An editor route as an object, so a task can be added to its query. */
type EditorRoute = Exclude<RouteLocationRaw, string>;

/** What review mode needs from the editor it runs in. */
export interface ProposalReviewHost {
  openLaw: OpenLaw;
  /** An editor route for a law and article in the current traject; it carries the task unless told not to. */
  routeFor: (
    lawId: string | null | undefined,
    articleNumber: string | null | undefined,
    options?: { keepReviewTask?: boolean },
  ) => EditorRoute;
  /** Where to go when the reviewed regelwerk does not exist (a rejected `law_create`). */
  leaveRoute: () => RouteLocationRaw;
  /** Throw away the edits to the open article. */
  discard: () => void;
  /** Re-anchor the notes after the law text changed. */
  reloadNotes: () => void;
}

const DUMP_OPTIONS = { lineWidth: 80, noRefs: true };

export function useProposalReview(host: ProposalReviewHost) {
  const route = useRoute();
  const router = useRouter();
  const verdicts = useEnrichVerdictsStore();
  const {
    lawId,
    articles,
    selectedArticle,
    selectedArticleNumber,
    loading,
    error,
    seedFromYaml,
    editedText,
    machineReadable,
    currentEtag,
    reload: reloadLaw,
  } = host.openLaw;

  // useTaskActions(), not useTasks(): this module is itself called
  // unconditionally in EditorView's setup(), so joining the polled
  // useTasks() here would start the 30s poll for every editor visitor
  // (including anonymous ones).
  const { fetchTask, resolveTask, fetchJobTasks, applyEnrichment } = useTaskActions();
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

  async function rejectTask(): Promise<void> {
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


  // The review task in the URL (`?task=`). Editor routes carry it along
  // (`taskId`, which EditorView's `editorRouteFor` reads), so while a task is
  // under review it travels with every navigation inside the editor: tab,
  // article and law switches. All of review mode below hangs on this source.
  const reviewTaskIdParam = computed(() =>
    typeof route.query.task === 'string' ? route.query.task : null,
  );

  // --- Review-modus (job_review-taak) -----------------------------------
  // `?task=<id>`: show a job_review task's
  // proposed law YAML as an unsaved edit rather than fetching it
  // separately. The proposal is applied to the first article where it
  // diverges from the saved law (seeding `editedText`/`machineReadable`,
  // the same pane-local "current" refs a manual edit would touch), so the
  // existing dirty-tracking and Wijzigingenbalk (Opslaan/Wijzigingen-
  // ongedaan) drive the review UI the same way they drive a manual edit.
  // Seeden is artikel-scoped: de taak wijst het artikel aan (`payload.article`)
  // en de editor toont dat.
  //
  // Opslaan schrijft niet meer. Een verrijking is de eenheid die geschreven
  // wordt, dus "Neem voorstel over" legt het oordeel over dít onderdeel vast en
  // brengt je naar het volgende; pas als elk onderdeel een oordeel heeft, gaat
  // alles in één keer naar de backend, die er één commit van maakt. Tot dat
  // moment verandert er niets aan de wet.
  // `law_create`-taak: de wet bestaat nog niet in het traject; het voorstel
  // wordt integraal in de editor geseed en Opslaan gaat via het create-pad
  // (POST) in plaats van de PUT.
  const reviewIsLawCreate = computed(
    () => reviewTask.value?.payload?.kind === 'law_create',
  );
  // Het artikel waar deze taak over gaat. De worker maakt sinds de opsplitsing
  // één review-taak per gewijzigd artikel en zet het nummer in de payload, zodat
  // een beoordelaar zich uitspreekt over wat er werkelijk veranderd is.
  //
  // Zonder nummer gaat de taak over de hele wet. Dat blijft bestaan voor een
  // `law_create` (de wet ís het voorstel), voor een voorstel dat de worker niet
  // kon opsplitsen, en voor taken van vóór deze wijziging.
  const reviewArticleNumber = computed(() => {
    const number = reviewTask.value?.payload?.article;
    return number == null ? null : String(number);
  });
  // Kijk je op dit moment naar waar de taak over gaat? De taak reist mee met de
  // navigatie (zie `editorRouteFor`), zodat een uitstapje naar een ander artikel
  // de beoordeling niet weggooit - maar de beoordeel-weergave hoort alleen te
  // staan boven het artikel waarover je je uitspreekt. Anders hangt de balk over
  // vreemde inhoud en keurt "Opslaan" de taak goed met een artikel dat de
  // verrijking nooit heeft aangeraakt.
  //
  // Een taak zonder artikelnummer gaat over de hele wet: die slaat het voorstel
  // integraal op (`reviewSavesWholeLaw`), dus dan is elk artikel van die wet een
  // geldige plek om hem te beoordelen.
  const reviewOnTarget = computed(() => {
    const task = reviewTask.value;
    if (!task) return false;
    const taskLawId = task.payload?.law_id;
    if (taskLawId && lawId.value && taskLawId !== lawId.value) return false;
    const number = reviewArticleNumber.value;
    if (number == null) return true;
    return String(selectedArticleNumber.value ?? '') === number;
  });
  // De beoordeel-weergave (balk, Opslaan-als-goedkeuren, Verwerpen) staat aan.
  // Bewust smaller dan "er is een taak geladen": zie `reviewOnTarget`.
  const reviewActive = computed(() => !!reviewTask.value && reviewOnTarget.value);

  // Een taak die één artikel aanwijst, accordeert dat artikel zoals het in de
  // panes staat - inclusief wat de beoordelaar er zelf nog aan veranderde. Een
  // taak zonder artikelnummer accordeert het voorstel in zijn geheel, want daar
  // is geen kleinere eenheid om over te beslissen.
  const reviewSavesWholeLaw = computed(() => reviewActive.value && !reviewArticleNumber.value);
  // Guards against re-firing loadReview for a task id already attempted -
  // approve/reject null out `reviewTask`, which would otherwise look
  // indistinguishable from "not loaded yet" and re-trigger against the
  // task we just resolved.
  let reviewAttemptedForTaskId: string | null = null;

  // Drop `?task=` from the URL once the review is resolved (approved or
  // rejected) so a refresh/back-navigation doesn't re-open review mode.
  // Rebuilt from the CURRENT law/article (not `route.params`, which still
  // names whatever article the URL originally pointed at): applyProposedContent
  // may have moved `selectedArticleNumber` to the article the proposal
  // actually touches, and `onBeforeRouteUpdate` would otherwise see the
  // stale route param disagree with `selectedArticleNumber` and snap the
  // editor back to the pre-review article right after resolving.
  function clearReviewQuery() {
    router.replace(
      host.routeFor(lawId.value, selectedArticleNumber.value, { keepReviewTask: false }),
    );
  }

  // Whether the proposal seeded anything (false when every proposed article
  // matches the saved law, or the only differences are articles the saved
  // law doesn't have - see the comment on the loop below).
  const reviewSeeded = ref(false);
  // Whether the proposal touches article(s) beyond the single one seeded into
  // the editor panes (or, when nothing was seedable, the proposal touches
  // anything at all). The visible editor is always single-article-scoped, but
  // Opslaan now approves the FULL proposal regardless - this only drives the
  // banner copy that points the reviewer at the YAML panel for the rest.
  const reviewHasHiddenChanges = ref(false);
  // The article the proposal should land in (`payload.article`, or - for a
  // proposal about the whole law - the article `proposalDivergence` picked), and
  // the article it actually sits in right now. The two part as soon as you look
  // at another article: useOpenLaw then drops the edits and the panes show the
  // saved law again. That difference is exactly the signal to seed again on
  // return.
  const reviewSeedTarget = ref<string | null>(null);
  // Where the proposal actually sits in the panes right now, as `law::article`.
  // The law belongs in the key: two laws in one traject can share article
  // numbers ("B 5", "1", "2" are not unique), and a law switch then leaves
  // `selectedArticleNumber` as it is while the panes went back to the saved law.
  // On the number alone the proposal would not go in again on return.
  const reviewSeededAt = ref<string | null>(null);
  function reviewSeedKey(number: string | null | undefined) {
    return `${lawId.value ?? ''}::${number ?? ''}`;
  }
  watch([lawId, selectedArticleNumber], ([, nr]) => {
    if (reviewSeededAt.value && reviewSeededAt.value !== reviewSeedKey(nr)) {
      reviewSeededAt.value = null;
    }
  });

  function applyProposedContent(proposedYaml: string) {
    reviewSeeded.value = false;
    reviewHasHiddenChanges.value = false;
    let proposed: LawDocument | null;
    try {
      // Unchecked YAML; `articles` is checked with Array.isArray below.
      proposed = yaml.load(proposedYaml) as LawDocument | null;
    } catch {
      return; // malformed proposal - leave the saved content in place
    }
    const proposedArticles: LawArticle[] = Array.isArray(proposed?.articles) ? proposed.articles : [];
    // v1 can only seed an EXISTING article as an unsaved edit (same
    // single-article model as `currentLawYaml`'s KNOWN LIMITATION below,
    // which has no way to splice in an article the saved law doesn't have),
    // and can't show a removed article either - proposalDivergence folds both
    // into `hiddenChanges` so the banner points at the YAML panel for them.
    const { target, hiddenChanges } = proposalDivergence(
      articles.value,
      proposedArticles,
      reviewArticleNumber.value,
    );
    reviewHasHiddenChanges.value = hiddenChanges;
    if (!target) return; // nothing seedable differs - nothing to seed
    reviewSeeded.value = true;
    reviewSeedTarget.value = String(target.number);
    // Set before seeding: the watches that clean up on an article switch and
    // re-seed on return run in the next flush, and must already see this as
    // seeded - otherwise they would seed over it again.
    reviewSeededAt.value = reviewSeedKey(String(target.number));
    // Selects the article and puts the proposal in its edits, in one call. The
    // proposal is the leave guard's baseline: until the reviewer edits it,
    // leaving loses nothing, since it is seeded again on return.
    host.openLaw.seedProposal(target);
  }

  // Variant + copy for the review-mode bar (a low, full-width nldd-banner -
  // see PR #935 UX feedback; it used to be a page-height nldd-inline-dialog
  // that ate half the viewport). Opslaan always approves the FULL proposal
  // (see `handleLawSave`), so the copy says so explicitly; when the proposal
  // touches more than the single seeded article (or nothing could be seeded
  // at all), point the reviewer at the YAML panel (`panel.yaml_editor`) for
  // the parts the article-scoped editor can't show. A bar has no room for
  // paragraphs, so copy stays to a heading line + one short supporting line.
  const REVIEW_HIDDEN_CHANGES_NOTE = 'Zie ook het YAML-paneel voor wijzigingen buiten dit artikel.';
  // Accent, not neutral: a generated proposal is the one thing on screen that
  // asks for a decision, and a neutral bar reads as background chrome.
  const reviewBannerVariant = computed(() => {
    if (loadError.value) return 'critical';
    if (reviewIsLawCreate.value) return 'accent';
    if (reviewActive.value && !reviewSeeded.value) return 'warning';
    return 'accent';
  });
  const reviewBannerText = computed(() =>
    reviewIsLawCreate.value ? 'Nieuw regelwerk uit documentconversie' : 'Voorstel uit verrijking',
  );
  const reviewBannerSupportingText = computed(() => {
    if (loadError.value) return loadError.value;
    if (reviewIsLawCreate.value) {
      return 'Controleer het regelwerk; Opslaan voegt het toe aan het traject, Verwerpen wijst af.';
    }
    if (!reviewSeeded.value) {
      return reviewArticleNumber.value
        ? `Artikel ${reviewArticleNumber.value} wijkt niet af van het voorstel; er valt niets te wijzigen.`
        : 'Voorstel raakt alleen inhoud die hier niet zichtbaar is - bekijk het YAML-paneel.';
    }
    // Artikel-scoped: het oordeel gaat over dit artikel zoals het hier staat,
    // dus eigen aanpassingen gaan wél mee.
    if (!reviewSavesWholeLaw.value) {
      return `Overnemen bewaart artikel ${reviewArticleNumber.value} zoals het hier staat, Verwerpen laat het ongemoeid.`;
    }
    return (
      'Overnemen neemt het volledige voorstel over (eigen aanpassingen gaan niet mee), Verwerpen wijst af.' +
      (reviewHasHiddenChanges.value ? ` ${REVIEW_HIDDEN_CHANGES_NOTE}` : '')
    );
  });

  // Which panes carry the proposal, for the review notice. An enrichment writes
  // machine_readable and scenarios, not the law text, so Tekst is not among
  // them. YAML always shows the whole proposal; Machine only when seeding
  // worked. Scenario's is left out on purpose: the worker does stage
  // `features/*.feature`, but `loadReview` only takes the law YAML from the
  // result blobs, so no scenario ever arrives.
  const reviewChangedPanes = computed(() => {
    const panes = [];
    if (reviewSeeded.value) panes.push('Machine');
    panes.push('YAML');
    return panes;
  });
  const reviewStatusText = computed(() => {
    if (loadError.value) return loadError.value;
    const what = reviewIsLawCreate.value
      ? 'Dit is een nieuw regelwerk uit documentconversie'
      : 'Dit is een gegenereerd voorstel';
    const panes = reviewChangedPanes.value;
    const list =
      panes.length > 1 ? `${panes.slice(0, -1).join(', ')} en ${panes.at(-1)}` : panes[0];
    return `${what}. Beoordeel ${list}.${reviewProgressNote.value}`;
  });

  // Waar de beoordelaar in de verrijking staat, en - zolang er nog onderdelen
  // open staan - dat de wet nog niet is bijgewerkt. Zonder die tweede zin leest
  // "Neem voorstel over" als "opgeslagen", terwijl er tot het laatste oordeel
  // niets is weggeschreven.
  const reviewProgressNote = computed(() => {
    if (reviewIsLawCreate.value || partCount.value <= 1) return '';
    const position =
      partIndex.value > 0
        ? ` Onderdeel ${partIndex.value} van ${partCount.value}.`
        : '';
    return undecidedParts.value.length > 1
      ? `${position} Het regelwerk wordt pas bijgewerkt als alle onderdelen zijn beoordeeld.`
      : `${position} Dit is het laatste onderdeel; daarna wordt het regelwerk bijgewerkt.`;
  });

  // Fires once the law + its first article have finished loading (whether
  // that's the initial load or a tab-restore open), so it works
  // regardless of how the route.query.task navigation happened to arrive.
  // `reviewTaskIdParam` is itself a source: navigating from the taken-lijst
  // to a wet that is ALREADY open (the target law/article is unchanged) is a
  // query-only route change - `loading`/`selectedArticle` never flip, so
  // without this source the watch would simply never re-fire and the review
  // would never activate.
  watch(
    [loading, selectedArticle, reviewTaskIdParam],
    ([isLoading, article, taskId]) => {
      if (isLoading || !article || !taskId) return;
      if (reviewAttemptedForTaskId === taskId) return;
      reviewAttemptedForTaskId = taskId;
      loadReview(taskId).then(() => {
        if (!proposedContent.value) return;
        // Alleen seeden waar de taak over gaat - dezelfde vraag als voor de balk,
        // dus dezelfde `reviewOnTarget`. Een taak die één artikel aanwijst hoort
        // in dát artikel te landen, en `applyProposedContent` springt er zo nodig
        // heen. Meestal ben je er al (de takenlijst linkt erheen), maar doordat de
        // taak meereist met de navigatie staat `?task=` ook op de route van een
        // ander artikel - en van een andere wet. Kom je daar binnen (een refresh
        // terwijl je even elders keek), dan wint wat er op het scherm staat: niet
        // ongevraagd wegspringen, en zeker het voorstel niet in vreemde inhoud
        // laten landen. Het artikelnummer alleen is daarvoor geen toets: nummers
        // als "B 5" bestaan in meerdere wetten. Het seed-doel wordt wel
        // vastgelegd, zodat de her-seed-watch het oppakt zodra je er terug bent.
        if (!reviewOnTarget.value) {
          reviewSeedTarget.value = reviewArticleNumber.value;
          return;
        }
        applyProposedContent(proposedContent.value);
      });
    },
    { immediate: true },
  );

  // `law_create`-variant van de watch hierboven: de wet bestaat nog niet, dus
  // de load eindigt in een 404 en `selectedArticle` komt er nooit - de normale
  // watch kan niet vuren. Bij een 404 mét ?task= halen we de taak alsnog op;
  // is het een law_create-taak voor precies deze wet, dan wordt het voorstel
  // integraal geseed (`seedFromYaml`) in plaats van gespliced - de hele wet ÍS
  // het voorstel. Elke andere combinatie laat de bestaande 404-foutstaat staan.
  watch(
    [error, reviewTaskIdParam],
    ([err, taskId]) => {
      if (!taskId || err?.status !== 404) return;
      if (reviewAttemptedForTaskId === taskId) return;
      reviewAttemptedForTaskId = taskId;
      loadReview(taskId).then(() => {
        const task = reviewTask.value;
        if (
          task?.payload?.kind === 'law_create' &&
          task?.payload?.law_id === lawId.value &&
          proposedContent.value
        ) {
          seedFromYaml(proposedContent.value);
          // Niets is als dirty edit geseed: de banner levert de primaire
          // opslaan-knop (zie `reviewSeeded` in de template), niet de
          // Wijzigingenbalk.
          reviewSeeded.value = false;
          reviewHasHiddenChanges.value = false;
        }
      });
    },
    { immediate: true },
  );

  // De keerzijde van de twee watches hierboven: `?task=` is niet alleen hoe een
  // review begint, het is ook wat hem gaande houdt. Elke artikel- of wetwissel in
  // de editor (een tabwissel, `selectTab`) stampt de URL opnieuw met
  // `editorRouteFor`, en die draagt geen query - de taak valt er dus vanzelf uit
  // zodra je iets anders bekijkt. Zonder deze watch bleef `reviewTask` staan: de
  // beoordeel-balk hing over een artikel waar de taak niet over gaat, terwijl het
  // geseede voorstel allang was teruggezet door `watch(selectedArticle)`. Erger
  // dan lelijk: "Opslaan" keurde de taak dan goed met de inhoud van dát andere
  // artikel, en zolang de modus aanstond hield `reviewReady` de knop "Beoordeel
  // voorstel" verborgen waarmee je terug had gekund.
  //
  // De guard gaat mee terug op null: kom je later met dezelfde taak terug (via de
  // takenlijst of "Beoordeel voorstel"), dan hoort het voorstel opnieuw geladen en
  // geseed te worden in plaats van als "al geprobeerd" te worden overgeslagen.
  watch(reviewTaskIdParam, (taskId) => {
    if (taskId) return;
    reviewAttemptedForTaskId = null;
    reset();
    reviewSeeded.value = false;
    reviewHasHiddenChanges.value = false;
    reviewSeedTarget.value = null;
    reviewSeededAt.value = null;
  });

  // Terug op het artikel van de taak: zet het voorstel er weer in. De panes zijn
  // bij de artikelwissel teruggezet naar de opgeslagen wet, dus zonder dit zie je
  // een beoordeling zonder iets te beoordelen - balk aan, voorstel weg. Dat was
  // de tweede helft van de gemelde bug.
  //
  // `law_create` blijft erbuiten: daar is de wet zelf het voorstel en is het
  // integraal geseed (`seedFromYaml`), niet als artikel-splice.
  watch([reviewOnTarget, selectedArticle], ([onTarget, article]) => {
    if (!onTarget || !article || reviewIsLawCreate.value) return;
    if (!proposedContent.value) return;
    if (reviewSeededAt.value === reviewSeedKey(String(article.number))) return;
    // Waar het voorstel heen moet. Bij een taak over één artikel ligt dat vast;
    // bij een voorstel over de hele wet kiest `proposalDivergence` het artikel, en
    // is het doel dus pas bekend na de eerste keer seeden. Kwam je binnen op een
    // andere wet, dan is dat nog niet gebeurd - laat `applyProposedContent` het
    // dan alsnog bepalen (die springt zo nodig zelf naar het juiste artikel).
    if (reviewSeedTarget.value) {
      if (String(article.number) !== reviewSeedTarget.value) return;
    } else if (reviewArticleNumber.value) {
      // Artikel-taak zonder doel: `applyProposedContent` vond niets seedbaars.
      return;
    }
    applyProposedContent(proposedContent.value);
  });

  // "Verwerpen": bij een `law_create` is er niets samen te stellen - die taak
  // staat op zichzelf en wordt direct afgehandeld. Bij een verrijking is dit een
  // oordeel over één onderdeel, net als "Overnemen".
  async function rejectReview() {
    if (reviewIsLawCreate.value) {
      await rejectTask();
      // De wet bestaat niet (en komt er na verwerpen ook niet): terugroutes
      // naar de wetsroute zouden op een 404 landen, dus terug naar Home.
      router.replace(host.leaveRoute());
      return;
    }
    await decideReview('rejected');
  }

  // "Rond af, neem de rest niet over": de vastgelegde oordelen plus "niet
  // overnemen" voor alles wat nog open staat, in één verwerking. Zonder deze weg
  // zou een verrijking van dertig artikelen dertig klikken kosten voordat er ook
  // maar iets geschreven kan worden.
  async function rejectRestOfReview() {
    if (!reviewActive.value || reviewIsLawCreate.value) return;
    await processReview({ rejectRemaining: true });
  }

  /**
   * De inhoud die de beoordelaar met "Overnemen" accordeert.
   *
   * Voor een artikel-taak: het artikel zoals het nu in de panes staat - het
   * geseede voorstel plus wat de beoordelaar er zelf nog aan veranderde. Zelfde
   * splice-regels als `currentLawYaml`, maar dan alléén het artikel: wat dat
   * betekent voor het bestand stelt de backend samen, want alleen daar kan het
   * tegen precies de bytes waartegen de `If-Match` is gecontroleerd.
   *
   * `null` betekent "neem het ruwe voorstel over" - dat is wat de backend doet
   * als er geen inhoud meekomt. Dat is de juiste terugval wanneer de editor het
   * artikel niet kan tonen (het staat niet in de opgeslagen wet), want dan heeft
   * de beoordelaar er ook niets aan kunnen veranderen.
   */
  function acceptedReviewContent() {
    if (reviewSavesWholeLaw.value) return proposedContent.value;
    const base = selectedArticle.value;
    if (!base || String(base.number) !== reviewArticleNumber.value) return null;
    const patched = { ...base, text: editedText.value };
    if (machineReadable.value != null) {
      patched.machine_readable = machineReadable.value;
    } else {
      delete patched.machine_readable;
    }
    try {
      return yaml.dump(patched, DUMP_OPTIONS);
    } catch {
      return null;
    }
  }

  // Eén oordeel over het onderdeel dat nu open staat. Is het het laatste, dan
  // wordt de verrijking meteen verwerkt; anders schuift de editor door naar het
  // volgende onbeoordeelde onderdeel.
  async function decideReview(action: Verdict['action']) {
    const { done, next } = decide(
      action,
      action === 'approved' ? acceptedReviewContent() : null,
    );
    if (done) {
      await processReview();
      return;
    }
    // No review task is loaded (the review ended while the reject-confirmation
    // dialog was open): there is nothing to decide, so keep the panes as they are.
    if (!next) return;
    // De geseede wijziging is vastgelegd in het oordeel; de panes gaan terug
    // naar de opgeslagen wet voordat het volgende onderdeel eroverheen komt.
    host.discard();
    router.replace(reviewRouteForPart(next));
  }

  // De editor-route van een ander onderdeel van dezelfde verrijking, met de
  // taak-id als `?task=` zodat de bestemming hem oppakt.
  function reviewRouteForPart(part: JobTaskSummary) {
    const target = host.routeFor(lawId.value, part.article ?? selectedArticleNumber.value);
    return { ...target, query: { ...(target.query ?? {}), task: part.id } };
  }

  // Verwerk de verrijking: één schrijfactie, één commit, alle taken dicht. De
  // backend stelt de eindstand samen, dus de wet wordt daarna opnieuw geladen -
  // de client kent de uitkomst niet.
  const reviewProcessing = ref(false);
  const reviewProcessError = ref<RequestError | null>(null);
  async function processReview(options: { rejectRemaining?: boolean } = {}) {
    reviewProcessing.value = true;
    reviewProcessError.value = null;
    try {
      await processEnrichment(currentEtag.value, options);
      await reloadLaw();
      host.discard();
      // De wettekst kan gewijzigd zijn, dus notities moeten opnieuw ankeren.
      host.reloadNotes();
      clearReviewQuery();
    } catch (e) {
      // processEnrichment and reloadLaw reject with an Error (an ApiError for HTTP failures).
      reviewProcessError.value = e as RequestError;
    } finally {
      reviewProcessing.value = false;
    }
  }

  // "Overnemen" from the changes bar. For an enrichment it is a verdict on the
  // part that is open (`decideReview`); only a `law_create` saves here - that
  // task stands alone and the law does not exist yet, so it goes through the
  // create path (POST); later saves take the normal PUT.
  async function approve(): Promise<void> {
    if (!reviewIsLawCreate.value) {
      await decideReview('approved');
      return;
    }
    const content = reviewSavesWholeLaw.value ? proposedContent.value : undefined;
    if (reviewSavesWholeLaw.value && !content) return;
    const outcome = await host.openLaw.save({ content: content ?? undefined, create: true });
    if (outcome.status === 'failed') {
      // The failure is surfaced via the law's saveError; log for dev visibility.
      console.warn('saveLaw failed:', host.openLaw.saveError.value);
      return;
    }
    if (outcome.status !== 'saved') return;
    if (outcome.touchedText) host.reloadNotes();
    // A successful save IS the approval (save first, then resolve).
    if (reviewActive.value) {
      await approveAfterSave();
      clearReviewQuery();
    }
  }

  return {
    /** The review task in the URL, which editor routes carry along. */
    taskId: reviewTaskIdParam,
    /** Review mode is on: a task is loaded and the open article is the one it is about. */
    active: reviewActive,
    isLawCreate: reviewIsLawCreate,
    loadError,
    statusText: reviewStatusText,
    variant: reviewBannerVariant,
    /** More than one part is still undecided, so "finish, don't take the rest" is a real shortcut. */
    canRejectRest: computed(() => reviewActive.value && !reviewIsLawCreate.value && undecidedParts.value.length > 1),
    processing: reviewProcessing,
    processError: reviewProcessError,
    approve,
    reject: rejectReview,
    rejectRest: rejectRestOfReview,
  };
}
