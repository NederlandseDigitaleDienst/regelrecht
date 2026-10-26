<script setup lang="ts">
import { computed, defineAsyncComponent, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import type { NLDDModalDialog } from '@nldd/design-system/modal-dialog';
import { useColorScheme } from '@regelrecht/frontend-shared';
import { FEATURES, useDemo } from './store/demoStore';
import { delegationLabel } from './data/delegation';
import { LOCALES, useI18n } from './i18n/index.js';
import { localeRouteName } from './router';
import PresentationDeck from './presentation/PresentationDeck.vue';
import { usePresentation } from './presentation/usePresentation';
import { replay } from './walkthrough/replay';
import { viewEpoch } from './walkthrough/viewEpoch';
import ReplayAside from './walkthrough/ReplayAside.vue';
import ReplayControls from './walkthrough/ReplayControls.vue';
import ReplayOverlay from './walkthrough/ReplayOverlay.vue';
import { lockWhy, probeWhy, unlockWhy, whyAvailable, whyUnlocked } from './why/why';

// The workspace shell: one bar with the tab bar and the presenter menu, and
// the active tab below it. Every tab is a route; <keep-alive> keeps the tabs
// the presenter already visited mounted, so opened law tabs, a running
// scenario or an expanded tile survive switching back and forth.

const route = useRoute();
const router = useRouter();
const demo = useDemo();
const { t, locale, setLocale } = useI18n();
const { ready, loadError, profile, profileKey, corpus, state, delegations, delegationEnabled, activeDelegation, features } = demo;

/**
 * The path of a tab in the language that is on.
 *
 * Without params: a tab points at the root of its section, and carrying the
 * current route's params along would put `/regelwerken/:lawId` on the Graph tab.
 * The views restore their own last position when they mount.
 */
function pathFor(page: string) {
  return router.resolve({ name: localeRouteName(page, locale.value) }).path;
}

/** Heeft de presentator een vlag omgezet? Dan kan hij terug naar het profiel. */
const hasFeatureOverrides = computed(() => Object.keys(state.featureOverrides ?? {}).length > 0);

// The design system derives its scroll mode (document vs. per-pane) from the
// outermost split view once, at connect. Ours arrives later (the tab views are
// lazy routes), so the shell settles on "document scrolls" and the pane
// headers stop sticking. Re-deriving after every route change puts it right;
// it is a design-system timing gap, not something the demo should own.
// `_evaluateScrollMode` and `_derivedMode` are private on NLDDShell, so the
// ref is typed by just the two members read here.
const appView = ref<{ _evaluateScrollMode?: () => void; _derivedMode?: string } | null>(null);
function refreshScrollMode(attempt = 0) {
  nextTick(() => {
    const view = appView.value;
    view?._evaluateScrollMode?.();
    // The split view measures itself a frame or two after it upgrades; retry
    // until the derived mode is in, then stop.
    if (view && view._derivedMode !== 'nested' && attempt < 6) setTimeout(() => refreshScrollMode(attempt + 1), 100 * (attempt + 1));
  });
}
onMounted(() => {
  demo.boot().catch(() => {});
  probeWhy();
  refreshScrollMode();
});
// Not `afterEach(refreshScrollMode)`: the hook passes the route, which would
// land in `attempt` and stop the retries after a tab switch.
router.afterEach(() => refreshScrollMode());

// The presentation deck drives the tabs; it needs the router, the store (to
// switch persona) and the slides from the demo config once that has loaded.
const presentation = usePresentation();

// The walkthrough recorder, in dev and only with `?record` in the address. A
// literal `import.meta.env.DEV` so the production build drops the import
// altogether: the deployed demo has no microphone or camera permission anyway.
const RecorderPanel =
  import.meta.env.DEV && typeof window !== 'undefined' && new URLSearchParams(window.location.search).has('record')
    ? defineAsyncComponent(() => import('./walkthrough/RecorderPanel.vue'))
    : null;
presentation.init({ router, demo });
// Not while the recorded walkthrough runs: it shows the deck as it was
// recorded, and a language switch by the viewer must not swap it out.
watch(corpus, (c) => replay.active || presentation.init({ slides: c?.config?.slides ?? [] }), { immediate: true });
// De modus staat in de store (en dus in localStorage); het dek houdt er zijn
// eigen ref voor, zodat de store niet om de presentatiemodule heen cirkelt.
watch(() => state.presentationMode, (m) => presentation.setMode(m), { immediate: true });
function onGlobalKey(e: KeyboardEvent) {
  // While the recorded walkthrough runs it drives the deck; Shift+P would
  // pull the deck out from under it.
  if (replay.active) return;
  if (e.key === 'P' && e.shiftKey && !(e.target as Element | null)?.closest?.('input, textarea, select, [contenteditable]')) {
    e.preventDefault();
    // Een schakelaar, ook als het dek uit beeld staat. `start()` navigeert naar
    // de dia, en dat is precies wat je níet wilt van iemand die zelf naar een
    // ander tabblad is gelopen: die wil zijn toetsenbord terug, niet ergens
    // anders heen gesleept worden. Uitzetten kan altijd, hier en met Escape.
    if (presentation.active.value) presentation.stop();
    else presentation.start(presentation.index.value);
  }
}
onMounted(() => window.addEventListener('keydown', onGlobalKey));
onUnmounted(() => window.removeEventListener('keydown', onGlobalKey));

const tabs = computed(() => [
  { name: 'home', text: t('app.tabs.home'), icon: 'home', to: pathFor('home'), iconOnly: true },
  { name: 'presentatie', text: t('app.tabs.presentatie'), icon: 'display', to: pathFor('presentatie') },
  { name: 'wetten', text: t('app.tabs.wetten'), icon: 'books', to: pathFor('wetten') },
  { name: 'graaf', text: t('app.tabs.graaf'), icon: 'centralized-network', to: pathFor('graaf') },
  { name: 'scenarios', text: t('app.tabs.scenarios'), icon: 'checklist', to: pathFor('scenarios') },
  { name: 'simulatie', text: t('app.tabs.simulatie'), icon: 'chart-x-y-axis-line', to: pathFor('simulatie') },
  // Namens een onderneming heet het tabblad naar die onderneming: 'Mijn
  // overheid' gaat over de ingelogde burger, en dat is dan niet het onderwerp.
  //
  // `portal_tab_label` komt uit demo-config.yaml, en die configuratie wordt per
  // taal geleverd: de store kiest tussen de Nederlandse en de Engelse versie.
  // Het tabblad leest hem dus gewoon, in welke taal hij ook binnenkomt. De
  // sleutel is de terugval voor een profiel dat er geen heeft.
  {
    name: 'portaal',
    text:
      activeDelegation.value?.subjectType === 'BUSINESS'
        ? activeDelegation.value.subjectName
        : profile.value?.portal_tab_label || t('app.tabs.portaal'),
    icon: activeDelegation.value?.subjectType === 'BUSINESS' ? 'building' : 'person',
    to: pathFor('portaal'),
  },
  { name: 'zaaksysteem', text: t('app.tabs.zaaksysteem'), icon: 'inbox', to: pathFor('zaaksysteem') },
]);

function isActive(tab: { name: string }) {
  // On the page, not on the route name: the same page has one route name per
  // language (`wetten` and `wetten:en`), and `meta.page` is what the two share.
  return (route.meta?.page ?? route.name) === tab.name;
}

// De tabbalk krimpt met het venster mee in plaats van tabbladen weg te laten
// vallen. Gemeten met zeven tabbladen: icoon met tekst 878px, alleen tekst
// 696px, alleen icoon 314px. Bij de drempels zit ruimte voor de knoppen rechts
// (namens wie, profiel) en de overloopknop.
const viewportWidth = ref(typeof window === 'undefined' ? 1600 : window.innerWidth);
function onResize() {
  viewportWidth.value = window.innerWidth;
}
onMounted(() => window.addEventListener('resize', onResize));
onUnmounted(() => window.removeEventListener('resize', onResize));

/**
 * De tabbladen in het vangnet-menu hebben een `href` zodat ze als echte link
 * renderen: dat zet `aria-current="page"` op het actieve tabblad en houdt
 * middenklik heel. Het menu-item roept er alleen geen preventDefault op, dus
 * de browser volgt die href en herlaadt de hele pagina naast de
 * router-navigatie (gemeten: vier framenavigaties).
 *
 * De luisteraar moet op het menu van de toolbar zelf: dat menu bevat klonen
 * van onze items en is naar document.body verplaatst, dus een handler op onze
 * eigen nldd-menu-group in de template ziet die klik nooit. Capture-fase, zodat
 * we er vóór het menu-item bij zijn. Een klik met een modifier laten we staan,
 * want dat is iemand die bewust een nieuw tabblad of venster wil.
 */
function onOverflowMenuClick(event: MouseEvent) {
  if (event.defaultPrevented) return;
  if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || event.button !== 0) return;
  const item = event.composedPath?.().find((n) => (n as Element | null)?.tagName?.toLowerCase?.() === 'nldd-menu-item') as Element | undefined;
  const to = item?.getAttribute?.('href');
  // `tab`, niet `t`: `t` is hier de vertaalfunctie, en die naam nog eens als
  // lusvariabele gebruiken werkt wel maar leest als een fout.
  if (!to || !tabs.value.some((tab) => tab.to === to)) return;
  event.preventDefault();
  router.push(to);
}

