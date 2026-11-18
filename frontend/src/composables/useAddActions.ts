import { ref } from 'vue';

/**
 * useAddActions - gedeelde open-triggers voor de universele "Toevoegen"-knop in
 * de header (AppShell). De knop leeft in de header, maar de sheets/pickers/panes
 * die hij opent leven in LibraryView (en de leden-pane daarin), een kind via
 * router-view. AppShell kan die niet direct aanroepen, dus vuurt het intenties
 * die LibraryView oppakt - hetzelfde ontkoppel-patroon als useNewHarvestJob.
 *
 * Tellers i.p.v. booleans: dezelfde actie twee keer moet ook twee keer vuren, en
 * een teller-wijziging is altijd een verandering die een watcher ziet.
 * `membersChanged` is het tegenovergestelde signaal: de InviteMembersSheet bumpt
 * het na een geslaagde invite zodat de ledenlijst (een andere useTrajectMembers-
 * instance, in de leden-pane) herlaadt.
 */
const addLaw = ref(0);
const newWerkdoc = ref(0);
const uploadWerkdoc = ref(0);
const inviteMembers = ref(0);
const membersChanged = ref(0);

export function useAddActions() {
  return {
    addLaw,
    newWerkdoc,
    uploadWerkdoc,
    inviteMembers,
    membersChanged,
    triggerAddLaw: () => { addLaw.value += 1; },
    triggerNewWerkdoc: () => { newWerkdoc.value += 1; },
    triggerUploadWerkdoc: () => { uploadWerkdoc.value += 1; },
    triggerInviteMembers: () => { inviteMembers.value += 1; },
    triggerMembersChanged: () => { membersChanged.value += 1; },
  };
}
