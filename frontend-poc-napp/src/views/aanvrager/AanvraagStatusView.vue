<script setup>
import { computed, onMounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import PortalHeader from '../../components/PortalHeader.vue';
import NBanner from '../../components/NBanner.vue';
import LifecycleTimeline from '../../components/LifecycleTimeline.vue';
import { api } from '../../api.js';
import { euro, datum, datumTijd, onderdelen, BESLISSING_LABELS } from '../../format.js';

const route = useRoute();
const router = useRouter();
const item = ref(null);
const fout = ref('');

const componenten = computed(() => item.value?.besluit?.componenten ?? []);
const toegekendeComponenten = computed(() => componenten.value.filter((c) => c.toegekend));

function componentLabel(c) {
  if (c.soort === 'LANDELIJK') return 'Landelijke subsidie';
  const orgaan = {
    GEMEENTERAAD: 'Gemeenteraad',
    PROVINCIALE_STATEN: 'Provinciale staten',
    EILANDSRAAD: 'Eilandsraad',
    WATERSCHAP: 'Waterschap',
  }[c.orgaan] ?? c.orgaan;
  return `${orgaan} ${c.gebied ?? ''}`;
}

// De vier delen van art. 14 voor de landelijke component (indien toegekend).
const landelijkeDelen = computed(() => {
  const c = componenten.value.find((x) => x.soort === 'LANDELIJK' && x.delen);
  if (!c) return null;
  return [
    { label: 'Politieke partij (onderdeel a)', bedrag: c.delen.partij },
    { label: 'Politiek-wetenschappelijk instituut (b)', bedrag: c.delen.wetenschappelijk_instituut },
    { label: 'Politieke jongerenorganisatie (c)', bedrag: c.delen.jongerenorganisatie },
    { label: 'Instelling buitenlandse activiteiten (d)', bedrag: c.delen.buitenland },
  ].filter((d) => d.bedrag > 0);
});

async function laad() {
  try {
    item.value = await api.mijnAanvraag(route.params.id);
  } catch (e) {
    fout.value = e.message;
  }
}
onMounted(laad);

// Bezwaar maken (Awb 6:4 e.v.): het formulier levert de feiten voor de
// vereisten van 6:5; de wet oordeelt (incl. herstelgelegenheid, 6:6).
const bezwaarOpen = ref(false);
const bezwaarBezig = ref(false);
const bezwaarFout = ref('');
const bezwaarForm = ref({ naam: '', adres: '', gronden: '', ondertekend: false });

function veld(event) {
  return event.detail?.value ?? event.target?.value ?? '';
}

async function dienBezwaarIn() {
  bezwaarFout.value = '';
  bezwaarBezig.value = true;
  try {
    await api.dienBezwaarIn(item.value.besluit.id, {
      naam_indiener: bezwaarForm.value.naam.trim(),
      adres_indiener: bezwaarForm.value.adres.trim(),
      gronden: bezwaarForm.value.gronden.trim(),
      ondertekend: bezwaarForm.value.ondertekend,
    });
    bezwaarOpen.value = false;
    await laad();
  } catch (e) {
    bezwaarFout.value = e.message;
  } finally {
    bezwaarBezig.value = false;
  }
}

async function herstelBezwaar() {
  bezwaarFout.value = '';
  bezwaarBezig.value = true;
  try {
    await api.herstelBezwaar(item.value.bezwaar.id, {
      naam_indiener: bezwaarForm.value.naam.trim() || undefined,
      adres_indiener: bezwaarForm.value.adres.trim() || undefined,
      gronden: bezwaarForm.value.gronden.trim() || undefined,
      ondertekend: bezwaarForm.value.ondertekend || undefined,
    });
    await laad();
  } catch (e) {
    bezwaarFout.value = e.message;
  } finally {
    bezwaarBezig.value = false;
  }
}


// Welke 6:5-vereisten ontbreken volgens de wet (outputs van de toets);
// de herstelbanner benoemt ze, zodat duidelijk is wat aangevuld moet.
const ontbrekendeVereisten = computed(() => {
  const o = item.value?.bezwaar?.toets?.outputs ?? {};
  const eisen = [
    ['ontbreekt_naam_en_adres', 'naam en adres van de indiener'],
    ['ontbreken_gronden', 'de gronden van het bezwaar'],
    ['ontbreekt_ondertekening', 'de ondertekening'],
  ];
  return eisen.filter(([k]) => o[k]).map(([, label]) => label);
});

// Vul het herstelformulier vooraf met wat er al is, zodat alleen het
// ontbrekende hoeft te worden aangevuld.
watch(item, (i) => {
  const b = i?.bezwaar;
  if (b && b.status === 'HERSTEL') {
    bezwaarForm.value = {
      naam: b.naam_indiener ?? '',
      adres: b.adres_indiener ?? '',
      gronden: b.gronden ?? '',
      ondertekend: b.ondertekend ?? false,
    };
  }
});
</script>

<template>
  <nldd-page>
    <PortalHeader
      slot="header"
      portal="aanvrager"
      :items="[{ text: 'Mijn aanvragen', to: '/' }, { text: 'Nieuwe aanvraag', to: '/nieuw' }, { text: 'Mijn organisatie', to: '/organisatie' }]"
    />

    <nldd-simple-section v-if="fout" width="560px">
      <NBanner variant="critical" text="Aanvraag niet gevonden" :supporting-text="fout" />
    </nldd-simple-section>

    <template v-else-if="item">
      <nldd-simple-section width="820px">
        <nldd-button
          appearance="neutral-transparent"
          size="sm"
          text="Terug naar uw aanvragen"
          start-icon="chevron-left"
          @click="router.push('/')"
        ></nldd-button>
        <nldd-spacer size="16"></nldd-spacer>

        <nldd-title size="display-2">
          <span slot="overline">{{ item.aanvraag.partij_naam }}</span>
          <h2>Jaaraanvraag {{ item.aanvraag.subsidiejaar }} · {{ onderdelen(item.aanvraag.componenten.length) }}</h2>
        </nldd-title>
        <nldd-spacer size="24"></nldd-spacer>

        <NBanner
          v-if="item.besluit"
          :variant="item.besluit.subsidie_toegekend ? 'success' : 'critical'"
          :text="item.besluit.subsidie_toegekend
            ? `Subsidie toegekend: ${euro(item.besluit.subsidiebedrag)}`
            : 'Uw aanvraag is afgewezen'"
          :supporting-text="item.besluit.motivering"
        />
        <NBanner
          v-else
          variant="accent"
          text="Uw aanvraag is in behandeling"
          supporting-text="De Napp toetst uw aanvraag aan de Wet op de politieke partijen."
        />

        <!-- Het voorschot dat uit het besluit voortvloeit (art. 16/17 Wpp). -->
        <template v-if="item.betaalopdracht">
          <nldd-spacer size="12"></nldd-spacer>
          <NBanner
            v-if="item.betaalopdracht.status === 'AANGEHOUDEN'"
            variant="warning"
            :text="`Uitbetaling van het voorschot (${euro(item.betaalopdracht.bedrag)}) is aangehouden`"
            supporting-text="Er is nog geen rekeningnummer van uw rechtspersoon bekend (artikel 27 Wpp). Zodra het bestuur een rekening opgeeft bij Mijn organisatie, wordt de uitbetaling klaargezet."
          />
          <NBanner
            v-else-if="item.betaalopdracht.status === 'UITBETAALD'"
            variant="success"
            :text="`Het voorschot van ${euro(item.betaalopdracht.bedrag)} is uitbetaald`"
            :supporting-text="`Overgemaakt naar ${item.betaalopdracht.iban} op ${datumTijd(item.betaalopdracht.uitgevoerd_at)}.`"
          />
          <NBanner
            v-else
            variant="accent"
            :text="`Het voorschot van ${euro(item.betaalopdracht.bedrag)} staat klaar voor uitbetaling`"
            :supporting-text="item.betaalopdracht.betaaltermijn_einddatum
              ? `Uitbetaling op ${item.betaalopdracht.iban}, uiterlijk ${datum(item.betaalopdracht.betaaltermijn_einddatum)} (Awb 4:87).`
              : `Uitbetaling op ${item.betaalopdracht.iban}.`"
          />
        </template>
        <nldd-spacer size="32"></nldd-spacer>

        <template v-if="item.besluit">
          <nldd-title size="display-4"><h3>Specificatie per onderdeel</h3></nldd-title>
          <nldd-spacer size="12"></nldd-spacer>
          <nldd-table
            columns="minmax(220px,1fr) 110px 120px 150px"
            accessible-label="Specificatie van het besluit"
          >
            <nldd-table-row slot="header">
              <nldd-text-cell text="Onderdeel"></nldd-text-cell>
              <nldd-text-cell text="Zetels" horizontal-alignment="right"></nldd-text-cell>
              <nldd-text-cell text="Besluit"></nldd-text-cell>
              <nldd-text-cell text="Bedrag" horizontal-alignment="right"></nldd-text-cell>
            </nldd-table-row>
            <nldd-table-row v-for="c in componenten" :key="c.key">
              <nldd-text-cell :text="componentLabel(c)"></nldd-text-cell>
              <nldd-text-cell :text="String(c.zetels)" horizontal-alignment="right"></nldd-text-cell>
              <nldd-cell>
                <nldd-tag
                  :color="c.toegekend ? 'success' : 'critical'"
                  :text="c.toegekend ? 'Toegekend' : 'Afgewezen'"
                ></nldd-tag>
              </nldd-cell>
              <nldd-text-cell
                :text="c.toegekend ? euro(c.bedrag) : '—'"
                horizontal-alignment="right"
              ></nldd-text-cell>
            </nldd-table-row>
          </nldd-table>

          <template v-if="landelijkeDelen">
            <nldd-spacer size="16"></nldd-spacer>
            <nldd-title size="display-5"><h4>Landelijke subsidie in delen (art. 14)</h4></nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-list appearance="box-tinted">
              <nldd-list-item v-for="d in landelijkeDelen" :key="d.label" size="sm">
                <nldd-text-cell :text="d.label" color="secondary"></nldd-text-cell>
                <nldd-text-cell :text="euro(d.bedrag)" horizontal-alignment="right"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
          </template>
          <nldd-spacer size="32"></nldd-spacer>
        </template>

        <nldd-title size="display-4"><h3>Verloop van uw aanvraag</h3></nldd-title>
        <nldd-spacer size="12"></nldd-spacer>
        <LifecycleTimeline :aanvraag="item.aanvraag" :besluit="item.besluit" :bezwaar="item.bezwaar" />

        <template v-if="item.besluit?.bezwaartermijn_einddatum">
          <nldd-spacer size="24"></nldd-spacer>

          <!-- Lopend of beslist bezwaar -->
          <template v-if="item.bezwaar && (item.bezwaar.beslissing || item.bezwaar.status === 'HERSTEL')">
            <nldd-title size="display-4"><h3>Uw bezwaar</h3></nldd-title>
            <nldd-spacer size="12"></nldd-spacer>
            <NBanner
              v-if="item.bezwaar.beslissing"
              :variant="item.bezwaar.beslissing === 'GEGROND' ? 'success' : 'critical'"
              :text="`Beslissing op bezwaar: ${BESLISSING_LABELS[item.bezwaar.beslissing] ?? item.bezwaar.beslissing}`"
              :supporting-text="item.bezwaar.beslissing_motivering"
            />
            <template v-else-if="item.bezwaar.status === 'HERSTEL'">
              <NBanner
                variant="warning"
                text="Uw bezwaarschrift is onvolledig"
                :supporting-text="`Er ontbreekt: ${ontbrekendeVereisten.join(', ') || 'een of meer vereisten'} (artikel 6:5 Awb). U krijgt gelegenheid het verzuim te herstellen (artikel 6:6); anders kan het bezwaar niet-ontvankelijk worden verklaard.`"
              />
              <nldd-spacer size="12"></nldd-spacer>
              <nldd-form novalidate @submit.prevent="herstelBezwaar">
                <nldd-form-field label="Naam van de indiener">
                  <nldd-text-field
                    :value="bezwaarForm.naam"
                    name="naam"
                    @input="bezwaarForm.naam = veld($event)"
                  ></nldd-text-field>
                </nldd-form-field>
                <nldd-form-field label="Gronden van het bezwaar">
                  <nldd-multi-line-text-field
                    :value="bezwaarForm.gronden"
                    name="gronden"
                    rows="4"
                    resize="auto"
                    @input="bezwaarForm.gronden = veld($event)"
                  ></nldd-multi-line-text-field>
                </nldd-form-field>
                <nldd-form-field label="Adres van de indiener">
                  <nldd-text-field
                    :value="bezwaarForm.adres"
                    name="adres"
                    @input="bezwaarForm.adres = veld($event)"
                  ></nldd-text-field>
                </nldd-form-field>
                <nldd-form-field label="Ondertekening">
                  <nldd-checkbox-field
                    :checked="bezwaarForm.ondertekend || undefined"
                    label="Ik onderteken dit bezwaarschrift"
                    @change="bezwaarForm.ondertekend = $event.detail?.checked ?? !bezwaarForm.ondertekend"
                  ></nldd-checkbox-field>
                  <nldd-form-field-help-text>
                    Een bezwaarschrift moet ondertekend zijn (artikel 6:5
                    Awb); in dit portaal ondertekent u digitaal met deze
                    verklaring.
                  </nldd-form-field-help-text>
                </nldd-form-field>
                <template v-if="bezwaarFout">
                  <nldd-spacer size="8"></nldd-spacer>
                  <NBanner variant="critical" :text="bezwaarFout" />
                </template>
                <nldd-form-actions>
                  <nldd-button
                    appearance="primary"
                    type="submit"
                    text="Verzuim herstellen"
                    :disabled="bezwaarBezig || undefined"
                  ></nldd-button>
                </nldd-form-actions>
              </nldd-form>
            </template>
          </template>

          <!-- Nog geen bezwaar: informatie + formulier -->
          <template v-else-if="!item.bezwaar">
            <nldd-box>
              <nldd-container padding="16" gap="8">
                <nldd-rich-text>
                  <p>
                    Bent u het niet eens met dit besluit? U kunt tot en met
                    <strong>{{ datum(item.besluit.bezwaartermijn_einddatum) }}</strong>
                    bezwaar maken bij de Nederlandse autoriteit politieke partijen
                    (artikel 6:4 Algemene wet bestuursrecht).
                  </p>
                </nldd-rich-text>
                <div v-if="!bezwaarOpen">
                  <nldd-button
                    appearance="secondary"
                    text="Bezwaar maken"
                    @click="bezwaarOpen = true"
                  ></nldd-button>
                </div>
                <nldd-form v-else novalidate @submit.prevent="dienBezwaarIn">
                  <nldd-form-field label="Naam van de indiener">
                    <nldd-text-field
                      :value="bezwaarForm.naam"
                      name="naam"
                      @input="bezwaarForm.naam = veld($event)"
                    ></nldd-text-field>
                  </nldd-form-field>
                  <nldd-form-field label="Adres">
                    <nldd-text-field
                      :value="bezwaarForm.adres"
                      name="adres"
                      @input="bezwaarForm.adres = veld($event)"
                    ></nldd-text-field>
                  </nldd-form-field>
                  <nldd-form-field label="Gronden van het bezwaar">
                    <nldd-multi-line-text-field
                      :value="bezwaarForm.gronden"
                      name="gronden"
                      rows="4"
                      resize="auto"
                      placeholder="Waarom bent u het niet eens met het besluit?"
                      @input="bezwaarForm.gronden = veld($event)"
                    ></nldd-multi-line-text-field>
                    <nldd-form-field-help-text>
                      Het bezwaarschrift moet de gronden bevatten (artikel 6:5
                      Awb). Ontbreekt er iets, dan krijgt u gelegenheid tot
                      herstel (artikel 6:6).
                    </nldd-form-field-help-text>
                  </nldd-form-field>
                  <nldd-form-field label="Ondertekening">
                    <nldd-checkbox-field
                      :checked="bezwaarForm.ondertekend || undefined"
                      label="Ik onderteken dit bezwaarschrift"
                      @change="bezwaarForm.ondertekend = $event.detail?.checked ?? !bezwaarForm.ondertekend"
                    ></nldd-checkbox-field>
                    <nldd-form-field-help-text>
                      Een bezwaarschrift moet ondertekend zijn (artikel 6:5
                      Awb); in dit portaal ondertekent u digitaal met deze
                      verklaring.
                    </nldd-form-field-help-text>
                  </nldd-form-field>
                  <template v-if="bezwaarFout">
                    <nldd-spacer size="8"></nldd-spacer>
                    <NBanner variant="critical" :text="bezwaarFout" />
                  </template>
                  <nldd-form-actions>
                    <nldd-button-group orientation="horizontal">
                      <nldd-button
                        appearance="primary"
                        type="submit"
                        text="Bezwaarschrift indienen"
                        :disabled="bezwaarBezig || undefined"
                      ></nldd-button>
                      <nldd-button
                        appearance="secondary"
                        text="Annuleren"
                        @click="bezwaarOpen = false"
                      ></nldd-button>
                    </nldd-button-group>
                  </nldd-form-actions>
                </nldd-form>
              </nldd-container>
            </nldd-box>
          </template>
        </template>
      </nldd-simple-section>
    </template>
  </nldd-page>
</template>
