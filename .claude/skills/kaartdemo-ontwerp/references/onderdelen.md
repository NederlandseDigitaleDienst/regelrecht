# Onderdelen van de kaartdemo

Per onderdeel: wat het doet, hoe het eruitziet, met welke componenten en maten. Overgenomen uit
`frontend-poc-regels-op-een-plek` (stand 2026-11-18). Wijk alleen af als de casus erom vraagt, en
zeg dan waarom.

## Inhoud

1. Techniek
2. App-schil
3. Kaart
4. Legenda en kaartlagen
5. Paneel
6. Tabs en hun inhoud
7. Patronen in het paneel
8. Sheets
9. Graaf
10. Presentatie
11. Kleuren
12. Teksten

## 1. Techniek

- Vite + Vue 3 (`<script setup>`), JavaScript, geen TypeScript.
- `@nldd/design-system` (webcomponenten): `import '@nldd/design-system'` en
  `import '@nldd/design-system/styles'` in `main.js`. In `vite.config.js`:
  `isCustomElement: (tag) => tag.startsWith('nldd-')`.
- `leaflet` voor de kaart, `proj4` voor RD, `@vue-flow/core` + `@vue-flow/background` voor de graaf.
- RegelRecht-engine als WASM in `public/wasm/pkg/` (gekopieerd door `scripts/copy-assets.js` in
  `predev`/`prebuild`). Versie, commit en schema in de voetregel.
- `base: process.env.POC_BASE ?? '/'` zodat de app achter het portaal op `/<slug>/` draait.
- Tests met vitest en happy-dom. Rekenen staat in losse `.js`-modules zonder Vue, zodat het
  testbaar is; componenten tonen alleen.

## 2. App-schil (`App.vue`)

```
nldd-shell
└─ .app-frame              position:relative; flex column; height:100vh
   ├─ nldd-skip-link text="Direct naar de inhoud"
   │  └─ nldd-top-navigation-bar logo-title="<gebied>" logo-subtitle="Verkenning · <vraag>"
   ├─ .kop-knoppen         absolute top:16 left:16; z 5; gap 8
   │  ├─ nldd-button size=sm appearance=neutral-tinted start-icon=info-circle "Over deze PoC"
   │  └─ nldd-button size=sm appearance=neutral-tinted start-icon=play "Presentatie"   (optioneel)
   ├─ nldd-banner variant=warning size=sm    alleen bij verouderde gegevens
   ├─ main#hoofdinhoud.app-main tabindex=-1   flex:1; min-height:0; overflow-y:auto
   │  └─ KaartTab
   └─ footer.app-footer    border-top 1px neutral-200
      └─ nldd-container padding-inline=16 padding-block=8
         └─ nldd-text size=xs color=secondary
            "RegelRecht-engine <v>, commit <9 tekens>, schema t/m <v>, <n> regelingen geladen. Verkenning, geen besluit."
```

- `index.html`: in `#app` een platte melding "De gegevens van deze verkenning laden…" tot de app er
  staat; `#app` heeft `height:100%` (nldd-shell knipt zijn inhoud af).
- `main.js`: eerst de gegevens laden (top-level await), dan pas `App.vue` importeren. Mislukt het,
  dan vervangt een `nldd-banner variant="critical"` met de reden de inhoud van `#app`.
- Engine niet geladen: voetregel in `color="critical"`: "Engine niet geladen: <reden>".

## 3. Kaart (`KaartTab.vue`)

- `.kaarttab`: padding 16px, flex. `.kaartkader`: relative, flex, min-height 420px, rand 1px
  neutral-300, radius 8px, overflow hidden.
- Leaflet: `zoomControl: true`, `attributionControl: false`, daarna
  `L.control.attribution({ position: 'bottomleft', prefix: false })`.
- Tegels: `https://service.pdok.nl/brt/achtergrondkaart/wmts/v2_0/grijs/EPSG:3857/{z}/{x}/{y}.png`,
  maxZoom 19, bronvermelding "Kaart: PDOK, BRT Achtergrondkaart grijs (Kadaster)". In de monorepo
  moet de PDOK-host in `POC_CSP img-src` staan.
- Filter op `.leaflet-tile-pane`: licht `saturate(0) contrast(0.86) brightness(1.06)`, donker
  `grayscale(0.8) brightness(0.72) contrast(1.05)`.
- Lagen van onder naar boven: contextthema (canvas in eigen pane `context`, z-index 350), plangrenzen,
  bouwvlakken, accent op het punt, testpunten, gekozen punt.
- Plangrenzen: lijnkleur per plansoort, weight 1,5 (groot onderliggend plan 0,5), fillOpacity 0,07
  (groot 0,03), ontwerp gestippeld `5 4`. Een paraplu over de hele gemeente teken je niet.
