<script setup>
// De rechter (RFC-053): de rechtbank is een andere cel van de casus, met een
// eigen kroniek; zij staat onder Andere cellen (AndereCellenTab.vue).
// Zij oordeelt over een beroep tegen een beslissing op bezwaar en doet
// uitspraak, zoals het bestuursorgaan op een aanvraag beslist: met
// bevindingen. Welke dat zijn, zegt de wet: de open begrippen die zij per
// geval beoordeelt (`decided_per_case_by`) van het artikel van het beroep en
// van de artikelen die het leest. Wat zo'n gelezen artikel vraagt, telt
// alleen als de uitspraak dat artikel leest; anders noemt de cel het
// als niet gevraagd. Naast de bevindingen staat wat de uitspraak ermee zou
// worden (Voorvertoning.vue), zoals bij een besluit onder Bestuursorgaan.
// Na de uitspraak zendt de rechtbank een afschrift toe, en de cel van de
// casus legt de ontvangst vast.
import { computed, reactive } from 'vue';
import { beroepApi, ontvangstVan, uitspraakOp } from '../beroep.js';
import { artikelNaam } from '../logboek.js';
import { isBesluitwaarde, kort, leesbaar, state, stap, waarde, zonderLeeg } from '../state.js';
import { formulier, scenarioWaarden } from '../scenarioInvoer.js';
import UitScenario from './UitScenario.vue';
import VeldInvoer from './VeldInvoer.vue';
import Voorvertoning from './Voorvertoning.vue';

const artikel = computed(() => artikelNaam(...state.casus.beroep.split('#')));
const vorm = computed(() => state.vormen?.uitspraak ?? { bevindingen: [], basis: '' });
// De bevindingen op het artikel van het beroep zelf, en die op een artikel
// dat het leest.
const eigen = computed(() => vorm.value.bevindingen.filter((b) => b.basis === vorm.value.basis));
const gelezen = computed(() => vorm.value.bevindingen.filter((b) => b.basis !== vorm.value.basis));
const gelezenArtikelen = computed(() =>
  [...new Set(gelezen.value.map((b) => b.basis))].map((g) => artikelNaam(...g.split('#'))).join(', '),
);

const open = computed(() => state.rechter.filter((g) => g.kind === 'BEROEP' && !uitspraakOp(g.id)));
const uitspraken = computed(() => state.rechter.filter((g) => g.kind === 'UITSPRAAK'));

// Per beroep de bevindingen van de rechter.
// Volg je een scenario, dan staan haar bevindingen ingevuld (scenarioInvoer.js).
const uitspraakVorm = formulier(() => scenarioWaarden('uitspraak'));
const oordeelVoor = (id) => uitspraakVorm.waarden(id);

// Wat de cel bij een uitspraak teruggaf: wat niet gevraagd werd.
const nietGevraagd = reactive({});

async function uitspraakDoen(beroep) {
  const antwoord = await stap(() =>
    beroepApi.uitspraakDoen(beroep.id, zonderLeeg(oordeelVoor(beroep.id)), state.datum),
  );
  if (antwoord) nietGevraagd[antwoord.uitspraak.id] = antwoord.niet_gevraagd;
}

async function toezenden(uitspraak) {
  await stap(() => beroepApi.toezenden(uitspraak.id, state.datum));
}

const uitvoer = (g) => Object.entries(g.outputs ?? {}).map(([naam, w]) => ({ naam, w }));
</script>

