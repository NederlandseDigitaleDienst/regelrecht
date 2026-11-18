<script setup lang="ts">
import { computed, onMounted } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { useAuth } from './composables/useAuth';

// Bare "you're logged out" page (top-level route, no app-chrome). Reached when
// the session expired: both the router guard (on a protected route) and the
// fetch 401-guard redirect here with the page the user was on as `return_url`,
// instead of the old silent window.location bounce to /auth/login that could
// dead-end on a blank page. From here the user re-authenticates deliberately
// (returning to where they were) or steps out to the public environment.
const route = useRoute();
const router = useRouter();
const { login } = useAuth();

// Only accept a same-origin path (a single leading '/'), so a crafted
// ?return_url= can't turn the re-login button into an open redirect. Anything
// else falls back to Home. login() re-encodes it and the backend validates it
// too; this is the first line of defence.
const returnUrl = computed(() => {
  const raw = route.query.return_url;
  const ok =
    typeof raw === 'string' &&
    raw.startsWith('/') &&
    !raw.startsWith('//') &&
    !raw.startsWith('/\\');
  return ok ? raw : '/';
});

function relogin() {
  login(returnUrl.value);
}

function goPublic() {
  router.push('/');
}

onMounted(() => {
  document.title = 'Uitgelogd: sessie verlopen · RegelRecht';
});
</script>

<template>
  <nldd-shell>
    <nldd-page>
      <nldd-simple-section width="600px">
        <nldd-inline-dialog
          icon="logout"
          text="Je bent uitgelogd omdat je sessie is verlopen."
          supporting-text="Log opnieuw in om verder te gaan waar je gebleven was of ga naar de publieke omgeving."
        >
          <nldd-button slot="actions" appearance="primary" text="Opnieuw inloggen" @click="relogin"></nldd-button>
          <nldd-button slot="actions" appearance="secondary" text="Naar de publieke omgeving" @click="goPublic"></nldd-button>
        </nldd-inline-dialog>
      </nldd-simple-section>
    </nldd-page>
  </nldd-shell>
</template>
