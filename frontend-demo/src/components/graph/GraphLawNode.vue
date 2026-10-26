<script setup lang="ts">
import OrgLogo from '../OrgLogo.vue';
import { useDemo } from '../../store/demoStore';
import type { GraphLaw, LawNodeData } from '../../graph/lawGraph';
import type { LawEntry } from '../../data/loadCorpus';

// The frame of one law in the graph: organisation mark, name and executing
// organisation. The boxes with sources, inputs and outputs are child nodes.
// The node reads the law's name and its service on top of what the graph
// itself needs.
const props = defineProps<{ data: LawNodeData<GraphLaw & Pick<LawEntry, 'name' | 'service'>> }>();
const { corpus } = useDemo();
</script>

<template>
  <div :class="['graph-law', props.data.selected ? 'graph-law--selected' : '']" :title="props.data.law.id">
    <div class="graph-law__header">
      <OrgLogo :service="props.data.law.service" size="sm" />
      <div class="graph-law__title">
        <span class="graph-law__name">{{ props.data.law.name }}</span>
        <span class="graph-law__service">{{ corpus?.services?.[props.data.law.service]?.name ?? props.data.law.service }}</span>
      </div>
    </div>
  </div>
</template>
