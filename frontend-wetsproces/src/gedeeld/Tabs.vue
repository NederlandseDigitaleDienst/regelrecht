<script setup lang="ts">
// The tabs of one side of the app, as route navigation: a link per tab, which
// the router follows without loading the page again.
import { useRoute, useRouter } from 'vue-router';

defineProps<{ tabs: { naam: string; tekst: string; pad: string; icoon: string }[]; label: string }>();
const route = useRoute();
const router = useRouter();
</script>

<template>
  <nldd-tab-bar navigation :accessible-label="label">
    <nldd-tab-bar-item
      v-for="t in tabs"
      :key="t.naam"
      :text="t.tekst"
      :icon="t.icoon"
      :href="t.pad"
      :current="route.name === t.naam || undefined"
      @click.prevent="router.push(t.pad)"
    ></nldd-tab-bar-item>
  </nldd-tab-bar>
</template>
