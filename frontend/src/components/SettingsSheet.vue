<script setup lang="ts">
// "Instellingen" — the account settings sheet, opened from the account menu.
//
// Collects what used to be submenus of that menu (Weergave, Functies) plus the
// GitHub link. The link is a connection, not an action: it belongs with the
// other account state rather than between Help and Log uit, where its label
// also changed identity depending on whether you were connected.
//
// Settings are not a form — nothing is submitted, every control applies at
// once — so these are list rows under section titles, not fields in a
// fieldset.
//
// The composables backing this are singletons, so the sheet reads them
// directly instead of taking the whole set as props.
import { computed, ref } from 'vue';
import type { NLDDModalDialog } from '@nldd/design-system/modal-dialog';
import type { NLDDPopover } from '@nldd/design-system/popover';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import type { NLDDSwitch } from '@nldd/design-system/switch';
import { useFeatureFlags } from '../composables/useFeatureFlags';
import { useGithubAuth } from '../composables/useGithubAuth';
import { useSettingsSections } from '../composables/useSettingsSections';

const sheetEl = ref<NLDDSheet | null>(null);

function show(): void {
  sheetEl.value?.show?.();
}
function hide(): void {
  sheetEl.value?.hide?.();
}

defineExpose({ show, hide });

const { isEnabled, toggle: toggleFlag } = useFeatureFlags();
const {
  status: githubStatus,
  connect: connectGithub,
  disconnect: disconnectGithub,
} = useGithubAuth();

const editorPanelFlags: [key: string, label: string][] = [
  ['panel.article_text', 'Tekst editor'],
  ['panel.machine_readable', 'Machine editor'],
  ['panel.scenario_form', 'Scenario editor'],
  ['panel.yaml_editor', 'YAML editor'],
  ['panel.notes', 'Notities'],
  ['panel.regelrijm', 'RegelRijm (experimenteel)'],
];

// De sectie-poorten staan in useSettingsSections, want AppShell stelt dezelfde
// vraag om te bepalen of het menu-item "Instellingen" er hoort te staan.
const { signedIn, showBeheer: isAdmin, showKoppelingen } = useSettingsSections();
// Alleen "is er hier een GitHub-app geconfigureerd", zonder de required-helft:
// de schakelaar in Beheer is juist wat `required` aan- of uitzet.
const githubConfigured = computed(() => signedIn.value && !!githubStatus.value?.configured);
// Two different questions, deliberately not the same computed:
//   `required` — does the write path actually demand a personal token? The
//     backend folds the GITHUB_USER_TOKEN_REQUIRED env var AND the feature flag
//     into this one field, and the env var wins. Use it wherever the UI
//     describes behaviour.
//   `enforced` — is the feature flag on? That is the only half an admin can
//     change here, so the switch binds to this.
// They differ when the deployment forces it through config; then the switch
// would be a control that changes nothing, so it makes way for a plain line.
const githubRequired = computed(() => !!githubStatus.value?.required);
const githubEnforced = computed(() => isEnabled('github.user_oauth'));
const githubForcedByConfig = computed(() => githubRequired.value && !githubEnforced.value);
const githubConnected = computed(() => !!githubStatus.value?.connected);

function onPanelFlagChange(key: string, event: Event): void {
  // Follow the store rather than the switch's own DOM state.
  if ((event as CustomEvent<{ checked: boolean }>).detail.checked !== isEnabled(key)) toggleFlag(key);
}

// Enabling `github.user_oauth` is not a personal display preference like the
// panel flags: the flag is deployment-wide AND doubles as the backend's
// write-enforcement switch (`write_requires_user_token`), so turning it on
// makes every editor-writer's next traject save require a linked personal
// GitHub account (an unlinked user's save 428s into the connect flow).
// Intercept the enable with an explicit confirmation. Disabling restores the
// pre-existing service-token behaviour and stays a plain toggle.
const enforcementConfirm = ref<NLDDPopover | null>(null);
const enforcementSwitch = ref<NLDDSwitch | null>(null);

function onEnforcementChange(event: Event): void {
  // The change event comes from the nldd-switch itself.
  const target = event.target as NLDDSwitch;
  if (!isEnabled('github.user_oauth')) {
    // Revert the switch: it flipped itself, and the flag only actually turns
    // on once the confirmation is accepted.
    target.checked = false;
    if (enforcementConfirm.value) {
      enforcementConfirm.value.anchorElement = enforcementSwitch.value || target;
      enforcementConfirm.value.show();
    }
    return;
  }
  toggleFlag('github.user_oauth');
}

function confirmUserOauthEnforcement(): void {
  enforcementConfirm.value?.hide();
  toggleFlag('github.user_oauth');
}

// Release the anchor when the popover closes (confirm, cancel, or light
// dismiss): nldd-popover toggles itself on every subsequent click on its
// anchor element, so a stale anchor makes the switch reopen it.
function onEnforcementConfirmClose(): void {
  if (enforcementConfirm.value) enforcementConfirm.value.anchorElement = null;
}

