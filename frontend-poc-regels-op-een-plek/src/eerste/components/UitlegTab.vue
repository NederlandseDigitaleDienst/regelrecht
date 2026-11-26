<script setup>
// Tab Uitleg in het kaartpaneel: wat de termen in deze verkenning betekenen, met onthouding als
// eigen hoofdstuk. Andere tabs springen hierheen met een
// anker: id "uitleg-<begrip>". Het kruisje (en Escape) brengt je terug naar waar je was.
const emit = defineEmits(['sluit']);
const BEGRIPPEN = [
  { id: 'toegestaan', kop: 'Toegestaan', kleur: 'goed',
    tekst: 'Alle regels die de toets op deze plek kent, zeggen ja. Met de feiten van de kaart en wat je zelf hebt ingevuld. Het is een verkenning, geen vergunning: regels die nog niet in YAML staan, tellen niet mee.' },
  { id: 'niet-toegestaan', kop: 'Niet toegestaan', kleur: 'kritiek',
    tekst: 'Minstens één regel zegt nee. Eén nee is genoeg, ook als andere regels nog onbekend zijn. Het nee komt uit een feit van de kaart (bestemming, bouwvlak, maximale hoogte, woningtype) of uit iets wat je zelf als nee hebt ingevuld. Een ander bouwplan, bijvoorbeeld minder lagen, kan het nee opheffen.' },
  { id: 'valt-af', kop: 'Valt af', kleur: 'leeg',
    tekst: 'In de graaf: een tak van een regel die hier niet geldt, omdat de voorwaarde niet klopt. Bijvoorbeeld de regel voor een andere bestemming. De graaf toont ze grijs, zodat je ziet welke opties er waren.' },
];
const PLAN = [
  { id: 'geldend', kop: 'Geldend en beoogd',
    tekst: 'Geldend is vastgesteld recht. Beoogd is een ontwerp, zoals het ontwerp-TAM Het Ambacht: het laat zien waar de gemeente naartoe wil, maar het geldt nog niet. De toets rekent beoogd apart en zet het er steeds bij.' },
  { id: 'deel', kop: 'Tijdelijk en nieuw deel',
    tekst: 'Sinds de Omgevingswet heeft Veenendaal één omgevingsplan met twee delen. Het tijdelijk deel zijn de oude bestemmingsplannen (de bruidschat). Het nieuwe deel schrijft de gemeente zelf, in TAM-plannen. De toets rekent ze als twee kolommen naast elkaar, zonder voorrang.' },
  { id: 'tam', kop: 'TAM-plan',
    tekst: 'Een plan dat het omgevingsplan op een plek wijzigt. Is het vastgesteld, dan zet artikel 2 de bestemmingsplannen daar buiten toepassing; alleen de regels van het TAM-plan gelden dan.' },
  { id: 'paraplu', kop: 'Paraplu',
    tekst: 'Het parapluplan van 2022 vervangt in veel bestemmingsplannen een paar artikelen, zoals parkeren en archeologie. Alleen de artikelen uit de tabellen van het paraplu tellen mee.' },
  { id: 'eindoordeel', kop: 'Eindoordeel (artikel 22.29)',
    tekst: 'Artikel 22.29 van het omgevingsplan zegt wanneer de vergunning voor bouwen wordt verleend: de regels ter plaatse, welstand en bodem. Hier komen de uitkomsten van alle regels samen.' },
  { id: 'niet-in-yaml', kop: 'Niet in YAML',
    tekst: 'Het plan of de bestemming is nog niet vertaald naar regels die de engine kan rekenen. Daar zegt de toets niets. Op de kaart grijs.' },
  { id: 'bron-ontbreekt', kop: 'Bron ontbreekt',
    tekst: 'Het plan geldt, maar de regels zijn er alleen als scan, zoals Het Ambacht uit 2009. Hoe uit een scan te citeren is nog een open keuze. Tot die tijd: een onthouding.' },
  { id: 'markering', kop: 'Markering',
    tekst: 'Een stuk regel dat zich niet letterlijk laat vertalen, en dat een mens eerst moet accepteren. Tot dan rekent de engine dat artikel niet.' },
  { id: 'aanname', kop: 'Aanname',
    tekst: 'Een waarde die je zelf invult onder Simuleren, Gegevens invullen. Het is geen feit. Toets, Graaf en de kaart rekenen ermee, en melden dat.' },
];
const SOORTEN = [
  { kop: 'Een oordeel', tekst: 'Alleen een mens kan het geven: welstand, het puntensysteem, uiterlijk aanzien.' },
  { kop: 'Een onderzoek', tekst: 'Hoort bij de aanvraag: bodem, archeologie, geluid, parkeren.' },
  { kop: 'Een gegeven buiten de kaart', tekst: 'Over het perceel of het bouwplan: afstand tot de perceelsgrens, bestaande woningen, sociale woningen.' },
  { kop: 'Een bron die ontbreekt', tekst: 'Het plan bestaat alleen als scan.' },
  { kop: 'Een markering', tekst: 'De regel is nog niet geaccepteerd.' },
];
</script>

