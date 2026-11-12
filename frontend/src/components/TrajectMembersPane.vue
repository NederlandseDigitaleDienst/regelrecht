<script setup lang="ts">
// Traject members as a Home main-pane view. The member list + pending invites
// live here; "Uitnodigen" opens the shared InviteMembersSheet (via useAddActions),
// which also opens from the universal "+" in the header. Loads on mount, on
// traject change, and whenever an invite elsewhere signals a change.
import { computed, onMounted, ref, watch } from 'vue';
import type { NLDDModalDialog } from '@nldd/design-system/modal-dialog';
import { useTrajectMembers } from '../composables/useTrajectMembers';
import type { TrajectInvite, TrajectMember } from '../composables/useTrajectDetail';
import { useAddActions } from '../composables/useAddActions';
import { paneChromeVisible } from '../constants';

const props = withDefaults(
  defineProps<{
    /** Traject to manage (UUID id). */
    trajectId?: string | null;
  }>(),
  { trajectId: null },
);

const {
  members,
  pendingInvites,
  callerRole,
  loading,
  error: loadError,
  load,
  updateRole,
  removeMember,
  removeInvite,
} = useTrajectMembers();

const { triggerInviteMembers, membersChanged } = useAddActions();

function reload(): void {
  if (props.trajectId) load(props.trajectId);
}
onMounted(reload);
watch(() => props.trajectId, reload);
// De InviteMembersSheet (een andere useTrajectMembers-instance) bumpt dit na een
// geslaagde invite; herladen zodat de nieuwe openstaande uitnodiging verschijnt.
watch(membersChanged, reload);

const isOwner = computed(() => callerRole.value === 'owner');
function roleLabel(role: string): string {
  return role === 'owner' ? 'Beheerder' : 'Bijdrager';
}

const ownerCount = computed(() => members.value.filter((m) => m.role === 'owner').length);
function canManageMember(m: TrajectMember): boolean {
  return isOwner.value && (m.role !== 'owner' || ownerCount.value > 1);
}

// Row-level busy + error indicators keyed by member account_id / invite email.
const rowBusy = ref(new Set<string>());
const rowError = ref(new Map<string, string>());
function markBusy(key: string, busy: boolean): void {
  const next = new Set(rowBusy.value);
  if (busy) next.add(key);
  else next.delete(key);
  rowBusy.value = next;
}
function setRowError(key: string, message: string | null): void {
  const next = new Map(rowError.value);
  if (message) next.set(key, message);
  else next.delete(key);
  rowError.value = next;
}

// The row actions below act on the last loaded list, which stays on screen when
// the traject disappears (trajectId goes to null); then they do nothing.
async function changeMemberRole(member: TrajectMember, newRole: string): Promise<void> {
  const trajectId = props.trajectId;
  if (member.role === newRole || !trajectId) return;
  setRowError(member.account_id, null);
  markBusy(member.account_id, true);
  try {
    await updateRole(trajectId, member.account_id, newRole);
  } catch (e) {
    setRowError(member.account_id, (e as Error).message || 'Wijzigen mislukt');
  } finally {
    markBusy(member.account_id, false);
  }
}

async function clickRemoveMember(member: TrajectMember): Promise<void> {
  const trajectId = props.trajectId;
  if (!trajectId) return;
  setRowError(member.account_id, null);
  markBusy(member.account_id, true);
  try {
    await removeMember(trajectId, member.account_id);
  } catch (e) {
    setRowError(member.account_id, (e as Error).message || 'Verwijderen mislukt');
  } finally {
    markBusy(member.account_id, false);
  }
}

async function clickRemoveInvite(inv: TrajectInvite): Promise<void> {
  const trajectId = props.trajectId;
  if (!trajectId) return;
  setRowError(inv.email, null);
  markBusy(inv.email, true);
  try {
    await removeInvite(trajectId, inv.email);
  } catch (e) {
    setRowError(inv.email, (e as Error).message || 'Intrekken mislukt');
  } finally {
    markBusy(inv.email, false);
  }
}

// Retracting an invite is destructive, so confirm it in a modal that names the
// address before the actual removeInvite runs.
const confirmDialogEl = ref<NLDDModalDialog | null>(null);
const confirmInvite = ref<TrajectInvite | null>(null);

function askRemoveInvite(inv: TrajectInvite): void {
  confirmInvite.value = inv;
  confirmDialogEl.value?.show?.();
}
function cancelRemoveInvite(): void {
  confirmDialogEl.value?.hide?.();
}
async function confirmRemoveInvite(): Promise<void> {
  const inv = confirmInvite.value;
  confirmDialogEl.value?.hide?.();
  if (inv) await clickRemoveInvite(inv);
}

// `loadError` holds whatever load() caught; read its message the way the template did.
function errorMessage(e: unknown): string | undefined {
  return (e as { message?: string } | null)?.message;
}
</script>

