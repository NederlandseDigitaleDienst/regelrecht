<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { useRoute } from 'vue-router';
import CorrectionRows from '../components/CorrectionRows.vue';
import DataLineage from '../components/DataLineage.vue';
import EditValueSheet from '../components/EditValueSheet.vue';
import { fieldSpec, formatDateTime, formatValue, humanize } from '../data/format';
import { lineageFromTrace } from '../data/lineage';
import { caseReason, eventText, useDemo } from '../store/demoStore';
import { isDelegationProvider, producesBeschikking, subjectOf } from '../data/entrypoints';
import { awbOutcomes, statusOf } from '../data/lifecycle';
import { useI18n } from '../i18n/index.js';
import { useLocalePath } from '../i18n/useLocalePath';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import type { DemoCase, DemoClaim } from '../store/demoStore';
import type { LawEntry } from '../data/loadCorpus';
import type { LineageNode } from '../data/lineage';

// Naar een ander tabblad op naam, niet op pad: onder `/en/` leidt een
// letterlijk Nederlands pad de bezoeker ongemerkt het Nederlandse tabblad in.
const { goTo } = useLocalePath();
const { t } = useI18n();

// The caseworker's side: applications the citizen submitted, in three lanes,
// with the engine's fresh verdict next to what the citizen claimed, the
// corrections that wait for a decision, and the decision itself. The caseworker
// sees exactly what the citizen sees (the same data tree under the outcome) and
// can make every change the citizen can; such a correction applies at once.

const route = useRoute();
const demo = useDemo();
const { corpus, profile, state, dataVersion } = demo;

const service = ref<string | null>(null);
watch(() => profile.value?.zaaksysteem_service, (s) => { if (s && !service.value) service.value = s; }, { immediate: true });

// Every organisation that executes a law in the demo has a case system; the
// presenter can step into any of them. The profile picks the one to start in.
const services = computed(() => {
  if (!corpus.value) return [];
  const set = new Set([...corpus.value.latestById.values()].map((l) => l.service).filter((s): s is string => Boolean(s)));
  // `corpus.value!`: checked at the top; the sort runs synchronously within.
  return [...set].sort((a, b) => (corpus.value!.services[a]?.name ?? a).localeCompare(corpus.value!.services[b]?.name ?? b));
});
/**
 * A service's display name, for the template. The view renders only once the
 * corpus has loaded (App.vue). A null code used to index the key "null", which
 * no service has; the result is the same.
 */
function serviceName(code: string | null): string | null {
  return (code === null ? undefined : corpus.value?.services[code]?.name) ?? code;
}
/** The regelingen this organisation executes, with the number of cases per regeling. */
const orgLaws = computed(() => {
  if (!corpus.value || !service.value) return [];
  return [...corpus.value.latestById.values()]
    .filter((l) => l.service === service.value)
    .sort((a, b) => a.name.localeCompare(b.name))
    .map((law) => ({ law, count: state.cases.filter((c) => c.lawId === law.id).length }));
});

const cases = computed(() => state.cases.filter((c) => c.service === service.value));
// De banen volgen de fasen die de Awb aan een beschikking geeft (RFC-008), en
// niet een eigen indeling: te beoordelen, besloten maar nog niet verstuurd, en
// bekendgemaakt. Die middelste baan is de reden van deze verandering — een
// besluit dat genomen is maar nog niet is meegedeeld, is een echt moment in de
// wet en was hier eerder onzichtbaar.
const lanes = computed(() => [
  { key: 'IN_REVIEW', title: t('zaak.lane.in_review'), items: cases.value.filter((c) => statusOf(c) !== 'DECIDED') },
  { key: 'BESLUIT', title: t('zaak.lane.besluit'), items: cases.value.filter((c) => statusOf(c) === 'DECIDED' && !c.publishedAt) },
  { key: 'BEKENDMAKING', title: t('zaak.lane.bekendmaking'), items: cases.value.filter((c) => statusOf(c) === 'DECIDED' && c.publishedAt) },
]);

