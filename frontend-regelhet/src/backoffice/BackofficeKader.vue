<script setup lang="ts">
// The frame of every screen of the back office: the screen, and over it the
// tools of the test setup for who may set the clock, which the account menu
// opens by hash (routes.ts, `gereedschap`).
import { defineAsyncComponent } from 'vue';
import { RouterView } from 'vue-router';
import { ik } from '../gedeeld/ik';

const Testomgeving = defineAsyncComponent(() => import('./Testomgeving.vue'));
</script>

<template>
  <RouterView />
  <Testomgeving v-if="ik?.klok_zetten" />
</template>
