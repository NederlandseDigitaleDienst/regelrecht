/**
 * useEnrichState - de verrijk-/beoordeel-staat van één wet, zoals de lege
 * Machine- en YAML-panes die tonen: loopt er een verrijking, staat er een
 * voorstel klaar, en waar moet je heen als je erop klikt.
 *
 * Bestaat omdat Bibliotheek en Editor allebei dezelfde wet in dezelfde panes
 * tonen en dus hetzelfde moeten melden. Die blokken stonden regel-voor-regel
 * dubbel en liepen uit elkaar: de ene helft kreeg wél een refresh van de
 * gedeelde takenlijst en de andere niet, waardoor de editor bij een deeplink
 * "Verrijk deze wet" aanbood terwijl er al een voorstel klaarlag.
 *
 * Bewust op useTaskActions gebouwd en niet op useTasks: dit is de niet-pollende
 * helft. Een view die een wet toont mag de 30s-poll van de takenlijst niet
 * ongevraagd starten (die draait ook voor anonieme bezoekers), dus verversen we
 * gericht: één keer bij mount, na elke eigen verrijk-aanvraag, en zolang je naar
 * de bezig-staat kijkt (usePollWhile).
 */
import { computed, onMounted, toValue, type MaybeRefOrGetter } from 'vue';
import { useRouter } from 'vue-router';
import { useAuth } from './useAuth';
import { useTaskActions, usePollWhile } from './useTasks';
import { reviewTarget, sameTraject } from '../lib/taskReview';
import type { Task } from '../lib/taskCategories';

export interface EnrichStateSources {
  trajectRef: MaybeRefOrGetter<string | null | undefined>;
  lawId: MaybeRefOrGetter<string | null | undefined>;
  articleNumber: MaybeRefOrGetter<string | number | null | undefined>;
  reviewActive?: MaybeRefOrGetter<boolean | undefined>;
}

// Structural view of a running job (`RunningTaskJob`, owned by useTasks); only
// the fields read here.
interface RunningJobView {
  job_type?: string;
  law_id?: string | null;
  traject_ref?: string | null;
}

/**
 * @param {object} sources - refs/getters van de view.
 * @param {import('vue').Ref<string|null>} sources.trajectRef - actief traject.
 * @param {import('vue').Ref<string|null>} sources.lawId - de getoonde wet.
 * @param {import('vue').Ref<string|number|null>} sources.articleNumber - het geopende artikel.
 * @param {import('vue').Ref<boolean>} [sources.reviewActive] - beoordeel je die
 *   taak op dit moment al? Dan hoeft de pane 'er ligt een voorstel klaar' niet
 *   te melden. Alleen de editor kent die modus.
 */
