<script setup lang="ts">
/**
 * InviteMembersSheet - het uitnodig-formulier in een eigen sheet, losgemaakt uit
 * TrajectMembersPane zodat het overal te openen is: zowel via de "Uitnodigen"-
 * knop in de leden-pane als via de universele "+" in de header, zonder eerst naar
 * de leden-pagina te navigeren.
 *
 * `useTrajectMembers` is per-instance (verse refs), dus deze sheet heeft z'n eigen
 * kopie voor `inviteMany` + `trajectName`. De ledenlijst leeft in de pane, een
 * andere instance; na een geslaagde invite seint deze sheet dat via
 * useAddActions().triggerMembersChanged() zodat de pane herlaadt.
 */
import { computed, nextTick, ref, watch } from 'vue';
import type { NLDDSheet } from '@nldd/design-system/sheet';
import type { NLDDTokenField } from '@nldd/design-system/token-field';
import { useTrajectMembers } from '../composables/useTrajectMembers';
import type { FailedInvite } from '../composables/useTrajectMembers';
import { useAddActions } from '../composables/useAddActions';

const props = withDefaults(
  defineProps<{
    /** Traject to invite into (UUID id). */
    trajectId?: string | null;
  }>(),
  { trajectId: null },
);

/** The outcome of one invite batch, as inviteMany returns it. */
interface InviteResult {
  succeeded: string[];
  failed: FailedInvite[];
}

const { inviteMany, trajectName, load } = useTrajectMembers();
const { triggerMembersChanged } = useAddActions();

// De helptekst noemt de trajectnaam; die komt uit dezelfde load als de ledenlijst.
watch(() => props.trajectId, (id) => { if (id) load(id); }, { immediate: true });

const inviteSheetEl = ref<NLDDSheet | null>(null);
const inviteInputRef = ref<NLDDTokenField | null>(null);
const inviteEmails = ref<string[]>([]); // committed tokens
const invitePending = ref(''); // uncommitted input text
const inviteRole = ref('contributor');
const inviteBusy = ref(false);
const inviteError = ref<string | null>(null);
const inviteResult = ref<InviteResult | null>(null);

function onRoleChange(e: Event): void {
  const detail = (e as CustomEvent<{ selected: boolean; value: string } | null>).detail;
  if (detail?.selected) inviteRole.value = detail.value;
}

// Swap the form for the success view once every address in the batch went
// through. A partial batch keeps the form: the failures stay in the field so
// they can be corrected and retried, with the outcome shown inline.
const inviteSucceeded = computed(
  () =>
    !!inviteResult.value &&
    inviteResult.value.failed.length === 0 &&
    inviteResult.value.succeeded.length > 0,
);

// Reset the form to its empty state and re-seed the (async-mounted) token field.
// Shared by opening the sheet and by "Meer uitnodigen".
async function resetInviteForm(): Promise<void> {
  inviteEmails.value = [];
  invitePending.value = '';
  inviteRole.value = 'contributor';
  inviteError.value = null;
  inviteResult.value = null;
  await nextTick();
  if (inviteInputRef.value) inviteInputRef.value.values = [];
}

async function show(): Promise<void> {
  if (props.trajectId) load(props.trajectId);
  await resetInviteForm();
  inviteSheetEl.value?.show?.();
  await nextTick();
  inviteInputRef.value?.focus?.();
}
function closeInvite(): void {
  inviteSheetEl.value?.hide?.();
}

// "Meer uitnodigen" on the success view: swap it back for a fresh form.
async function inviteAgain(): Promise<void> {
  await resetInviteForm();
  await nextTick();
  inviteInputRef.value?.focus?.();
}

// Token-field handlers. Besides tracking the committed tokens / pending text, they
// clear the "voer een adres in" error as soon as the user acts, so the error and
// invalid outline disappear the moment they start correcting it.
function onEmailsChange(e: Event): void {
  inviteEmails.value = (e as CustomEvent<{ values?: string[] } | null>).detail?.values ?? inviteEmails.value;
  if (inviteError.value) inviteError.value = null;
}
function onEmailsInput(e: Event): void {
  invitePending.value = (e as CustomEvent<{ value?: string } | null>).detail?.value ?? invitePending.value;
  if (inviteError.value) inviteError.value = null;
}

async function submitInvite(): Promise<void> {
  // The traject can disappear while the sheet is open; then there is nothing to invite into.
  const trajectId = props.trajectId;
  if (!trajectId) return;
  inviteError.value = null;
  inviteResult.value = null;
  // Include the current (uncommitted) input text alongside the committed tokens,
  // so a last address typed without pressing Enter isn't silently dropped.
  const raw = [...inviteEmails.value];
  const pending = invitePending.value.trim();
  if (pending) raw.push(pending);
  const emails = [...new Set(raw.map((e) => e.trim()).filter(Boolean))];
  if (emails.length === 0) {
    inviteError.value = 'Voer minstens één e-mailadres in';
    await nextTick();
    inviteInputRef.value?.focus?.();
    return;
  }
  inviteBusy.value = true;
  try {
    const { succeeded, failed } = await inviteMany(trajectId, emails, inviteRole.value);
    inviteResult.value = { succeeded, failed };
    // Elke geslaagde invite maakt een openstaande uitnodiging aan; sein de
    // ledenlijst (andere instance, in de pane) dat er iets veranderd is.
    if (succeeded.length > 0) triggerMembersChanged();
    // Keep only the failures in the field so the user can fix and retry.
    inviteEmails.value = failed.map((f) => f.email);
    invitePending.value = '';
    if (inviteInputRef.value) inviteInputRef.value.values = inviteEmails.value;
  } catch (e) {
    inviteError.value = (e as Error).message || 'Uitnodigen mislukt';
  } finally {
    inviteBusy.value = false;
  }
}

