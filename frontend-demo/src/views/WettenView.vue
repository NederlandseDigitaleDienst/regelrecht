<script setup lang="ts">
import { computed, reactive, ref, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useLocalePath } from '../i18n/useLocalePath';
import { useI18n } from '../i18n/index.js';
import YamlNode from '@regelrecht/frontend-shared/components/YamlNode.vue';
import OrgLogo from '../components/OrgLogo.vue';
import LawGroupTree from '../components/LawGroupTree.vue';
import { useDemo } from '../store/demoStore';
import { serviceInfo, type LawEntry } from '../data/loadCorpus';
import type { NLDDNavigationSplitView } from '@nldd/design-system/navigation-split-view';

// The law browser: every demo law in the sidebar, grouped by the organisation
// that executes it; the selected law as a collapsible YAML tree in which every
// `source.regulation` is a link that opens the referenced law, with a back
// button retracing the walk. That walk from zorgtoeslag to BRP to the
// penitentiaire beginselenwet is the point of this tab.

const route = useRoute();
const router = useRouter();
// Het pad van dit tabblad in de taal die aan staat: een letterlijk `/wetten/...`
// zou een Engelse bezoeker bij elke wet die hij opent het Nederlandse tabblad
// in duwen.
const { localePath } = useLocalePath();
const { t, locale } = useI18n();
const { corpus, profile, profileKey } = useDemo();
// The law list is a sheet (primary-sidebar-as-sheet): closed by default so the
// law itself has the room, opened from the toolbar.
const splitView = ref<NLDDNavigationSplitView | null>(null);

// The laws walked through in this tab, in order; the back button in the
// header retraces them (zorgtoeslag → BRP → penitentiaire beginselenwet).
const trail = reactive<string[]>([]); // law ids
const activeId = ref<string | null>(null);
const showRaw = ref(false);
const query = ref('');
const expandState = reactive<{ paths: string[]; folded: string[]; version: number; all: boolean | null }>({ paths: [], folded: [], version: 0, all: null });

const lawIds = computed(() => new Set(corpus.value?.latestById.keys() ?? []));

/** Laws in the sidebar, honouring the profile's whitelist when it has one. */
const sidebarLaws = computed(() => {
  if (!corpus.value) return [];
  const all = [...corpus.value.latestById.values()];
  const allow = profile.value?.sidebar_laws;
  const filtered = allow ? all.filter((l) => allow.includes(l.law_path)) : all;
  const q = query.value.trim().toLowerCase();
  return q ? filtered.filter((l) => `${l.name} ${l.id} ${l.service}`.toLowerCase().includes(q)) : filtered;
});

const groups = computed(() => {
  const byService = new Map<string, LawEntry[]>();
  for (const law of sidebarLaws.value) {
    const key = law.service;
    if (!byService.has(key)) byService.set(key, []);
    byService.get(key)!.push(law); // set on the line above
  }
  return [...byService.entries()]
    .sort((a, b) => a[0].localeCompare(b[0]))
    // `sidebarLaws` is empty without a corpus, so there is one here.
    .map(([service, laws]) => ({ service, info: serviceInfo(corpus.value!, service), laws: laws.sort((a, b) => a.name.localeCompare(b.name)) }));
});

const activeLaw = computed(() => (activeId.value ? corpus.value?.lawById(activeId.value) : null));

function configFor(key: 'expanded_paths' | 'folded_paths', law: LawEntry): string[] {
  const cfg = corpus.value?.config?.[key] ?? {};
  return cfg[law.id] ?? cfg[law.law_path] ?? [];
}

function openLaw(lawId: string, { replaceRoute = false }: { replaceRoute?: boolean } = {}) {
  if (!lawIds.value.has(lawId)) return;
  if (trail.at(-1) !== lawId) trail.push(lawId);
  activeId.value = lawId;
  splitView.value?.hidePrimarySidebarSheet?.();
  // `lawIds` holds only ids of the loaded corpus, so both are there.
  const law = corpus.value!.lawById(lawId)!;
  expandState.paths = configFor('expanded_paths', law);
  expandState.folded = configFor('folded_paths', law);
  expandState.all = null;
  expandState.version += 1;
  const target = localePath('wetten', { lawId });
  if (route.fullPath !== target) (replaceRoute ? router.replace : router.push).call(router, target);
}

