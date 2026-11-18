import { ref } from 'vue';
import { apiFetch, apiFetchJson } from '../lib/apiFetch';
import type { TrajectDetail, TrajectInvite, TrajectMember } from './useTrajectDetail';

/** `POST /api/trajects/:id/members` (AddMemberResponse in trajects.rs). */
export interface AddMemberResponse {
  status: 'active' | 'pending';
  email: string;
}

export interface FailedInvite {
  email: string;
  message: string;
}

/**
 * Per-traject member + pending-invite state.
 *
 * Returns a fresh reactive state on every call so multiple dialogs (or
 * the same dialog reopened) don't leak previous data between trajects.
 * The component owning the dialog passes the traject id at load time;
 * the composable handles fetching, invite, role change, removal, and
 * leave.
 */
export function useTrajectMembers() {
  const members = ref<TrajectMember[]>([]);
  const pendingInvites = ref<TrajectInvite[]>([]);
  const callerRole = ref<string | null>(null);
  const trajectName = ref('');
  const loading = ref(false);
  const error = ref<unknown>(null);

  async function load(trajectId: string): Promise<void> {
    // Reset state before the await so a reopen against a different
    // traject can't briefly flash the previous traject's members.
    loading.value = true;
    error.value = null;
    members.value = [];
    pendingInvites.value = [];
    callerRole.value = null;
    trajectName.value = '';
    try {
      const body = await apiFetchJson<TrajectDetail>(`/api/trajects/${trajectId}`, {
        errorMessage: (status) => `Kon traject niet laden: ${status}`,
      });
      members.value = body.members || [];
      pendingInvites.value = body.pending_invites || [];
      callerRole.value = body.role || null;
      trajectName.value = body.name || '';
    } catch (e) {
      error.value = e;
    } finally {
      loading.value = false;
    }
  }

  async function invite(trajectId: string, email: string, role: string): Promise<AddMemberResponse> {
    const body = await apiFetchJson<AddMemberResponse>(`/api/trajects/${trajectId}/members`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ email, role }),
      // 400 from normalize_email/validate_role comes back with an empty
      // body; surface a specific message instead of "Uitnodigen mislukt: 400".
      errorMessage: (status, reason) =>
        reason ||
        (status === 400 ? 'Ongeldig e-mailadres of rol' : `Uitnodigen mislukt: ${status}`),
    });
    await load(trajectId);
    return body;
  }

  // Invite several addresses in one action: POST each (sequentially, so a rate
  // limit or one bad address doesn't abort the rest), collect per-address
  // outcomes, then reload the roster once. Never rejects — read `failed`.
  async function inviteMany(
    trajectId: string,
    emails: readonly string[],
    role: string,
  ): Promise<{ succeeded: string[]; failed: FailedInvite[] }> {
    const succeeded: string[] = [];
    const failed: FailedInvite[] = [];
    for (const email of emails) {
      try {
        const body = await apiFetchJson<AddMemberResponse>(`/api/trajects/${trajectId}/members`, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify({ email, role }),
          errorMessage: (status, reason) =>
            reason ||
            (status === 400 ? 'Ongeldig e-mailadres of rol' : `Mislukt: ${status}`),
        });
        succeeded.push(body.email || email);
      } catch (e) {
        // apiFetchJson rejects with an Error (ApiError or a network error).
        failed.push({ email, message: (e as Error).message || 'Uitnodigen mislukt' });
      }
    }
    await load(trajectId);
    return { succeeded, failed };
  }

  async function updateRole(trajectId: string, accountId: string, role: string): Promise<void> {
    await apiFetch(`/api/trajects/${trajectId}/members/${accountId}`, {
      method: 'PATCH',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ role }),
      // 409 is the backend's atomic "can't demote the last owner" guard - only
      // reachable when demoting an owner to contributor. Surface the workflow.
      errorMessage: (status, body) =>
        status === 409
          ? 'Een traject moet minstens één beheerder houden. Maak eerst een ander lid beheerder.'
          : body || `Rol wijzigen mislukt: ${status}`,
    });
    await load(trajectId);
  }

  async function removeMember(trajectId: string, accountId: string): Promise<void> {
    await apiFetch(`/api/trajects/${trajectId}/members/${accountId}`, {
      method: 'DELETE',
      errorMessage: (status, body) => body || `Verwijderen mislukt: ${status}`,
    });
    await load(trajectId);
  }

  async function removeInvite(trajectId: string, email: string): Promise<void> {
    await apiFetch(
      `/api/trajects/${trajectId}/invites/${encodeURIComponent(email)}`,
      {
        method: 'DELETE',
        errorMessage: (status, body) => body || `Uitnodiging intrekken mislukt: ${status}`,
      },
    );
    await load(trajectId);
  }

  return {
    members,
    pendingInvites,
    callerRole,
    trajectName,
    loading,
    error,
    load,
    invite,
    inviteMany,
    updateRole,
    removeMember,
    removeInvite,
  };
}
