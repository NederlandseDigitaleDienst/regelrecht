// De verkenning laadt eerst haar gegevens uit de bron (bron.js) en pas daarna de app: de
// modules van de toets lezen bij het laden al uit de bron.
import '@nldd/design-system';
import '@nldd/design-system/styles';
import './base.css';
import { createApp } from 'vue';
import { laadBron } from './bron.js';
import { volgPortaalStrook } from './portaalStrook.js';

volgPortaalStrook();
const plek = document.getElementById('app');
try {
  await laadBron();
  const { default: App } = await import('./App.vue');
  createApp(App).mount(plek);
} catch (e) {
  const melding = document.createElement('nldd-banner');
  melding.setAttribute('variant', 'critical');
  melding.setAttribute('text', 'De gegevens van deze verkenning zijn niet beschikbaar');
  melding.setAttribute('supporting-text', String(e?.message ?? e));
  melding.style.margin = '24px';
  plek.replaceChildren(melding);
}
