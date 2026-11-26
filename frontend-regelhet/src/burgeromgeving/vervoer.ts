// The transport of the citizen's entry (burger.html): how its calls reach
// the server, how the citizen gets in, and how they leave. The screens
// (src/burger/), the shell and the router know only this interface.
//
// Which transport a build carries is chosen when it is built: the import
// `#vervoer` resolves to one file of vervoer/ (vite.config.ts, by mode), so a
// bundle holds only its own transport:
//
// - vervoer/tauri.ts: the app (`--mode app`), every call a command of the
//   Rust core over FSC voor Burgers, getting in by activation;
// - vervoer/cookie.ts: the citizen's web environment (`--mode burger`), plain
//   HTTP with the session cookie, getting in through the login;
// - vervoer/fsc4b.ts: the citizen's own app in the browser (`--mode
//   burgerapp`), every call over FSC voor Burgers to a RegelHet of its list,
//   getting in by choosing one and activating the app there.
import type { Component } from 'vue';
import type { RouterHistory } from 'vue-router';
import type { Sessie } from '../gedeeld/sessie';

/** Whether the citizen is in, and under what name. */
export interface Toegang {
  binnen: boolean;
  naam: string | null;
}

export interface Vervoer {
  /** The session for every call; `bijGeenToegang` runs when the citizen has to get in again. */
  sessie(bijGeenToegang: (reden: string) => void): Sessie;
  /** Whether the citizen is in. */
  toegang(): Promise<Toegang>;
  /** The screen to get in: the activation of the app, or the login. */
  toegangScherm: () => Promise<Component>;
  /** The menu item to leave, and what it does. */
  afmelden: { tekst: string; doe: () => Promise<void> };
  /** A line under every screen once the citizen is in, or ''. */
  onderschrift(): string;
  /** How the router keeps its place. */
  history(): RouterHistory;
  /**
   * A panel on the application form that predicts the decision before the
   * citizen applies (`VOORSPELLING` in burger/voorspelling.ts), or none.
   */
  voorspelling?: () => Promise<Component>;
}
