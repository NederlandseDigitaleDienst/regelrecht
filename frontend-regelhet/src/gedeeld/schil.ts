// The tabs of the shell (Schil.vue), declared by each side on its own route
// (`meta.tabs`), so the shell shows the sections of the side a screen belongs
// to without importing that side.
import { onUnmounted, shallowRef, watchEffect } from 'vue';
import type { RouteLocationNormalizedLoaded } from 'vue-router';

/** A section of one side of the app, as a tab of the shell. */
export interface Tab {
  /** The route name the tab opens, and is current on. */
  naam: string;
  tekst: string;
  pad: string;
  icoon: string;
  /** Other route names the tab is current on, such as a screen under it. */
  ook?: string[];
  /** A right the account needs to see the tab. */
  recht?: 'klok_zetten' | 'partijen_invoeren' | 'uitvoerder' | 'partij';
}

/** The primary action of a side, as a button in the shell's bar beside the account. */
export interface Actie {
  tekst: string;
  /** The hash the button opens, on whatever screen the visitor is on: the
   *  action opens a sheet over it, so it belongs to the address they are at
   *  rather than to one of its own. */
  hash: string;
  icoon?: string;
  /** Whether the action cannot be undone, which its menu item shows in red. */
  destructief?: boolean;
  /** What the coach mark says to someone who has not used the action yet. */
  uitleg?: { tekst: string; toelichting: string };
}

/**
 * A menu of actions a screen offers in the shell's bar beside the account,
 * such as the facts its account may record. Unlike [`Actie`] it depends on
 * what the screen has loaded, so the screen sets it ([`useMenuactie`]).
 */
export interface Menuactie {
  tekst: string;
  icoon?: string;
  /** The options; those with a `groep` stand under that title, after the rest. */
  opties: { tekst: string; groep?: string; kies: () => void }[];
}

/** The menu of the screen on view, if it offers one. */
export const menuactie = shallowRef<Menuactie | null>(null);

/** Offer `menu` in the shell's bar while the calling screen shows; none when it gives null or no options. */
export function useMenuactie(menu: () => Menuactie | null): void {
  let eigen: Menuactie | null = null;
  watchEffect(() => {
    const m = menu();
    eigen = m?.opties.length ? m : null;
    menuactie.value = eigen;
  });
  onUnmounted(() => {
    if (menuactie.value === eigen) menuactie.value = null;
  });
}

/** The site's name, which ends every page title. */
export const SITENAAM = 'RegelHet';

/** What the screen on view adds in front of its view in the page title, such as the record it shows. */
export const schermtitel = shallowRef<string | null>(null);

/** Put `titel` in front of the view in the page title while the calling screen shows. */
export function useSchermtitel(titel: () => string | null | undefined): void {
  let eigen: string | null = null;
  watchEffect(() => {
    eigen = titel() || null;
    schermtitel.value = eigen;
  });
  onUnmounted(() => {
    if (schermtitel.value === eigen) schermtitel.value = null;
  });
}

/**
 * The page title: what is on view from specific to general, then the site,
 * so a tab of the browser says which view it holds (`Kroniek · RegelHet`).
 * A part the one before it already says is left out.
 */
export function paginatitel(delen: (string | null | undefined)[]): string {
  const alle = [...delen.filter((d): d is string => !!d), SITENAAM];
  return alle.filter((d, i) => d !== alle[i - 1]).join(' · ');
}

/** The name of the view of a route without tabs, from the deepest route that names one. */
export function paginanaamVan(route: Pick<RouteLocationNormalizedLoaded, 'matched'>): string | null {
  return [...route.matched].reverse().find((r) => r.meta.paginanaam)?.meta.paginanaam ?? null;
}

/** The tabs of the side `route` belongs to, from the deepest route that declares them. */
export function tabsVan(route: Pick<RouteLocationNormalizedLoaded, 'matched'>): Tab[] {
  return [...route.matched].reverse().find((r) => r.meta.tabs)?.meta.tabs ?? [];
}

/** The name of the side `route` belongs to, shown in the bar where a side has no tabs. */
export function titelVan(route: Pick<RouteLocationNormalizedLoaded, 'matched'>): string | null {
  return [...route.matched].reverse().find((r) => r.meta.titel)?.meta.titel ?? null;
}

/** The primary action of the side `route` belongs to. */
export function actieVan(route: Pick<RouteLocationNormalizedLoaded, 'matched'>): Actie | null {
  return [...route.matched].reverse().find((r) => r.meta.actie)?.meta.actie ?? null;
}

/** The tools of the test setup the side of `route` offers to an account with `rechten`. */
export function gereedschapVan(route: Pick<RouteLocationNormalizedLoaded, 'matched'>, rechten: Tab['recht'][]): Actie[] {
  const alle = [...route.matched].reverse().find((r) => r.meta.gereedschap)?.meta.gereedschap ?? [];
  return alle.filter((g) => rechten.includes(g.recht)).map(({ recht: _, ...g }) => g);
}

/** Whether `tab` is the one the route `naam` belongs to. */
export const isHuidig = (tab: Tab, naam: unknown): boolean =>
  tab.naam === naam || (typeof naam === 'string' && !!tab.ook?.includes(naam));

declare module 'vue-router' {
  interface RouteMeta {
    /** The sections of the side this route belongs to. */
    tabs?: Tab[];
    /** The name of the side, for a side that has no tabs. */
    titel?: string;
    /** The name of the view in the page title, on a side without tabs that name it. */
    paginanaam?: string;
    /** The one action the side offers from its bar. */
    actie?: Actie;
    /** The tools of the test setup the side offers in the account menu, each for who has its right. */
    gereedschap?: (Actie & { recht: NonNullable<Tab['recht']> })[];
  }
}
