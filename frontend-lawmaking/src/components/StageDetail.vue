<template>
  <Teleport to="body">
    <nldd-sheet
      ref="sheetEl"
      placement="right"
      width="420px"
      accessible-label="Toelichting fase"
      @close="onClose"
    >
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          :text="shownStage?.lawLabel || ''"
          dismiss-text="Sluit"
          @dismiss="sheetEl?.hide()"
        ></nldd-top-title-bar>

        <nldd-simple-section v-if="shownStage">
          <!-- Git ⇆ wetgeving mapping -->
          <nldd-list appearance="box-tinted">
            <nldd-list-item size="md">
              <nldd-text-cell text="Git / CI/CD" :supporting-text="shownStage.gitLabel"></nldd-text-cell>
            </nldd-list-item>
            <nldd-list-item size="md">
              <nldd-text-cell text="Wetgeving" :supporting-text="shownStage.lawLabel"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>

          <nldd-spacer size="16"></nldd-spacer>

          <nldd-rich-text>
            <p>{{ shownStage.description }}</p>
          </nldd-rich-text>

          <nldd-spacer size="16"></nldd-spacer>

          <div class="stage-detail__meta">
            <nldd-tag :text="shownStage.type"></nldd-tag>
            <nldd-rich-text v-if="shownStage.subtitle">
              <p class="stage-detail__subtitle">{{ shownStage.subtitle }}</p>
            </nldd-rich-text>
          </div>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch, nextTick } from 'vue';
import type { Stage } from '../data/types';

/** The two methods of `nldd-sheet` this component calls. */
interface SheetElement extends HTMLElement {
  show(): void;
  hide(): void;
}

const props = withDefaults(defineProps<{ stage?: Stage | null }>(), { stage: null });

const emit = defineEmits<{ close: [] }>();

const sheetEl = ref<SheetElement | null>(null);

// `shownStage` keeps the last stage rendered while the sheet plays its close
// animation, so the panel never flashes empty on dismiss. It is cleared only
// once the sheet's `close` event confirms the animation finished.
const shownStage = ref<Stage | null>(null);

// Drive the imperative nldd-sheet open/close from the `stage` prop so the
// parent stays declarative (`:stage` / `@close`).
watch(
  () => props.stage,
  async (stage) => {
    if (!stage) {
      sheetEl.value?.hide();
      return;
    }
    shownStage.value = stage;
    // Render the v-if="shownStage" content before opening so the sheet never
    // animates in empty. `{ flush: 'post' }` wouldn't suffice here: this watcher
    // itself sets `shownStage`, so the content render is still pending when the
    // callback runs - awaiting nextTick flushes it first.
    await nextTick();
    sheetEl.value?.show();
  },
);

// The single close path: fires once the sheet finishes closing, whatever
// triggered it (the Sluit button's hide(), Escape, backdrop, or the parent
// nulling `stage`). Clear the rendered stage and notify the parent so its
// `selectedStageId` stays in sync and the node can re-open later.
function onClose() {
  shownStage.value = null;
  emit('close');
}
</script>

<style scoped>
.stage-detail__meta {
  display: flex;
  align-items: center;
  gap: var(--spacing-2);
}

.stage-detail__subtitle {
  margin: 0;
}
</style>
