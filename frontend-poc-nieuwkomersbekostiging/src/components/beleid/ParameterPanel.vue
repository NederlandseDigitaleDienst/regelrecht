<template>
  <div class="param-panel">
    <!-- Een knop hier verandert de rekenregels, niet de tekst van de regeling
         ernaast. Dat verschil hoort zichtbaar te zijn: anders vertelt het
         artikel straks de oude regel terwijl de berekening de nieuwe volgt. De
         beleidsassistent schrijft de tekst wel mee. -->
    <nldd-banner v-if="hasChanges" variant="accent">
      Je wijzigt hier de rekenregels. De tekst van het artikel beweegt niet mee; die staat er nog zoals hij was.
      Vraag de beleidsassistent om de wijziging als je ook de tekst wilt laten meeschrijven.
    </nldd-banner>

    <nldd-form-field label="Document" supporting-label="één regeling tegelijk; oudere versies staan onderaan de lijst">
      <nldd-dropdown :key="`pp:${docsMetVersie.length}:${gekozenPad}`" size="sm" width="100%" @change="gekozenPad = $event.target.value ?? gekozenPad">
        <select :value="gekozenPad ?? ''" aria-label="Kies het document waarvan je de parameters bijstelt">
          <option v-for="doc in docsMetVersie" :key="doc.lawPath" :value="doc.lawPath">
            {{ doc.docName }} · geldig vanaf {{ datumLabel(doc.validFrom) }}{{ doc.ouder ? ' (oudere versie)' : '' }}{{ doc.bewerkt ? ' · bewerkt' : '' }}
          </option>
        </select>
      </nldd-dropdown>
    </nldd-form-field>

    <template v-if="gekozen">
      <details v-for="art in gekozen.articles" :key="art.article" class="pp-article" :open="!art.zelden ? true : undefined">
        <summary>Artikel {{ art.article }}<span v-if="onderwerp(art)" class="pp-onderwerp"> · {{ onderwerp(art) }}</span><span v-if="art.zelden" class="pp-zelden"> · minder gebruikt</span></summary>
        <div class="pp-fields">
          <nldd-form-field
            v-for="def in art.defs"
            :key="def.key"
            :label="fieldLabel(def)"
            :supporting-label="def.description || undefined"
          >
            <nldd-number-field
              :value="displayValue(def)"
              :min="0"
              :step="stepFor(def)"
              size="sm"
              @input="onInput(def, $event)"
            ></nldd-number-field>
          </nldd-form-field>
        </div>
      </details>
    </template>
    <p v-else class="pp-leeg">Geen bijstelbare parameters in dit document.</p>
  </div>
</template>

<script setup>
import { ref, computed, watch } from 'vue';
import { useLawStore } from '../../engine/lawStore.js';

const { definitionsByDoc, applyDefinitionChange, changedPaths, hasChanges } = useLawStore();
import { LAW_PO, artikelOnderwerp, leesbaar } from '../../lib/nieuwkomerFacts.js';

/** Documenten met een vlag voor oudere versies van dezelfde wet (dichtgeklapt). */
/**
 * Alleen beleidsknoppen: bedragen, percentages, kwartalen, drempels, termijnen.
 * Structurele constanten (nullen, jaartelling, maand- en dagnummers,
 * referentiedata, codelijsten) horen niet in een sessie te veranderen.
 */
const KNOP_NAAM = /(bedrag|kwartalen|drempel|termijn|weken|jaren|maanden|leeftijd|aantal|toeslag|fractie|percentage)/i;
const GEEN_KNOP = /^(geen_|maand_|.*_dag$|.*_maand$|referentie|.*bekostigingsjaar$|kwartaalfractie$|maanden_per_kwartaal$|codes_|schoolsoorten_|peildatum_)/i;
function isBeleidsknop(def) {
  if (typeof def.value !== 'number') return false;
  if (GEEN_KNOP.test(def.name)) return false;
  if (['eurocent', 'percentage', 'ratio'].includes(def.unit)) return true;
  return KNOP_NAAM.test(def.name);
}

/** Artikelen voor sbo en POL/GLO zijn zelden onderwerp van de sessie: dicht. */
const ZELDEN = /(sbo|pol_glo)/i;

