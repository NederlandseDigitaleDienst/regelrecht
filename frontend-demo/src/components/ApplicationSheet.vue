<script setup lang="ts">
import { computed, nextTick, reactive, ref, watch } from 'vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import DataLineage from './DataLineage.vue';
import { type FieldSpec, fieldSpec, formatDateTime, formatMissing, formatValue, humanize, verdictOf } from '../data/format';
import { type LineageNode, lineageFromTrace, leafValues } from '../data/lineage';
import { type AskedInput, askedInputsFor, claimKeyFor, evaluationParamsFor, inputKind, nextQuestions, parseAnswer } from '../data/askedInputs';
import type { DeclaredClaim } from '../data/declarations';
import type { LawEntry } from '../data/loadCorpus';
import type { EvaluateResult } from '../engine/useDemoEngine';
import { type CaseEvent, type DemoClaim, caseReason, eventText, useDemo } from '../store/demoStore';
import { t } from '../i18n/index.js';
import { awbOutcomes, canBeApplied, objectionOpen, statusOf } from '../data/lifecycle';
import { driftRows, driftSentence } from '../data/caseDrift';

// The citizen's side of an application, inside the portal. The flow the POC
// generated per regeling: first the questions only the citizen can answer
// (rent, service costs), then the outcome and the data it rests on to check,
// then the declaration and the submission, and afterwards the status of the
// application with, once decided, the possibility to object. The caseworker's
// side lives in the Zaaksysteem tab; nothing here jumps there.

const props = withDefaults(defineProps<{
  open?: boolean;
  law?: LawEntry | null;
  /** The tile's latest evaluation of this law (re-evaluated as data changes). */
  evaluation?: EvaluateResult | null;
}>(), {
  open: false,
  law: null,
  evaluation: null,
});
// `edit-value` carries `{ node, law }`, the shape LawTile emits, so the portal
// opens the same correction sheet for a value corrected from inside the application.
const emit = defineEmits<{
  close: [];
  'edit-value': [payload: { node: LineageNode; law: LawEntry }];
}>();

/**
 * A question's field, as `fieldSpec` returns it: the declared field from the
 * law, which also carries its description and, for a choice, its options.
 */
type QuestionSpec = FieldSpec & {
  description?: string;
  type_spec?: (FieldSpec['type_spec'] & { enum?: (string | number)[]; enum_labels?: Record<string, string> }) | null;
};
/** One question, with the citizen's earlier answer or the profile's declaration. */
type Question = AskedInput<DemoClaim | DeclaredClaim> & { spec: QuestionSpec | null };
/** The evaluation when it ran. */
type OkEvaluation = Extract<EvaluateResult, { ok: true }>;
/** The banner for the case's status. */
interface StatusView {
  variant: string;
  icon: string;
  text: string;
  supporting: string | null;
}
const demo = useDemo();
const { corpus, personaParams, claimFor, findCase, caseDrift, dataVersion } = demo;

const sheet = ref<NLDDSheet | null>(null);
const step = ref<'gegevens' | 'controleren' | 'status'>('gegevens'); // gegevens | controleren | status
const answers = reactive<Record<string, unknown>>({});
const declared = ref(false);
const error = ref('');
const objectionReason = ref('');
const justSubmitted = ref(false);

const service = computed(() => (props.law ? corpus.value?.services?.[props.law.service]?.name ?? props.law.service : ''));
const asked = computed((): Question[] => {
  void dataVersion.value;
  return props.law && corpus.value ? askedInputsFor(corpus.value, props.law, claimFor) : [];
});
// Just in time, as the POC did it: the engine runs with what it has, and the
// next question is the first thing it ran into that only the citizen knows.
const missing = computed(() => (props.law ? nextQuestions(asked.value, props.evaluation, props.law.id) : []));
const question = computed(() => missing.value[0] ?? null);
const answered = ref(0);
const currentCase = computed(() => {
  void dataVersion.value;
  return props.law ? findCase(props.law) : null;
});
const doc = computed(() => props.law?.doc);

