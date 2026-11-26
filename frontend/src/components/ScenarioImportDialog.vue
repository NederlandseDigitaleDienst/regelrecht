<script setup lang="ts">
/**
 * Import feature files into the open regelwerk's scenario folder, in a
 * traject. The caller calls `pick()`; the chosen files are checked in the
 * browser (`scenario/scenarioImport.ts`) and listed, one line each, before
 * anything is written. Only the files that fit are saved.
 *
 * There is no design-system file picker, so the picker is a hidden
 * `<input type="file">`, as in `composables/useDocumentUpload.ts`.
 */
import { computed, nextTick, ref } from 'vue';
import type { NLDDModalDialog } from '@nldd/design-system/modal-dialog';
import { useCorpus } from '../corpus';
import { checkImports, type ImportVerdict, type ScenarioImport } from '../scenario/scenarioImport';
import { writeNewScenarioFile } from '../scenario/openScenario';

const props = defineProps<{
  trajectRef: string | null;
  /** The open regelwerk; the files are imported into its scenario folder. */
  lawId: string | null;
  /** Filenames already in that folder. */
  existing: readonly string[];
}>();

const emit = defineEmits<{
  /**
   * Files were saved. Says where: the editor may have moved to another
   * regelwerk while the dialog was open.
   */
  imported: [result: ScenarioImport];
}>();

const fileInput = ref<HTMLInputElement | null>(null);
const dialogEl = ref<NLDDModalDialog | null>(null);
const open = ref(false);
const phase = ref<'checking' | 'review' | 'saving' | 'done'>('checking');
const verdicts = ref<ImportVerdict[]>([]);
const checkError = ref<string | null>(null);
// The regelwerk the open selection was checked for; a save goes there, even
// when the editor moved on while the dialog was open.
let target: { trajectRef: string; lawId: string } | null = null;

const importable = computed(() => verdicts.value.filter((v) => v.status !== 'error'));

const BADGE = {
  ok: { color: 'success', icon: 'success', label: 'Past' },
  warning: { color: 'warning', icon: 'warning', label: 'Past, met een kanttekening' },
  error: { color: 'critical', icon: 'error', label: 'Wordt niet geïmporteerd' },
} as const;

const supportingText = computed(() => {
  if (checkError.value) return checkError.value;
  if (phase.value === 'checking') return 'De bestanden worden gecontroleerd.';
  if (phase.value === 'done') return 'Niet alles kon worden opgeslagen.';
  const n = importable.value.length;
  if (n === 0) return 'Geen van de bestanden past bij dit regelwerk.';
  return n === 1
    ? '1 bestand wordt opgeslagen op de traject-branch.'
    : `${n} bestanden worden opgeslagen op de traject-branch.`;
});

/** Open the file picker. */
function pick(): void {
  if (!props.trajectRef || !props.lawId) return;
  fileInput.value?.click();
}
defineExpose({ pick });

async function onFileChange(e: Event): Promise<void> {
  const input = e.target as HTMLInputElement;
  const files = [...(input.files ?? [])];
  // Reset, so picking the same files again fires `change` again.
  input.value = '';
  if (!files.length || !props.trajectRef || !props.lawId) return;
  target = { trajectRef: props.trajectRef, lawId: props.lawId };
  const mine = target;
  phase.value = 'checking';
  verdicts.value = [];
  checkError.value = null;
  await nextTick();
  show();
  try {
    const candidates = await Promise.all(
      files.map(async (f) => ({ filename: f.name, text: await f.text(), size: f.size })),
    );
    const result = await checkImports(
      candidates,
      { lawId: mine.lawId, existing: props.existing },
      useCorpus().forTraject(mine.trajectRef),
    );
    if (target !== mine) return;
    verdicts.value = result;
  } catch (err) {
    if (target !== mine) return;
    checkError.value = `Het corpus van dit traject kon niet worden gelezen: ${(err as Error).message}`;
  }
  phase.value = 'review';
}

async function onConfirm(): Promise<void> {
  const mine = target;
  if (!mine || phase.value !== 'review' || !importable.value.length) return;
  phase.value = 'saving';
  const saved: string[] = [];
  const failed = new Map<string, string>();
  // One at a time: each save is a commit on the traject branch.
  for (const v of importable.value) {
    try {
      await writeNewScenarioFile({ ...mine, filename: v.filename }, v.text);
      saved.push(v.filename);
    } catch (err) {
      failed.set(v.filename, (err as Error).message);
    }
  }
  if (saved.length) emit('imported', { ...mine, filenames: saved });
  if (!failed.size) {
    close();
    return;
  }
  // Escape closes the dialog even while it saves; the failures must still be seen.
  if (!open.value) show();
  verdicts.value = verdicts.value.map((v) => {
    const message = failed.get(v.filename);
    if (message) return { ...v, status: 'error', reason: `Opslaan mislukt: ${message}` };
    if (saved.includes(v.filename)) return { ...v, reason: 'Opgeslagen' };
    return v;
  });
  phase.value = 'done';
}

function show(): void {
  open.value = true;
  dialogEl.value?.show?.();
}

function close(): void {
  target = null;
  verdicts.value = [];
  open.value = false;
  dialogEl.value?.hide?.();
}

// Escape or a click beside the dialog. A running save finishes regardless,
// and reopens the dialog when a file failed.
function onClose(): void {
  open.value = false;
  if (phase.value !== 'saving') target = null;
}
</script>

<template>
  <input ref="fileInput" type="file" accept=".feature" multiple hidden data-testid="scenario-import-input" @change="onFileChange" />
  <Teleport to="body">
    <nldd-modal-dialog
      ref="dialogEl"
      icon="import"
      text="Scenario's importeren"
      :supporting-text="supportingText"
      data-testid="scenario-import-dialog"
      @close="onClose"
    >
      <nldd-activity-indicator v-if="phase === 'checking'" show-text text="Controleren"></nldd-activity-indicator>
      <nldd-list v-else-if="verdicts.length" appearance="box-tinted" accessible-label="Gekozen bestanden">
        <nldd-list-item
          v-for="(v, i) in verdicts"
          :key="i"
          size="md"
          :data-testid="`scenario-import-${v.filename}`"
          :data-status="v.status"
        >
          <nldd-cell width="fit-content">
            <nldd-badge
              size="sm"
              :color="BADGE[v.status].color"
              :icon="BADGE[v.status].icon"
              :accessible-label="BADGE[v.status].label"
            ></nldd-badge>
          </nldd-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell :text="v.filename" :supporting-text="v.reason"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-button
        v-if="phase === 'review' || phase === 'saving'"
        slot="actions"
        appearance="primary"
        :text="importable.length > 1 ? `Importeer ${importable.length}` : 'Importeer'"
        :disabled="!importable.length || undefined"
        :loading="phase === 'saving' || undefined"
        loading-text="Opslaan"
        data-testid="scenario-import-submit"
        @click="onConfirm"
      ></nldd-button>
      <nldd-button
        slot="actions"
        appearance="secondary"
        :text="phase === 'done' ? 'Sluit' : 'Annuleer'"
        :disabled="phase === 'saving' || undefined"
        data-testid="scenario-import-cancel"
        @click="close"
      ></nldd-button>
    </nldd-modal-dialog>
  </Teleport>
</template>
