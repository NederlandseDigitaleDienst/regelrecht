<template>
  <nldd-page>
    <nldd-simple-section>
      <div class="intro-kop">
        <nldd-title heading-level="1" size="display-1">
          <span slot="overline">Voor scholen</span>
          <span>Het Bestand Nieuwkomers van één school</span>
        </nldd-title>
        <div class="kop-rechts">
          <nldd-form-field label="School">
            <nldd-dropdown :key="`${schoolId}:${schoolGroepen.length}`" size="sm" width="360px" @change="schoolId = $event.target.value ?? schoolId">
              <select :value="schoolId ?? ''" aria-label="Kies een school">
                <option v-if="!schoolId" value="" disabled>Kies een school…</option>
                <optgroup v-for="groep in schoolGroepen" :key="groep.label" :label="groep.label">
                  <option v-for="s in groep.items" :key="s.id" :value="s.id">{{ s.label }}</option>
                </optgroup>
              </select>
            </nldd-dropdown>
          </nldd-form-field>
          <nldd-form-field label="Peildatum">
            <nldd-dropdown :key="`${peildatum}:${peildata.length}`" size="sm" width="220px" @change="peildatum = $event.target.value ?? peildatum">
              <select :value="peildatum" aria-label="Peildatum">
                <option v-for="pd in peildata" :key="pd" :value="pd">{{ datumLabel(pd) }}</option>
              </select>
            </nldd-dropdown>
          </nldd-form-field>
          <nldd-form-field label="Vergelijk met">
            <variant-switcher :model-value="variantId" @update:model-value="variantId = $event" />
          </nldd-form-field>
        </div>
      </div>

      <div class="meldingen">
        <nldd-banner v-if="initError" variant="critical">De rekenmachine kon niet worden geladen: {{ initError.message }}</nldd-banner>
        <nldd-banner v-else-if="!ready" variant="accent">Een moment, de regelgeving wordt geladen…</nldd-banner>
        <nldd-banner v-if="error" variant="critical">{{ error }}</nldd-banner>
      </div>
    </nldd-simple-section>

    <template v-if="ready && ist">
      <!-- De uitkomst staat boven het bestand: dat is waar de bezoeker voor
           komt, en bij twee kolommen naast elkaar is het ook het enige dat de
           variant van huidig recht onderscheidt. De leerlingenlijst is de
           onderbouwing en staat eronder. -->
      <nldd-simple-section>
        <nldd-title heading-level="2" size="display-4">
          <span>Wat er uit het bestand volgt</span>
          <span slot="supporting-text">Drempel, formulieren, deadline en het geld voor deze peildatum.</span>
        </nldd-title>
        <!-- Deze melding stond bovenaan de pagina, buiten beeld tegen de tijd
             dat je bij de kolommen bent. Twee identieke kolommen lezen dan als
             een fout, terwijl het antwoord is dat de variant later ingaat. -->
        <nldd-banner v-if="variantNogNietGeldig" variant="accent">
          Deze variant geldt vanaf {{ datumLabel(variantVanafDatum) }}; op deze peildatum is hij gelijk aan huidig recht.
        </nldd-banner>
        <div class="kolommen">
          <aanvraag-kaart :titel="istTitel" :entry="ist.entry" :sector="sector" :peildatum="peildatum" :deadline="ist.deadline" />
          <aanvraag-kaart v-if="variant" :titel="variantTitel" :entry="variant.entry" :sector="sector" :peildatum="peildatum" :deadline="variant.deadline" />
        </div>
      </nldd-simple-section>

      <nldd-simple-section background="tinted">
        <div class="school-kop">
          <nldd-title heading-level="2" size="display-3">
            <span>{{ schoolId }}</span>
            <span slot="supporting-text">{{ sectorLabel(sector) }} · {{ pupils.length }} nieuwkomers in de periode · {{ ist.entry.in_bestand }} in het bestand op {{ datumLabel(peildatum) }} (klaar op de {{ bestandDatum(peildatum).slice(8) }}e in Mijn DUO)</span>
          </nldd-title>
          <nldd-segmented-control v-if="variant" size="sm" :value="bestandKolom" @change="bestandKolom = $event.detail?.value ?? bestandKolom">
            <nldd-segmented-control-item value="ist" :text="istTitel"></nldd-segmented-control-item>
            <nldd-segmented-control-item value="variant" :text="variantTitel"></nldd-segmented-control-item>
          </nldd-segmented-control>
        </div>
        <nldd-activity-indicator v-if="loading" size="24" timing="instant"></nldd-activity-indicator>
        <bestand-nieuwkomers v-else :leerlingen="(bestandKolom === 'variant' && variant ? variant : ist).leerlingen" :sector="sector" />
      </nldd-simple-section>

      <nldd-simple-section>
        <nldd-title heading-level="2" size="display-4">
          <span>Handelingen voor deze school op deze peildatum</span>
          <span slot="supporting-text">Welke stappen, hoeveel minuten, en wat dat kost tegen het uurtarief.</span>
        </nldd-title>
        <div class="kolommen">
          <div class="kolom">
            <span class="kolom-titel">{{ istTitel }}</span>
            <handelingen-lijst :handelingen="handelingenFor(null)" :entry="ist.entry" :sector="sector" :peildatum="peildatum" />
          </div>
          <div v-if="variant" class="kolom">
            <span class="kolom-titel">{{ variantTitel }}</span>
            <handelingen-lijst :handelingen="handelingenFor(variantId)" :entry="variant.entry" :sector="sector" :peildatum="peildatum" />
          </div>
        </div>
      </nldd-simple-section>
    </template>
  </nldd-page>
