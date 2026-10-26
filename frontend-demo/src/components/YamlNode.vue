<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { childPath as pathOf, initiallyOpen, scalarHint, type OpenQuery } from './yamlExpand';

// One node of a parsed YAML document rendered as a collapsible tree. Mappings
// and sequences fold; scalars show typed. A `source.regulation: <law>` value
// becomes a link that opens that law in a new law tab.

/** The prepared open state the whole tree shares; `version` changes to re-apply it. */
type ExpandState = Pick<OpenQuery, 'all' | 'paths' | 'folded'> & { version: number };

const props = withDefaults(defineProps<{
  name?: string | number | null;
  value?: unknown;
  path?: string;
  depth?: number;
  /** Dotted paths that start expanded, together with the nodes above them; '*' matches any segment. */
  expanded?: ExpandState;
  // Only `has` is read; a `Pick` also keeps the runtime prop type `Object`.
  lawIds?: Pick<ReadonlySet<string>, 'has'>;
  parentKey?: string;
}>(), {
  name: null,
  // withDefaults only types a factory for an `unknown` prop, and that factory
  // must return `{}`; the cast keeps the plain `default: null` it had.
  value: null as never,
  path: '',
  depth: 0,
  expanded: () => ({ paths: [], folded: [], version: 0, all: null }),
  lawIds: () => new Set(),
  parentKey: '',
});
// `isLawLink` guarantees the value is a string where the template emits it.
const emit = defineEmits<{ 'open-law': [lawId: string] }>();

const isMap = computed(() => props.value !== null && typeof props.value === 'object' && !Array.isArray(props.value));
const isList = computed(() => Array.isArray(props.value));
const isContainer = computed(() => isMap.value || isList.value);

/** De voorbereide stand: zie yamlExpand.ts voor waarom die zo staat. */
function openHere() {
  return initiallyOpen({ path: props.path, depth: props.depth, all: props.expanded.all, paths: props.expanded.paths, folded: props.expanded.folded });
}

const open = ref(openHere());
watch(() => props.expanded.version, () => {
  open.value = openHere();
});

const entries = computed((): [string | number, unknown][] => {
  if (isMap.value) return Object.entries(props.value as Record<string, unknown>);
  if (isList.value) return (props.value as unknown[]).map((v, i) => [i, v]);
  return [];
});

function childPath(key: string | number, child: unknown) {
  return pathOf(props.path, key, child, isList.value);
}

function summary() {
  if (isList.value) return `${(props.value as unknown[]).length} ${(props.value as unknown[]).length === 1 ? 'item' : 'items'}`;
  // Only called for a container, so what is not a list is a mapping.
  const v = props.value as Record<string, unknown>;
  if (v.number !== undefined && v.text !== undefined) return `artikel ${v.number}`;
  const hint = scalarHint(v) ?? [v.operation, v.regulation].find((h) => typeof h === 'string');
  return hint !== undefined ? String(hint) : `${Object.keys(v).length} velden`;
}

const scalarClass = computed(() => {
  const v = props.value;
  if (typeof v === 'string' && v.startsWith('$')) return 'yaml-var';
  if (typeof v === 'string') return 'yaml-string';
  return 'yaml-scalar';
});

const isLawLink = computed(
  () => props.parentKey === 'source' && props.name === 'regulation' && typeof props.value === 'string',
);

// Long legal text (the `text` of an article) starts folded to its first
// sentence; the audience came for the machine-readable part.
const LONG = 220;
const isLong = computed(() => typeof props.value === 'string' && props.value.length > LONG);
const showAll = ref(false);

function scalarText() {
  const v = props.value;
  if (v === null || v === undefined) return 'null';
  if (typeof v === 'string') {
    if (isLong.value && !showAll.value) return `${v.slice(0, LONG).trimEnd()}…`;
    return v;
  }
  return String(v);
}
</script>

<template>
  <div class="yaml-node" :data-path="path">
    <template v-if="isContainer">
      <button type="button" class="yaml-toggle" :aria-expanded="open ? 'true' : 'false'" @click="open = !open">
        <span class="yaml-key">{{ name === null ? '' : typeof name === 'number' ? '-' : name }}</span><span v-if="name !== null && typeof name !== 'number'">:</span>
        <span v-if="!open" class="yaml-scalar"> {{ summary() }}</span>
      </button>
      <div v-if="open">
        <YamlNode
          v-for="[k, v] in entries"
          :key="k"
          :name="k"
          :value="v"
          :path="childPath(k, v)"
          :depth="depth + 1"
          :expanded="expanded"
          :law-ids="lawIds"
          :parent-key="isMap ? String(name ?? '') : parentKey"
          @open-law="emit('open-law', $event)"
        />
      </div>
    </template>
    <template v-else>
      <span class="yaml-key">{{ isList || typeof name === 'number' ? '-' : name }}</span><span v-if="!(isList || typeof name === 'number')">:</span>
      <button v-if="isLawLink && lawIds.has(value as string)" type="button" class="yaml-link" :title="`Open ${value}`" @click="emit('open-law', value as string)">
        {{ value }}
      </button>
      <span v-else :class="scalarClass" style="white-space: pre-wrap"> {{ scalarText() }}</span>
      <button v-if="isLong" type="button" class="yaml-link" @click="showAll = !showAll">{{ showAll ? 'minder' : 'meer' }}</button>
    </template>
  </div>
</template>