// Het menu bestaat pas nadat de toolbar het heeft aangemaakt, en het verhuist
// naar document.body. Daarom luisteren we op document en filteren we op de
// href van een eigen tabblad.
onMounted(() => document.addEventListener('click', onOverflowMenuClick, true));
onUnmounted(() => document.removeEventListener('click', onOverflowMenuClick, true));

const tabVariant = computed(() => {
  if (viewportWidth.value >= 1240) return 'icon-and-text';
  if (viewportWidth.value >= 1040) return 'text';
  return 'icon';
});

const profileOptions = computed(() => Object.entries(corpus.value?.config?.profiles ?? {}));

function onProfileSelect(e: Event) {
  const value = (e.target as Element | null)?.getAttribute?.('value');
  if (value) demo.setProfile(value);
}

// ---- machtigingen ----------------------------------------------------------
// Namens wie er gehandeld wordt. De lijst komt uit de wet (elke wet die het
// machtigingscontract vervult), niet uit de app.

/** Toon de keuze pas als er echt iets te kiezen valt. */
const showDelegation = computed(() => delegationEnabled.value && delegations.value.length > 1);

/** Wat er in de knop staat: 'Mezelf' of degene namens wie gehandeld wordt. */
const delegationButtonText = computed(() => activeDelegation.value?.subjectName ?? t('app.delegation.self'));

