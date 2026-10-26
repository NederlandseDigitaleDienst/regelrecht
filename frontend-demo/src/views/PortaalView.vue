<script setup lang="ts">
import { computed, reactive, ref, shallowReactive } from 'vue';
import LawTile from '../components/LawTile.vue';
import EditValueSheet from '../components/EditValueSheet.vue';
import ApplicationSheet from '../components/ApplicationSheet.vue';
import ChangeWizardSheet from '../components/ChangeWizardSheet.vue';
import { fieldSpec, numericImpact } from '../data/format';
import { loadFailures } from '../engine/useDemoEngine';
import { delegationLabel, permissionLabel } from '../data/delegation';
import { useDemo } from '../store/demoStore';
import { useI18n } from '../i18n/index.js';
import { useNarrow } from '../useNarrow';
import type { LawEntry } from '../data/loadCorpus';
import type { EvaluateResult } from '../engine/useDemoEngine';
import type { LineageNode } from '../data/lineage';

// The citizen's (or entrepreneur's) portal: every regeling the persona can
// discover, evaluated live, ordered by financial impact. This is where the
// audience sees the law do its work for one person, corrects a value and
// submits an application.

const demo = useDemo();
const { t } = useI18n();
// Op een smal scherm blijft alleen de kop staan; zie de toelichting in de
// template bij nldd-title.
const narrow = useNarrow();
const { profile, persona, portalLaws, corpus, state, activeDelegation, canSubmitClaims, features } = demo;

// Impact per law (from the tiles' evaluations) drives the ordering.
const impact = reactive<Record<string, number>>({});
/** Latest evaluation per law, so the application sheet follows the tile. */
const evaluations = shallowReactive<Record<string, EvaluateResult | null>>({});
function onEvaluated({ law, evaluation }: { law: LawEntry; evaluation: EvaluateResult | null }) {
  evaluations[law.id] = evaluation;
  if (!evaluation?.ok) {
    impact[law.id] = -1;
    return;
  }
  const cfg = corpus.value?.config?.dashboard_outputs ?? {};
  const name = cfg[`${law.service}/${law.law_path}`];
  const outputs = evaluation.outputs ?? {};
  let value = 0;
  if (name && typeof outputs[name] === 'number') value = Math.abs(numericImpact(outputs[name], fieldSpec(law.doc, name)));
  else {
    for (const [k, v] of Object.entries(outputs)) {
      if (typeof v === 'number') value = Math.max(value, Math.abs(numericImpact(v, fieldSpec(law.doc, k))));
    }
  }
  if (outputs.voldoet_aan_voorwaarden === false) value = -0.5;
  impact[law.id] = value;
}

const sortedLaws = computed(() =>
  [...portalLaws.value].sort((a, b) => (impact[b.id] ?? 0) - (impact[a.id] ?? 0) || a.name.localeCompare(b.name)),
);

// Wijziging doorgeven staat achter een vlag per profiel (de POC's
// FEATURE_CHANGE_WIZARD) en alleen voor wie zelf mag corrigeren: namens een
// ander met alleen leesrecht valt er niets door te geven. Ook alleen voor een
// burger — de wizard gaat over inkomen, huur, adres en huishouden.
const wizardOpen = ref(false);
const showWizard = computed(
  () => features.value.CHANGE_WIZARD && canSubmitClaims.value && activeDelegation.value?.subjectType !== 'BUSINESS' && profile.value?.type !== 'ondernemer',
);

/** What LawTile and ApplicationSheet emit with `edit-value`. */
interface EditRequest {
  node: LineageNode;
  law: LawEntry;
  selfDeclared?: boolean;
}
const editing = ref<EditRequest | null>(null); // { node, law }
function onEditValue(payload: EditRequest) {
  editing.value = payload;
}
const applying = ref<LawEntry | null>(null); // the law being applied for
function onApply({ law }: { law: LawEntry }) {
  applying.value = law;
}

// De banner gaat over wat er op déze pagina wacht. Namens een onderneming is
// dat wat op haar KvK-nummer staat: `subjectBsn()` is dan de gemachtigde zelf,
// en zonder dit onderscheid verschenen zijn eigen privécorrecties boven de
// regelingen van het bedrijf.
const pendingClaims = computed(() => {
  const key = activeDelegation.value?.subjectType === 'BUSINESS' ? activeDelegation.value.subjectId : null;
  return state.claims.filter(
    (c) => c.status === 'PENDING' && (key ? c.keyValue === key : c.bsn === demo.subjectBsn()),
  );
});

