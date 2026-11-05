<script setup lang="ts">
import { useAddActions } from '../composables/useAddActions';

// The items of the shell's "Nieuw" menu, rendered bare so the same items fill
// both the button's popup menu and its alternative in the toolbar's overflow
// menu. They act on `select`: the toolbar copies an overflow item into its menu
// and sends `select` on to the original, never `click`.
const { triggerAddLaw, triggerNewWerkdoc, triggerUploadWerkdoc, triggerInviteMembers } = useAddActions();
</script>

<template>
  <nldd-menu-item icon="new-book" text="Regelwerk toevoegen…" @select="triggerAddLaw"></nldd-menu-item>
  <nldd-menu-item icon="new-text-document" text="Werkdocument toevoegen">
    <nldd-menu>
      <nldd-menu-item icon="new-text-document" text="Nieuw document" @select="triggerNewWerkdoc"></nldd-menu-item>
      <nldd-menu-item icon="upload-to-cloud" text="Document uploaden…" @select="triggerUploadWerkdoc"></nldd-menu-item>
    </nldd-menu>
  </nldd-menu-item>
  <nldd-menu-item icon="add-user" text="Leden uitnodigen…" @select="triggerInviteMembers"></nldd-menu-item>
</template>
