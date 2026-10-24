<script setup>
// Het logboek: elke handeling in de kroniek, de nieuwste bovenaan, en bij de
// gekozen handeling waarom zij er is en wat er gebeurde. Niets hiervan is
// apart opgeslagen. De handelingen zijn de chronolexogrammen; de reden is
// waarnaar een chronolexogram verwijst (`refers_to`); de keten van een
// besluit komt uit het besluit opnieuw genomen (`api.logboek`). Een stap
// die de cel weigerde, legde niets vast en staat er dus niet in.
import { computed, reactive, ref, watch } from 'vue';
import { api } from '../api.js';
import { eigenAntwoord, genomenBij, genummerd, toon, uitslag } from '../logboek.js';
import { herroepen, kort, leesbaar, rol, state } from '../state.js';
import HoeBeantwoord from './HoeBeantwoord.vue';
import KetenStap from './KetenStap.vue';

const emit = defineEmits(['kroniek']);

const handelingen = ref([]);
const geladen = ref(false);
const mislukt = ref(false);
let verzoek = 0;
const gekozen = ref(null);
const spoor = ref(false);

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
  if (!handelingen.value.some((h) => h.gram.id === gekozen.value)) {
    gekozen.value = handelingen.value[0]?.gram.id ?? null;
  }
}
watch(() => state.grammen, laden, { immediate: true });
watch(gekozen, () => (spoor.value = false));

const handeling = computed(() => handelingen.value.find((h) => h.gram.id === gekozen.value) ?? null);
const keten = computed(() => genummerd(handeling.value?.waarom?.keten ?? []));
const gram = (id) => state.grammen.find((g) => g.id === id);
const naam = (id) => {
  const g = gram(id);
  return g ? `${rol(g).toLowerCase()} ${kort(id)}` : kort(id);
};
const dag = (g) => g.recorded_on.split('-').reverse().join('-');
const bevindingen = computed(() =>
  (handeling.value?.waarom?.bevindingen ?? []).map((f) => ({
    naam: f,
    waarde: handeling.value.gram.fields[f],
  })),
);
const bij = computed(() => (handeling.value ? genomenBij(handeling.value.gram.id, state.grammen) : null));

