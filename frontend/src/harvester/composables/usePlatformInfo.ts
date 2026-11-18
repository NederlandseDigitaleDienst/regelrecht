import { ref } from 'vue';
import { apiFetchJson } from '@regelrecht/frontend-shared';

/** GET /api/harvest-admin/info (`PlatformInfo` in packages/admin/src/handlers.rs). */
export type PlatformInfo = {
  deployment_name: string;
  component_name: string;
};

const info = ref<PlatformInfo | null>(null);
let fetched = false;

async function load() {
  try {
    info.value = await apiFetchJson<PlatformInfo>('/api/harvest-admin/info');
  } catch {
    // ignore
  }
}

export function usePlatformInfo() {
  if (!fetched) {
    fetched = true;
    load();
  }

  return { info };
}
