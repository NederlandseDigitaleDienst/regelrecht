<script setup>
// Het logboek: elke handeling in de kroniek, met de feiten die de andere
// cellen vastlegden (executogrammen), de nieuwste bovenaan. Elke handeling
// klapt open in haar eigen regel (een nldd-list van type tree): waarom zij er
// is en wat er gebeurde, en bij een besluit de keten als takken eronder.
// Niets hiervan is apart opgeslagen. De handelingen zijn de chronolexogrammen;
// de reden is waarnaar een chronolexogram verwijst (`refers_to`); de keten van
// een besluit komt uit het besluit opnieuw genomen (`api.logboek`). Een stap
// die de cel weigerde, legde niets vast en staat er dus niet in.
import { nextTick, reactive, ref, watch } from 'vue';
import { api } from '../api.js';
import { celNaam, eigenAntwoord, genomenBij, genummerd, toon, uitslag } from '../logboek.js';
import { herroepen, kort, leesbaar, rol, state } from '../state.js';
import HoeBeantwoord from './HoeBeantwoord.vue';
import KetenStap from './KetenStap.vue';

const emit = defineEmits(['kroniek']);

const handelingen = ref([]);
const geladen = ref(false);
const mislukt = ref(false);
let verzoek = 0;
// Welke handelingen open zijn; na het eerste laden de nieuwste.
const open = ref(new Set());
let eerste = true;

async function laden() {
  const dit = ++verzoek;
  try {
    const antwoord = await api.logboek();
    // Alleen het antwoord op het laatste verzoek telt.
    if (dit !== verzoek) return;
    handelingen.value = antwoord.handelingen;
  } catch (e) {
    if (dit !== verzoek) return;
    // De banner zegt wat er misging; het logboek toont dan niets.
    state.fout = e.message;
    handelingen.value = [];
    mislukt.value = true;
    return;
  }
  geladen.value = true;
  mislukt.value = false;
  if (eerste && handelingen.value.length) {
    open.value = new Set([handelingen.value[0].gram.id]);
    eerste = false;
  }
}
// Ook een feit dat een andere cel vastlegt, zoals een inschrijving bij de
// Kamer van Koophandel, staat in het logboek.
watch(() => [state.grammen, state.andere], laden, { immediate: true });

