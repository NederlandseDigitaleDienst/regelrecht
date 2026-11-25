<script setup lang="ts">
// The technical text behind the readable error messages (lib/userError), for
// whoever reports or debugs the problem. Mounted once in the shell and driven
// by the errors store. The code viewer brings its own copy button.
import { storeToRefs } from 'pinia';
import { useErrorsStore } from '../stores/errors';

const store = useErrorsStore();
const { detailsOpen, detailsOf } = storeToRefs(store);
</script>

<template>
  <Teleport to="body">
    <nldd-sheet placement="right" width="560px" :open="detailsOpen" data-testid="error-details-sheet" @close="store.closeDetails">
      <nldd-page sticky-header>
        <nldd-top-title-bar slot="header" text="Technische details" dismiss-text="Sluit"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-rich-text>
            <p>Stuur de technische tekst mee als je een probleem meldt.</p>
          </nldd-rich-text>
          <template v-for="(error, index) in detailsOf" :key="index">
            <nldd-spacer size="24"></nldd-spacer>
            <nldd-title size="display-5" heading-level="2"><span>{{ error.title }}</span></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-rich-text>
              <p>{{ error.summary }}</p>
            </nldd-rich-text>
            <template v-if="error.details">
              <nldd-spacer size="12"></nldd-spacer>
              <!-- Keyed: the viewer reads its slot once, so new text needs a new viewer. -->
              <nldd-code-viewer :key="error.details" wrap data-testid="error-details-text">{{ error.details }}</nldd-code-viewer>
            </template>
          </template>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
