<script setup>
// De reducties van de cel (RFC-048, RFC-051): hoe de cel haar kroniek
// terugleest om een benoemd antwoord te geven. Een reductie vult zelf geen
// begrip; beleid van wie beslist koppelt een open begrip aan zo'n antwoord.
// Per reductie haar tekst en haar YAML, de antwoorden die de cel eruit geeft
// (cel.yaml), welk beleidsartikel elk antwoord aan een begrip koppelt, het
// beleid dat zij voor een norm aanroept, en per invoer de rijen die zij uit
// de kroniek krijgt: de besluiten onder een artikel (zoals dat van de casus
// of Awb 7:11) of de executogrammen van een feit dat een artikel vereist
// (zoals de vestiging van een school). Ook de invoer van een reductie die
// zij aanroept, zoals die van de Awb, staat erbij: die leest uit dezelfde
// kroniek. Wat de cel bij een besluit antwoordde
// en waarom, staat bij dat besluit (tabblad Bestuursorgaan).
import { onMounted, ref, watch } from 'vue';
import { api } from '../api.js';
import { alsJson, leesbaar, state } from '../state.js';
import { artikelNaam, opsomming } from '../logboek.js';

const stand = ref(null);

// Alleen ophalen, geen stap: een fout van de vorige stap blijft dan staan.
async function laden() {
  try {
    stand.value = await api.reducties(state.datum);
  } catch (e) {
    state.fout = e.message;
  }
}

onMounted(laden);
watch(() => [state.grammen.length, state.datum], laden);

/**
 * Waar de reductie in het corpus van de casus staat: bij haar cel, of bij
 * geen cel, vanaf de wortel van dat corpus.
 */
const pad = (r) => {
  const map = state.casus.regelwerken.find((w) => w.id === r.id).map;
  return `${map}/${r.geldig_vanaf}.yaml`;
};

/** De koppelingen van beleid aan het antwoord `naam` van de reductie `r`. */
const koppelingen = (r, naam) =>
  stand.value.antwoorden.filter((k) => k.reduction === r.id && k.antwoord === naam);

/** Het beleid of de reductie die de reductie voor een norm aanroept, als tekst. */
const normen = (r) =>
  opsomming(
    r.normen.map((n) => `${n.laag === 'REDUCTIE' ? 'reductie' : 'beleid'} ${n.regelwerk} art. ${n.artikel}`),
  );
</script>

<template>
  <template v-if="stand">
    <nldd-simple-section>
      <nldd-title slot="header" size="3" :text="`Wat de cel van ${state.casus.cel_naam} uit haar kroniek antwoordt`">
        <nldd-text slot="supporting-text">
          Een reductie geeft een benoemd antwoord. Beleid van wie beslist koppelt een open begrip
          aan dat antwoord; dan vraagt de engine het aan de cel. Het bestuursorgaan krijgt die
          vraag niet; bij elk besluit staat wat de cel antwoordde.
        </nldd-text>
      </nldd-title>
    </nldd-simple-section>
    <template v-for="r in stand.reducties" :key="r.id">
      <nldd-simple-section>
        <nldd-title slot="header" size="3" :text="r.id">
          <nldd-text slot="supporting-text">
            Geen regelgeving: een regelwerk van de soort REDUCTIE van de houder, in de corpus
            ({{ pad(r) }}). Alleen een reductie leest een kroniek.
          </nldd-text>
        </nldd-title>
        <nldd-container gap="8">
          <nldd-text v-for="t in r.tekst" :key="t.artikel">{{ t.tekst }}</nldd-text>
        </nldd-container>
      </nldd-simple-section>
      <nldd-simple-section>
        <nldd-title slot="header" size="4" text="Antwoorden van de cel"></nldd-title>
        <nldd-list appearance="box-base" accessible-label="Antwoorden van de cel">
          <nldd-list-item v-for="naam in r.antwoorden" :key="naam" size="sm">
            <nldd-text-cell
              :text="naam"
              :supporting-text="koppelingen(r, naam).length
                ? koppelingen(r, naam)
                    .map((k) => `beleid ${k.beleid} art. ${k.artikel} koppelt het aan ${leesbaar(k.term.open_term)} (${artikelNaam(k.term.law, k.term.article)})`)
                    .join('; ')
                : 'geen beleid koppelt dit antwoord nu aan een begrip'"
            ></nldd-text-cell>
          </nldd-list-item>
        </nldd-list>
        <nldd-text v-if="r.normen.length" size="sm" color="secondary">
          Voor een norm roept de reductie {{ normen(r) }} aan; wat zij uit de kroniek haalt, geeft
          zij mee als parameter.
        </nldd-text>
      </nldd-simple-section>
      <nldd-simple-section>
        <nldd-title slot="header" size="4" text="Zoals geschreven"></nldd-title>
        <nldd-code-viewer language="yaml" appearance="box-tinted">{{ r.yaml }}</nldd-code-viewer>
      </nldd-simple-section>
      <nldd-simple-section v-for="i in r.invoer" :key="`${i.reductie}-${i.naam}`">
        <nldd-title
          slot="header"
          size="4"
          :text="i.reductie === r.id ? `Invoer ${i.naam}` : `Invoer ${i.naam} van ${i.reductie}`"
          :supporting-text="i.executogrammen_van
            ? `${i.rijen.length} rij(en): één per executogram onder ${i.executogrammen_van} in de kroniek, met zijn gegevens en zijn id.`
            : `${i.rijen.length} rij(en): één per besluit onder ${i.besluiten_onder} in de kroniek, met de gegevens en de uitvoer van dat besluit, zijn artikel en zijn id.`"
        ></nldd-title>
        <nldd-code-viewer language="json" appearance="box-tinted">{{ alsJson(i.rijen) }}</nldd-code-viewer>
      </nldd-simple-section>
    </template>
  </template>
</template>