// true/false when the law decided, 'unknown' when it could not for lack of
// facts (RFC-036): then there is nothing to submit yet. A law without a
// verdict output is computed for everyone.
const verdict = computed(() => {
  if (!props.evaluation?.ok) return null;
  const v = verdictOf(props.evaluation.outputs);
  return v === null ? true : v;
});
const requirementsMet = computed(() => verdict.value === true);
const lawName = (id: string) => corpus.value?.lawById(id)?.name ?? id;
// An 'unknown' verdict comes from an evaluation that ran.
const verdictMissing = computed(() => (verdict.value === 'unknown' ? formatMissing((props.evaluation as OkEvaluation).outputs.voldoet_aan_voorwaarden, { ownLaw: props.law?.id, lawName }) : ''));
const primaryName = computed(() => corpus.value?.config?.dashboard_outputs?.[`${props.law?.service}/${props.law?.law_path}`] ?? null);
const outcomeRows = computed(() => {
  const o = props.evaluation?.ok ? props.evaluation.outputs : {};
  const rows = Object.entries(o).filter(([k]) => k !== 'voldoet_aan_voorwaarden');
  // Dezelfde keuze als de tegel (`tile_details`), anders hing het af van de
  // volgorde van de uitvoer wat er in beeld kwam: bij de terrasvergunning viel
  // de weigeringsgrond als zevende weg, en stond er "Aanvragen heeft geen zin"
  // zonder te zeggen waarom. Een lege tekst is geen grond en blijft weg.
  const wanted = corpus.value?.config?.tile_details?.[`${props.law?.service}/${props.law?.law_path}`];
  if (wanted) {
    const pick = (name: string | null) => rows.find(([k, v]) => k === name && v !== '');
    return [primaryName.value, ...wanted].map(pick).filter((row): row is [string, unknown] => Boolean(row));
  }
  rows.sort(([a], [b]) => (a === primaryName.value ? -1 : b === primaryName.value ? 1 : 0));
  return rows.slice(0, 6);
});
// The same tree the tile shows under "Gebruikte gegevens": register values,
// and under each law that computed a value the values that law used in turn.
const lineage = computed(() => {
  if (!props.evaluation?.trace || !props.law) return [];
  return lineageFromTrace(props.evaluation.trace, props.law.id, evaluationParamsFor(personaParams(), asked.value));
});
const usedCount = computed(() => leafValues(lineage.value).length);

watch(
  () => props.open,
  async (open) => {
    if (!open) {
      sheet.value?.hide?.();
      return;
    }
    error.value = '';
    declared.value = false;
    objectionReason.value = '';
    justSubmitted.value = false;
    for (const k of Object.keys(answers)) delete answers[k];
    answered.value = 0;
    step.value = currentCase.value ? 'status' : missing.value.length ? 'gegevens' : 'controleren';
    await nextTick();
    sheet.value?.show?.();
  },
  { immediate: true },
);

// ---- step 1: the questions ----------------------------------------------------
function kindOf(a: Question) {
  if (Array.isArray(a.spec?.type_spec?.enum)) return 'enum';
  return inputKind(a.spec, a.claim?.newValue ?? null);
}
function enumOptions(a: Question) {
  const labels = a.spec?.type_spec?.enum_labels ?? {};
  return (a.spec?.type_spec?.enum ?? []).map((v) => ({ value: String(v), label: labels[v] ?? labels[String(v)] ?? String(v) }));
}
function placeholderFor(a: Question) {
  const k = kindOf(a);
  return k === 'amount' ? t('sheet.application.placeholder.amount') : k === 'date' ? t('sheet.application.placeholder.date') : k === 'number' ? t('sheet.application.placeholder.number') : '';
}
function labelFor(a: Question) {
  const k = kindOf(a);
  return k === 'amount' ? t('sheet.application.label.amount', { field: humanize(a.name) }) : humanize(a.name);
}
function setAnswer(a: Question, e: Event) {
  answers[a.name] = (e as CustomEvent<{ value?: unknown } | null | undefined>).detail?.value ?? (e.target as (EventTarget & { value?: unknown }) | null)?.value ?? '';
}
/** What an nldd field's event carries: the value in `detail`, or on the field itself. */
function inputValue(e: Event): string {
  return (e as CustomEvent<{ value?: string } | null | undefined>).detail?.value ?? (e.target as EventTarget & { value: string }).value;
}
function onDeclaredChange(e: Event) {
  declared.value = !!((e as CustomEvent<{ checked?: boolean } | null | undefined>).detail?.checked ?? (e.target as (EventTarget & { checked?: boolean }) | null)?.checked);
}
const canContinue = computed(() => !!question.value && parseAnswer(kindOf(question.value), answers[question.value.name]) !== undefined);

