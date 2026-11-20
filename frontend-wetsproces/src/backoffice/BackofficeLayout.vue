<script setup lang="ts">
// The back office: the work queue of the executor and its chronicle, for the
// admin the admin panel in which they enter what an external party did, and
// in test mode the clock. A tab shows only when the account may use what is
// behind it; the server checks every call itself.
import { computed } from 'vue';
import { ik } from '../gedeeld/ik';
import Tabs from '../gedeeld/Tabs.vue';

const tabs = computed(() => [
  { naam: 'werkvoorraad', tekst: 'Werkvoorraad', pad: '/backoffice', icoon: 'inbox' },
  { naam: 'kroniek', tekst: 'Kroniek', pad: '/backoffice/kroniek', icoon: 'history' },
  ...(ik.value?.partijen_invoeren ? [{ naam: 'beheer', tekst: 'Beheerpaneel', pad: '/backoffice/beheer', icoon: 'form' }] : []),
  ...(ik.value?.klok_zetten ? [{ naam: 'klok', tekst: 'Klok', pad: '/backoffice/klok', icoon: 'calendar' }] : []),
]);
</script>

<template>
  <nldd-simple-section padding-bottom="0">
    <Tabs :tabs="tabs" label="Backoffice" />
  </nldd-simple-section>
  <RouterView />
</template>
