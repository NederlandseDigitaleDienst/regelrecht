<script setup lang="ts">
// Sending the message of a decision, and forwarding it: two acts in the
// chronicle of the sender (`verzending` in the message spec). Sending records
// the fact of sending with the message frozen as it stands now (its content
// and the hash of it, bericht.ts `verzendvelden`); the citizen reads it in her
// dossier from that fact, and the day it went is the day the decision was
// made known. Forwarding records the fact of forwarding; the core delivers it
// to the cell whose receipt the article names, which records it in its own
// chronicle. Nothing is overwritten: a message sent again replaces the last
// one and names it.
import { computed, ref } from 'vue';
import { berichtsoortVoor, maakBericht, verzendingenVan, verzendvelden } from '../gedeeld/bericht';
import { gedeeldApi } from '../gedeeld/api';
import { foutTekst, nieuweSleutel } from '../gedeeld/http';
import type { Gram, Vormen, Waarom } from '../gedeeld/types';
import { backofficeApi } from './api';

const props = defineProps<{
  gram: Gram;
  grammen: Gram[];
  vormen: Vormen | null;
  waarom?: Waarom | null;
  celNaam: (cel: string) => string;
}>();
const emit = defineEmits<{ verzonden: [] }>();

const spec = computed(() => props.vormen?.berichten ?? null);
const verzendingen = computed(() => verzendingenVan(props.gram.id, props.grammen, spec.value));
const laatste = computed(() => verzendingen.value.at(-1) ?? null);
const doorgestuurd = computed(() => {
  const v = spec.value?.verzending?.doorsturen;
  const l = laatste.value;
  return !!v && !!l && props.grammen.some((g) => `${g.basis}#${g.fact}` === v && g.fields?.bericht === l.id);
});
const bezig = ref(false);
const fout = ref<string | null>(null);
let sleutel = nieuweSleutel();

async function verzenden() {
  const s = spec.value;
  if (!s?.verzending) return;
  const soort = berichtsoortVoor(props.gram, 'besluit', s);
  if (!soort) return;
  bezig.value = true;
  fout.value = null;
  try {
    const model = maakBericht(
      soort,
      { gram: props.gram, rol: 'besluit', vormen: props.vormen, grammen: props.grammen, waarom: props.waarom, afzender: props.celNaam(props.gram.recording_actor), celNaam: props.celNaam },
      s,
    );
    const { datum } = await gedeeldApi.klok();
    const velden = verzendvelden(model, props.gram, props.grammen, s, datum, laatste.value?.id ?? null);
    await backofficeApi.vastleggen({ cel: props.gram.recording_actor, feit: s.verzending.feit, velden }, sleutel);
    sleutel = nieuweSleutel();
    emit('verzonden');
  } catch (e) {
    fout.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}

async function doorsturen() {
  const v = spec.value?.verzending?.doorsturen;
  const l = laatste.value;
  if (!v || !l) return;
  bezig.value = true;
  fout.value = null;
  try {
    const extra = Object.fromEntries(Object.keys(spec.value?.verzending?.velden ?? {}).map((k) => [k, l.fields?.[k] ?? null]));
    const velden = { ...extra, bericht: l.id, berichtsoort: l.fields?.berichtsoort, inhoud: l.fields?.inhoud, hash: l.fields?.hash };
    await backofficeApi.vastleggen({ cel: props.gram.recording_actor, feit: v, velden }, sleutel);
    sleutel = nieuweSleutel();
    emit('verzonden');
  } catch (e) {
    fout.value = foutTekst(e);
  } finally {
    bezig.value = false;
  }
}
</script>

<template>
  <nldd-container v-if="spec?.verzending" gap="8" layout="row">
    <nldd-button
      v-if="!laatste"
      variant="primary"
      text="Bericht verzenden"
      :disabled="bezig || undefined"
      @click="verzenden"
    ></nldd-button>
    <nldd-button
      v-if="laatste && spec.verzending.doorsturen && !doorgestuurd"
      variant="secondary"
      text="Bericht doorsturen"
      :disabled="bezig || undefined"
      @click="doorsturen"
    ></nldd-button>
    <nldd-inline-dialog v-if="fout" icon="warning" icon-color="critical" :text="fout"></nldd-inline-dialog>
  </nldd-container>
</template>
