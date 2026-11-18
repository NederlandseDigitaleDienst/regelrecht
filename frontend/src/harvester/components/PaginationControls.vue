<script setup lang="ts">
defineProps<{
  currentPage: number;
  totalPages: number;
}>();

const emit = defineEmits<{
  'page-change': [page: number];
}>();

function onPageChange(event: Event) {
  emit('page-change', (event as CustomEvent<{ page: number; href?: string }>).detail.page);
}
</script>

<template>
  <nldd-pagination
    v-if="totalPages > 1"
    centered
    :current="currentPage"
    :total="totalPages"
    @page-change="onPageChange"
  ></nldd-pagination>
</template>