<template>
  <nldd-container layout="stack" gap="16" class="uitleg">
    <div class="sluit">
      <nldd-icon-button size="sm" icon="close" text="Uitleg sluiten" appearance="neutral-tinted" @click="emit('sluit')"></nldd-icon-button>
    </div>
    <nldd-title size="display-4" :heading-level="2" overline="Uitleg" text="Wat de termen betekenen"
      supporting-text="Deze verkenning rekent met de RegelRecht-engine. Hij geeft per regel ja, nee of onthouding."></nldd-title>

    <section id="uitleg-uitkomsten" class="blok">
      <nldd-title size="display-5" :heading-level="3" text="De drie uitkomsten"></nldd-title>
      <div v-for="b in BEGRIPPEN.slice(0, 2)" :id="`uitleg-${b.id}`" :key="b.id" class="begrip">
        <span class="stip" :class="b.kleur"></span>
        <div><nldd-text><strong>{{ b.kop }}</strong></nldd-text><nldd-text size="sm">{{ b.tekst }}</nldd-text></div>
      </div>
      <div class="begrip">
        <span class="stip open"></span>
        <div><nldd-text><strong>Onthouding</strong></nldd-text><nldd-text size="sm">Zie hieronder.</nldd-text></div>
      </div>
    </section>

    <section id="uitleg-onthouding" class="blok nadruk">
      <nldd-title size="display-5" :heading-level="3" overline="De belangrijkste" text="Onthouding"></nldd-title>
      <nldd-text size="sm">
        Een onthouding betekent: <strong>de engine zegt niet ja en niet nee, omdat er een gegeven ontbreekt dat de regel nodig heeft.</strong>
        Het is geen "misschien" en geen kans. Het is een gat dat iemand moet vullen.
      </nldd-text>
      <nldd-text size="sm">
        De engine verzint niets. Ontbreekt een gegeven, dan raadt hij niet, maar zegt hij welk gegeven ontbreekt.
        Liever geen antwoord dan een antwoord dat klinkt als waar. In de Toets staat bij elke onthouding welke gegevens missen.
      </nldd-text>

      <nldd-text size="sm"><strong>Waar een onthouding vandaan komt</strong></nldd-text>
      <nldd-rich-text>
        <ul>
          <li v-for="s in SOORTEN" :key="s.kop"><strong>{{ s.kop }}.</strong> {{ s.tekst }}</li>
        </ul>
      </nldd-rich-text>

      <nldd-text size="sm"><strong>Hoe ja, nee en onbekend samengaan</strong></nldd-text>
      <table class="logica">
        <caption class="sr-only">Hoe de engine ja, nee en onbekend combineert</caption>
        <thead><tr><th scope="col">Soort regel</th><th scope="col">Uitkomst</th></tr></thead>
        <tbody>
          <tr>
            <th scope="row">Alles moet kloppen (en)</th>
            <td>Eén nee: <strong>nee</strong>. Geen nee, wel iets onbekend: <strong>onthouding</strong>. Alles ja: <strong>ja</strong>.</td>
          </tr>
          <tr>
            <th scope="row">Eén route is genoeg (of)</th>
            <td>Eén ja: <strong>ja</strong>. Geen ja, wel iets onbekend: <strong>onthouding</strong>. Alles nee: <strong>nee</strong>.</td>
          </tr>
        </tbody>
      </table>
      <nldd-text size="sm">
        Daarom kan een plek "niet toegestaan" zijn terwijl er nog dingen onbekend zijn: één harde nee is genoeg.
        En daarom is een plek zonder harde nee nog geen "toegestaan": welstand en bodem uit artikel 22.29 zijn in een
        verkenning altijd onbekend. Zonder invullen is onthouding dus het beste wat er op de kaart kan staan.
      </nldd-text>

      <nldd-text size="sm"><strong>Wat je ermee doet</strong></nldd-text>
      <nldd-text size="sm">
        Vul onder Simuleren, Gegevens invullen, in wat je aanneemt. De onthouding wordt dan ja of nee, en op de kaartlaag
        Uitkomst zie je waar. Met "Alles gunstig" zie je de plekken waar alleen nog oordelen en onderzoeken in de weg
        staan: daar beslist de aanvraag, niet het plan. Blijft het nee, dan zegt het plan zelf nee.
      </nldd-text>
    </section>

    <section id="uitleg-plannen" class="blok">
      <nldd-title size="display-5" :heading-level="3" text="Plannen en lagen"></nldd-title>
      <div v-for="b in PLAN" :id="`uitleg-${b.id}`" :key="b.id" class="begrip zonder-stip">
        <nldd-text><strong>{{ b.kop }}</strong></nldd-text>
        <nldd-text size="sm">{{ b.tekst }}</nldd-text>
      </div>
      <div :id="`uitleg-${BEGRIPPEN[2].id}`" class="begrip zonder-stip">
        <nldd-text><strong>{{ BEGRIPPEN[2].kop }}</strong></nldd-text>
        <nldd-text size="sm">{{ BEGRIPPEN[2].tekst }}</nldd-text>
      </div>
    </section>

    <nldd-text size="xs" color="secondary">Verkenning, geen besluit. Een vergunning vraagt altijd de beoordeling van het bevoegd gezag.</nldd-text>
  </nldd-container>