const selected = computed(() => state.cases.find((c) => c.id === route.params.caseId) ?? null);
watch(selected, (c) => { if (c && c.service !== service.value) service.value = c.service; }, { immediate: true });

// The case opens in a sheet over the board, not in an inspector column beside
// it. A case carries the banner, both outcomes, the whole data tree and the
// corrections; the inspector is a narrow fixed rail and squeezed all of that
// into a column too thin to read. It also matches the citizen's side, where an
// application opens the same way (ApplicationSheet), so the same case looks the
// same from both ends.
const caseSheet = ref<NLDDSheet | null>(null);
watch(
  selected,
  async (c) => {
    if (!c) return caseSheet.value?.hide?.();
    await nextTick();
    caseSheet.value?.show?.();
  },
  { immediate: true },
);

function open(c: DemoCase) {
  goTo('zaaksysteem', { caseId: c.id });
}
function close() {
  goTo('zaaksysteem');
}

function personaName(bsn: string | undefined) {
  // A BSN without a persona (e.g. a case persisted before profiles.yaml
  // changed) is stated as a fact, not passed off as a name.
  // An undefined BSN indexes the key "undefined", which no persona has.
  return corpus.value?.profiles?.profiles?.[bsn as string]?.name ?? t('zaak.unknown_person', { bsn });
}
// Wie de zaak betreft. Een aanvraag namens een onderneming gaat over die
// onderneming en heeft geen BSN; dan staat haar naam er, of haar KvK-nummer.
function subjectName(c: DemoCase) {
  if (c.acting?.subjectType === 'BUSINESS') return c.acting.subjectName;
  if (c.bsn) return personaName(c.bsn);
  if (c.kvk) return t('zaak.portaal.kvk', { number: c.kvk });
  return personaName(c.bsn);
}
function lawOf(c: DemoCase) {
  return corpus.value?.lawById(c.lawId) ?? null;
}

// The engine's current verdict for the selected case (with approved
// corrections applied), next to the citizen's claimed result.
const verified = computed(() => {
  const c = selected.value;
  const law = c ? lawOf(c) : null;
  if (!c || !law) return null;
  void dataVersion.value;
  return demo.evaluate(law, c.parameters);
});

// The same tree the citizen sees in the application: the values the law used,
// keyed on the case's own parameters (not the active profile's).
const lineage = computed(() => {
  const c = selected.value;
  if (!c || !verified.value?.ok || !verified.value.trace) return [];
  return lineageFromTrace(verified.value.trace, c.lawId, c.parameters ?? {});
});
/** The lineage value node the caseworker is correcting; null = sheet closed. */
const editing = ref<LineageNode | null>(null);

function outputRows(c: DemoCase) {
  const law = lawOf(c);
  const claimed = c.claimedResult ?? {};
  // A failed recomputation has no "now" value to compare against: show the
  // claimed values as they are and let the warning banner explain the failure.
  const recomputed = !!verified.value?.ok;
  const now = recomputed ? verified.value.outputs : {};
  const names = [...new Set([...Object.keys(claimed), ...Object.keys(now)])];
  return names.map((name) => ({
    name,
    spec: fieldSpec(law?.doc, name),
    claimed: claimed[name],
    now: recomputed ? now[name] : claimed[name],
    differs: recomputed && JSON.stringify(claimed[name]) !== JSON.stringify(now[name]),
  }));
}

// `selected.value!` inside the filter: the ternary checked it, and the computed
// cannot change while the filter runs.
const caseClaims = computed(() => (selected.value ? state.claims.filter((cl) => cl.caseId === selected.value!.id || (cl.bsn === (selected.value!.claimsBsn ?? selected.value!.bsn) && cl.tileLawId === selected.value!.lawId)) : []));
// A claim without `tileLawId` looks up `undefined`/`null`, which finds no law.
const serviceClaims = computed(() => state.claims.filter((cl) => cl.status === 'PENDING' && (corpus.value?.lawById(cl.tileLawId as string)?.service === service.value || corpus.value?.lawById(cl.lawId)?.service === service.value)));

