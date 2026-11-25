<script setup lang="ts">
// The one notification for the errors the editor reports (errors store). One
// error shows as itself; several show as a count, with the list in the
// details sheet, so a run of failures does not stack up on screen. Critical,
// so it stays until dismissed.
//
// Renders nothing: the notification is raised from code (lib/raiseNotification).
// A notification moves itself to a shared area, so an element Vue rendered is
// no longer where Vue left it, and a `v-if` that comes back true inserts the
// next one into nowhere.
import { computed, onBeforeUnmount, watch } from 'vue';
import { storeToRefs } from 'pinia';
import { raiseNotification, type NotificationHandle, type NotificationOptions } from '../lib/raiseNotification';
import { useErrorsStore } from '../stores/errors';

const store = useErrorsStore();
const { visible, detailsOpen } = storeToRefs(store);

const single = computed(() => (visible.value.length === 1 ? visible.value[0] : null));

/** What to show, or null for nothing. */
const content = computed<{ text: string; supportingText: string; actionText: string | null } | null>(() => {
  // It steps aside while the details sheet is open: a notification moves into
  // the open overlay, where it would cover the sheet's title bar and its way
  // out.
  if (visible.value.length === 0 || detailsOpen.value) return null;
  if (single.value) {
    return {
      text: single.value.title,
      supportingText: single.value.summary,
      // An error whose sentence says it all has nothing more to show.
      actionText: single.value.details ? 'Technische details' : null,
    };
  }
  return {
    text: `${visible.value.length} foutmeldingen`,
    supportingText: 'Er ging meer dan één ding mis. Bekijk de meldingen voor wat er aan de hand is.',
    actionText: 'Bekijk meldingen',
  };
});

let handle: NotificationHandle | null = null;

function drop(): void {
  handle?.dismiss();
  handle = null;
}

function options(shown: NonNullable<typeof content.value>): NotificationOptions {
  return {
    variant: 'critical',
    text: shown.text,
    supportingText: shown.supportingText,
    actions: shown.actionText
      ? [{
          text: shown.actionText,
          onClick: () => {
            // A click on an action takes the notification away by itself.
            handle = null;
            store.showDetails(visible.value);
          },
        }]
      : [],
  };
}

watch(
  content,
  (shown, previous) => {
    if (!shown) {
      drop();
      return;
    }
    // The same content again is not news: an update would slide it back in.
    if (handle && previous && JSON.stringify(shown) === JSON.stringify(previous)) return;
    if (handle) {
      handle.update(options(shown));
      return;
    }
    const raised = raiseNotification(options(shown));
    if (!raised) return;
    raised.element.dataset.testid = 'error-notification';
    // Fired by the close button and by Escape, not by `dismiss()` here.
    raised.element.addEventListener('dismiss', () => {
      if (handle !== raised) return;
      handle = null;
      store.dismissVisible();
    });
    handle = raised;
  },
  { immediate: true },
);

onBeforeUnmount(drop);
</script>

<template>
  <!-- Renders nothing; see above. -->
</template>
