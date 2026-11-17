<script setup lang="ts">
/**
 * HarvesterView - the harvester-admin "Corpusinwinning" section, merged into the editor.
 *
 * Top-level route (sibling of AppShell, not nested) so it carries its own
 * compact chrome instead of the editor's app chrome - mirroring the other
 * standalone views (TrajectChooserView). It hosts the two
 * sub-screens (Law Entries / Jobs) via a nested <router-view>, matching the
 * original standalone admin dashboard's App shell.
 *
 * Reads/writes go to `/api/harvest-admin/*`, which editor-api proxies to the
 * standalone harvester-admin service (forwarding the session cookie). Access is
 * gated at the route (meta.requiresRole) and the menu item; write actions are
 * enforced server-side by the harvester-admin API.
 */
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useAuth } from '../composables/useAuth';
import { useColorScheme } from '../composables/useColorScheme';
import { usePlatformInfo } from './composables/usePlatformInfo';
import { useNewHarvestJob } from './composables/useNewHarvestJob';
import { harvesterReturnPath } from '../composables/useLastVisitedRoute';
import NewHarvestJobSheet from './components/NewHarvestJobSheet.vue';
import './harvester.css';

const route = useRoute();
const router = useRouter();
const { authenticated, oidcConfigured, person, loading: authLoading, login, logout } = useAuth();
const { colorScheme, setColorScheme } = useColorScheme();
const { info } = usePlatformInfo();
const { open: openNewHarvestJob } = useNewHarvestJob();

const colorSchemeOptions: [value: string, label: string][] = [
  ['auto', 'Systeem'],
  ['light', 'Licht'],
  ['dark', 'Donker'],
];

// Show the deployment name (e.g. a prN preview) as a badge, but not for the
// canonical production deployment.
const deploymentName = computed(() =>
  info.value?.deployment_name && info.value.deployment_name !== 'regelrecht'
    ? info.value.deployment_name
    : null,
);

const tabs = [
  { key: 'overview', label: 'Overzicht', route: '/harvesting/overview' },
  { key: 'law-entries', label: 'Wetten', route: '/harvesting/law-entries' },
  { key: 'jobs', label: 'Taken', route: '/harvesting/jobs' },
  { key: 'markings', label: 'Markeringen', route: '/harvesting/markings' },
  // The predecessor of markings, for laws still pinned to schema v0.5.x.
  { key: 'untranslatables', label: 'Untranslatables', route: '/harvesting/untranslatables' },
];
const activeTab = computed(() => route.name);

function goToLibrary() {
  // Return to whichever page (Home or Editor) the user came from.
  router.push(harvesterReturnPath());
}
</script>

<template>
  <nldd-shell>
    <div class="harvesting-view">
      <nldd-container padding="8">
        <nldd-toolbar size="md">
          <nldd-toolbar-item slot="start">
            <nldd-icon-button
              icon="arrow-left"
              text="Terug"
              tooltip-timing="never"
              appearance="neutral-tinted"
              @click="goToLibrary"
            ></nldd-icon-button>
            <nldd-menu-item slot="overflow" icon="arrow-left" text="Terug" @select="goToLibrary"></nldd-menu-item>
          </nldd-toolbar-item>
          <nldd-toolbar-title
            slot="start"
            text="Harvester"
            supporting-text="RegelRecht"
            min-width="fit-content"
          ></nldd-toolbar-title>
          <nldd-toolbar-item slot="center">
            <nldd-tab-bar accessible-label="Onderdelen">
              <nldd-tab-bar-item
                v-for="tab in tabs"
                :key="tab.key"
                :text="tab.label"
                :current="activeTab === tab.key ? '' : undefined"
                @click="router.push(tab.route)"
              ></nldd-tab-bar-item>
            </nldd-tab-bar>
            <nldd-menu-group slot="overflow" text="Onderdelen">
              <nldd-menu-item
                v-for="tab in tabs"
                :key="`overflow-${tab.key}`"
                type="radio"
                :text="tab.label"
                :selected="activeTab === tab.key || undefined"
                @select="router.push(tab.route)"
              ></nldd-menu-item>
            </nldd-menu-group>
          </nldd-toolbar-item>
          <nldd-toolbar-item v-if="deploymentName" slot="end">
            <nldd-tag
              color="warning"
              :text="deploymentName"
            ></nldd-tag>
            <!-- The tag names the environment, it is no action: in the overflow
                 menu it is a group title without items, not a disabled item. -->
            <nldd-menu-group slot="overflow" :text="deploymentName"></nldd-menu-group>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button
              icon="plus-small"
              text="Nieuwe harvest-job"
              tooltip-timing="never"
              appearance="neutral-tinted"
              @click="openNewHarvestJob"
            ></nldd-icon-button>
            <nldd-menu-item slot="overflow" icon="plus-small" text="Nieuwe harvest-job" @select="openNewHarvestJob"></nldd-menu-item>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button size="md" icon="account" text="Account" tooltip-timing="never" expandable>
              <nldd-menu slot="popup">
                <nldd-menu-item
                  v-if="!authLoading && authenticated"
                  :text="person?.name || person?.email"
                  disabled
                ></nldd-menu-item>
                <nldd-menu-group text="Weergave">
                  <nldd-menu-item
                    v-for="[value, label] in colorSchemeOptions"
                    :key="`scheme-${value}`"
                    type="radio"
                    :selected="colorScheme === value || undefined"
                    :text="label"
                    @select="setColorScheme(value)"
                  ></nldd-menu-item>
                </nldd-menu-group>
                <nldd-menu-divider></nldd-menu-divider>
                <nldd-menu-item
                  v-if="!authLoading && authenticated"
                  text="Uitloggen"
                  icon="logout"
                  @click="logout"
                ></nldd-menu-item>
                <nldd-menu-item
                  v-else-if="!authLoading && oidcConfigured"
                  text="Inloggen"
                  icon="login"
                  @click="login()"
                ></nldd-menu-item>
              </nldd-menu>
            </nldd-icon-button>
            <nldd-menu-item slot="overflow" icon="account" text="Account">
              <nldd-menu>
                <nldd-menu-item
                  v-if="!authLoading && authenticated"
                  :text="person?.name || person?.email"
                  disabled
                ></nldd-menu-item>
                <nldd-menu-group text="Weergave">
                  <nldd-menu-item
                    v-for="[value, label] in colorSchemeOptions"
                    :key="`overflow-scheme-${value}`"
                    type="radio"
                    :selected="colorScheme === value || undefined"
                    :text="label"
                    @select="setColorScheme(value)"
                  ></nldd-menu-item>
                </nldd-menu-group>
                <nldd-menu-divider></nldd-menu-divider>
                <nldd-menu-item
                  v-if="!authLoading && authenticated"
                  text="Uitloggen"
                  icon="logout"
                  @select="logout"
                ></nldd-menu-item>
                <nldd-menu-item
                  v-else-if="!authLoading && oidcConfigured"
                  text="Inloggen"
                  icon="login"
                  @select="login()"
                ></nldd-menu-item>
              </nldd-menu>
            </nldd-menu-item>
          </nldd-toolbar-item>
        </nldd-toolbar>
      </nldd-container>
      <nldd-divider></nldd-divider>

      <nldd-page>
        <router-view></router-view>
      </nldd-page>
    </div>
    <NewHarvestJobSheet />
  </nldd-shell>
</template>