// One answer; the engine recomputes (the tile re-evaluates on the new claim)
// and `missing` tells whether it ran into another question.
function submitAnswer() {
  error.value = '';
  const a = question.value;
  if (!a) return;
  const value = parseAnswer(kindOf(a), answers[a.name]);
  if (value === undefined) {
    error.value = t('sheet.application.fill_in', { field: humanize(a.name).toLowerCase() });
    return;
  }
  // The question form only renders inside `v-if="law"`.
  const law = props.law!;
  demo.submitClaim({
    lawId: law.id,
    tileLawId: law.id,
    input: a.name,
    ...claimKeyFor(law, personaParams()),
    oldValue: null,
    newValue: value,
    reason: t('sheet.application.claim_reason'),
    selfDeclared: true,
  });
  answered.value += 1;
}
// When the recomputation has no further question, move on by itself.
watch([missing, step], ([m, st]) => {
  if (st === 'gegevens' && m.length === 0 && answered.value > 0) step.value = 'controleren';
});

/**
 * Het invoerveld krijgt de aandacht, bij elke volgende vraag opnieuw.
 *
 * De wet stelt haar vragen één voor één (just in time), en na een antwoord
 * staat de volgende vraag er met een leeg veld. Zonder dit moet je er elke keer
 * eerst heen klikken; met dit kun je een aanvraag al typend en enterend
 * afmaken, wat in een demo voor de zaal het verschil is tussen vertellen en
 * laten zien. Enter zit op het omhullende veld, zodat elk soort vraag het erft.
 *
 * Welk veld dat is, hangt van de vraag af (tekst, datum, keuzelijst). Ze
 * melden zich alle drie als invoerveld met `static isFormInput = true` — de
 * afspraak waarop `nldd-form-field` zelf ook zijn label laat wijzen — en
 * hebben een eigen `focus()` die het schaduw-DOM afhandelt. Dat is stabieler
 * dan hier een lijst met tagnamen bijhouden.
 */
const questionField = ref<HTMLElement | null>(null);
function isFormInput(el: Element) {
  return el instanceof HTMLInputElement || el instanceof HTMLSelectElement || (el.constructor as { isFormInput?: boolean } | undefined)?.isFormInput === true;
}
watch(
  [question, () => props.open],
  async ([q, open]) => {
    if (!q || !open) return;
    await nextTick();
    const control = [...(questionField.value?.querySelectorAll?.('*') ?? [])].find(isFormInput);
    // A form input (native or nldd) has `focus()`.
    (control as HTMLElement | undefined)?.focus?.();
  },
  { immediate: true },
);

// ---- step 2/3: check and submit --------------------------------------------------
// Een aanslag vraag je niet aan: via "Aanvullen" komt ook de accijns hier, en
// dan rekent de aanvraag mee maar dient niets in.
const applicable = computed(() => canBeApplied((props.law?.doc?.articles ?? []).map((a) => a.machine_readable?.execution?.produces).find(Boolean) ?? null));
const canSubmit = computed(() => applicable.value && props.evaluation?.ok && verdict.value === true && declared.value && missing.value.length === 0);
function submitApplication() {
  // The button is inside `v-if="law"` and disabled until `canSubmit`, which needs an evaluation.
  const c = demo.submitCase(props.law!, props.evaluation!, evaluationParamsFor(personaParams(), asked.value));
  if (c) {
    justSubmitted.value = true;
    step.value = 'status';
  }
}

