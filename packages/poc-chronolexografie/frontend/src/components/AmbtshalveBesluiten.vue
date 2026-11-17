<script setup>
// Decisions the case's cell takes ex officio, without a submission
// (`casus.ambtshalve`, derived from the law), such as setting the statutory
// interest a payment owes: the authority picks the article, the decision it
// concerns where there is one, and gives what the article asks
// (`state.vormen.besluiten[artikel]`). Following a scenario, the step's
// article, decision and values are filled in (scenarioInvoer.js). Below
// the form stand the decisions taken this way.
import { computed, reactive, watch } from "vue";
import { api } from "../api.js";
import {
  ambtshalveArtikel,
  formulier,
  huidigeStap,
  scenarioOver,
  scenarioWaarden,
} from "../scenarioInvoer.js";
import {
  besluitenOnder,
  isBesluitSoort,
  kort,
  leesbaar,
  longDate,
  state,
  stap,
  waarde,
  zonderLeeg,
} from "../state.js";
import ArtikelLink from "./ArtikelLink.vue";
import VeldInvoer from "./VeldInvoer.vue";

const artikelen = computed(() =>
  (state.casus.ambtshalve ?? [])
    .filter((a) => a.cel === state.casus.cel)
    .map((a) => a.artikel),
);

// The article chosen; at an ex officio step of a scenario, the step's own.
// What the visitor chose holds until the scenario moves to its next step.
// Without either, nothing is chosen and the form asks for a choice first
// (ambtshalveArtikel): the first article is no default.
const keuze = reactive({ artikel: null, over: {} });
watch(
  () => huidigeStap()?.index,
  () => {
    keuze.artikel = null;
    keuze.over = {};
  },
);
const artikel = computed(() =>
  ambtshalveArtikel(keuze.artikel, artikelen.value),
);
const vorm = computed(() => state.vormen?.besluiten?.[artikel.value] ?? null);

const besluitVorm = formulier((a) =>
  scenarioWaarden("ambtshalve", { artikel: a }),
);
// A switch that is off says no, as on the form of a decision on an
// application; a finding that may stay empty says nothing untouched.
const gegeven = (a) => ({
  ...Object.fromEntries(
    (vorm.value?.bevindingen ?? [])
      .filter((o) => o.type === "boolean" && !o.mag_leeg)
      .map((o) => [o.naam, false]),
  ),
  ...zonderLeeg(besluitVorm.waarden(a)),
});

// The decisions of the case's cell an ex officio decision can concern.
// Every decision of the cell, whatever its article; a scenario's reference
// that is none of these is offered too, so the form shows what it submits.
const besluiten = computed(() => state.grammen.filter(isBesluitSoort));
const extra = (a) => {
  const id = over(a);
  return id && !besluiten.value.some((b) => b.id === id) ? [id] : [];
};
const over = (a) => (a in keuze.over ? keuze.over[a] : (scenarioOver(a) ?? ""));

async function nemen() {
  const a = artikel.value;
  if (!a) return;
  const ok = await stap(() =>
    api.ambtshalve(a, gegeven(a), state.datum, over(a) || null),
  );
  if (ok) {
    besluitVorm.wis(a);
    delete keuze.over[a];
  }
}

const genomen = computed(() =>
  artikelen.value.flatMap((a) => besluitenOnder(a)),
);
</script>

<template>
  <nldd-container gap="16">
    <nldd-card>
      <nldd-container padding="16" gap="12">
        <nldd-form-field
          label="Besluit"
          supporting-label="Het artikel waaronder het bestuursorgaan ambtshalve besluit"
        >
          <nldd-dropdown>
            <select
              :value="artikel ?? ''"
              @change="keuze.artikel = $event.target.value || null"
            >
              <option value="" disabled>Kies een besluit</option>
              <option v-for="a in artikelen" :key="a" :value="a">
                {{ a }}
              </option>
            </select>
          </nldd-dropdown>
        </nldd-form-field>
        <template v-if="artikel">
          <nldd-text size="sm" color="secondary"
            ><ArtikelLink :basis="artikel"
          /></nldd-text>
          <nldd-form-field
            label="Over besluit"
            supporting-label="Het besluit waarover dit besluit gaat, of geen"
            optional
          >
            <nldd-dropdown>
              <select
                :value="over(artikel)"
                @change="keuze.over[artikel] = $event.target.value"
              >
                <option value="">Geen</option>
                <option v-for="id in extra(artikel)" :key="id" :value="id">
                  Record {{ kort(id) }}
                </option>
                <option v-for="b in besluiten" :key="b.id" :value="b.id">
                  Besluit {{ kort(b.id) }} van {{ longDate(b.recorded_on) }}
                </option>
              </select>
            </nldd-dropdown>
          </nldd-form-field>
          <VeldInvoer
            v-for="o in vorm?.bevindingen ?? []"
            :key="o.naam"
            :veld="o"
            :model-value="besluitVorm.waarden(artikel)[o.naam]"
            @update:model-value="besluitVorm.zet(artikel, o.naam, $event)"
          />
          <nldd-button
            appearance="primary"
            text="Ambtshalve besluiten"
            @click="nemen"
          ></nldd-button>
        </template>
      </nldd-container>
    </nldd-card>
    <nldd-card v-for="g in genomen" :key="g.id">
      <nldd-container padding="16" gap="12">
        <nldd-title
          heading-level="3"
          size="display-5"
          :text="`Besluit ${kort(g.id)}`"
          :supporting-text="`ambtshalve genomen op ${longDate(g.recorded_on)}${g.refers_to ? ` · over besluit ${kort(g.refers_to)}` : ''}`"
        ></nldd-title>
        <nldd-text size="sm" color="secondary"
          ><ArtikelLink :basis="g.basis" :datum="g.recorded_on"
        /></nldd-text>
        <nldd-list appearance="box-base">
          <nldd-list-item
            v-for="(w, naam) in g.outputs ?? {}"
            :key="naam"
            size="sm"
          >
            <nldd-text-cell :text="leesbaar(naam)"></nldd-text-cell>
            <nldd-text-cell
              width="fit-content"
              :text="waarde(w)"
            ></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
      </nldd-container>
    </nldd-card>
  </nldd-container>
</template>
