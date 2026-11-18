/**
 * Licht/donker voor de twee pagina's die het portaal zelf rendert.
 *
 * Eigen module, en als eerste geïmporteerd in nldd-components.js: een `import`
 * wordt gehoist en vóór de rest van dat bestand uitgevoerd, dus code onderaan
 * daar zou pas draaien nadat het hele ontwerpsysteem is geëvalueerd. Wie donker
 * heeft staan zag de pagina dan eerst in het licht.
 *
 * Het contract is dat van de rest van regelrecht: het ontwerpsysteem leest
 * `data-scheme` op het root-element, en 'systeem' is de *afwezigheid* van dat
 * attribuut zodat `prefers-color-scheme` het overneemt. De keuze staat onder
 * `rr-theme` in localStorage.
 *
 * Waarom niet packages/frontend-shared/src/useColorScheme.ts: dat is Vue, en
 * deze twee pagina's worden in Rust gerenderd (packages/poc-portal/src/pagina.rs).
 * Waarom geen inline <script> in de pagina: POC_CSP staat `script-src 'self'`
 * zonder unsafe-inline.
 */
const SLEUTEL = 'rr-theme';
const THEMAS = ['systeem', 'licht', 'donker'];

// Onder `rr-theme` staan twee schrijfwijzen naast elkaar: de Vue-apps schrijven
// 'auto' (packages/frontend-shared/src/useColorScheme.ts) en de docs-site
// 'system' (docs/src/layouts/Base.astro). Allebei lezen, zodat een keuze in de
// editor of in de docs hier niet stil genegeerd wordt.
const UIT_OPSLAG = { auto: 'systeem', system: 'systeem', light: 'licht', dark: 'donker' };
const NAAR_ATTRIBUUT = { licht: 'light', donker: 'dark' };

function leesThema() {
  try {
    const rauw = window.localStorage?.getItem(SLEUTEL);
    const genormaliseerd = UIT_OPSLAG[rauw] ?? rauw;
    if (THEMAS.includes(genormaliseerd)) return genormaliseerd;
  } catch {
    // Privémodus of geblokkeerde opslag: dan volgen we gewoon het systeem.
  }
  return 'systeem';
}

function pasToe(thema) {
  const attribuut = NAAR_ATTRIBUUT[thema];
  if (attribuut) document.documentElement.setAttribute('data-scheme', attribuut);
  else document.documentElement.removeAttribute('data-scheme');
}

function bewaar(thema) {
  try {
    // Terugschrijven als 'auto' en niet als 'system': dat is wat
    // useColorScheme.js schrijft, en dus wat editor, admin en demo in dezelfde
    // browser al onder deze sleutel hebben staan.
    const naarOpslag = { systeem: 'auto', licht: 'light', donker: 'dark' };
    window.localStorage?.setItem(SLEUTEL, naarOpslag[thema]);
  } catch {
    // Zie leesThema: de keuze geldt dan alleen voor deze pagina.
  }
}

// Meteen, nog voor het ontwerpsysteem geladen is.
pasToe(leesThema());

document.addEventListener('DOMContentLoaded', () => {
  const menu = document.querySelector('[data-thema-menu]');
  if (!menu) return;

  const markeer = (thema) => {
    for (const item of menu.querySelectorAll('nldd-menu-item[value]')) {
      if (item.getAttribute('value') === thema) item.setAttribute('selected', '');
      else item.removeAttribute('selected');
    }
  };
  markeer(leesThema());

  // `select` van nldd-menu-item bubbelt en is composed, en draagt geen detail:
  // de waarde staat op het item zelf. Eén luisteraar op het menu volstaat.
  menu.addEventListener('select', (event) => {
    const item = event.composedPath?.().find((n) => n?.tagName === 'NLDD-MENU-ITEM');
    const thema = item?.getAttribute('value');
    if (!THEMAS.includes(thema)) return;
    pasToe(thema);
    bewaar(thema);
    markeer(thema);
  });

  // Volg het systeem zolang niemand expliciet kiest.
  window.matchMedia?.('(prefers-color-scheme: dark)').addEventListener('change', () => {
    if (leesThema() === 'systeem') pasToe('systeem');
  });
});
