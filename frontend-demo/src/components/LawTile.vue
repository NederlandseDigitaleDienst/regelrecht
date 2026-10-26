<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import OrgLogo from './OrgLogo.vue';
import DataLineage from './DataLineage.vue';
import TraceView from './TraceView.vue';
import WhySheet from './WhySheet.vue';
import { whyUnlocked } from '../why/why';
import { fieldSpec, formatMissing, formatValue, humanize, isUnknown, verdictOf } from '../data/format';
import { lineageFromTrace, leafValues, type LineageNode } from '../data/lineage';
import { askedInputsFor, claimKeyFor, evaluationParamsFor, nextQuestions, type AskedInput } from '../data/askedInputs';
import { dateInputFor, phraseOutcome, phrasingFor } from '../data/outcomePhrasing';
import { driftSentence } from '../data/caseDrift';
import { useDemo } from '../store/demoStore';
import { canBeApplied, objectionOpen, statusOf } from '../data/lifecycle';
import { decisionOf } from '@regelrecht/frontend-shared/produces';
import { useLocalePath } from '../i18n/useLocalePath';
import { activeLocale, useI18n } from '../i18n/index.js';
import type { LawEntry } from '../data/loadCorpus';
import type { EvaluateResult } from '../engine/useDemoEngine';
import type { Produces } from '@regelrecht/frontend-shared/produces';

// Naar een ander tabblad op naam, niet op pad: onder `/en/` leidt een
// letterlijk Nederlands pad de bezoeker ongemerkt het Nederlandse tabblad in.
const { goTo } = useLocalePath();
const { t } = useI18n();

// One regeling on the portal: the outcome of the law for this persona, the
// values it used (expandable, each correctable), the application button and
// the state of a submitted case. Re-evaluates whenever registered data changes.

/** A value the citizen wants to correct or supply: what `edit-value` carries. */
interface EditValueRequest {
  law: LawEntry;
  /** Set when the citizen supplies a value no register holds. */
  selfDeclared?: boolean;
  node: LineageNode;
}
/** The tile's law with its latest evaluation: what `evaluated` and `apply` carry. */
interface TileEvaluation {
  law: LawEntry;
  evaluation: EvaluateResult | null;
}

// In the template: `corpus!`, because the portal renders tiles from the loaded
// corpus only.
const props = defineProps<{
  law: LawEntry;
}>();
const emit = defineEmits<{
  'edit-value': [request: EditValueRequest];
  evaluated: [result: TileEvaluation];
  apply: [result: TileEvaluation];
}>();
const demo = useDemo();
const { corpus, dataVersion, profile, personaParams, findCase, caseDrift, canSubmitClaims } = demo;

const evaluation = ref<EvaluateResult | null>(null);
const showData = ref(false);
const traceSheet = ref<NLDDSheet | null>(null);
const showTrace = ref(false);
const showWhy = ref(false);

function run() {
  evaluation.value = demo.evaluate(props.law, evaluationParams());
  emit('evaluated', { law: props.law, evaluation: evaluation.value });
}

const doc = computed(() => props.law.doc);
const primaryOutputName = computed(() => {
  const cfg = corpus.value?.config?.dashboard_outputs ?? {};
  return cfg[`${props.law.service}/${props.law.law_path}`] ?? null;
});
const outputs = computed(() => {
  const o = evaluation.value?.ok ? evaluation.value.outputs : {};
  return Object.entries(o).filter(([k]) => k !== 'voldoet_aan_voorwaarden');
});
// true/false when the law decided; 'unknown' when it could not for lack of
// facts (RFC-036), which is never shown as a yes; a law without a verdict
// output (a tax, a registration) is computed for everyone.
const verdict = computed(() => {
  if (!evaluation.value?.ok) return null;
  const v = verdictOf(evaluation.value.outputs);
  return v === null ? true : v;
});
const requirementsMet = computed(() => verdict.value === true);
const lawName = (id: string) => corpus.value?.lawById(id)?.name ?? id;
// What the verdict lacks, when it is unknown.
// An 'unknown' verdict implies an ok evaluation; the second test only narrows the type.
const verdictMissing = computed(() => (verdict.value === 'unknown' && evaluation.value?.ok ? formatMissing(evaluation.value.outputs.voldoet_aan_voorwaarden, { ownLaw: props.law.id, lawName }) : ''));
const primary = computed(() => {
  if (!evaluation.value?.ok) return null;
  const name = primaryOutputName.value && evaluation.value.outputs[primaryOutputName.value] !== undefined
    ? primaryOutputName.value
    : outputs.value.find(([, v]) => typeof v === 'number')?.[0] ?? outputs.value[0]?.[0];
  if (!name) return null;
  return { name, value: evaluation.value.outputs[name], spec: fieldSpec(doc.value, name) };
});
/**
 * What the tile shows under the outcome.
 *
 * A citizen asks three things: do I get it, how much, and do I have to do
 * something. Everything else is justification, and that belongs one click away
 * (Berekening, Gebruikte gegevens, Wettekst) where it can also be corrected.
 * Showing every remaining output put the reasoning on the front page —
 * "Uitzondering caribisch nederland van toepassing: Nee" for someone living in
 * the Netherlands.
 *
 * `tile_details` in demo-config.yaml names the values that do matter, per law;
 * an empty list means the outcome speaks for itself. A law with no entry keeps
 * the old behaviour, so this grows law by law.
 */
