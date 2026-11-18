/**
 * uploadFormats - hoe een gekozen werkdocument-upload behandeld wordt, zodat de
 * bevestigingsdialoog de gebruiker concreet kan vertellen of er AI aan te pas
 * komt.
 *
 * De indeling komt van de server (`/api/document-upload-formats`), die 'm op
 * zijn beurt afleidt uit de convertertabel in de pipeline. Bewust géén
 * hardgecodeerde lijst hier: een tweede lijst zou onvermijdelijk gaan afwijken
 * van wat de conversie echt kan, en dan belooft de UI iets dat de backend niet
 * waarmaakt.
 *
 * De classificatie zelf is puur ('welke van de drie categorieën'), zodat een
 * test 'm zonder fetch kan uitoefenen; het ophalen zit in `loadUploadFormats`,
 * dat één keer per pagina fetcht en het resultaat deelt.
 */
import { apiFetchJson } from './apiFetch';

/** Direct opslaan, geen conversie: markdown ís al het doelformaat. */
export const PASSTHROUGH = 'passthrough';
/** Om te zetten met een tool (pandoc/pdftotext), zonder taalmodel. */
export const DETERMINISTIC = 'deterministic';
/** Alleen om te zetten met een taalmodel. */
export const LLM_ONLY = 'llm-only';
/** De server-indeling is niet beschikbaar; we weten het simpelweg niet. */
export const UNKNOWN = 'unknown';

export type UploadCategory =
  | typeof PASSTHROUGH
  | typeof DETERMINISTIC
  | typeof LLM_ONLY
  | typeof UNKNOWN;

/** The `/api/document-upload-formats` response: extensions per category. */
export interface UploadFormats {
  passthrough?: string[];
  deterministic?: string[];
}

let formatsPromise: Promise<UploadFormats | null> | null = null;

/**
 * Haal de formaatindeling op (één keer per pagina, daarna uit de cache).
 * Faalt de call, dan is het resultaat `null` — de aanroeper valt dan terug op
 * {@link UNKNOWN} in plaats van te gokken.
 */
export async function loadUploadFormats(): Promise<UploadFormats | null> {
  if (!formatsPromise) {
    formatsPromise = apiFetchJson<UploadFormats>('/api/document-upload-formats', {
      errorMessage: (status) => `HTTP ${status}`,
    }).catch((e) => {
      console.warn('Uploadformaten niet geladen:', e.message);
      // Niet cachen als mislukking-voor-altijd: een volgende poging (de
      // gebruiker kiest zo meteen een ander bestand) mag het opnieuw proberen.
      formatsPromise = null;
      return null;
    });
  }
  return formatsPromise;
}

/** De kleingeschreven extensie van een bestandsnaam, of '' als die ontbreekt. */
export function extensionOf(filename: string | null | undefined): string {
  const name = String(filename ?? '');
  const dot = name.lastIndexOf('.');
  // Een naam die met een punt begint (`.gitignore`) heeft geen extensie maar
  // een naam - net als op de server, waar `Path::extension` hetzelfde oordeelt.
  if (dot <= 0) return '';
  return name.slice(dot + 1).toLowerCase();
}

/**
 * In welke categorie valt `filename`, gegeven de serverindeling `formats`?
 * Zonder indeling (`null`, mislukte fetch) is het antwoord {@link UNKNOWN}.
 */
export function classifyUpload(
  filename: string | null | undefined,
  formats: UploadFormats | null | undefined,
): UploadCategory {
  if (!formats) return UNKNOWN;
  const ext = extensionOf(filename);
  if (formats.passthrough?.includes(ext)) return PASSTHROUGH;
  if (formats.deterministic?.includes(ext)) return DETERMINISTIC;
  return LLM_ONLY;
}
