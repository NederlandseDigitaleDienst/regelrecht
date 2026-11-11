<script setup lang="ts">
import { computed, ref, nextTick, onMounted, onBeforeUnmount, provide } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import type { NLDDModalDialog } from '@nldd/design-system/modal-dialog';
import type { NLDDPopover } from '@nldd/design-system/popover';
import TrajectMenu from './components/TrajectMenu.vue';
import MobileTrajectSheet from './components/MobileTrajectSheet.vue';
import AboutSheet from './components/AboutSheet.vue';
import SupportSheet from './components/SupportSheet.vue';
import SettingsSheet from './components/SettingsSheet.vue';
import NewMenuItems from './components/NewMenuItems.vue';
import AccountMenuItems from './components/AccountMenuItems.vue';
import { useAuth } from './composables/useAuth';
import { useTrajects } from './composables/useTrajects';
import {
  lastHomePath,
  lastEditorPath,
  editorTabTarget as buildEditorTabTarget,
  homeTabTarget,
  isHomeSection,
} from './composables/useLastVisitedRoute';
import { useAppChrome, openSearch, openSearchAt, onBarSearchKeydown } from './composables/useAppChrome';
import { SEARCH_PLACEHOLDER, SEARCH_ACCESSIBLE_LABEL } from './constants';
import type { OpenTab } from './lib/openTabsStorage';

// Persistent shell that owns the shared chrome (tab-bar, search trigger,
// TrajectMenu, settings menu) and a nested <router-view> for the editor /
// library bodies. Because both views are children of this one route record,
// switching between them swaps only the nested router-view - the shell
// instance is reused, so the chrome never rebuilds (no refresh flash).

const { authenticated, loading: authLoading, oidcConfigured, person, login } = useAuth();
const { activeTrajectRef } = useTrajects();

// "Over RegelRecht" about sheet, opened from the account menu.
const aboutSheet = ref<InstanceType<typeof AboutSheet> | null>(null);
function openAbout() {
  // Let the account menu popover close first, then raise the sheet.
  nextTick(() => aboutSheet.value?.show?.());
}

// "Help" sheet, opened from the account menu.
const supportSheet = ref<InstanceType<typeof SupportSheet> | null>(null);
function openSupport() {
  // Let the account menu popover close first, then raise the sheet.
  nextTick(() => supportSheet.value?.show?.());
}

// Rejecting a proposal throws the seeded edit away, so it asks first. Owned
// here because the Verwerp button lives in the changes bar; EditorView keeps
// the actual reject logic.
const rejectConfirm = ref<NLDDModalDialog | null>(null);
function confirmReject() {
  rejectConfirm.value?.hide();
  editorActions.value?.reject?.();
}

// "Rond af, neem de rest niet over" sluit de hele verrijking af zonder de
// resterende onderdelen te bekijken - destructiever dan één verwerping, dus
// die vraagt het ook eerst.
const rejectRestConfirm = ref<NLDDModalDialog | null>(null);
function confirmRejectRest() {
  rejectRestConfirm.value?.hide();
  editorActions.value?.rejectRest?.();
}

// In review mode the bar decides on a proposal, not on your own edits, so the
// labels say so. Outside review "Opslaan" stays what it always was. Het
// oordeel gaat over één onderdeel van de verrijking en schrijft nog niets weg,
// dus "Neem over" en niet "Sla op" - er wordt pas geschreven als alle
// onderdelen beoordeeld zijn.
const inReview = computed(() => !!editorChanges.value?.review);
const saveLabel = computed(() => (inReview.value ? 'Neem voorstel over' : 'Opslaan'));
const canRejectRest = computed(() => !!editorChanges.value?.reviewCanRejectRest);

// Not dismissible: the notice explains what Verwerp and Opslaan below it refer
// to, so hiding it would leave two decision buttons without their subject. It
// disappears by deciding, not by clicking it away.
//
// Only while the editor has an article open that carries a proposal. Outside
// the editor `editorChanges` is null, which is what keeps the notice off Home.
const reviewNotice = computed(() => editorChanges.value?.reviewStatus ?? null);

// "Instellingen" sheet, opened from the account menu. Owns Weergave, the panel
// flags and the GitHub link — all of which used to hang off this menu directly.
const settingsSheet = ref<InstanceType<typeof SettingsSheet> | null>(null);
function openSettings() {
  // Let the account menu popover close first, then raise the sheet.
  nextTick(() => settingsSheet.value?.show?.());
}

const route = useRoute();
const router = useRouter();

// Which top-level section is active, derived from the route. Both tabs are
// always rendered; the active one shows `selected` and does not navigate,
// the other carries the cross-section target (traject re-stamped via
// sectionTarget) - matching the previous per-app behaviour.
// True on the Home section (public landing, a public law, the traject landing,
// or a traject law). Kept named isLibraryRoute to limit this refactor's blast
// radius.
const isLibraryRoute = computed(() => isHomeSection(route.name));
// Home tab: the last home path with the active traject re-stamped onto it
// (see homeTabTarget) - switching Home<->Editor keeps you in the traject
// you're working in, instead of restoring a stale scope (e.g. Corpus juris
// after opening a public deep-link). The Editor tab does the same via
// sectionTarget, which additionally carries the editor's traject logic
// (chooser + law-as-query when no traject is active).
const libraryTabTarget = computed(() =>
  homeTabTarget(router, lastHomePath.value, activeTrajectRef.value),
);
const editorTabTarget = computed(() =>
  buildEditorTabTarget(router, lastEditorPath.value, activeTrajectRef.value),
);
const libraryTabHref = computed(() => router.resolve(libraryTabTarget.value).href);
const editorTabHref = computed(() => router.resolve(editorTabTarget.value).href);

