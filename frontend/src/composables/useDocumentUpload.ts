/**
 * useDocumentUpload - the shared "hidden native file input + trigger" wiring for
 * de uploadknoppen (werkdocument én wet), used by both the launcher sheet and
 * the standalone page. No NLDD file-upload component exists, so the picker is a
 * hidden `<input type="file">`; this keeps that one bit of non-design-system
 * plumbing in a single place.
 *
 * Optioneel zit er een bevestigingsstap tussen het kiezen en het uploaden
 * (`confirm: true`). Die stap bestaat voor de werkdocument-upload, waar de
 * gebruiker moet kunnen zien én kiezen of er een taalmodel aan te pas komt. Hij
 * komt bewust ná de bestandskeuze: pas dan is de extensie bekend en kan de
 * vraag concreet gesteld worden in plaats van in het algemeen. De wet-upload
 * laat 'm uit — daar bestaat geen conversie zonder AI, dus zou een vinkje een
 * schijnkeuze zijn.
 *
 * @param {(file: File, options?: object) => Promise<{ ok: boolean }>} uploadFn
 *   performs the upload; krijgt de keuzes uit de bevestigingsstap als tweede
 *   argument (bijv. `{ allowLlm: true }`).
 * @param {(result: object) => void} [onUploaded]  called after a successful upload
 *   (e.g. start polling) with the upload's result, so the consumer can act on
 *   `targetPath` - the path the conversion will write, which only the upload
 *   response knows.
 * @param {{ confirm?: boolean }} [options]  `confirm: true` houdt de upload vast
 *   in `pendingFile` totdat de aanroeper `confirmUpload()` of `cancelUpload()`
 *   aanroept.
 */
import { ref } from 'vue';

/** What useDocumentUpload reads from an upload's result. */
export interface UploadOutcome {
  ok: boolean;
  error?: string | null;
  retryable?: boolean;
}

export function useDocumentUpload<R extends UploadOutcome, O = unknown>(
  uploadFn: (file: File, options?: O) => Promise<R | null | undefined>,
  onUploaded?: ((result: R) => void) | null,
  { confirm = false }: { confirm?: boolean } = {},
) {
  const fileInput = ref<HTMLInputElement | null>(null);
  // Surfaced to the consumer so a failed upload (400/413/503/network) is shown,
  // not silently swallowed when the file picker closes.
  const uploadError = ref<string | null>(null);
  // Whether an "opnieuw proberen" retry makes sense. False for a permanent
  // server-side gap (e.g. the upload endpoint isn't supported yet), so the
  // consumer can drop its retry action.
  const uploadRetryable = ref(true);
  // Het gekozen bestand dat op bevestiging wacht; `null` zolang er niets
  // openstaat. De aanroeper rendert hierop zijn bevestigingsdialoog.
  const pendingFile = ref<File | null>(null);

  function onUpload(): void {
    fileInput.value?.click();
  }

  async function startUpload(file: File, options?: O): Promise<void> {
    const result = await uploadFn(file, options);
    if (result?.ok) {
      if (onUploaded) onUploaded(result);
    } else {
      // Set retryability before the message so a consumer watching the error
      // reads the matching value. A result without `retryable` defaults to true.
      uploadRetryable.value = result?.retryable !== false;
      uploadError.value = result?.error || 'Uploaden mislukt.';
    }
  }

  async function onFileChange(e: Event): Promise<void> {
    // The change event of the hidden file input.
    const input = e.target as HTMLInputElement;
    const file = input.files?.[0];
    // Reset the input so re-picking the same file fires `change` again.
    input.value = '';
    if (!file) return;
    uploadError.value = null;
    uploadRetryable.value = true;
    if (confirm) {
      pendingFile.value = file;
      return;
    }
    await startUpload(file);
  }

  /** Bevestig de wachtende upload, met de keuzes uit de dialoog. */
  async function confirmUpload(options?: O): Promise<void> {
    const file = pendingFile.value;
    // Meteen leegmaken: de dialoog sluit op dit signaal, en een tweede klik op
    // "Uploaden" mag niet hetzelfde bestand nogmaals versturen.
    pendingFile.value = null;
    if (!file) return;
    await startUpload(file, options);
  }

  /** Laat de wachtende upload vallen; er is niets verstuurd. */
  function cancelUpload(): void {
    pendingFile.value = null;
  }

  return {
    fileInput,
    uploadError,
    uploadRetryable,
    pendingFile,
    onUpload,
    onFileChange,
    confirmUpload,
    cancelUpload,
  };
}
