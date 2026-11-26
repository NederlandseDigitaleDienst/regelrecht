<script setup lang="ts">
defineProps<{
  data: { label: string };
}>();
</script>

<template>
  <button
    type="button"
    class="close"
    aria-label="Verberg regelwerk"
  >
    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
      <path
        fill="none"
        stroke="currentColor"
        stroke-linecap="round"
        stroke-linejoin="round"
        stroke-width="2"
        d="M18 6L6 18M6 6l12 12"
      />
    </svg>
  </button>

  <div class="label">{{ data.label }}</div>
</template>

<style scoped>
.close {
  float: right;
  cursor: pointer;
  border-radius: 6px;
  /* Translucent ink so the close button sits on any service colour
   * (light or dark mode) without a per-mode override. color-mix on
   * --semantics-content-color picks up the theme's high-contrast
   * ink - black-ish in light, white-ish in dark - at 65% opacity.
   *
   * color-mix(in oklch, ...) needs Chrome 111+/FF 113+/Safari 16.2+,
   * which is strictly within the design system's existing light-dark()
   * floor (Chrome 123+/FF 120+/Safari 17.5+) - anything that can paint
   * the rest of the page can paint this too. The fallback color
   * declaration below makes the button visible on the (long-defunct)
   * older browsers that load this stylesheet but can't parse color-mix. */
  background: var(--semantics-content-color);
  background: color-mix(in oklch, var(--semantics-content-color), transparent 35%);
  padding: 4px;
  color: var(--semantics-surfaces-background-color);
  border: 0;
  line-height: 0;
}

.close svg {
  width: 20px;
  height: 20px;
}

.label {
  text-align: center;
}
</style>
