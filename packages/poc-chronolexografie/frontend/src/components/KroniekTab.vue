<script setup>
// De kroniek van de cel: elk chronolexogram, in de volgorde waarin het werd
// vastgelegd, en wat erin staat (RFC-046). Daaronder de kronieken van de
// andere cellen van de casus (RFC-052): elke cel houdt de hare bij; het
// bestuursorgaan vraagt ze, maar schrijft er niet in. De rechtbank is er
// een van (RFC-053).
//
// Elk chronolexogram klapt open in zijn eigen regel (een nldd-list van type
// tree): waarnaar het verwijst (`refers_to`), wat het van een andere cel
// ontving (`received`) en na welk chronolexogram het opnieuw is genomen
// (`after`), welke bevindingen een besluit gebruikte, en zijn gegevens en
// uitvoer. Het geheel als JSON, met decimalen als getal, staat in een sheet.
// Welke bevindingen een besluit gebruikte, staat niet in de kroniek: de cel
// leidt het af uit het besluit opnieuw genomen (Cell::findings, RFC-050).
// Wat uit een aanvraag, bezwaar of beroep volgt, zoals de laatste dag van de
// beslistermijn, staat er ook niet in: de cel rekent het uit bij het
// openklappen (Cell::submission_outputs).
import { computed, nextTick, reactive, ref, watch } from 'vue';
import { api } from '../api.js';
import { beroepApi, herkomst } from '../beroep.js';
import { celNaam, toon } from '../logboek.js';
import { isBesluit, kort, leesbaar, rol, state } from '../state.js';
import GramCode from './GramCode.vue';

// De kroniek van de cel van de casus, dan die van de andere cellen. Waarom
// een besluit zo uitviel, vraagt de app aan de cel van de casus en aan de
// rechtbank; de redenering van een andere cel is de hare (RFC-048).
const kronieken = computed(() => [
  { naam: state.casus.cel_naam, grammen: state.grammen, eigen: true, rechter: false },
  ...state.andere.map((a) => ({
    naam: a.cel_naam,
    grammen: a.grammen,
    eigen: false,
    rechter: a.cel === state.casus.rechter?.cel,
  })),
]);
const vind = (id) => {
  for (const k of kronieken.value) {
    const g = k.grammen.find((x) => x.id === id);
    if (g) return { gram: g, eigen: k.eigen, rechter: k.rechter };
  }
  return null;
};

// Welke chronolexogrammen open zijn, en per besluit de bevindingen die het
// gebruikte, opgehaald bij het openklappen.
const open = ref(new Set());
const bevindingen = reactive({});
const gevolgen = reactive({});
const isOpen = (id) => open.value.has(id);

async function laadBevindingen(id) {
  const gevonden = vind(id);
  if (!gevonden || !(gevonden.eigen || gevonden.rechter) || !isBesluit(gevonden.gram)) return;
  if (bevindingen[id]) return;
  try {
    const antwoord = await (gevonden.rechter ? beroepApi.waarom(id) : api.waarom(id));
    bevindingen[id] = antwoord.bevindingen;
  } catch (e) {
    // De regel zegt dat het niet lukte; opnieuw openklappen probeert het weer.
    bevindingen[id] = false;
    state.fout = e.message;
  }
}

const isIndiening = (g) => ['AANVRAAG', 'BEZWAAR', 'BEROEP'].includes(g.kind);

async function laadGevolgen(id) {
  const g = vind(id)?.gram;
  if (!g || !isIndiening(g) || gevolgen[id]) return;
  try {
    gevolgen[id] = await api.watVolgt(id);
  } catch (e) {
    gevolgen[id] = false;
    state.fout = e.message;
  }
}

function toggle(id) {
  const nieuw = new Set(open.value);
  if (nieuw.has(id)) nieuw.delete(id);
  else {
    nieuw.add(id);
    laadBevindingen(id);
    laadGevolgen(id);
  }
  open.value = nieuw;
}