const DELEGATION_ICONS: Record<string, string> = { SELF: 'person', CITIZEN: 'person', BUSINESS: 'building' };

function onDelegationSelect(e: Event) {
  const value = (e.target as Element | null)?.getAttribute?.('value');
  if (!value) return;
  demo.setDelegation(delegations.value.find((d) => `${d.subjectType}:${d.subjectId}` === value) ?? null);
}

const { colorScheme, setColorScheme } = useColorScheme();
const colorSchemeOptions = computed(() => [
  ['auto', t('app.appearance.auto'), 'display'],
  ['light', t('app.appearance.light'), 'light-mode'],
  ['dark', t('app.appearance.dark'), 'dark-mode'],
]);

// ---- taal ------------------------------------------------------------------
// De taal staat in het menu, naast Weergave, en niet als eigen knop in de
// werkbalk. Het is dezelfde soort instelling als licht/donker: je zet hem één
// keer goed en raakt hem daarna niet meer aan, waar 'namens wie' en het profiel
// de knoppen zijn waarmee tijdens een demo het verhaal wordt verteld.
//
// Het label komt uit de talentabel en niet uit het woordenboek: elke taal noemt
// zichzelf in de eigen taal ('Nederlands', 'English'), in wélke taal de demo ook
// staat. Als sleutel zou hij in elk woordenboek dezelfde waarde hebben, en dat
// is precies wat de pariteitstest als onvertaald aanmerkt.
const languageOptions = computed(() => LOCALES.map((l): [string, string] => [l.code, l.label]));