// De uitleg van de cel bij een begrip, in een sheet.
const sheet = reactive({ open: false, knoop: null, uitleg: null, besluit: null });
async function uitleg(knoop) {
  const besluit = gekozen.value;
  Object.assign(sheet, { open: true, knoop, uitleg: null, besluit });
  try {
    const antwoord = await api.hoeBeantwoord(besluit, eigenAntwoord(knoop.stap));
    // Alleen als intussen niet een ander begrip of besluit gekozen is.
    if (sheet.knoop?.nummer === knoop.nummer && gekozen.value === besluit) {
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
  <nldd-side-by-side-split-view v-else-if="handelingen.length" panes="2">
    <nldd-page slot="pane-1" landmarks="region" accessible-label="Handelingen">
      <nldd-simple-section>
        <nldd-title slot="header" size="3" text="Logboek">
          <nldd-text slot="supporting-text">
            Wat er gebeurde, de nieuwste stap bovenaan. Elke stap is een chronolexogram uit de
            kroniek. Bij een besluit staat waarom het zo uitviel: de cel neemt het opnieuw en laat
            zien welke artikelen meeliepen en wie elk open begrip invulde. Een stap die de cel
            weigerde, legde niets vast en staat hier dus niet.
          </nldd-text>
        </nldd-title>
        <nldd-list appearance="box-base" accessible-label="Handelingen">
          <nldd-list-item
            v-for="h in handelingen"
            :key="h.gram.id"
            button
            :selected="h.gram.id === gekozen || undefined"
            @click="gekozen = h.gram.id"
          >
            <nldd-text-cell
              :overline="`${dag(h.gram)} · cel ${h.gram.recording_actor}`"
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
          </nldd-list-item>
        </nldd-list>
      </nldd-simple-section>
    </nldd-page>

    <nldd-page
      v-if="handeling"
      slot="pane-2"
      landmarks="region"
      background="tinted"
      accessible-label="Waarom en wat er gebeurde"
    >
      <nldd-simple-section>
        <nldd-title
          slot="header"
          size="4"
          :text="`${rol(handeling.gram)} ${kort(handeling.gram.id)}`"
        >
          <nldd-text slot="supporting-text">
            {{ dag(handeling.gram) }} · {{ handeling.gram.kind }} onder {{ handeling.gram.basis }}
          </nldd-text>
        </nldd-title>
        <nldd-container gap="16">
          <nldd-container gap="4">
            <nldd-text size="sm">
              Reden:
              <nldd-link
                v-if="handeling.gram.refers_to"
                href="#"
                :text="naam(handeling.gram.refers_to)"
                @click.prevent="gekozen = handeling.gram.refers_to"
              ></nldd-link>
              <template v-else-if="handeling.gram.kind === 'EXECUTOGRAM'">geen; de cel legde dit uit zichzelf vast.</template>
              <template v-else>geen; dit is het begin van een dossier.</template>
            </nldd-text>
            <nldd-text v-if="bij" size="sm">
              Genomen in:
              <nldd-link href="#" :text="naam(bij.id)" @click.prevent="gekozen = bij.id"></nldd-link>,
              in de plaats van het herroepen besluit.
            </nldd-text>
            <nldd-text size="sm">
              Leidde tot:
              <template v-if="handeling.leidde_tot.length">
                <template v-for="(id, i) in handeling.leidde_tot" :key="id">
                  <template v-if="i > 0">, </template>
                  <nldd-link href="#" :text="naam(id)" @click.prevent="gekozen = id"></nldd-link>
                </template>
              </template>
              <template v-else>nog niets.</template>
            </nldd-text>
            <nldd-text size="sm">
              Vastgelegd:
              <nldd-link
                href="#"
                :text="`${handeling.gram.kind} ${kort(handeling.gram.id)} in de kroniek`"
                @click.prevent="emit('kroniek', handeling.gram.id)"
              ></nldd-link>
            </nldd-text>
          </nldd-container>

          <nldd-list appearance="box-base" accessible-label="Uitslag">
            <nldd-list-item size="sm">
              <nldd-text-cell text="Uitslag"></nldd-text-cell>
              <nldd-cell>
                <nldd-badge
                  size="sm"
                  :color="uitslag(handeling.gram).kleur"
                  :text="uitslag(handeling.gram).tekst"
                ></nldd-badge>
              </nldd-cell>
            </nldd-list-item>
            <nldd-list-item v-for="f in bevindingen" :key="f.naam" size="sm">
              <nldd-text-cell :text="leesbaar(f.naam)" supporting-text="bevinding van het bestuursorgaan"></nldd-text-cell>
              <nldd-text-cell width="fit-content" :text="toon(f.waarde)"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>

          <template v-if="keten.length">
            <nldd-text size="sm" color="secondary">
              De keten: het besluit opnieuw genomen, stap voor stap. Rekenwerk en het opzoeken van
              waarden zijn weggelaten; die staan in het volledige spoor.
            </nldd-text>
            <nldd-list type="tree" appearance="box-base" accessible-label="Keten van het besluit">
              <KetenStap v-for="k in keten" :key="k.nummer" :knoop="k" @uitleg="uitleg" />
            </nldd-list>
            <nldd-button
              appearance="secondary"
              size="sm"
              expandable
              :expanded="spoor"
              text="Volledig spoor"
              @click="spoor = !spoor"
            ></nldd-button>
            <nldd-code-viewer v-if="spoor" appearance="box-tinted">{{ handeling.waarom.spoor }}</nldd-code-viewer>
          </template>
          <nldd-text v-else-if="handeling.gram.kind === 'EXECUTOGRAM'" size="sm" color="secondary">
            Geen besluit. De cel legde het feit {{ handeling.gram.fact }} vast, dat
            {{ handeling.gram.basis }} vereist (een executogram, RFC-051). Een reductie van de cel
            leest het terug, zoals de vestiging van een school waaruit de regeling de regio
            afleidt.
          </nldd-text>
          <nldd-text v-else size="sm" color="secondary">
            Geen besluit. De cel legde vast wat de aanvrager indiende. Wat het artikel daarbij
            uitrekende, zoals de laatste dag van de termijn, staat in de kroniek.
          </nldd-text>
        </nldd-container>
      </nldd-simple-section>
    </nldd-page>
  </nldd-side-by-side-split-view>

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
  </Teleport>
</template>