// The editor requires login. Rather than letting the route guard bounce an
// unauthenticated user straight to the SSO screen (an unannounced surprise),
// intercept the Editor tab and first show a small login-warning popover anchored
// to the clicked tab. Authenticated users navigate as before.
const loginWarning = ref<NLDDPopover | null>(null);
// Where the "Inloggen" button returns after SSO. Defaults to the editor tab;
// callers (e.g. the Bibliotheek "Bewerken" button) point it at a specific
// article so login lands straight on the page being edited.
const loginRedirect = ref<string | null>(null);

// The login-warning popover is popover=auto: a re-tap on the trigger that opened
// it light-dismisses it on pointerdown, but showLoginWarning would then reopen
// it right away. Snapshot the open state at pointerdown (capture, before the
// dismiss) so a re-tap toggles it closed. Every login trigger (the Editor tabs,
// the Bibliotheek "Bewerken" button) wires @pointerdown.capture to this.
let loginWarningWasOpen = false;
function onLoginTriggerPointerdown() {
  loginWarningWasOpen = loginWarning.value?.open ?? false;
}
provide('onLoginTriggerPointerdown', onLoginTriggerPointerdown);

// Show the login-warning popover anchored to `anchorEl`. Provided to the nested
// views so every editor entry point (the Editor tab, the Bibliotheek
// "Bewerken" button) shows the same heads-up instead of bouncing to SSO.
function showLoginWarning(anchorEl: EventTarget | null | undefined, redirectHref?: string | null) {
  if (!loginWarning.value) return;
  // Consume the pointerdown snapshot so a later programmatic call (no pointerdown,
  // e.g. gating on navigation) still shows instead of inheriting a stale flag.
  const wasOpen = loginWarningWasOpen;
  loginWarningWasOpen = false;
  // Re-tap on the trigger that opened it: close instead of reopening. hide() is a
  // no-op if native light-dismiss already closed it on pointerdown.
  if (wasOpen) {
    loginWarning.value.hide();
    return;
  }
  loginRedirect.value = redirectHref ?? editorTabHref.value;
  // Callers pass the clicked element (an event's currentTarget); assigned as is.
  loginWarning.value.anchorElement = anchorEl as Element | null;
  loginWarning.value.show();
}
provide('showLoginWarning', showLoginWarning);

// Secondary action on the login popover: to the public "Account aanvragen"
// page. Close the popover first so it isn't left hanging over the new page.
const accountRequestHref = computed(() => router.resolve({ name: 'account-aanvragen' }).href);
function goToAccountRequest() {
  loginWarning.value?.hide();
  router.push({ name: 'account-aanvragen' });
}

function onEditorTab(e: Event) {
  if (!authenticated.value) {
    showLoginWarning(e.currentTarget);
    return;
  }
  if (isLibraryRoute.value) router.push(editorTabTarget.value);
}

// View-specific toolbar bits published by the active view.
const { lastSavedPr, documentTabs, activeDocumentTab, documentTabsTrajectRef, tabActions, editorChanges, editorActions, libraryEmpty } = useAppChrome();

// The toolbars' overflow-menu alternatives. A `select` there arrives on the
// original menu item, which sits hidden in its collapsed toolbar item, so a
// popover these open anchors to the toolbar instead of to that item.
function overflowAnchor(e: Event): Element | null {
  return (e.currentTarget as Element | null)?.closest?.('nldd-toolbar') ?? null;
}
function onEditorTabOverflow(e: Event) {
  if (!authenticated.value) {
    showLoginWarning(overflowAnchor(e));
    return;
  }
  if (isLibraryRoute.value) router.push(editorTabTarget.value);
}
function openSearchFromOverflow(e: Event) {
  openSearchAt(overflowAnchor(e));
}
// A menu item has no `target`; the PR opens in a new tab like its button, so
// the editor state is not lost.
function openLastSavedPr() {
  if (lastSavedPr.value) window.open(lastSavedPr.value.url, '_blank', 'noopener');
}