- Zoekgebied: donkerblauw `#154273`, weight 2, dashArray `6 4`, geen vulling, label "afgeleid, geen
  officiële grens" als dat zo is.
- Gekozen punt: `circleMarker` radius 7, rand zwart 2, vulling geel `#ffb612`.
- Accent op het punt: de vlakken die de feiten leveren. Bestemming oranje `#e17000` weight 2
  fillOpacity 0,08; bouwvlak zwart, dashArray `4 3`.
- Testpunten: `L.divIcon` met label (P1, P2, …), donkerblauw met witte rand 1,5 px, radius 5,
  schaduw `0 1px 4px rgb(0 0 0 / .35)`, vet 11 px. Klik = kies dat punt. Een tweede reeks
  (scenario's) in violet `#42145f` met eigen voorvoegsel (W1, B1).
- Planlabels (thema Plan): plannen vanaf 1 ha met "nr · naam", kleinere alleen het nummer;
  tooltip met naam bij aanwijzen, rand dikker bij hover.
- Klik op de kaart: punt kiezen, RD in de URL, testpuntselectie wissen. In Toets/Simuleren blijf je
  in die tab (die rekenen meteen op het nieuwe punt); anders naar Op dit punt.
- Naar een punt springen (`kiesRd`): `setView` zonder animatie, dan `panBy(paneelbreedte / 2)` zodat
  het punt midden in het vrije deel links van het paneel ligt. `fitBounds` ook met
  `paddingBottomRight: [paneelbreedte + 24, 24]`.
- `ResizeObserver` op de kaart → `invalidateSize()`; ook na wisselen van tab (paneelbreedte).
- Duizenden vlakken: canvas-renderer. Alleen SVG als de vlakken een tooltip nodig hebben.
- Zware lagen pas bouwen als ze aan gaan; zwaar rekenen met voortgangsbalk en een rondenummer zodat
  een oude ronde een nieuwe niet overschrijft. Opnieuw rekenen na 350 ms rust.

## 4. Legenda en kaartlagen

Linksonder in het kaartkader, boven de bronvermelding.

- Positie `left:12px; bottom:30px`, z 650, padding 8px 12px, radius 8, achtergrond
  `color-mix(in srgb, <base-background> 90%, transparent)` met `backdrop-filter: blur(6px)`,
  rand neutral-400 op 35 %, schaduw `0 2px 10px rgb(0 0 0 / .10)`, max-width 460px,
  max-height `calc(100% - 120px)` met eigen scroll. Verborgen in de graafweergave.
- Inhoud van boven naar onder:
  1. `nldd-text size=sm` **Kaartlagen**
  2. `nldd-segmented-control size=sm` met de thema's (korte namen: Plan, Hoogte, Functie, Toets,
     Uitkomst, …). Eén thema tegelijk.
  3. `nldd-text size=xs` **lange naam van het thema**
  4. Bediening van het thema waar nodig: tweede segmented control "Geldend recht | Ontwerp-…",
     een regel met het bouwplan en de aannames plus link "Wijzigen", een `nldd-progress-bar` tijdens
     rekenen.
  5. Groep: uitleg (`xs secondary`), dan per klasse een vlakje en tekst, met het aantal tussen
     haakjes waar dat er is. Link "Wat is een onthouding?" naar de Uitleg.
  6. Bij Uitkomst een `<details open>` "Wat ontbreekt nog (aantal vakken)": meest voorkomend eerst,
     invulbare gegevens als link naar Gegevens invullen.
  7. `nldd-checkbox-field` "Bouwvlakken (waar een hoofdgebouw mag)".
  8. `<details>` **Overige tekens**: groepen Gebied, Plannen, Op het gekozen punt.
- Legendatekens: vlak 18×12, rand 1,5 px, radius 2, vulling `color-mix(kleur 72%)` (contextlagen) of
  14 % (plangrenzen), gestippeld waar de kaart stippelt; lijn 20 px breed; punt 12 px geel met
  zwarte rand; nummer als klein donkerblauw label "P1".
- Vulling op de kaart: contextthema's 0,5–0,72; "rekent hier niet" en "gelijk" lichter (0,22–0,28).

## 5. Paneel

- `aside.paneel`: absolute `top/right/bottom 12px`, z 1000, breedte `clamp(380px, 40vw, 620px)`,
  breed `clamp(460px, 54vw, 960px)`, `transition: width .18s`. Radius 10, rand neutral-400 op 40 %,
  schaduw `0 6px 24px rgb(0 0 0 / .14)`, zelfde glas als de legenda maar op 74 %.
