<script setup lang="ts">
// The timeline of one application, on the citizen's screen and the
// employee's alike: every record that happened about it, in order, and under
// it what can or must happen next and who is to act. The server derives both
// (`GET /api/dossier/tijdlijn/{id}`, `GET /api/backoffice/tijdlijn/{id}`) and
// keeps them to what the reader may see; this names them (tijdlijn.ts). A
// row whose record this side can show has a details button at its end, which
// opens it (`opent`): the row itself does not.
// Built as the design system draws steps under each other: an `nldd-list`
// with an `nldd-step-cell` per row, wide enough for a check mark on what is
// done. The past is done, the first step still to come current and the steps
// after it still ahead. The record on screen beside it is a row like any
// other, but it does not open again. Given the requests of the policy
// (`verzoeken`), the records of each exchange and the step that waits on its
// answer stand under the request: the answer as a step, the receipts as minor
// steps. An info button on the request opens a sheet with what it adds
// (VerzoekSheet.vue). Given what opens the letter about a record
// (`bericht`), a row with one has an info button that opens the letter
// instead of the record.
import { computed, ref } from 'vue';
import { metVerzoeken, regels, stap } from './tijdlijn';
import type { Kant, Regel, Verzoekregel } from './tijdlijn';
import type { Casus, Gram, Tijdlijn, Veldtekst, VerzoekInDossier } from './types';
import VerzoekSheet from './VerzoekSheet.vue';

const props = defineProps<{
  tijdlijn: Tijdlijn | null;
  casus: Casus;
  kant: Kant;
  teksten?: Record<string, Veldtekst>;
  /** The record on screen beside it, which its row does not open. */
  huidig?: string;
  /** What opens a record, if this side can show it. */
  opent?: (g: Gram) => (() => void) | undefined;
  /** Why the timeline did not load. */
  fout?: string;
  /** The requests of the policy in it, each shown as one row. */
  verzoeken?: VerzoekInDossier[];
  /** What opens the letter about a record, if it has one. */
  bericht?: (g: Gram) => (() => void) | undefined;
}>();

const isVerzoek = (r: Regel): r is Verzoekregel => 'verzoek' in r;

/** A row of the track: a record, a stage of a request under it, or a next step. */
interface Lijnrij {
  sleutel: string;
  level: 'major' | 'minor';
  gedaan: boolean;
  tekst: string;
  regel: string;
  stand?: Regel['stand'];
  opent?: () => void;
  brief?: () => void;
  verzoek?: Verzoekregel;
}

const lijn = computed((): Lijnrij[] => {
  const t = props.tijdlijn;
  if (!t) return [];
  const alle = regels(t, props.casus);
  const rijen = props.verzoeken ? metVerzoeken(alle, props.verzoeken, t, props.casus, props.kant) : alle;
  const verzoeken = new Set(rijen.filter(isVerzoek).map((r) => r.gram.id));
  return [
    ...rijen.flatMap((r): Lijnrij[] => [
      {
        sleutel: r.gram.id,
        level: 'major',
        gedaan: true,
        tekst: r.tekst,
        regel: r.regel,
        stand: r.stand,
        opent: isVerzoek(r) || r.gram.id === props.huidig ? undefined : props.opent?.(r.gram),
        brief: isVerzoek(r) ? undefined : props.bericht?.(r.gram),
        verzoek: isVerzoek(r) ? r : undefined,
      },
      ...(isVerzoek(r) ? r.verzoek.stappen : []).map((s, j): Lijnrij => ({ sleutel: `${r.gram.id}-${j}`, ...s })),
    ]),
    // The step that waits on a request's answer stands under the request.
    ...t.volgende
      .filter((s) => !(s.soort === 'antwoord' && verzoeken.has(s.op)))
      .map((s, j): Lijnrij => ({ sleutel: `volgende-${j}`, level: 'major', gedaan: false, ...stap(s, t, props.casus, props.kant, props.teksten) })),
  ];
});
const eersteOpen = computed(() => lijn.value.findIndex((r) => !r.gedaan));
const status = (i: number) => (lijn.value[i]!.gedaan ? 'past' : i === eersteOpen.value ? 'current' : 'future');
/** Where row `i` stands, for the track of the step cells. */
function positie(i: number): 'first' | 'between' | 'last' | 'only' {
  const n = lijn.value.length;
  if (n === 1) return 'only';
  if (i === 0) return 'first';
  return i === n - 1 ? 'last' : 'between';
}

/** The request open in the sheet: its row has a button that opens it, not a row that does. */
const verzoek = ref<Verzoekregel | null>(null);
</script>

<template>
  <nldd-box>
    <nldd-container padding="16">
      <nldd-title heading-level="4" size="display-5" text="Tijdlijn"></nldd-title>
      <nldd-spacer size="8"></nldd-spacer>
      <nldd-inline-dialog v-if="fout" icon="warning" icon-color="critical" text="De tijdlijn laadt niet" :supporting-text="fout"></nldd-inline-dialog>
      <nldd-activity-indicator v-else-if="!tijdlijn" text="De tijdlijn laden" show-text></nldd-activity-indicator>
      <nldd-list v-else appearance="simple" accessible-label="Tijdlijn van de aanvraag">
        <nldd-list-item v-for="(r, i) in lijn" :key="r.sleutel" size="md">
          <nldd-step-cell
            size="md"
            :level="r.level"
            :status="status(i)"
            :position="positie(i)"
            :icon="r.gedaan && r.level === 'major' ? 'check-mark' : undefined"
          ></nldd-step-cell>
          <nldd-spacer-cell size="12"></nldd-spacer-cell>
          <nldd-text-cell :overline="r.gedaan ? undefined : 'Volgende stap'" :text="r.tekst" :supporting-text="r.regel"></nldd-text-cell>
          <nldd-cell v-if="r.stand">
            <nldd-tag :text="r.stand.tekst" :color="r.stand.kleur" size="sm"></nldd-tag>
          </nldd-cell>
          <template v-if="r.verzoek">
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-cell>
              <nldd-icon-button
                size="sm"
                appearance="neutral-base"
                icon="info"
                text="Over dit verzoek"
                @click="verzoek = r.verzoek"
              ></nldd-icon-button>
            </nldd-cell>
          </template>
          <template v-else-if="r.brief">
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-cell>
              <nldd-icon-button
                size="sm"
                appearance="neutral-base"
                icon="info"
                text="Bericht"
                :accessible-label="`Bericht bij ${r.tekst.toLowerCase()}`"
                @click="r.brief()"
              ></nldd-icon-button>
            </nldd-cell>
          </template>
          <template v-else-if="r.opent">
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-cell>
              <nldd-icon-button
                size="sm"
                appearance="neutral-base"
                icon="info"
                text="Details"
                :accessible-label="`Details van ${r.tekst.toLowerCase()}`"
                @click="r.opent()"
              ></nldd-icon-button>
            </nldd-cell>
          </template>
        </nldd-list-item>
      </nldd-list>
      <template v-if="tijdlijn && eersteOpen < 0">
        <nldd-spacer size="8"></nldd-spacer>
        <nldd-text size="sm" color="secondary"><p>Er staat nu niets open bij deze aanvraag.</p></nldd-text>
      </template>
    </nldd-container>
  </nldd-box>
  <VerzoekSheet v-if="verzoeken" :rij="verzoek" :teksten="teksten" @sluit="verzoek = null" />
</template>
