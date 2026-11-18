<template>
  <details ref="el" class="panel" :open="open ? true : undefined" @toggle="onToggle">
    <summary class="panel-header">
      <span class="panel-title">{{ title }}</span>
      <nldd-badge v-if="badge" size="sm" color="warning" :text="badge"></nldd-badge>
      <span v-if="!isOpen && summary" class="panel-summary">{{ summary }}</span>
    </summary>
    <p v-if="subtitle" class="panel-subtitle">{{ subtitle }}</p>
    <div class="panel-content"><slot /></div>
  </details>
</template>

<!--
  Collapsible section (progressive disclosure): closed it shows a one-line
  summary, open the content. One title style for every section in the policy
  view, so the left column reads as a list of tasks.
-->

<script setup lang="ts">
import { ref, watch } from 'vue';

const props = withDefaults(
  defineProps<{
    title: string;
    subtitle?: string;
    /** One line shown while the section is closed (the current state of its controls). */
    summary?: string;
    badge?: string;
    /**
     * Open it. When this goes from false to true while the section is closed,
     * it opens: there is something to see that the visitor came for.
     */
    open?: boolean;
  }>(),
  { subtitle: '', summary: '', badge: '', open: false },
);
const isOpen = ref(props.open);
const el = ref<HTMLDetailsElement | null>(null);

function onToggle(event: Event) {
  isOpen.value = (event.target as HTMLDetailsElement).open;
}

// Only on the change to true, and only to open. An `open` that stays true must
// not keep pushing open a section the visitor closed themselves; the panel
// would then fight whoever operates it.
watch(() => props.open, (now, before) => {
  if (!now || before) return;
  if (el.value && !el.value.open) el.value.open = true;
});
</script>

<style scoped>
.panel {
  border: 1px solid var(--semantics-dividers-color);
  border-radius: var(--semantics-surfaces-corner-radius);
  background: var(--semantics-surfaces-base-background-color);
}
.panel-header {
  display: flex; align-items: baseline; gap: var(--primitives-space-8); flex-wrap: wrap;
  padding: var(--primitives-space-12) var(--primitives-space-16);
  cursor: pointer; list-style: none;
}
.panel-header::-webkit-details-marker { display: none; }
.panel-header::before { content: '▸'; color: var(--semantics-content-secondary-color); width: 1em; }
.panel[open] > .panel-header::before { content: '▾'; }
.panel-title { font-weight: 600; font-size: 1.05em; }
.panel-summary { font-size: 0.9em; color: var(--semantics-content-secondary-color); margin-left: auto; text-align: right; }
.panel-subtitle { margin: 0; padding: 0 var(--primitives-space-16); font-size: 0.9em; color: var(--semantics-content-secondary-color); }
.panel-content { padding: var(--primitives-space-12) var(--primitives-space-16) var(--primitives-space-16); display: flex; flex-direction: column; gap: var(--primitives-space-12); }
</style>
