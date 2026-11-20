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
  recht?: 'klok_zetten';
}

/** The tabs of the side `route` belongs to, from the deepest route that declares them. */
export function tabsVan(route: Pick<RouteLocationNormalizedLoaded, 'matched'>): Tab[] {
  return [...route.matched].reverse().find((r) => r.meta.tabs)?.meta.tabs ?? [];
}

/** Whether `tab` is the one the route `naam` belongs to. */
export const isHuidig = (tab: Tab, naam: unknown): boolean =>
  tab.naam === naam || (typeof naam === 'string' && !!tab.ook?.includes(naam));

declare module 'vue-router' {
  interface RouteMeta {
    /** The sections of the side this route belongs to. */
    tabs?: Tab[];
  }
}
