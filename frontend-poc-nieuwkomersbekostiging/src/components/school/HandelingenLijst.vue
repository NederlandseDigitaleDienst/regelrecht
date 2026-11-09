<template>
  <div class="hl">
    <div v-for="groep in groepen" :key="groep.partij" class="hl-groep">
      <div class="hl-kop">
        <span class="hl-partij">{{ partijLabel(groep.partij) }}</span>
        <span class="hl-tot">{{ minutes(groep.minuten) }} · {{ euro(groep.kosten) }}</span>
      </div>
      <nldd-list appearance="box-tinted">
        <nldd-list-item v-for="h in groep.items" :key="h.id" size="sm">
          <nldd-text-cell size="sm" :text="handelingTitel(h)" :color="h.aantal ? 'content' : 'secondary'">
            <span slot="supporting-text" class="hl-toelichting">
              <span v-if="h.omschrijving">{{ h.omschrijving }}</span>
              <span class="hl-meta">{{ aanleidingLabel(h.aanleiding) }}{{ h.grondslag?.article ? ` · art. ${h.grondslag.article}${h.grondslag.lid ? ` lid ${h.grondslag.lid}` : ''}` : '' }}</span>
            </span>
          </nldd-text-cell>
          <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :color="h.aantal ? 'content' : 'secondary'" :text="h.aantal ? `${aantalTekst(h)} · ${minutes(h.totaalMinuten)}` : 'n.v.t.'" :supporting-text="h.aantal ? euro(h.kosten) : ''"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </div>
    <p v-if="!groepen.length" class="hl-leeg">Geen handelingen voor deze sector in het model.</p>
  </div>
</template>

<script setup>
import { computed } from 'vue';
import { euro, minutes, decimal } from '../../lib/format.js';
import { partijLabel, aanleidingLabel, isEenJanuari, handelingTitel } from '../../lib/nieuwkomerFacts.js';

const props = defineProps({
  handelingen: { type: Object, default: null }, // handelingenFor(variantId)
  entry: { type: Object, required: true },
  sector: { type: String, default: 'po' },
  peildatum: { type: String, required: true },
});

function aantalVoor(h) {
  const e = props.entry;
  const fractie = Number(props.handelingen?.fracties?.bezwaar_per_aanvraag ?? 0);
  switch (h.aanleiding) {
    case 'per_school_peildatum': return e.in_bestand > 0 ? 1 : 0;
    case 'per_leerling_peildatum': return e.tellend ?? 0;
    case 'per_ambigu_leerling': return e.ambigu ?? 0;
    case 'per_aanvraag': return e.aanvraag ? 1 : 0;
    case 'per_aanvraag_1_januari': return e.aanvraag && isEenJanuari(props.peildatum) ? 1 : 0;
    case 'per_bezwaar': return e.aanvraag ? fractie : 0;
    case 'per_school_jaar': return e.tellend > 0 ? 0.25 : 0;
    case 'per_leerling_zonder_bsn': return e.zonder_bsn ?? 0;
    case 'per_aanvraag_voorbereidingskosten': return e.aanvraag_voorbereidingskosten ? 1 : 0;
    default: return 0;
  }
}

function aantalTekst(h) {
  if (h.aanleiding === 'per_school_jaar') return 'jaarlijks (¼)';
  if (h.aanleiding === 'per_bezwaar') return `kans ${decimal(h.aantal * 100, 0)}%`;
  return `${decimal(h.aantal, 0)}×`;
}

const groepen = computed(() => {
  const doc = props.handelingen;
  if (!doc) return [];
  const tarieven = doc.tarieven ?? {};
  const byPartij = new Map();
  const jaar = Number(String(props.peildatum ?? '').slice(0, 4));
  for (const h of doc.handelingen ?? []) {
    // Jaarvenster van de variant: stappen die vervallen (tot_jaar) of erbij komen (vanaf_jaar).
    if (h.vanaf_jaar != null && jaar < Number(h.vanaf_jaar)) continue;
    if (h.tot_jaar != null && jaar >= Number(h.tot_jaar)) continue;
    if (h.sector && h.sector !== props.sector) continue;
    const aantal = aantalVoor(h);
    const totaalMinuten = aantal * Number(h.minuten ?? 0);
    const tarief = typeof h.tarief === 'number' ? h.tarief : Number(tarieven[h.tarief] ?? 0);
    const kosten = (totaalMinuten / 60) * tarief;
    if (!byPartij.has(h.partij)) byPartij.set(h.partij, { partij: h.partij, items: [], minuten: 0, kosten: 0 });
    const g = byPartij.get(h.partij);
    g.items.push({ ...h, aantal, totaalMinuten, kosten });
    g.minuten += totaalMinuten;
    g.kosten += kosten;
  }
  const order = ['school', 'duo'];
  return [...byPartij.values()].sort((a, b) => (order.indexOf(a.partij) + 9) % 9 - (order.indexOf(b.partij) + 9) % 9);
});
</script>

<style scoped>
.hl { display: flex; flex-direction: column; gap: var(--primitives-space-12); }
.hl-groep { display: flex; flex-direction: column; gap: var(--primitives-space-8); }
.hl-kop { display: flex; justify-content: space-between; align-items: baseline; gap: var(--primitives-space-8); }
.hl-partij { font-weight: 600; font-size: 0.9em; }
.hl-tot { font-size: 0.85em; color: var(--semantics-content-secondary-color); font-variant-numeric: tabular-nums; }
.hl-leeg { margin: 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.hl-toelichting { display: flex; flex-direction: column; gap: 2px; }
.hl-meta { color: var(--semantics-content-secondary-color); }
</style>
