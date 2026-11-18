<script setup>
import { onMounted } from 'vue';
import { refreshSession } from '../session.js';

onMounted(refreshSession);
</script>

<template>
  <nldd-shell>
    <router-view />
  </nldd-shell>
</template>
