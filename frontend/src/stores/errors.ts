import { computed, ref, toRaw } from 'vue';
import { defineStore } from 'pinia';
import type { UserError } from '../lib/userError';

/** An error with the heading it is shown under. */
export interface ReportedError extends UserError {
  title: string;
}

interface Report {
  /** The object the source handed in; the same object again is the same failure. */
  source: UserError;
  shown: ReportedError;
  dismissed: boolean;
  onDismiss?: () => void;
}

/**
 * The errors the editor currently has to tell the user about, and the sheet
 * with their technical details.
 *
 * A source reports its error under a key of its own and clears it when the
 * failure is over. The shell shows what is reported as one notification
 * (`ErrorNotification`): the error itself when there is one, a count when
 * there are more, so a run of failures does not stack up on screen. The
 * details sheet (`ErrorDetailsSheet`) lists them.
 */
export const useErrorsStore = defineStore('errors', () => {
  const reports = ref(new Map<string, Report>());

  /** What the notification shows: every reported error not yet dismissed. */
  const visible = computed<ReportedError[]>(() =>
    [...reports.value.values()].filter((r) => !r.dismissed).map((r) => r.shown),
  );

  /**
   * Report the error of one source. Reporting the same error object again
   * changes nothing, so a dismissed error stays away until the source has a
   * new one.
   */
  function report(key: string, title: string, error: UserError, onDismiss?: () => void): void {
    // Raw on both sides: the map hands back a reactive proxy, and so may the caller.
    const previous = reports.value.get(key);
    if (previous && toRaw(previous.source) === toRaw(error)) return;
    reports.value.set(key, { source: error, shown: { title, ...error }, dismissed: false, onDismiss });
  }

  /** The source's failure is over. */
  function clear(key: string): void {
    reports.value.delete(key);
  }

  /** The user dismissed the notification, with everything it stood for. */
  function dismissVisible(): void {
    for (const r of reports.value.values()) {
      if (r.dismissed) continue;
      r.dismissed = true;
      r.onDismiss?.();
    }
  }

  const detailsOpen = ref(false);
  // Kept after closing, so the text does not vanish during the close animation.
  const detailsOf = ref<ReportedError[]>([]);

  function showDetails(errors: ReportedError[]): void {
    detailsOf.value = errors;
    detailsOpen.value = true;
  }

  function closeDetails(): void {
    detailsOpen.value = false;
  }

  return { visible, report, clear, dismissVisible, detailsOpen, detailsOf, showDetails, closeDetails };
});