const secondary = computed(() => {
  // Een lege tekst ("geen weigeringsgrond") zegt niets en blijft weg, net als in de aanvraag.
  const rest = outputs.value.filter(([k, v]) => k !== primary.value?.name && v !== '');
  const wanted = corpus.value?.config?.tile_details?.[`${props.law.service}/${props.law.law_path}`];
  if (!wanted) return rest.slice(0, 6);
  return wanted.map((name) => rest.find(([k]) => k === name)).filter((e): e is [string, unknown] => Boolean(e));
});

// The tile's sentence (see outcomePhrasing.ts): a lead, the outcome big, and
// the unit after it — "Je huurtoeslag is waarschijnlijk € 302,96 per jaar".
// Null for a law nobody wrote wording for, and then the general rendering
// below stands unchanged.
const phrasing = computed(() => phrasingFor(corpus.value?.config, props.law.service, props.law.law_path));
// The date a yes/no law is about ("de verkiezingen van 29 oktober 2025"). It
// is an input the law resolved, so it is in the lineage the tile already
// builds; absent, the lead degrades to a sentence without a date.
const outcomeDate = computed(() => {
  const name = dateInputFor(phrasing.value);
  if (!name) return null;
  const hit = leafValues(lineage.value).find((n) => n.name === name);
  return hit && !isUnknown(hit.value) && hit.value !== null ? formatValue(hit.value, null) : null;
});
const phrased = computed(() => {
  if (!primary.value) return null;
  return phraseOutcome(phrasing.value, {
    met: requirementsMet.value,
    value: formatValue(primary.value.value, primary.value.spec),
    unknown: isUnknown(primary.value.value),
    isYesNo: typeof primary.value.value === 'boolean',
    date: outcomeDate.value,
  });
});

const lineage = computed(() => {
  if (!evaluation.value?.trace) return [];
  return lineageFromTrace(evaluation.value.trace, props.law.id, evaluationParams());
});
const valueCount = computed(() => leafValues(lineage.value).length);

const currentCase = computed(() => findCase(props.law));
/**
 * Een lopende aanvraag die niet meer klopt met wat de wet nu zegt.
 *
 * De burger heeft ergens een gegeven gewijzigd — misschien bij een hele
 * andere regeling — en deze wet rekent daar al mee, terwijl de aanvraag nog
 * op het oude bedrag staat. Dat hoort hij te zien op de plek waar dat besluit
 * staat, niet pas als het geld anders binnenkomt.
 */
const drift = computed(() => caseDrift(props.law, evaluation.value));
const driftText = computed(() => driftSentence(drift.value, doc.value));

