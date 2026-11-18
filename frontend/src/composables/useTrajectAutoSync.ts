/**
 * Automatic "Bijwerken": while a traject is open, keep its branch level with
 * the working branch it was cut from, without anyone pressing a button.
 *
 * Every AUTO_SYNC_MS (and when the tab becomes visible again) the editor asks
 * how the two branches relate. When the working branch changed files the
 * traject lacks, it brings them in: a fast-forward if the traject has no work
 * of its own, otherwise a regular git three-way merge on GitHub. That merge
 * resolves everything that does not touch the same lines, also inside one
 * file. Only a real conflict (same lines on both sides) stays behind as a
 * notice; git cannot resolve that without throwing one side's work away.
 *
 * After a conflict the loop does not keep retrying the same merge: it waits
 * until either branch changes (a new commit, or the conflict resolved in a
 * clone) before it tries again.
 *
 * State is module-level and follows the active traject: AppShell starts and
 * stops the loop, and any component reads the same state.
 */
import { ref } from 'vue';
import { useCorpus } from '../corpus';
import {
  canSync,
  fetchBaseStatus,
  proposeToBase,
  syncBase,
  type BaseStatus,
} from '../lib/trajectSync';
import type { RequestError } from '../lib/apiFetch';

export const AUTO_SYNC_MS = 3 * 60 * 1000;
// A tab that comes back into view checks right away, unless it checked
// within this window.
const VISIBLE_RECHECK_MS = 60 * 1000;

export interface SyncNotice {
  text: string;
  url?: string;
  auto?: boolean;
}

const trajectRef = ref<string | null>(null);
const status = ref<BaseStatus | null>(null);
/** A success message to show once. */
const notice = ref<SyncNotice | null>(null);
/** Message of the last failed sync or proposal (conflict included). */
const error = ref<string | null>(null);
const busy = ref<'sync' | 'propose' | null>(null);
/** Bumped after every sync that moved the traject branch. */
const syncCount = ref(0);

let timer: ReturnType<typeof setInterval> | null = null;
let lastCheck = 0;
// Divergence the last conflict happened on; no automatic retry until it changes.
let conflictSignature: string | null = null;
// A member without write access gets 403 on sync; stop trying for this traject.
let autoDisabled = false;

const statusOf = (e: unknown) => (e as RequestError | null)?.status;
const messageOf = (e: unknown) => (e instanceof Error ? e.message : String(e));

function signature(s: BaseStatus): string {
  return JSON.stringify([s.behind_by, s.ahead_by, s.base_files, s.traject_files]);
}

async function refresh(): Promise<BaseStatus | null> {
  const current = trajectRef.value;
  if (!current) return null;
  lastCheck = Date.now();
  try {
    const next = await fetchBaseStatus(current);
    if (trajectRef.value !== current) return null;
    status.value = next;
    return next;
  } catch (e) {
    if (trajectRef.value !== current) return null;
    status.value = null;
    // 409: central corpus, no GitHub working branch, or no GitHub link (the
    // backend turns a missing link into 409 on purpose, so this background
    // call never starts the connect flow). 404: a branch is gone.
    const code = statusOf(e);
    if (code !== 404 && code !== 409) console.warn('base-status failed:', e);
    return null;
  }
}

async function runSync({ auto }: { auto: boolean }): Promise<void> {
  const current = trajectRef.value;
  if (!current) return;
  busy.value = 'sync';
  if (!auto) {
    error.value = null;
    notice.value = null;
  }
  try {
    const res = await syncBase(current, { auto });
    if (trajectRef.value !== current) return;
    conflictSignature = null;
    error.value = null;
    if (res.sha) {
      useCorpus().forTraject(current).invalidate();
      syncCount.value += 1;
      notice.value = {
        auto,
        text: auto
          ? 'Nieuwe wijzigingen van de werkbranch zijn automatisch in dit traject gezet.'
          : res.outcome === 'fast_forward'
            ? 'Het traject staat weer gelijk met de werkbranch.'
            : 'Werkbranch is in het traject samengevoegd.',
      };
    } else if (!auto) {
      notice.value = { text: 'Het traject was al bij.' };
    }
  } catch (e) {
    if (trajectRef.value !== current) return;
    if (auto && statusOf(e) === 403) {
      autoDisabled = true;
      return;
    }
    if (statusOf(e) === 409 && status.value) conflictSignature = signature(status.value);
    error.value = messageOf(e);
  } finally {
    if (trajectRef.value === current) busy.value = null;
  }
  await refresh();
}

/** One round of the loop: look, and bring in what can be brought in. */
export async function tick(): Promise<void> {
  if (busy.value) return;
  const s = await refresh();
  if (!s || autoDisabled || !canSync(s)) return;
  if (conflictSignature && conflictSignature === signature(s)) return;
  await runSync({ auto: true });
}

function onVisibility(): void {
  if (document.visibilityState === 'visible' && Date.now() - lastCheck > VISIBLE_RECHECK_MS) {
    void tick();
  }
}

export function stopAutoSync(): void {
  if (timer) clearInterval(timer);
  timer = null;
  if (typeof document !== 'undefined') document.removeEventListener('visibilitychange', onVisibility);
  trajectRef.value = null;
  status.value = null;
  notice.value = null;
  error.value = null;
  busy.value = null;
  conflictSignature = null;
  autoDisabled = false;
}

/** Start (or restart, for another traject) the loop. `null` stops it. */
export function startAutoSync(next: string | null): void {
  if (next === trajectRef.value && timer) return;
  stopAutoSync();
  if (!next) return;
  trajectRef.value = next;
  timer = setInterval(() => {
    if (typeof document === 'undefined' || document.visibilityState === 'visible') void tick();
  }, AUTO_SYNC_MS);
  if (typeof document !== 'undefined') document.addEventListener('visibilitychange', onVisibility);
  void tick();
}

async function propose(): Promise<void> {
  const current = trajectRef.value;
  if (!current) return;
  busy.value = 'propose';
  error.value = null;
  notice.value = null;
  try {
    const pr = await proposeToBase(current);
    notice.value = { text: `Pull request #${pr.number} staat klaar.`, url: pr.url };
  } catch (e) {
    error.value = messageOf(e);
  } finally {
    busy.value = null;
  }
}

export function useTrajectAutoSync() {
  return {
    trajectRef,
    status,
    notice,
    error,
    busy,
    syncCount,
    syncNow: () => runSync({ auto: false }),
    propose,
    dismissNotice: () => {
      notice.value = null;
    },
    dismissError: () => {
      error.value = null;
    },
  };
}
