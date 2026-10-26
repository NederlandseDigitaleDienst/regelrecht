<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import { type ChangeAnswers, type ChangeClaim, CHANGE_TYPES, claimsFromAnswers, eventLabel, eventUnsupported, fieldLabel, groupLabel, typeDescription, typeLabel } from '../data/changeWizard';
import { formatValue } from '../data/format';
import { useI18n } from '../i18n/index.js';
import { useDemo } from '../store/demoStore';

// Wijziging doorgeven: iemand meldt een verandering in zijn leven (verhuisd,
// ander inkomen, uit elkaar) in plaats van per regeling een waarde te
// corrigeren. Elke melding wordt vertaald naar correcties op de wet die het
// gegeven bezit (zie data/changeWizard.ts), en daarna rekent elke regeling die
// ervan afhangt opnieuw.
//
// Drie stappen: wat wil je doorgeven, wat verandert er, en een bevestiging die
// laat zien wat er precies ingaat en naar welke wet.

const props = withDefaults(defineProps<{
  open?: boolean;
}>(), {
  open: false,
});
const emit = defineEmits<{
  close: [];
  submitted: [payload: { type: string; claims: ChangeClaim[] }];
}>();
const { t } = useI18n();
const { corpus, submitClaim, subjectBsn, features } = useDemo();

const sheet = ref<NLDDSheet | null>(null);
const step = ref(0);
const typeId = ref<string | null>(null);
const answers = ref<ChangeAnswers>({});
const reason = ref('');
const error = ref('');
/** Wat er is ingediend, voor de slotstap. */
const submitted = ref<{ count: number; label: string; law: string | null } | null>(null);

const type = computed(() => CHANGE_TYPES.find((t) => t.id === typeId.value) ?? null);
const claims = computed(() => claimsFromAnswers(type.value, answers.value));
/** De gekozen gebeurtenis bij 'huishouden', ook als die nog niet kan. */
const chosenEvent = computed(() => (type.value?.events ?? []).find((e) => e.value === answers.value.event) ?? null);
const lawName = computed(() => (type.value ? corpus.value?.lawById(type.value.law)?.name ?? type.value.law : null));

function reset() {
  step.value = 0;
  typeId.value = null;
  answers.value = {};
  reason.value = '';
  error.value = '';
  submitted.value = null;
}

watch(
  () => props.open,
  async (open) => {
    if (open) {
      reset();
      await nextTick();
      sheet.value?.show?.();
    } else {
      sheet.value?.hide?.();
    }
  },
);

function chooseType(id: string) {
  typeId.value = id;
  answers.value = {};
  error.value = '';
  step.value = 1;
}

function setAnswer(name: string, value: unknown) {
  answers.value = { ...answers.value, [name]: value };
  error.value = '';
}

/** What an nldd field's input event carries: the value in `detail`, or on the field itself. */
function inputValue(event: Event): string {
  return (event as CustomEvent<{ value?: string } | null | undefined>).detail?.value ?? (event.target as EventTarget & { value: string }).value;
}

function back() {
  error.value = '';
  step.value = Math.max(0, step.value - 1);
}

function toConfirm() {
  if (chosenEvent.value?.unsupportedKey) {
    error.value = eventUnsupported(chosenEvent.value);
    return;
  }
  if (!claims.value.length) {
    error.value = type.value?.events ? t('sheet.change.error.event') : t('sheet.change.error.empty');
    return;
  }
  error.value = '';
  step.value = 2;
}

/**
 * De melding indienen: elke vertaalde waarde wordt een correctie op haar wet,
 * met dezelfde weg als een losse correctie op het portaal — een behandelaar
 * beoordeelt haar, tenzij het profiel automatisch goedkeurt.
 */
function submit() {
  // Submit is only reachable from step 2, which needs a chosen type.
  const chosen = type.value!;
  const bsn = subjectBsn();
  const list = claims.value;
  const label = typeLabel(type.value);
  for (const c of list) {
    submitClaim({
      lawId: c.law,
      tileLawId: null,
      input: c.input,
      keyField: chosen.keyField,
      keyValue: bsn,
      oldValue: null,
      newValue: c.value,
      reason: reason.value || t('sheet.change.reason', { label: label.toLowerCase() }),
    });
  }
  submitted.value = { count: list.length, label, law: lawName.value };
  emit('submitted', { type: chosen.id, claims: list });
  step.value = 3;
}