// Namens een ander gaat de pagina over die ander: de vraag "waar heb ik recht
// op" is dan niet de goede vraag, en de persoonsbeschrijving van de
// gemachtigde hoort er evenmin te staan.
const heading = computed(() => {
  const d = activeDelegation.value;
  if (!d) return profile.value?.portal_heading;
  return t(d.subjectType === 'BUSINESS' ? 'zaak.portaal.heading.business' : 'zaak.portaal.heading.citizen', { name: d.subjectName });
});
const subtitle = computed(() => {
  const d = activeDelegation.value;
  if (!d) return profile.value?.portal_subtitle;
  return t(d.subjectType === 'BUSINESS' ? 'zaak.portaal.subtitle.business' : 'zaak.portaal.subtitle.citizen');
});

// Namens wie er gehandeld wordt. De machtiging komt uit een wet, en die wet
// staat erbij: waaróm iemand dit mag zien is onderdeel van het antwoord.
const actingText = computed(() => {
  const d = activeDelegation.value;
  if (!d) return null;
  const kind = delegationLabel(d);
  return t('zaak.portaal.acting.text', { name: d.subjectName, kind: kind ? ` (${kind})` : '' });
});
const actingSupport = computed(() => {
  const d = activeDelegation.value;
  if (!d) return null;
  const rights = d.permissions.map((p) => permissionLabel(p)).join(', ').toLowerCase();
  const source = d.lawName ? t('zaak.portaal.acting.source', { law: d.lawName }) : '';
  return t('zaak.portaal.acting.support', { name: d.subjectName, rights, source });
});

// A law the engine refused to load (a type-check finding, RFC-037) is missing
// from every tile that depends on it. The refusal is shown here, not buried in
// the console, with the engine's own message per law.
const loadFailureText = computed(() => loadFailures.value.map((f) => `${f.id} (${f.path}): ${f.message}`).join(' — '));
</script>

