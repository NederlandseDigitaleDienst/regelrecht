<script setup lang="ts">
// What a record holds, field by field: what was submitted, what the
// authority found, what came out. Empty fields stay out. A row names the
// field and has the value on its right, as the form shows what is already
// known (SpecFormulier.vue); a row from `bij`, such as the article, whose value
// can run long, turns that round: its name is as wide as its words. Where the field has an origin or help, a small
// info button after its name shows them in a popover, so the value stays the
// first thing the reader sees. A yes-or-no value has a check mark or a cross
// before its words. A list of rows in a field the UI spec gives a table
// (`tabellen`) shows as that table under the list, as the form shows it
// (TabelUitKroniek.vue); without `tabellen` every value is a row. The fields stand
// in the order of the UI spec (`volgorde`) and under the headings of its
// groups, with its label and help, as on the forms (formulier.ts). `noem`
// names a record a field refers to, such as the contested decision of an
// objection. `herkomst` says per field where its value came from, where the
// system filled it, such as the rule of a policy. `bij` adds rows that are no
// field of the record, such as the article it rests on, at the end of the
// group that says what the record is about. That is the group of the first
// field the UI spec marks as its summary (`record.summary`), such as the
// decision asked for; it comes first and without a heading, since the screen
// is about that record. Without a summary the groups keep their
// order and `bij` leads as a group of its own: the article belongs with what
// the record is about, not with the personal details. In that leading group
// the summary comes first, then the article, then the rest of what the
// record is about, such as the day it is dated. A screen that puts other
// sections between what the record is about and the rest shows the two
// apart (`deel`); rows of its own, such as how a submission stands, go in
// front of the leading group (slot `voor`). It renders no wrapper of its own, so where it stands in a
// container with a gap the caller gives it a container without one.
import { computed, useId, useSlots } from 'vue';
import { kleineLetter } from '@regelrecht/frontend-shared/chronolex/waarden';
import { geordend, kaalSchermveld, stukWaarde } from './formulier';
import { alsRijen } from './tabel';
import TabelUitKroniek from './TabelUitKroniek.vue';
import type { Tabel, Vormen } from './types';

const props = defineProps<{
  waarden?: Record<string, unknown>;
  teksten?: Vormen['teksten'];
  volgorde?: string[];
  label: string;
  noem?: (id: string) => string | undefined;
  herkomst?: Record<string, string>;
  bij?: { naam: string; label: string; waarde: string }[];
  /** Heading level for the group titles, where they head a part of the page; without it they are a line of text. */
  kopniveau?: number;
  /** Only the leading group, what the record is about (`over`), or only the groups after it (`rest`). */
  deel?: 'over' | 'rest';
  /** The tables of the UI spec: a field one of them names shows its rows as that table. */
  tabellen?: Tabel[];
}>();
const slots = useSlots();
/** A label inside a sentence: a capital only where the word is an abbreviation. */
/** Names the popovers of this component apart from those of another on the same page. */
const id = useId();
const groepen = computed(() => {
  const waarden = props.waarden ?? {};
  const velden = Object.keys(waarden)
    .filter((naam) => waarden[naam] !== null && waarden[naam] !== undefined)
    .map((naam) => kaalSchermveld(naam, props.teksten));
  const tabelVan = (naam: string) => {
    const tabel = props.tabellen?.find((t) => t.veld === naam);
    const rijen = tabel && alsRijen(waarden[naam]);
    return tabel && rijen ? { tabel, rijen } : undefined;
  };
  const groepen = geordend(velden, props.volgorde).map((g) => ({
    titel: g.titel,
    samenvatting: g.velden.some((s) => props.teksten?.[s.veld.naam]?.record?.summary),
    rijen: g.velden.map((s) => ({
      naam: s.veld.naam,
      label: s.label,
      waarde: stukWaarde(s.veld.naam, waarden[s.veld.naam], props.teksten, props.noem),
      uitleg: [props.herkomst?.[s.veld.naam], s.help].filter((t): t is string => !!t),
      icoon: waarden[s.veld.naam] === true ? 'checked' : waarden[s.veld.naam] === false ? 'dismiss' : undefined,
      tabel: tabelVan(s.veld.naam),
      bij: false,
      samenvatting: !!props.teksten?.[s.veld.naam]?.record?.summary,
    })),
  }));
  // The group that says what the record is about leads, without a heading: the screen is about that record.
  // Without such a group, what is no field of the record leads as a group of its own.
  const over = groepen.findIndex((g) => g.samenvatting);
  const bij = (props.bij ?? []).map((b) => ({ ...b, uitleg: [] as string[], icoon: undefined, tabel: undefined, bij: true, samenvatting: false }));
  const rijen = over < 0 ? [] : groepen[over]!.rijen;
  const leidend = { titel: '', rijen: [...rijen.filter((r) => r.samenvatting), ...bij, ...rijen.filter((r) => !r.samenvatting)] };
  const overige = groepen.filter((_, i) => i !== over);
  // Rows of the screen's own need the leading group to stand in, also when it has no rows of the record.
  const metLeidend = leidend.rijen.length || slots.voor ? [leidend] : [];
  if (props.deel === 'over') return metLeidend;
  if (props.deel === 'rest') return overige;
  return [...metLeidend, ...overige];
});
</script>

