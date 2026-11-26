<template>
  <div class="tijdlijn">
    <nldd-table
      :columns="gridColumns"
      accessible-label="Peildata en bekostiging per kolom"
    >
      <nldd-inline-dialog slot="empty" text="Geen peildata"></nldd-inline-dialog>
      <nldd-table-row slot="header">
        <nldd-text-cell size="sm" text="Peildatum"></nldd-text-cell>
        <nldd-text-cell size="sm" :text="istTitel" supporting-text="telt · categorie · jaar · bedrag"></nldd-text-cell>
        <nldd-text-cell v-for="k in kolommen" :key="k.id" size="sm" :text="k.titel" supporting-text="telt · categorie · jaar · bedrag"></nldd-text-cell>
        <nldd-text-cell size="sm" text=""></nldd-text-cell>
      </nldd-table-row>

      <nldd-table-row v-for="i in zichtbaar" :key="peildata[i]" :selected="peildata[i] === geselecteerd ? true : undefined">
        <nldd-text-cell size="sm" :text="datumLabel(peildata[i])" :supporting-text="kwartaalLabel(peildata[i])" :color="ist[i]?.in_bestand ? 'content' : 'secondary'"></nldd-text-cell>
        <nldd-text-cell size="sm" :color="kleur(ist[i])">
          <p>
            <span class="tl-cel">
              <nldd-icon :icon="ist[i]?.telt ? 'check-mark-circle' : 'dismiss-circle'" size="16"></nldd-icon>
              <nldd-tag v-if="ist[i]?.telt" size="sm" :color="categorieColor(ist[i].categorie_effectief)" :text="categorieLabel(ist[i].categorie_effectief)"></nldd-tag>
              <span v-if="ist[i]?.telt" class="tl-jaar">jaar {{ ist[i].bekostigingsjaar }}</span>
              <span class="tl-bedrag">{{ ist[i]?.telt ? euro(ist[i].bedrag) : uitlegNietTellend(ist[i]) }}</span>
            </span>
          </p>
        </nldd-text-cell>
        <nldd-text-cell v-for="k in kolommen" :key="k.id" size="sm" :color="kleur(k.timeline?.[i])">
          <p>
            <span class="tl-cel">
              <nldd-icon :icon="k.timeline?.[i]?.telt ? 'check-mark-circle' : 'dismiss-circle'" size="16"></nldd-icon>
              <nldd-tag v-if="k.timeline?.[i]?.telt" size="sm" :color="categorieColor(k.timeline[i].categorie_effectief)" :text="categorieLabel(k.timeline[i].categorie_effectief)"></nldd-tag>
              <span v-if="k.timeline?.[i]?.telt" class="tl-jaar">jaar {{ k.timeline[i].bekostigingsjaar }}</span>
              <span class="tl-bedrag">{{ k.timeline?.[i]?.telt ? euro(k.timeline[i].bedrag) : uitlegNietTellend(k.timeline?.[i]) }}</span>
              <span v-if="verschil(k, i)" class="tl-delta" :class="verschil(k, i) > 0 ? 'tl-plus' : 'tl-min'">{{ euroDelta(verschil(k, i)) }}</span>
            </span>
          </p>
        </nldd-text-cell>
        <nldd-text-cell size="sm" horizontal-alignment="right">
          <p>
            <nldd-button
              text="Trace"
              size="xs"
              appearance="neutral-transparent"
              start-icon="text-document"
              @click="emit('trace', peildata[i])"
            ></nldd-button>
          </p>
        </nldd-text-cell>
      </nldd-table-row>

      <nldd-table-row class="tl-totaal">
        <nldd-text-cell size="sm" text="Totaal over de tijdlijn"></nldd-text-cell>
        <nldd-text-cell size="sm">
          <p><strong>{{ euro(totaal(ist)) }}</strong></p>
          <p slot="supporting-text">{{ kwartalen(ist) }} tellende peildata</p>
        </nldd-text-cell>
        <nldd-text-cell v-for="k in kolommen" :key="k.id" size="sm">
          <p><strong>{{ euro(totaal(k.timeline)) }}</strong></p>
          <p slot="supporting-text">{{ kwartalen(k.timeline) }} tellende peildata · {{ euroDelta(totaal(k.timeline) - totaal(ist)) }}</p>
        </nldd-text-cell>
        <nldd-text-cell size="sm" text=""></nldd-text-cell>
      </nldd-table-row>
    </nldd-table>
    <div v-if="verborgen > 0 || alles" class="tl-voet">
      <nldd-button
        size="xs"
        appearance="neutral-transparent"
        :text="alles ? 'Alleen de peildata die ertoe doen' : `Toon alle ${peildata.length} peildata (${verborgen} zonder inschrijving of recht verborgen)`"
        @click="alles = !alles"
      ></nldd-button>
    </div>
  </div>
