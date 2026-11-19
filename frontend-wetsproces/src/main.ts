import '@nldd/design-system';
import '@nldd/design-system/styles';
import { createApp } from 'vue';
import { useColorScheme } from '@regelrecht/frontend-shared';
import App from './App.vue';
import { maakRouter } from './router';

useColorScheme();
createApp(App).use(maakRouter()).mount('#app');
