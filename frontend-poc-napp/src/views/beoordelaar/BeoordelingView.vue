<script setup>
import { computed, onMounted, ref } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import PortalHeader from '../../components/PortalHeader.vue';
import NBanner from '../../components/NBanner.vue';
import LifecycleTimeline from '../../components/LifecycleTimeline.vue';
import { api } from '../../api.js';
import { euro, datum, datumTijd, BETAAL_LABELS, betaalKleur, BESLISSING_LABELS } from '../../format.js';

const route = useRoute();
const router = useRouter();

const item = ref(null);
const uitkomst = ref(null);
const fout = ref('');
const bezig = ref(false);

const eigen = computed(() => item.value?.aanvraag.parameters ?? {});

const verklaringen = computed(() => {
  const p = eigen.value;
  return [
    { label: 'Geen anonieme giften', ok: p.ontvangt_anonieme_giften === false },
    { label: 'Geen giften van niet-ingezetenen', ok: p.ontvangt_giften_niet_ingezetenen === false },
    { label: 'Meldplicht giften ≥ € 10.000 nageleefd', ok: p.voldoet_aan_meldplicht_giften === true },
    { label: 'Financiën openbaar op website', ok: p.financien_openbaar_op_website === true },
  ];
});

// De specificatie: uit het besluit (na vaststelling) of de proefberekening.
const specificatie = computed(
  () => item.value?.besluit?.componenten ?? uitkomst.value?.componenten ?? [],
);

const samenvattingPerGroep = computed(() => {
  const groepen = new Map();
  for (const c of specificatie.value) {
    const naam = c.soort === 'LANDELIJK'
      ? 'Landelijk'
      : { GEMEENTERAAD: 'Gemeenteraden', PROVINCIALE_STATEN: 'Provinciale staten', EILANDSRAAD: 'Eilandsraden', WATERSCHAP: 'Waterschappen' }[c.orgaan] ?? c.orgaan;
    const g = groepen.get(naam) ?? { naam, aantal: 0, toegekend: 0, bedrag: 0 };
    g.aantal += 1;
    if (c.toegekend) {
      g.toegekend += 1;
      g.bedrag += c.bedrag;
    }
    groepen.set(naam, g);
  }
  return [...groepen.values()];
});

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
  const c = specificatie.value.find((x) => x.soort === 'LANDELIJK' && x.delen);
  if (!c) return null;
  return [
    { label: 'Politieke partij (onderdeel a)', bedrag: c.delen.partij },
    { label: 'Politiek-wetenschappelijk instituut (b)', bedrag: c.delen.wetenschappelijk_instituut },
    { label: 'Politieke jongerenorganisatie (c)', bedrag: c.delen.jongerenorganisatie },
    { label: 'Instelling buitenlandse activiteiten (d)', bedrag: c.delen.buitenland },
  ].filter((d) => d.bedrag > 0);
});

const neveninstellingen = computed(() => {
  const p = eigen.value;
  const items = [];
  if (p.heeft_wetenschappelijk_instituut) items.push('politiek-wetenschappelijk instituut');
  if (p.heeft_jongerenorganisatie) {
    items.push(`politieke jongerenorganisatie (${p.aantal_leden_jongerenorganisatie ?? 0} leden)`);
  }
  if (p.heeft_instelling_buitenland) items.push('instelling voor buitenlandse activiteiten');
  return items;
});

const voorschot = computed(() => {
  const bron = item.value?.besluit ?? uitkomst.value;
  if (!bron?.subsidie_toegekend) return null;
  // De proefberekening levert het voorschot rechtstreeks; bij een
  // vastgesteld besluit volgt het uit art. 17 (80% van het verleende bedrag).
  return uitkomst.value?.betaalopdracht_bedrag ?? Math.round(bron.subsidiebedrag * 0.8);
});

async function laad() {
  try {
    item.value = await api.aanvraag(route.params.id);
    if (item.value.aanvraag.status === 'BEHANDELING') {
      uitkomst.value = await api.proefberekening(route.params.id);
    }
  } catch (e) {
    fout.value = e.message;
  }
}