// ---- status and objection --------------------------------------------------------
// `variant` decides the banner's colour AND its default icon; there is no
// `info` variant (neutral | accent | success | warning | critical), so the two
// statuses that used it rendered without an icon at all — an empty column to
// the left of the text. `accent` is the neutral-but-noticed one. The icons
// mirror the tile's status tag, so the same case looks the same in both.
const statusView = computed((): StatusView | null => {
  const c = currentCase.value;
  if (!c) return null;
  if (c.objection?.status === 'PENDING') return { variant: 'warning', icon: 'flag', text: t('sheet.application.status.objection'), supporting: t('sheet.application.status.objection.supporting') };
  // Uit de fase, niet uit het opgeslagen veld (zie lifecycle.ts).
  const status = statusOf(c);
  if (status === 'DECIDED') {
    if (c.objection) return c.approved ? { variant: 'success', icon: 'check-mark-circle', text: t('sheet.application.status.granted_after_objection'), supporting: caseReason(c) } : { variant: 'critical', icon: 'dismiss-circle', text: t('sheet.application.status.refused_objection_dismissed'), supporting: caseReason(c) };
    return c.approved ? { variant: 'success', icon: 'check-mark-circle', text: t('sheet.application.status.granted'), supporting: caseReason(c) } : { variant: 'critical', icon: 'dismiss-circle', text: t('sheet.application.status.refused'), supporting: caseReason(c) };
  }
  if (status === 'IN_REVIEW') return { variant: 'accent', icon: 'clock', text: t('sheet.application.status.in_review'), supporting: t('sheet.application.status.in_review.supporting') };
  return { variant: 'accent', icon: 'paper-plane', text: t('sheet.application.status.submitted'), supporting: t('sheet.application.status.submitted.supporting') };
});
/** Toegekend of afgewezen, gelezen van de gebeurtenis en niet van haar tekst. */
function decidedText(e: CaseEvent) {
  if (e.approved !== undefined) return e.approved ? t('citizen.event.granted') : t('citizen.event.refused');
  if (e.key) return e.key.includes('granted') ? t('citizen.event.granted') : t('citizen.event.refused');
  return String(e.text ?? '').includes('oegekend') ? t('citizen.event.granted') : t('citizen.event.refused');
}

/** Bezwaar gegrond of ongegrond, op dezelfde manier. */
function objectionText(e: CaseEvent) {
  if (e.upheld !== undefined) return e.upheld ? t('citizen.event.objection_upheld') : t('citizen.event.objection_dismissed');
  if (e.key) return e.key.includes('upheld') ? t('citizen.event.objection_upheld') : t('citizen.event.objection_dismissed');
  const text = String(e.text ?? '');
  const upheld = text.includes('gegrond:') && !text.includes('ongegrond');
  return upheld ? t('citizen.event.objection_upheld') : t('citizen.event.objection_dismissed');
}

const citizenEvents = computed(() =>
  (currentCase.value?.events ?? []).map((e) => ({
    at: e.at,
    // Op de gebeurtenis beslissen en niet op de woorden ervan. Dit las eerder
    // `e.text.startsWith('Toegekend')`, en die tak zou stil breken zodra de zin
    // in een andere taal staat. `approved` en `upheld` staan nu als veld op de
    // gebeurtenis; een zaak die al in localStorage stond draagt ze niet, dus
    // daar valt het terug op de opgeslagen tekst.
    text: e.type === 'SUBMITTED' ? t('citizen.event.submitted')
      : e.type === 'IN_REVIEW' ? t('citizen.event.in_review')
      : e.type === 'DECIDED' ? decidedText(e)
      : e.type === 'OBJECTION' ? t('citizen.event.objection')
      : e.type === 'OBJECTION_DECIDED' ? objectionText(e)
      : eventText(e),
  })),
);
// Bezwaar kan pas als de bezwaartermijn loopt, en die begint de dag ná de
// bekendmaking (Awb 6:8). Zolang het besluit nog niet is bekendgemaakt weet de
// burger van niets en is er niets om bezwaar tegen te maken.
const canObject = computed(() => objectionOpen(currentCase.value));
/** Wat de Awb aan dit besluit heeft toegevoegd: de termijn, de einddatum. */
const awb = computed(() => awbOutcomes(currentCase.value));

