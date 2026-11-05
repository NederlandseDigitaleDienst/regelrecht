<template>
  <li class="trace-node">
    <div class="tn-line" :class="`tn-${node.node_type}`">
      <nldd-icon-button
        v-if="hasChildren"
        class="tn-toggle"
        size="xs"
        appearance="neutral-transparent"
        :icon="open ? 'caret-down-small' : 'caret-right-small'"
        :expanded="open ? true : undefined"
        :accessible-label="open ? 'Inklappen' : 'Uitklappen'"
        @click="open = !open"
      ></nldd-icon-button>
      <span v-else class="tn-toggle tn-leaf" aria-hidden="true"></span>

      <span class="tn-type">{{ typeLabel }}</span>
      <!-- Staat er een wetsartikel achter deze stap, dan is de naam een link
           naar dat artikel op wetten.overheid.nl. De engine levert die URL zelf
           (anchor.url), dus hij kan niet afwijken van waar de stap vandaan
           kwam. In een nieuw tabblad: de trace is je plaats in het verhaal, en
           die wil je niet kwijt om een artikel na te lezen. -->
      <a
        v-if="wetsartikelUrl"
        class="tn-name tn-link"
        :href="wetsartikelUrl"
        target="_blank"
        rel="noopener"
        :title="`${wetsartikelLabel} openen op wetten.overheid.nl`"
      >{{ node.name }}<nldd-icon icon="external-link" size="inherit"></nldd-icon></a>
      <span v-else class="tn-name">{{ node.name }}</span>
      <span v-if="node.result !== null && node.result !== undefined" class="tn-result">
        = {{ formatResult(node.result) }}
      </span>
    </div>
    <ul v-if="hasChildren && open" class="tn-children">
      <trace-node v-for="(child, i) in node.children" :key="i" :node="child" :depth="depth + 1" />
    </ul>
  </li>
</template>

<script setup>
import { ref, computed } from 'vue';

const props = defineProps({
  node: { type: Object, required: true },
  depth: { type: Number, default: 0 },
});

const hasChildren = computed(() => Array.isArray(props.node.children) && props.node.children.length > 0);
// Toon de bovenste drie lagen open; dieper dichtgeklapt om de boom leesbaar te houden.
const open = ref(props.depth < 2);

const TYPE_LABELS = {
  article: 'artikel',
  action: 'berekening',
  operation: 'bewerking',
  resolve: 'waarde',
  open_term_resolution: 'open norm',
  cross_law_reference: 'verwijzing',
};
const typeLabel = computed(() => TYPE_LABELS[props.node.node_type] ?? props.node.node_type);

/**
 * De link naar het wetsartikel achter deze stap.
 *
 * `anchor` komt uit het wetsmodel en zegt waar de engine stond; `legal_basis`
 * is de citatie die de auteur van de YAML erbij schreef. Die twee worden met
 * opzet uit elkaar gehouden, dus de voorkeur gaat naar het anker: dat is waar
 * de berekening werkelijk vandaan kwam.
 */
const anker = computed(() => props.node.anchor ?? props.node.legal_basis ?? null);
const wetsartikelUrl = computed(() => {
  const url = anker.value?.url;
  // Alleen http(s): een `javascript:`-url uit een corpusbestand zou anders
  // hier als link belanden.
  return typeof url === 'string' && /^https?:\/\//i.test(url) ? url : null;
});
const wetsartikelLabel = computed(() => {
  const a = anker.value;
  if (!a) return '';
  const delen = [a.law, a.article ? `artikel ${a.article}` : null].filter(Boolean);
  return delen.join(', ') || 'Het artikel';
});

function formatResult(value) {
  if (typeof value === 'boolean') return value ? 'waar' : 'onwaar';
  if (typeof value === 'number') {
    if (!Number.isInteger(value)) return value.toLocaleString('nl-NL', { maximumFractionDigits: 2 });
    return value.toLocaleString('nl-NL');
  }
  return String(value);
}
</script>

<style scoped>
.trace-node { list-style: none; }
.tn-line {
  display: flex;
  align-items: baseline;
  gap: var(--primitives-space-4, 4px);
  padding: 2px 0;
  font-size: 0.85em;
  line-height: 1.5;
}
.tn-toggle {
  flex: none;
  align-self: center;
}
/* Uitlijning van bladeren (zonder knop) met de icon-button ernaast. */
.tn-leaf {
  display: inline-block;
  width: 24px;
  height: 24px;
}
.tn-type {
  flex: none;
  font-size: 0.75em;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--semantics-content-secondary-color);
  min-width: 5.5em;
}
.tn-name {
  font-family: var(--primitives-font-family-monospace, monospace);
  color: var(--semantics-content-color);
}
.tn-article > .tn-name { font-weight: 700; }
.tn-link {
  color: var(--semantics-content-accent-color);
  text-decoration: underline;
  display: inline-flex; align-items: center; gap: 3px;
}
.tn-link nldd-icon { opacity: .7; }
.tn-result {
  font-family: var(--primitives-font-family-monospace, monospace);
  color: var(--semantics-content-accent-color);
  font-weight: 600;
}
.tn-children {
  margin: 0;
  padding-left: 18px;
  border-left: 1px dashed var(--semantics-dividers-color);
  margin-left: 8px;
}
</style>