async function stelVast() {
  bezig.value = true;
  fout.value = '';
  try {
    await api.stelBesluitVast(route.params.id);
    uitkomst.value = null;
    await laad();
  } catch (e) {
    fout.value = e.message;
  } finally {
    bezig.value = false;
  }
}

async function maakBekend() {
  bezig.value = true;
  fout.value = '';
  try {
    await api.bekendmaking(route.params.id);
    await laad();
  } catch (e) {
    fout.value = e.message;
  } finally {
    bezig.value = false;
  }
}

// Voorschot/betaalopdracht bij dit dossier (indien het besluit er een
// opleverde). Uitbetalen is de feitelijke handeling naar het
// (gesimuleerde) betaalsysteem.
async function betaalUit() {
  bezig.value = true;
  fout.value = '';
  try {
    await api.betaalopdrachtUitbetalen(item.value.betaalopdracht.id);
    await laad();
  } catch (e) {
    fout.value = e.message;
  } finally {
    bezig.value = false;
  }
}

// Bezwaarbehandeling (Awb hoofdstuk 7): horen of geldig afzien (7:3),
// daarna beslissen; bij gegrond volgt volledige heroverweging (7:11).
const bezwaarFout = ref('');
const afzienGrond = ref('KENNELIJK_ONGEGROND');
const beslisKeuze = ref('ONGEGROND');
const correctieLeden = ref('');

const AFZIEN_GRONDEN = [
  { value: 'KENNELIJK_NIET_ONTVANKELIJK', label: 'Kennelijk niet-ontvankelijk' },
  { value: 'KENNELIJK_ONGEGROND', label: 'Kennelijk ongegrond' },
  { value: 'INDIENER_ZIET_AF', label: 'Indiener ziet af van horen' },
  { value: 'VOLLEDIG_TEGEMOETGEKOMEN', label: 'Volledig tegemoetgekomen' },
];

async function registreerHoren(gehoord) {
  bezwaarFout.value = '';
  bezig.value = true;
  try {
    await api.bezwaarHoren(item.value.bezwaar.id, {
      gehoord,
      afzien_grond: gehoord ? undefined : afzienGrond.value,
    });
    await laad();
  } catch (e) {
    bezwaarFout.value = e.message;
  } finally {
    bezig.value = false;
  }
}

async function beslisBezwaar() {
  bezwaarFout.value = '';
  bezig.value = true;
  try {
    const payload = { beslissing: beslisKeuze.value };
    const leden = parseInt(correctieLeden.value, 10);
    if (beslisKeuze.value === 'GEGROND' && Number.isFinite(leden)) {
      payload.gecorrigeerde_parameters = { aantal_betalende_leden: leden };
    }
    await api.bezwaarBeslissen(item.value.bezwaar.id, payload);
    await laad();
  } catch (e) {
    bezwaarFout.value = e.message;
  } finally {
    bezig.value = false;
  }
}

function bezwaarVeld(event) {
  return event.detail?.value ?? event.target?.value ?? '';
}

onMounted(laad);
</script>