</template>

<script setup>
import { ref, computed, onMounted, watch } from 'vue';
import { useEngine } from '../engine/useEngine.js';
import { useLawStore } from '../engine/lawStore.js';
import { usePersonas, PERSONA_PEILDATA } from '../composables/usePersonas.js';
import { useColumnEngines } from '../composables/useColumnEngines.js';
import { useSimulation, shortTitle } from '../composables/useSimulation.js';
import { useHandelingen } from '../composables/useHandelingen.js';
import {
  evaluateLeerlingTimeline,
  buildSchoolRecord,
  evaluateSchoolOpPeildatum,
  sectorVan,
} from '../sim/simulate.js';
import VariantSwitcher from '../components/VariantSwitcher.vue';
import BestandNieuwkomers from '../components/school/BestandNieuwkomers.vue';
import AanvraagKaart from '../components/school/AanvraagKaart.vue';
import HandelingenLijst from '../components/school/HandelingenLijst.vue';
import { persistedRef } from '../composables/useBewaardeStand.js';
import {
  LAW_WERKINSTRUCTIE,
  datumLabel,
  sectorLabel,
  bestandDatum,
  aanvraagDeadline,
  tabbladVoor,
  peildataVoorJaren, variantVanaf } from '../lib/nieuwkomerFacts.js';

const { ready, initError, lawIndex, initEngine } = useEngine();
const { initStore, version, variants, werkversie, changeCount } = useLawStore();
const { personas, fetchPersonas } = usePersonas();
const { engineFor } = useColumnEngines();
const { records, jaren, ensureRecords } = useSimulation();
const { fetchHandelingen, ensureVariantDoc, handelingenFor } = useHandelingen();

const schoolId = persistedRef('school.id', null);
// 1 januari 2028: de eerste peildatum waarop de varianten gelden. Stond op
// 2025-10-01, de eerste peildatum met leerlingen in het bestand, maar daar valt
// een variant per definitie samen met huidig recht — twee identieke kolommen,
// wat als een fout leest in plaats van als "geldt hier nog niet".
//
// Geen `isValid`-controle erbij: een bewaarde keuze wint van de standaard, en
// dat hoort zo. Wie zelf een eerdere peildatum kiest, meent dat. Alleen wie de
// pagina eerder opende houdt 2025-10-01 tot hij de keuze zelf verzet.
const peildatum = persistedRef('school.peildatum', '2028-01-01');
const variantId = persistedRef('school.variant', null);
const bestandKolom = ref('ist');
const ist = ref(null);
const variant = ref(null);
const loading = ref(false);
const error = ref(null);

const variantVanafDatum = computed(() => variantVanaf(variants.value.find((x) => x.id === variantId.value)));
const variantNogNietGeldig = computed(() => !!variantId.value && !!variantVanafDatum.value && String(peildatum.value) < variantVanafDatum.value);

const variantTitel = computed(() => {
  const v = variants.value.find((x) => x.id === variantId.value);
  const bewerkt = variantId.value && variantId.value === werkversie.value && changeCount.value ? ' (werkversie, bewerkt)' : '';
  return (v ? shortTitle(v) : 'Variant') + bewerkt;
});
/** Links staat huidig recht; als dat de werkversie is met bewerkingen, zeg dat. */
const istTitel = computed(() => (werkversie.value === null && changeCount.value ? 'Huidig recht (werkversie, bewerkt)' : 'Huidig recht'));

