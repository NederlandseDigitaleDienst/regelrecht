<template>
  <div class="hp">
    <div v-if="hasOverrides" class="hp-head">
      <nldd-button
        text="Herstel het model"
        start-icon="undo"
        appearance="neutral-transparent"
        size="sm"
        @click="resetOverrides"
      ></nldd-button>
    </div>
    <nldd-banner v-if="loadError" variant="warning">
      data/handelingen.yaml ontbreekt nog: uitvoeringslast blijft nul tot het bestand er is.
    </nldd-banner>

    <details v-for="groep in groepen" :key="groep.partij" class="hp-group">
      <summary>{{ partijLabel(groep.partij) }} <nldd-badge size="sm" color="neutral" :number="groep.items.length" accessible-label="aantal handelingen"></nldd-badge></summary>
      <div class="hp-fields">
        <nldd-form-field
          v-for="h in groep.items"
          :key="h.id"
          :label="`${handelingTitel(h)} (min)`"
          :supporting-label="`${h.omschrijving ? `${h.omschrijving} · ` : ''}${aanleidingLabel(h.aanleiding)}${h.sector ? ` · ${h.sector}` : ''} · tarief ${h.tarief}${grondslag(h)}`"
        >
          <nldd-number-field
            :value="overrides.minuten[h.id] ?? h.minuten"
            :min="0"
            :step="5"
            size="sm"
            @input="debounced(`m:${h.id}`, $event, (value) => setMinuten(h.id, value))"
          ></nldd-number-field>
        </nldd-form-field>
      </div>
    </details>

    <details class="hp-group">
      <summary>Tarieven (€ per uur)</summary>
      <div class="hp-fields">
        <nldd-form-field v-for="(bedrag, naam) in alleTarieven" :key="naam" :label="`${naam} (€/uur)`">
          <nldd-number-field
            :value="(overrides.tarieven[naam] ?? bedrag) / 100"
            :min="0"
            :step="5"
            size="sm"
            @input="debounced(`t:${naam}`, $event, (value) => setTarief(naam, Math.round(value * 100)))"
          ></nldd-number-field>
        </nldd-form-field>
        <nldd-form-field
          v-for="(ratio, naam) in base?.fracties ?? {}"
          :key="naam"
          :label="`${naam} (%)`"
          supporting-label="fractie van het aantal aanvragen"
        >
          <nldd-number-field
            :value="Math.round((overrides.fracties[naam] ?? ratio) * 1000) / 10"
            :min="0"
            :max="100"
            :step="0.5"
            size="sm"
            @input="debounced(`f:${naam}`, $event, (value) => setFractie(naam, value / 100))"
          ></nldd-number-field>
        </nldd-form-field>
      </div>
    </details>

    <details class="hp-group">
      <summary>Investeringen <nldd-badge size="sm" color="neutral" :number="alleInvesteringen.length" accessible-label="aantal investeringen"></nldd-badge></summary>
      <div class="hp-fields">
        <template v-for="inv in alleInvesteringen" :key="inv.id">
          <nldd-switch-field
            :label="`Actief: ${inv.id}`"
            :checked="(overrides.investeringen[inv.id]?.actief ?? inv.actief ?? true) ? true : undefined"
            @change="setInvestering(inv.id, { actief: $event.detail?.checked ?? true })"
          ></nldd-switch-field>
          <nldd-form-field :label="`${inv.omschrijving ?? inv.id} (€)`" :supporting-label="`eenmalig, ${partijLabel(inv.partij)}`">
            <nldd-number-field
              :value="(overrides.investeringen[inv.id]?.bedrag ?? inv.bedrag) / 100"
              :min="0"
              :step="100000"
              size="sm"
              @input="debounced(`i:${inv.id}:b`, $event, (value) => setInvestering(inv.id, { bedrag: Math.round(value * 100) }))"
            ></nldd-number-field>
          </nldd-form-field>
          <div class="hp-row">
            <nldd-form-field label="Afschrijving (jaren)">
              <nldd-number-field
                :value="overrides.investeringen[inv.id]?.afschrijving_jaren ?? inv.afschrijving_jaren"
                :min="1"
                :max="20"
                size="sm"
                @input="debounced(`i:${inv.id}:a`, $event, (value) => setInvestering(inv.id, { afschrijving_jaren: value }))"
              ></nldd-number-field>
            </nldd-form-field>
            <nldd-form-field label="Vanaf jaar" supporting-label="invoeringsjaar">
              <nldd-number-field
                :value="overrides.investeringen[inv.id]?.vanaf_jaar ?? inv.vanaf_jaar"
                :min="2025"
                :max="2035"
                size="sm"
                @input="debounced(`i:${inv.id}:v`, $event, (value) => setInvestering(inv.id, { vanaf_jaar: value }))"
              ></nldd-number-field>
            </nldd-form-field>
          </div>
        </template>
        <p v-if="!alleInvesteringen.length" class="hp-leeg">Geen investeringen in dit model.</p>
      </div>
    </details>

    <details class="hp-group">
      <summary>Budget (€ mln per jaar)</summary>
      <div class="hp-fields">
        <nldd-form-field v-for="post in BUDGET_POSTEN" :key="post.key" :label="post.label">
          <nldd-number-field
            :value="Math.round(((overrides.budget[post.key] ?? base?.budget?.[post.key] ?? 0) / 1e8) * 10) / 10"
            :min="0"
            :step="1"
            size="sm"
            @input="debounced(`b:${post.key}`, $event, (value) => setBudget(post.key, Math.round(value * 1e8)))"
          ></nldd-number-field>
        </nldd-form-field>
      </div>
    </details>
  </div>
