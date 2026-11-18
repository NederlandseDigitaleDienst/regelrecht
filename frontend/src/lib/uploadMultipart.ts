/** What `uploadMultipart` resolves to. */
export type UploadResult =
  | { ok: true; json: unknown }
  | { ok: false; error: string; retryable: boolean };

/**
 * uploadMultipart - de gedeelde multipart-POST voor bestandsuploads
 * (werkdocument- en wet-upload). Eén plek voor de raw-fetch (bewust geen
 * Content-Type: de browser zet zelf de multipart-boundary), de
 * 404/405/501-classificatie ("endpoint bestaat niet op deze server, retry
 * helpt niet") en de `{ ok, error, retryable }`-resultaatvorm die
 * `useDocumentUpload` verwacht. Op succes draagt `json` de geparste
 * responsebody (of `null` bij een lege body) zodat de aanroeper er zijn
 * eigen velden (target_path, job_id) uit kan lezen.
 */
export async function uploadMultipart(url: string, file: Blob): Promise<UploadResult> {
  const form = new FormData();
  form.append('file', file);
  try {
    const res = await fetch(url, { method: 'POST', body: form });
    if (!res.ok) {
      const unsupported = res.status === 404 || res.status === 405 || res.status === 501;
      let text = '';
      try {
        text = await res.text();
      } catch {
        /* generieke melding hieronder */
      }
      const error = unsupported
        ? 'Uploaden wordt door de server nog niet ondersteund.'
        : (text || `Uploaden mislukt (foutcode ${res.status}).`);
      return { ok: false, error, retryable: !unsupported };
    }
    let json: unknown = null;
    try {
      json = await res.json();
    } catch {
      /* lege of niet-JSON-body: json blijft null */
    }
    return { ok: true, json };
  } catch (e) {
    // fetch rejects with an Error (TypeError on a network failure, AbortError).
    return { ok: false, error: (e as Error).message, retryable: true };
  }
}
