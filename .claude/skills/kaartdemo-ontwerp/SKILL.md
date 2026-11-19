---
name: kaartdemo-ontwerp
description: >
  Het vaste ontwerp voor demo's, PoC's en apps in de fysieke leefomgeving (omgevingsplan,
  bestemmingsplannen, DSO, bouwen of gebruik op een plek): een volle kaart met de grijze
  PDOK-achtergrond, een half doorzichtig paneel rechts met tabs (Op dit punt, Toets, Simuleren,
  Testpunten, Uitleg), kaartlagen en legenda linksonder, een schakelaar Kaart | Graaf, sheets voor
  "Over deze PoC" en de tekst van een artikel, en een voetregel met de engine. Vue 3, Leaflet en het
  NLDD design system. Gebruik deze skill zodra er in het fysieke domein een app, demo, poc, kaart of
  scherm gebouwd of uitgebreid wordt: "maak een demo voor deze plek", "bouw een kaart met de regels",
  "nieuwe poc voor gemeente X", "zet de toets in een app", "voeg een tab of kaartlaag toe", "maak het
  net zo als de poc Regels op een plek", ook als niemand het ontwerp noemt. Niet voor het
  verzamelen van de regels op een plek, en niet voor git-stappen (die staan in AGENTS.md).
---

# Kaartdemo fysieke leefomgeving

Elke demo in het fysieke domein krijgt dezelfde opbouw als de poc "Regels op een plek"
(`frontend-poc-regels-op-een-plek`). Wie er één heeft gezien, kan de volgende meteen lezen. Deze skill beschrijft die
opbouw en levert een sjabloon om hem te maken.

## Waarom dit ontwerp

- **De plek is de ingang.** Alles begint met een klik op de kaart, of met een testpunt. De kaart
  blijft altijd in beeld; het paneel ligt eroverheen in plaats van ernaast.
- **Elk oordeel is terug te volgen.** Banner met het oordeel, daaronder de gerekende artikelen met
  een tag per artikel, een klik op een artikel opent de tekst, en daarachter de YAML en de trace.
  Een uitkomst zonder weg terug naar het artikel hoort niet in de demo.
- **Onthouding is een gewone uitkomst.** Toegestaan, niet toegestaan, onthouding (er ontbreekt een
  gegeven) en rekent hier niet: altijd deze vier, altijd met dezelfde kleuren en altijd met tekst.
- **Verkenning, geen besluit.** Dat staat in de lege staat, in de voetregel en bij beoogd recht.
  Geldend en beoogd recht (ontwerp) staan nooit door elkaar zonder label.