</template>

<style scoped>
.uitleg { position: relative; }
/* Het kruisje blijft rechtsboven staan, ook als je ver naar beneden scrolt. */
.sluit { position: sticky; top: 0; z-index: 2; display: flex; justify-content: flex-end; height: 0; overflow: visible; }
.blok { display: flex; flex-direction: column; gap: 8px; scroll-margin-top: 8px; }
.nadruk {
  padding: 12px 14px; border-radius: 8px;
  border-left: 4px solid #fab219;
  background: color-mix(in srgb, #fab219 10%, transparent);
}
.begrip { display: flex; gap: 10px; align-items: flex-start; scroll-margin-top: 8px; }
.begrip > div { display: flex; flex-direction: column; gap: 2px; }
.zonder-stip { flex-direction: column; gap: 2px; }
.stip { flex: none; width: 14px; height: 14px; margin-top: 4px; border-radius: 3px; background: var(--status); }
.goed { --status: #0ca30c; }
.open { --status: #fab219; }
.kritiek { --status: #d03b3b; }
.leeg { --status: #8a8a86; }
.logica { border-collapse: collapse; font-size: 0.82rem; width: 100%; }
.logica th, .logica td { text-align: left; vertical-align: top; padding: 6px 8px; border-top: 1px solid color-mix(in srgb, #999 35%, transparent); }
.logica thead th { border-top: none; font-weight: 600; }
.logica tbody th { font-weight: 600; width: 38%; }
.sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
</style>
