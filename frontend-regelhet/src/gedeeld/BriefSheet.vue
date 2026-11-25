<script setup lang="ts">
// The letter about a record, in a bottom sheet over whatever is open
// (`#bericht/<id>`, so a link to it works): the acknowledgement of an
// application, an invitation to complete it, a message sent. The timeline
// opens it from the row of its record (TijdlijnLijst.vue), on the citizen's
// side and in the back office; the citizen's list of messages from the
// message. A submission's letter reads what it gives under its own article,
// such as the last day of the period to decide (`uitkomsten`, from the API of
// the side it is on).
import { computed, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import Bericht from './Bericht.vue';
import { BERICHT } from './bericht';
import { celNaam, isIndiening, rolVan } from './keten';
import type { Casus, Gram, Stand, Vormen } from './types';

const props = defineProps<{
  casus: Casus;
  vormen: Vormen | null;
  /** The records the letter may name, the record itself among them. */
  grammen: Gram[];
  stand: Pick<Stand, 'rollen'>;
  weergave: 'burger' | 'medewerker';
  /** What a submission gives under its own article, from the server. */
  uitkomsten?: (id: string) => Promise<Record<string, unknown>>;
}>();
const route = useRoute();
const router = useRouter();

const stuk = computed(() => {
  if (!route.hash.startsWith(BERICHT)) return null;
  const id = decodeURIComponent(route.hash.slice(BERICHT.length));
  return props.grammen.find((g) => g.id === id) ?? null;
});
const uitkomsten = ref<Record<string, unknown> | null>(null);
watch(
  () => (stuk.value && isIndiening(stuk.value, props.stand) ? stuk.value.id : undefined),
  async (id) => {
    uitkomsten.value = null;
    if (!id || !props.uitkomsten) return;
    try {
      uitkomsten.value = await props.uitkomsten(id);
    } catch {
      // Without them the letter shows what the submission holds.
    }
  },
  { immediate: true },
);
const naamVanCel = (cel: string) => celNaam(cel, props.casus);
const sluit = () => void router.push({ path: route.path, query: route.query });
</script>

<template>
  <Teleport to="body">
    <nldd-sheet placement="bottom" accessible-label="Bericht" :open="stuk ? true : undefined" @close="stuk && sluit()">
      <nldd-page sticky-header>
        <nldd-top-title-bar slot="header" heading-level="2" text="Bericht" dismiss-text="Sluit" collapse-anchor="bericht-titel"></nldd-top-title-bar>
        <nldd-simple-section>
          <nldd-title id="bericht-titel" size="display-2" heading-level="2" text="Bericht"></nldd-title>
          <nldd-spacer size="16"></nldd-spacer>
          <Bericht
            v-if="stuk"
            :gram="stuk"
            :rol="rolVan(stuk, stand)"
            :vormen="vormen"
            :grammen="grammen"
            :uitkomsten="uitkomsten"
            :afzender="naamVanCel(stuk.recording_actor)"
            :cel-naam="naamVanCel"
            :weergave="weergave"
          />
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
