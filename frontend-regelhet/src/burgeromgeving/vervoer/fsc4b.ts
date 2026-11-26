// The transport of the citizen's own app in the browser (`--mode burgerapp`):
// every call goes over FSC voor Burgers to the RegelHet the citizen chose,
// through the core in ../fsc4b/kern.ts, which holds the app key, the
// credential and the session. Getting in is choosing a RegelHet from the
// app's list and activating the app there. See ../vervoer.ts.
import { createWebHashHistory } from 'vue-router';
import { ref } from 'vue';
import type { Vervoer } from '../vervoer';
import { burgerapp, gekozen, kies } from '../fsc4b/app';
import { kernSessie } from '../tauri/kern';
import type { AppStatus, Invoke } from '../tauri/kern';
import { opslagTekst } from '../tauri/status';

export const invoke: Invoke = async <T>(commando: string, args?: Record<string, unknown>) =>
  (await burgerapp()).invoke(() => gekozen.value)<T>(commando, args);

/** The line under every screen: which RegelHet, and where the key lives. */
const onderschrift = ref('');

export const vervoer: Vervoer = {
  sessie: (bijGeenToegang) => kernSessie(invoke, (f) => bijGeenToegang(f.kind)),
  async toegang() {
    const id = gekozen.value;
    if (!id) return { binnen: false, naam: null };
    const app = await burgerapp().catch(() => null);
    const s: AppStatus | null = app ? await app.status(id).catch(() => null) : null;
    const h = s?.holder;
    onderschrift.value = s && h ? `${app?.regelhet(id).naam ?? id}. ${opslagTekst(s)}` : '';
    return { binnen: !!h, naam: h ? (h.name ?? `BSN ${h.bsn}`) : null };
  },
  toegangScherm: () => import('../fsc4b/KeuzeView.vue'),
  afmelden: {
    tekst: 'Andere regeling kiezen',
    doe: async () => kies(null),
  },
  onderschrift: () => onderschrift.value,
  // A static app under any path, without a server that falls back to it.
  history: () => createWebHashHistory(),
  voorspelling: () => import('../fsc4b/VoorspellingPaneel.vue'),
};