</template>

<script setup>
import { computed } from 'vue';
import { useHandelingen } from '../../composables/useHandelingen.js';
import { partijLabel, aanleidingLabel, handelingTitel } from '../../lib/nieuwkomerFacts.js';

const {
  base, loadError, overrides, hasOverrides, alleHandelingen, alleInvesteringen, alleTarieven,
  setMinuten, setTarief, setFractie, setInvestering, setBudget, resetOverrides,
} = useHandelingen();

const BUDGET_POSTEN = [
  { key: 'regeling_po', label: 'Regeling po' },
  { key: 'regeling_vo', label: 'Regeling vo' },
  { key: 'uitvoering', label: 'Uitvoering (school + DUO + investering)' },
];

const groepen = computed(() => {
  const byPartij = new Map();
  for (const h of alleHandelingen.value) {
    if (!byPartij.has(h.partij)) byPartij.set(h.partij, []);
    byPartij.get(h.partij).push(h);
  }
  const order = ['school', 'duo'];
  return [...byPartij.entries()]
    .sort((a, b) => (order.indexOf(a[0]) === -1 ? 9 : order.indexOf(a[0])) - (order.indexOf(b[0]) === -1 ? 9 : order.indexOf(b[0])))
    .map(([partij, items]) => ({ partij, items }));
});

function grondslag(h) {
  const g = h.grondslag;
  if (!g) return '';
  const art = g.article ? ` art. ${g.article}${g.lid ? ` lid ${g.lid}` : ''}` : '';
  return ` · grondslag${art}`;
}

/**
 * The number in a number field, or null while the field is empty. Since design
 * system 0.18 an emptied field reports null; Number(null) would read that as 0.
 */
function num(event) {
  const v = event.detail?.value ?? event.target?.value;
  if (v === null || v === undefined || v === '') return null;
  const n = Number(v);
  return Number.isNaN(n) ? null : n;
}

const timers = new Map();
// An empty field is no answer yet: the value it had stays until a number is typed.
function debounced(key, event, apply) {
  const n = num(event);
  if (n === null) return;
  clearTimeout(timers.get(key));
  timers.set(key, setTimeout(() => apply(n), 250));
}
</script>

<style scoped>
.hp { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
.hp-head { display: flex; align-items: center; justify-content: space-between; gap: var(--primitives-space-8); }
.hp-title { font-weight: 600; }
.hp-group {
  border: 1px solid var(--semantics-dividers-color);
  border-radius: var(--semantics-surfaces-corner-radius);
  padding: var(--primitives-space-8) var(--primitives-space-12);
}
.hp-group summary { cursor: pointer; font-weight: 600; font-size: 0.9em; }
.hp-fields { display: flex; flex-direction: column; gap: var(--primitives-space-12); margin-top: var(--primitives-space-12); }
.hp-row { display: flex; gap: var(--primitives-space-12); flex-wrap: wrap; }
.hp-leeg { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
</style>
