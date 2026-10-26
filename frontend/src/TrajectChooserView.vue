<script setup lang="ts">
import { computed, onMounted, watchEffect } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { singleQuery } from './lib/routeParams';
import { useTrajects, refreshTrajects } from './composables/useTrajects';
import type { TrajectSummary } from './composables/useTrajects';
import { lastHomePath, homeTarget } from './composables/useLastVisitedRoute';

// Trajectkeuze-pagina - de landing wanneer de editor (of een traject-scoped
// bibliotheek) een traject vereist. Hier kies je een bestaand traject of ga je
// door naar de aanmaakpagina. Een meegekregen wet (query `law`/`article`, gezet
// door de redirect van oude no-traject editor-links) opent na de keuze direct.
//
// Top-level route (geen AppShell-child): geen app-chrome. De pagina draagt z'n
// eigen top-title-bar met terugknop naar de bibliotheek.
const route = useRoute();
const router = useRouter();
const { trajects, loading, error } = useTrajects();

// Set when the editor (or a traject-scoped view) redirected here because the
// requested traject is missing/inaccessible - see EditorView's trajectMissing.
// The ref is unknown at that point (no access = no name lookup), so the banner
// stays name-agnostic.
const trajectMissing = computed(() => route.query.melding === 'traject-missing');

// Always refetch on entry: the landing page should reflect trajects
// created elsewhere (other tab, other device) since the cached
// module-level fetch.
onMounted(() => {
  refreshTrajects();
});

watchEffect(() => {
  document.title = 'Trajecten · RegelRecht';
});

// Terug naar waar de user vandaan kwam vóór de trajectkeuze: de bibliotheek, via
// z'n in sessionStorage bewaarde last-visited-pad. Dat pad overleeft de full-
// page SSO-redirect die een uitgelogde user naar deze auth-only pagina bracht;
// een history-pop zou op de login-redirect landen.
function goBack() {
  router.push(lastHomePath.value);
}

// Where picking a traject takes you, from `sectie` (default editor): the
// library or editor traject-scoped route, carrying the law/article the user was
// heading for so they land on their intended destination.
function trajectTarget(trajectRef: string) {
  const lawId = singleQuery(route.query.law) || undefined;
  const articleNumber = singleQuery(route.query.article) || undefined;
  // `sectie=library` came from the Home section; editor otherwise. Home splits
  // into traject-home / library-traject on whether a law is carried.
  return route.query.sectie === 'library'
    ? homeTarget({ trajectRef, lawId, articleNumber })
    : { name: 'editor-traject', params: { trajectRef, lawId, articleNumber } };
}

function selectTraject(t: TrajectSummary) {
  // `t.ref` serialises to null when the backend builds a TrajectSummary
  // without fill_ref() - refuse to navigate (same guard as TrajectMenu).
  if (!t.ref) {
    console.warn('TrajectChooser: traject has no ref', t);
    return;
  }
  router.push(trajectTarget(t.ref));
}

function openCreate() {
  // Query meegeven zodat een meegekregen wet ook na het aanmaken opent.
  router.push({ name: 'editor-nieuw-traject', query: route.query });
}

function trajectSupportingText(t: TrajectSummary) {
  const parts: string[] = [];
  if (t.status === 'afgerond') parts.push('Afgerond');
  if (t.description) parts.push(t.description);
  return parts.join(' - ') || undefined;
}
</script>

<template>
  <nldd-shell>
    <nldd-page sticky-header>
      <nldd-top-title-bar
        slot="header"
        text="Trajecten"
        back-text="Home"
        collapse-anchor="kies-traject-titel"
        @back="goBack"
      ></nldd-top-title-bar>

      <nldd-simple-section width="800px">
        <nldd-title id="kies-traject-titel" size="3"><h3>Trajecten</h3></nldd-title>
        <nldd-spacer size="16"></nldd-spacer>
        <template v-if="trajectMissing">
          <nldd-banner
            variant="warning"
            text="Het traject dat je probeerde te openen bestaat niet, niet meer of je hebt geen toegang"
            supporting-text="Kies een ander traject of maak een nieuw traject aan."
          ></nldd-banner>
          <nldd-spacer size="16"></nldd-spacer>
        </template>
        <nldd-activity-indicator v-if="loading" text="Trajecten laden" show-text></nldd-activity-indicator>
        <nldd-inline-dialog
          v-else-if="error"
          variant="alert"
          text="Trajecten zijn niet geladen"
          supporting-text="De gegevens konden niet worden opgehaald."
        >
          <nldd-button slot="actions" appearance="primary" text="Probeer opnieuw" @click="refreshTrajects()"></nldd-button>
        </nldd-inline-dialog>
        <!-- "Nieuw traject" is een gewoon list item onderaan, zodat de
             interactie identiek is mét bestaande trajecten (onderaan de
             lijst) en zonder (als enige item). -->
        <nldd-list v-else appearance="box-tinted">
          <nldd-list-item
            v-for="t in trajects"
            :key="t.id"
            size="md"
            button
            @click="selectTraject(t)"
          >
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-icon-cell size="20"><nldd-icon icon="traject"></nldd-icon></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell :text="t.name" :supporting-text="trajectSupportingText(t)"></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell size="20">
              <nldd-icon icon="chevron-right"></nldd-icon>
            </nldd-icon-cell>
          </nldd-list-item>
          <nldd-list-item size="md"
            button
            @click="openCreate"
          >
            <nldd-spacer-cell size="12"></nldd-spacer-cell>
            <nldd-icon-cell size="20"><nldd-icon icon="plus"></nldd-icon></nldd-icon-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-text-cell text="Nieuw traject"></nldd-text-cell>
            <nldd-spacer-cell size="8"></nldd-spacer-cell>
            <nldd-icon-cell size="20">
              <nldd-icon icon="chevron-right"></nldd-icon>
            </nldd-icon-cell>
          </nldd-list-item>
        </nldd-list>
      </nldd-simple-section>

      <nldd-page-footer slot="footer"></nldd-page-footer>
    </nldd-page>
  </nldd-shell>
</template>
