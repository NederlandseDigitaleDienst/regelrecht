<script setup lang="ts">
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useAuth } from '../composables/useAuth';
import { useGithubAuth } from '../composables/useGithubAuth';
import { useColorScheme } from '../composables/useColorScheme';
import { useSettingsSections } from '../composables/useSettingsSections';
import { rememberHarvesterOrigin } from '../composables/useLastVisitedRoute';

// The contents of the shell's account menu (its identity header and items),
// rendered bare so the same contents fill both the account button's popup
// menu and its alternative in the toolbar's overflow menu. The items act on
// `select`: the toolbar copies an overflow item into its menu and sends
// `select` on to the original, never `click`. What opens one of the shell's
// sheets is emitted; the shell owns those.
const emit = defineEmits<{
  settings: [];
  about: [];
  support: [];
  'request-account': [];
}>();

const { authenticated, loading: authLoading, oidcConfigured, person, hasAnyRole, login, logout } = useAuth();

// Only for the identity line in the header — linking itself lives in the
// settings sheet. The login is shown only when writes actually go out under
// it, otherwise it would misstate who authors the commit. `required` is the
// effective answer (env var OR feature flag), which is why the flag is not
// read here directly.
const { status: githubStatus } = useGithubAuth();
const showGithubLine = computed(
  () =>
    !!githubStatus.value?.configured &&
    !!githubStatus.value?.required &&
    !!githubStatus.value?.connected,
);

// Roles that may reach the harvester-admin "Corpusinwinning" section. Any harvester-*
// tier (reader/writer/admin) or the spanning regelrecht-admin sees the menu
// item; write actions inside the section are still enforced server-side by
// the harvester-admin API. Composite-role expansion means a higher tier
// already carries the lower ones, but we list all four so a directly-assigned
// role can never be missed.
const HARVESTER_ROLES = [
  'harvester-reader',
  'harvester-writer',
  'harvester-admin',
  'regelrecht-admin',
];
const canViewHarvesting = computed(
  () => authenticated.value && hasAnyRole(HARVESTER_ROLES),
);
const route = useRoute();
const router = useRouter();
function goToHarvesting() {
  // Remember where we came from so the harvester's back button returns here.
  rememberHarvesterOrigin(route.fullPath);
  router.push('/harvesting');
}

// The colour scheme is a submenu here rather than a setting in the settings
// sheet: it is something you switch briefly, and the radios show at once which
// one is on.
const { colorScheme, setColorScheme } = useColorScheme();
const colorSchemeOptions = [
  ['auto', 'Systeem', 'display'],
  ['light', 'Licht', 'light-mode'],
  ['dark', 'Donker', 'dark-mode'],
];
// `select` carries no payload; the chosen value is on the item itself.
function onColorSchemeSelect(e: Event) {
  const value = (e.target as Element | null)?.getAttribute?.('value');
  if (value) setColorScheme(value);
}

// Without Weergave the sheet only keeps sections behind a role or an
// environment setting. If none is left, the menu item goes too: an
// "Instellingen" that opens an empty sheet is worse than none.
const { hasContent: settingsHasContent } = useSettingsSections();
</script>

<template>
  <nldd-container v-if="!authLoading && authenticated" slot="header" padding-inline="16" padding-block="12">
    <nldd-identity :text="person?.name || person?.email">
      <span v-if="person?.name || showGithubLine" slot="supporting-text">
        <template v-if="person?.name">{{ person?.email }}</template>
        <br v-if="person?.name && showGithubLine">
        <template v-if="showGithubLine">GitHub: {{ githubStatus?.github_login }}</template>
      </span>
    </nldd-identity>
  </nldd-container>
  <template v-if="!authLoading && oidcConfigured && !authenticated">
    <nldd-menu-item text="Inloggen" icon="login" @select="login()"></nldd-menu-item>
    <nldd-menu-item text="Account aanvragen" icon="new-account" @select="emit('request-account')"></nldd-menu-item>
    <nldd-menu-divider></nldd-menu-divider>
  </template>
  <nldd-menu-item text="Weergave" icon="appearance">
    <nldd-menu @select="onColorSchemeSelect">
      <nldd-menu-item
        v-for="[value, label, icon] in colorSchemeOptions"
        :key="value"
        type="radio"
        :value="value"
        :text="label"
        :icon="icon"
        :selected="colorScheme === value || undefined"
      ></nldd-menu-item>
    </nldd-menu>
  </nldd-menu-item>
  <nldd-menu-item v-if="settingsHasContent" text="Instellingen" icon="gear" @select="emit('settings')"></nldd-menu-item>
  <nldd-menu-item v-if="canViewHarvesting" text="Harvester" icon="harvest" @select="goToHarvesting"></nldd-menu-item>
  <nldd-menu-divider></nldd-menu-divider>
  <nldd-menu-item text="Over RegelRecht" icon="info" @select="emit('about')"></nldd-menu-item>
  <nldd-menu-item text="Help" icon="help" @select="emit('support')"></nldd-menu-item>
  <template v-if="!authLoading && authenticated">
    <nldd-menu-divider></nldd-menu-divider>
    <nldd-menu-item text="Log uit" icon="logout" @select="logout"></nldd-menu-item>
  </template>
</template>
