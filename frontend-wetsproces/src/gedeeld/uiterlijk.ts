// The accent colour of the screens, as the server names it
// (WETSPROCES_ACCENT_COLOR, GET /api/uiterlijk): a theme of the NLDD design
// system. The design system recolours itself through `data-nldd-theme` on
// <html>; the themes themselves come from '@nldd/design-system/themes', which
// start.ts loads. Without an accent, or without an answer, the screens keep
// the design system's own accent: the colour is no reason to stop a screen.
// The same answer names the organisation: on a process that belongs to one,
// its name and its front on this side, which the shell shows as the link back;
// on the front itself, its name and logo.
import { ref } from 'vue';
import { haal } from './http';

/** The organisation in front of this server, as `GET /api/uiterlijk` names it. */
export interface UiterlijkOrganisatie {
  naam: string;
  /** The front on this side, for the link back; absent on the front itself. */
  url?: string | null;
  logo?: string | null;
  /** Whether this server is the organisation's front itself. */
  voorkant?: boolean;
}

export interface Uiterlijk {
  /** A theme of the NLDD design system, or null for its own accent. */
  accentkleur: string | null;
  /** The organisation, or null (or absent) when the process stands on its own. */
  organisatie?: UiterlijkOrganisatie | null;
}

/** The organisation the server named, for the shell. */
export const organisatie = ref<UiterlijkOrganisatie | null>(null);

/** Sets the theme `uiterlijk` names on `root`, or removes it. */
export function zetUiterlijk(uiterlijk: Uiterlijk, root: HTMLElement = document.documentElement): void {
  if (uiterlijk.accentkleur) root.dataset.nlddTheme = uiterlijk.accentkleur;
  else delete root.dataset.nlddTheme;
  organisatie.value = uiterlijk.organisatie ?? null;
}

/** Asks the server how the screens look, and sets it. */
export async function laadUiterlijk(root: HTMLElement = document.documentElement): Promise<void> {
  try {
    zetUiterlijk(await haal<Uiterlijk>('/api/uiterlijk'), root);
  } catch (e) {
    console.warn('the accent colour could not be loaded; the screens keep the default', e);
  }
}
