<script setup>
// "Over deze PoC": een NLDD-sheet die links inschuift.
const open = defineModel('open', { type: Boolean, default: false });

// Hoe de simulatie rekent.
const SIMULATIE = [
  { kop: 'Dezelfde regels en dezelfde engine', tekst: 'De browser laadt de RegelRecht-engine (WASM) en dezelfde 37 regelingen in YAML die het DSO-spoor gebruikt. Per vak staan de plannen en de artikelen uit het DSO al klaar. Met de standaardinstelling komt er precies hetzelfde uit als vooraf gerekend; een test bewaakt dat.' },
  { kop: 'Het bouwplan', tekst: 'Lagen maal meter per laag geeft de hoogte. De bouwregels vergelijken die met de maximale hoogte of het aantal lagen op de plankaart, en het woningtype met de bouwaanduiding (bijvoorbeeld gestapeld).' },
  { kop: 'Gegevens invullen', tekst: 'Wat de kaart niet weet, zet je op gunstig, onbekend of ongunstig: welstand, bodem, en alle overige artikelen die het DSO-spoor als gegeven aanneemt (archeologie, parkeren, geluid, de meeste regels van andere overheden). Onbekend geeft een onthouding. De diepte van de fundering telt bij het verbod van de provincie vanaf 30 m.' },
  { kop: 'Uitkomst per aantal lagen', tekst: 'Voor de gekozen plek rekent de toets 1 tot en met 15 lagen. Een streep markeert waar een regel omslaat, bijvoorbeeld van toegestaan naar niet toegestaan bij de maximale hoogte. Het eindoordeel komt uit artikel 22.29 van het omgevingsplan.' },
  { kop: 'Op de kaart', tekst: 'Met "Toon de uitkomst op de kaart" rekent de browser alle 1.266 vakken opnieuw (een paar seconden) en verschijnt onder het thema DSO de laag Simulatie. Verander je daarna iets, dan rekent de laag opnieuw.' },
  { kop: 'Eén bouwplan', tekst: 'Het bouwplan is gedeeld met Simuleren van de eerste PoC: zet je daar 7 lagen, dan rekent ook het DSO-spoor met 7 lagen. De aannames zijn apart, omdat de sporen andere gegevens vragen.' },
];

// De stappen door de app; de laatste is de tab "Met en zonder DSO".
const STAPPEN = [
  { kop: 'Kies een plek', tekst: 'Klik op de kaart, of kies een plek onder Testpunten. Onder "Op dit punt" zie je welke plannen daar liggen, welk plan de regels geeft, wat erbovenop komt en wat opzij gaat, met de reden.' },
  { kop: 'Toets', tekst: 'Kan het bouwplan op die plek? De toets rekent het tijdelijk deel (de oude bestemmingsplannen) en het nieuwe deel (TAM-plannen) naast elkaar. Het eindoordeel komt uit artikel 22.29 van het omgevingsplan: toegestaan, niet toegestaan of onthouding.' },
  { kop: 'Simuleren', tekst: 'Stel het bouwplan in. Zie per aantal lagen (1 tot en met 15) waar een regel omslaat. Vul onder "Gegevens invullen" in wat de kaart niet weet, zoals welstand, parkeren of de afstand tot de perceelsgrens.' },
  { kop: 'Kaartlagen', tekst: 'Linksonder kies je wat de kaart kleurt: welk plan waar ligt, maximale hoogte, functie, waar de toets rekent, de uitkomst, of het DSO. "Uitkomst" laat per vak van 40 meter zien waar het bouwplan wel, niet of nog niet te beoordelen is.' },
  { kop: 'Graaf', tekst: 'Linksboven op de kaart wissel je naar de graaf: alle regels en verwijzingen achter de uitkomst op de gekozen plek, ook de opties die afvallen.' },
  { kop: 'Uitleg', tekst: 'De knop met het boek in het paneel legt de termen uit, zoals onthouding, tijdelijk en nieuw deel, en geldend en beoogd.' },
  { kop: 'Met en zonder DSO', tekst: 'De tab die het DSO-spoor toevoegt. "Op dit punt" zet beide sporen naast elkaar: zonder DSO (de toets hierboven) en met DSO, met de artikelen per overheid en de plichten; klik op een artikel voor de tekst uit het DSO. Verder: simuleren met het DSO-spoor, twee reeksen scenario\'s (de woontoren en echte besluiten van de gemeente) en de kerncijfers. Het kaartthema DSO kleurt vóór DSO, met DSO, alle overheden samen of het verschil.' },
];
</script>

