<template>
  <nldd-sheet
    ref="sheetEl"
    placement="right"
    width="760px"
    accessible-label="Wet als YAML bewerken"
    @close="emit('close')"
  >
    <nldd-page>
      <div class="ye-content">
        <nldd-title heading-level="2" size="display-3">
          <span slot="overline">Structuur bewerken</span>
          <span>{{ docName }}</span>
        </nldd-title>
        <p class="ye-hint">
          Bewerk de wet rechtstreeks als YAML; de wijziging komt als bewerking in de werkversie. Bij een fout blijft de laatst
          werkende versie geladen en verschijnt hieronder een melding.
        </p>

        <div class="ye-actions">
          <nldd-button :text="`Toepassen in ${werkversieLabel}`" appearance="primary" start-icon="checked" @click="apply"></nldd-button>
          <nldd-button text="Terugzetten" appearance="neutral-transparent" start-icon="undo" @click="reset"></nldd-button>
        </div>

        <nldd-banner v-if="error" variant="critical">
          Kon de YAML niet toepassen: {{ error }}
        </nldd-banner>
        <nldd-banner v-else-if="applied" variant="success">
          Wijziging toegepast en de wet opnieuw geladen.
        </nldd-banner>

        <nldd-code-editor
          ref="editorEl"
          :value="draft"
          rows="24"
          wrap
          @input="onInput"
        ></nldd-code-editor>
      </div>
    </nldd-page>
  </nldd-sheet>
</template>

<script setup>
import { ref, computed, watch, nextTick } from 'vue';
import { useLawStore } from '../../engine/lawStore.js';

const props = defineProps({
  open: { type: Boolean, default: false },
  lawPath: { type: String, default: null },
});
const emit = defineEmits(['close']);

const { docYaml, applyRawYaml, werkversieLabel } = useLawStore();

const sheetEl = ref(null);
const editorEl = ref(null);
const draft = ref('');
const error = ref(null);
const applied = ref(false);

const doc = computed(() => (props.lawPath ? docYaml(props.lawPath) : null));
const docName = computed(() => doc.value?.entry?.name ?? 'Document');

function loadDraft() {
  draft.value = doc.value?.currentYaml ?? '';
  error.value = null;
  applied.value = false;
}

// nldd-code-editor synct een via het framework gezette `value`-property niet
// altijd door naar zijn interne textarea (die leest de waarde bij connect).
// Zet hem daarom expliciet zodra de sheet zichtbaar is.
function syncEditor() {
  if (editorEl.value) editorEl.value.value = draft.value;
}

function onInput(event) {
  draft.value = event.detail?.value ?? event.target?.value ?? draft.value;
  applied.value = false;
}

function apply() {
  error.value = null;
  applied.value = false;
  try {
    applyRawYaml(props.lawPath, draft.value);
    applied.value = true;
  } catch (e) {
    error.value = String(e?.message ?? e);
  }
}

function reset() {
  loadDraft();
  syncEditor();
}

watch(
  () => props.open,
  async (open) => {
    if (!open) {
      sheetEl.value?.hide();
      return;
    }
    loadDraft();
    await nextTick();
    sheetEl.value?.show();
    syncEditor();
  },
  { immediate: true },
);
</script>

<style scoped>
.ye-content { padding: var(--primitives-space-24); display: flex; flex-direction: column; gap: var(--primitives-space-16); }
.ye-hint { margin: 0; font-size: 0.9em; color: var(--semantics-content-secondary-color); }
.ye-actions { display: flex; gap: var(--primitives-space-8); }
.ye-content nldd-multi-line-text-field { display: block; max-height: 62vh; overflow: auto; }
</style>
