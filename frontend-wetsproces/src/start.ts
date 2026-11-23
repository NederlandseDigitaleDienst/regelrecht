// Mounts a web build of the screens, with the session cookie of the login:
// the combined app (main.ts, index.html) with both sides, and the back
// office on its own (backofficeomgeving/main.ts, backoffice.html). Each
// passes the routes of the sides it carries, so a build holds only those.
import '@nldd/design-system';
import '@nldd/design-system/styles';
import '@nldd/design-system/themes';
import { createApp } from 'vue';
import type { RouteRecordRaw } from 'vue-router';
import { useColorScheme } from '@regelrecht/frontend-shared';
import App from './App.vue';
import { laadUiterlijk } from './gedeeld/uiterlijk';
import { maakRouter } from './router';

export function start(kanten: RouteRecordRaw[]): void {
  useColorScheme();
  void laadUiterlijk();
  createApp(App).use(maakRouter(kanten)).mount('#app');
}
