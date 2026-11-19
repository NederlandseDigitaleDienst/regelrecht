// The transport of the app: every call goes to a command of the Rust core
// (packages/wetsproces-app) through Tauri's `invoke`; the citizen gets in by
// activating the app. See ../vervoer.ts.
import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { createWebHashHistory } from 'vue-router';
import type { Vervoer } from '../vervoer';
import { kernSessie } from '../tauri/kern';
import type { Invoke } from '../tauri/kern';
import { appStatus, laadStatus, opslagTekst } from '../tauri/status';

export const invoke: Invoke = (commando, args) => tauriInvoke(commando, args);

export const vervoer: Vervoer = {
  sessie: (bijGeenToegang) =>
    kernSessie(invoke, (f) => {
      void laadStatus(invoke).catch(() => undefined);
      bijGeenToegang(f.kind);
    }),
  async toegang() {
    const s = await laadStatus(invoke).catch(() => appStatus.value);
    const h = s?.holder;
    return { binnen: !!h, naam: h ? (h.name ?? `BSN ${h.bsn}`) : null };
  },
  toegangScherm: () => import('../tauri/ActiverenView.vue'),
  afmelden: {
    tekst: 'Activering intrekken',
    doe: async () => {
      appStatus.value = await invoke('afmelden');
    },
  },
  onderschrift: () => (appStatus.value?.holder ? opslagTekst(appStatus.value) : ''),
  // The webview loads the app from a file and has no server to fall back on.
  history: () => createWebHashHistory(),
};
