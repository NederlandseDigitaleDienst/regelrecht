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
// too, and the server takes them from the request. A fact recorded without a
// request while one is open for it (`open`) is stopped here, with a word on
// where to record it instead: the server refuses it anyway.
import { computed, ref, watch } from 'vue';
import { zonderLeeg } from '@regelrecht/frontend-shared/chronolex/waarden';
import { artikelVan, beginwaarden, schermveld } from '../gedeeld/formulier';
import type { Formulierveld, Schermveld } from '../gedeeld/formulier';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import { artikelNaam } from '../gedeeld/keten';
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
  /** The open requests this fact answers, by the fields each holds. */
  open?: { vast: Record<string, unknown> }[];
  /** The order of the UI spec, for the fields of the fact. */
  volgorde?: string[];
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
/**
 * Every field of the fact for the form; one the system fills or the request
 * holds carries its value and where it comes from (`systeem`), and shows
 * read-only in its place in its group.
 */
const formvelden = computed<Formulierveld[]>(() =>
  alle.value.map((v: Schermveld) => {
    const naam = v.veld.naam;
    if (naam in uitVerzoek.value) return { ...v, systeem: { waarde: uitVerzoek.value[naam], herkomst: 'Uit het verzoek.' } };
    const hoe = systeem.value[naam];
    if (!hoe) return v;
    return {
      ...v,
      systeem: {
        waarde: hoe.waarde,
        herkomst: hoe.soort === 'datum' ? 'De dag van vastleggen, volgens de klok van het systeem.' : 'Vult het systeem in.',
      },
    };
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
  melding.value = '';
  // The server's rule (`verzoek::open_voor`): a request is this fact's when
  // every field both hold has the same value, over the fields the form sends.
  const gezet = zonderLeeg(waarden.value);
  const zelfde = (vast: Record<string, unknown>) =>
    Object.entries(vast)
      .filter(([naam]) => naam in gezet)
      .every(([naam, w]) => gezet[naam] === w);
  if (!props.keuze.verzoek && (props.open ?? []).some((v) => zelfde(v.vast))) {
    melding.value = 'Hierom is al gevraagd, en dat verzoek staat nog open. Leg het antwoord vast vanuit dat verzoek, in de lijst open verzoeken.';
    return;
  }
  bezig.value = true;
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
        <nldd-top-title-bar slot="header" heading-level="2" :text="titel" dismiss-text="Sluit"></nldd-top-title-bar>
        <nldd-simple-section v-if="keuze">
          <nldd-container gap="16">
            <slot></slot>
            <nldd-text color="secondary"><p>De gegevens komen uit {{ artikelNaam(artikelVan(keuze.handeling.feit)) }}.</p></nldd-text>
            <SpecFormulier :velden="formvelden" :waarden="waarden" :volgorde="volgorde" @zet="(n, v) => (waarden[n] = v)" />
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