// The questions this regeling has for the citizen (see askedInputs.ts): the
// inputs no register holds and the application-form parameters. Until
// answered they are unknown; the tile asks for them instead of showing an
// error, and the answers are passed as parameters where the law wants them.
const askedInputs = computed(() => {
  void dataVersion.value;
  return corpus.value ? askedInputsFor(corpus.value, props.law, demo.claimFor) : [];
});
// What the engine ran into that only the citizen can answer (just in time).
// The template reads `missingInputs[0]!` only under `v-else-if="missingInputs.length"`.
const missingInputs = computed(() => nextQuestions(askedInputs.value, evaluation.value, props.law.id));
/**
 * The self-supplied values this tile shows: the ones already answered, plus the
 * ones the law actually ran into.
 *
 * Not every question a law could ask is a question for this person. The kieswet
 * declares "ingezetenschapsduur jaren" and "werkzaam in nederlandse openbare
 * dienst" for the exception that keeps Dutch nationals abroad enfranchised; for
 * someone living in the Netherlands the law never reaches them, and listing
 * them anyway asks for paperwork nobody needs and suggests the outcome is
 * incomplete when it is settled. `nextQuestions` already knows which ones the
 * engine hit (RFC-036 reports the facts it missed), so the tile follows that.
 */
const ownInputs = computed(() => {
  const answered = askedInputs.value.filter((a) => a.claim);
  const asked = missingInputs.value.filter((a) => !a.claim);
  return [...answered, ...asked];
});

function evaluationParams() {
  return evaluationParamsFor(personaParams(), askedInputs.value);
}
// After the computeds it reads: the immediate run needs `askedInputs`.
watch([dataVersion, () => profile.value?.bsn], run, { immediate: true });

function supply(input: AskedInput) {
  const params = personaParams();
  emit('edit-value', {
    law: props.law,
    selfDeclared: true,
    node: { kind: 'value', law: props.law.id, name: input.name, value: input.claim?.newValue ?? null, service: null, ...claimKeyFor(props.law, params) },
  });
}
/**
 * Correct an outcome of this law: the aggregates on the tile (the primary
 * amount and the secondary rows) are claims like any other value, so citizen
 * and caseworker can both dispute them, as they could in the POC.
 */
function correctOutcome(name: string, value: unknown) {
  const params = personaParams();
  emit('edit-value', {
    law: props.law,
    node: { kind: 'law', law: props.law.id, name, value, service: props.law.service, ...claimKeyFor(props.law, params) },
  });
}
// De beslissing van het eerste artikel dat iets voortbrengt, in beide vormen
// van `produces` (schema v0.8.1).
const produces = computed(() => {
  for (const a of doc.value.articles ?? []) {
    const p = a.machine_readable?.execution?.produces;
    if (p) return decisionOf(p);
  }
  return null;
});
// Een machtiging zonder het recht om aanvragen in te dienen mag alleen kijken:
// dan verdwijnen de knoppen, niet alleen hun werking. Een aanslag vraag je niet
// aan (`canBeApplied`): de precariotegel bood Claudia eerst aan om haar eigen
// belasting aan te vragen.
// A `Decision` is the object form of `produces`; it only lacks ProducedItem's
// index signature, hence the cast for `canBeApplied`.
const canApply = computed(() => canSubmitClaims.value && evaluation.value?.ok && verdict.value === true && !currentCase.value && canBeApplied(produces.value as Produces));
/** A decided-and-rejected case the citizen has not objected to yet (Awb art. 6:5). */
// Bezwaar pas als de termijn loopt: die begint de dag ná de bekendmaking
// (Awb 6:8), dus een besluit dat nog niet is verstuurd geeft nog geen knop.
const canObject = computed(() => canSubmitClaims.value && currentCase.value?.approved === false && objectionOpen(currentCase.value));

// The application stays in the portal (a sheet); the case system is the
// caseworker's world.
function apply() {
  emit('apply', { law: props.law, evaluation: evaluation.value });
}

/**
 * What the "why" explanation is written from: the outcome as this tile shows
 * it, already formatted (so the model copies "€ 1.234,56" instead of dividing
 * cents itself), and the engine's full trace for the reasoning behind it.
 */
const whyPayload = computed(() => {
  if (!showWhy.value || !evaluation.value?.ok) return null;
  const all = evaluation.value.outputs ?? {};
  return {
    locale: activeLocale.value,
    law: { id: props.law.id, name: props.law.name, service: corpus.value?.services[props.law.service]?.name ?? props.law.service },
    reference_date: demo.state.referenceDate,
    headline: phrased.value ? [phrased.value.lead, phrased.value.headline, phrased.value.unit].filter(Boolean).join(' ') : null,
    outcome: Object.entries(all).map(([name, value]) => ({ label: humanize(name), value: formatValue(value, fieldSpec(doc.value, name)) })),
    trace_text: evaluation.value.traceText ?? '',
  };
});