// Just-in-time coach-mark on the toolbar search affordance: shown while the
// library is empty (nothing curated yet). In the bare corpus it's app-driven and
// non-dismissable (it disappears once there's content); inside a traject it's
// dismissable, since there are other functions to discover. Each breakpoint
// renders its search control in a different bar (sm icon-button, md text button,
// lg search field), each in a pane that is display:none off-breakpoint. So we
// resolve the active breakpoint and activate only the coach-mark whose control
// is actually visible, never anchoring a popover to a hidden control.
const viewport = ref('lg'); // 'sm' | 'md' | 'lg', aligned with the DS bar breakpoints
let mdQuery: MediaQueryList | null = null;
let lgQuery: MediaQueryList | null = null;
// DS bar breakpoints, mirrored here for matchMedia. Keep in sync with
// @nldd/design-system (src/assets/styles/breakpoints.ts): md >= 641px, lg >= 1008px.
// If the DS shifts these thresholds, update them here too - otherwise the
// coach-mark can anchor to a control that is hidden at the current breakpoint.
const DS_MD_MIN = '(min-width: 641px)';
const DS_LG_MIN = '(min-width: 1008px)';
function updateViewport() {
  viewport.value = lgQuery?.matches ? 'lg' : mdQuery?.matches ? 'md' : 'sm';
}
onMounted(() => {
  mdQuery = window.matchMedia?.(DS_MD_MIN) || null;
  lgQuery = window.matchMedia?.(DS_LG_MIN) || null;
  updateViewport();
  mdQuery?.addEventListener?.('change', updateViewport);
  lgQuery?.addEventListener?.('change', updateViewport);
});
onBeforeUnmount(() => {
  mdQuery?.removeEventListener?.('change', updateViewport);
  lgQuery?.removeEventListener?.('change', updateViewport);
});
// Inside a traject the coach-mark is dismissable (other functions to discover -
// Instellingen, Werkdocumenten): the dismiss button persists so it won't nag
// again; a click outside hides it for the session. In the bare corpus it stays
// non-dismissable until content appears.
const JIT_DISMISS_KEY = 'regelrecht:jit-traject-search-dismissed';
function loadJitDismissed() {
  try { return localStorage.getItem(JIT_DISMISS_KEY) === '1'; } catch { return false; }
}
const trajectActive = computed(() => !!activeTrajectRef.value);
const jitDismissed = ref(loadJitDismissed());
const jitHiddenSession = ref(false);
const searchHintActive = computed(
  () => libraryEmpty.value && !(trajectActive.value && (jitDismissed.value || jitHiddenSession.value)),
);
const showSearchHintSm = computed(() => searchHintActive.value && viewport.value === 'sm');
const showSearchHintMd = computed(() => searchHintActive.value && viewport.value === 'md');
const showSearchHintLg = computed(() => searchHintActive.value && viewport.value === 'lg');
function onSearchHintClose(e: Event) {
  jitHiddenSession.value = true;
  if ((e as CustomEvent<{ reason?: string } | null> | undefined)?.detail?.reason === 'dismissed') {
    jitDismissed.value = true;
    try { localStorage.setItem(JIT_DISMISS_KEY, '1'); } catch { /* ignore */ }
  }
}

// Editor with open tabs → the mobile traject row splits 50/50 to fit a tabs
// button next to the traject menu, and the md+ document-tab-bar shows. The
// library never publishes tabs, so its mobile row keeps the full-width traject
// menu (the two sections are intentionally decoupled here).
const hasDocumentTabs = computed(
  () => documentTabs.value.length > 0 && !!tabActions.value,
);

// Which tab replaces a dismissed one is the bar's call, not ours: its dismiss
// handler picks the neighbour (right, else left, skipping tabs overflow has
// hidden - layout state only it can see), marks that item selected itself, and
// reports it as `nextItem` here. Inventing a second policy alongside it selects
// two tabs: the bar writes `selected` straight onto its own element, which Vue
// never learns about, so its pick stays lit next to ours.
//
// Map elements back to tabs by data-tab-key rather than by index - the bar
// reorders its own DOM to keep the selected tab visible when tabs overflow.
// The bar's `tabdismiss` detail: the dismissed item and the one it selected.
interface TabDismissDetail {
  item?: HTMLElement | null;
  nextItem?: HTMLElement | null;
}

async function onTabDismiss(e: Event) {
  const actions = tabActions.value;
  if (!actions) return;
  const detail = (e as CustomEvent<TabDismissDetail | null | undefined>).detail;
  const toTab = (el: HTMLElement | null | undefined) =>
    documentTabs.value.find((t) => actions.key(t) === el?.dataset?.tabKey) ?? null;
  const dismissed = toTab(detail?.item);
  if (dismissed && !(await actions.close(dismissed, toTab(detail?.nextItem)))) restoreTabSelection();
}

async function onTabClick(tab: OpenTab) {
  const actions = tabActions.value;
  if (actions && !(await actions.select(tab))) restoreTabSelection();
}

// The bar marks a clicked tab, or a closed tab's neighbour, as selected before
// the editor answers. When the editor keeps the open article (the user chose
// to keep unsaved edits), put the selection back on it. The bar keeps no
// selection of its own beyond each item's `selected`, and Vue does not patch
// a binding whose value did not change, so this sets it directly.
const tabBarEl = ref<HTMLElement | null>(null);
function restoreTabSelection() {
  const actions = tabActions.value;
  const active = activeDocumentTab.value;
  const activeKey = actions && active ? actions.key(active) : null;
  tabBarEl.value
    ?.querySelectorAll<HTMLElement & { selected: boolean }>('nldd-document-tab-bar-item')
    .forEach((item) => {
      item.selected = item.dataset.tabKey === activeKey;
    });
}
</script>

