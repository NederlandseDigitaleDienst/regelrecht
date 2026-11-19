// De ingang van de bundel. De volgorde telt: eerst de opmaak en de
// componenten van het ontwerpsysteem, dan die van de pagina, dan de app.
import '@nldd/design-system/styles';
import './nldd-components.js';
import './stijl.css';
import { createApp } from 'vue';
import App from './App.vue';

createApp(App).mount('#app');