- Breed bij tabs met veel inhoud: Toets, Simuleren, Uitleg, vergelijkingen.
- Kop (`padding 12px 16px 8px`, flex, gap 8):
  - `nldd-tab-bar` met `nldd-tab-bar-item` (tekst + icoon, `:current`), flex 1.
  - Uitleg is hulp, geen werktab: een eigen knop. Breed: `nldd-button` "Uitleg" (accent-filled als
    open); smal: `nldd-icon-button icon=book`. Escape en het kruisje in de uitleg brengen je terug
    naar de vorige tab.
  - `nldd-icon-button icon=chevron-double-right` "Paneel minimaliseren".
- Inhoud: flex 1, eigen scroll, padding `8px 16px 20px`, `overscroll-behavior: contain`,
  `container-type: inline-size; container-name: paneel` (kolommen klappen onder 620 px samen via
  `@container paneel`).
- Klein: rechtsboven, `width:max-content`. Een `nldd-button appearance=primary
  start-icon=chevron-double-left` "Paneel openen", per tab een `nldd-icon-button` (huidige tab
  `accent-filled`), en daaronder "<tab> · RD x, y". Klein-of-niet onthouden in `localStorage`
  (in try/catch).
- Onder 800 px: kaart (60vh), legenda en paneel onder elkaar, allebei `position:static`, zonder
  schaduw; de pagina scrollt. (De referentie-app zet alleen het paneel onder de kaart, waardoor de
  legenda over het paneel valt; het sjabloon doet het zo beter.)

## 6. Tabs en hun inhoud

Vaste volgorde en iconen. Laat weg wat de casus (nog) niet heeft.

