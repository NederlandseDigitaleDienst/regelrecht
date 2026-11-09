<script setup lang="ts">
// "Help" — a short support sheet opened from the account menu. Points
// users at the support inbox when they run into something. Exposes show()/hide()
// so the shell can drive it from the menu item.
import { ref } from 'vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import { SUPPORT_EMAIL } from '../constants';

const sheetEl = ref<NLDDSheet | null>(null);

function show(): void {
  sheetEl.value?.show?.();
}
function hide(): void {
  sheetEl.value?.hide?.();
}

defineExpose({ show, hide });
</script>

<template>
  <Teleport to="body">
    <nldd-sheet ref="sheetEl" placement="right" width="480px" @close="hide">
      <nldd-page sticky-header>
        <nldd-top-title-bar slot="header" text="Help" dismiss-text="Sluit" @dismiss="hide"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <nldd-rich-text>
            <p>
              Loop je ergens tegenaan of heb je een vraag over RegelRecht? Stuur ons gerust een e-mail,
              dan helpen we je zo snel mogelijk verder.
            </p>
          </nldd-rich-text>
          <nldd-spacer size="24"></nldd-spacer>
          <nldd-link
            size="md"
            :href="`mailto:${SUPPORT_EMAIL}`"
            end-icon="email"
            :text="SUPPORT_EMAIL"
          ></nldd-link>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
