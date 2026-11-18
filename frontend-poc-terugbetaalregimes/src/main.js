import '@nldd/design-system';
import '@nldd/design-system/styles';
import './assets/base.css';
import { createApp } from 'vue';
import App from './App.vue';
import router from './router.js';
import { reloadOnStaleBundle } from '@regelrecht/frontend-shared/reloadOnStaleBundle';

const app = createApp(App);
app.use(router);
// After a deploy, a tab this browser has not opened yet reloads instead of
// going blank; see reloadOnStaleBundle.js.
reloadOnStaleBundle(router);
app.mount('#app');
