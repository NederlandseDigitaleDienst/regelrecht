<script setup lang="ts">
// The prediction on the application form of the citizen's own app. When the
// form opens, the app fetches what the executor signed about the citizen and
// the regelwerken, replays the records on this device and computes the same
// answers; an answer that differs from the signed one shows as an error, with
// both values. On request it predicts the decision on what the form holds
// now. A fact that only comes after an application, such as the GGD's
// advice, makes the prediction conditional: one line per thing the GGD can
// establish. The form's values go into the kern here and nowhere else.
import { hoofdletter, leesbaar, toon as inWoorden } from '@regelrecht/frontend-shared/chronolex/waarden';
import { computed, onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue';
import type { Indieningsvorm } from '../../gedeeld/types';
import { longDate } from '../../gedeeld/datum';
import { foutTekst } from '../../gedeeld/http';
import { alsFout } from '../tauri/kern';
import { burgerapp, gekozen, rekenkern } from './app';
import type { Uitkomst, Voorspelling } from './rekenen';
import { bereidVoor, voorspel } from './voorspelling';
import type { Voorbereid } from './voorspelling';

const props = defineProps<{ artikel: string; vorm: Indieningsvorm; velden: Record<string, unknown>; naam: string | null }>();

const voorbereid = shallowRef<Voorbereid | null>(null);
const laden = ref(true);
const fout = ref('');
const uitkomst = ref<Voorspelling | null>(null);
const rekenfout = ref('');

/** Whether the panel is gone: a kern that is ready only after that is freed at once. */
let weg = false;

onMounted(async () => {
  try {
    const id = gekozen.value;
    if (!id) throw new Error('Er is geen regeling gekozen.');
    const [app, maak] = await Promise.all([burgerapp(), rekenkern()]);
    const klaar = await bereidVoor(app, id, maak, props.vorm);
    if (weg) klaar.kern.sluit();
    else voorbereid.value = klaar;
  } catch (e) {
    // The core throws its errors as `KernFout`; the screens read them as messages.
    fout.value = foutTekst(alsFout(e));
  } finally {
    laden.value = false;
  }
});
onBeforeUnmount(() => {
  weg = true;
  voorbereid.value?.kern.sluit();
});

// A prediction is about the form as it was; once the citizen changes it, it
// goes, so an old outcome never stands beside new answers.
watch(
  () => props.velden,
  () => {
    uitkomst.value = null;
    rekenfout.value = '';
  },
  { deep: true },
);

const afwijkend = computed(() => voorbereid.value?.vergelijking.antwoorden.filter((v) => !v.gelijk) ?? []);
const ondertekend = computed(() => {
  const g = voorbereid.value?.gegevens;
  return g ? `Ondertekend door de organisatie met OIN ${g.iss}, volgens haar stand van ${longDate(g.peildatum)}.` : '';
});
/** A value in words. A silent cell (`null`) knows nothing about it, which is no `nee`. */
const toon = (w: unknown): string => (w === null || w === undefined ? 'Niet bekend bij de regeling' : hoofdletter(inWoorden(w)));

function bereken() {
  if (!voorbereid.value) return;
  rekenfout.value = '';
  try {
    uitkomst.value = voorspel(voorbereid.value, props.vorm, props.artikel, props.velden, props.naam);
  } catch (e) {
    uitkomst.value = null;
    rekenfout.value = foutTekst(e);
  }
}

/** The one outcome, when it does not depend on what comes after the application. */
const zeker = computed<Uitkomst | null>(() => {
  const u = uitkomst.value;
  if (!u?.zeker) return null;
  return u.takken[0]?.uitkomst ?? u.nu.uitkomst;
});

const tekstVan: Record<Uitkomst, string> = { toegekend: 'Toegekend', afgewezen: 'Afgewezen', onbekend: 'Niet te voorspellen' };
const kleurVan: Record<Uitkomst, string> = { toegekend: 'success', afgewezen: 'critical', onbekend: 'warning' };

/** The condition of a branch, in words: "Als GGD Wilgendaal vaststelt: langdurige loopbeperking". */
function voorwaarde(t: Voorspelling['takken'][number]): string {
  if (t.anders) return 'In alle andere gevallen';
  if (t.vastgesteld.length) return `Als ${t.celNaam} vaststelt: ${t.vastgesteld.join(' en ')}`;
  return `Als ${t.celNaam} niets hiervan vaststelt`;
}
</script>

<template>
  <nldd-container gap="16">
    <nldd-title size="display-5"><h3>Wat u kunt verwachten</h3></nldd-title>
    <nldd-text color="secondary">
      <p>
        Deze app rekent op dit apparaat uit wat de regeling zou besluiten. Wat u hierboven invult, gaat pas naar de
        regeling als u de aanvraag indient.
      </p>
    </nldd-text>
    <nldd-activity-indicator v-if="laden"></nldd-activity-indicator>
    <nldd-inline-dialog v-else-if="fout" icon="warning" icon-color="critical" text="Voorspellen lukt niet" :supporting-text="fout"></nldd-inline-dialog>
    <template v-else-if="voorbereid">
      <nldd-inline-dialog
        v-if="afwijkend.length"
        icon="warning"
        icon-color="critical"
        text="De app rekent iets anders uit dan de regeling ondertekende"
        supporting-text="Bij de gegevens hieronder met Verschil komt de app op dit apparaat tot een andere uitkomst dan de regeling. Een voorspelling is dan niet betrouwbaar."
      ></nldd-inline-dialog>
      <nldd-inline-dialog
        v-if="!voorbereid.vergelijking.regelwerkenGelijk"
        icon="warning"
        icon-color="critical"
        text="De app rekent met andere versies van de regels dan de regeling"
        supporting-text="De regeling rekende met andere versies van de regelwerken dan de app op dit apparaat heeft. Een voorspelling is dan niet betrouwbaar."
      ></nldd-inline-dialog>
      <nldd-text v-if="voorbereid.vergelijking.leeg" color="secondary">
        <p>Deze procedure geeft nog geen gegevens uit. De voorspelling rekent alleen met wat u invult en wat na uw aanvraag nog wordt vastgesteld.</p>
      </nldd-text>
      <nldd-list v-else appearance="box-base" accessible-label="Wat de regeling al van u weet">
        <nldd-list-item v-for="v in voorbereid.vergelijking.antwoorden" :key="v.naam" size="md">
          <nldd-text-cell
            :text="hoofdletter(leesbaar(v.naam))"
            :supporting-text="v.gelijk ? toon(v.ondertekend) : `Ondertekend: ${toon(v.ondertekend)}. Op dit apparaat: ${toon(v.lokaal)}.`"
          ></nldd-text-cell>
          <nldd-cell>
            <nldd-tag :text="v.gelijk ? 'Klopt' : 'Verschil'" :color="v.gelijk ? 'success' : 'critical'" size="sm"></nldd-tag>
          </nldd-cell>
        </nldd-list-item>
      </nldd-list>
      <nldd-text size="sm" color="secondary"><p>{{ ondertekend }}</p></nldd-text>
      <nldd-button-group orientation="horizontal">
        <nldd-button type="button" appearance="secondary" text="Voorspel de uitkomst" @click="bereken"></nldd-button>
      </nldd-button-group>
      <nldd-inline-dialog v-if="rekenfout" icon="warning" icon-color="critical" text="Voorspellen lukt niet" :supporting-text="rekenfout"></nldd-inline-dialog>
      <template v-if="uitkomst">
        <nldd-inline-dialog
          v-if="zeker"
          :variant="zeker === 'toegekend' ? 'success' : undefined"
          :icon="zeker === 'toegekend' ? undefined : 'warning'"
          :text="`Verwachte uitkomst: ${tekstVan[zeker].toLowerCase()}`"
          :supporting-text="uitkomst.takken.length ? 'Wat er na uw aanvraag nog wordt vastgesteld, verandert dit niet.' : (uitkomst.nu.reden ?? '')"
        ></nldd-inline-dialog>
        <template v-else>
          <nldd-text>
            <p>De uitkomst hangt af van wat er na uw aanvraag nog wordt vastgesteld:</p>
          </nldd-text>
          <nldd-list appearance="box-base" accessible-label="Mogelijke uitkomsten">
            <nldd-list-item v-for="(t, i) in uitkomst.takken" :key="i" size="md">
              <nldd-text-cell :text="voorwaarde(t)" :supporting-text="t.reden ?? `Stap: ${t.taak}`"></nldd-text-cell>
              <nldd-cell>
                <nldd-tag :text="tekstVan[t.uitkomst]" :color="kleurVan[t.uitkomst]" size="sm"></nldd-tag>
              </nldd-cell>
            </nldd-list-item>
          </nldd-list>
        </template>
      </template>
    </template>
  </nldd-container>
</template>
