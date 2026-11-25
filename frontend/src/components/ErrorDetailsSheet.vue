<script setup lang="ts">
// The technical text behind a readable error message (lib/userError), for
// whoever reports or debugs the problem. Mounted once in the shell and driven
// by the errorDetails store. The code viewer brings its own copy button.
import { storeToRefs } from 'pinia';
import { useErrorDetailsStore } from '../stores/errorDetails';

const store = useErrorDetailsStore();
const { open, current } = storeToRefs(store);
</script>

<template>
  <Teleport to="body">
    <nldd-sheet placement="right" width="560px" :open="open" data-testid="error-details-sheet" @close="store.close">
      <nldd-page sticky-header>
        <nldd-top-title-bar slot="header" text="Technische details" dismiss-text="Sluit"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-rich-text>
            <p>{{ current?.summary }}</p>
            <p>Stuur de tekst hieronder mee als je dit probleem meldt.</p>
          </nldd-rich-text>
          <nldd-spacer size="16"></nldd-spacer>
          <!-- Keyed: the viewer reads its slot once, so new text needs a new viewer. -->
          <nldd-code-viewer :key="current?.details ?? ''" wrap data-testid="error-details-text">{{ current?.details }}</nldd-code-viewer>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
