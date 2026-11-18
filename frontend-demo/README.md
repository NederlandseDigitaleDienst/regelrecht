# RegelRecht demo

De demo-werkruimte van RegelRecht: presentatie, wettenbrowser, afhankelijkheidsgraaf,
scenario-runner, simulatie, burger-/ondernemersportaal en zaaksysteem. Opvolger van de losse
`poc-machine-law`-repository; bestemd voor `demo.regelrecht.rijks.app`.

## Architectuur

- **Geen backend.** De regelrecht-engine draait als WebAssembly in de browser
  (`public/wasm/pkg`, gebouwd door `just wasm-build` of de Dockerfile).
- **Corpus** in `corpus/demo/`: wetten (schema v0.5.8), scenario's, `bindings.yaml`,
  `profiles.yaml`, `demo-config.yaml`, `services.yaml`. `scripts/copy-demo-corpus.mjs`
  kopieert het bij `predev`/`prebuild` naar `public/data` en schrijft `index.json`.
- **Data**: `src/data/materialize.js` past de bindings toe op de persona-tabellen en
  levert per wet records op die als wet-gebonden databron in de engine gaan
  (`engine.registerDataSourceForLaw`). Correcties van de burger komen als bron
  `correcties` met hogere prioriteit bovenop. Dezelfde module gebruikt de
  scenario-converter (`corpus/demo/tools/convert_features.mjs`).
- **Aanvragen** gebeuren in het portaal (`ApplicationSheet.vue`), just in time zoals de
  POC: `src/data/askedInputs.ts` bepaalt uit de laatste uitvoeringstrace welke input die
  alleen de burger kent de engine daadwerkelijk tegenkwam, vraagt die één voor één en
  laat de engine na elk antwoord opnieuw rekenen. Antwoorden worden claims (`selfDeclared`)
  op de wet, gesleuteld op de identiteit van die wet (`kvk_nummer` voor een bedrijfswet), en
  gelden ook als parameters bij het materialiseren, zodat een terraslocatie de juiste
  registerrij vindt. Welke vraag aan de beurt is, zegt de uitkomst zelf: een onbekende
  uitkomst draagt de ontbrekende feiten (RFC-036), en het portaal vraagt precies die,
  in de volgorde waarin de engine ze tegenkwam. Een nog niet beantwoorde vraag wordt
  niet als `null` meegegeven: `null` zou "er is geen" betekenen.
- **Toestand** (profiel, aanvragen, correcties) in `src/store/demoStore.ts`, bewaard in
  `localStorage`; "Demo resetten" in het menu wist het.
- **Home** (`src/views/HomeView.vue`): de landingspagina op `/`. Wie de demo opent zonder
  te weten wat het is, leest daar in een paar regels wat er te zien valt, start de
  presentatie, en vindt de weg naar regelrecht.rijks.app, de documentatie en het onderzoek.
  De QR-code ernaast wijst naar de pagina zelf (`window.location.origin`, dus ook goed op
  een preview-deploy of een laptop in de zaal), zodat iemand meekijkt op zijn telefoon.
  In de tabbalk draagt Home alleen zijn icoon; de presentatie staat sindsdien op
  `/presentatie` en start niet meer vanzelf bij binnenkomst.
- **Presentatie** (`src/presentation/`): de dia's uit `demo-config.yaml` (`slides:`) als
  overlay; een dia met `route` opent dat tabblad, wisselt zo nodig van persona (`profile`)
  en wijst een deel van het scherm aan (`highlight`). Shift+P opent het dek overal.
- **Graaf** (`src/graph/lawGraph.ts`, `src/components/graph/`): per wet een kader met
  bronnen, invoer en uitvoer, lijnen van invoer naar de leverende uitvoer, de waarden van
  de persona uit een evaluatie plus de trace. Alle wetten worden gelegd, alleen de
  selectie (profiel: `graph_laws`) met haar directe buren is zichtbaar, zodat "Alles"
  niets verschuift. Een profiel met `graph_focus` zet die wet bij de wissel in focus
  en zoomt erop in; Regelwerken en Scenario's openen dan `default_law` en `default_feature`. `LawGroupTree.vue` is de lijst per organisatie die Regelwerken en
  Graaf delen; de lijsten zijn zijpanelen (`primary-sidebar-as-sheet`), standaard dicht.
- **Scenario's** draaien met de gedeelde Gherkin-runner uit
  `@regelrecht/frontend-shared/gherkin` (canonieke grammar); `src/data/gherkinNl.ts`
  geeft de stappen in het Nederlands weer.
- **Simulatie** (`src/simulation/`): een gegenereerde populatie burgers of bedrijven
  (`population.ts`, seeded) in dezelfde tabelvorm als `profiles.yaml`, door de
  materialiser en de engine gehaald voor elke regeling van het portaal
  (`runner.ts`), samengevat en uitgesplitst (`stats.ts`). Constanten uit de
  `definitions` van een wet zijn per run aan te passen (`lawParameters.ts`): de
  wet wordt met gewijzigde waarden herladen en na de run teruggezet. De
  simulatie vervangt tijdelijk de persona-data in de engine en zet die daarna
  terug. Een wet die een andere wet als besloten zaak leest (`kind: cases`,
  zoals precario de terrasvergunning), krijgt eerst de aanvragen van de
  populatie (`aanvragen` in de formulierwaarden); wat die wet toekent, wordt een
  zaak. Grafieken met echarts, zoals in de editor.

## Draaien

```bash
just demo            # WASM bouwen, Vite op :7400, browser open
just dev-demo        # hetzelfde zonder browser
just demo-check      # alles controleren: wetten, scenario's, tests, WASM, build
just bdd-demo        # alleen de demo-scenario's, natively met cargo
just validate-demo   # alleen de wetten: schema en typecontrole
```