// Naar een ander chronolexogram: open het en breng het in beeld.
async function ga(id) {
  if (!vind(id)) return;
  if (!isOpen(id)) toggle(id);
  await nextTick();
  document.getElementById(`gram-${id}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
}

// Vanuit het logboek: dat chronolexogram staat open, zodra het er is.
const gevraagd = ref(state.inKroniek);
state.inKroniek = null;
watch(
  [gevraagd, kronieken],
  ([id]) => {
    if (id && vind(id)) {
      gevraagd.value = null;
      ga(id);
    }
  },
  { immediate: true },
);

const anker = (id) => {
  const g = vind(id)?.gram;
  return g ? `${rol(g)} ${kort(g.id)} · ${g.basis}` : kort(id);
};

/** De regels onder een opengeklapt chronolexogram. */
function regels(k, g) {
  const uit = [
    {
      naam: 'Vastgelegd',
      waarde: `${g.recorded_on} · ${celNaam(g.recording_actor)}`,
      toelichting: `${g.kind} onder ${g.basis}${g.fact ? `, feit ${leesbaar(g.fact)}` : ''}`,
    },
  ];
  const doel = (id) => (vind(id) ? id : null);
  if (g.refers_to) uit.push({ naam: 'Verwijst naar', waarde: anker(g.refers_to), naar: doel(g.refers_to) });
  if (g.received) {
    uit.push({
      naam: 'Ontvangen van',
      waarde: `${g.received.from} · ${anker(g.received.id)}`,
      naar: doel(g.received.id),
    });
  }
  if (g.after) uit.push({ naam: 'Opnieuw genomen na', waarde: anker(g.after), naar: doel(g.after) });
  if ((k.eigen || k.rechter) && isBesluit(g)) {
    const wie = k.rechter ? 'de rechter' : 'het bestuursorgaan';
    const lijst = bevindingen[g.id] || [];
    if (lijst.length === 0) {
      const stand = bevindingen[g.id];
      const waarde = stand === false ? 'niet opgehaald, zie de melding' : stand ? 'geen' : '…';
      uit.push({ naam: `Bevindingen van ${wie}`, waarde });
    }
    for (const f of lijst) {
      uit.push({ naam: leesbaar(f), waarde: toon(g.fields?.[f]), toelichting: `bevinding van ${wie} (RFC-050)` });
    }
  }
  const gebruikt = new Set(bevindingen[g.id] || []);
  for (const [naam, w] of Object.entries(g.fields ?? {})) {
    if (!gebruikt.has(naam)) uit.push({ naam: leesbaar(naam), waarde: toon(w), toelichting: 'gegeven' });
  }
  for (const [naam, w] of Object.entries(g.outputs ?? {})) {
    uit.push({ naam: leesbaar(naam), waarde: toon(w), toelichting: 'uitvoer' });
  }
  if (gevolgen[g.id] === false) uit.push({ naam: 'Volgt uit de wet', waarde: 'niet uitgerekend, zie de melding' });
  for (const [naam, w] of Object.entries(gevolgen[g.id] || {})) {
    uit.push({ naam: leesbaar(naam), waarde: toon(w), toelichting: 'volgt uit de wet, niet vastgelegd' });
  }
  return uit;
}

// Het hele chronolexogram als JSON, in een sheet.
const json = ref(null);
</script>

<template>
  <nldd-simple-section v-for="k in kronieken" :key="k.naam">
    <nldd-title slot="header" size="3" :text="`Kroniek van ${k.naam}`">
      <nldd-text v-if="k.eigen" slot="supporting-text">
        Eén kroniek, van de cel van de uitvoerder. Elke stap in het proces legt er één
        chronolexogram in vast (een beslissing op bezwaar die herroept, legt er twee vast); er wordt
        nooit iets in gewijzigd of uit verwijderd.<template v-if="state.casus?.begin"> Bovenaan
        staat wat de cel vóór het eerste scenario vastlegde, zoals de vestiging van een school
        (RFC-051).</template> Klik op een chronolexogram om te zien wat erin staat.
      </nldd-text>
      <nldd-text v-else-if="k.rechter" slot="supporting-text">
        De kroniek van de rechtbank, een andere cel. Een beroep verwijst niet naar het bestreden
        besluit, want dat staat in een andere kroniek: het draagt wat de rechtbank ontving.
      </nldd-text>
      <nldd-text v-else slot="supporting-text">Een andere cel van de casus, met haar eigen kroniek.</nldd-text>
    </nldd-title>
    <nldd-inline-dialog
      v-if="k.grammen.length === 0"
      icon="info"
      text="De kroniek is leeg"
      :supporting-text="k.eigen ? 'Dien als aanvrager een aanvraag in om het eerste chronolexogram te maken.' : k.rechter ? 'Stel als aanvrager beroep in tegen een beslissing op bezwaar.' : undefined"
    ></nldd-inline-dialog>
    <nldd-list v-else type="tree" appearance="box-base" :accessible-label="`Chronolexogrammen van ${k.naam}`">
      <nldd-list-item
        v-for="(g, i) in k.grammen"
        :id="`gram-${g.id}`"
        :key="g.id"
        button
        :expanded="isOpen(g.id)"
        @click="toggle(g.id)"
      >
        <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell
          :overline="`${i + 1} · ${g.recorded_on}`"
          :text="`${g.kind} · ${g.basis}`"
          :supporting-text="[kort(g.id), herkomst(g)].filter(Boolean).join(' · ')"
        ></nldd-text-cell>
        <!-- De regels staan er altijd; `expanded` verbergt ze. Zo blijft
             elke rij een tak, die pijl-rechts openklapt. -->
        <nldd-list-item
          v-for="(r, j) in regels(k, g)"
          :key="`${r.naam}-${j}`"
          slot="children"
          size="sm"
          :button="!!r.naar || undefined"
          @click.stop="r.naar && ga(r.naar)"
        >
          <nldd-spacer-cell size="24"></nldd-spacer-cell>
          <nldd-text-cell :text="r.naam" :supporting-text="r.toelichting"></nldd-text-cell>
          <nldd-text-cell width="fit-content" :color="r.naar ? 'accent' : undefined" :text="r.waarde"></nldd-text-cell>
        </nldd-list-item>
        <nldd-list-item slot="children" size="sm" @click.stop>
          <nldd-spacer-cell size="24"></nldd-spacer-cell>
          <nldd-list-item-segment button @click.stop="json = g">
            <nldd-text-cell color="accent" text="Als JSON bekijken"></nldd-text-cell>
          </nldd-list-item-segment>
        </nldd-list-item>
      </nldd-list-item>
    </nldd-list>
  </nldd-simple-section>

  <Teleport to="body">
    <nldd-sheet width="720px" :open="json ? true : undefined" @close="json = null">
      <nldd-page>
        <nldd-container slot="header" padding="16">
          <nldd-top-title-bar
            heading-level="2"
            :text="json ? `Chronolexogram ${kort(json.id)}` : ''"
            supporting-text="Zoals de kroniek het bewaart"
            dismiss-text="Sluiten"
          ></nldd-top-title-bar>
        </nldd-container>
        <nldd-simple-section>
          <GramCode v-if="json" :gram="json" />
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
