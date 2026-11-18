<script setup lang="ts">
// Het structuurrapport van een traject, in een bottom-sheet.
//
// De URL is de open-stand: de route `traject-structuur-controle` opent deze
// sheet en sluiten navigeert terug naar Instellingen > Algemeen. Zonder dat
// laatste blijft het adres beweren dat de sheet openstaat en gaat hij bij een
// herlaad meteen weer open.
//
// Het rapport zelf zit in TrajectIntegrityPane. Die doet zijn eigen netwerkpoot
// en laadt bij mount, dus de `v-if` hieronder zorgt er meteen voor dat elke
// keer dat je de sheet opent een verse controle draait - wat je van een
// controle ook verwacht.
import { nextTick, ref, watch } from 'vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import TrajectIntegrityPane from './TrajectIntegrityPane.vue';

const props = withDefaults(
  defineProps<{
    /** Traject-ref uit de URL (`{slug}-{8hex}`). */
    trajectRef?: string | null;
    /** Staat de route die deze sheet toont? */
    open?: boolean;
  }>(),
  { trajectRef: null, open: false },
);

const emit = defineEmits<{ close: [] }>();

const sheetRef = ref<NLDDSheet | null>(null);

watch(
  () => props.open,
  async (open) => {
    await nextTick();
    // `?.` op show/hide: in tests (happy-dom) is het custom element niet
    // geüpgraded en bestaan die methodes niet.
    if (open) sheetRef.value?.show?.();
    else sheetRef.value?.hide?.();
  },
  { immediate: true },
);
</script>

<template>
  <nldd-sheet
    ref="sheetRef"
    placement="bottom"
    accessible-label="Traject-structuur controleren"
    @close="emit('close')"
  >
    <nldd-page sticky-header>
      <nldd-top-title-bar
        slot="header"
        text="Traject-structuur controleren"
        dismiss-text="Sluit"
        collapse-anchor="structuur-controle-titel"
        @dismiss="emit('close')"
      ></nldd-top-title-bar>

      <TrajectIntegrityPane v-if="open" :traject-ref="trajectRef"></TrajectIntegrityPane>
    </nldd-page>
  </nldd-sheet>
</template>