// ---- Schoolkeuze --------------------------------------------------------------

const personaScholen = computed(() => {
  const by = new Map();
  for (const p of personas.value) {
    if (!p.school_id) continue;
    if (!by.has(p.school_id)) by.set(p.school_id, []);
    by.get(p.school_id).push(p);
  }
  return [...by.entries()].map(([id, ps]) => ({
    id,
    bron: 'persona',
    sector: sectorVan(ps[0]),
    label: `${id} · casussen: ${ps.map((p) => p.naam).join(', ')}`,
  }));
});

const simScholen = computed(() => {
  const by = new Map();
  for (const r of records.value) {
    if (!by.has(r.school_id)) by.set(r.school_id, { id: r.school_id, sector: sectorVan(r), n: 0 });
    by.get(r.school_id).n++;
  }
  const all = [...by.values()].sort((a, b) => b.n - a.n);
  const label = (s, hint) => ({ ...s, bron: 'sim', label: `${s.id} · ${s.n} nieuwkomers 2023-2028 · ${s.sector}${hint ? ` · ${hint}` : ''}` });
  const groot = all.slice(0, 8).map((s) => label(s, 'groot'));
  const rondDrempel = all.filter((s) => s.sector === 'po' && s.n >= 4 && s.n <= 9).slice(0, 8).map((s) => label(s, 'rond de drempel'));
  const klein = all.filter((s) => s.n <= 3).slice(0, 6).map((s) => label(s, 'klein'));
  const seen = new Set();
  return [...groot, ...rondDrempel, ...klein].filter((s) => (seen.has(s.id) ? false : seen.add(s.id)));
});

const schoolGroepen = computed(() => [
  { label: 'Scholen van de casussen', items: personaScholen.value },
  { label: 'Gesimuleerde scholen', items: simScholen.value },
].filter((g) => g.items.length));

const bron = computed(() => (personaScholen.value.some((s) => s.id === schoolId.value) ? 'persona' : 'sim'));

const pupils = computed(() => {
  if (!schoolId.value) return [];
  return bron.value === 'persona'
    ? personas.value.filter((p) => p.school_id === schoolId.value)
    : records.value.filter((r) => r.school_id === schoolId.value);
});

const sector = computed(() => (pupils.value.length ? sectorVan(pupils.value[0]) : 'po'));

const peildata = computed(() => (bron.value === 'persona' ? PERSONA_PEILDATA : peildataVoorJaren(jaren.value)));

watch(peildata, (list) => {
  if (!list.includes(peildatum.value)) peildatum.value = list.find((pd) => pd >= '2025-01-01') ?? list[0];
});

// ---- Doorrekenen ---------------------------------------------------------------

function tabbladVia(engine, record, pd, uitkomst) {
  if (engine.hasLaw?.(LAW_WERKINSTRUCTIE)) {
    try {
      engine.registerDataSource('personas', 'bsn', [record]);
      const res = engine.execute(LAW_WERKINSTRUCTIE, 'tabblad', { bsn: record.bsn }, pd);
      const t = Number(res.outputs?.tabblad);
      if (t >= 1 && t <= 3) return t;
    } catch {
      // werkinstructie kent geen tabblad-output: codelijsten
    } finally {
      engine.clearDataSources();
    }
  }
  return tabbladVoor(record, uitkomst.categorie);
}

function deadlineVia(engine, pd) {
  if (engine.hasLaw?.(LAW_WERKINSTRUCTIE)) {
    try {
      const res = engine.execute(LAW_WERKINSTRUCTIE, 'uiterste_aanvraagdatum', {}, pd);
      const d = res.outputs?.uiterste_aanvraagdatum;
      if (typeof d === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(d)) return d;
    } catch {
      // geen output: benadering
    }
  }
  return aanvraagDeadline(pd);
}