defineExpose({ show });
</script>

<template>
  <Teleport to="body">
    <nldd-sheet ref="inviteSheetEl" placement="right" width="480px" @close="closeInvite">
      <nldd-page sticky-header sticky-footer>
        <nldd-top-title-bar slot="header" text="Uitnodigen" dismiss-text="Annuleer" @dismiss="closeInvite"></nldd-top-title-bar>
        <nldd-simple-section width="full">
          <!-- Success view: a new view in the sheet, shown once the whole batch was sent.
               (inviteSucceeded implies inviteResult is set, hence the `!`.) -->
          <nldd-inline-dialog
            v-if="inviteSucceeded"
            variant="success"
            :text="`${inviteResult!.succeeded.length} uitnodiging${inviteResult!.succeeded.length === 1 ? '' : 'en'} verstuurd`"
            supporting-text="Toegang wordt actief bij de eerste login."
          >
            <nldd-button slot="actions" appearance="primary" size="md" width="full" text="Sluit" @click="closeInvite"></nldd-button>
            <nldd-button slot="actions" appearance="secondary" size="md" width="full" text="Meer uitnodigen" @click="inviteAgain"></nldd-button>
          </nldd-inline-dialog>

          <nldd-form v-else>
            <form novalidate @submit.prevent="submitInvite">
              <nldd-form-field label="E-mailadressen">
                <nldd-token-field
                  ref="inviteInputRef"
                  allow-custom
                  type="email"
                  name="emails"
                  accessible-label="E-mailadressen"
                  placeholder="Toevoegen..."
                  :invalid="inviteError ? true : undefined"
                  :unmet="inviteError ? 'invite-email-error' : undefined"
                  @change="onEmailsChange"
                  @input="onEmailsInput"
                ></nldd-token-field>
                <nldd-form-field-help-text>
                  Typ een adres en gebruik een komma om nog een adres in te voeren. Toegang tot '{{ trajectName }}' wordt actief bij de eerste login.
                </nldd-form-field-help-text>
                <nldd-validation-list>
                  <nldd-validation-item id="invite-email-error">
                    {{ inviteError }}
                  </nldd-validation-item>
                </nldd-validation-list>
              </nldd-form-field>

              <nldd-form-field label="Rol">
                <nldd-toggle-button-group type="radio" size="md" accessible-label="Rol" @change="onRoleChange">
                  <nldd-toggle-button value="contributor" text="Bijdrager" :selected="inviteRole === 'contributor' || undefined"></nldd-toggle-button>
                  <nldd-toggle-button value="owner" text="Beheerder" :selected="inviteRole === 'owner' || undefined"></nldd-toggle-button>
                </nldd-toggle-button-group>
                <nldd-form-field-help-text>
                  Een bijdrager kan regelwerken en scenario's in dit traject bekijken en bewerken. Een beheerder kan daarnaast ook leden en instellingen van dit traject aanpassen.
                </nldd-form-field-help-text>
              </nldd-form-field>

              <!-- Partial batch (a full success shows the success view above instead). -->
              <template v-if="inviteResult">
                <nldd-spacer size="12"></nldd-spacer>
                <nldd-text v-if="inviteResult.succeeded.length" size="sm" color="secondary">
                  {{ inviteResult.succeeded.length }} uitnodiging{{ inviteResult.succeeded.length === 1 ? '' : 'en' }} verstuurd. Toegang wordt actief bij de eerste login.
                </nldd-text>
                <nldd-spacer v-if="inviteResult.succeeded.length && inviteResult.failed.length" size="4"></nldd-spacer>
                <nldd-text v-if="inviteResult.failed.length" size="sm" color="critical">
                  Mislukt:
                  <span v-for="(f, i) in inviteResult.failed" :key="f.email">{{ f.email }} ({{ f.message }}){{ i < inviteResult.failed.length - 1 ? ', ' : '' }}</span>
                </nldd-text>
                <nldd-spacer size="4"></nldd-spacer>
              </template>

              <nldd-form-actions>
                <nldd-button
                  appearance="primary"
                  size="md"
                  type="submit"
                  width="full"
                  :text="inviteBusy ? 'Bezig…' : 'Nodig uit'"
                  :disabled="inviteBusy || undefined"
                ></nldd-button>
              </nldd-form-actions>
            </form>
          </nldd-form>
        </nldd-simple-section>
      </nldd-page>
    </nldd-sheet>
  </Teleport>
</template>