/**
 * De aanvraag die er ligt, tegen wat de wet nu zegt. Zelfde vergelijking als
 * op de tegel; hier staat de knop waarmee de burger er iets aan kan doen.
 */
const drift = computed(() => {
  void dataVersion.value;
  return props.law ? caseDrift(props.law, props.evaluation) : null;
});
const rows = computed(() => driftRows(drift.value, doc.value));
const driftText = computed(() => driftSentence(drift.value, doc.value));
// Both buttons are inside `v-else-if="currentCase"`.
function resubmit() {
  demo.resubmitCase(currentCase.value!.id, props.evaluation, evaluationParamsFor(personaParams(), asked.value));
}
function fileObjection() {
  demo.objectToCase(currentCase.value!.id, objectionReason.value.trim() || t('sheet.application.objection.default_reason'));
  objectionReason.value = '';
}
const claimedPrimary = computed(() => {
  const c = currentCase.value;
  if (!c) return null;
  const name = primaryName.value ?? Object.keys(c.claimedResult ?? {}).find((k) => typeof c.claimedResult[k] === 'number');
  return name ? { name, value: c.claimedResult?.[name] } : null;
});
// The corrections on this case, the citizen's own and the caseworker's: the
// same list the caseworker reads in the Zaaksysteem, so both sides see one thing.
const caseClaims = computed(() => {
  const c = currentCase.value;
  if (!c) return [];
  return demo.state.claims.filter((cl) => cl.caseId === c.id || (cl.bsn === (c.claimsBsn ?? c.bsn) && cl.tileLawId === c.lawId));
});
function claimSpec(cl: DemoClaim) {
  return fieldSpec(corpus.value?.lawById(cl.lawId)?.doc, cl.input);
}
function claimStatus(cl: DemoClaim) {
  if (cl.status === 'APPROVED') return { color: 'success', text: cl.claimant === 'BEHANDELAAR' ? t('sheet.application.claim.applied') : t('sheet.application.claim.approved') };
  if (cl.status === 'REJECTED') return { color: 'critical', text: t('sheet.application.claim.rejected') };
  return { color: 'neutral', text: t('sheet.application.claim.pending') };
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet ref="sheet" placement="right" width="560px" :accessible-label="t('sheet.application.label')" @close="emit('close')">
      <nldd-page v-if="law">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="law.name" :supporting-text="service" :dismiss-text="t('sheet.dismiss')" @dismiss="emit('close')"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="16" gap="16">
          <nldd-step-bar v-if="step !== 'status'" :current="step === 'gegevens' ? 1 : 2" :accessible-label="t('sheet.application.steps')">
            <nldd-step-bar-item :text="t('sheet.application.step.data')"></nldd-step-bar-item>
            <nldd-step-bar-item :text="t('sheet.application.step.check')"></nldd-step-bar-item>
          </nldd-step-bar>

          <!-- Step 1: the questions only the citizen can answer -->
          <template v-if="step === 'gegevens'">
            <nldd-rich-text spacing="tight">
              <p v-if="answered === 0">{{ t('sheet.application.intro.first') }}</p>
              <p v-else>{{ t('sheet.application.intro.next') }}</p>
            </nldd-rich-text>
            <template v-if="question">
              <!-- Enter op het veld is Verder, wat voor soort vraag het ook is:
                   een demo loopt zo van vraag naar vraag zonder de muis. Op het
                   omhullende veld, zodat elk invoertype het erft. -->
              <nldd-form-field ref="questionField" :label="labelFor(question)" @keydown.enter="canContinue && submitAnswer()">
                <nldd-dropdown v-if="kindOf(question) === 'enum'" width="full">
                  <select :value="answers[question.name] ?? ''" @change="setAnswer(question, $event)">
                    <option value="" disabled>{{ t('sheet.application.choose') }}</option>
                    <option v-for="o in enumOptions(question)" :key="o.value" :value="o.value">{{ o.label }}</option>
                  </select>
                </nldd-dropdown>
                <nldd-dropdown v-else-if="kindOf(question) === 'boolean'" width="full">
                  <select :value="answers[question.name] ?? ''" @change="setAnswer(question, $event)">
                    <option value="" disabled>{{ t('sheet.application.choose') }}</option>
                    <option value="true">{{ t('sheet.application.yes') }}</option>
                    <option value="false">{{ t('sheet.application.no') }}</option>
                  </select>
                </nldd-dropdown>
                <nldd-date-field v-else-if="kindOf(question) === 'date'" :value="answers[question.name] ?? ''" width="full" @change="setAnswer(question, $event)"></nldd-date-field>
                <!-- An unanswered amount is empty, not 0: nldd-number-field has no empty state (it starts at 0 and
                     an emptied field falls back to the last value), so the question is a text field with a numeric
                     keyboard; parseAnswer reads the Dutch notation. -->
                <nldd-text-field v-else-if="kindOf(question) === 'amount' || kindOf(question) === 'number'" :value="answers[question.name] ?? ''" width="full" :keyboard="kindOf(question) === 'amount' ? 'decimal' : 'numeric'" :placeholder="placeholderFor(question)" @input="setAnswer(question, $event)"></nldd-text-field>
                <nldd-text-field v-else :value="answers[question.name] ?? ''" width="full" :placeholder="placeholderFor(question)" @input="setAnswer(question, $event)"></nldd-text-field>
                <nldd-form-field-help-text v-if="question.spec?.description">{{ question.spec.description }}</nldd-form-field-help-text>
              </nldd-form-field>
              <nldd-banner v-if="error" variant="critical" :text="error"></nldd-banner>
              <nldd-form-actions>
                <nldd-button appearance="primary" :text="t('sheet.application.continue')" :disabled="!canContinue || undefined" @click="submitAnswer"></nldd-button>
              </nldd-form-actions>
            </template>
            <template v-else>
              <nldd-activity-indicator timing="instant" size="24"></nldd-activity-indicator>
              <nldd-rich-text spacing="tight"><p>{{ t('sheet.application.recomputing') }}</p></nldd-rich-text>
            </template>
            <nldd-list v-if="asked.some((a) => a.claim)" appearance="simple" :accessible-label="t('sheet.application.declared.label')">
              <nldd-list-item v-for="a in asked.filter((x) => x.claim)" :key="a.name" size="sm">
                <nldd-icon-cell icon="checked" size="16" color="success"></nldd-icon-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-text-cell size="sm" :text="humanize(a.name)" :supporting-text="t('sheet.application.declared.by_you')"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="formatValue(a.claim?.newValue, a.spec)"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
          </template>

          <!-- Step 2: check the outcome and the data, declare, submit -->
          <template v-else-if="step === 'controleren'">
            <template v-if="!evaluation?.ok">
              <nldd-activity-indicator timing="instant" size="24"></nldd-activity-indicator>
              <nldd-rich-text spacing="tight"><p>{{ t('sheet.application.computing') }}</p></nldd-rich-text>
            </template>
            <template v-else>
              <nldd-banner v-if="verdict === 'unknown'" variant="accent" :text="t('sheet.application.unknown.title')" :supporting-text="t('sheet.application.unknown.supporting', { missing: verdictMissing })"></nldd-banner>
              <nldd-list v-else appearance="box-tinted" :accessible-label="t('sheet.application.outcome')">
                <nldd-list-item size="md">
                  <nldd-icon-cell :icon="requirementsMet ? 'check-mark-circle' : 'dismiss-circle'" :color="requirementsMet ? 'success' : 'critical'"></nldd-icon-cell>
                  <nldd-spacer-cell size="12"></nldd-spacer-cell>
                  <nldd-title-cell size="4" :overline="requirementsMet ? t('sheet.application.meets') : t('sheet.application.meets_not')" :text="requirementsMet && outcomeRows[0] ? formatValue(outcomeRows[0][1], fieldSpec(doc, outcomeRows[0][0])) : requirementsMet ? t('sheet.application.yes') : t('sheet.application.pointless')" :supporting-text="requirementsMet && outcomeRows[0] ? humanize(outcomeRows[0][0]) : ''"></nldd-title-cell>
                </nldd-list-item>
              </nldd-list>
              <nldd-list v-if="outcomeRows.length > 1" appearance="simple" :accessible-label="t('sheet.application.outcome.other')">
                <nldd-list-item v-for="[name, value] in outcomeRows.slice(1)" :key="name" size="sm">
                  <nldd-text-cell size="sm" color="secondary" :text="humanize(name)"></nldd-text-cell>
                  <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="formatValue(value, fieldSpec(doc, name))"></nldd-text-cell>
                </nldd-list-item>
              </nldd-list>

              <nldd-title size="5">
                <h3>{{ t('sheet.application.basis.title') }}</h3>
                <span slot="supporting-text">{{ t('sheet.application.basis.subtitle', { n: usedCount }) }}</span>
              </nldd-title>
              <nldd-list type="tree" appearance="box-tinted" :accessible-label="t('sheet.application.basis.label')">
                <DataLineage :nodes="lineage" @edit="emit('edit-value', { node: $event, law })" />
              </nldd-list>
              <nldd-rich-text spacing="tight"><p><small>{{ t('sheet.application.basis.hint') }}</small></p></nldd-rich-text>

              <template v-if="requirementsMet && applicable">
                <nldd-checkbox-field :label="t('sheet.application.declaration')" :checked="declared || undefined" @change="onDeclaredChange"></nldd-checkbox-field>
                <nldd-form-actions>
                  <nldd-button appearance="primary" start-icon="paper-plane" :text="t('sheet.application.submit')" :disabled="!canSubmit || undefined" @click="submitApplication"></nldd-button>
                </nldd-form-actions>
              </template>
              <nldd-banner v-else-if="verdict === false" variant="warning" :text="t('sheet.application.refuse.title')" :supporting-text="t('sheet.application.refuse.supporting')"></nldd-banner>
            </template>
          </template>

          <!-- Status of the application -->
          <template v-else-if="currentCase">
            <nldd-banner :variant="statusView?.variant" :icon="statusView?.icon" :text="statusView?.text" :supporting-text="statusView?.supporting"></nldd-banner>
            <nldd-rich-text v-if="justSubmitted" spacing="tight"><p>{{ t('sheet.application.submitted', { service }) }}</p></nldd-rich-text>

            <!-- Wat er ligt klopt niet meer met wat de wet nu zegt. De demo
                 rekent niets opnieuw af achter de rug van de burger om: het
                 besluit blijft staan en hij krijgt te zien wat er veranderd is,
                 met de weg terug ernaast. Aanvraag per aanvraag, want elke
                 aanvraag is een eigen besluit. -->
            <template v-if="drift && !justSubmitted">
              <nldd-banner variant="warning" :text="t('sheet.application.drift.title')" :supporting-text="driftText"></nldd-banner>
              <nldd-list appearance="box-tinted" :accessible-label="t('sheet.application.drift.label')">
                <nldd-list-item v-for="row in rows" :key="row.name" size="sm">
                  <nldd-text-cell size="sm" color="secondary" min-width="50%" :text="humanize(row.name)"></nldd-text-cell>
                  <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="`${row.was} → ${row.now}`"></nldd-text-cell>
                </nldd-list-item>
              </nldd-list>
              <nldd-form-actions>
                <nldd-button appearance="primary" start-icon="paper-plane" :text="t('sheet.application.drift.amend')" @click="resubmit"></nldd-button>
              </nldd-form-actions>
            </template>
            <nldd-list v-if="claimedPrimary" appearance="box-tinted" :accessible-label="t('sheet.application.claimed.label')">
              <nldd-list-item size="sm">
                <nldd-text-cell size="sm" color="secondary" :text="t('sheet.application.claimed.row', { field: humanize(claimedPrimary.name) })"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="formatValue(claimedPrimary.value, fieldSpec(doc, claimedPrimary.name))"></nldd-text-cell>
              </nldd-list-item>
              <nldd-list-item size="sm">
                <nldd-text-cell size="sm" color="secondary" :text="t('sheet.application.submitted_on')"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="formatDateTime(currentCase.submittedAt)"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
            <template v-if="caseClaims.length">
              <nldd-title size="5"><h3>{{ t('sheet.application.corrections.title') }}</h3></nldd-title>
              <nldd-list appearance="box-tinted" :accessible-label="t('sheet.application.corrections.title')">
                <nldd-list-item v-for="cl in caseClaims" :key="cl.id" size="sm">
                  <nldd-text-cell size="sm" :text="`${humanize(cl.input)}: ${formatValue(cl.oldValue, claimSpec(cl))} → **${formatValue(cl.newValue, claimSpec(cl))}**`">
                    <span slot="supporting-text">
                      {{ cl.claimant === 'BEHANDELAAR' ? t('sheet.application.corrections.by_officer') : t('sheet.application.corrections.by_you') }} · {{ cl.reason }}
                      <template v-if="cl.hardship?.clause"><br />{{ t('sheet.application.hardship.invoked', { clause: cl.hardship.clause }) }}</template>
                      <template v-if="cl.evidence"><br />{{ t('sheet.application.evidence', { name: cl.evidence.name }) }}</template>
                    </span>
                  </nldd-text-cell>
                  <nldd-cell v-if="cl.hardship?.clause"><nldd-tag size="sm" color="warning" :text="t('sheet.application.hardship.tag')"></nldd-tag></nldd-cell>
                  <!-- Twee losse labels naast elkaar: zonder tussenruimte lezen ze als een. -->
                  <nldd-spacer-cell v-if="cl.hardship?.clause" size="8"></nldd-spacer-cell>
                  <nldd-cell><nldd-tag size="sm" :color="claimStatus(cl).color" :text="claimStatus(cl).text"></nldd-tag></nldd-cell>
                </nldd-list-item>
              </nldd-list>
            </template>
            <nldd-title size="5"><h3>{{ t('sheet.application.history.title') }}</h3></nldd-title>
            <nldd-list appearance="simple" :accessible-label="t('sheet.application.history.label')">
              <nldd-list-item v-for="(e, i) in citizenEvents" :key="i" size="sm">
                <nldd-text-cell size="sm" :text="e.text" :supporting-text="formatDateTime(e.at)"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
            <!-- De termijn komt uit de wet en niet uit dit scherm: artikel 6:7
                 Awb geeft de zes weken, artikel 6:8 rekent de einddatum uit
                 vanaf de bekendmaking, en een bijzondere wet die daarvan
                 afwijkt is er al in verwerkt. Daarom staat hier een datum en
                 geen vaste tekst. -->
            <nldd-list v-if="awb.bezwaartermijnEinde" appearance="box-tinted" :accessible-label="t('sheet.application.objection.deadline.label')">
              <nldd-list-item size="sm">
                <nldd-text-cell size="sm" color="secondary" :text="t('sheet.application.objection.until')"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="formatValue(awb.bezwaartermijnEinde, null)"></nldd-text-cell>
              </nldd-list-item>
              <nldd-list-item v-if="awb.bezwaartermijnWeken" size="sm">
                <nldd-text-cell size="sm" color="secondary" :text="t('sheet.application.objection.term')"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="t('sheet.application.objection.weeks', { n: awb.bezwaartermijnWeken })"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
            <template v-if="canObject">
              <nldd-form-field :label="t('sheet.application.objection.field')" optional>
                <nldd-multi-line-text-field :value="objectionReason" rows="3" :placeholder="t('sheet.application.objection.placeholder')" @input="objectionReason = inputValue($event)"></nldd-multi-line-text-field>
              </nldd-form-field>
              <nldd-form-actions>
                <nldd-button appearance="secondary" start-icon="flag" :text="t('sheet.application.objection.submit')" @click="fileObjection"></nldd-button>
              </nldd-form-actions>
            </template>
            <!-- Besloten, maar nog niet de deur uit. Eerlijk benoemen dat de
                 termijn nog niet loopt is beter dan een knop die er wel staat
                 maar niets betekent. -->
            <nldd-banner
              v-else-if="currentCase && !currentCase.publishedAt && currentCase.decidedAt"
              variant="accent"
              :text="t('sheet.application.unannounced.title')"
              :supporting-text="t('sheet.application.unannounced.supporting')"
            ></nldd-banner>
          </template>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