function goBack() {
  if (trail.length < 2) return;
  trail.pop();
  const previous = trail.pop()!; // at least one entry left after the first pop
  openLaw(previous, { replaceRoute: true });
}

function expandAll() {
  expandState.all = true;
  expandState.version += 1;
}
function collapseAll() {
  expandState.all = false;
  expandState.version += 1;
}
function resetExpansion() {
  expandState.all = null;
  expandState.paths = activeLaw.value ? configFor('expanded_paths', activeLaw.value) : [];
  expandState.folded = activeLaw.value ? configFor('folded_paths', activeLaw.value) : [];
  expandState.version += 1;
}

// Route → state (deep link, and the presentation's hand-off). The profile's
// default law opens when the tab is entered without one.
watch(
  // Op `meta.page` en niet op `route.name`: dezelfde pagina heeft per taal een
  // eigen routenaam (`wetten` en `wetten:en`), en op de naam vergelijken laat
  // deze watcher onder /en/ruleworks meteen terugkeren. Het tabblad opent dan geen
  // enkele wet, ook niet de standaardwet van het profiel, en toont een leeg
  // paneel zonder dat er iets faalt.
  () => [route.meta?.page, route.params.lawId, corpus.value, profileKey.value],
  ([page, lawId, , key], old) => {
    // Een ander persona begint bij zijn eigen wet. <keep-alive> houdt dit
    // tabblad gemount, dus zonder deze reset bleef na de wissel naar Claudia
    // de zorgtoeslag van Merijn open staan. Van `null` naar het standaard-
    // profiel (het corpus is net geladen) is geen wissel: dan moet een deeplink
    // gewoon openen. Net zo een link die tegelijk met het profiel verandert
    // (een dia met `profile` en een wet in `route`); alleen de oude link wijkt.
    const switched = old?.[3] != null && old[3] !== key;
    if (switched) {
      trail.splice(0);
      activeId.value = null;
    }
    if (page !== 'wetten' || !corpus.value) return;
    if (lawId && typeof lawId === 'string' && !(switched && lawId === old[1])) {
      openLaw(decodeURIComponent(lawId), { replaceRoute: true });
    } else if (!activeId.value && profile.value?.default_law) {
      const d = profile.value.default_law;
      const law = corpus.value.lawByPath(d.law_path, d.service);
      if (law) openLaw(law.id, { replaceRoute: true });
    }
  },
  { immediate: true },
);

function tabInfo(lawId: string) {
  return corpus.value?.lawById(lawId);
}

const references = computed(() => {
  // Laws this law depends on, for the inspector column.
  const law = activeLaw.value;
  if (!law) return [];
  const ids = new Set<string>();
  const walk = (node: unknown): void => {
    if (Array.isArray(node)) node.forEach(walk);
    else if (node && typeof node === 'object') {
      const regulation = (node as { regulation?: unknown }).regulation;
      if (typeof regulation === 'string') ids.add(regulation);
      Object.values(node).forEach(walk);
    }
  };
  walk(law.doc.articles);
  // An active law means the corpus is loaded.
  return [...ids].map((id) => corpus.value!.lawById(id)).filter((l): l is LawEntry => Boolean(l));
});

const referencedBy = computed(() => {
  const law = activeLaw.value;
  if (!law) return [];
  // An active law means the corpus is loaded.
  return [...corpus.value!.latestById.values()].filter((other) => other.id !== law.id && other.text.includes(`regulation: ${law.id}\n`));
});
</script>