watch(showTrace, async (open) => {
  if (!open) return traceSheet.value?.hide?.();
  await nextTick();
  traceSheet.value?.show?.();
});

const statusTag = computed(() => {
  const c = currentCase.value;
  if (!c) return null;
  // Uit de fase en niet uit het opgeslagen veld: die twee horen hetzelfde te
  // zeggen, en als er ooit één achterloopt is de fase de bron (zie lifecycle.ts).
  const status = statusOf(c);
  if (status === 'DECIDED') return c.approved ? { color: 'success', text: t('wet.tile.status.granted'), icon: 'checked' } : { color: 'critical', text: t('wet.tile.status.refused'), icon: 'dismiss-circle' };
  if (status === 'IN_REVIEW') return { color: 'warning', text: t('wet.tile.status.in_review'), icon: 'clock' };
  return { color: 'neutral', text: t('wet.tile.status.submitted'), icon: 'paper-plane' };
});
</script>

<template>
  <nldd-card :accessible-label="law.name">
    <nldd-container slot="header" padding="20" layout="row" gap="12" vertical-alignment="top">
      <OrgLogo :service="law.service" />
      <nldd-title-cell size="5" :text="law.name" :supporting-text="corpus!.services[law.service]?.name ?? law.service"></nldd-title-cell>
      <nldd-tag v-if="statusTag" :color="statusTag.color" :text="statusTag.text" :icon="statusTag.icon" size="sm"></nldd-tag>
    </nldd-container>

    <nldd-container padding-inline="20" padding-bottom="20" gap="16">
      <template v-if="!evaluation">
        <nldd-activity-indicator timing="instant" size="24"></nldd-activity-indicator>
      </template>
      <template v-else-if="missingInputs.length">
        <nldd-inline-dialog icon="edit" :text="t('wet.tile.need_fact.title')" :supporting-text="t('wet.tile.need_fact.body', { field: humanize(missingInputs[0]!.name).toLowerCase() })"></nldd-inline-dialog>
      </template>
      <template v-else-if="!evaluation.ok">
        <nldd-inline-dialog variant="alert" :text="t('wet.tile.error.title')" :supporting-text="evaluation.error"></nldd-inline-dialog>
      </template>
      <template v-else>
        <!-- Wat er al lag klopt niet meer met wat de wet nu zegt. Geen
             herberekening die iets besluit: alleen de constatering, met de
             weg terug ernaast (de knop "Wijzigen" in de voettekst). Het
             besluit zelf blijft staan tot de burger zijn aanvraag wijzigt of
             een behandelaar ernaar kijkt. Boven de uitkomst en niet in plaats
             daarvan: het nieuwe bedrag is juist wat hij moet zien.

             Een banner en geen inline-dialog: die laatste is de lege-staat, met
             het icoon bóven de tekst en alles gecentreerd. Dat leest als een
             andere soort mededeling dan de rest van de tegel, terwijl dit er
             juist naast hoort te staan. De aanvraag gebruikt dezelfde banner,
             dus beide kanten zien hetzelfde. -->
        <nldd-banner
          v-if="drift"
          variant="warning"
          :text="t('wet.tile.drift.title')"
          :supporting-text="driftText"
        ></nldd-banner>
        <nldd-inline-dialog v-if="verdict === 'unknown'" icon="info" :text="t('wet.tile.undecidable.title')" :supporting-text="t('wet.tile.undecidable.body', { missing: verdictMissing })"></nldd-inline-dialog>
        <nldd-list v-else appearance="box-tinted" :accessible-label="t('wet.tile.outcome.label')">
          <nldd-list-item size="md" :button="primary ? true : undefined" @click="primary && correctOutcome(primary.name, primary.value)">
            <nldd-icon-cell :icon="requirementsMet ? 'check-mark-circle' : 'dismiss-circle'" :color="requirementsMet ? 'success' : 'critical'"></nldd-icon-cell>
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <!-- The law's own sentence, when it has one: the lead as overline,
                 the outcome as the title, the unit under it. Same cell as the
                 general rendering below, so the pencil and the row keep working. -->
            <!-- Size follows what the headline is. With a lead it is a short
                 figure ("€ 406,95", "STEMRECHT") and carries the tile, so it is
                 large. Without one the headline IS the whole sentence ("Je krijgt
                 waarschijnlijk geen kindgebonden budget."), and set that large it
                 shouts a non-result across three lines. -->
            <nldd-title-cell
              v-if="phrased"
              :size="phrased.lead ? '3' : '5'"
              :overline="phrased.lead || undefined"
              :text="phrased.headline"
              :supporting-text="phrased.unit || undefined"
            ></nldd-title-cell>
            <nldd-title-cell
              v-else
              size="4"
              :overline="requirementsMet ? t('wet.tile.conditions.met') : t('wet.tile.conditions.not_met')"
              :text="requirementsMet ? (primary ? formatValue(primary.value, primary.spec) : t('wet.tile.outcome.yes')) : t('wet.tile.outcome.not_applicable')"
              :supporting-text="requirementsMet && primary ? (isUnknown(primary.value) ? t('wet.tile.outcome.missing', { field: humanize(primary.name), missing: formatMissing(primary.value, { ownLaw: law.id, lawName }) }) : humanize(primary.name)) : ''"
            ></nldd-title-cell>
          </nldd-list-item>
        </nldd-list>

        <nldd-list v-if="secondary.length" appearance="simple" :accessible-label="t('wet.tile.other_outcomes')">
          <nldd-list-item v-for="[name, value] in secondary" :key="name" size="sm" button @click="correctOutcome(name, value)">
            <nldd-text-cell size="sm" color="secondary" min-width="55%" :text="humanize(name)"></nldd-text-cell>
            <nldd-text-cell size="sm" width="fit-content" max-width="45%" horizontal-alignment="right" :color="isUnknown(value) ? 'secondary' : 'content'" :text="formatValue(value, fieldSpec(doc, name))"></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell icon="edit" size="16" color="secondary"></nldd-icon-cell>
          </nldd-list-item>
        </nldd-list>

        <!-- These rows are questions the registers cannot answer, so only the
             citizen can. Until one is answered it is not "door jou opgegeven" —
             saying that of an empty row claims the person supplied something
             they never did. Unanswered reads as a question, answered says who
             gave the answer. -->
        <nldd-list v-if="ownInputs.length" appearance="simple" :accessible-label="t('wet.tile.own_inputs.label')">
          <nldd-list-item v-for="input in ownInputs" :key="input.name" size="sm" button @click="supply(input)">
            <nldd-icon-cell :icon="input.claim ? 'edit' : 'question-mark-circle'" size="16" :color="input.claim ? 'accent' : 'secondary'"></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell size="sm" :text="humanize(input.name)" :supporting-text="input.claim ? t('wet.tile.own_inputs.supplied') : t('wet.tile.own_inputs.only_you')"></nldd-text-cell>
            <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :color="input.claim ? 'content' : 'secondary'" :text="input.claim ? formatValue(input.claim.newValue, input.spec) : t('wet.tile.own_inputs.empty')"></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
        <!-- Outlined, not tinted. The tinted box is the answer; giving the same
             fill to a link into the reasoning made the two read as equals, and
             the eye had nowhere to land. This one is a door, not a statement. -->
        <nldd-list type="tree" appearance="box-base" :accessible-label="t('wet.tile.data.label')">
          <!-- Only once the presenter unlocked it from the menu: the
               explanation runs on a language model behind a password. -->
          <nldd-list-item v-if="whyUnlocked && evaluation.traceText" size="sm" button @click="showWhy = true">
            <nldd-icon-cell icon="question" size="16" color="secondary"></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell size="sm" :text="t('wet.tile.why.open')"></nldd-text-cell>
          </nldd-list-item>
          <nldd-list-item size="sm" button :expanded="showData" @click="showData = !showData">
            <nldd-icon-cell icon="rectangle-stack" size="16" color="secondary"></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell size="sm" :text="t('wet.tile.data.heading', { n: valueCount })" :supporting-text="showData ? t('wet.tile.data.hint') : undefined"></nldd-text-cell>
            <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
            <DataLineage v-if="showData" :nodes="lineage" nested @edit="emit('edit-value', { node: $event, law })" />
          </nldd-list-item>
        </nldd-list>
      </template>
    </nldd-container>

    <!-- wrap, not row: in a narrow tile (three-column grid) a whole button moves to a
         second line instead of a button breaking its label. The buttons stay direct
         children: a nested container has size containment and so no intrinsic width
         in a flex line.
         No flexible spacer between the primary and the secondary actions. It ate
         every leftover pixel, so a long primary label ("Gegevens aanvullen") pushed
         the row over its width and dropped "Wettekst" alone onto a second line,
         while a short one ("Aanvragen") kept all three together. The footer then
         looked different from tile to tile for no reason the reader can see. Left
         alignment with a plain gap wraps the same way for every label length.
         The primary label is kept short for the same reason: "Gegevens aanvullen"
         made the three buttons 374px wide in a 384px footer, so padding and gaps
         pushed "Wettekst" onto a second line while every neighbouring tile kept
         its buttons on one. The heading above the button already says which data
         is missing. -->
    <nldd-container slot="footer" padding="20" layout="wrap" gap="12" vertical-alignment="center">
      <!-- A rejected decision leads with the objection, not with the file. Awb
           art. 6:5 gives the citizen six weeks to disagree, and hiding that
           route one click deep behind the file made the tile a dead end: the POC
           put it in view. Once an objection is running the button goes back to
           the file, which is where its status is.
           Labels are one word for a measured reason: the three buttons have
           about 313px of a 384px footer before padding and gaps push the last
           one onto a second line. "Mijn aanvraag" needed 336px and "Bezwaar
           maken" 347px, so both wrapped; "Aanvraag" and "Bezwaar" fit. -->
      <nldd-button v-if="canObject" appearance="primary" size="sm" start-icon="flag" :text="t('wet.tile.action.object')" @click="apply"></nldd-button>
      <nldd-button v-else-if="drift" appearance="primary" size="sm" start-icon="edit" :text="t('wet.tile.action.amend')" @click="apply"></nldd-button>
      <nldd-button v-else-if="currentCase" appearance="secondary" size="sm" start-icon="file-text" :text="t('wet.tile.action.case')" @click="apply"></nldd-button>
      <!-- `data-highlight` is what the presentation slide pulses (demo-config.yaml,
           `highlight:`). It selected on the label once, `[text="Gegevens aanvullen"]`,
           and went dark without a sound when the label was shortened, and in every
           language but Dutch from the start. An attribute nobody translates or
           relabels does not have that failure; `slides.test.ts` checks it exists. -->
      <nldd-button v-else-if="canSubmitClaims && evaluation && missingInputs.length && produces?.legal_character === 'BESCHIKKING'" data-highlight="complete-data" appearance="primary" size="sm" start-icon="edit" :text="t('wet.tile.action.complete')" @click="apply"></nldd-button>
      <nldd-button v-else-if="canApply" appearance="primary" size="sm" start-icon="paper-plane" :text="t('wet.tile.action.apply')" @click="apply"></nldd-button>
      <nldd-button v-if="evaluation?.ok" appearance="neutral-transparent" size="sm" start-icon="list" :text="t('wet.tile.action.calculation')" @click="showTrace = true"></nldd-button>
      <nldd-button appearance="neutral-transparent" size="sm" start-icon="book" :text="t('wet.tile.action.law_text')" @click="goTo('wetten', { lawId: law.id })"></nldd-button>
    </nldd-container>

    <WhySheet :open="showWhy" :law="law" :payload="whyPayload" @close="showWhy = false" />
    <Teleport to="body">
      <nldd-sheet ref="traceSheet" placement="right" width="1400px" :accessible-label="t('wet.tile.trace.title')" @close="showTrace = false">
        <nldd-page>
          <nldd-container slot="header" padding="12">
            <nldd-top-title-bar :text="t('wet.tile.trace.title')" :supporting-text="law.name" :dismiss-text="t('wet.tile.trace.close')" @dismiss="showTrace = false"></nldd-top-title-bar>
          </nldd-container>
          <nldd-container padding="16">
            <nldd-rich-text spacing="tight"><p>{{ t('wet.tile.trace.intro') }}</p></nldd-rich-text>
            <nldd-spacer size="12"></nldd-spacer>
            <TraceView v-if="showTrace" :text="evaluation?.ok ? evaluation.traceText : undefined" />
          </nldd-container>
        </nldd-page>
      </nldd-sheet>
    </Teleport>
  </nldd-card>
</template>
