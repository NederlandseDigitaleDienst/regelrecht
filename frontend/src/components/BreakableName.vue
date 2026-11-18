<script setup lang="ts">
import { computed } from 'vue';

/**
 * Renders an identifier so the browser may wrap *after* each underscore.
 *
 * Long snake_case names (e.g. `geldige_partnertypen`) have no native break
 * opportunity, so a narrow column breaks them mid-word - one character per
 * line in the worst case. Splitting on `_` and emitting a real <wbr> just
 * *before* each underscore gives a clean, opt-in break point: the name stays
 * on one line when it fits, and wraps only at underscores when it doesn't,
 * with the underscore leading the continuation line (a clearer "this
 * continues" signal than a trailing underscore). Unlike a zero-width space
 * this leaves nothing behind in copied text.
 *
 * Read-only display only - editable fields keep the raw value.
 */
const props = defineProps<{
  name: string;
}>();

// 'a_b_c' → ['a', 'b', 'c']. Rendered as: a + <wbr> + '_b' + <wbr> + '_c'
const segments = computed(() => String(props.name).split('_'));
</script>

<template><span class="breakable-name"><template
  v-for="(seg, i) in segments"
  :key="i"
><template v-if="i > 0"><wbr>_</template>{{ seg }}</template></span></template>
