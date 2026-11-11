import './nldd-components.js';
import '@nldd/design-system/styles';
import { computed, createApp } from 'vue';
import { createPinia } from 'pinia';
import App from './App.vue';
import router from './router';
import { useFeatureFlagsStore } from './stores/featureFlags';
import { useTrajectsStore } from './stores/trajects';
import { installApiAuthGuard } from './lib/apiAuthGuard';
import { installDevSlowMode } from './lib/devSlowMode';
import { createCorpusReader, createHttpTransport, installCorpus } from './corpus';
import { singleParam } from './lib/routeParams';

// Catch 401s on /api/* calls fired while a page is open and redirect to login,
// mirroring the router's navigation guard. Must run before any fetch.
installApiAuthGuard();

// Dev-only: `?slow=1500` delays /api/* responses so loading states are actually
// visible locally. No-op in a production build and when unset. Installed after
// the auth guard so a 401 redirect is not held up by the delay.
installDevSlowMode();

// Corpus reads go to the traject the route names; see src/corpus.
installCorpus(createCorpusReader({
  transport: createHttpTransport(),
  trajectRef: computed(() => singleParam(router.currentRoute.value.params.trajectRef) ?? null),
}));

// App-wide state lives in Pinia stores (src/stores); see the editor's
// component page in the docs for what goes in a store and what does not.
const app = createApp(App);
const pinia = createPinia();
app.use(pinia);
app.use(router);
// The server's feature flags (the defaults hold until they arrive) and the
// user's trajecten, which the shell shows on every page.
useFeatureFlagsStore(pinia).load();
useTrajectsStore(pinia).ensureReady();
app.mount('#app');
