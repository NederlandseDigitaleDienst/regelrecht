<script setup lang="ts">
// The sheet in which a fact is recorded: the fields the article that requires
// it lists, as the UI spec names them, and one call to record it. The work
// queue records a fact of the executor with it, the admin panel one of an
// external party; the caller passes the call (`vastleggen`). A fact that
// answers a request comes with that request (`verzoek`): the fields the
// request already holds, such as for whom it is, are shown and not asked,
// and the server takes them from the request.
import { computed, ref, watch } from 'vue';
import { zonderLeeg } from '@regelrecht/frontend-shared/chronolex/waarden';
import { beginwaarden, schermveld } from '../gedeeld/formulier';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import Gegevens from '../gedeeld/Gegevens.vue';
import { artikelNaam } from '../gedeeld/keten';
import SpecFormulier from '../gedeeld/SpecFormulier.vue';
import type { Gram, Handeling, Vormen } from '../gedeeld/types';
import type { Vastlegging } from './api';
import { feitNaam } from './feit';

const props = defineProps<{
  /**
   * The fact to record and the cell that records it; none closes the sheet.
   * With `verzoek`, the fact answers that request, whose fields are `vast`.
   */
  keuze: { cel: string; handeling: Handeling; verzoek?: string; vast?: Record<string, unknown> } | null;
  teksten?: Vormen['teksten'];
  vastleggen: (v: Vastlegging, sleutel: string) => Promise<Gram>;
}>();
const emit = defineEmits<{ sluit: []; vastgelegd: [naam: string] }>();

const waarden = ref<Record<string, unknown>>({});
const melding = ref('');
const bezig = ref(false);
let sleutel = nieuweSleutel();
/** The fields the request already holds: not asked, but shown. */
const vast = computed(() => props.keuze?.vast ?? {});
const velden = computed(() =>
  (props.keuze?.handeling.velden ?? []).filter((v) => !(v.naam in vast.value)).map((v) => schermveld(v, props.teksten)),
);

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
    const { cel, handeling, verzoek } = props.keuze;
    await props.vastleggen({ cel, feit: handeling.feit, velden: zonderLeeg(waarden.value), ...(verzoek ? { verzoek } : {}) }, sleutel);
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
          <nldd-top-title-bar heading-level="2" :text="keuze ? `${feitNaam(keuze.handeling.feit)} vastleggen` : ''" dismiss-text="Sluiten"></nldd-top-title-bar>
        </nldd-container>
        <nldd-simple-section v-if="keuze">
          <nldd-container gap="16">
            <slot></slot>
            <nldd-text color="secondary">De gegevens komen uit {{ artikelNaam(keuze.handeling.feit.split('#').slice(0, 2).join('#')) }}.</nldd-text>
            <template v-if="keuze.verzoek">
              <nldd-title heading-level="3" size="display-6" text="Uit het verzoek"></nldd-title>
              <Gegevens :waarden="vast" :teksten="teksten" label="Uit het verzoek" />
            </template>
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
