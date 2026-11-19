<script setup lang="ts">
// The public side: the citizen's dossier and a new application.
import Tabs from '../gedeeld/Tabs.vue';

const tabs = [
  { naam: 'dossier', tekst: 'Mijn dossier', pad: '/burger', icoon: 'document' },
  { naam: 'aanvraag', tekst: 'Aanvraag doen', pad: '/burger/aanvraag', icoon: 'add' },
];
</script>

<template>
  <nldd-simple-section padding-bottom="0">
    <Tabs :tabs="tabs" label="Burger" />
  </nldd-simple-section>
  <RouterView />
</template>