const reason = ref('');
function decide(approved: boolean) {
  if (!selected.value) return;
  // Without a typed reason, say who decided — not what the law says. A
  // caseworker can approve or reject against the engine's own outcome, and
  // "Voldoet niet aan de voorwaarden" then claimed the law had rejected the
  // citizen while the tile beside it showed the amount the law had granted.
  // Awb art. 3:46 asks a besluit to rest on a deugdelijke motivering; the
  // honest fallback is that none was given.
  const text = reason.value.trim() || t(approved ? 'zaak.granted_no_reason' : 'zaak.refused_no_reason');
  demo.decideCase(selected.value.id, approved, text, verified.value?.ok ? verified.value.outputs : null);
  reason.value = '';
}
/** Het besluit gaat de deur uit (Awb 3:41); daarna pas loopt de termijn. */
function publish() {
  if (!selected.value) return;
  demo.publishCase(selected.value.id);
}
/** Wat de Awb aan dit besluit heeft toegevoegd. */
const awb = computed(() => awbOutcomes(selected.value));

function decideObjection(upheld: boolean) {
  if (!selected.value) return;
  demo.decideObjection(selected.value.id, upheld, reason.value.trim() || t(upheld ? 'zaak.objection.upheld_default' : 'zaak.objection.dismissed_default'));
  reason.value = '';
}

/**
 * De zaken op dit bord die niet meer kloppen met wat de wet nu zegt.
 *
 * Dezelfde vergelijking als in de portal (`caseDrift`), maar dan vanaf de
 * andere kant: een behandelaar moet kunnen zien dát er iets is veranderd
 * zonder elke zaak open te klikken. Alleen de zaken van deze organisatie, dus
 * het rekenwerk blijft beperkt tot wat op het scherm staat.
 */
const driftedIds = computed(() => {
  void dataVersion.value;
  const ids = new Set<string>();
  for (const c of cases.value) {
    const law = lawOf(c);
    if (!law) continue;
    const evaluation = demo.evaluate(law, c.parameters);
    if (demo.caseDrift(law, evaluation, c)) ids.add(c.id);
  }
  return ids;
});

function laneTag(c: DemoCase) {
  if (c.objection?.status === 'PENDING') return { color: 'warning', text: t('zaak.tag.objection') };
  if (statusOf(c) === 'DECIDED') return c.approved ? { color: 'success', text: t('zaak.tag.granted') } : { color: 'critical', text: t('zaak.tag.refused') };
  return { color: 'neutral', text: t('zaak.tag.in_review') };
}
/** Voor wie deze regeling is, afgeleid uit de wet zelf (RFC-038). */
function lawAudience(law: LawEntry) {
  if (isDelegationProvider(law.doc) || !producesBeschikking(law.doc)) return t('zaak.audience.provider');
  return subjectOf(law.doc) === 'BUSINESS' ? t('zaak.audience.business') : t('zaak.audience.citizen');
}
/** The text of a text field's `input` event, from the web component or a native field. */
function inputValue(e: Event): string {
  return (e as CustomEvent<{ value?: string } | null | undefined>).detail?.value ?? (e.target as EventTarget & { value: string }).value;
}
function onServiceChange(e: Event) {
  service.value = (e.target as HTMLSelectElement).value;
}
function claimLawName(cl: DemoClaim) {
  return corpus.value?.lawById(cl.lawId)?.name ?? cl.lawId;
}
</script>