## Afwezig en onbekend (RFC-036)

De engine kent twee soorten "niets". `null` is afwezigheid: het register is
gezaghebbend en zegt dat er niets is (geen partner, geen huur, geen vergunning);
de wet toetst dat met `EQUALS … null` en rekent er niet mee. Onbekend is een
feit dat bestaat maar dat niemand heeft aangeleverd; de engine geeft dan een
onbekende uitkomst die benoemt welke feiten ontbreken en waarom (`no_data`: een
registerinput zonder waarde; `not_passed`: een optionele parameter die de
aanroeper wegliet). Niets wordt stilzwijgend ingevuld.

De materialiser (`src/data/materialize.js`) schrijft daarom alleen wat de data
zegt. Wat een ontbrekende registerrij betekent, staat per binding in
`corpus/demo/bindings.yaml` onder `absent:` (zie de kop van dat bestand):
`unknown` laat de sleutel weg (de engine meldt de input als ontbrekend feit),
`null` schrijft een afwezigheid, `0` een telling van niets (de Belastingdienst
kent geen loon: het loon is 0). Een `kind: claim`-input bestaat pas als de
burger hem opgeeft en wordt tot die tijd weggelaten. Een opzoeking op een
afwezige sleutel (het inkomen van een partner die er niet is) is `null`, wat
`absent` ook zegt: er is niemand om op te zoeken. Records bevatten nooit
`undefined`; de WASM-grens zou dat als `null` lezen.

In de weergave heet `null` "geen" en een onbekende waarde "onbekend", met waar
ruimte is "ontbreekt: …" (`src/data/format.ts`). Een onbekend
`voldoet_aan_voorwaarden` is nooit een ja: de tegel zegt "Nog niet te bepalen",
een aanvraag met een onbekende uitkomst gaat naar de behandelaar, en in de
simulatie telt zo'n uitkomst apart ("onbekend") en maakt hij het besteedbaar
inkomen van die burger onbekend in plaats van 0.

De scenario's onder `corpus/demo/regulation/**/scenarios/` volgen dezelfde
regel: een lege cel is een weggelaten sleutel (onbekend), het woord `null` is
een afwezigheid. `corpus/demo/tools/apply_absent_semantics.mjs` heeft de
gegenereerde tabellen daarop herschreven; zie `corpus/demo/tools/CONVERSION_NOTES.md`.

## Eigen CSS bovenop het design system

`src/css/main.css` bevat, naast een box-sizing/body-reset, alleen hooks waarvoor het
design system geen component heeft:

| Selector | Waarom |
|---|---|
| `src/presentation/*` (deck, scoped CSS en `presentation.css`) | De presentatie: een Rijkshuisstijl-blauw dek (`lintblauw-750` → `-700`, RijksoverheidSerif voor titels, RijksSans voor tekst; alle kleuren als token, met `color-scheme: light` omdat de palet-schalen tussen licht en donker omkeren en een presentatie één verschijning heeft) dat voluit staat bij intro en afsluiting en als linker rail de demo rechts aanstuurt (`html.rr-presenting body { padding-left }`, puls `.rr-present-pulse`). Naar het voorbeeld van Begane Grond. Het design system heeft geen presentatiecomponent; de serif-fonts staan in `public/fonts` (Rijkshuisstijl-licentie). |
| `.org-logo` | Organisatielogo in een blokje met padding (40px/24px). `nldd-avatar` snijdt een afbeelding bij (`object-fit: cover`), wat een woordmerk afsnijdt; `nldd-image` vult altijd de volle breedte. Organisaties zonder logo krijgen wél een `nldd-avatar` met initialen. |
| `.yaml-tree*` | Opvouwbare YAML-boom met kruiswet-links; `nldd-code-viewer` highlight wel YAML maar vouwt niet en kent geen links. |
| `.gherkin*` | Gherkin-weergave met slaag/faal-markering per stap (de datatabellen zijn `nldd-table`); de viewer kent Gherkin als taal maar geen stapstatus. |
| `.trace` | Monospace box-drawing-trace. |
| `.qr-frame` en `svg.qr` (`views/HomeView.vue`, `components/QrCode.vue`) | De QR-code op de landingspagina. Het design system heeft geen QR-component, en een QR-code is geen stijlkeuze maar een tekening met een vaste betekenis: de modules moeten donker op licht staan, óók in donkere modus, dus de code tekent zijn eigen witte vlak in plaats van de ondergrond te erven. Het kadertje eromheen zet alleen een maximale maat en een radius. |
| `.graph-canvas`, `.graph-law*`, `.graph-box*`, `.graph-item*`, `.graph-dim`, `.graph-zoom` | vue-flow heeft een expliciete hoogte nodig; de knopen tekenen het POC-beeld (wet als kader met vakken voor bronnen, invoer en uitvoer, met de waarde voor de persona) en dimmen wat buiten de selectie valt. Een graafcanvas bestaat niet in het design system (zelfde uitzondering als de editor). De vlakken komen uit de oppervlakte-tokens, de organisatiekleur zit alleen in rand en label. `.graph-zoom` plaatst alleen de knopgroep over het canvas; de knoppen zelf zijn `nldd-icon-button`, want de zoomknoppen van vue-flow brachten hun eigen stylesheet en vormgeving mee. |

Eén afwijking buiten CSS: `App.vue` roept na elke routewissel `_evaluateScrollMode()` van
`nldd-shell` aan. Het design system leidt bij het koppelen af of het document of elk
paneel scrolt, maar onze split view komt pas later (lazy routes) en de afleiding blijft dan
op "document". Zonder die aanroep scrollen de panelen niet zelf en blijven de kopregels
niet staan.
