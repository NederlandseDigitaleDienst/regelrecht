<script setup lang="ts">
// The sheet in which a fact is recorded: the fields the article that requires
// it lists, as the UI spec names them, and one call to record it. The work
// queue records a fact of the executor with it, the admin panel one of an
// external party; the caller passes the call (`vastleggen`). A fact about an
// earlier record, such as an act after a grant, carries that record
// (`verwijst_naar`). A field the UI spec has the system fill shows
// read-only with the value the server records; the sheet does not send it.
// A fact that answers a request comes with that request (`verzoek`): the
// fields the request already holds, such as for whom it is, show read-only
// too, and the server takes them from the request.
import { computed, ref, watch } from 'vue';
import { zonderLeeg } from '@regelrecht/frontend-shared/chronolex/waarden';
import { beginwaarden, schermveld } from '../gedeeld/formulier';
import type { Schermveld } from '../gedeeld/formulier';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import { artikelNaam, waardeTekst } from '../gedeeld/keten';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import type { Gram, Handeling, Vormen } from '../gedeeld/types';
import type { Vastlegging } from './api';
import { feitNaam } from './feit';

const props = defineProps<{
  /**
   * The fact to record and the cell that records it, the record it is about
   * if any, and the sheet's title if not the fact's name; none closes the
   * sheet. With `verzoek`, the fact answers that request, which holds the
   * fields `vast`.
   */
  keuze: {
    cel: string;
    handeling: Handeling;
    verwijst_naar?: string;
    titel?: string;
    verzoek?: string;
    vast?: Record<string, unknown>;
  } | null;
  teksten?: Vormen['teksten'];
  vastleggen: (v: Vastlegging, sleutel: string) => Promise<Gram>;
}>();
const emit = defineEmits<{ sluit: []; vastgelegd: [naam: string] }>();

const waarden = ref<Record<string, unknown>>({});
const melding = ref('');
const bezig = ref(false);
let sleutel = nieuweSleutel();
const systeem = computed(() => props.keuze?.handeling.systeem ?? {});
/** The fields the request already holds. */
const uitVerzoek = computed(() => props.keuze?.vast ?? {});
const alle = computed(() => (props.keuze?.handeling.velden ?? []).map((v) => schermveld(v, props.teksten)));
/** The fields the employee fills. */
const velden = computed(() => alle.value.filter((v) => !(v.veld.naam in systeem.value) && !(v.veld.naam in uitVerzoek.value)));
/** The fields the system fills or the request holds, with their value and where it comes from. */
const vast = computed(() =>
  alle.value.flatMap((v: Schermveld) => {
    const naam = v.veld.naam;
    if (naam in uitVerzoek.value) return [{ ...v, waarde: waardeTekst(uitVerzoek.value[naam]), herkomst: 'Uit het verzoek.' }];
    const hoe = systeem.value[naam];
    if (!hoe) return [];
    return [
      {
        ...v,
        waarde: waardeTekst(hoe.waarde),
        herkomst: hoe.soort === 'datum' ? 'De dag van vastleggen, volgens de klok van het systeem.' : 'Vult het systeem in.',
      },
    ];
  }),
);
const titel = computed(() => (props.keuze ? (props.keuze.titel ?? `${feitNaam(props.keuze.handeling.feit)} vastleggen`) : ''));

// Each fact the sheet opens on starts from an empty form and a new key.
watch(
  () => props.keuze,
  () => {
    waarden.value = beginwaarden(velden.value);
    melding.value = '';
    sleutel = nieuweSleutel();
  },
);

async function opslaan() {
  if (!props.keuze) return;
  bezig.value = true;
  melding.value = '';
  try {
    const { cel, handeling, verwijst_naar, verzoek } = props.keuze;
    const velden = zonderLeeg(waarden.value);
    for (const naam of [...Object.keys(systeem.value), ...Object.keys(uitVerzoek.value)]) delete velden[naam];
    await props.vastleggen(
      { cel, feit: handeling.feit, velden, ...(verwijst_naar ? { verwijst_naar } : {}), ...(verzoek ? { verzoek } : {}) },
      sleutel,
    );
    emit('vastgelegd', feitNaam(handeling.feit));
  } catch (e) {
    melding.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet width="720px" :open="keuze ? true : undefined" @close="emit('sluit')">
      <nldd-page>
        <nldd-container slot="header" padding="16">
          <nldd-top-title-bar heading-level="2" :text="titel" dismiss-text="Sluiten"></nldd-top-title-bar>
        </nldd-container>
        <nldd-simple-section v-if="keuze">
          <nldd-container gap="16">
            <slot></slot>
            <nldd-text color="secondary">De gegevens komen uit {{ artikelNaam(keuze.handeling.feit.split('#').slice(0, 2).join('#')) }}.</nldd-text>
            <nldd-form-field v-for="v in vast" :key="v.veld.naam" :label="v.label" :supporting-label="v.veld.basis">
              <nldd-text-field :value="v.waarde" readonly></nldd-text-field>
              <nldd-form-field-help-text>{{ [v.help, v.herkomst].filter(Boolean).join(' ') }}</nldd-form-field-help-text>
            </nldd-form-field>
            <SpecFormulier :velden="velden" :waarden="waarden" @zet="(n, v) => (waarden[n] = v)" />
            <nldd-inline-dialog v-if="melding" icon="warning" icon-color="critical" text="Niet vastgelegd" :supporting-text="melding"></nldd-inline-dialog>
            <nldd-button-group orientation="horizontal">
              <nldd-button appearance="primary" text="Vastleggen" :loading="bezig || undefined" @click="opslaan"></nldd-button>
              <nldd-button appearance="neutral-transparent" text="Annuleren" @click="emit('sluit')"></nldd-button>
            </nldd-button-group>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