<template>
  <nldd-page>
    <PortalHeader
      slot="header"
      portal="beoordelaar"
      :items="[{ text: 'Werkvoorraad', to: '/' }, { text: 'Partijregister', to: '/partijregister' }, { text: 'Scenario’s', to: '/scenarios' }]"
    />

    <template v-if="item">
      <nldd-simple-section>
        <nldd-button
          appearance="neutral-transparent"
          size="sm"
          text="Terug naar de werkvoorraad"
          start-icon="chevron-left"
          @click="router.push('/')"
        ></nldd-button>
        <nldd-spacer size="16"></nldd-spacer>

        <nldd-title size="2">
          <span slot="overline">Jaaraanvraag {{ item.aanvraag.subsidiejaar }} · KVK {{ item.aanvraag.kvk_nummer }}</span>
          <h2>{{ item.aanvraag.partij_naam }}</h2>
        </nldd-title>
        <nldd-spacer size="24"></nldd-spacer>

        <nldd-one-half-one-half-section padding-block="0">
          <div slot="left">
            <nldd-title size="4"><h3>Eigen opgaven</h3></nldd-title>
            <nldd-spacer size="12"></nldd-spacer>
            <nldd-list appearance="box-tinted">
              <nldd-list-item size="sm">
                <nldd-text-cell text="Onderdelen in deze aanvraag" color="secondary"></nldd-text-cell>
                <nldd-text-cell :text="String(item.aanvraag.componenten.length)" horizontal-alignment="right"></nldd-text-cell>
              </nldd-list-item>
              <nldd-list-item size="sm">
                <nldd-text-cell text="Betalende leden · eigen opgave" color="secondary"></nldd-text-cell>
                <nldd-text-cell :text="String(eigen.aantal_betalende_leden ?? 0)" horizontal-alignment="right"></nldd-text-cell>
              </nldd-list-item>
              <nldd-list-item v-if="neveninstellingen.length" size="sm">
                <nldd-text-cell text="Aangewezen neveninstellingen" color="secondary"></nldd-text-cell>
                <nldd-text-cell :text="neveninstellingen.join(', ')" horizontal-alignment="right"></nldd-text-cell>
              </nldd-list-item>
              <nldd-list-item v-if="item.aanvraag.beslistermijn_einddatum" size="sm">
                <nldd-text-cell text="Besluit vóór · Wpp art. 17" color="secondary"></nldd-text-cell>
                <nldd-text-cell :text="datum(item.aanvraag.beslistermijn_einddatum)" horizontal-alignment="right"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
            <nldd-spacer size="24"></nldd-spacer>

            <nldd-title size="4"><h3>Transparantieverklaringen (art. 5)</h3></nldd-title>
            <nldd-spacer size="12"></nldd-spacer>
            <nldd-list appearance="box-tinted">
              <nldd-list-item v-for="v in verklaringen" :key="v.label" size="sm">
                <nldd-icon-cell
                  :icon="v.ok ? 'check-mark-circle' : 'dismiss-circle'"
                  :color="v.ok ? 'success' : 'critical'"
                  size="20"
                ></nldd-icon-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-text-cell :text="v.label"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>

            <template v-if="item.besluit">
              <nldd-spacer size="24"></nldd-spacer>
              <nldd-title size="4"><h3>Verloop</h3></nldd-title>
              <nldd-spacer size="12"></nldd-spacer>
              <LifecycleTimeline :aanvraag="item.aanvraag" :besluit="item.besluit" :bezwaar="item.bezwaar" />
              <nldd-spacer size="16"></nldd-spacer>
              <nldd-button
                v-if="item.aanvraag.status === 'BESLUIT'"
                appearance="primary"
                text="Besluit bekendmaken"
                start-icon="paper-plane"
                :disabled="bezig || undefined"
                @click="maakBekend"
              ></nldd-button>
              <nldd-rich-text v-else-if="item.besluit.bezwaartermijn_einddatum">
                <p>
                  Bekendgemaakt op {{ datum(item.besluit.bekendmaking_datum) }}.
                  De bezwaartermijn loopt tot en met
                  {{ datum(item.besluit.bezwaartermijn_einddatum) }}.
                </p>
              </nldd-rich-text>
            </template>

            <template v-if="item.betaalopdracht">
              <nldd-spacer size="24"></nldd-spacer>
              <nldd-title size="4">
                <h3>Voorschot (art. 16/17 Wpp)</h3>
                <div slot="actions">
                  <nldd-tag
                    :color="betaalKleur(item.betaalopdracht.status)"
                    :text="BETAAL_LABELS[item.betaalopdracht.status] ?? item.betaalopdracht.status"
                  ></nldd-tag>
                </div>
              </nldd-title>
              <nldd-spacer size="12"></nldd-spacer>
              <nldd-list appearance="box-tinted">
                <nldd-list-item size="sm">
                  <nldd-text-cell text="Bedrag (80% van rechtswege)" color="secondary"></nldd-text-cell>
                  <nldd-text-cell :text="euro(item.betaalopdracht.bedrag)" horizontal-alignment="right"></nldd-text-cell>
                </nldd-list-item>
                <nldd-list-item size="sm">
                  <nldd-text-cell text="Rekening" color="secondary"></nldd-text-cell>
                  <nldd-text-cell
                    :text="item.betaalopdracht.iban ?? 'Nog geen rekening bekend (art. 27)'"
                    horizontal-alignment="right"
                  ></nldd-text-cell>
                </nldd-list-item>
                <nldd-list-item v-if="item.betaalopdracht.betaaltermijn_einddatum" size="sm">
                  <nldd-text-cell text="Betalen vóór · Awb 4:87" color="secondary"></nldd-text-cell>
                  <nldd-text-cell
                    :text="datum(item.betaalopdracht.betaaltermijn_einddatum)"
                    horizontal-alignment="right"
                  ></nldd-text-cell>
                </nldd-list-item>
                <nldd-list-item v-if="item.betaalopdracht.uitgevoerd_at" size="sm">
                  <nldd-text-cell text="Uitbetaald op" color="secondary"></nldd-text-cell>
                  <nldd-text-cell :text="datumTijd(item.betaalopdracht.uitgevoerd_at)" horizontal-alignment="right"></nldd-text-cell>
                </nldd-list-item>
              </nldd-list>
              <template v-if="item.betaalopdracht.status === 'AANGEMAAKT'">
                <nldd-spacer size="12"></nldd-spacer>
                <nldd-button
                  appearance="secondary"
                  text="Uitbetalen (gesimuleerd betaalsysteem)"
                  start-icon="euro-sign"
                  :disabled="bezig || undefined"
                  @click="betaalUit"
                ></nldd-button>
              </template>
            </template>

            <template v-if="item.bezwaar">
              <nldd-spacer size="24"></nldd-spacer>
              <nldd-title size="4">
                <h3>Bezwaar (Awb hoofdstuk 6/7)</h3>
                <div slot="actions">
                  <nldd-tag
                    :color="item.bezwaar.beslissing ? (item.bezwaar.beslissing === 'GEGROND' ? 'success' : 'critical') : 'accent'"
                    :text="item.bezwaar.beslissing ? (BESLISSING_LABELS[item.bezwaar.beslissing] ?? item.bezwaar.beslissing) : item.bezwaar.status"
                  ></nldd-tag>
                </div>
              </nldd-title>
              <nldd-spacer size="12"></nldd-spacer>
              <nldd-list appearance="box-tinted">
                <nldd-list-item size="sm">
                  <nldd-text-cell text="Indiener" color="secondary"></nldd-text-cell>
                  <nldd-text-cell :text="item.bezwaar.naam_indiener" horizontal-alignment="right"></nldd-text-cell>
                </nldd-list-item>
                <nldd-list-item size="sm">
                  <nldd-text-cell text="Gronden" color="secondary"></nldd-text-cell>
                  <nldd-text-cell :text="item.bezwaar.gronden || '— (vormgebrek)'" horizontal-alignment="right"></nldd-text-cell>
                </nldd-list-item>
                <nldd-list-item size="sm">
                  <nldd-text-cell text="Tijdig (Awb 6:9) · oordeel wet" color="secondary"></nldd-text-cell>
                  <nldd-cell width="fit-content">
                    <nldd-tag
                      :color="item.bezwaar.toets?.tijdig ? 'success' : 'critical'"
                      :text="item.bezwaar.toets?.tijdig ? 'Tijdig' : 'Niet tijdig'"
                    ></nldd-tag>
                  </nldd-cell>
                </nldd-list-item>
                <nldd-list-item size="sm">
                  <nldd-text-cell text="Vereisten (Awb 6:5) · oordeel wet" color="secondary"></nldd-text-cell>
                  <nldd-cell width="fit-content">
                    <nldd-tag
                      :color="item.bezwaar.toets?.voldoet_aan_vereisten ? 'success' : 'warning'"
                      :text="item.bezwaar.toets?.voldoet_aan_vereisten ? 'Compleet' : 'Onvolledig'"
                    ></nldd-tag>
                  </nldd-cell>
                </nldd-list-item>
                <nldd-list-item v-if="item.bezwaar.beslistermijn_einddatum" size="sm">
                  <nldd-text-cell text="Beslissen vóór · Awb 7:10" color="secondary"></nldd-text-cell>
                  <nldd-text-cell :text="datum(item.bezwaar.beslistermijn_einddatum)" horizontal-alignment="right"></nldd-text-cell>
                </nldd-list-item>
                <nldd-list-item v-if="item.bezwaar.gehoord !== null" size="sm">
                  <nldd-text-cell text="Horen (Awb 7:2/7:3)" color="secondary"></nldd-text-cell>
                  <nldd-text-cell
                    :text="item.bezwaar.gehoord ? 'Gehoord' : `Afgezien: ${item.bezwaar.afzien_grond}`"
                    horizontal-alignment="right"
                  ></nldd-text-cell>
                </nldd-list-item>
              </nldd-list>

              <template v-if="!item.bezwaar.beslissing && item.bezwaar.status === 'BEHANDELING'">
                <template v-if="item.bezwaar.gehoord === null">
                  <nldd-spacer size="12"></nldd-spacer>
                  <nldd-button
                    appearance="secondary"
                    text="Indiener gehoord"
                    :disabled="bezig || undefined"
                    @click="registreerHoren(true)"
                  ></nldd-button>
                  <nldd-spacer size="8"></nldd-spacer>
                  <nldd-form-field label="Of afzien van horen (Awb 7:3)">
                    <nldd-dropdown>
                      <select :value="afzienGrond" @change="afzienGrond = bezwaarVeld($event)">
                        <option v-for="g in AFZIEN_GRONDEN" :key="g.value" :value="g.value">{{ g.label }}</option>
                      </select>
                    </nldd-dropdown>
                  </nldd-form-field>
                  <nldd-spacer size="8"></nldd-spacer>
                  <nldd-button
                    appearance="neutral-tinted"
                    text="Afzien van horen"
                    :disabled="bezig || undefined"
                    @click="registreerHoren(false)"
                  ></nldd-button>
                </template>
                <template v-else>
                  <nldd-spacer size="12"></nldd-spacer>
                  <nldd-form-field label="Beslissing op bezwaar">
                    <nldd-dropdown>
                      <select :value="beslisKeuze" @change="beslisKeuze = bezwaarVeld($event)">
                        <option value="NIET_ONTVANKELIJK">Niet-ontvankelijk</option>
                        <option value="ONGEGROND">Ongegrond</option>
                        <option value="GEGROND">Gegrond (heroverweging, Awb 7:11)</option>
                      </select>
                    </nldd-dropdown>
                  </nldd-form-field>
                  <template v-if="beslisKeuze === 'GEGROND'">
                    <nldd-form-field label="Gecorrigeerd ledental (eigen opgave)">
                      <nldd-text-field
                        :value="correctieLeden"
                        name="correctie-leden"
                        placeholder="bijv. 24000"
                        @input="correctieLeden = bezwaarVeld($event)"
                      ></nldd-text-field>
                      <nldd-form-field-help-text>
                        De wet wordt opnieuw uitgevoerd op de gecorrigeerde
                        feiten; het besluit en het bewijs worden herzien.
                      </nldd-form-field-help-text>
                    </nldd-form-field>
                  </template>
                  <nldd-spacer size="8"></nldd-spacer>
                  <nldd-button
                    appearance="primary"
                    text="Beslissing op bezwaar vaststellen"
                    :disabled="bezig || undefined"
                    @click="beslisBezwaar"
                  ></nldd-button>
                </template>
              </template>
              <template v-if="bezwaarFout">
                <nldd-spacer size="8"></nldd-spacer>
                <NBanner variant="critical" :text="bezwaarFout" />
              </template>
              <template v-if="item.bezwaar.beslissing_motivering">
                <nldd-spacer size="12"></nldd-spacer>
                <nldd-rich-text><p>{{ item.bezwaar.beslissing_motivering }}</p></nldd-rich-text>
              </template>
            </template>
          </div>

          <div slot="right">
            <nldd-title size="4">
              <h3>{{ item.besluit ? 'Besluit' : 'Uitkomst volgens de wet' }}</h3>
            </nldd-title>
            <nldd-spacer size="12"></nldd-spacer>

            <template v-if="item.besluit || uitkomst">
              <NBanner
                :variant="(item.besluit ?? uitkomst).subsidie_toegekend ? 'success' : 'critical'"
                :text="(item.besluit ?? uitkomst).subsidie_toegekend
                  ? `${item.besluit ? 'Toegekend' : 'Toekenning'}: ${euro((item.besluit ?? uitkomst).subsidiebedrag)}`
                  : 'Afwijzing'"
                :supporting-text="(item.besluit ?? uitkomst).motivering"
              />
              <nldd-spacer size="16"></nldd-spacer>

              <nldd-list appearance="box-tinted">
                <nldd-list-item v-for="g in samenvattingPerGroep" :key="g.naam" size="sm">
                  <nldd-text-cell
                    :text="g.naam"
                    :supporting-text="`${g.toegekend} van ${g.aantal} toegekend`"
                  ></nldd-text-cell>
                  <nldd-text-cell :text="euro(g.bedrag)" horizontal-alignment="right"></nldd-text-cell>
                </nldd-list-item>
                <nldd-list-item v-if="voorschot !== null" size="sm">
                  <nldd-text-cell
                    text="Voorschot bij verlening (80%, art. 17)"
                    color="secondary"
                  ></nldd-text-cell>
                  <nldd-text-cell :text="euro(voorschot)" horizontal-alignment="right"></nldd-text-cell>
                </nldd-list-item>
              </nldd-list>
              <nldd-spacer size="16"></nldd-spacer>

              <nldd-button
                v-if="item.aanvraag.status === 'BEHANDELING' && uitkomst"
                appearance="primary"
                :text="uitkomst.subsidie_toegekend
                  ? `Besluit vaststellen: toekennen (${euro(uitkomst.subsidiebedrag)})`
                  : 'Besluit vaststellen: afwijzen'"
                start-icon="check-mark"
                :disabled="bezig || undefined"
                @click="stelVast"
              ></nldd-button>

              <nldd-spacer size="24"></nldd-spacer>
              <nldd-title size="5"><h4>Specificatie per onderdeel</h4></nldd-title>
              <nldd-spacer size="8"></nldd-spacer>
              <nldd-table
                columns="minmax(180px,1fr) 70px 110px 130px"
                accessible-label="Specificatie per onderdeel"
              >
                <nldd-table-row slot="header">
                  <nldd-text-cell text="Onderdeel"></nldd-text-cell>
                  <nldd-text-cell text="Zetels" horizontal-alignment="right"></nldd-text-cell>
                  <nldd-text-cell text="Besluit"></nldd-text-cell>
                  <nldd-text-cell text="Bedrag" horizontal-alignment="right"></nldd-text-cell>
                </nldd-table-row>
                <nldd-table-row v-for="c in specificatie" :key="c.key">
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
                <nldd-title size="5"><h4>Landelijke subsidie in delen (art. 14)</h4></nldd-title>
                <nldd-spacer size="8"></nldd-spacer>
                <nldd-list appearance="box-tinted">
                  <nldd-list-item v-for="d in landelijkeDelen" :key="d.label" size="sm">
                    <nldd-text-cell :text="d.label" color="secondary"></nldd-text-cell>
                    <nldd-text-cell :text="euro(d.bedrag)" horizontal-alignment="right"></nldd-text-cell>
                  </nldd-list-item>
                </nldd-list>
              </template>
            </template>
          </div>
        </nldd-one-half-one-half-section>

        <template v-if="fout">
          <nldd-spacer size="16"></nldd-spacer>
          <NBanner variant="critical" text="Er ging iets mis" :supporting-text="fout" />
        </template>
      </nldd-simple-section>
    </template>
  </nldd-page>
</template>