const docsMetVersie = computed(() => {
  const docs = definitionsByDoc.value;
  const lijst = docs
    .map((d) => ({
      ...d,
      articles: d.articles
        .map((a) => ({ ...a, defs: a.defs.filter(isBeleidsknop) }))
        .filter((a) => a.defs.length > 0)
        .map((a) => ({ ...a, zelden: a.defs.every((def) => ZELDEN.test(def.name)) })),
      ouder: docs.some((o) => o !== d && o.lawId && o.lawId === d.lawId && String(o.validFrom) > String(d.validFrom)),
      bewerkt: changedPaths.value.includes(d.lawPath),
    }))
    .filter((d) => d.articles.length > 0);
  // Nieuwste versies eerst, de po-regeling voorop; oudere versies onderaan.
  return [
    ...lijst.filter((d) => !d.ouder).sort((a, b) => (a.lawId === LAW_PO ? -1 : b.lawId === LAW_PO ? 1 : 0)),
    ...lijst.filter((d) => d.ouder),
  ];
});

const gekozenPad = ref(null);
watch(docsMetVersie, (docs) => {
  if (!docs.some((d) => d.lawPath === gekozenPad.value)) gekozenPad.value = docs[0]?.lawPath ?? null;
}, { immediate: true });
const gekozen = computed(() => docsMetVersie.value.find((d) => d.lawPath === gekozenPad.value) ?? null);

function datumLabel(iso) {
  if (!iso) return '?';
  const [y, m, d] = String(iso).split('-');
  return `${Number(d)}-${Number(m)}-${y}`;
}

// Debounce per definitie-key zodat slepen op de spinner niet elke tussenstap
// de wet herlaadt.
const timers = new Map();

function isRatio(def) {
  return def.unit === 'ratio';
}
function isEuro(def) {
  return def.unit === 'eurocent';
}

/** Toon eurocent als euro's en ratio's als procenten. */
function displayValue(def) {
  if (isEuro(def)) return Math.round(def.value) / 100;
  if (isRatio(def)) return Math.round(def.value * 10000) / 100; // 0.84 -> 84
  return def.value;
}

/** Zet de getoonde waarde terug naar de opslag-eenheid. */
function toStored(def, shown) {
  if (isEuro(def)) return Math.round(shown * 100);
  if (isRatio(def)) return Math.round((shown / 100) * 10000) / 10000;
  return shown;
}

function stepFor(def) {
  if (isRatio(def)) return 1; // 1 procentpunt
  if (isEuro(def)) return 1; // 1 euro
  return 1;
}

function fieldLabel(def) {
  const eenheid = isEuro(def) ? ' (€)' : isRatio(def) ? ' (%)' : '';
  return `${leesbaar(def.name)}${eenheid}`;
}

function onderwerp(art) {
  return artikelOnderwerp(gekozen.value?.lawId, art.article);
}

function onInput(def, event) {
  // An emptied field reports null (design system 0.18): no answer yet, so the
  // parameter keeps its value. Number(null) would make that 0.
  const raw = event.detail?.value ?? event.target?.value;
  if (raw === null || raw === undefined || raw === '') return;
  const shown = Number(raw);
  if (Number.isNaN(shown)) return;
  const stored = toStored(def, shown);
  const key = def.key;
  clearTimeout(timers.get(key));
  timers.set(
    key,
    setTimeout(() => {
      applyDefinitionChange(def.lawPath, def.article, def.name, stored);
    }, 250),
  );
}
</script>

<style scoped>
.param-panel { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
.pp-article {
  border: 1px solid var(--semantics-dividers-color);
  border-radius: var(--semantics-surfaces-corner-radius);
  padding: var(--primitives-space-8) var(--primitives-space-12);
  margin-bottom: var(--primitives-space-8);
}
.pp-article summary { cursor: pointer; font-weight: 600; font-size: 1em; }
.pp-onderwerp { font-weight: 400; color: var(--semantics-content-secondary-color); }
.pp-fields { display: flex; flex-direction: column; gap: var(--primitives-space-12); margin-top: var(--primitives-space-12); }
.pp-zelden { font-weight: 400; color: var(--semantics-content-secondary-color); }
.pp-leeg { margin: 0; font-size: 0.9em; color: var(--semantics-content-secondary-color); }
</style>