<template>
  <!-- No wrapper: the lists, headings and spacers stand in the slot of the section around it. -->
  <template v-if="groepen.length">
    <template v-for="(g, i) in groepen" :key="i">
      <nldd-spacer v-if="i > 0" size="24"></nldd-spacer>
      <template v-if="g.titel">
        <nldd-title v-if="kopniveau" :heading-level="kopniveau" size="display-5" :text="g.titel"></nldd-title>
        <nldd-text v-else size="lg" weight="medium"><p>{{ g.titel }}</p></nldd-text>
        <nldd-spacer size="8"></nldd-spacer>
      </template>
      <nldd-list
        v-if="g.rijen.some((r) => !r.tabel) || (i === 0 && !g.titel && $slots.voor)"
        appearance="box-base"
        :accessible-label="g.titel || label"
      >
        <slot v-if="i === 0 && !g.titel" name="voor"></slot>
        <nldd-list-item v-for="r in g.rijen.filter((x) => !x.tabel)" :key="r.naam" size="md">
          <nldd-text-cell v-if="r.uitleg.length">
            <p>
              {{ r.label }}
              <nldd-icon-button
                size="xs"
                appearance="neutral-base"
                icon="info"
                :text="`Over ${kleineLetter(r.label)}`"
                :popovertarget="`${id}-${i}-${r.naam}`"
              ></nldd-icon-button>
            </p>
          </nldd-text-cell>
          <nldd-text-cell v-else :text="r.label" :width="r.bij ? 'fit-content' : undefined"></nldd-text-cell>
          <nldd-spacer-cell size="16"></nldd-spacer-cell>
          <template v-if="r.icoon">
            <nldd-icon-cell size="20" :icon="r.icoon"></nldd-icon-cell>
            <nldd-spacer-cell size="4"></nldd-spacer-cell>
          </template>
          <nldd-text-cell :text="r.waarde" :width="r.bij ? undefined : 'fit-content'" horizontal-alignment="right"></nldd-text-cell>
        </nldd-list-item>
      </nldd-list>
      <template v-for="r in g.rijen.filter((x) => x.tabel)" :key="r.naam">
        <nldd-spacer v-if="g.rijen.some((x) => !x.tabel)" size="16"></nldd-spacer>
        <TabelUitKroniek
          v-if="r.tabel"
          vullend
          :tabel="{ ...r.tabel.tabel, label: r.label, help: r.uitleg.join(' ') }"
          :rijen="r.tabel.rijen"
        />
      </template>
      <!-- The popovers stand beside the list: a text cell takes only its paragraph. -->
      <template v-for="r in g.rijen" :key="r.naam">
        <nldd-popover
          v-if="r.uitleg.length && !r.tabel"
          :id="`${id}-${i}-${r.naam}`"
          role="region"
          :accessible-label="`Over ${kleineLetter(r.label)}`"
        >
          <nldd-container padding="16">
            <template v-for="(u, j) in r.uitleg" :key="j">
              <nldd-spacer v-if="j > 0" size="8"></nldd-spacer>
              <nldd-text><p>{{ u }}</p></nldd-text>
            </template>
          </nldd-container>
        </nldd-popover>
      </template>
    </template>
  </template>
  <nldd-text v-else-if="deel !== 'rest'" color="secondary"><p>Geen gegevens.</p></nldd-text>
</template>