export function useEnrichState({ trajectRef, lawId, articleNumber, reviewActive }: EnrichStateSources) {
  const router = useRouter();
  const { authenticated } = useAuth();
  const {
    refresh: refreshTasks,
    requestEnrich: requestEnrichRaw,
    running: runningJobs,
    tasks: openTasks,
  } = useTaskActions();

  // Loopt er al een verrijking voor deze wet in dit traject? Dan meldt de lege
  // pane dat in plaats van opnieuw de knoppen aan te bieden; een tweede
  // aanvraag zou toch op een 409 stuiten.
  //
  // Ook hier op traject matchen: `running` is net als de takenlijst
  // account-breed, terwijl de 409 per (wet, traject) geldt
  // (`idx_unique_active_enrich_job`). Een verrijking die in traject A loopt
  // zette de pane in traject B anders op "bezig" en verborg daar de "Verrijk
  // deze wet"-knop voor een aanvraag die gewoon gehonoreerd zou worden.
  const isEnriching = computed(() => {
    const traject = toValue(trajectRef);
    if (!traject) return false;
    return runningJobs.value.some(
      (job: RunningJobView) =>
        job.job_type === 'enrich' &&
        job.law_id === toValue(lawId) &&
        sameTraject(job.traject_ref, traject),
    );
  });
  // Alleen pollen zolang je naar de bezig-staat kijkt; zie usePollWhile. Staat
  // hier ná isEnriching en achter de argumenten van deze composable: de watch
  // leest zijn bron meteen om de dependency te leggen, dus alles wat hij
  // aanraakt moet al bestaan. Doordat de view zijn refs als argument meegeeft,
  // is die volgorde hier structureel goed in plaats van een valkuil die elke
  // aanroeper zelf moet onthouden.
  usePollWhile(isEnriching);

  // Staat er al een beoordeelbaar voorstel klaar voor deze wet? Eén verrijking
  // levert een taak per gewijzigd artikel, dus meestal staan er meerdere open
  // voor deze wet. Je kijkt naar één artikel, dus de taak van dát artikel gaat
  // voor; is die er niet, dan de eerste van de wet - beter naar een naburig
  // voorstel wijzen dan naar niets.
  //
  // Ook op traject matchen, niet alleen op wet: de takenlijst is account-breed
  // en dezelfde wet leeft in meerdere trajecten. Een open taak uit traject A
  // kaapte anders de pane in traject B - inclusief de "Beoordeel voorstel"-knop,
  // die dan naar A navigeert en de "Verrijk deze wet"-knop van B verdringt.
  // Zonder actief traject (globale corpus-weergave) matcht er dus niets: een
  // voorstel hoort bij een traject en valt daarbuiten niet te beoordelen.
  //
  // Vergelijken via sameTraject en niet op de hele string: de taak draagt de
  // ref van het aanvraagmoment, en het slug-deel daarvan verandert mee met de
  // naam van het traject. Na een hernoeming zou een letterlijke vergelijking
  // het eigen voorstel niet meer herkennen.
  const pendingReviewTask = computed(() => {
    const traject = toValue(trajectRef);
    if (!traject) return null;
    const forLaw: Task[] = openTasks.value.filter(
      (t: Task) =>
        t.task_type === 'job_review' &&
        t.payload?.law_id === toValue(lawId) &&
        sameTraject(t.payload?.traject_ref, traject),
    );
    if (!forLaw.length) return null;
    const here = String(toValue(articleNumber) ?? '');
    // forLaw is non-empty (checked above).
    return forLaw.find((t) => String(t.payload?.article ?? '') === here) ?? forLaw[0]!;
  });
  const reviewReady = computed(
    () => !!pendingReviewTask.value && !toValue(reviewActive),
  );
  const reviewArticleForPane = computed(() =>
    pendingReviewTask.value?.payload?.article == null
      ? ''
      : String(pendingReviewTask.value.payload.article),
  );

  function openReviewForLaw() {
    const target = reviewTarget(pendingReviewTask.value);
    if (target) router.push(target);
  }

  // Naar de takenlijst, gefilterd op deze wet: daar staat de lopende aanvraag
  // als rij met activity-indicator.
  function openTasksForLaw() {
    const ref_ = toValue(trajectRef);
    const id = toValue(lawId);
    if (!ref_ || !id) return;
    router.push({
      name: 'taken-traject',
      params: { trajectRef: ref_, categorie: 'wet', contextLawId: id },
    });
  }

  // `running`/`tasks` worden alleen door een refresh gevuld; deze composable
  // pollt niet uit zichzelf. Eén keer bij mount is genoeg om na een herlaad of
  // een deeplink te weten wat er voor deze wet loopt of klaarstaat.
  onMounted(() => {
    if (authenticated.value) refreshTasks();
  });

  // Verrijken aanvragen mét de refresh erachteraan: zonder die refresh blijft
  // `isEnriching` false, slaat de bezig-staat niet om en begint usePollWhile
  // nooit te pollen. Die koppeling hoort hier en niet bij elke aanroeper - daar
  // is hij één keer vergeten.
  async function requestEnrich(provider?: string) {
    // Both callers (LibraryView, EditorView) only call this with a traject and
    // a law; nothing here checks that.
    const result = await requestEnrichRaw(toValue(trajectRef) as string, toValue(lawId) as string, provider);
    await refreshTasks();
    return result;
  }

  return {
    isEnriching,
    pendingReviewTask,
    reviewReady,
    reviewArticleForPane,
    openReviewForLaw,
    openTasksForLaw,
    requestEnrich,
    refreshTasks,
  };
}