<template>
  <nldd-sheet placement="left" width="560px" :open="open || undefined" accessible-label="Over deze PoC" @close="open = false">
    <nldd-top-title-bar text="Over deze PoC" supporting-text="Spoorzone Veenendaal, met en zonder DSO" :heading-level="2">
      <nldd-icon-button slot="toolbar" size="sm" icon="close" text="Sluiten" appearance="neutral-tinted" @click="open = false"></nldd-icon-button>
    </nldd-top-title-bar>
    <div class="inhoud">
      <section>
        <nldd-title size="display-5" :heading-level="3" text="Wat is dit?"></nldd-title>
        <nldd-text size="sm">
          Een proof of concept (PoC) van RegelRecht voor de fysieke leefomgeving. De vraag: kan er in de Spoorzone van
          Veenendaal een woontoren van 5 lagen komen, en zo nee, waarom niet? Elke uitkomst is terug te volgen naar het
          artikel waar hij uit komt.
        </nldd-text>
        <nldd-text size="sm">
          De kaart, de toets en de simulatie zijn die van de eerste PoC: plannen zelf opgehaald, lagen met de hand gelegd,
          regels per bestemming geschreven, alleen de gemeente. Daarnaast staat het DSO-spoor: plannen, regels en teksten
          uit het Digitaal Stelsel Omgevingswet (DSO), via één zoekvraag op het gebied, voor gemeente, provincie,
          waterschap en rijk tegelijk. Het DSO-spoor leidt de lagen af uit de gegevens en genereert de regels in een paar
          vaste vormen. Beide rekenen op dezelfde RegelRecht-engine. De tab "Met en zonder DSO" zet ze naast elkaar.
        </nldd-text>
      </section>

      <section>
        <nldd-title size="display-5" :heading-level="3" text="Wat we willen leren"></nldd-title>
        <nldd-text size="sm">
          Wat het DSO levert en wat niet. Het DSO zegt per regel bij welke activiteit en plek hij hoort, maar niet wat de
          regel doet. Welke activiteiten bij een bouwplan horen, blijft handwerk. Het verschil tussen de sporen laat zien
          waar dat uitmaakt.
        </nldd-text>
        <nldd-text size="sm">
          Het verschil komt niet alleen door het DSO. Het DSO-spoor kijkt naar meer plannen, ook van provincie,
          waterschap en rijk. Dat had ook zonder het DSO gekund, met een uitgebreidere analyse van de plannen. Het DSO
          maakte het wel makkelijker om die plannen direct te verzamelen.
        </nldd-text>
      </section>

      <section>
        <nldd-title size="display-5" :heading-level="3" text="Zo gebruik je het"></nldd-title>
        <ol class="stappen">
          <li v-for="(s, i) in STAPPEN" :key="s.kop">
            <span class="nr">{{ i + 1 }}</span>
            <div><nldd-text size="sm"><strong>{{ s.kop }}</strong></nldd-text><nldd-text size="sm">{{ s.tekst }}</nldd-text></div>
          </li>
        </ol>
      </section>

      <section>
        <nldd-title size="display-5" :heading-level="3" text="Zo werkt simuleren met het DSO-spoor"></nldd-title>
        <nldd-text size="sm">
          Het kaartthema DSO en "Op dit punt" onder Met en zonder DSO tonen een uitkomst die vooraf is gerekend, voor één
          bouwplan (5 lagen van 3,2 m) en met alle open gegevens gunstig. Onder Met en zonder DSO, Simuleren, reken je het
          DSO-spoor opnieuw, in je eigen browser. Er gaat niets naar een server.
        </nldd-text>
        <ol class="stappen">
          <li v-for="(s, i) in SIMULATIE" :key="s.kop">
            <span class="nr">{{ i + 1 }}</span>
            <div><nldd-text size="sm"><strong>{{ s.kop }}</strong></nldd-text><nldd-text size="sm">{{ s.tekst }}</nldd-text></div>
          </li>
        </ol>
        <nldd-text size="sm">
          Grenzen: hier rekent alleen het DSO-spoor opnieuw; in de kaartlaag Vóór DSO blijft de vooraf gerekende uitkomst. De
          eerste PoC rekent opnieuw onder de tab Simuleren en in de kaartlaag Uitkomst. Het aantal woningen telt niet mee, omdat de gegenereerde bouwregels het niet lezen. Een verbouw of omzetten naar
          wonen kan niet. Waar het DSO-spoor zich onthoudt omdat een tekst of hoogte ontbreekt, helpt een andere aanname niet.
        </nldd-text>
      </section>

      <section>
        <nldd-title size="display-5" :heading-level="3" text="Wat het niet is"></nldd-title>
        <nldd-text size="sm">
          Geen vergunning en geen juridisch advies. Open gegevens (welstand, parkeren, fundering) zijn gunstig ingevuld.
          Het DSO-materiaal komt deels uit de pre-productieomgeving, met testregelingen; die tellen niet mee. Het Ambacht uit
          2009 staat niet in het DSO. Verkenning, geen besluit.
        </nldd-text>
      </section>

      <section>
        <nldd-title size="display-5" :heading-level="3" text="Bronnen"></nldd-title>
        <nldd-text size="sm">
          Ruimtelijke plannen opvragen v4 (productie), Omgevingsdocumenten presenteren v8 en Geometrie opvragen v1
          (pre-productie), bekendmakingen van officielebekendmakingen.nl, de achtergrondkaart van PDOK.
        </nldd-text>
      </section>
    </div>
  </nldd-sheet>
</template>

<style scoped>
/* De sheet zelf scrolt niet en knipt af; de tekst onder de titelbalk scrolt zelf. */
.inhoud {
  flex: 1; min-height: 0; overflow-y: auto; overscroll-behavior: contain;
  padding: 8px 20px 24px; display: flex; flex-direction: column; gap: 20px;
}
section { display: flex; flex-direction: column; gap: 6px; }
.stappen { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 10px; }
.stappen li { display: flex; gap: 10px; align-items: flex-start; }
.stappen li > div { display: flex; flex-direction: column; gap: 2px; }
.nr {
  flex: none; width: 22px; height: 22px; border-radius: 50%; margin-top: 1px;
  display: inline-flex; align-items: center; justify-content: center;
  font: 700 12px/1 var(--primitives-font-family-body, system-ui, sans-serif);
  background: #154273; color: #fff;
}
</style>
