import '@nldd/design-system';
import '@nldd/design-system/styles';
import './assets/base.css';
import { createApp } from 'vue';
import App from './App.vue';
import router from './router.js';
import { reloadOnStaleBundle } from '@regelrecht/frontend-shared/reloadOnStaleBundle';

const app = createApp(App);
app.use(router);
reloadOnStaleBundle(router);
app.mount('#app');