<template>
  <nldd-page>
    <!-- No title bar above the heading: it repeated the portal's name right on
         top of the page heading that follows, and the tab in the app bar already
         names it. What the bar carried that nothing else did is the disclaimer,
         so that moves into the overline, where it stays next to the persona the
         visitor is logged in as. -->
    <!-- 1440px: op 1200 bleef er van drie kolommen 384px per tegel over, en dat
         is krap voor een bedrag met een zin eromheen. Breder geeft dezelfde drie
         kolommen meer ruimte in plaats van een vierde erbij. De kop loopt mee, zodat
         de tekst boven de tegels op dezelfde marge staat. -->
    <nldd-simple-section width="1440px">
      <!-- De kop draagt alleen de vraag en haar toelichting. Wie er is
           ingelogd stond eerder in de overline, bóven de kop, en dat zette een
           voetnoot op de plek van het onderwerp: de pagina gaat over waar deze
           burger recht op heeft, niet over wie er is ingelogd. De identiteit
           staat daarom onder de kop, waar zij de vraag beantwoordt in plaats
           van hem aan te kondigen. -->
      <nldd-title slot="header" size="2">
        <h1>{{ heading }}</h1>
        <span v-if="!narrow" slot="supporting-text">{{ subtitle }}</span>
      </nldd-title>
      <!-- Naam, voorbehoud en beschrijving horen bij elkaar: het zijn drie
           dingen over dezelfde persoon. Ze staan in één blok onder de kop, met
           de naam als eerste regel en de beschrijving eronder.

           Het blok zit in de header-slot, naast nldd-title, en niet in de body
           van de sectie. nldd-simple-section zet 32px tussen zijn header en
           zijn inhoud, en die afstand is niet in te stellen: in de body kwam de
           identiteit daardoor even ver van de kop te staan als de tegels, terwijl
           zij bij de kop hoort. In de header is die 32px juist de scheiding naar
           de tegels (gemeten: 80px tot de eerste tegel). De 12px erboven is het
           ritme dat nldd-title intern ook aanhoudt; zonder padding sloot de
           naam strak op de subtitel aan (gemeten: 0px). Dit kost geen eigen CSS
           en geen negatieve marge.

           Op een smal scherm blijft alleen de kop staan: vijf lagen tekst
           vulden daar het scherm voordat de eerste tegel in beeld kwam, en wie
           is ingelogd staat ook in de werkbalk. Namens wie er gehandeld wordt
           blijft wél staan, want dat verandert de betekenis van alles eronder.

           De persona-tags (Ouderlijk gezag, Ondernemer, KVK-nummer) stonden
           hier als derde kanaal naast de beschrijving en zijn weg: "alleenstaande
           ouder met twee jonge kinderen" zegt al wat "Ouderlijk gezag" zegt, en
           een tag die niets filtert en nergens heen leidt is een label om het
           label. -->
      <nldd-container v-if="!narrow || activeDelegation" slot="header" padding-top="12">
        <nldd-rich-text spacing="tight">
          <p>
            <!-- `?.` for the type checker only: narrow, this container renders
                 only with an activeDelegation (its v-if). -->
            <template v-if="narrow">{{ t('zaak.portaal.overline.acting_narrow', { name: activeDelegation?.subjectName }) }}</template>
            <template v-else>{{ t('zaak.portaal.signed_in.lead') }} <strong>{{ persona?.name ?? profile?.name }}</strong><template v-if="activeDelegation"> · {{ t('zaak.portaal.overline.acting', { name: activeDelegation.subjectName }) }}</template> · {{ t('zaak.portaal.overline.disclaimer') }}</template>
          </p>
          <!-- Namens een ander zegt de beschrijving van de gemachtigde niets. -->
          <p v-if="!narrow && persona?.description && !activeDelegation">{{ persona.description }}</p>
        </nldd-rich-text>
      </nldd-container>
      <!-- Eén ingang voor 'er is iets veranderd', naast de tegels die elk over
           één regeling gaan. Onder de kop en niet ernaast: het is een actie op
           de hele pagina, geen eigenschap van de persoon. -->
      <nldd-container v-if="showWizard" padding-top="8">
        <nldd-button size="sm" appearance="secondary" start-icon="edit" :text="t('zaak.portaal.change_wizard')" @click="wizardOpen = true"></nldd-button>
      </nldd-container>
      <!-- De banners krijgen hun eigen container met een marge, zodat ze los
           staan van wat erboven eindigt. -->
      <nldd-container v-if="loadFailures.length || pendingClaims.length || activeDelegation" padding-top="16" gap="12">
      <!-- Namens een ander handelen is niet hetzelfde als zelf inloggen; dat
           hoort in beeld te blijven zolang het duurt, met de wet erbij. -->
      <nldd-banner
        v-if="activeDelegation"
        variant="accent"
        :icon="activeDelegation.subjectType === 'BUSINESS' ? 'building' : 'person'"
        :text="actingText"
        :supporting-text="actingSupport"
      ></nldd-banner>
      <nldd-banner
        v-if="activeDelegation && !canSubmitClaims"
        variant="warning"
        :text="t('zaak.portaal.read_only.title')"
        :supporting-text="t('zaak.portaal.read_only.body')"
      ></nldd-banner>
      <nldd-banner
        v-if="loadFailures.length"
        variant="critical"
        :text="t.plural(loadFailures.length, 'zaak.portaal.load_failure')"
        :supporting-text="t('zaak.portaal.load_failure.body', { details: loadFailureText })"
      ></nldd-banner>
      <nldd-banner
        v-if="pendingClaims.length"
        variant="accent"
        :text="t.plural(pendingClaims.length, 'zaak.portaal.pending_claims')"
        :supporting-text="t('zaak.portaal.pending_claims.body')"
      ></nldd-banner>
      </nldd-container>
    </nldd-simple-section>

    <nldd-simple-section width="1440px" padding-top="0">
      <!-- `item-width` is een minimum, geen breedte: de collectie verdeelt haar
           1200px over zoveel kolommen als er passen en rekt de rest uit. Op 400px
           bleven er dus twee over van 588px elk. 360px geeft er drie, en drie
           tegels naast elkaar laat het oog rustig scannen. Ruimte hoort binnen de
           tegel (padding 20, gap 16), niet in de doos. -->
      <nldd-collection layout="grid" item-width="360px" max-items="60">
        <LawTile v-for="law in sortedLaws" :key="law.id" :law="law" @edit-value="onEditValue" @evaluated="onEvaluated" @apply="onApply" />
      </nldd-collection>
      <nldd-inline-dialog v-if="sortedLaws.length === 0" icon="inbox" :text="t('zaak.portaal.empty.title')" :supporting-text="t('zaak.portaal.empty.body')"></nldd-inline-dialog>
    </nldd-simple-section>

    <EditValueSheet :open="!!editing" :node="editing?.node ?? null" :tile-law-id="editing?.law?.id ?? null" :self-declared="!!editing?.selfDeclared" @close="editing = null" />
    <ApplicationSheet :open="!!applying" :law="applying" :evaluation="applying ? evaluations[applying.id] ?? null : null" @close="applying = null" @edit-value="onEditValue" />
    <ChangeWizardSheet :open="wizardOpen" @close="wizardOpen = false" />
  </nldd-page>
</template>
