<script setup lang="ts">
/**
 * NoteCard — the inner rendering of a single note: comment (rich-text), author
 * identity, and a per-note action toolbar (edit / share / delete). Shared by the
 * Notities pane and the annotation-badge popover so both render identically.
 *
 * Emits edit/share/delete; the parent owns the actual actions (and their confirm
 * modals) and the group/span context an edit needs. The card renders only the
 * inner content — the caller wraps it in a card/container as needed.
 */
import { computed } from 'vue';
import type { Note } from '../composables/useNotes';

/** The part of a note body this card reads (W3C TextualBody or SpecificResource). */
interface NoteBody {
  value?: string;
  source?: string;
}

/** A note creator: the { id, name } shape, or a legacy plain string. */
type NoteCreator = { name?: string } | string;

const props = withDefaults(defineProps<{
  note: Note;
  // Whether the signed-in user has traject write access (needed to share).
  canEdit?: boolean;
  // A share is in flight — disables the share button.
  saving?: boolean;
}>(), {
  canEdit: false,
  saving: false,
});
defineEmits<{
  edit: [event: Event];
  share: [];
  delete: [];
}>();

// The note's primary body value: the comment, or a linked source.
const detailText = computed(() => {
  // Notes are not validated (see Note), so the body shape is assumed here.
  const noteBody = props.note?.body as NoteBody | NoteBody[] | null | undefined;
  const body = Array.isArray(noteBody) ? noteBody[0] : noteBody;
  return body?.value || body?.source || '';
});
// Author display name, tolerating the { id, name } shape and legacy strings.
const creatorName = computed(() => {
  const creator = props.note?.creator as NoteCreator | null | undefined;
  // `.name` on a legacy string is undefined, so it falls through to the string.
  return (creator as { name?: string } | null | undefined)?.name ?? creator ?? '';
});
// A draft lives in this browser's localStorage, so it is the user's by
// construction: deletable, and shareable when a traject is active. Committed
// notes are not __draft, so they carry no actions here.
const isDraft = computed(() => !!props.note?.__draft);
const canDelete = computed(() => isDraft.value);
const canShare = computed(() => isDraft.value && props.canEdit);
</script>

<template>
  <nldd-rich-text>
    <p>{{ detailText || '—' }}</p>
  </nldd-rich-text>
  <template v-if="creatorName">
    <nldd-spacer size="4"></nldd-spacer>
    <nldd-identity :text="creatorName"></nldd-identity>
  </template>
  <template v-if="canShare || canDelete">
    <nldd-spacer size="8"></nldd-spacer>
    <nldd-toolbar size="md" accessible-label="Notitie-acties">
      <nldd-toolbar-item v-if="canDelete" slot="start" label="Bewerken">
        <nldd-icon-button
          icon="edit"
          text="Bewerken"
          appearance="secondary"
          size="md"
          @click="$emit('edit', $event)"
        ></nldd-icon-button>
        <nldd-menu-item slot="overflow" icon="edit" text="Bewerken" @select="$emit('edit', $event)"></nldd-menu-item>
      </nldd-toolbar-item>
      <nldd-toolbar-item v-if="canShare" slot="start" label="Delen">
        <nldd-button
          text="Delen"
          appearance="secondary"
          size="md"
          :disabled="saving || undefined"
          @click="$emit('share')"
        ></nldd-button>
        <nldd-menu-item slot="overflow" icon="share" text="Delen" @select="$emit('share')"></nldd-menu-item>
      </nldd-toolbar-item>
      <nldd-toolbar-item v-if="canDelete" slot="end" label="Verwijderen">
        <nldd-icon-button
          icon="trash"
          text="Verwijderen"
          appearance="destructive"
          size="md"
          @click="$emit('delete')"
        ></nldd-icon-button>
        <nldd-menu-item slot="overflow" icon="trash" text="Verwijderen" @select="$emit('delete')"></nldd-menu-item>
      </nldd-toolbar-item>
    </nldd-toolbar>
  </template>
</template>
