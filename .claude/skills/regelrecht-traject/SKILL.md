---
name: regelrecht-traject
description: Voordeur voor het werken in een regelrecht-traject. Gebruik dit aan het begin van een traject, of bij twijfel waar te beginnen. Beschrijft de cyclus waarin een vertaling tot een geattesteerde versie komt (werkronde → uitspraak → verwerken → attestatie) en zegt welke skill op welk moment aan zet is. Routeert naar regelrecht-stelselanalyse (desk), regelrecht-audit-products (sessie) en regelrecht-verantwoording (toelichting, uitspraak en attestatie). Casus-agnostisch.
allowed-tools: Read, Glob, Grep, AskUserQuestion
---

# Regelrecht traject — voordeur

Een traject is de eenheid waarin een wet machine-leesbaar wordt gemaakt. In de editor heeft
het een eigen branch, een eigen corpusconfiguratie, een annotaties-sidecar, taken, één
gedeelde pull request en leden met een rol. Het eindpunt van een traject is een
**geattesteerde versie**: het gedrag is vastgelegd in scenario's, en elk artikel met
interpretatieruimte heeft een toelichting waarover de bevoegde uitspraak deed. Samen zijn dat
de moderne, artikelsgewijze memorie van toelichting. Deze skill doet zelf geen werk. Hij zegt
welke skill wanneer aan zet is.

## De cyclus

| moment | wie | wat gebeurt er | skill |
|---|---|---|---|
| **werkronde** | model, analist | tekst wordt YAML; interpretatiekeuzes worden claims, vastgelegd in scenario's; feitelijke fouten worden gefixt | `law-interpret`, `regelrecht-stelselanalyse` |
| **uitspraak** | de bevoegde | bekrachtigt, corrigeert, weerlegt of laat met reden open; in de editor of in een sessie | editor, `regelrecht-audit-products` |
| **verwerken** | analist | het corpus volgt de uitspraak; een correctie wordt een nieuwe claim | `regelrecht-stelselanalyse` |
| **attestatie** | de bevoegde | versie H is productiewaardig: gedrag groen, elk toelichtingsartikel gedekt | `regelrecht-verantwoording` (Attestatie) |

```
 werkronde ──▶ uitspraak ──▶ verwerken ──▶ attestatie ──▶ volgende versie
     ▲                           │
     └──────── gecorrigeerd ─────┘
```

Wie een claim vastlegt, sluit haar niet zelf: daarvoor is een uitspraak van een ander nodig.
De bevoegde kan dat asynchroon doen in de editor. Een sessie is er voor wat asynchroon niet
lukt: doorpraten als het antwoord "het hangt ervan af" is, samen scenario's bouwen, veel
punten tegelijk sluiten.

`regelrecht-verantwoording` is geen moment in de cyclus. Je pakt die skill telkens wanneer
een keuze wordt gemaakt of gesloten. Hetzelfde geldt voor `regelrecht-scenario-traces`
(casuïstiek en ketens) en `law-wetsgeschiedenis` (waar een bepaling vandaan komt).

## Wie sluit een open punt?

| niveau | wie | wanneer |
|---|---|---|
| desk | de analist: modelfout, ontbrekende bron, of een lezing als werkhypothese | werkronde |
| bevoegde | uitvoering (feit, praktijk) of jurist (lezing) | uitspraak |
| buiten het traject | bestuur of normsteller | gaat met reden mee naar de volgende cyclus |

Het niveau wordt afgeleid, niet gekozen. Zie `regelrecht-verantwoording`, stap 2.

## Soort bevinding

Het label zegt wat een bevinding *is* en welke actie volgt. Wie haar mag sluiten volgt uit de
tabel hierboven. Definities: `regelrecht-stelselanalyse/references/classification.md`.

| label | actie | landt in |
|---|---|---|
| modellering-fout | desk fixt de YAML | `modellering-fixes-plan` |
| engine-limitatie | desk trackt, engine-issue | `engine-limitaties` |
| wetgevings-fout | desk documenteert; de normsteller is bevoegd | `wetgevingsfouten-analyse` |
| acceptabele untranslatable | een claim met een bevoegde; rekent door als werkhypothese | annotaties-sidecar |

Kort: een feitelijk defect lost de desk zelf op, een oordeel wacht op de bevoegde.

## Waar begin je

| situatie | start |
|---|---|
| geen corpus, of een ruw corpus | werkronde: `law-interpret` |
| corpus bestaat maar is niet gevalideerd | werkronde: `regelrecht-stelselanalyse` |
| claims wachten op een uitspraak | de bevoegde in de editor; wat blijft liggen gaat naar de sessie |
| je mist domeinkennis of buy-in | een verkennende sessie (mag vroeg) |
| na een uitspraak | verwerken: `regelrecht-stelselanalyse` |
| een versie moet naar productie | de attestatie in `regelrecht-verantwoording` |
| de wet of het model is gewijzigd | *Als het regelwerk verandert* in `regelrecht-verantwoording` |

Alleen het laatste punt uit de triage kun je niet uit het corpus afleiden: heb je
domeinkennis of buy-in nodig die je nog niet hebt? Twijfel je, vraag het dan met
`AskUserQuestion` voordat je routeert.

## Twee soorten sessies

- **Verkennend**: vroeg, ook op een ruwe scope-analyse. Domeinkennis ophalen, scope
  bekrachtigen. Geen voorwaarden vooraf.
- **Validerend**: de bevoegden spreken zich uit over wat nog openstaat. Voorwaarden: het
  schema valideert, de tests zijn groen, de modelleringsfouten zijn gefixt, en wat overblijft
  is een oordeelsvraag. Experts valideren geen fouten die wij zelf hadden moeten vinden.

## Wat elk moment achterlaat

Skills praten niet met de editor-api. Ze laten bestanden achter in de repo, en de editor leest
dezelfde bestanden.

| moment | laat achter | waar |
|---|---|---|
| werkronde | wet-YAML, claims, scenario's met `@claim:<id>` | `regulation/…`, `annotations/{law_id}/annotations.yaml`, `…/scenarios/*.feature` |
| uitspraak | een `replying`-notitie | dezelfde sidecar |
| verwerken | gewijzigde YAML en de claim die zegt waarom | corpus en sidecar |
| attestatie | een annotated tag `attestatie/<law_id>/<valid_from>` | de commit met versie H |

Voor een nieuwe regelanalist: `references/zakkaart.md`, één A4.
