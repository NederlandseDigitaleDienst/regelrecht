// The tabs of the shell (Schil.vue), declared by each side on its own route
// (`meta.tabs`), so the shell shows the sections of the side a screen belongs
// to without importing that side.
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
  recht?: 'klok_zetten' | 'partijen_invoeren';
}

/** The primary action of a side, as a button in the shell's bar beside the account. */
export interface Actie {
  tekst: string;
  /** The route the button opens. */
  pad: string;
  icoon?: string;
  /** What the coach mark says to someone who has not used the action yet. */
  uitleg?: { tekst: string; toelichting: string };
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

/** Whether `tab` is the one the route `naam` belongs to. */
export const isHuidig = (tab: Tab, naam: unknown): boolean =>
  tab.naam === naam || (typeof naam === 'string' && !!tab.ook?.includes(naam));

declare module 'vue-router' {
  interface RouteMeta {
    /** The sections of the side this route belongs to. */
    tabs?: Tab[];
    /** The name of the side, for a side that has no tabs. */
    titel?: string;
    /** The one action the side offers from its bar. */
    actie?: Actie;
  }
}