## De opbouw

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ [i Over deze PoC] [▶ Presentatie]      Gebied · Verkenning · vraag           │  nldd-top-navigation-bar
├──────────────────────────────────────────────────────────────────────────────┤
│ ┌──────────────────────────────────────────────────────────────────────────┐ │
│ │ [+]  [Kaart|Graaf]                      ┌──────────────────────────────┐ │ │
│ │ [-]                                     │ Op dit punt│Toets│Simuleren… │ │ │  paneel: glas,
│ │                                         │              [Uitleg] [»]    │ │ │  rechts over de kaart
│ │        PDOK BRT grijs                   │──────────────────────────────│ │ │
│ │        plangrenzen, testpunten P1..     │ Titel (display-5)            │ │ │
│ │        gekozen punt (geel, zwart rand)  │ [banner: oordeel]            │ │ │
│ │                                         │ lijst: artikel ····· [tag]   │ │ │
│ │ ┌───────────────────────┐               │ ▸ Uitklap: trace             │ │ │
│ │ │ Kaartlagen            │               │                              │ │ │
│ │ │ [Plan|Hoogte|Functie…]│               │                              │ │ │
│ │ │ ▭ legenda (aantallen) │               │                              │ │ │
│ │ │ ☐ Bouwvlakken         │               │                              │ │ │
│ │ │ ▸ Overige tekens      │               └──────────────────────────────┘ │ │
│ │ └───────────────────────┘ bron PDOK                                      │ │
│ └──────────────────────────────────────────────────────────────────────────┘ │
├──────────────────────────────────────────────────────────────────────────────┤
│ RegelRecht-engine x.y, commit …, N regelingen geladen. Verkenning, geen besluit│  voetregel
└──────────────────────────────────────────────────────────────────────────────┘
```

Onder 800 px breed schuift het paneel onder de kaart. Het paneel kan klein (alleen iconen
rechtsboven), en wordt breder voor tabs met veel inhoud (Toets, Simuleren, Uitleg, vergelijkingen).

## Zo maak je het

1. **Waar komt de app.** In deze repo: eerst de skill `poc-add` (register, map
   `frontend-poc-<slug>`, subpad, deploy); branch en PR volgens `AGENTS.md`. Daarna dit ontwerp
   erin. In een losse repo: dezelfde mapnaam en opbouw, zodat de app later zonder verbouwing hierheen
   kan.
2. **Kopieer het sjabloon** uit `assets/sjabloon/` naar de app-map. Vervang elke `%%...%%`
   (zoek erop met `grep -rn '%%' .`). Het sjabloon draait zonder engine en zonder
   gegevens: een kaart, een leeg paneel, een legenda. Bouw van daaruit verder.
3. **Vul per casus** (zie "Wat per casus verschilt"): gebied en zoekgebied, plannen en hun kleuren,
   kaartthema's, testpunten, tabs.
4. **Bouw de tabs** in de vaste volgorde en vorm uit `references/onderdelen.md`. Begin met Op dit
   punt en Testpunten; Toets en Simuleren pas als er een engine en regels zijn.
5. **Controleer** met de lijst onderaan, en bekijk het in de browser (skill `run`). Een scherm dat
   niet in de browser is bekeken, is niet af; zeg dat dan ook.

Voor details per onderdeel (maten, kleuren, componenten, teksten): lees
`references/onderdelen.md` voordat je een onderdeel bouwt of wijzigt.

## Vaste regels

- **NLDD-componenten** (`nldd-*`) voor alles wat een component heeft: knoppen, tabs, segmented
  controls, banners, lijsten, tags, titels, sheets, code-viewer. Geen eigen knoppen of tabbalken.
  Eigen elementen alleen waar NLDD niets heeft: `Uitklap.vue` (geen accordeon in NLDD), de
  legendatekens en de labels op de kaart. Gebruik alleen slots die het component echt heeft
  (de monorepo controleert dat met `script/check-nldd-slots.mjs`).
- **Booleans in NLDD** als `:selected="x || undefined"`, nooit `false` doorgeven (anders staat het
  attribuut er toch).
- **Kleuren** uit `src/kleuren.js`: de vier uitkomstklassen, rijkshuisstijl voor lijnen, de
  hoogteklassen. Statuskleur nooit zonder tekst in de legenda.
- **Achtergrondkaart**: PDOK BRT Achtergrondkaart grijs, tegels extra ontkleurd met een CSS-filter,
  donkerder in dark mode. Bronvermelding linksonder, want rechts ligt het paneel.
- **Coördinaten in RD** (EPSG:28992) naar buiten: in de deellink, in de koppen ("Vak bij RD x, y"),
  in testpunten. Leaflet rekent intern in WGS84.
- **Deellinks**: `?rd=x,y`, `?thema=`, `?tab=`, `?weergave=graaf`, via `history.replaceState`.
  Standaardwaarden laat je weg uit de URL.
- **Light en dark** volgen het OS: `color-scheme: light dark` en `color-mix()` met
  NLDD-tokens als achtergrond, nooit een vaste witte achtergrond.
- **Taal**: Nederlands, nuchter, B1. Werkwoorden in knoppen ("Paneel openen"), zinnen in legenda's.
  Geen "AI", geen "slim". Zie de tekstlijst in `references/onderdelen.md`.

## Wat per casus verschilt

| Wat | Waar |
|---|---|
| Naam, subtitel, vraag (bijv. "kan hier een woontoren van 5 lagen?") | `App.vue`, `index.html` |
| Startbeeld en zoekgebied (GeoJSON, gestippeld donkerblauw) | `KaartTab.vue` |
| Plansoorten en hun lijnkleur | `KLEUR_PLAN` in `kleuren.js` |
| Kaartthema's (Plan, Hoogte, Functie, Toets, Uitkomst, …) | `THEMA` in `KaartTab.vue` |
| Testpunten P1..Pn en voorbeeldplekken | `data/testpunten.json` |
| Tabs die meedoen | `TABS` in `KaartTab.vue` |
| Herkomst van de gegevens (bron achter het portaal of `public/`) | `main.js` |

## Controle voor je klaar bent

- `npm run build` slaagt; in de monorepo ook `check-nldd-imports` en `check-nldd-slots`.
- Lege staat: zonder gekozen punt zegt het paneel wat je moet doen ("Klik op een plek in …").
- Laden mislukt: een `nldd-banner variant="critical"` met de reden, geen lege pagina.
- Deellink `?rd=` opent op het punt, met het punt links van het paneel in beeld.
- Paneel klein en weer groot; smal scherm (< 800 px); dark mode.
- Elke kleur op de kaart staat met tekst in de legenda, met aantallen waar die er zijn.
- Elk oordeel leidt naar artikelen; elk artikel naar zijn tekst.

## Referentie

De bron van dit ontwerp is de app `frontend-poc-regels-op-een-plek` in deze repo. Kijk daar voor
uitgewerkte voorbeelden, onder `src/components/`: `KaartTab.vue` (kaart, legenda, paneel),
`PuntPaneel.vue` (twee kolommen naast elkaar), `ArtikelSheet.vue`, `PresentatieDeck.vue`
(presentatie in de app); en onder `src/eerste/` de toets, het simuleren en de graaf met vue-flow.
Neem het ontwerp over, niet de gegevens: de regels, de kaart en de uitkomsten van die poc staan in
een afgeschermde bron en horen niet in deze repo. Deze skill zelf noemt geen casus; houd dat zo.