function switchLocale(next: string) {
  if (!next || next === locale.value) return;
  // Every route of this app is named with a string (router.ts), never a symbol.
  const page = (route.meta?.page ?? route.name) as string | undefined;
  // Het staartstuk van het huidige pad (`/zorgtoeslagwet`, of een heel
  // bestandspad op het scenario-tabblad) wordt letterlijk overgenomen, vóór
  // `setLocale` het pad onder ons verandert.
  //
  // Letterlijk, en niet via de params: het scenario-tabblad vangt de rest van
  // het pad op (`:featurePath(.*)`), en `resolve` percent-codeert de schuine
  // strepen daarin tot `%2F`. Het bestand laadt dan nog wel — vue-router
  // decodeert het weer — maar het adres blijft onleesbaar in de balk staan, en
  // dat is precies het adres dat tijdens een presentatie op het scherm komt of
  // gedeeld wordt.
  const here = page ? router.resolve({ name: localeRouteName(page, locale.value) }).path : '';
  const tail = here && route.path.startsWith(here) ? route.path.slice(here.length) : '';
  setLocale(next);
  // `replace`, niet `push`: een taal is geen plek om naar terug te keren, en de
  // terugknop hoort niet tussen twee talen heen en weer te springen.
  //
  // De routenaam verschilt per taal (`wetten` en `wetten:en`), maar beide
  // wijzen naar hetzelfde component, en daarop keyt keep-alive. De gemounte
  // view wordt dus hergebruikt en een open wettab of een lopend scenario
  // overleeft de wissel; `query` en `hash` gaan mee, zodat een gedeelde
  // deeplink niet halverwege iets kwijtraakt.
  if (!page) return;
  router.replace({
    path: `${router.resolve({ name: localeRouteName(page, next) }).path}${tail}`,
    query: route.query,
    hash: route.hash,
  });
}

function toggleFullscreen() {
  if (document.fullscreenElement) document.exitFullscreen?.();
  else document.documentElement.requestFullscreen?.();
}

// Unlocking the "why" explanation (why/why.ts). The password is checked by the
// server; the dialog only passes it on and says whether it was accepted.
const whyDialog = ref<NLDDModalDialog | null>(null);
const whyInput = ref('');
const whyStatus = ref<'wrong' | 'unavailable' | null>(null); // null | 'wrong' | 'unavailable'
const whyBusy = ref(false);
function askWhyPassword() {
  whyInput.value = '';
  whyStatus.value = null;
  whyDialog.value?.show?.();
}
async function confirmWhyPassword() {
  if (!whyInput.value || whyBusy.value) return;
  whyBusy.value = true;
  try {
    const result = await unlockWhy(whyInput.value);
    if (result === 'ok') whyDialog.value?.hide?.();
    else whyStatus.value = result;
  } finally {
    whyBusy.value = false;
  }
}

const resetDialog = ref<NLDDModalDialog | null>(null);
function askReset() {
  resetDialog.value?.show?.();
}
function confirmReset() {
  resetDialog.value?.hide?.();
  demo.resetState();
  router.push(pathFor('home'));
}