const isOpen = (id) => open.value.has(id);
function toggle(id) {
  const nieuw = new Set(open.value);
  if (nieuw.has(id)) nieuw.delete(id);
  else nieuw.add(id);
  open.value = nieuw;
}
// Naar een andere handeling: open haar en breng haar in beeld.
async function ga(id) {
  if (!isOpen(id)) toggle(id);
  await nextTick();
  document.getElementById(`stap-${id}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
}

const gram = (id) => handelingen.value.find((h) => h.gram.id === id)?.gram;
const naam = (id) => {
  const g = gram(id);
  return g ? `${rol(g).toLowerCase()} ${kort(id)}` : kort(id);
};
const dag = (g) => g.recorded_on.split('-').reverse().join('-');

/** De regels onder een opengeklapte handeling, vóór de keten. */
function regels(h) {
  const g = h.gram;
  const uit = [];
  if (g.refers_to) uit.push({ naam: 'Reden', waarde: naam(g.refers_to), naar: g.refers_to });
  else if (g.kind === 'EXECUTOGRAM') {
    uit.push({
      naam: 'Reden',
      waarde: 'geen',
      toelichting: `${celNaam(g.recording_actor)} legde het feit ${leesbaar(g.fact ?? '')} vast, dat ${g.basis} vereist (een executogram, RFC-051). Een reductie van die cel leest het terug.`,
    });
  } else uit.push({ naam: 'Reden', waarde: 'geen', toelichting: 'dit is het begin van een dossier' });
  const bij = genomenBij(g.id, state.grammen);
  if (bij) {
    uit.push({
      naam: 'Genomen in',
      waarde: naam(bij.id),
      naar: bij.id,
      toelichting: 'in de plaats van het herroepen besluit',
    });
  }
  if (h.leidde_tot.length === 0) uit.push({ naam: 'Leidde tot', waarde: 'nog niets' });
  for (const id of h.leidde_tot) uit.push({ naam: 'Leidde tot', waarde: naam(id), naar: id });
  for (const f of h.waarom?.bevindingen ?? []) {
    uit.push({ naam: leesbaar(f), waarde: toon(g.fields[f]), toelichting: 'bevinding van het bestuursorgaan' });
  }
  if (!h.waarom && g.kind !== 'EXECUTOGRAM') {
    uit.push({
      naam: 'Geen besluit',
      toelichting:
        'De cel legde vast wat de aanvrager indiende. Wat het artikel daarbij uitrekende, zoals de laatste dag van de termijn, staat in de kroniek.',
    });
  }
  return uit;
}

// Het volledige spoor van een besluit, in een sheet.
const spoor = ref(null);

// De uitleg van de cel bij een begrip, in een sheet.
const sheet = reactive({ open: false, knoop: null, uitleg: null, besluit: null });
async function uitleg(besluit, knoop) {
  Object.assign(sheet, { open: true, knoop, uitleg: null, besluit });
  try {
    const antwoord = await api.hoeBeantwoord(besluit, eigenAntwoord(knoop.stap));
    // Alleen als intussen niet een ander begrip of besluit gekozen is.
    if (sheet.knoop?.nummer === knoop.nummer && sheet.besluit === besluit) {
      sheet.uitleg = antwoord.antwoorden[knoop.stap.keer] ?? null;
    }
  } catch (e) {
    state.fout = e.message;
    sheet.open = false;
  }
}
</script>

<template>
  <nldd-simple-section v-if="!geladen && !mislukt">
    <nldd-inline-dialog variant="loading" text="Het logboek wordt geladen"></nldd-inline-dialog>
  </nldd-simple-section>
  <nldd-simple-section v-else-if="!mislukt && handelingen.length === 0">
    <nldd-inline-dialog
      icon="info"
      text="Het logboek is leeg"
      supporting-text="Dien als aanvrager een aanvraag in. Elke stap die de cel vastlegt, komt hier te staan."
    ></nldd-inline-dialog>
  </nldd-simple-section>
  <nldd-simple-section v-else-if="handelingen.length">
    <nldd-title slot="header" size="3" text="Logboek">
      <nldd-text slot="supporting-text">
        Wat er gebeurde, de nieuwste stap bovenaan. Elke stap is een chronolexogram uit de
        kroniek. Ook een feit dat een andere cel vastlegde, zoals een inschrijving bij de Kamer
        van Koophandel of een onderzoek van de GGD, staat erin; besluiten van een andere cel
        niet, want hun waarom is van die cel. Klik op een stap om te zien waarom zij er is. Bij
        een besluit staat ook waarom het zo uitviel: de cel neemt het opnieuw en laat zien welke
        artikelen meeliepen en wie elk open begrip invulde. Een stap die de cel weigerde, legde
        niets vast en staat hier dus niet.
      </nldd-text>
    </nldd-title>
    <nldd-list type="tree" appearance="box-base" accessible-label="Handelingen">
      <nldd-list-item
        v-for="h in handelingen"
        :id="`stap-${h.gram.id}`"
        :key="h.gram.id"
        button
        :expanded="isOpen(h.gram.id)"
        @click="toggle(h.gram.id)"
      >
        <nldd-icon-cell disclosure icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
        <nldd-spacer-cell size="8"></nldd-spacer-cell>
        <nldd-text-cell
          :overline="`${dag(h.gram)} · ${celNaam(h.gram.recording_actor)}`"
          :text="`${rol(h.gram)} ${kort(h.gram.id)}`"
          :supporting-text="h.gram.basis"
        ></nldd-text-cell>
        <nldd-cell>
          <nldd-badge size="sm" :color="uitslag(h.gram).kleur" :text="uitslag(h.gram).tekst"></nldd-badge>
        </nldd-cell>
        <nldd-spacer-cell v-if="herroepen(h.gram)" size="4"></nldd-spacer-cell>
        <nldd-cell v-if="herroepen(h.gram)">
          <nldd-badge size="sm" color="warning" text="herroepen"></nldd-badge>
        </nldd-cell>
        <template v-if="isOpen(h.gram.id)">
          <nldd-list-item
            v-for="(r, i) in regels(h)"
            :key="`${r.naam}-${i}`"
            slot="children"
            size="sm"
            :button="!!r.naar || undefined"
            @click.stop="r.naar && ga(r.naar)"
          >
            <nldd-spacer-cell size="24"></nldd-spacer-cell>
            <nldd-text-cell :text="r.naam" :supporting-text="r.toelichting"></nldd-text-cell>
            <nldd-text-cell
              v-if="r.waarde"
              width="fit-content"
              :color="r.naar ? 'accent' : undefined"
              :text="r.waarde"
            ></nldd-text-cell>
          </nldd-list-item>
          <KetenStap
            v-for="k in genummerd(h.waarom?.keten ?? [])"
            :key="k.nummer"
            slot="children"
            :knoop="k"
            :diepte="1"
            @uitleg="uitleg(h.gram.id, $event)"
          />
          <nldd-list-item slot="children" size="sm">
            <nldd-spacer-cell size="24"></nldd-spacer-cell>
            <nldd-list-item-segment button @click.stop="emit('kroniek', h.gram.id)">
              <nldd-text-cell color="accent" :text="`${h.gram.kind} ${kort(h.gram.id)} in de kroniek`"></nldd-text-cell>
            </nldd-list-item-segment>
            <nldd-list-item-segment v-if="h.waarom" button @click.stop="spoor = h">
              <nldd-text-cell color="accent" text="Volledig spoor"></nldd-text-cell>
            </nldd-list-item-segment>
          </nldd-list-item>
        </template>
      </nldd-list-item>
    </nldd-list>
  </nldd-simple-section>

  <Teleport to="body">
    <nldd-sheet width="720px" :open="sheet.open || undefined" @close="sheet.open = false">
      <nldd-page>
        <nldd-container slot="header" padding="16">
          <nldd-top-title-bar
            heading-level="2"
            text="Hoe de cel antwoordde"
            :supporting-text="sheet.knoop ? leesbaar(sheet.knoop.stap.naam) : ''"
            dismiss-text="Sluiten"
          ></nldd-top-title-bar>
        </nldd-container>
        <nldd-simple-section>
          <HoeBeantwoord v-if="sheet.uitleg" :uitleg="sheet.uitleg" :besluit="sheet.besluit" />
          <nldd-text v-else size="sm" color="secondary">Even geduld…</nldd-text>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
    <nldd-sheet width="720px" :open="spoor ? true : undefined" @close="spoor = null">
      <nldd-page>
        <nldd-container slot="header" padding="16">
          <nldd-top-title-bar
            heading-level="2"
            text="Volledig spoor"
            :supporting-text="spoor ? `${rol(spoor.gram)} ${kort(spoor.gram.id)}` : ''"
            dismiss-text="Sluiten"
          ></nldd-top-title-bar>
        </nldd-container>
        <nldd-simple-section>
          <nldd-text size="sm" color="secondary">
            De keten in het logboek laat rekenwerk en het opzoeken van waarden weg; hier staat het
            hele spoor van het besluit opnieuw genomen.
          </nldd-text>
          <nldd-code-viewer v-if="spoor" appearance="box-tinted">{{ spoor.waarom.spoor }}</nldd-code-viewer>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
