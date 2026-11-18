/**
 * Laadt en parseert de persona's uit /data/personas.yaml.
 *
 * De records dragen exact het schema dat simulate.js verwacht (zie
 * data/personas.yaml). keuzemomenten stuurt welke keuze-kaarten de
 * burger-view toont.
 */
import { ref, computed } from 'vue';
import * as yaml from 'js-yaml';
import { b } from '../basePad.js';
import { readPersisted, writePersisted } from './useBewaardeStand.js';

const uitYaml = ref([]);
// Zelf toegevoegde scenario's. Ze overleven een ververs maar gaan niet naar de
// repo: een scenario is een sessievondst, geen brongegeven. Wie er een wil
// bewaren voor iedereen, plakt hem met "Kopieer als YAML" in personas.yaml.
const eigen = ref(readPersisted('scenarios.eigen', []));
const personas = computed(() => [...uitYaml.value, ...eigen.value]);
const loaded = ref(false);
const loadError = ref(null);
let loadPromise = null;

async function fetchPersonas() {
  if (loadPromise) return loadPromise;
  loadPromise = (async () => {
    try {
      const res = await fetch(b('/data/personas.yaml'));
      if (!res.ok) throw new Error(`Ophalen mislukt (${res.status})`);
      const doc = yaml.load(await res.text());
      uitYaml.value = doc?.personas ?? [];
      loaded.value = true;
    } catch (e) {
      loadError.value = e;
      throw e;
    }
    return personas.value;
  })();
  return loadPromise;
}

function bewaarEigen() {
  writePersisted('scenarios.eigen', eigen.value);
}

/** Toevoegen of, als het bsn al bestaat, bijwerken. */
function bewaarScenario(record) {
  const i = eigen.value.findIndex((s) => s.bsn === record.bsn);
  if (i >= 0) eigen.value = eigen.value.map((s, j) => (j === i ? record : s));
  else eigen.value = [...eigen.value, record];
  bewaarEigen();
}

function verwijderScenario(bsn) {
  eigen.value = eigen.value.filter((s) => s.bsn !== bsn);
  bewaarEigen();
}

export function usePersonas() {
  return { personas, eigen, loaded, loadError, fetchPersonas, bewaarScenario, verwijderScenario };
}
