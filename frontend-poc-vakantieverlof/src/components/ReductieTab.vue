<script setup>
// De reductie van de cel (RFC-048): hoe de school haar kroniek terugleest om
// te weten of de jongere dit schooljaar al verlof kreeg. Bovenaan wat de
// school zo beantwoordt (Cell::answers); dan de reductie als geschreven,
// met per invoer de rijen die zij uit de kroniek krijgt: de besluiten onder
// artikel 13a en de beslissingen op bezwaar (Awb 7:11). Wat zij bij een
// besluit antwoordde en waarom, staat bij dat besluit (tabblad School).
import { onMounted, ref, watch } from 'vue';
import { api } from '../api.js';
import { alsJson, leesbaar, spel } from '../spel.js';

const stand = ref(null);

// Alleen ophalen, geen stap: een fout van de vorige stap blijft dan staan.
async function laden() {
  try {
    stand.value = await api.reducties(spel.datum);
  } catch (e) {
    spel.fout = e.message;
  }
}

onMounted(laden);
watch(() => [spel.grammen.length, spel.datum], laden);
</script>

<template>
  <template v-if="stand">
    <nldd-simple-section>
      <nldd-title slot="header" size="3" text="Deze school beantwoordt uit haar kroniek">
        <nldd-text slot="supporting-text">
          Een open begrip dat een reductie van de school vult, vraagt de engine aan de cel. Het
          hoofd krijgt die vraag niet; bij elk besluit staat wat de school antwoordde.
        </nldd-text>
      </nldd-title>
      <nldd-list appearance="box-base" accessible-label="Wat de school beantwoordt">
        <nldd-list-item v-for="a in stand.antwoorden" :key="`${a.reduction}@${a.valid_from}`" size="sm">
          <nldd-text-cell
            :text="leesbaar(a.term.open_term)"
            :supporting-text="`${a.term.law} artikel ${a.term.article} · reductie ${a.reduction}, geldig vanaf ${a.valid_from}`"
          ></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
    </nldd-simple-section>
    <template v-for="r in stand.reducties" :key="r.id">
      <nldd-simple-section>
        <nldd-title slot="header" size="3" :text="r.id">
          <nldd-text slot="supporting-text">
            Geen regelgeving: een regelwerk van de soort REDUCTIE dat bij de cel hoort
            (corpus/cells/obs_de_klimop/reducties/{{ r.id }}/{{ r.geldig_vanaf }}.yaml). Waar
            de engine het begrip leest, ook als Awb 7:11 artikel 13a opnieuw toepast, draait de
            cel haar op de kroniek.
          </nldd-text>
        </nldd-title>
        <nldd-code-viewer language="yaml" appearance="box-tinted">{{ r.yaml }}</nldd-code-viewer>
      </nldd-simple-section>
      <nldd-simple-section v-for="i in r.invoer" :key="i.naam">
        <nldd-title
          slot="header"
          size="4"
          :text="`Invoer ${i.naam}`"
          :supporting-text="`${i.rijen.length} rij(en): één per besluit onder ${i.besluiten_onder} in de kroniek, met de gegevens en de uitvoer van dat besluit en zijn id.`"
        ></nldd-title>
        <nldd-code-viewer language="json" appearance="box-tinted">{{ alsJson(i.rijen) }}</nldd-code-viewer>
      </nldd-simple-section>
    </template>
  </template>
</template>
