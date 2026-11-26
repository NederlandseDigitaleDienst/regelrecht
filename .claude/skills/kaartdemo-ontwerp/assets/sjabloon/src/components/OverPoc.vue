<script setup>
// "Over deze PoC": een NLDD-sheet die links inschuift. Vaste secties; vul de teksten per casus.
const open = defineModel('open', { type: Boolean, default: false });

// De stappen door de app, in de volgorde van de tabs.
const STAPPEN = [
  { kop: 'Kies een plek', tekst: 'Klik op de kaart, of kies een plek onder Testpunten. Onder "Op dit punt" zie je welke plannen daar liggen, welk plan de regels geeft, wat erbovenop komt en wat opzij gaat, met de reden.' },
  { kop: 'Toets', tekst: '%%WAT_DE_TOETS_DOET%%' },
  { kop: 'Simuleren', tekst: 'Stel het bouwplan in en zie waar een regel omslaat. Vul onder "Gegevens invullen" in wat de kaart niet weet.' },
  { kop: 'Kaartlagen', tekst: 'Linksonder kies je wat de kaart kleurt.' },
  { kop: 'Graaf', tekst: 'Linksboven op de kaart wissel je naar de graaf: alle regels en verwijzingen achter de uitkomst op de gekozen plek, ook de opties die afvallen.' },
  { kop: 'Uitleg', tekst: 'De knop met het boek in het paneel legt de termen uit, zoals onthouding en geldend en beoogd recht.' },
];
</script>

<template>
  <nldd-sheet placement="left" width="560px" :open="open || undefined" accessible-label="Over deze PoC" @close="open = false">
    <nldd-top-title-bar text="Over deze PoC" supporting-text="%%GEBIED%%" :heading-level="2">
      <nldd-icon-button slot="toolbar" size="sm" icon="close" text="Sluiten" appearance="neutral-tinted" @click="open = false"></nldd-icon-button>
    </nldd-top-title-bar>
    <div class="inhoud">
      <section>
        <nldd-title size="display-5" :heading-level="3" text="Wat is dit?"></nldd-title>
        <nldd-text size="sm">
          <p>
            Een proof of concept (PoC) van RegelRecht voor de fysieke leefomgeving. De vraag: %%VRAAG%%
            Elke uitkomst is terug te volgen naar het artikel waar hij uit komt.
          </p>
        </nldd-text>
      </section>
      <section>
        <nldd-title size="display-5" :heading-level="3" text="Wat we willen leren"></nldd-title>
        <nldd-text size="sm"><p>%%WAT_WE_WILLEN_LEREN%%</p></nldd-text>
      </section>
      <section>
        <nldd-title size="display-5" :heading-level="3" text="Zo gebruik je het"></nldd-title>
        <ol class="stappen">
          <li v-for="s in STAPPEN" :key="s.kop"><nldd-text size="sm"><span><strong>{{ s.kop }}.</strong> {{ s.tekst }}</span></nldd-text></li>
        </ol>
      </section>
      <section>
        <nldd-title size="display-5" :heading-level="3" text="Wat het niet is"></nldd-title>
        <nldd-text size="sm"><p>Een verkenning, geen besluit. %%VOORBEHOUD%%</p></nldd-text>
      </section>
    </div>
  </nldd-sheet>
</template>

<style scoped>
.inhoud {
  flex: 1; min-height: 0; overflow-y: auto; overscroll-behavior: contain;
  padding: 8px 20px 24px; display: flex; flex-direction: column; gap: 20px;
}
section { display: flex; flex-direction: column; gap: 8px; }
.stappen { margin: 0; padding-left: 20px; display: flex; flex-direction: column; gap: 6px; }
</style>