<template>
  <nldd-navigation-split-view ref="splitView" primary-sidebar-as-sheet :primary-sidebar-accessible-label="t('wet.sidebar.label')" :inspector-accessible-label="t('wet.inspector.label')">
    <nldd-split-view-pane slot="sidebar" has-content background="tinted">
      <nldd-page sticky-header background="inherit">
        <nldd-container slot="header" padding="12" gap="8">
          <nldd-top-title-bar :text="t('wet.sidebar.label')" :supporting-text="t.plural(sidebarLaws.length, 'wet.sidebar.count')"></nldd-top-title-bar>
          <nldd-search-field :placeholder="t('wet.sidebar.search')" size="sm" :value="query" @input="query = $event.detail?.value ?? $event.target.value"></nldd-search-field>
        </nldd-container>
        <nldd-container padding-inline="8" padding-bottom="16">
          <LawGroupTree :groups="groups" mode="pick" :active-id="activeId" @pick="openLaw" />
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane slot="main" has-content>
      <nldd-page landmarks="page" sticky-header>
        <nldd-container slot="header" padding="8">
          <nldd-toolbar size="sm">
            <nldd-toolbar-item slot="start">
              <nldd-button size="sm" appearance="neutral-tinted" start-icon="books" :text="t('wet.sidebar.label')" :supporting-text="`${sidebarLaws.length}`" @click="splitView?.showPrimarySidebarSheet?.()"></nldd-button>
            </nldd-toolbar-item>
            <template v-if="activeLaw">
              <nldd-toolbar-item slot="start" v-if="trail.length > 1">
                <!-- `trail.length > 1` guarantees `trail.at(-2)`; an active law guarantees the corpus. -->
                <nldd-icon-button size="sm" appearance="neutral-transparent" icon="chevron-left" :text="t('wet.back', { name: tabInfo(trail.at(-2)!)?.name ?? t('wet.back.fallback') })" @click="goBack"></nldd-icon-button>
              </nldd-toolbar-item>
              <nldd-toolbar-title slot="start" :text="activeLaw.name" :supporting-text="t('wet.valid_from', { service: serviceInfo(corpus!, activeLaw.service).name, date: activeLaw.valid_from })" max-width="480px"></nldd-toolbar-title>
              <nldd-toolbar-item slot="end">
                <nldd-segmented-control size="sm" width="fit-content" :value="showRaw ? 'raw' : 'tree'" @change="showRaw = $event.detail?.value === 'raw'">
                  <nldd-segmented-control-item value="tree" :text="t('wet.view.tree')"></nldd-segmented-control-item>
                  <nldd-segmented-control-item value="raw" :text="t('wet.view.raw')"></nldd-segmented-control-item>
                </nldd-segmented-control>
              </nldd-toolbar-item>
              <nldd-toolbar-item slot="end" v-if="!showRaw">
                <nldd-button-bar size="sm">
                  <nldd-icon-button icon="chevron-up-chevron-down" :text="t('wet.expand.default')" @click="resetExpansion"></nldd-icon-button>
                  <nldd-icon-button icon="chevron-down" :text="t('wet.expand.all')" @click="expandAll"></nldd-icon-button>
                  <nldd-icon-button icon="chevron-up" :text="t('wet.collapse.all')" @click="collapseAll"></nldd-icon-button>
                </nldd-button-bar>
              </nldd-toolbar-item>
              <nldd-toolbar-item slot="end">
                <nldd-button size="sm" appearance="neutral-tinted" end-icon="external-link" text="wetten.overheid.nl" :href="activeLaw.doc.url" target="_blank"></nldd-button>
              </nldd-toolbar-item>
            </template>
          </nldd-toolbar>
        </nldd-container>

        <nldd-simple-section v-if="!activeLaw" height="60vh">
          <nldd-inline-dialog icon="books" :text="t('wet.empty.title')" :supporting-text="t('wet.empty.body')">
            <nldd-button slot="actions" appearance="primary" size="sm" :text="t('wet.sidebar.label')" @click="splitView?.showPrimarySidebarSheet?.()"></nldd-button>
          </nldd-inline-dialog>
        </nldd-simple-section>
        <nldd-simple-section v-else width="full">
          <!-- De wettekst blijft Nederlands, en dat hoort uitgelegd te worden
               waar hij staat. Alleen in het Engels: voor een Nederlandse lezer
               valt er niets te verklaren. -->
          <template v-if="locale !== 'nl'">
            <nldd-banner variant="neutral" :text="t('wet.dutch_only.title')" :supporting-text="t('wet.dutch_only.body')">
              <nldd-button
                slot="actions"
                size="sm"
                appearance="neutral-tinted"
                end-icon="external-link"
                :text="t('wet.dutch_only.published')"
                :href="activeLaw.doc.url"
                target="_blank"
              ></nldd-button>
            </nldd-banner>
            <nldd-spacer size="16"></nldd-spacer>
          </template>
          <!-- `lang="nl"` op het paneel zelf, ook als de pagina op Engels staat.
               Geen decoratie: een schermlezer kiest hierop zijn stem, en de
               tekst eronder is Nederlands. Het is de eerlijke opmaak. -->
          <nldd-code-viewer v-if="showRaw" lang="nl" language="yaml" wrap>{{ activeLaw.text }}</nldd-code-viewer>
          <nldd-box v-else>
            <nldd-container padding="12">
              <div class="yaml-tree" lang="nl">
                <YamlNode :key="activeLaw.id" :value="activeLaw.doc" :expanded="expandState" :law-ids="lawIds" @open-law="openLaw" />
              </div>
            </nldd-container>
          </nldd-box>
        </nldd-simple-section>
      </nldd-page>
    </nldd-split-view-pane>

    <nldd-split-view-pane v-if="activeLaw" slot="inspector" has-content background="tinted">
      <nldd-page background="inherit">
        <nldd-container slot="header" padding="12">
          <nldd-top-title-bar :text="t('wet.inspector.label')"></nldd-top-title-bar>
        </nldd-container>
        <nldd-container padding="12" gap="16">
          <nldd-list appearance="box-base" :accessible-label="t('wet.executed_by')">
            <nldd-list-item size="md">
              <!-- An active law means the corpus is loaded. -->
              <nldd-cell><OrgLogo :service="activeLaw.service" /></nldd-cell>
              <nldd-spacer-cell size="12"></nldd-spacer-cell>
              <nldd-text-cell :overline="t('wet.executed_by')" :text="serviceInfo(corpus!, activeLaw.service).name"></nldd-text-cell>
            </nldd-list-item>
          </nldd-list>
          <nldd-container gap="4">
            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">{{ t('wet.uses_data_from') }}</nldd-text></nldd-container>
            <nldd-list appearance="box-base" :accessible-label="t('wet.uses_data_from')">
              <nldd-list-item v-if="references.length === 0" size="sm"><nldd-text-cell size="sm" color="secondary" :text="t('wet.no_other_laws')"></nldd-text-cell></nldd-list-item>
              <nldd-list-item v-for="ref in references" :key="ref.id" size="sm" button @click="openLaw(ref.id)">
                <nldd-cell><OrgLogo :service="ref.service" size="sm" /></nldd-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-text-cell size="sm" :text="ref.name" :supporting-text="ref.id"></nldd-text-cell>
                <nldd-icon-cell icon="chevron-right" size="16"></nldd-icon-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-container>
          <nldd-container gap="4">
            <nldd-container padding-inline="12"><nldd-text size="sm" weight="medium" color="secondary">{{ t('wet.used_by') }}</nldd-text></nldd-container>
            <nldd-list appearance="box-base" :accessible-label="t('wet.used_by')">
              <nldd-list-item v-if="referencedBy.length === 0" size="sm"><nldd-text-cell size="sm" color="secondary" :text="t('wet.no_other_laws')"></nldd-text-cell></nldd-list-item>
              <nldd-list-item v-for="ref in referencedBy" :key="ref.id" size="sm" button @click="openLaw(ref.id)">
                <nldd-cell><OrgLogo :service="ref.service" size="sm" /></nldd-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-text-cell size="sm" :text="ref.name" :supporting-text="ref.id"></nldd-text-cell>
                <nldd-icon-cell icon="chevron-right" size="16"></nldd-icon-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-container>
        </nldd-container>
      </nldd-page>
    </nldd-split-view-pane>
  </nldd-navigation-split-view>
</template>