// Disconnecting revokes the token at GitHub and, with the flag on, makes the
// next traject save fail with a 428 — so it asks first instead of firing on
// the click.
const disconnectConfirm = ref<NLDDModalDialog | null>(null);

function confirmDisconnect(): void {
  disconnectConfirm.value?.hide();
  disconnectGithub();
}
</script>

<template>
  <Teleport to="body">
    <nldd-sheet ref="sheetEl" placement="right" width="480px" full-height @close="hide">
      <nldd-page sticky-header>
        <nldd-top-title-bar
          slot="header"
          text="Instellingen"
          dismiss-text="Sluit"
          @dismiss="hide"
        ></nldd-top-title-bar>

        <nldd-simple-section width="full">

          <template v-if="showKoppelingen">
            <nldd-spacer size="24"></nldd-spacer>
            <nldd-title size="5">
              <h2>Koppelingen</h2>
            </nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-list appearance="box-tinted">
              <nldd-list-item>
                <nldd-text-cell
                  text="GitHub"
                  :supporting-text="githubConnected ? githubStatus?.github_login : 'Niet gekoppeld'"
                ></nldd-text-cell>
                <nldd-cell>
                  <nldd-button
                    v-if="githubConnected"
                    appearance="destructive"
                    text="Ontkoppelen"
                    @click="disconnectConfirm?.show()"
                  ></nldd-button>
                  <nldd-button
                    v-else
                    appearance="primary"
                    text="Koppelen"
                    end-icon="external-link"
                    @click="connectGithub()"
                  ></nldd-button>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </template>

          <template v-if="isAdmin">
            <nldd-spacer size="24"></nldd-spacer>
            <nldd-title size="5">
              <h2>Beheer</h2>
              <span slot="supporting-text">Geldt voor alle gebruikers en trajecten in deze installatie.</span>
            </nldd-title>
            <nldd-spacer size="8"></nldd-spacer>
            <nldd-list appearance="box-tinted">
              <nldd-list-item v-for="[key, label] in editorPanelFlags" :key="key">
                <nldd-text-cell :text="label"></nldd-text-cell>
                <nldd-cell>
                  <nldd-switch
                    :accessible-label="label"
                    :checked="isEnabled(key) || undefined"
                    @change="onPanelFlagChange(key, $event)"
                  ></nldd-switch>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item v-if="githubConfigured">
                <nldd-text-cell
                  text="Met eigen GitHub-account schrijven"
                  :supporting-text="githubForcedByConfig
                    ? 'Aan, vastgezet in de configuratie van deze omgeving.'
                    : 'Uit: RegelRecht schrijft met één gedeeld account.'"
                ></nldd-text-cell>
                <nldd-cell v-if="!githubForcedByConfig">
                  <nldd-switch
                    ref="enforcementSwitch"
                    accessible-label="Met eigen GitHub-account schrijven"
                    :checked="githubEnforced || undefined"
                    @change="onEnforcementChange"
                  ></nldd-switch>
                </nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </template>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>

    <nldd-popover
      ref="enforcementConfirm"
      accessible-label="GitHub-koppeling inschakelen"
      width="360px"
      @close="onEnforcementConfirmClose"
    >
      <nldd-container padding="16">
        <nldd-inline-dialog
          icon="exclamation-triangle"
          text="GitHub-koppeling voor iedereen inschakelen?"
          supporting-text="Dit geldt voor de hele omgeving, niet alleen voor jou: opslaan in een traject vereist daarna voor elke gebruiker een gekoppeld GitHub-account. Wie nog niet gekoppeld heeft, wordt bij de eerstvolgende opslag naar de koppel-flow geleid."
        >
          <nldd-button
            slot="actions"
            appearance="primary"
            text="Inschakelen"
            @click="confirmUserOauthEnforcement"
          ></nldd-button>
          <nldd-button
            slot="actions"
            text="Annuleren"
            @click="enforcementConfirm?.hide()"
          ></nldd-button>
        </nldd-inline-dialog>
      </nldd-container>
    </nldd-popover>

    <nldd-modal-dialog
      ref="disconnectConfirm"
      variant="alert"
      icon="exclamation-triangle"
      text="GitHub-account ontkoppelen?"
      supporting-text="Het token wordt ingetrokken bij GitHub. Zolang schrijven met een eigen account vereist is, mislukt je eerstvolgende opslag in een traject tot je opnieuw koppelt."
    >
      <nldd-button
        slot="actions"
        appearance="primary"
        text="Behoud koppeling"
        @click="disconnectConfirm?.hide()"
      ></nldd-button>
      <nldd-button
        slot="actions"
        appearance="destructive"
        text="Ontkoppel"
        @click="confirmDisconnect"
      ></nldd-button>
    </nldd-modal-dialog>
  </Teleport>
</template>
