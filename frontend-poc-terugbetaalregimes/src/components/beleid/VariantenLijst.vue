<template>
  <nldd-list appearance="box-tinted">
    <nldd-list-item size="sm">
      <nldd-text-cell
        :text="werkversie ? 'Huidig recht' : 'Huidig recht **(werkversie)**'"
        supporting-text="de wet zoals die nu geldt; de basis uit main"
      ></nldd-text-cell>
    </nldd-list-item>
    <nldd-list-item v-for="v in variants" :key="v.id" size="sm">
      <nldd-text-cell
        :text="werkversie === v.id ? `${variantMetSoort(kortTitel(v))} **(werkversie)**` : variantMetSoort(kortTitel(v))"
        :supporting-text="COST[v.id] ? `raming Stand van de Uitvoering: ${COST[v.id]} · branch ${v.branch ?? `variant/${v.id}`}` : `branch ${v.branch ?? `variant/${v.id}`}`"
      ></nldd-text-cell>
    </nldd-list-item>
  </nldd-list>
  <p class="vl-hint">Kies bovenaan welke varianten als kolom meerekenen, en in de balk bovenaan de werkversie waarop je bewerkt.</p>
  <p class="vl-hint">{{ VARIANT_UITLEG }} Niet te verwarren met een terugbetaalregime (SF15-oud, SF15-nieuw, SF15-LLLK, SF35): {{ REGIME_UITLEG.charAt(0).toLowerCase() + REGIME_UITLEG.slice(1) }}</p>
</template>

<!--
  Overzicht van de beleidsvarianten (branches) met de kostenraming uit de
  Stand van de Uitvoering OCW 2026. Kiezen gebeurt in de werkversiebalk, zodat
  er één plek is waar je bepaalt waarop je bewerkt.
-->

<script setup>
import { useLawStore, kortTitel } from '../../engine/lawStore.js';
import { variantMetSoort, VARIANT_UITLEG, REGIME_UITLEG, VARIANT_RAMING as COST } from '../../lib/regimeFacts.js';

const { variants, werkversie } = useLawStore();
</script>

<style scoped>
.vl-hint { margin: var(--primitives-space-8) 0 0; font-size: 0.85em; color: var(--semantics-content-secondary-color); }
</style>
