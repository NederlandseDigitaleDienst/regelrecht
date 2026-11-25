// The accent colour of the screens, as the server names it
// (REGELHET_ACCENT_COLOR, GET /api/uiterlijk): a theme of the NLDD design
// system. The design system recolours itself through `data-nldd-theme` on
// <html>; the themes themselves come from '@nldd/design-system/themes', which
// start.ts loads. Without an accent, or without an answer, the screens wear
// `donkerblauw` (STANDAARD_ACCENTKLEUR), not the design system's own green:
// this is the one place that default lives, and the colour is no reason to
// stop a screen.
// The same answer names the title of the bar (REGELHET_TITLE, or else the
// case's own `titel`), which the shell shows on every side of a process.
// It also names the organisation: on a process that belongs to one,
// its name and its front on this side, which the shell shows as the link back;
// on the front itself, its name and logo.
import { ref } from 'vue';
import { haal } from './http';

/** The organisation in front of this server, as `GET /api/uiterlijk` names it. */
/** The theme the screens wear when the server names none, or does not answer. */
export const STANDAARD_ACCENTKLEUR = 'donkerblauw';

export interface UiterlijkOrganisatie {
  naam: string;
  /** The front on this side, for the link back; absent on the front itself. */
  url?: string | null;
  logo?: string | null;
  /** Whether this server is the organisation's front itself. */
  voorkant?: boolean;
}

export interface Uiterlijk {
  /** A theme of the NLDD design system, or null for STANDAARD_ACCENTKLEUR. */
  accentkleur: string | null;
  /** The title of the bar; absent on the front of an organisation, which carries its name. */
  titel?: string | null;
  /** The organisation, or null (or absent) when the process stands on its own. */
  organisatie?: UiterlijkOrganisatie | null;
}

/** The organisation the server named, for the shell. */
export const organisatie = ref<UiterlijkOrganisatie | null>(null);

/** The title of the bar the server named, for the shell. */
export const titel = ref<string | null>(null);

/** Sets the theme `uiterlijk` names on `root`, or else the default. */
export function zetUiterlijk(uiterlijk: Uiterlijk, root: HTMLElement = document.documentElement): void {
  root.dataset.nlddTheme = uiterlijk.accentkleur || STANDAARD_ACCENTKLEUR;
  organisatie.value = uiterlijk.organisatie ?? null;
  titel.value = uiterlijk.titel ?? null;
}

/** The latest request for the look: an earlier one that answers later counts for nothing. */
let vraag = 0;

/**
 * Asks the server how the screens look, and sets it. The citizen's app asks
 * again for every procedure it is pointed at, so only the answer to the last
 * request counts, and a failed one puts back the default title and
 * organisation as well, not only the colour.
 */
export async function laadUiterlijk(root: HTMLElement = document.documentElement): Promise<void> {
  const deze = ++vraag;
  try {
    const uiterlijk = await haal<Uiterlijk>('/api/uiterlijk');
    if (deze === vraag) zetUiterlijk(uiterlijk, root);
  } catch (e) {
    if (deze !== vraag) return;
    zetUiterlijk({ accentkleur: null }, root);
    console.warn('the accent colour could not be loaded; the screens wear the default', e);
  }
}