<template>
  <nldd-navigation-split-view>
    <nldd-split-view-pane slot="main" has-content>
      <nldd-page landmarks="page" sticky-header>
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="t('zaak.title')" :supporting-text="serviceName(service) ?? ''">
            <nldd-dropdown slot="toolbar" size="sm" :accessible-label="t('zaak.organisation')">
              <select :value="service" @change="onServiceChange">
                <option v-for="s in services" :key="s" :value="s">{{ serviceName(s) }}</option>
              </select>
            </nldd-dropdown>
          </nldd-top-title-bar>
        </nldd-container>

        <nldd-simple-section width="full">
          <nldd-container layout="grid" column-count="3" sm-column-count="1" gap="16">
            <nldd-box v-for="lane in lanes" :key="lane.key" background="tinted">
              <nldd-container padding="12" gap="8">
                <nldd-container layout="row" gap="8" vertical-alignment="center" padding-inline="4">
                  <nldd-title-cell size="6" :text="lane.title" heading-level="2"></nldd-title-cell>
                  <nldd-badge color="neutral" :number="lane.items.length" :accessible-label="t('zaak.lane.count', { n: lane.items.length })"></nldd-badge>
                </nldd-container>
                <nldd-container v-if="lane.items.length === 0" padding-inline="4" padding-block="8">
                  <nldd-text-cell size="sm" color="secondary" :text="t('zaak.lane.empty')"></nldd-text-cell>
                </nldd-container>
                <nldd-list v-for="c in lane.items" :key="c.id" appearance="box-base" :accessible-label="c.lawName">
                  <nldd-list-item size="md" button :selected="selected?.id === c.id || undefined" @click="open(c)">
                    <!-- The status tag sits in the overline, not in an end cell: a lane is narrow
                         and an end cell never shrinks, so beside the tag the title would break
                         per letter. In the overline the title keeps the whole card width. -->
                    <nldd-text-cell :text="c.lawName" :supporting-text="`${subjectName(c)} · ${formatDateTime(c.submittedAt)}`">
                      <nldd-tag slot="overline" size="sm" :color="laneTag(c).color" :text="laneTag(c).text"></nldd-tag>
                      <!-- Er is een gegeven gewijzigd waarmee deze wet nu op iets
                           anders uitkomt dan waarop besloten is. Het besluit staat
                           nog; dit zegt alleen dat ernaar gekeken moet worden. -->
                      <nldd-tag v-if="driftedIds.has(c.id)" slot="overline" size="sm" color="warning" icon="warning" :text="t('zaak.tag.changed')"></nldd-tag>
                    </nldd-text-cell>
                  </nldd-list-item>
                </nldd-list>
              </nldd-container>
            </nldd-box>
          </nldd-container>
        </nldd-simple-section>

        <nldd-simple-section width="full" padding-top="0">
          <nldd-container gap="4">
            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">{{ t('zaak.laws.heading', { service: serviceName(service) }) }}</nldd-text></nldd-container>
            <nldd-list appearance="box-tinted" :accessible-label="t('zaak.laws.label')">
              <nldd-list-item v-if="orgLaws.length === 0" size="sm"><nldd-text-cell size="sm" color="secondary" :text="t('zaak.laws.empty')"></nldd-text-cell></nldd-list-item>
              <nldd-list-item v-for="{ law, count } in orgLaws" :key="law.id" size="sm" button @click="goTo('wetten', { lawId: law.id })">
                <nldd-text-cell size="sm" :text="law.name" :supporting-text="lawAudience(law)"></nldd-text-cell>
                <nldd-cell><nldd-tag size="sm" :color="count ? 'accent' : 'neutral'" :text="t.plural(count, 'zaak.laws.cases')"></nldd-tag></nldd-cell>
                <nldd-icon-cell icon="chevron-right" size="16" color="secondary"></nldd-icon-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-container>
        </nldd-simple-section>

        <nldd-simple-section v-if="serviceClaims.length" width="full" padding-top="0">
          <nldd-container gap="4">
            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">{{ t('zaak.claims.heading') }}</nldd-text></nldd-container>
            <nldd-list appearance="box-tinted" :accessible-label="t('zaak.claims.label')">
              <CorrectionRows :claims="serviceClaims" :origin="(cl) => `${personaName(cl.bsn)} · ${claimLawName(cl)}`" />
            </nldd-list>
          </nldd-container>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>

  </nldd-navigation-split-view>

  <Teleport to="body">
    <nldd-sheet ref="caseSheet" placement="right" width="720px" :accessible-label="t('zaak.sheet.label')" @close="close">
      <nldd-page v-if="selected">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="selected.lawName" :supporting-text="t('zaak.sheet.subtitle', { id: selected.id.slice(-5), person: subjectName(selected) })" :dismiss-text="t('zaak.sheet.close')" @dismiss="close"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="16" gap="16">
          <nldd-banner
            :variant="selected.status === 'DECIDED' ? (selected.approved ? 'success' : 'critical') : 'accent'"
            :text="selected.status === 'DECIDED' ? t(selected.approved ? 'zaak.tag.granted' : 'zaak.tag.refused') : selected.objection?.status === 'PENDING' ? t('zaak.banner.objection') : t('zaak.banner.awaiting')"
            :supporting-text="caseReason(selected) ?? eventText(selected.events.at(-1))"
          ></nldd-banner>

          <nldd-container gap="4">

            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">{{ t('zaak.outcome') }}</nldd-text><nldd-text size="xs" color="secondary">{{ verified?.ok ? t('zaak.outcome.recomputed') : t('zaak.outcome.claimed') }}</nldd-text></nldd-container>

            <nldd-list appearance="box-tinted" :accessible-label="t('zaak.outcome')">
              <nldd-list-item v-for="row in outputRows(selected)" :key="row.name" size="sm">
                <nldd-text-cell size="sm" :text="humanize(row.name)"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :color="row.differs ? 'warning' : 'content'">
                  <template v-if="row.differs"><s>{{ formatValue(row.claimed, row.spec) }}</s> → {{ formatValue(row.now, row.spec) }}</template>
                  <template v-else>{{ formatValue(row.now, row.spec) }}</template>
                </nldd-text-cell>
              </nldd-list-item>
            </nldd-list>

          </nldd-container>
          <nldd-banner v-if="verified && !verified.ok" variant="warning" :text="t('zaak.recompute_failed')" :supporting-text="verified.error"></nldd-banner>
          <!-- De levensloop kon niet verder. Het besluit blijft staan zoals het
               was, maar wat de Awb eraan toevoegt (de termijn, de einddatum)
               ontbreekt dan, en dat hoort niet stil te blijven. -->
          <nldd-banner v-if="selected.lifecycleError" variant="warning" :text="t('zaak.lifecycle_failed')" :supporting-text="selected.lifecycleError"></nldd-banner>

          <nldd-container v-if="lineage.length" gap="4">
            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">{{ t('zaak.data') }}</nldd-text><nldd-text size="xs" color="secondary">{{ t('zaak.data.hint') }}</nldd-text></nldd-container>
            <nldd-list type="tree" appearance="box-tinted" :accessible-label="t('zaak.data')">
              <DataLineage :nodes="lineage" @edit="editing = $event" />
            </nldd-list>
          </nldd-container>

          <nldd-container v-if="caseClaims.length" padding-inline="12" padding-block="6"><nldd-text-cell size="sm" color="secondary" :text="t('zaak.corrections')"></nldd-text-cell></nldd-container>
          <nldd-list v-if="caseClaims.length" appearance="box-tinted" :accessible-label="t('zaak.corrections')">
            <CorrectionRows :claims="caseClaims" />
          </nldd-list>

          <template v-if="selected.status !== 'DECIDED' && !selected.objection">
            <nldd-form-field :label="t('zaak.motivation')">
              <nldd-multi-line-text-field :value="reason" rows="2" :placeholder="t('zaak.motivation.placeholder')" @input="reason = inputValue($event)"></nldd-multi-line-text-field>
            </nldd-form-field>
            <nldd-button-group orientation="horizontal">
              <nldd-button appearance="primary" start-icon="checked" :text="t('zaak.grant')" @click="decide(true)"></nldd-button>
              <nldd-button appearance="destructive" start-icon="dismiss" :text="t('zaak.refuse')" @click="decide(false)"></nldd-button>
            </nldd-button-group>
          </template>
          <template v-else-if="selected.objection?.status === 'PENDING'">
            <nldd-rich-text spacing="tight"><p><strong>{{ t('zaak.objection.prefix') }}</strong> {{ selected.objection.reason }}</p></nldd-rich-text>
            <nldd-form-field :label="t('zaak.motivation')">
              <nldd-multi-line-text-field :value="reason" rows="2" @input="reason = inputValue($event)"></nldd-multi-line-text-field>
            </nldd-form-field>
            <nldd-button-group orientation="horizontal">
              <nldd-button appearance="primary" :text="t('zaak.objection.uphold')" @click="decideObjection(true)"></nldd-button>
              <nldd-button appearance="secondary" :text="t('zaak.objection.dismiss')" @click="decideObjection(false)"></nldd-button>
            </nldd-button-group>
          </template>
          <!-- Besloten, maar nog niet verstuurd. De Awb maakt van het besluit
               (art. 1:3) en de bekendmaking ervan (art. 3:41) twee momenten, en
               pas het tweede laat de bezwaartermijn beginnen (art. 6:8). Dat is
               hier dus ook een eigen handeling, en geen bijzaak van toekennen. -->
          <template v-else-if="!selected.publishedAt">
            <nldd-banner
              variant="accent"
              :text="t('zaak.publish.title')"
              :supporting-text="t('zaak.publish.body')"
            ></nldd-banner>
            <nldd-button-group orientation="horizontal">
              <nldd-button appearance="primary" start-icon="paper-plane" :text="t('zaak.publish.action')" @click="publish"></nldd-button>
            </nldd-button-group>
          </template>
          <template v-else-if="!selected.objection">
            <!-- De termijn komt uit de wet: 6:7 geeft het aantal weken, 6:8 de
                 einddatum vanaf de bekendmaking. -->
            <nldd-list v-if="awb.bezwaartermijnEinde" appearance="box-tinted" :accessible-label="t('zaak.term.label')">
              <nldd-list-item size="sm">
                <nldd-text-cell size="sm" color="secondary" :text="t('zaak.term.until')"></nldd-text-cell>
                <nldd-text-cell size="sm" width="fit-content" horizontal-alignment="right" :text="formatValue(awb.bezwaartermijnEinde, null)"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>
            <nldd-rich-text spacing="tight"><p><small>{{ t('zaak.published.note') }}</small></p></nldd-rich-text>
          </template>

          <nldd-container gap="4">

            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">{{ t('zaak.events') }}</nldd-text></nldd-container>

            <nldd-list appearance="box-tinted" :accessible-label="t('zaak.events')">
              <nldd-list-item v-for="(ev, i) in selected.events" :key="i" size="sm">
                <nldd-step-cell :status="i === selected.events.length - 1 ? 'future' : 'past'" :position="selected.events.length === 1 ? 'only' : i === 0 ? 'first' : i === selected.events.length - 1 ? 'last' : 'between'"></nldd-step-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-text-cell size="sm" :text="eventText(ev)" :supporting-text="formatDateTime(ev.at)"></nldd-text-cell>
              </nldd-list-item>
            </nldd-list>

          </nldd-container>
        </nldd-container>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
  <!-- The same correction sheet as on the portal, filled in by the caseworker for this case. -->
  <EditValueSheet
    v-if="selected"
    :open="!!editing"
    :node="editing"
    :tile-law-id="selected.lawId"
    claimant="BEHANDELAAR"
    :bsn="selected.bsn"
    :case-id="selected.id"
    @close="editing = null"
  />
</template>
