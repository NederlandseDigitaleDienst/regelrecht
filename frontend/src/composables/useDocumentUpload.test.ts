// De bevestigingsstap in useDocumentUpload: met `confirm: true` mag het kiezen
// van een bestand nog niets versturen. Dat is de hele reden dat de stap bestaat
// - anders zou de inhoud al onderweg zijn voordat de gebruiker de vraag over AI
// heeft gezien.
import { describe, it, expect, vi } from 'vitest';
import { useDocumentUpload } from './useDocumentUpload';

/** Een change-event zoals het hidden file-input dat afvuurt. */
// Only `target.files` and `target.value` are read; cast once here.
function changeEvent(file: File | null): Event {
  return { target: { files: file ? [file] : [], value: 'c:\\fake' } } as unknown as Event;
}

// A stand-in: the composable only passes the file along.
const FILE = { name: 'rapport.docx' } as File;

describe('useDocumentUpload zonder bevestiging (wet-upload)', () => {
  it('uploadt direct bij het kiezen van een bestand', async () => {
    const uploadFn = vi.fn().mockResolvedValue({ ok: true });
    const onUploaded = vi.fn();
    const { onFileChange, pendingFile } = useDocumentUpload(uploadFn, onUploaded);

    await onFileChange(changeEvent(FILE));

    expect(uploadFn).toHaveBeenCalledWith(FILE, undefined);
    expect(onUploaded).toHaveBeenCalled();
    expect(pendingFile.value).toBeNull();
  });
});

describe('useDocumentUpload met bevestiging (werkdocument-upload)', () => {
  it('verstuurt niets tot er bevestigd is', async () => {
    const uploadFn = vi.fn().mockResolvedValue({ ok: true });
    const { onFileChange, pendingFile } = useDocumentUpload(uploadFn, null, { confirm: true });

    await onFileChange(changeEvent(FILE));

    expect(uploadFn).not.toHaveBeenCalled();
    expect(pendingFile.value).toStrictEqual(FILE);
  });

  it('geeft de keuze uit de dialoog door aan de upload', async () => {
    const uploadFn = vi.fn().mockResolvedValue({ ok: true });
    const onUploaded = vi.fn();
    const { onFileChange, confirmUpload, pendingFile } = useDocumentUpload(
      uploadFn,
      onUploaded,
      { confirm: true },
    );

    await onFileChange(changeEvent(FILE));
    await confirmUpload({ allowLlm: true });

    expect(uploadFn).toHaveBeenCalledWith(FILE, { allowLlm: true });
    expect(onUploaded).toHaveBeenCalled();
    expect(pendingFile.value).toBeNull();
  });

  it('annuleren laat het bestand vallen zonder iets te versturen', async () => {
    const uploadFn = vi.fn().mockResolvedValue({ ok: true });
    const { onFileChange, cancelUpload, confirmUpload, pendingFile } = useDocumentUpload(
      uploadFn,
      null,
      { confirm: true },
    );

    await onFileChange(changeEvent(FILE));
    cancelUpload();
    // Ook een bevestiging ná annuleren mag niets meer versturen.
    await confirmUpload({ allowLlm: true });

    expect(uploadFn).not.toHaveBeenCalled();
    expect(pendingFile.value).toBeNull();
  });

  it('meldt een mislukte upload zoals voorheen', async () => {
    const uploadFn = vi
      .fn()
      .mockResolvedValue({ ok: false, error: 'Alleen met AI', retryable: false });
    const { onFileChange, confirmUpload, uploadError, uploadRetryable } = useDocumentUpload(
      uploadFn,
      null,
      { confirm: true },
    );

    await onFileChange(changeEvent(FILE));
    await confirmUpload({ allowLlm: false });

    expect(uploadError.value).toBe('Alleen met AI');
    expect(uploadRetryable.value).toBe(false);
  });
});