</template>

<script setup>
import { ref, computed } from 'vue';
import { euro, euroDelta } from '../../lib/format.js';
import { datumLabel, kwartaalLabel, categorieLabel, categorieColor } from '../../lib/nieuwkomerFacts.js';

const props = defineProps({
  peildata: { type: Array, required: true },
  ist: { type: Array, required: true },
  /**
   * De regelingen naast huidig recht: [{ id, titel, timeline }]. Meerdere
   * tegelijk, zodat de impact van varianten op één leerling naast elkaar
   * staat in plaats van twee aan twee vergeleken te moeten worden.
   */
  kolommen: { type: Array, default: () => [] },
  istTitel: { type: String, default: 'Huidig recht' },
  geselecteerd: { type: String, default: null },
});
const emit = defineEmits(['trace']);

// Standaard alleen de peildata waarop de leerling in het bestand zit of telt
// (in een van beide kolommen); de rest is ruis van "nog niet ingeschreven".
const alles = ref(false);
const relevant = computed(() => props.peildata.map((_, i) => i).filter((i) => {
  const a = props.ist[i];
  if (a?.in_bestand || a?.telt) return true;
  // Een peildatum telt ook mee als hij in één van de gekozen regelingen iets
  // doet; anders verdwijnt juist het verschil dat je wilde zien.
  return props.kolommen.some((k) => k.timeline?.[i]?.in_bestand || k.timeline?.[i]?.telt);
}));
const zichtbaar = computed(() => (alles.value || relevant.value.length === 0 ? props.peildata.map((_, i) => i) : relevant.value));
const verborgen = computed(() => props.peildata.length - zichtbaar.value.length);

const gridColumns = computed(() => {
  const n = props.kolommen.length;
  if (!n) return 'minmax(150px, 0.8fr) minmax(260px, 2fr) 80px';
  // Naarmate er meer regelingen naast elkaar staan, mag elke kolom smaller;
  // onder de 200px wordt een cel met tag, jaar en bedrag onleesbaar.
  const breedte = n >= 3 ? 'minmax(200px, 1fr)' : 'minmax(260px, 1.4fr)';
  return `minmax(150px, 0.8fr) ${breedte} ${Array(n).fill(breedte).join(' ')} 80px`;
});

function kleur(u) {
  if (!u) return 'secondary';
  if (u.fout) return 'critical';
  return u.telt ? 'content' : 'secondary';
}

function uitlegNietTellend(u) {
  if (!u) return '—';
  if (u.fout) return 'fout';
  if (!u.in_bestand) return u.peildatum < (u.eerste_inschrijfdatum ?? '') ? 'nog niet ingeschreven' : 'niet in bestand';
  if (u.categorie === 'GEEN') return 'geen nieuwkomer';
  if (u.bekostigbare_kwartalen != null && u.kwartalen_verstreken != null && u.kwartalen_verstreken >= u.bekostigbare_kwartalen) {
    return `kwartalen op (${u.bekostigbare_kwartalen})`;
  }
  return 'telt niet';
}

function verschil(kolom, i) {
  return (kolom.timeline?.[i]?.bedrag ?? 0) - (props.ist[i]?.bedrag ?? 0);
}

function totaal(list) {
  return (list ?? []).reduce((s, u) => s + (u?.bedrag ?? 0), 0);
}
function kwartalen(list) {
  return (list ?? []).filter((u) => u?.telt).length;
}
</script>

<style scoped>
.tijdlijn { width: 100%; }
.tl-voet { display: flex; justify-content: flex-end; margin-top: var(--primitives-space-4); }
.tl-cel { display: inline-flex; align-items: center; gap: 6px; flex-wrap: wrap; }
.tl-jaar { font-size: 0.85em; color: var(--semantics-content-secondary-color); }
.tl-bedrag { font-variant-numeric: tabular-nums; }
.tl-delta { font-size: 0.85em; font-variant-numeric: tabular-nums; }
.tl-plus { color: var(--semantics-content-success-color); }
.tl-min { color: var(--semantics-content-critical-color); }
</style>