async function evalKolom(vid, school, list, pdList, k) {
  const engine = await engineFor(vid);
  const cache = new Map();
  const timelines = list.map((p) => evaluateLeerlingTimeline(engine, p, pdList, cache));
  const firstK = pdList.findIndex((_, i) => timelines.some((t) => t[i].telt));
  const nieuw = list.every((p) => p.eerste_inschrijfdatum >= pdList[0]);
  const eersteKeer = nieuw && firstK === k;
  const resultaten = list.map((p, i) => ({ record: p, uitkomst: timelines[i][k] }));
  const { record, facts } = buildSchoolRecord(school, resultaten, pdList[k], eersteKeer);
  engine.registerDataSource('scholen', 'school_id', [record]);
  let res;
  try {
    res = evaluateSchoolOpPeildatum(engine, school, record, facts, pdList[k], cache);
  } finally {
    engine.clearDataSources();
  }
  const leerlingen = resultaten
    .filter((r) => r.uitkomst.in_bestand)
    .map((r) => ({ ...r, tabblad: tabbladVia(engine, r.record, pdList[k], r.uitkomst) }));
  const entry = {
    Ap: record.aantal_asielzoekers_peildatum,
    Vp: record.aantal_overige_vreemdelingen_peildatum,
    At: record.aantal_asielzoekers_telling_1_februari,
    tweedejaars: record.aantal_tweedejaars_asielzoekers_peildatum,
    nieuwkomers_vo: record.aantal_nieuwkomers_peildatum,
    eerste_keer: eersteKeer,
    in_bestand: facts.in_bestand,
    tellend: facts.tellend,
    onbeslist: facts.onbeslist,
    ambigu: facts.ambigu,
    zonder_bsn: facts.zonder_bsn,
    eerste_cat_vo: facts.eerste_cat_vo,
    tweede_cat_vo: facts.tweede_cat_vo,
    ...res,
  };
  return { leerlingen, entry, deadline: deadlineVia(engine, pdList[k]) };
}

let runId = 0;
async function herbereken() {
  if (!ready.value || !schoolId.value || !pupils.value.length) {
    ist.value = null;
    variant.value = null;
    return;
  }
  const id = ++runId;
  loading.value = true;
  error.value = null;
  try {
    const pdList = peildata.value;
    const k = pdList.indexOf(peildatum.value);
    const school = { school_id: schoolId.value, sector: sector.value, schoolsoort: pupils.value[0].schoolsoort ?? (sector.value === 'vo' ? 'vo' : 'basisschool') };
    const a = await evalKolom(null, school, pupils.value, pdList, k);
    let b = null;
    if (variantId.value) {
      await ensureVariantDoc(variantId.value);
      b = await evalKolom(variantId.value, school, pupils.value, pdList, k);
    }
    if (id !== runId) return;
    ist.value = a;
    variant.value = b;
    if (a.entry.fout) error.value = `Schoolniveau: ${a.entry.fout}`;
  } catch (e) {
    if (id === runId) error.value = String(e?.message ?? e);
  } finally {
    if (id === runId) loading.value = false;
  }
}

watch([schoolId, peildatum, variantId, version, ready, pupils], herbereken);

onMounted(async () => {
  await fetchPersonas().catch(() => {});
  await fetchHandelingen();
  try {
    const engine = await initEngine();
    await initStore(engine, lawIndex.value);
    if (!records.value.length) await ensureRecords().catch(() => {});
    if (!variantId.value) {
      const amb = variants.value.find((v) => /ambtshalve/i.test(v.title) || /ambtshalve/i.test(v.id));
      variantId.value = amb?.id ?? variants.value[0]?.id ?? null;
    }
    if (!schoolId.value) schoolId.value = personaScholen.value[0]?.id ?? simScholen.value[0]?.id ?? null;
  } catch {
    // fout via initError
  }
});
</script>

<style scoped>
.intro-kop { display: flex; flex-direction: column; gap: var(--primitives-space-16); }
.kop-rechts { display: flex; gap: var(--primitives-space-16); align-items: flex-end; flex-wrap: wrap; }
.kop-rechts nldd-form-field { flex: 1 1 240px; max-width: 420px; }
.meldingen { display: flex; flex-direction: column; gap: var(--primitives-space-8); margin-top: var(--primitives-space-16); }
.meldingen:empty { display: none; }
.school-kop { display: flex; justify-content: space-between; align-items: flex-start; gap: var(--primitives-space-16); flex-wrap: wrap; margin-bottom: var(--primitives-space-12); }
.kolommen { display: grid; grid-template-columns: repeat(auto-fit, minmax(340px, 1fr)); gap: var(--primitives-space-16); margin-top: var(--primitives-space-12); }
.kolom { display: flex; flex-direction: column; gap: var(--primitives-space-8); }
.kolom-titel { font-weight: 600; }
</style>