<template>
  <nldd-simple-section>
    <nldd-title slot="header" size="3" :text="`Ingekomen beroepen bij ${state.casus.rechter.cel_naam}`">
      <nldd-text slot="supporting-text">
        De rechtbank beslist op een beroep onder {{ artikel }}, in haar eigen cel. Het beroep
        draagt het afschrift van de beslissing op bezwaar dat {{ state.casus.cel_naam }} maakte.
        De rechtbank oordeelt op de grondslag van de overgelegde stukken (Awb 8:69). Voorziet zij
        zelf in de zaak (Awb 8:72, derde lid, onder b), dan neemt zij die beslissing opnieuw, met
        haar eigen bevindingen. De bevindingen hieronder zijn haar oordeel.
      </nldd-text>
    </nldd-title>
    <nldd-inline-dialog
      v-if="open.length === 0"
      icon="info"
      text="Geen open beroepen"
      supporting-text="Een beroep stel je in onder het tabblad Aanvrager, bij een beslissing op bezwaar."
    ></nldd-inline-dialog>
    <nldd-container gap="16">
      <nldd-card v-for="b in open" :key="b.id">
        <nldd-container padding="16" gap="12">
          <nldd-title
            size="5"
            :text="`Beroep ${kort(b.id)} tegen beslissing ${kort(b.received.id)} van ${state.casus.cel_naam}`"
            :supporting-text="`ontvangen ${b.recorded_on} · ${b.fields.gronden_bezwaar_of_beroep ?? ''}`"
          ></nldd-title>
          <nldd-list appearance="box-base">
            <nldd-list-item v-for="u in uitvoer(b)" :key="u.naam" size="sm">
              <nldd-text-cell :text="leesbaar(u.naam)"></nldd-text-cell>
              <nldd-text-cell width="fit-content" :text="waarde(u.w)"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-container layout="grid" gap="16">
            <nldd-container gap="12">
              <UitScenario soort="uitspraak" />
              <VeldInvoer
                v-for="o in eigen"
                :key="o.naam"
                :model-value="oordeelVoor(b.id)[o.naam]"
                :veld="o"
                @update:model-value="uitspraakVorm.zet(b.id, o.naam, $event)"
              />
              <template v-if="gelezen.length">
                <nldd-text size="sm" color="secondary">
                  Wat {{ gelezenArtikelen }} vraagt, telt alleen als de uitspraak dat artikel leest:
                  bij een gegrond beroep, en wat het bestreden besluit vraagt alleen als de rechtbank
                  zelf in de zaak voorziet.
                </nldd-text>
                <VeldInvoer
                  v-for="o in gelezen"
                  :key="o.naam"
                  :model-value="oordeelVoor(b.id)[o.naam]"
                  :veld="o"
                  @update:model-value="uitspraakVorm.zet(b.id, o.naam, $event)"
                />
              </template>
              <nldd-button appearance="primary" text="Uitspraak doen" @click="uitspraakDoen(b)"></nldd-button>
            </nldd-container>
            <Voorvertoning soort="uitspraak" :op="b.id" :bevindingen="zonderLeeg(oordeelVoor(b.id))" />
          </nldd-container>
        </nldd-container>
      </nldd-card>
    </nldd-container>
  </nldd-simple-section>

  <nldd-simple-section>
    <nldd-title slot="header" size="3" text="Uitspraken">
      <nldd-text slot="supporting-text">
        Elke uitspraak, met wat zij inhoudt. Toezenden geeft {{ state.casus.cel_naam }} een
        afschrift (Awb 8:79); zij legt de ontvangst vast in haar eigen kroniek, op de speeldatum.
      </nldd-text>
    </nldd-title>
    <nldd-inline-dialog v-if="uitspraken.length === 0" icon="info" text="Nog geen uitspraken"></nldd-inline-dialog>
    <nldd-container gap="16">
      <nldd-card v-for="u in uitspraken" :key="u.id">
        <nldd-container padding="16" gap="12">
          <nldd-title
            size="5"
            :text="`Uitspraak ${kort(u.id)} op beroep ${kort(u.refers_to)} (${u.recorded_on})`"
            :supporting-text="u.outputs.uitspraak_strekt_tot"
          >
            <nldd-tag
              v-if="ontvangstVan(u.id)"
              slot="end"
              color="success"
              :text="`Toegezonden, ontvangen ${ontvangstVan(u.id).recorded_on}`"
            ></nldd-tag>
          </nldd-title>
          <nldd-list appearance="box-base">
            <nldd-list-item v-for="o in uitvoer(u)" :key="o.naam" size="sm">
              <nldd-text-cell :text="leesbaar(o.naam)"></nldd-text-cell>
              <!-- Een besluitwaarde is lang: zij krijgt de ruimte, niet de naam. -->
              <nldd-text-cell :width="isBesluitwaarde(o.w) ? undefined : 'fit-content'" :text="waarde(o.w)"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-text v-if="nietGevraagd[u.id]?.length" size="sm" color="secondary">
            Niet gevraagd, dus niet gebruikt: {{ nietGevraagd[u.id].map(leesbaar).join(', ') }}.
          </nldd-text>
          <nldd-button
            v-if="!ontvangstVan(u.id)"
            appearance="primary"
            :text="`Toezenden aan ${state.casus.cel_naam}`"
            @click="toezenden(u)"
          ></nldd-button>
        </nldd-container>
      </nldd-card>
    </nldd-container>
  </nldd-simple-section>
</template>
