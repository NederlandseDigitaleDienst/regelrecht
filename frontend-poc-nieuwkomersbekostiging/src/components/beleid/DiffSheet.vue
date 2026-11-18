<template>
  <nldd-sheet
    ref="sheetEl"
    placement="right"
    width="720px"
    accessible-label="Wetswijziging bekijken"
    @close="emit('close')"
  >
    <nldd-page>
      <div class="ds-content">
        <nldd-title heading-level="2" size="display-3">
          <span slot="overline">Wetswijziging</span>
          <span>{{ heading }}</span>
        </nldd-title>

        <nldd-banner v-if="!anyChange" variant="accent">
          Er zijn geen wijzigingen ten opzichte van het huidige recht.
        </nldd-banner>

        <template v-else>
          <section v-if="defChanges.length" class="ds-block">
            <nldd-title heading-level="3" size="display-5"><span>Gewijzigde parameters</span></nldd-title>
            <ul class="ds-defs">
              <li v-for="(c, i) in defChanges" :key="i">
                <span class="ds-art">artikel {{ c.article }}</span>
                <code>{{ c.name }}</code>:
                <span class="ds-old">{{ c.before }}</span>
                <nldd-icon icon="arrow-right" size="16"></nldd-icon>
                <span class="ds-new">{{ c.after }}</span>
              </li>
            </ul>
          </section>

          <section v-if="tekstChanges.length" class="ds-block">
            <nldd-title heading-level="3" size="display-5"><span>Gewijzigde wettekst</span></nldd-title>
            <div v-for="(c, i) in tekstChanges" :key="i" class="ds-tekst">
              <span class="ds-art">artikel {{ c.article }}</span>
              <p class="ds-tekst-oud">{{ c.before }}</p>
              <p class="ds-tekst-nieuw">{{ c.after }}</p>
            </div>
          </section>

          <section v-for="d in fileDiffs" :key="d.lawPath" class="ds-block">
            <nldd-title heading-level="3" size="display-5"><span>{{ d.docName }}</span></nldd-title>
            <div class="ds-diff">
              <div
                v-for="(line, i) in d.lines"
                :key="i"
                class="ds-line"
                :class="`ds-${line.type}`"
              >
                <span class="ds-sign">{{ sign(line.type) }}</span>
                <span class="ds-text">{{ line.text }}</span>
              </div>
            </div>
          </section>
        </template>
      </div>
    </nldd-page>
  </nldd-sheet>
</template>

<script setup>
import { ref, computed, watch, nextTick } from 'vue';
import * as yaml from 'js-yaml';
import { useLawStore } from '../../engine/lawStore.js';
import { compactDiff, definitionDiff, articleTextDiff } from '../../lib/diff.js';

const props = defineProps({
  open: { type: Boolean, default: false },
});
const emit = defineEmits(['close']);

const { editableDocs, activeVariant, werkversieLabel, changeCount, version } = useLawStore();

const sheetEl = ref(null);

const heading = computed(() =>
  activeVariant.value ? `Werkversie ${werkversieLabel.value} ten opzichte van huidig recht` : `Handmatige wijzigingen in huidig recht`,
);

const fileDiffs = computed(() => {
  version.value;
  return editableDocs.value
    .filter((d) => d.currentYaml !== d.baseYaml)
    .map((d) => ({
      lawPath: d.entry.path,
      docName: d.entry.name,
      lines: compactDiff(d.baseYaml, d.currentYaml),
    }));
});

const defChanges = computed(() => {
  version.value;
  const all = [];
  for (const d of editableDocs.value) {
    if (d.currentYaml === d.baseYaml) continue;
    // Vergelijk het geparste basis-doc met het huidige doc.
    all.push(...definitionDiff(parseBase(d), d.doc));
  }
  return all;
});

/**
 * Artikelen waarvan de wettekst meebewoog. Staat apart van de parameters,
 * want een wet is zijn tekst: wie alleen de getallen ziet veranderen, leest
 * niet wat er juridisch gebeurt.
 */
const tekstChanges = computed(() => {
  version.value;
  const all = [];
  for (const d of editableDocs.value) {
    if (d.currentYaml === d.baseYaml) continue;
    all.push(...articleTextDiff(parseBase(d), d.doc));
  }
  return all;
});

// Parse de basis-YAML on demand (goedkoop; alleen bij open sheet).
function parseBase(d) {
  try {
    return yaml.load(d.baseYaml);
  } catch {
    return {};
  }
}

const anyChange = computed(() => fileDiffs.value.length > 0 || defChanges.value.length > 0);

function sign(type) {
  return { add: '+', del: '−', context: ' ', gap: '' }[type] ?? ' ';
}

watch(
  () => props.open,
  async (open) => {
    if (!open) {
      sheetEl.value?.hide();
      return;
    }
    await nextTick();
    sheetEl.value?.show();
  },
  { immediate: true },
);
</script>

<style scoped>
.ds-tekst { display: flex; flex-direction: column; gap: 4px; margin-bottom: var(--primitives-space-12); }
.ds-tekst-oud, .ds-tekst-nieuw { margin: 0; padding: var(--primitives-space-8); border-radius: var(--semantics-surfaces-corner-radius); font-size: 0.9em; }
.ds-tekst-oud { background: var(--semantics-surfaces-tinted-background-color); text-decoration: line-through; color: var(--semantics-content-secondary-color); }
.ds-tekst-nieuw { background: var(--semantics-surfaces-tinted-background-color); }

.ds-content { padding: var(--primitives-space-24); display: flex; flex-direction: column; gap: var(--primitives-space-16); }
.ds-block { display: flex; flex-direction: column; gap: var(--primitives-space-8); }
.ds-defs { margin: 0; padding-left: var(--primitives-space-16); display: flex; flex-direction: column; gap: 4px; }
.ds-defs li { font-size: 0.9em; display: flex; align-items: center; gap: 6px; flex-wrap: wrap; }
.ds-art { color: var(--semantics-content-secondary-color); font-size: 0.85em; }
.ds-old { color: var(--semantics-content-critical-color); text-decoration: line-through; }
.ds-new { color: var(--semantics-content-success-color); font-weight: 600; }
.ds-diff {
  font-family: var(--primitives-font-family-monospace, monospace);
  font-size: 0.78em;
  line-height: 1.5;
  border: 1px solid var(--semantics-dividers-color);
  border-radius: var(--semantics-surfaces-corner-radius);
  overflow-x: auto;
  background: var(--semantics-surfaces-tinted-background-color);
}
.ds-line { display: flex; gap: 8px; padding: 0 8px; white-space: pre; }
.ds-sign { flex: none; width: 1ch; color: var(--semantics-content-secondary-color); }
.ds-add { background: color-mix(in srgb, var(--semantics-content-success-color) 14%, transparent); }
.ds-del { background: color-mix(in srgb, var(--semantics-content-critical-color) 12%, transparent); }
.ds-gap { color: var(--semantics-content-secondary-color); justify-content: center; }
</style>
