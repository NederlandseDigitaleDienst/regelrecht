<script setup>
// A field that holds a table (schema v0.8.20): the rows in an nldd-table, a
// column per column the law declares. A row is edited in a sheet, as the
// NLDD pattern "bewerken in een sheet" does it: the table stays in view, the
// way out sits in the title bar, and "Bewaar" under the last field. Every
// column may stay empty; an empty cell is null. A yes-or-no column therefore
// asks ja, nee or leeg in a dropdown, not with a switch, which knows only on
// and off. A row added or removed, or a row saved, gives the form the whole
// table anew; removing the last row empties the field, and saving a new row
// with nothing filled in adds nothing.
import { computed, ref } from 'vue';
import { leesbaar } from '../state.js';
import { bronRij, celTekst, kolomVeld, sleutelKolommen } from '../tabel.js';
import VeldInvoer from './VeldInvoer.vue';

const props = defineProps({
  veld: { type: Object, required: true },
  modelValue: { type: Array, default: undefined },
  herkomst: { type: String, default: '' },
});
const emit = defineEmits(['update:modelValue']);

const rijen = computed(() => props.modelValue ?? []);
const kolommen = computed(() => props.veld.kolommen);
// Every column takes an equal share, the buttons what they need.
const sporen = computed(() => [...kolommen.value.map(() => 'minmax(120px, 1fr)'), 'auto'].join(' '));

// The row in the sheet: `index` is its place in the table, null for a new
// row; `rij` the values the sheet holds until "Bewaar". It stays after the
// sheet closes, so the sheet keeps its content while it slides away.
const concept = ref(null);
const open = ref(false);
const titelId = computed(() => `tabel-${props.veld.naam}-rij`);

const sleutels = computed(() => kolommen.value.filter((k) => sleutelKolommen(kolommen.value).includes(k.naam)));
/** What names row `i`: its key columns, else every filled column, else its number. */
const rijNaam = (rij, i) => bronRij(rij, sleutels.value) || bronRij(rij, kolommen.value) || `Rij ${i + 1}`;

const kop = computed(() => {
  const c = concept.value;
  if (!c) return '';
  return c.index === null
    ? `Rij toevoegen aan ${leesbaar(props.veld.naam)}`
    : `${rijNaam(rijen.value[c.index], c.index)} bewerken`;
});

function bewerk(index) {
  concept.value = { index, rij: { ...rijen.value[index] } };
  open.value = true;
}
function nieuw() {
  concept.value = { index: null, rij: {} };
  open.value = true;
}
function bewaar() {
  const { index, rij } = concept.value;
  const volledig = Object.fromEntries(kolommen.value.map((k) => [k.naam, rij[k.naam] ?? null]));
  open.value = false;
  if (index === null && Object.values(volledig).every((v) => v === null)) return;
  const uit = [...rijen.value];
  if (index === null) uit.push(volledig);
  else uit[index] = volledig;
  emit('update:modelValue', uit);
}

/** A yes-or-no cell as the dropdown holds it, and back. */
const jaNeeKeuze = (v) => (v === true ? 'ja' : v === false ? 'nee' : '');
const jaNeeWaarde = (k) => (k === 'ja' ? true : k === 'nee' ? false : null);
function verwijder(index) {
  const uit = rijen.value.filter((_, i) => i !== index);
  emit('update:modelValue', uit.length ? uit : undefined);
}
</script>

<template>
  <nldd-form-section :text="leesbaar(veld.naam)" :supporting-text="veld.basis">
    <nldd-container gap="12">
      <nldd-table :accessible-label="leesbaar(veld.naam)" :columns="sporen">
        <nldd-table-row slot="header">
          <nldd-text-cell v-for="k in kolommen" :key="k.naam" :text="leesbaar(k.naam)"></nldd-text-cell>
          <nldd-cell width="fit-content"><nldd-visually-hidden>Acties</nldd-visually-hidden></nldd-cell>
        </nldd-table-row>
        <nldd-table-row v-for="(rij, i) in rijen" :key="i">
          <nldd-text-cell
            v-for="(k, j) in kolommen"
            :key="k.naam"
            :row-header="j === 0 ? true : undefined"
            :text="celTekst(rij[k.naam])"
          ></nldd-text-cell>
          <nldd-cell width="fit-content">
            <nldd-icon-button
              appearance="neutral-transparent"
              size="sm"
              icon="edit"
              text="Bewerk"
              :accessible-label="`${rijNaam(rij, i)} bewerken`"
              @click="bewerk(i)"
            ></nldd-icon-button>
            <nldd-icon-button
              appearance="neutral-transparent"
              size="sm"
              icon="delete"
              text="Verwijder"
              :accessible-label="`${rijNaam(rij, i)} verwijderen`"
              @click="verwijder(i)"
            ></nldd-icon-button>
          </nldd-cell>
        </nldd-table-row>
        <nldd-inline-dialog slot="empty" icon="info" text="Nog geen rijen"></nldd-inline-dialog>
      </nldd-table>
      <nldd-button-group orientation="horizontal">
        <nldd-button appearance="secondary" start-icon="add" text="Rij toevoegen" @click="nieuw"></nldd-button>
      </nldd-button-group>
      <nldd-text v-if="herkomst" size="sm" color="secondary">{{ herkomst }}</nldd-text>
    </nldd-container>
  </nldd-form-section>

  <Teleport to="body">
    <nldd-sheet placement="right" width="480px" :open="open ? true : undefined" @close="open = false">
      <nldd-page>
        <nldd-top-title-bar
          slot="header"
          heading-level="2"
          :text="kop"
          dismiss-text="Annuleer"
          :collapse-anchor="titelId"
        ></nldd-top-title-bar>
        <nldd-simple-section v-if="concept">
          <nldd-title :id="titelId" slot="header" size="display-3" heading-level="2" :text="kop"></nldd-title>
          <nldd-form @submit.prevent="bewaar">
            <template v-for="k in kolommen" :key="k.naam">
              <nldd-form-field v-if="k.type === 'boolean'" :label="leesbaar(k.naam)" optional>
                <nldd-dropdown>
                  <select
                    :value="jaNeeKeuze(concept.rij[k.naam])"
                    @change="concept.rij[k.naam] = jaNeeWaarde($event.target.value)"
                  >
                    <option value="">Leeg</option>
                    <option value="ja">Ja</option>
                    <option value="nee">Nee</option>
                  </select>
                </nldd-dropdown>
              </nldd-form-field>
              <VeldInvoer
                v-else
                :veld="kolomVeld(k)"
                :model-value="concept.rij[k.naam] ?? undefined"
                @update:model-value="concept.rij[k.naam] = $event"
              />
            </template>
            <nldd-form-actions>
              <nldd-button-group orientation="horizontal">
                <nldd-button appearance="primary" type="submit" text="Bewaar"></nldd-button>
              </nldd-button-group>
            </nldd-form-actions>
          </nldd-form>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
