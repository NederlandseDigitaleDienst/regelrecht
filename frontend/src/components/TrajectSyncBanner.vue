<script setup lang="ts">
// Trajectloader-melding. De lus in useTrajectAutoSync (gestart door AppShell)
// houdt het traject bij met zijn werkbranch; deze component toont wat die lus
// niet zelf kan afhandelen: een echt conflict, wijzigingen van het traject die
// nog naar de werkbranch moeten, of wijzigingen die nog binnenkomen terwijl de
// lus wacht. Stil als er niets te melden is, en op het centrale corpus.
//
// Notificaties plaatsen zichzelf in de gedeelde regio boven de panes, dus deze
// component zegt niets over layout. `duration="0"` op de statusmelding: die
// beschrijft een toestand, geen gebeurtenis, en mag niet wegtellen met de
// knoppen erin.
import { computed, ref } from 'vue';
import { canPropose, canSync, needsSyncPrompt, syncDetail, syncSummary, type BaseStatus } from '../lib/trajectSync';
import { useTrajectAutoSync } from '../composables/useTrajectAutoSync';

const { status, notice, error, busy, syncNow, propose, dismissNotice, dismissError } = useTrajectAutoSync();

// A notification always has a close button and leaves removal to its
// consumer. Closing the status hides it until the status itself changes,
// so a new commit on either branch brings it back.
const statusKey = (s: BaseStatus): string => JSON.stringify([s.base_files, s.traject_files]);
const dismissedKey = ref<string | null>(null);
const prompt = computed(() => {
  const s = status.value;
  if (!needsSyncPrompt(s) || !s) return null;
  return statusKey(s) === dismissedKey.value ? null : s;
});
function dismissPrompt(): void {
  if (status.value) dismissedKey.value = statusKey(status.value);
}
</script>

<template>
  <nldd-notification
    v-if="prompt"
    variant="warning"
    duration="0"
    :text="syncSummary(prompt)"
    :supporting-text="syncDetail(prompt) || undefined"
    data-testid="traject-sync-notice"
    @dismiss="dismissPrompt"
  >
    <nldd-button
      v-if="canSync(prompt)"
      slot="actions"
      appearance="primary"
      text="Bijwerken"
      :loading="busy === 'sync' || undefined"
      :disabled="busy || undefined"
      @click="syncNow"
    ></nldd-button>
    <nldd-button
      v-if="canPropose(prompt)"
      slot="actions"
      appearance="secondary"
      text="Voorstel naar werkbranch"
      :loading="busy === 'propose' || undefined"
      :disabled="busy || undefined"
      @click="propose"
    ></nldd-button>
  </nldd-notification>
  <nldd-notification
    v-if="error"
    variant="critical"
    duration="0"
    text="Bijwerken met de werkbranch lukte niet"
    :supporting-text="error"
    @dismiss="dismissError"
  ></nldd-notification>
  <nldd-notification
    v-if="notice"
    :text="notice.text"
    @dismiss="dismissNotice"
  >
    <nldd-button
      v-if="notice.url"
      slot="actions"
      appearance="secondary"
      text="Openen op GitHub"
      :href="notice.url"
      target="_blank"
      rel="noopener noreferrer"
    ></nldd-button>
  </nldd-notification>
</template>