function onWhyInput(e: Event) {
  whyInput.value = (e as CustomEvent<{ value?: string } | null | undefined>).detail?.value ?? (e.target as EventTarget & { value: string }).value;
  whyStatus.value = null;
}

function toggleManualReview() {
  state.manualReview = !state.manualReview;
}
function toggleAutoAnnounce() {
  state.autoAnnounce = !state.autoAnnounce;
}

const openCases = computed(() => state.cases.filter((c) => c.status === 'IN_REVIEW').length);
</script>

<template>
  <nldd-shell ref="appView" background="tinted">
    <!-- While the recorded walkthrough plays, the deck carries its controls
         and, under the slide, the presenter's bubble and the questions. -->
    <PresentationDeck>
      <template v-if="replay.active" #aside><ReplayAside /></template>
      <template v-if="replay.active" #footer><ReplayControls /></template>
    </PresentationDeck>
    <ReplayOverlay v-if="replay.active" />
    <component :is="RecorderPanel" v-if="RecorderPanel" />
    <nldd-bar-split-view>
      <nldd-container slot="toolbar" padding="8" background="base">
        <nldd-toolbar size="md" :accessible-label="t('app.toolbar.label')">
          <!-- Eén tab-bar met alle tabbladen, niet één per tabblad. Een tab-bar
               per tabblad leek de overloop netjes op te lossen, maar elke bar
               rendert zijn eigen `<nav>`-landmark en regelt pijltjesnavigatie
               binnen zijn eigen items: zeven bars gaven zeven gelijknamige
               landmarks, zeven tabstops achter elkaar, en pijltjes die nergens
               meer heen gingen.
               De balk blijft heel en wordt smal via `variant`: alleen iconen
               meet 314px tegen 878px met tekst, dus hij past tot ruim onder
               400px. De tekst blijft de toegankelijke naam van elk item. -->
          <!-- Hoogste priority: de navigatie is het laatste wat mag wijken.
               Namens wie (20) en het profiel (30) gaan eerst het menu in, en
               die hebben daar allebei een eigen menu-variant voor. -->
          <nldd-toolbar-item slot="start" :priority="90">
            <nldd-tab-bar
              size="md"
              navigation
              :accessible-label="t('app.tabs.label')"
              :variant="tabVariant"
            >
              <!-- Home draagt alleen zijn huisje: het icoon is eenduidig genoeg
                   en het achtste tabblad kostte de balk anders de breedte die
                   de tekstvariant net tot 1240px overeind hield. Een item mag
                   zijn eigen `variant` zetten; de tab-bar leest die bij het
                   koppelen en laat hem voorgaan op die van de balk. `text`
                   blijft staan en is dan de toegankelijke naam. -->
              <nldd-tab-bar-item
                v-for="tab in tabs"
                :key="tab.name"
                :text="tab.text"
                :variant="tab.iconOnly ? 'icon' : undefined"
                :href="tab.to"
                :current="isActive(tab) || undefined"
                @click.prevent="router.push(tab.to)"
              >
                <nldd-icon slot="icon" :icon="tab.icon"></nldd-icon>
              </nldd-tab-bar-item>
            </nldd-tab-bar>
            <!-- Vangnet: past zelfs de iconenbalk niet meer, dan verbergt de
                 toolbar dit item en komen de tabbladen hier terug. Zonder dit
                 was de navigatie onder 500px weg, dezelfde fout als eerst. -->
            <nldd-menu-group slot="overflow" :text="t('app.tabs.goto')">
              <!-- Met `href` rendert het item als een echte link en zet het
                   `aria-current="page"` op het actieve tabblad. Zonder href
                   doet `selected` hier niets: het vinkje hoort bij checkbox
                   en radio, en aria-current komt alleen op de link-variant.
                   Middenklik en 'openen in nieuw tabblad' werken zo ook. -->
              <nldd-menu-item
                v-for="tab in tabs"
                :key="tab.name"
                :text="tab.text"
                :icon="tab.icon"
                :href="tab.to"
                :selected="isActive(tab) || undefined"
              ></nldd-menu-item>
            </nldd-menu-group>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end" v-if="openCases > 0">
            <nldd-button size="sm" appearance="neutral-tinted" start-icon="inbox" :text="t.plural(openCases, 'app.cases.pending')" @click="router.push(pathFor('zaaksysteem'))"></nldd-button>
          </nldd-toolbar-item>
          <!-- Namens wie: alleen als de wet meer dan één mogelijkheid geeft.
               Staat naast het profiel, want het hoort bij wie er ingelogd is. -->
          <nldd-toolbar-item slot="end" v-if="showDelegation" class="rr-hide-presenting" :priority="20">
            <nldd-button
              size="md"
              :appearance="activeDelegation ? 'accent-tinted' : 'neutral-transparent'"
              :start-icon="activeDelegation ? DELEGATION_ICONS[activeDelegation.subjectType] : 'switch'"
              :text="delegationButtonText"
              expandable
              popup-type="menu"
            >
              <nldd-menu slot="popup" :accessible-label="t('app.delegation.label')" @select="onDelegationSelect">
                <nldd-menu-item
                  v-for="d in delegations"
                  :key="`${d.subjectType}:${d.subjectId}`"
                  type="radio"
                  :value="`${d.subjectType}:${d.subjectId}`"
                  :text="d.subjectName"
                  :details="delegationLabel(d)"
                  :icon="DELEGATION_ICONS[d.subjectType] ?? 'person'"
                  :selected="(activeDelegation ? `${activeDelegation.subjectType}:${activeDelegation.subjectId}` : `SELF:${profile?.bsn}`) === `${d.subjectType}:${d.subjectId}` || undefined"
                ></nldd-menu-item>
              </nldd-menu>
            </nldd-button>
            <!-- Dezelfde keuze als menu, voor als de knop niet meer past.
                 `@select` per item: het overloopmenu toont een kloon en de
                 toolbar dispatcht `select` op het originele item, dus een
                 handler op de groep eromheen vuurt nooit. -->
            <nldd-menu-group slot="overflow" :text="t('app.delegation.label')">
              <nldd-menu-item
                v-for="d in delegations"
                :key="`${d.subjectType}:${d.subjectId}`"
                type="radio"
                :value="`${d.subjectType}:${d.subjectId}`"
                :text="d.subjectName"
                :details="delegationLabel(d)"
                :icon="DELEGATION_ICONS[d.subjectType] ?? 'person'"
                :selected="(activeDelegation ? `${activeDelegation.subjectType}:${activeDelegation.subjectId}` : `SELF:${profile?.bsn}`) === `${d.subjectType}:${d.subjectId}` || undefined"
                @select="demo.setDelegation(d)"
              ></nldd-menu-item>
            </nldd-menu-group>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end" v-if="profile" class="rr-hide-presenting" :priority="30">
            <nldd-button size="md" appearance="neutral-transparent" start-icon="person" :text="profile.name" expandable popup-type="menu">
              <nldd-menu slot="popup" :accessible-label="t('app.profile.label')" @select="onProfileSelect">
                <nldd-menu-item
                  v-for="[key, p] in profileOptions"
                  :key="key"
                  type="radio"
                  :value="key"
                  :text="p.name"
                  :details="p.type"
                  :selected="profileKey === key || undefined"
                ></nldd-menu-item>
              </nldd-menu>
            </nldd-button>
            <!-- Idem voor het personage: zonder deze variant verdween de
                 profielkiezer onder 1130px zonder vervanging. -->
            <nldd-menu-group slot="overflow" :text="t('app.profile.label')">
              <nldd-menu-item
                v-for="[key, p] in profileOptions"
                :key="key"
                type="radio"
                :value="key"
                :text="p.name"
                :details="p.type"
                :selected="profileKey === key || undefined"
                @select="demo.setProfile(key)"
              ></nldd-menu-item>
            </nldd-menu-group>
          </nldd-toolbar-item>
          <!-- Het demo-menu hangt aan de toolbar zelf en niet meer aan een eigen
               ellipsis-knop. Die knop stond náást de overloopknop van de
               toolbar, allebei als `...`, en zodra er iets overliep won de
               overloopknop: het demo-menu was dan onbereikbaar. Als vaste
               inhoud van `slot="overflow"` staat alles onder één knop, met de
               overgelopen tabbladen erboven. -->
          <!-- Alles plat in groepen, geen uitklappers. Een submenu werkt hier
               niet: nldd-menu verplaatst een geopend submenu naar
               document.body, en de toolbar luistert op het hoofdmenu om het
               `select` van een kloon terug te mappen naar het origineel.
               Buiten dat menu bubbelt het event er nooit heen, dus Features en
               Weergave deden als uitklapper helemaal niets, op elke breedte.
               Groepen mét een titel geven dezelfde ordening zonder die klik. -->
          <nldd-menu-group slot="overflow" :text="t('app.features.label')">
            <!-- Geen `details` op deze items: dat is een kort label rechts,
                 geen ondertitel. Een hele zin erin duwt het label op een smal
                 scherm in een kolom van één woord breed, zodat "Wijziging
                 doorgeven" over vier regels brak. -->
            <nldd-menu-item
              v-for="f in FEATURES"
              :key="f.key"
              type="checkbox"
              :text="t(`app.features.${f.key}`)"
              :icon="f.icon"
              :selected="features[f.key] || undefined"
              @select="demo.toggleFeature(f.key)"
            ></nldd-menu-item>
            <!-- Handmatig beoordelen staat hier zonder streep ertussen: het is
                 dezelfde soort schakelaar als de vlaggen erboven. Het gaat over
                 aanvragen waar 'correcties direct goedkeuren' over correcties
                 gaat, maar dat is geen ander soort instelling. Het verschil is
                 alleen dat het in de demostaat zit en niet in de vlaggen, en
                 dat is niets wat een presentator hoeft te zien. -->
            <nldd-menu-item
              type="checkbox"
              :text="t('app.features.manualReview')"
              icon="checklist"
              :selected="state.manualReview || undefined"
              @select="toggleManualReview"
            ></nldd-menu-item>
            <nldd-menu-item
              type="checkbox"
              :text="t('app.features.autoAnnounce')"
              icon="paper-plane"
              :selected="state.autoAnnounce || undefined"
              @select="toggleAutoAnnounce"
            ></nldd-menu-item>
            <nldd-menu-item
              v-if="hasFeatureOverrides"
              :text="t('app.features.reset')"
              icon="refresh"
              @select="demo.resetFeatures()"
            ></nldd-menu-item>
          </nldd-menu-group>
          <nldd-menu-group slot="overflow" :text="t('app.language.label')">
            <!-- `@select` op het item en niet op de groep: het overloopmenu
                 toont een kloon en de toolbar dispatcht `select` op het
                 originele item, dus een handler op de groep eromheen vuurt
                 nooit. Zelfde reden als bij 'Namens wie' hierboven. -->
            <nldd-menu-item
              v-for="[value, label] in languageOptions"
              :key="value"
              type="radio"
              :value="value"
              :text="label"
              icon="languages"
              :selected="locale === value || undefined"
              @select="switchLocale(value)"
            ></nldd-menu-item>
          </nldd-menu-group>
          <nldd-menu-group slot="overflow" :text="t('app.appearance.label')">
            <nldd-menu-item
              v-for="[value, label, icon] in colorSchemeOptions"
              :key="value"
              type="radio"
              :value="value"
              :text="label"
              :icon="icon"
              :selected="colorScheme === value || undefined"
              @select="setColorScheme(value)"
            ></nldd-menu-item>
          </nldd-menu-group>
          <nldd-menu-group slot="overflow" :text="t('app.demo.label')">
            <nldd-menu-item :text="t('app.demo.fullscreen')" icon="square-arrow-up" @select="toggleFullscreen"></nldd-menu-item>
            <nldd-menu-item :text="t('app.demo.reset')" icon="refresh" @select="askReset"></nldd-menu-item>
            <!-- Only when a server answers /api/why; without one the feature
                 does not exist and the menu does not mention it. -->
            <nldd-menu-item v-if="whyAvailable && !whyUnlocked" :text="t('app.why.unlock')" icon="unlocked" @select="askWhyPassword"></nldd-menu-item>
            <nldd-menu-item v-if="whyUnlocked" :text="t('app.why.lock')" icon="locked" @select="lockWhy"></nldd-menu-item>
          </nldd-menu-group>
        </nldd-toolbar>
      </nldd-container>

      <nldd-split-view-pane slot="main" has-content>
        <nldd-page v-if="loadError">
          <nldd-simple-section>
            <nldd-banner variant="critical" :text="t('app.error.title')" :supporting-text="String(loadError)"></nldd-banner>
          </nldd-simple-section>
        </nldd-page>
        <nldd-page v-else-if="!ready">
          <nldd-simple-section height="60vh">
            <nldd-activity-indicator show-text :text="t('app.loading')" timing="instant" size="48"></nldd-activity-indicator>
          </nldd-simple-section>
        </nldd-page>
        <router-view v-else v-slot="{ Component }">
          <!-- Keyed so the walkthrough can drop every cached tab when it
               jumps (walkthrough/viewEpoch.ts). -->
          <keep-alive :key="viewEpoch">
            <component :is="Component" />
          </keep-alive>
        </router-view>
      </nldd-split-view-pane>
    </nldd-bar-split-view>

    <nldd-modal-dialog
      ref="whyDialog"
      :text="t('app.why.dialog.title')"
      :supporting-text="t('app.why.dialog.body')"
      :accessible-label="t('app.why.dialog.title')"
      horizontal-alignment="left"
    >
      <nldd-form-field :label="t('app.why.dialog.password')" @keydown.enter="confirmWhyPassword">
        <nldd-password-field
          :value="whyInput"
          autocomplete="current-password"
          :invalid="whyStatus === 'wrong' || undefined"
          :show-button-text="t('app.why.dialog.show')"
          :hide-button-text="t('app.why.dialog.hide')"
          :show-button-accessible-label="t('app.why.dialog.show_label')"
          :hide-button-accessible-label="t('app.why.dialog.hide_label')"
          @input="onWhyInput"
        ></nldd-password-field>
        <nldd-form-field-help-text v-if="whyStatus">{{ t(`app.why.dialog.${whyStatus}`) }}</nldd-form-field-help-text>
      </nldd-form-field>
      <nldd-button slot="actions" appearance="primary" :text="t('app.why.dialog.confirm')" :disabled="!whyInput || whyBusy || undefined" @click="confirmWhyPassword"></nldd-button>
      <nldd-button slot="actions" appearance="secondary" :text="t('app.why.dialog.cancel')" @click="whyDialog?.hide?.()"></nldd-button>
    </nldd-modal-dialog>

    <nldd-modal-dialog
      ref="resetDialog"
      variant="alert"
      :text="t('app.reset.title')"
      :supporting-text="t('app.reset.body')"
      :accessible-label="t('app.reset.label')"
    >
      <nldd-button slot="actions" appearance="destructive" :text="t('app.reset.confirm')" @click="confirmReset"></nldd-button>
      <nldd-button slot="actions" appearance="secondary" :text="t('app.reset.cancel')" @click="resetDialog?.hide?.()"></nldd-button>
    </nldd-modal-dialog>
  </nldd-shell>
</template>
