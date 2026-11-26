<script setup lang="ts">
// The contents of the shell's account menu, rendered bare, as the editor
// does it (frontend/src/components/AccountMenuItems.vue), so the same items
// fill the account button's popup and its place in the overflow menu: who
// is in, the colour scheme, the tools of the test setup (`gereedschap`),
// and leaving. `kop` adds the identity as the
// menu's header, which only the popup has room for; the menu leaves that
// region unpadded on purpose, so the container here carries the inset.
import { useColorScheme } from '@regelrecht/frontend-shared';
import WeergaveMenuItem from '@regelrecht/frontend-shared/components/WeergaveMenuItem.vue';

defineProps<{
  naam: string | null;
  afmeldTekst: string;
  kop?: boolean;
  /** What the account is here, under the name: the role and the cell it acts for. */
  onder?: string | null;
  /** Tools of the test setup, such as setting the clock, each opening its own sheet or dialog. */
  gereedschap?: { tekst: string; icoon?: string; destructief?: boolean; kies: () => void }[];
}>();
const emit = defineEmits<{ afmelden: [] }>();
const { colorScheme, setColorScheme } = useColorScheme();
</script>

<template>
  <nldd-container v-if="kop && naam" slot="header" padding-inline="16" padding-block="12">
    <nldd-identity :text="naam" :supporting-text="onder || undefined">
      <nldd-avatar slot="avatars" :name="naam" decorative></nldd-avatar>
    </nldd-identity>
  </nldd-container>
  <WeergaveMenuItem :waarde="colorScheme" @kies="setColorScheme" />
  <template v-if="gereedschap?.length">
    <nldd-menu-divider></nldd-menu-divider>
    <nldd-menu-item v-for="g in gereedschap" :key="g.tekst" :text="g.tekst" :icon="g.icoon" :destructive="g.destructief || undefined" @select="g.kies()"></nldd-menu-item>
  </template>
  <template v-if="naam">
    <nldd-menu-divider></nldd-menu-divider>
    <nldd-menu-item :text="afmeldTekst" icon="logout" @select="emit('afmelden')"></nldd-menu-item>
  </template>
</template>