<template>
  <nldd-shell>
    <nldd-bar-split-view>
      <!-- Primary Bar: md only - search and settings as buttons. The bar-split-
           view draws the divider automatically where the bar group meets main:
           on the library it sits under this bar; on the editor the document-tab-
           bar sits between, so toolbar + tabs read as one group above the single
           main divider. -->
      <nldd-split-view-pane slot="primary-bar-md" only="md">
        <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar size="md" navigation accessible-label="Onderdelen">
                <nldd-tab-bar-item :current="isLibraryRoute || undefined" :href="isLibraryRoute ? undefined : libraryTabHref" @click.prevent="isLibraryRoute || router.push(libraryTabTarget)" text="Home"></nldd-tab-bar-item>
                <nldd-tab-bar-item :current="!isLibraryRoute || undefined" :href="authenticated && isLibraryRoute ? editorTabHref : undefined" @click.prevent="onEditorTab" @pointerdown.capture="onLoginTriggerPointerdown" text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
              <nldd-menu-group slot="overflow" text="Onderdelen">
                <nldd-menu-item type="radio" text="Home" :selected="isLibraryRoute || undefined" @select="isLibraryRoute || router.push(libraryTabTarget)"></nldd-menu-item>
                <nldd-menu-item type="radio" text="Editor" :selected="!isLibraryRoute || undefined" @select="onEditorTabOverflow"></nldd-menu-item>
              </nldd-menu-group>
            </nldd-toolbar-item>
            <nldd-toolbar-item v-if="lastSavedPr" slot="end">
              <!-- Federated write-back indicator (editor only). New tab so the
                   editor state isn't lost. -->
              <nldd-button size="md" start-icon="external-link" :text="`PR #${lastSavedPr.number}`" :href="lastSavedPr.url" target="_blank" rel="noopener"></nldd-button>
              <nldd-menu-item slot="overflow" icon="external-link" :text="`PR #${lastSavedPr.number}`" @select="openLastSavedPr"></nldd-menu-item>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-just-in-time-education
                placement="bottom"
                arrow-length="160px"
                text="Zoek een regelwerk om te openen"
                supporting-text="Markeer een regelwerk als favoriet om het later snel terug te vinden."
                :active="showSearchHintMd || undefined"
                :dismissable="trajectActive || undefined"
                @nldd-close="onSearchHintClose"
              >
                <nldd-button data-search-trigger size="md" start-icon="search" text="Zoeken" @click="openSearch"></nldd-button>
              </nldd-just-in-time-education>
              <nldd-menu-item slot="overflow" icon="search" text="Zoeken" @select="openSearchFromOverflow"></nldd-menu-item>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end" v-if="trajectActive || (!authLoading && oidcConfigured && !authenticated)">
              <nldd-icon-button size="md" icon="plus-small" text="Nieuw" tooltip-timing="never" expandable>
                <nldd-menu v-if="trajectActive" slot="popup">
                  <NewMenuItems />
                </nldd-menu>
                <!-- Niet ingelogd: dezelfde "+" blijft staan als ontdekpunt, maar
                     opent een popover die uitnodigt in te loggen. Zelfde id als het
                     menu, zodat de knop-popovertarget statisch blijft (patroon van
                     de trajecten-knop). -->
                <nldd-popover v-else slot="popup" accessible-label="Toevoegen" width="320px">
                  <nldd-container padding="16">
                    <nldd-inline-dialog
                      icon="login"
                      text="Log in om iets toe te voegen"
                      supporting-text="Zodra je bent ingelogd kun je regelwerken, werkdocumenten en leden aan een traject toevoegen."
                    >
                      <nldd-button slot="actions" appearance="primary" text="Inloggen" @click="login()"></nldd-button>
                      <nldd-button slot="actions" appearance="secondary" text="Account aanvragen" :href="accountRequestHref" @click.prevent="goToAccountRequest"></nldd-button>
                    </nldd-inline-dialog>
                  </nldd-container>
                </nldd-popover>
              </nldd-icon-button>
              <nldd-menu-item slot="overflow" icon="plus-small" text="Nieuw">
                <nldd-menu v-if="trajectActive">
                  <NewMenuItems />
                </nldd-menu>
                <!-- Logged out: what the button's popover offers, as a menu. -->
                <nldd-menu v-else>
                  <nldd-menu-item icon="login" text="Inloggen" @select="login()"></nldd-menu-item>
                  <nldd-menu-item icon="new-account" text="Account aanvragen" @select="goToAccountRequest"></nldd-menu-item>
                </nldd-menu>
              </nldd-menu-item>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button-bar size="md">
                <TrajectMenu />
                <nldd-button-bar-divider></nldd-button-bar-divider>
                <nldd-icon-button size="md" :icon="authenticated ? undefined : 'account'" text="Account" tooltip-timing="never" expandable>
                  <nldd-avatar v-if="authenticated" slot="icon" :name="person?.name || person?.email" color="inherit" icon-aligned decorative></nldd-avatar>
                <nldd-menu slot="popup">
                  <AccountMenuItems @settings="openSettings" @about="openAbout" @support="openSupport" @request-account="goToAccountRequest" />
                </nldd-menu>
                </nldd-icon-button>
              </nldd-button-bar>
              <TrajectMenu slot="overflow" menu-item />
              <nldd-menu-item slot="overflow" icon="account" text="Account">
                <nldd-menu>
                  <AccountMenuItems @settings="openSettings" @about="openAbout" @support="openSupport" @request-account="goToAccountRequest" />
                </nldd-menu>
              </nldd-menu-item>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Primary Bar: lg+ - search as input field in center slot -->
      <nldd-split-view-pane slot="primary-bar-lg" above="lg">
        <nldd-container padding="8">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar size="md" navigation accessible-label="Onderdelen">
                <nldd-tab-bar-item :current="isLibraryRoute || undefined" :href="isLibraryRoute ? undefined : libraryTabHref" @click.prevent="isLibraryRoute || router.push(libraryTabTarget)" text="Home"></nldd-tab-bar-item>
                <nldd-tab-bar-item :current="!isLibraryRoute || undefined" :href="authenticated && isLibraryRoute ? editorTabHref : undefined" @click.prevent="onEditorTab" @pointerdown.capture="onLoginTriggerPointerdown" text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
              <nldd-menu-group slot="overflow" text="Onderdelen">
                <nldd-menu-item type="radio" text="Home" :selected="isLibraryRoute || undefined" @select="isLibraryRoute || router.push(libraryTabTarget)"></nldd-menu-item>
                <nldd-menu-item type="radio" text="Editor" :selected="!isLibraryRoute || undefined" @select="onEditorTabOverflow"></nldd-menu-item>
              </nldd-menu-group>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="center" min-width="240px" width="33%" max-width="480px">
              <nldd-just-in-time-education
                placement="bottom"
                arrow-length="160px"
                text="Zoek een regelwerk om te openen"
                supporting-text="Markeer een regelwerk als favoriet om het later snel terug te vinden."
                :active="showSearchHintLg || undefined"
                :dismissable="trajectActive || undefined"
                @nldd-close="onSearchHintClose"
              >
                <nldd-search-field
                  data-search-trigger
                  size="md"
                  :placeholder="SEARCH_PLACEHOLDER"
                  :accessible-label="SEARCH_ACCESSIBLE_LABEL"
                  @click="openSearch"
                  @keydown="onBarSearchKeydown"
                ></nldd-search-field>
              </nldd-just-in-time-education>
              <nldd-menu-item slot="overflow" icon="search" text="Zoeken" @select="openSearchFromOverflow"></nldd-menu-item>
            </nldd-toolbar-item>
            <nldd-toolbar-item v-if="lastSavedPr" slot="end">
              <nldd-button size="md" start-icon="external-link" :text="`PR #${lastSavedPr.number}`" :href="lastSavedPr.url" target="_blank" rel="noopener"></nldd-button>
              <nldd-menu-item slot="overflow" icon="external-link" :text="`PR #${lastSavedPr.number}`" @select="openLastSavedPr"></nldd-menu-item>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end" v-if="trajectActive || (!authLoading && oidcConfigured && !authenticated)">
              <nldd-icon-button size="md" icon="plus-small" text="Nieuw" tooltip-timing="never" expandable>
                <nldd-menu v-if="trajectActive" slot="popup">
                  <NewMenuItems />
                </nldd-menu>
                <nldd-popover v-else slot="popup" accessible-label="Toevoegen" width="320px">
                  <nldd-container padding="16">
                    <nldd-inline-dialog
                      icon="login"
                      text="Log in om iets toe te voegen"
                      supporting-text="Zodra je bent ingelogd kun je regelwerken, werkdocumenten en leden aan een traject toevoegen."
                    >
                      <nldd-button slot="actions" appearance="primary" text="Inloggen" @click="login()"></nldd-button>
                      <nldd-button slot="actions" appearance="secondary" text="Account aanvragen" :href="accountRequestHref" @click.prevent="goToAccountRequest"></nldd-button>
                    </nldd-inline-dialog>
                  </nldd-container>
                </nldd-popover>
              </nldd-icon-button>
              <nldd-menu-item slot="overflow" icon="plus-small" text="Nieuw">
                <nldd-menu v-if="trajectActive">
                  <NewMenuItems />
                </nldd-menu>
                <!-- Logged out: what the button's popover offers, as a menu. -->
                <nldd-menu v-else>
                  <nldd-menu-item icon="login" text="Inloggen" @select="login()"></nldd-menu-item>
                  <nldd-menu-item icon="new-account" text="Account aanvragen" @select="goToAccountRequest"></nldd-menu-item>
                </nldd-menu>
              </nldd-menu-item>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-button-bar size="md">
                <TrajectMenu />
                <nldd-button-bar-divider></nldd-button-bar-divider>
                <nldd-icon-button size="md" :icon="authenticated ? undefined : 'account'" text="Account" tooltip-timing="never" expandable>
                  <nldd-avatar v-if="authenticated" slot="icon" :name="person?.name || person?.email" color="inherit" icon-aligned decorative></nldd-avatar>
                <nldd-menu slot="popup">
                  <AccountMenuItems @settings="openSettings" @about="openAbout" @support="openSupport" @request-account="goToAccountRequest" />
                </nldd-menu>
                </nldd-icon-button>
              </nldd-button-bar>
              <TrajectMenu slot="overflow" menu-item />
              <nldd-menu-item slot="overflow" icon="account" text="Account">
                <nldd-menu>
                  <AccountMenuItems @settings="openSettings" @about="openAbout" @support="openSupport" @request-account="goToAccountRequest" />
                </nldd-menu>
              </nldd-menu-item>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Document Tab Bar (editor only, md+). Hidden on sm - there the tabs
           live in the MobileTrajectSheet opened from the traject row. Rendered
           only while the active view publishes open tabs, so the library never
           shows an empty bar. -->
      <nldd-split-view-pane v-if="hasDocumentTabs" slot="document-tabs" above="md">
        <nldd-container padding-inline="8" padding-top="0" padding-bottom="8">
          <!-- Drag-reorder: the bar reorders its items visually and fires
               nldd-reorder; without this listener that move never reaches
               `openTabs`, so it was lost (reverted from localStorage) the next
               time the editor mounted. EditorView's `reorderTabs` mirrors the
               move into the array and persists it. -->
          <!-- `tabdismiss` (the bar), not `dismiss` (the item): the bar picks
               the replacement for a dismissed tab itself and hands it over as
               `nextItem`. See onTabDismiss. -->
          <!-- `:key` on the bar (the traject the tabs belong to, published WITH
               the tabs by the editor - see documentTabsTrajectRef): a traject
               switch rebuilds the whole bar, tearing down its overflow menu,
               ResizeObserver and every element reference it holds, so no
               orphaned <nldd-document-tab-bar-item> ("spooktab") can survive
               into the next traject. Keying on this shell's own activeTrajectRef
               would flip a tick before documentTabs and rebuild against the old
               set. The item :key is traject-prefixed for the same reason.
               No has-dismiss-button (the item renders its own dismiss button in
               0.8.44); accessible-label names the navigation landmark. -->
          <nldd-document-tab-bar
            ref="tabBarEl"
            :key="documentTabsTrajectRef ?? ''"
            accessible-label="Open artikelen"
            @nldd-reorder="tabActions?.reorder($event.detail.fromIndex, $event.detail.toIndex)"
            @tabdismiss="onTabDismiss"
          >
            <nldd-document-tab-bar-item
              v-for="tab in documentTabs"
              :key="`${documentTabsTrajectRef ?? ''}:${tabActions?.key(tab)}`"
              :data-tab-key="tabActions?.key(tab)"
              :text="`Artikel ${tab.articleNumber}`"
              :supporting-text="tabActions?.displayName(tab)"
              :short-text="`Art. ${tab.articleNumber}`"
              :short-supporting-text="tabActions?.displayName(tab)"
              :selected="activeDocumentTab && tabActions?.key(activeDocumentTab) === tabActions?.key(tab) || undefined"
              @click="onTabClick(tab)"
            >
            </nldd-document-tab-bar-item>
          </nldd-document-tab-bar>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Review-melding (editor only): direct onder de document-tab-bar, dus
           vóór `main` in de DOM. Eigen pane in de bar-split-view zodat hij niet
           met de panes meescrollt. Op sm bestaat de tab-bar niet (above="md"),
           daar landt hij dus vanzelf bovenaan. -->
      <nldd-split-view-pane v-if="reviewNotice" slot="review-notice">
        <nldd-container padding="8" padding-top="0">
          <nldd-banner
            size="sm"
            :variant="editorChanges?.reviewVariant || 'accent'"
            :text="reviewNotice"
          ></nldd-banner>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Main content area - the active section's body. -->
      <nldd-split-view-pane slot="main">
        <router-view />
      </nldd-split-view-pane>


      <!-- Wijzigingenbalk (editor only): one article-level bar with Opslaan +
           Wijzigingen-ongedaan (+ text undo/redo), replacing the per-pane save
           footers. Published by EditorView via useAppChrome; shown only while
           the article has unsaved changes. Sits after `main` in the DOM, so on
           sm it lands above the two mobile bars and on md+ it's the bottom bar. -->
      <nldd-split-view-pane v-if="editorChanges && (editorChanges.dirty || editorChanges.review)" slot="changes-bar">
        <nldd-container padding="8" sm-padding-bottom="0">
          <nldd-toolbar size="md" accessible-label="Wijzigingen">
            <!-- Undo en redo zijn losse knoppen: die gebruik je herhaald, en
                 dan is elke klik door een menu er één te veel. De discard is
                 destructief en houdt daarom z'n eigen chevron-knop, zodat hij
                 niet per ongeluk geraakt wordt naast de twee ernaast. -->
            <nldd-toolbar-item slot="start" label="Wijzigingen" :priority="1">
              <nldd-button-bar>
                <nldd-icon-button
                  icon="undo"
                  text="Ongedaan maken"
                  :disabled="!editorChanges.canUndo || undefined"
                  @click="editorActions?.undo?.()"
                ></nldd-icon-button>
                <nldd-button-bar-divider></nldd-button-bar-divider>
                <nldd-icon-button
                  icon="redo"
                  text="Opnieuw"
                  :disabled="!editorChanges.canRedo || undefined"
                  @click="editorActions?.redo?.()"
                ></nldd-icon-button>
                <nldd-button-bar-divider></nldd-button-bar-divider>
                <nldd-icon-button icon="chevron-down" text="Meer acties" tooltip-timing="never">
                  <nldd-menu slot="popup">
                    <nldd-menu-item
                      text="Maak alle wijzigingen ongedaan"
                      destructive
                      @select="editorActions?.discard?.()"
                    ></nldd-menu-item>
                    <nldd-menu-item
                      v-if="canRejectRest"
                      text="Rond af, neem de rest niet over"
                      destructive
                      @select="rejectRestConfirm?.show()"
                    ></nldd-menu-item>
                  </nldd-menu>
                </nldd-icon-button>
              </nldd-button-bar>
              <nldd-menu-item
                slot="overflow"
                icon="undo"
                text="Ongedaan maken"
                :disabled="!editorChanges.canUndo || undefined"
                @select="editorActions?.undo?.()"
              ></nldd-menu-item>
              <nldd-menu-item
                slot="overflow"
                icon="redo"
                text="Opnieuw"
                :disabled="!editorChanges.canRedo || undefined"
                @select="editorActions?.redo?.()"
              ></nldd-menu-item>
              <nldd-menu-item
                slot="overflow"
                text="Maak alle wijzigingen ongedaan"
                destructive
                @select="editorActions?.discard?.()"
              ></nldd-menu-item>
              <nldd-menu-item
                v-if="canRejectRest"
                slot="overflow"
                text="Rond af, neem de rest niet over"
                destructive
                @select="rejectRestConfirm?.show()"
              ></nldd-menu-item>
            </nldd-toolbar-item>
            <!-- Review-modus zet de beslissing hier. Twee losse toolbar-items,
                 geen button-bar: die plakt een destructieve knop tegen een
                 bevestigende aan, precies wat de ontwerprichtlijn ("geef ze
                 visuele en fysieke afstand") wil voorkomen. Verwerp heeft de
                 lagere prioriteit, dus die wijkt als eerste naar het
                 overloopmenu. -->
            <nldd-toolbar-item v-if="inReview" slot="end" label="Verwerp voorstel" :priority="2">
              <nldd-button
                appearance="destructive"
                size="md"
                start-icon="dismiss-circle"
                text="Verwerp voorstel"
                :disabled="editorChanges.saving || undefined"
                @click="rejectConfirm?.show()"
              ></nldd-button>
              <nldd-menu-item
                slot="overflow"
                icon="dismiss"
                text="Verwerp voorstel"
                destructive
                :disabled="editorChanges.saving || undefined"
                @select="rejectConfirm?.show()"
              ></nldd-menu-item>
            </nldd-toolbar-item>
            <!-- Buiten review staat Opslaan er alleen, en dan houdt hij zijn
                 eigen brede vlak: geen icoon nodig om zich van een buurknop te
                 onderscheiden. In review staat Verwerp ernaast, en daar doen de
                 iconen het onderscheidende werk op eigen breedte. -->
            <nldd-toolbar-item
              slot="end"
              :label="saveLabel"
              :width="inReview ? undefined : '320px'"
              :priority="3"
            >
              <nldd-button
                appearance="primary"
                size="md"
                :start-icon="inReview ? 'check-mark-circle' : undefined"
                :width="inReview ? undefined : 'full'"
                :text="saveLabel"
                :loading="editorChanges.saving || undefined"
                @click="editorActions?.save?.()"
              ></nldd-button>
              <nldd-menu-item
                slot="overflow"
                icon="save"
                :text="saveLabel"
                :disabled="editorChanges.saving || undefined"
                @select="editorActions?.save?.()"
              ></nldd-menu-item>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>

      <!-- Mobile Bar (sm only): the combined MobileTrajectSheet button on top
           (traject + open articles in one sheet), then the tab bar + search +
           account row below. -->
      <nldd-split-view-pane slot="mobile-bar" only="sm">
        <nldd-container padding="8" padding-bottom="0">
          <nldd-toolbar size="md">
            <nldd-toolbar-item slot="start" width="100%">
              <MobileTrajectSheet />
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
        <nldd-container padding="8">
          <nldd-toolbar size="lg">
            <nldd-toolbar-item slot="start">
              <nldd-tab-bar navigation accessible-label="Onderdelen">
                <nldd-tab-bar-item :current="isLibraryRoute || undefined" :href="isLibraryRoute ? undefined : libraryTabHref" @click.prevent="isLibraryRoute || router.push(libraryTabTarget)" icon="home" text="Home"></nldd-tab-bar-item>
                <nldd-tab-bar-item :current="!isLibraryRoute || undefined" :href="authenticated && isLibraryRoute ? editorTabHref : undefined" @click.prevent="onEditorTab" @pointerdown.capture="onLoginTriggerPointerdown" icon="edit" text="Editor"></nldd-tab-bar-item>
              </nldd-tab-bar>
              <nldd-menu-group slot="overflow" text="Onderdelen">
                <nldd-menu-item type="radio" icon="home" text="Home" :selected="isLibraryRoute || undefined" @select="isLibraryRoute || router.push(libraryTabTarget)"></nldd-menu-item>
                <nldd-menu-item type="radio" icon="edit" text="Editor" :selected="!isLibraryRoute || undefined" @select="onEditorTabOverflow"></nldd-menu-item>
              </nldd-menu-group>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-just-in-time-education
                placement="top"
                arrow-length="160px"
                text="Zoek een regelwerk om te openen"
                supporting-text="Markeer een regelwerk als favoriet om het later snel terug te vinden."
                :active="showSearchHintSm || undefined"
                :dismissable="trajectActive || undefined"
                @nldd-close="onSearchHintClose"
              >
                <nldd-icon-button data-search-trigger size="lg" icon="search" text="Zoeken" @click="openSearch"></nldd-icon-button>
              </nldd-just-in-time-education>
              <nldd-menu-item slot="overflow" icon="search" text="Zoeken" @select="openSearchFromOverflow"></nldd-menu-item>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end" v-if="trajectActive || (!authLoading && oidcConfigured && !authenticated)">
              <nldd-icon-button size="lg" icon="plus-small" text="Nieuw" tooltip-timing="never">
                <nldd-menu v-if="trajectActive" slot="popup">
                  <NewMenuItems />
                </nldd-menu>
                <nldd-popover v-else slot="popup" accessible-label="Toevoegen" width="320px">
                  <nldd-container padding="16">
                    <nldd-inline-dialog
                      icon="login"
                      text="Log in om iets toe te voegen"
                      supporting-text="Zodra je bent ingelogd kun je regelwerken, werkdocumenten en leden aan een traject toevoegen."
                    >
                      <nldd-button slot="actions" appearance="primary" text="Inloggen" @click="login()"></nldd-button>
                      <nldd-button slot="actions" appearance="secondary" text="Account aanvragen" :href="accountRequestHref" @click.prevent="goToAccountRequest"></nldd-button>
                    </nldd-inline-dialog>
                  </nldd-container>
                </nldd-popover>
              </nldd-icon-button>
              <nldd-menu-item slot="overflow" icon="plus-small" text="Nieuw">
                <nldd-menu v-if="trajectActive">
                  <NewMenuItems />
                </nldd-menu>
                <!-- Logged out: what the button's popover offers, as a menu. -->
                <nldd-menu v-else>
                  <nldd-menu-item icon="login" text="Inloggen" @select="login()"></nldd-menu-item>
                  <nldd-menu-item icon="new-account" text="Account aanvragen" @select="goToAccountRequest"></nldd-menu-item>
                </nldd-menu>
              </nldd-menu-item>
            </nldd-toolbar-item>
            <nldd-toolbar-item slot="end">
              <nldd-icon-button size="lg" :icon="authenticated ? undefined : 'account'" text="Account" tooltip-timing="never">
                <nldd-avatar v-if="authenticated" slot="icon" :name="person?.name || person?.email" color="inherit" icon-aligned decorative></nldd-avatar>
                <nldd-menu slot="popup">
                  <AccountMenuItems @settings="openSettings" @about="openAbout" @support="openSupport" @request-account="goToAccountRequest" />
                </nldd-menu>
              </nldd-icon-button>
              <nldd-menu-item slot="overflow" icon="account" text="Account">
                <nldd-menu>
                  <AccountMenuItems @settings="openSettings" @about="openAbout" @support="openSupport" @request-account="goToAccountRequest" />
                </nldd-menu>
              </nldd-menu-item>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>
    </nldd-bar-split-view>

    <AboutSheet ref="aboutSheet"></AboutSheet>
    <SupportSheet ref="supportSheet"></SupportSheet>
    <SettingsSheet ref="settingsSheet"></SettingsSheet>
  </nldd-shell>

  <!-- Editor requires login: a heads-up popover anchored to the clicked Editor
       tab (sm/md/lg) so the SSO screen never appears unannounced. -->
  <nldd-popover ref="loginWarning" accessible-label="Inloggen" width="320px">
    <nldd-container padding="16">
      <nldd-inline-dialog
        icon="login"
        text="Log in om de editor te gebruiken"
        supporting-text="Zodra je bent ingelogd kies je een traject en kun je aan de slag."
      >
        <nldd-button slot="actions" appearance="primary" text="Inloggen" @click="login(loginRedirect || editorTabHref)"></nldd-button>
        <nldd-button slot="actions" appearance="secondary" text="Account aanvragen" :href="accountRequestHref" @click.prevent="goToAccountRequest"></nldd-button>
      </nldd-inline-dialog>
    </nldd-container>
  </nldd-popover>

  <nldd-modal-dialog
    ref="rejectConfirm"
    variant="alert"
    icon="exclamation-triangle"
    text="Voorstel verwerpen?"
    supporting-text="De taak wordt afgesloten en het gegenereerde voorstel gaat verloren. Een nieuw voorstel vraag je opnieuw aan met Verrijk dit regelwerk."
  >
    <nldd-button
      slot="actions"
      appearance="primary"
      text="Behoud voorstel"
      @click="rejectConfirm?.hide()"
    ></nldd-button>
    <nldd-button
      slot="actions"
      appearance="destructive"
      text="Verwerp voorstel"
      @click="confirmReject"
    ></nldd-button>
  </nldd-modal-dialog>

  <nldd-modal-dialog
    ref="rejectRestConfirm"
    variant="alert"
    icon="exclamation-triangle"
    text="Verrijking afronden?"
    supporting-text="Wat je al hebt overgenomen wordt weggeschreven; alle onderdelen die je nog niet hebt beoordeeld worden niet overgenomen en gaan verloren."
  >
    <nldd-button
      slot="actions"
      appearance="primary"
      text="Verder beoordelen"
      @click="rejectRestConfirm?.hide()"
    ></nldd-button>
    <nldd-button
      slot="actions"
      appearance="destructive"
      text="Rond af"
      @click="confirmRejectRest"
    ></nldd-button>
  </nldd-modal-dialog>
</template>
