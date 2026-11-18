import '@nldd/design-system';
import '@nldd/design-system/styles';
import { createApp } from 'vue';
import { useColorScheme } from '@regelrecht/frontend-shared';
import App from './App.vue';

const app = createApp(App);
// Participate in the shared color-scheme system (localStorage-backed default):
// applies any stored preference to <html data-scheme> and otherwise follows the
// OS via prefers-color-scheme. Lawmaking has no auth/API, so it consumes only
// the color-scheme primitive from the shared package.
useColorScheme();
app.mount('#app');
