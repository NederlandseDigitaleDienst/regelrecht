<script setup lang="ts">
// The options of a screen's menu (Menuactie): those without a group first,
// then each group under its title, such as the facts per party in the admin
// panel. An nldd-menu-group draws the divider above itself.
import { computed } from 'vue';
import type { Menuactie } from './schil';

const props = defineProps<{ opties: Menuactie['opties'] }>();

const groepen = computed(() => {
  const per = new Map<string, Menuactie['opties']>();
  for (const o of props.opties) per.set(o.groep ?? '', [...(per.get(o.groep ?? '') ?? []), o]);
  return [...per].map(([titel, opties]) => ({ titel, opties })).sort((a, b) => Number(!!a.titel) - Number(!!b.titel));
});
</script>

<template>
  <template v-for="g in groepen" :key="g.titel">
    <nldd-menu-group v-if="g.titel" :text="g.titel">
      <nldd-menu-item v-for="o in g.opties" :key="o.tekst" :text="o.tekst" @select="o.kies()"></nldd-menu-item>
    </nldd-menu-group>
    <template v-else>
      <nldd-menu-item v-for="o in g.opties" :key="o.tekst" :text="o.tekst" @select="o.kies()"></nldd-menu-item>
    </template>
  </template>
</template>