<template>
  <nldd-simple-section>
    <nldd-title v-if="paneChromeVisible(loading)" id="instellingen-pane-titel" size="display-3"><h3>Leden</h3></nldd-title>
    <nldd-spacer v-if="paneChromeVisible(loading)" size="16"></nldd-spacer>
    <nldd-toolbar v-if="isOwner && paneChromeVisible(loading)" accessible-label="Ledenacties">
      <nldd-toolbar-item slot="start">
        <nldd-button appearance="secondary" size="md" start-icon="plus-small" text="Uitnodigen" @click="triggerInviteMembers"></nldd-button>
        <nldd-menu-item slot="overflow" icon="plus-small" text="Uitnodigen" @select="triggerInviteMembers"></nldd-menu-item>
      </nldd-toolbar-item>
    </nldd-toolbar>
    <nldd-spacer v-if="isOwner && paneChromeVisible(loading)" size="16"></nldd-spacer>

    <nldd-activity-indicator v-if="loading" text="Leden laden" show-text></nldd-activity-indicator>
    <nldd-inline-dialog v-else-if="loadError" variant="alert" text="Leden niet geladen" :supporting-text="errorMessage(loadError)"></nldd-inline-dialog>
    <template v-else>
    <nldd-list appearance="box-tinted">
      <template v-for="m in members" :key="m.account_id">
        <nldd-list-item size="md">
          <nldd-icon-cell size="24"><nldd-icon icon="user"></nldd-icon></nldd-icon-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell :supporting-text="m.name ? m.email : null">
            <span class="member-name">
              <span>{{ m.name || m.email }}</span>
              <nldd-tag size="sm" :color="m.role === 'owner' ? 'accent' : 'neutral'" :text="roleLabel(m.role)"></nldd-tag>
            </span>
          </nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-cell v-if="canManageMember(m)">
            <nldd-icon-button
              icon="more"
              text="Meer acties"
              tooltip-timing="never"
              :disabled="rowBusy.has(m.account_id) || undefined"
            >
              <nldd-menu slot="popup">
                <nldd-menu-item
                  v-if="m.role === 'owner'"
                  text="Maak bijdrager"
                  @select="changeMemberRole(m, 'contributor')"
                ></nldd-menu-item>
                <template v-else>
                  <nldd-menu-item text="Maak beheerder" @select="changeMemberRole(m, 'owner')"></nldd-menu-item>
                  <nldd-menu-divider></nldd-menu-divider>
                  <nldd-menu-item text="Verwijder lid" destructive @select="clickRemoveMember(m)"></nldd-menu-item>
                </template>
              </nldd-menu>
            </nldd-icon-button>
          </nldd-cell>
        </nldd-list-item>
        <nldd-container v-if="rowError.get(m.account_id)" padding-top="8">
          <nldd-text size="sm" color="critical">{{ rowError.get(m.account_id) }}</nldd-text>
        </nldd-container>
      </template>
      <!-- Pending invites live in the same list so a long roster stays filterable
           instead of scrolling them out of view in a separate section below. -->
      <template v-for="inv in pendingInvites" :key="inv.email">
        <nldd-list-item size="md">
          <nldd-icon-cell size="24"><nldd-icon icon="user"></nldd-icon></nldd-icon-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-text-cell supporting-text="Openstaande uitnodiging. Wacht op eerste login voor activatie.">
            <span class="member-name">
              <span>{{ inv.email }}</span>
              <nldd-tag size="sm" :color="inv.role === 'owner' ? 'accent' : 'neutral'" :text="roleLabel(inv.role)"></nldd-tag>
            </span>
          </nldd-text-cell>
          <nldd-spacer-cell size="8"></nldd-spacer-cell>
          <nldd-cell v-if="isOwner">
            <nldd-button
              appearance="destructive"
              size="sm"
              text="Intrekken"
              :disabled="rowBusy.has(inv.email) || undefined"
              @click="askRemoveInvite(inv)"
            ></nldd-button>
          </nldd-cell>
        </nldd-list-item>
        <nldd-container v-if="rowError.get(inv.email)" padding-top="8">
          <nldd-text size="sm" color="critical">{{ rowError.get(inv.email) }}</nldd-text>
        </nldd-container>
      </template>
    </nldd-list>
    </template>
  </nldd-simple-section>

  <!-- Confirm dialog for retracting a pending invite. -->
  <Teleport to="body">
    <nldd-modal-dialog
      ref="confirmDialogEl"
      :text="confirmInvite ? `Uitnodiging voor ${confirmInvite.email} intrekken?` : ''"
      supporting-text="Je kunt deze persoon op een later moment weer uitnodigen."
    >
      <nldd-button slot="actions" appearance="primary" size="md" width="full" text="Behoud uitnodiging" @click="cancelRemoveInvite"></nldd-button>
      <nldd-button slot="actions" appearance="destructive" size="md" width="full" text="Trek uitnodiging in" @click="confirmRemoveInvite"></nldd-button>
    </nldd-modal-dialog>
  </Teleport>
</template>

<style scoped>
.member-name {
  display: inline-flex;
  align-items: center;
  gap: 8px;
}
</style>
