<script setup lang="ts">
/**
 * Compact row-actions control: a single "more" icon-button that opens a
 * menu with Edit + Delete. Replaces the inline "Bewerk" button + "—"
 * delete icon-button pair to save horizontal space in dense list rows
 * (machine-readable definitions / parameters / inputs / outputs /
 * actions).
 *
 * Emits `edit` and `delete`; the parent owns the actual handlers and the
 * delete-confirmation flow. The test-id props are forwarded onto the
 * matching menu-items so existing data-testid selectors keep working.
 */
withDefaults(defineProps<{
  // Accessible label / tooltip for the trigger (e.g. "Acties voor input bsn").
  accessibleLabel?: string;
  editTestid?: string;
  deleteTestid?: string;
}>(), {
  accessibleLabel: 'Acties',
  editTestid: undefined,
  deleteTestid: undefined,
});

defineEmits<{
  edit: [];
  delete: [];
}>();
</script>

<template>
  <!-- The menu sits in the button's popup slot: the button anchors and toggles
       it itself, so no per-instance id is needed. -->
  <nldd-icon-button
    icon="more"
    :text="accessibleLabel"
    tooltip-timing="never"
    appearance="neutral-tinted"
  >
    <nldd-menu slot="popup">
      <nldd-menu-item
        text="Bewerk"
        icon="edit"
        :data-testid="editTestid"
        @click.stop="$emit('edit')"
      ></nldd-menu-item>
      <nldd-menu-divider></nldd-menu-divider>
      <nldd-menu-item
        text="Verwijder"
        icon="delete"
        destructive
        :data-testid="deleteTestid"
        @click.stop="$emit('delete')"
      ></nldd-menu-item>
    </nldd-menu>
  </nldd-icon-button>
</template>
