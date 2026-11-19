// Eerst de gegevens, dan pas de app: modules die bij het laden al gegevens lezen, vinden ze dan.
// Mislukt het laden, dan een melding met de reden in plaats van een lege pagina.
import '@nldd/design-system';
import '@nldd/design-system/styles';
import './base.css';
import { createApp } from 'vue';

const plek = document.getElementById('app');
try {
  // %%GEGEVENS_LADEN%%: bijv. await laadBron() achter het portaal, of niets als alles in public/ staat.
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
