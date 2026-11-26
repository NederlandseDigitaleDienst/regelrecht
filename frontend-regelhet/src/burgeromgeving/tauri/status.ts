// What the app knows of the core, for the whole app: asked on every
// navigation and after an activation. It holds no key, token or
// credential; the core keeps those (packages/regelhet-app).
import { ref } from 'vue';
import type { AppStatus, Invoke } from './kern';

/** The core's status, or null before it is asked. */
export const appStatus = ref<AppStatus | null>(null);

/** Ask the core how it stands. */
export async function laadStatus(invoke: Invoke): Promise<AppStatus> {
  appStatus.value = await invoke<AppStatus>('status');
  return appStatus.value;
}

/** In words, where the app key lives: the screens say it as it is. */
export function opslagTekst(s: AppStatus): string {
  switch (s.key_storage.kind) {
    case 'memory':
      return 'De sleutel van deze app staat alleen in het geheugen, in software. Hij verdwijnt als de app sluit. Geen beveiligde hardware beschermt hem.';
    case 'file':
      return 'De sleutel van deze app staat in een bestand op dit apparaat, in software. Geen beveiligde hardware beschermt hem.';
    case 'hardware':
      return `De sleutel van deze app staat in de beveiligde hardware van dit apparaat (${s.key_storage.platform}) en verlaat die niet.`;
    case 'browser':
      return 'De sleutel van deze app staat in de opslag van deze browser. De browser geeft hem niet vrij, maar het is software: geen beveiligde hardware beschermt hem. Wie deze browser gebruikt, kan de app laten ondertekenen.';
  }
}
