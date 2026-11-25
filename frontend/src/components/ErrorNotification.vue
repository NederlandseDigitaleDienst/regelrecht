<script setup lang="ts">
// The one notification for the errors the editor reports (errors store). One
// error shows as itself; several show as a count, with the list in the
// details sheet, so a run of failures does not stack up on screen. Critical,
// so it stays until dismissed. It steps aside while the details sheet is
// open: a notification moves into the open overlay, where it would cover the
// sheet's title bar and its way out.
import { computed } from 'vue';
import { storeToRefs } from 'pinia';
import { useErrorsStore } from '../stores/errors';

const store = useErrorsStore();
const { visible, detailsOpen } = storeToRefs(store);

const single = computed(() => (visible.value.length === 1 ? visible.value[0] : null));
const text = computed(() => single.value?.title ?? `${visible.value.length} foutmeldingen`);
const supportingText = computed(
  () => single.value?.summary ?? 'Er ging meer dan één ding mis. Bekijk de meldingen voor wat er aan de hand is.',
);
// One error whose sentence says it all has nothing more to show.
const hasMore = computed(() => !single.value || !!single.value.details);
</script>

<template>
  <nldd-notification
    v-if="visible.length > 0 && !detailsOpen"
    variant="critical"
    :text="text"
    :supporting-text="supportingText"
    data-testid="error-notification"
    @dismiss="store.dismissVisible"
  >
    <nldd-button
      v-if="hasMore"
      slot="actions"
      :text="single ? 'Technische details' : 'Bekijk meldingen'"
      data-testid="error-notification-details"
      @click="store.showDetails(visible)"
    ></nldd-button>
  </nldd-notification>
</template>
