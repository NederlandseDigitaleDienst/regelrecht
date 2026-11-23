# Routing — de regelrecht dossier-flow (canoniek)

Eén bron van waarheid voor hoe `regelrecht-stelselanalyse` (desk) en
`regelrecht-audit-products` (workshop) samenhangen. Beide werk-skills verwijzen hierheen.

## Twee lagen, geen alternatieven

- **Desk (machinekamer)** — `regelrecht-stelselanalyse`. Harvest → modelleer → valideer →
  **4-weg classificeer** → fix → documenteer → verifieer. Draait eerst en continu.
- **Workshop (validatielaag)** — `regelrecht-audit-products`. Valideert met domein-experts.
  Levert consensus, correctiepunten en testcases.

## De flow (gesloten lus)

```
   ┌────────────── DESK · regelrecht-stelselanalyse ──────────────┐
   │  harvest → modelleer → valideer → 4-weg classificeer → fix   │ ◀──┐
   └───┬──────────────────────────────────────────────────────────┘    │
       │  modellering-fout → fix · engine-limitatie → engine           │
       │  wetgevings-fout → notitie                                    │
       ▼                                                               │
   ┌──────────── WORKSHOP · regelrecht-audit-products ────────────┐    │
   │  scope-analyse → audit-doc → sessie → correctiepunten +      │    │
   │  consensus + testcase-scenario's                             │ ───┘
   └──────────────────────────────────────────────────────────────┘
       omlaag: corpus workshop-rijp (validerend) of bewust ruw (verkennend)
       omhoog: correctiepunten + bevestigde interpretaties → implementeer + verifieer
```

## De router-taal: vier-weg-classificatie

Het label van een bevinding bepaalt de bestemming (zie
`regelrecht-stelselanalyse/references/classification.md` voor de definities):

| Classificatie | Route | Landt in |
|---|---|---|
| **modellering-fout** | desk fixt de YAML | `modellering-fixes-plan` |
| **engine-limitatie** | desk trackt; engine-issue | `engine-limitaties` |
| **wetgevings-fout** | desk documenteert → wetgevings-notitie (stakeholders) | `wetgevingsfouten-analyse` |
| **acceptabele untranslatable / interpretatie-/praktijk-onzeker** | **workshop** | beslispunten in `audit-doc`/`workshop-draaiboek` |

In één regel: **feitelijke defecten naar binnen (desk), oordeels-/praktijkvragen naar
buiten (workshop).**

## Entry-router (waar begin je?)

| Situatie | Start |
|---|---|
| Geen corpus, of ruw/onvolledig | desk — harvest + modelleer |
| Corpus bestaat, modellering onzeker/ongevalideerd | desk — valideer + classificeer + fix |
| Corpus intern consistent; open punten zijn judgment/praktijk | workshop — validerend |
| Je hebt domeinkennis/buy-in nodig die je mist | workshop — verkennend (mag vroeg) |
| Na een workshop | terug naar desk — implementeer + verifieer |

## Twee workshop-modi (gate is mode-afhankelijk)

- **Verkennend / ontginnend** — vroeg, zelfs op een ruwe scope-analyse. Doel:
  domeinkennis ontginnen, praktijk ophalen, scope bekrachtigen. **Geen gate.**
- **Validerend** — een gemodelleerd corpus toetsen. **Gate**: schema valide + tests
  groen + modellering-fouten gefixt + resterende open punten zijn *judgment* (niet
  *factual*).

Het gate-criterium beschermt alleen de validerende modus: laat experts niet valideren
wat eigenlijk onze modelleerfout is. Voor kennis-ontginning is een ruw corpus prima.

## Handoff-lus — wat stroomt welke kant op

**Desk → workshop**
- een rijp (validerend) of bewust-ruw (verkennend) corpus;
- de *judgment*-set untranslatables wordt de **beslispunten** van de workshop;
- de scope-analyse/wet-graph komt uit de desk-producten `cross-law-diagram` +
  `corpus-status`.

**Workshop → desk**
- correctiepunten → modellering-fixes (implementeren + valideren);
- bevestigde/weerlegde interpretaties → resolutie-tracker (verifiëren tegen bronnen);
- door experts bevestigde wet-tekortkomingen → wetgevings-fouten-analyse;
- expert-bevestigde testcases → engine-tests (regressie).

## Gedeelde bruggen

| Brug | Desk-kant | Workshop-kant |
|---|---|---|
| Scope/relaties | `cross-law-diagram`, `corpus-status` | `scope-analyse` (hergebruikt) |
| Untranslatables | factual vs judgment geclassificeerd | judgment-set = beslispunten |
| Scenario's | `engine-tests` (regressie) | `testcase-scenarios` (expert-validatie) |
| Verslag/rapport | `eindrapport` (per cyclus) | `verslag-intern`/`-extern` (per sessie) |

Beide scenario-bruggen zijn gebouwd met de techniek-skill **`regelrecht-scenario-traces`**
(casus vindbaar via persona's/assen; keten leesbaar/geassert via keten-checkpoints +
golden-traces). Het is geen vierde laag in de flow maar een gedeelde methode die desk
(`engine-tests`, de features-vs-YAML meta-check) en workshop (`testcase-scenarios`) allebei
gebruiken.