/** Hoe een waarde in de bevestiging leest: een bedrag als bedrag, een adres als regel. */
function show(claim: ChangeClaim) {
  if (claim.value === null) return t('sheet.change.not_applicable');
  if (typeof claim.value === 'object') {
    return Object.entries(claim.value)
      .filter(([k]) => k !== 'type')
      .map(([, v]) => v)
      .join(' ');
  }
  if (typeof claim.value === 'number' && claim.value >= 1000) {
    return formatValue(claim.value, { type: 'amount', type_spec: { unit: 'eurocent' } });
  }
  return String(claim.value);
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet ref="sheet" placement="right" :accessible-label="t('sheet.change.label')" @close="emit('close')">
      <nldd-page>
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar
            :text="t('sheet.change.label')"
            :supporting-text="type ? typeLabel(type) : t('sheet.change.subtitle')"
            :dismiss-text="t('sheet.dismiss')"
            @dismiss="emit('close')"
          ></nldd-top-title-bar>
        </nldd-container>

        <nldd-container padding="16" gap="16">
          <!-- Stap 1: wat wil je doorgeven -->
          <template v-if="step === 0">
            <nldd-rich-text spacing="tight">
              <p>{{ t('sheet.change.intro') }}</p>
            </nldd-rich-text>
            <nldd-list appearance="box" :accessible-label="t('sheet.change.kind.label')">
              <nldd-list-item v-for="ct in CHANGE_TYPES" :key="ct.id" size="md" button @click="chooseType(ct.id)">
                <nldd-icon-cell :icon="ct.icon" size="20" color="accent"></nldd-icon-cell>
                <nldd-spacer-cell size="12"></nldd-spacer-cell>
                <nldd-text-cell :text="typeLabel(ct)" :supporting-text="typeDescription(ct)"></nldd-text-cell>
                <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
              </nldd-list-item>
            </nldd-list>
          </template>

          <!-- Stap 2: wat verandert er -->
          <template v-else-if="step === 1">
            <!-- Een gebeurtenis (huishouden): kiezen, geen invullen. -->
            <template v-if="type?.events">
              <nldd-list appearance="box" :accessible-label="t('sheet.change.what.label')">
                <nldd-list-item
                  v-for="e in type.events"
                  :key="e.value"
                  size="md"
                  button
                  :selected="answers.event === e.value || undefined"
                  @click="setAnswer('event', e.value)"
                >
                  <nldd-icon-cell :icon="e.icon" size="20" :color="e.unsupportedKey ? 'secondary' : 'accent'"></nldd-icon-cell>
                  <nldd-spacer-cell size="12"></nldd-spacer-cell>
                  <nldd-text-cell :text="eventLabel(e)" :supporting-text="e.unsupportedKey ? t('sheet.change.unsupported') : undefined"></nldd-text-cell>
                  <nldd-icon-cell v-if="answers.event === e.value && !e.unsupportedKey" icon="checked" size="16" color="success"></nldd-icon-cell>
                </nldd-list-item>
              </nldd-list>
            </template>

            <!-- Waarden: alleen wat verandert hoeft ingevuld. -->
            <template v-else>
              <nldd-rich-text spacing="tight">
                <p>{{ t('sheet.change.values.intro') }}</p>
              </nldd-rich-text>
              <template v-for="group in type?.groups ?? []" :key="group.labelKey">
                <nldd-title size="4"><h2>{{ groupLabel(group) }}</h2></nldd-title>
                <nldd-form-field v-for="f in group.fields" :key="f.name" :label="fieldLabel(f)" optional>
                  <nldd-text-field
                    :value="answers[f.name] ?? ''"
                    width="full"
                    :keyboard="f.kind === 'text' ? undefined : 'numeric'"
                    :prefix="f.scale ? '€' : undefined"
                    @input="setAnswer(f.name, inputValue($event))"
                  ></nldd-text-field>
                </nldd-form-field>
              </template>
            </template>

            <nldd-banner v-if="error" variant="critical" :text="error"></nldd-banner>
            <nldd-form-actions>
              <nldd-button-group orientation="horizontal">
                <nldd-button appearance="primary" :text="t('sheet.change.continue')" @click="toConfirm"></nldd-button>
                <nldd-button appearance="secondary" :text="t('sheet.change.back')" @click="back"></nldd-button>
              </nldd-button-group>
            </nldd-form-actions>
          </template>

          <!-- Stap 3: bevestigen. Wat er ingaat en naar welke wet, voordat het ingaat. -->
          <template v-else-if="step === 2">
            <!-- De wetnaam staat vet in de zin. `t()` levert tekst en geen
                 opmaak, dus de zin bestaat uit twee sleutels met de naam
                 ertussen; in beide talen staat de naam op dezelfde plek. -->
            <nldd-rich-text spacing="tight">
              <p>{{ t('sheet.change.confirm.before') }}<strong>{{ lawName }}</strong>{{ t('sheet.change.confirm.after') }}</p>
            </nldd-rich-text>
            <nldd-list appearance="box-tinted" :accessible-label="t('sheet.change.confirm.label')">
              <nldd-list-item v-for="c in claims" :key="c.input" size="md">
                <nldd-text-cell :text="c.label" :supporting-text="c.input"></nldd-text-cell>
                <nldd-text-cell width="fit-content" horizontal-alignment="right" :text="show(c)"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
            <nldd-form-field :label="t('sheet.change.note')" optional>
              <nldd-multi-line-text-field
                :value="reason"
                rows="3"
                :placeholder="t('sheet.change.note.placeholder')"
                @input="reason = inputValue($event)"
              ></nldd-multi-line-text-field>
              <nldd-form-field-help-text>
                {{ features.AUTO_APPROVE_CLAIMS ? t('sheet.change.note.help.immediate') : t('sheet.change.note.help.pending') }}
              </nldd-form-field-help-text>
            </nldd-form-field>
            <nldd-form-actions>
              <nldd-button-group orientation="horizontal">
                <nldd-button appearance="primary" :text="t('sheet.change.submit')" @click="submit"></nldd-button>
                <nldd-button appearance="secondary" :text="t('sheet.change.back')" @click="back"></nldd-button>
              </nldd-button-group>
            </nldd-form-actions>
          </template>

          <!-- Stap 4: wat er nu gebeurt. -->
          <template v-else>
            <nldd-inline-dialog
              icon="checked"
              icon-color="success"
              :text="t('sheet.change.done.title')"
              :supporting-text="t.plural(submitted?.count ?? 0, 'sheet.change.done', { law: submitted?.law })"
            ></nldd-inline-dialog>
            <nldd-form-actions>
              <nldd-button-group orientation="horizontal">
                <nldd-button appearance="primary" :text="t('sheet.change.done.to_portal')" @click="emit('close')"></nldd-button>
                <nldd-button appearance="secondary" :text="t('sheet.change.done.another')" @click="reset"></nldd-button>
              </nldd-button-group>
            </nldd-form-actions>
          </template>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
