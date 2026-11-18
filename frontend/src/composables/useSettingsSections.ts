// Welke secties de instellingen-sheet toont, op één plek.
//
// Gedeeld met AppShell, dat het menu-item weglaat zodra er niets te tonen is:
// een "Instellingen" dat een lege sheet opent is erger dan geen "Instellingen".
// Zonder deze composable zouden die poorten op twee plekken staan en vroeg of
// laat uit elkaar lopen - dan verdwijnt het menu-item terwijl de sheet nog wel
// iets had, of andersom.
//
// Weergave staat er bewust niet bij: het kleurschema is uit de sheet gehaald en
// zit nu als submenu in het account-menu, waar je het in twee klikken omzet.
import { computed } from 'vue';
import { useAuth } from './useAuth';
import { useGithubAuth } from './useGithubAuth';

export function useSettingsSections() {
  const { authenticated, loading: authLoading, hasRole } = useAuth();
  const { status: githubStatus } = useGithubAuth();

  const signedIn = computed(() => !authLoading.value && authenticated.value);

  // Feature flags are deployment-wide and the PUT is behind `editor-admin`
  // (main.rs, "Admin routes"). Showing the switches to anyone else would give
  // them controls that silently revert on the 403.
  const showBeheer = computed(() => signedIn.value && hasRole('editor-admin'));

  // `configured: false` means this deployment has no GitHub OAuth App wired up.
  // `required` means the write path actually demands a personal token; without
  // it the section would describe a rule that is not in force.
  const showKoppelingen = computed(
    () =>
      signedIn.value &&
      !!githubStatus.value?.configured &&
      !!githubStatus.value?.required,
  );

  const hasContent = computed(() => showKoppelingen.value || showBeheer.value);

  return { signedIn, showBeheer, showKoppelingen, hasContent };
}
