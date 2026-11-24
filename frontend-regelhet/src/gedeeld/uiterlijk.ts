// The accent colour of the screens, as the server names it
// (REGELHET_ACCENT_COLOR, GET /api/uiterlijk): a theme of the NLDD design
// system. The design system recolours itself through `data-nldd-theme` on
// <html>; the themes themselves come from '@nldd/design-system/themes', which
// start.ts loads. Without an accent, or without an answer, the screens keep
// the design system's own accent: the colour is no reason to stop a screen.
import { haal } from './http';

export interface Uiterlijk {
  /** A theme of the NLDD design system, or null for its own accent. */
  accentkleur: string | null;
}

/** Sets the theme `uiterlijk` names on `root`, or removes it. */
export function zetUiterlijk(uiterlijk: Uiterlijk, root: HTMLElement = document.documentElement): void {
  if (uiterlijk.accentkleur) root.dataset.nlddTheme = uiterlijk.accentkleur;
  else delete root.dataset.nlddTheme;
}

/** Asks the server how the screens look, and sets it. */
export async function laadUiterlijk(root: HTMLElement = document.documentElement): Promise<void> {
  try {
    zetUiterlijk(await haal<Uiterlijk>('/api/uiterlijk'), root);
  } catch (e) {
    console.warn('the accent colour could not be loaded; the screens keep the default', e);
  }
}
