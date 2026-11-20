<script setup lang="ts">
// The colour scheme as a submenu of an account menu: system, light or dark,
// with the one that is on marked. A submenu rather than a setting, because it
// is something you switch briefly and the radios show at once which one is on.
// Shared by the editor and the wetsproces screens; the caller keeps the
// scheme (useColorScheme) and sets what is chosen.
defineProps<{ waarde: string }>();
const emit = defineEmits<{ kies: [waarde: string] }>();

const opties = [
  ['auto', 'Systeem', 'display'],
  ['light', 'Licht', 'light-mode'],
  ['dark', 'Donker', 'dark-mode'],
] as const;

// `select` carries no payload; the chosen value is on the item itself.
function onSelect(e: Event) {
  const value = (e.target as Element | null)?.getAttribute?.('value');
  if (value) emit('kies', value);
}
</script>

<template>
  <nldd-menu-item text="Weergave" icon="appearance">
    <nldd-menu @select="onSelect">
      <nldd-menu-item
        v-for="[value, label, icon] in opties"
        :key="value"
        type="radio"
        :value="value"
        :text="label"
        :icon="icon"
        :selected="waarde === value || undefined"
      ></nldd-menu-item>
    </nldd-menu>
  </nldd-menu-item>
</template>