| Tab | Icoon | Inhoud |
|---|---|---|
| Op dit punt | `map-pin` | Welke plannen hier liggen, welk plan de regels geeft, wat erbovenop komt en wat opzij gaat, met de reden. |
| Toets | `checklist` | Kan het bouwplan hier? Kolommen per deel van het recht (bijv. tijdelijk en nieuw deel), eindoordeel, gerekende stappen. |
| Simuleren | `slider-horizontal-3` | Bouwplan instellen, uitkomst per variant (bijv. 1–15 lagen) met de kantelpunten, "Gegevens invullen" (gunstig / onbekend / ongunstig), knop "Toon de uitkomst op de kaart". |
| Testpunten | `list` | `nldd-list appearance=simple`, items als knop met `nldd-title-cell` (naam, waarom) en `nldd-tag` (testpunt = accent, voorbeeld = neutral). |
| Vergelijking (optioneel, bijv. "Met en zonder DSO") | `columns-2` | Delen via segmented control (Op dit punt, Simuleren, Scenario's, Kerncijfers). |
| Uitleg | `book` | Begrippen met een anker `uitleg-<begrip>`, zodat andere plekken ernaar kunnen springen. |

Lege staat van Op dit punt:

```html
<nldd-container layout="stack" gap="12">
  <nldd-title size="display-3" :heading-level="2" text="Klik op een plek in <gebied>"></nldd-title>
  <nldd-text>Je ziet dan … (één alinea: wat je ziet en hoe de rangorde werkt)</nldd-text>
  <nldd-text size="sm" color="secondary">Of kies een plek onder Testpunten. Verkenning, geen besluit.</nldd-text>
</nldd-container>
```

## 7. Patronen in het paneel

- **Kop van een plek**: `nldd-title size=display-5`, `overline` = "Geldend recht" of "Beoogd recht:
  ontwerp, geen geldend recht", `text` = naam of "Vak bij RD x, y", `supporting-text` = het bouwplan
  en de aannames in één zin.
- **Oordeel**: `nldd-banner size=sm` met variant per klasse (success / critical / warning / neutral),
  `supporting-text` = de reden in één zin.
- **Stappenlijst**: `nldd-list appearance=simple`, `nldd-list-item size=sm`, `nldd-text-cell`
  ("Regeling, artikel x") en rechts `nldd-cell width=fit-content` met `nldd-tag`. Klikbaar
  (`button`) als er een artikeltekst achter zit.
- **Kopjes binnen een blok**: `nldd-text size=sm` met `<strong>`, aantal tussen haakjes.
- **Bijzin of voorbehoud**: `nldd-text size=xs color=secondary`.
- **Twee kolommen naast elkaar** (vergelijken): grid 1fr 1fr, gap 16; elke kolom `border-top: 4px
  solid`, grijs `#8a8a86` voor de oude of eerste aanpak, lintblauw `#01689b` voor de nieuwe; elke
  kolom een eigen titel met `overline`. Onder 620 px paneelbreedte één kolom.
- **Groepen per overheid**: `<details>` per overheid (gemeente standaard open), daarin per regeling
  een grijze kop en een lijst.
- **Uitklap** (`Uitklap.vue`): voor trace, tabel per variant en andere grote blokken. Soort `sectie`
  (kader, md-titel) of `groep` (klein). Pas rekenen als het blok opengaat.
- **Trace / YAML**: `nldd-code-viewer wrap max-height="420px" language="text"` (of `yaml`).
- **Open punten**: `nldd-banner variant=warning size=sm`, één per punt.

## 8. Sheets

- **Over deze PoC**: `nldd-sheet placement=left width=560px`, `nldd-top-title-bar` met sluitknop in
  `slot="toolbar"`. Secties: Wat is dit?, Wat we willen leren, Zo gebruik je het (genummerde stappen
  langs de tabs), Hoe de simulatie rekent, Wat het niet is. "Wat we willen leren" is de leervraag van
  de PoC over RegelRecht zelf (schema, engine, werkwijze in dit domein), niet een bijvangst over een
  bron of een koppeling; die komt hooguit als tweede alinea ("Daarbij zien we ook ...").
- **Artikel**: `nldd-sheet placement=right width=640px`. Kop: titel van het artikel,
  supporting-text "overheid · regeling". Tags (rol, bundel, "als gegeven"), activiteit en
  kwalificatie, dan segmented control **Tekst | YAML**. Tekst met `white-space: pre-wrap`; YAML in
  een code-viewer met daarboven "Uit <bestand>: zo rekent de toets dit artikel."

## 9. Graaf

Schakelaar `nldd-segmented-control size=sm` linksboven (`top:10px; left:56px`, naast de
zoomknoppen, z 1200): **Kaart** (icoon `map`) | **Graaf** (icoon `centralized-network`). De graaf
(vue-flow) ligt over de hele kaart (z 1100); de kaart blijft eronder bestaan. In de graaf: alle
regels en verwijzingen achter de uitkomst op het gekozen punt, ook de opties die afvallen
(plannen, bestemmingen, takken), afvallers lichter.

## 10. Presentatie

Optioneel, voor demo's die verteld worden. Een schermvullend deck in de app zelf
(`PresentatieDeck.vue`): knoppen Vorige / Volgende / Sluiten (`nldd-icon-button`), pijltjestoetsen,
dia-nummer in de URL, stappen binnen een dia. Eerste dia de vraag, laatste dia wat open is.

## 11. Kleuren

Zie `assets/sjabloon/src/kleuren.js`. Samengevat:

| Wat | Kleur |
|---|---|
| Toegestaan | `#0ca30c` |
| Onthouding: er ontbreekt een gegeven | `#fab219` |
| Onthouding: bron alleen als scan | `#8c6d1f` |
| Niet toegestaan | `#d03b3b` |
| Rekent hier niet | `#8a8a86` |
| Bestemmingsplan (lijn) | lintblauw `#01689b` |
| TAM-plan / omgevingsplan nieuw (lijn) | donkergroen `#275937`, ontwerp gestippeld |
| Facetplan | rood `#d52b1e` |
| Zoekgebied, testpunten | donkerblauw `#154273` |
| Bestemming op het punt | oranje `#e17000` |
| Gekozen punt | geel `#ffb612`, zwarte rand |
| Tweede reeks punten | violet `#42145f` |
| Hoogte (5 klassen, licht naar donker) | `#86b6ef` `#5598e7` `#2a78d6` `#1c5cab` `#104281` |
| Functie | wonen `#2a78d6`, gemengd `#1baf7a`, andere functie `#6250d6`, groen/water/verkeer `#8a8a86` |

Tags en banners: toegestaan = success, niet = critical, onthouding = warning, open = neutral.

## 12. Teksten

| Plek | Tekst |
|---|---|
| Subtitel | "Verkenning · <vraag>" |
| Lege staat | "Klik op een plek in <gebied>" |
| Voetregel en lege staat | "Verkenning, geen besluit." |
| Recht | "Geldend recht" / "Ontwerp-…" ; overline "Beoogd recht: ontwerp, geen geldend recht" |
| Klassen | "Toegestaan", "Niet toegestaan", "Onthouding: er ontbreekt een gegeven", "Rekent hier niet: geen plan of geen regels" |
| Zoekgebied | "Zoekgebied (afgeleid, geen officiële grens)" |
| Paneelknoppen | "Paneel openen", "Paneel minimaliseren", "Uitleg", "Sluiten" |
| Laden | "De gegevens van deze verkenning laden…", "Engine laden…", "<iets> rekenen" (voortgangsbalk) |
| Fout | "<Wat> niet geladen: <reden>" |

Getallen met een komma (3,2 m), duizendtallen met een punt (1.266 vakken).
