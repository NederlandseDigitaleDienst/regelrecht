<script setup lang="ts">
import { reactive } from 'vue';
import OrgLogo from './OrgLogo.vue';
import type { ServiceInfo } from '../data/loadCorpus';
import { useI18n } from '../i18n/index.js';

const { t } = useI18n();

// Laws grouped by the organisation that executes them, as one design-system
// tree: an organisation is a branch row (mark, name, count) that folds, its
// laws are the child rows. Two modes: `pick` (one law is open, the row is a
// button) and `check` (any number of laws are on, the row toggles a checkbox).
// Shared by the Wetten and Graaf side panels so they read the same.

/** The fields of a law the tree reads. */
interface TreeLaw {
  id: string;
  name: string;
}
/** One organisation with its laws. */
interface LawGroup {
  service: string;
  info: Pick<ServiceInfo, 'name'>;
  laws: TreeLaw[];
}

const props = withDefaults(defineProps<{
  groups: LawGroup[];
  mode?: 'pick' | 'check';
  activeId?: string | null;
  // Only `has` is read; a `Pick` also keeps the runtime prop type `Object`.
  checked?: Pick<ReadonlySet<string>, 'has'>;
  /** Optional supporting text per law. */
  supportingText?: ((law: TreeLaw) => string | undefined) | null;
}>(), {
  mode: 'pick',
  activeId: null,
  checked: () => new Set(),
  supportingText: null,
});
const emit = defineEmits<{
  pick: [lawId: string];
  toggle: [lawId: string];
}>();

const folded = reactive<Record<string, boolean>>({});
function isOpen(service: string) {
  return !folded[service];
}
function checkedCount(group: LawGroup) {
  return group.laws.filter((l) => props.checked.has(l.id)).length;
}
function onRow(law: TreeLaw) {
  if (props.mode === 'check') emit('toggle', law.id);
  else emit('pick', law.id);
}
</script>

<template>
  <nldd-list type="tree" :accessible-label="t('wet.tree.label')">
    <nldd-list-item v-for="group in groups" :key="group.service" size="md" button :expanded="isOpen(group.service)" @click="folded[group.service] = !folded[group.service]">
      <nldd-cell><OrgLogo :service="group.service" size="sm" /></nldd-cell>
      <nldd-spacer-cell size="12"></nldd-spacer-cell>
      <nldd-text-cell :text="group.info.name" :supporting-text="t.plural(group.laws.length, 'wet.sidebar.count')"></nldd-text-cell>
      <nldd-cell v-if="mode === 'check' && checkedCount(group)"><nldd-badge color="accent" :number="checkedCount(group)" :accessible-label="t('wet.tree.chosen', { n: checkedCount(group) })"></nldd-badge></nldd-cell>
      <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
      <nldd-list-item
        v-for="law in group.laws"
        :key="law.id"
        slot="children"
        size="sm"
        button
        :selected="mode === 'pick' && law.id === activeId ? true : undefined"
        :accessible-label="mode === 'check' ? t('wet.tree.law_state', { name: law.name, state: t(checked.has(law.id) ? 'wet.tree.law_on' : 'wet.tree.law_off') }) : undefined"
        @click.stop="onRow(law)"
      >
        <nldd-spacer-cell size="20"></nldd-spacer-cell>
        <nldd-cell v-if="mode === 'check'"><nldd-checkbox :checked="checked.has(law.id) || undefined" aria-hidden="true" tabindex="-1"></nldd-checkbox></nldd-cell>
        <nldd-spacer-cell v-if="mode === 'check'" size="12"></nldd-spacer-cell>
        <nldd-text-cell size="sm" :text="law.name" :supporting-text="supportingText ? supportingText(law) : undefined"></nldd-text-cell>
        <nldd-icon-cell v-if="mode === 'pick'" icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
      </nldd-list-item>
    </nldd-list-item>
    <nldd-inline-dialog slot="empty" :text="t('wet.tree.empty.title')" :supporting-text="t('wet.tree.empty.body')"></nldd-inline-dialog>
  </nldd-list>
</template>
