---
name: regelrecht-traject
description: Front-door router voor het werken in een regelrecht-traject. Gebruik dit aan het begin van een traject, of bij twijfel over waar te beginnen — het triageert de situatie en routeert naar de juiste skill (regelrecht-stelselanalyse voor desk-review/corpus-completion, regelrecht-audit-products voor live expert-validatie) en de juiste volgorde. Bevat het routeringsprincipe: feitelijke defecten routeren naar de desk, oordeels-/praktijkvragen naar de workshop. Dossier-agnostisch.
allowed-tools: Read, Glob, Grep, AskUserQuestion
---

# Regelrecht traject — router & flow

De ingang voor het werken aan een regelrecht-traject. Bepaalt **waar je begint** en
**hoe de twee werk-skills in één flow samenhangen**. Dit is een dunne dispatcher: het
eigenlijke werk gebeurt in de twee skills waarnaar het routeert.

- **`regelrecht-stelselanalyse`** — de machinekamer (desk): harvest, modelleer, valideer,
  classificeer, fix, documenteer, verifieer. Levert een corpus + geclassificeerde
  bevindingen.
- **`regelrecht-audit-products`** — de menselijke validatielaag (live): valideert met
  domein-experts. Levert consensus + correctiepunten + testcases.
- **`regelrecht-scenario-traces`** — gedeelde techniek-laag (geen aparte fase): maakt
  casuïstiek vindbaar (persona's/assen) en norm-ketens leesbaar/geassert (keten-checkpoints
  + golden-traces). Wordt door beide werk-skills gebruikt bij het schrijven van scenario's
  en het reviewen van traces.
- **`law-wetsgeschiedenis`** — gedeelde bron-laag (geen aparte fase): de wetsgeschiedenis
  per artikel en lid uit de Kamerstukken, met herkomst, versies door de tijd en
  toelichting, elk met het stuk om te citeren. De desk gebruikt het bij het verifiëren en
  classificeren, de workshop om experts te laten zien hoe een bepaling zo is gekomen.

De volledige flow, het gate-criterium en de handoff-lus staan in `references/routing.md`
(de canonieke bron; beide werk-skills verwijzen hierheen). Voor het inwerken van nieuwe
regelanalisten: `references/zakkaart.md` — een printbare één-A4 met de drie deuren, de
flow, de 4-weg-classificatie en de beslis-kaart (methode bekend verondersteld).

## Het routeringsprincipe in één regel

**Feitelijke defecten routeren naar binnen (desk); oordeels-/praktijkvragen routeren naar
buiten (workshop).** De vier-weg-classificatie van een bevinding bepaalt de bestemming:

| Classificatie | Route |
|---|---|
| modellering-fout | desk → wij fixen de YAML |
| engine-limitatie | desk trackt → engine-issue |
| wetgevings-fout | desk documenteert → wetgevings-notitie (stakeholders, niet de validatieworkshop) |
| acceptabele untranslatable / interpretatie- of praktijk-onzeker | **workshop** |

Canonieke definities + de open-norm-beslisregel: `regelrecht-stelselanalyse/references/classification.md`
(één bron van waarheid; deze tabel is de routing-samenvatting).

## Triage: waar begin je?

Stel (of leid af) een paar dingen vast, dan volgt de route:

1. **Is er al een corpus?** Nee → desk: harvest eerst (`regelrecht-stelselanalyse`).
2. **Is het corpus gemodelleerd en gevalideerd?** Nee → desk: valideer + classificeer + fix.
3. **Wat is de openstaande vraag — feitelijk of oordeel/praktijk?**
   - Feitelijk (klopt onze YAML / de wet / kan de engine het?) → **desk**.
   - Oordeel/praktijk (open norm, formule-vs-praktijk, scope-keuze) → **workshop**.
4. **Heb je domeinkennis of stakeholder-buy-in nodig die je nog niet hebt?** → een
   **verkennende workshop** mag vroeg (zie de twee workshop-modi hieronder), ook vóór
   het corpus volledig gefixt is.

Bij twijfel: gebruik `AskUserQuestion` om de situatie scherp te krijgen voordat je routeert.

## Twee workshop-modi (het gate-criterium is mode-afhankelijk)

- **Verkennende / ontginnende workshop** — vroeg, zelfs op een ruwe scope-analyse. Doel:
  domeinkennis ontginnen, praktijk boven tafel krijgen, scope bekrachtigen. **Geen
  gate** — je hoeft het corpus niet eerst volledig te fixen. (Dit is hoe een eerste
  sessie vaak nuttig is, ook al staat de modellering nog niet vast.)
- **Validerende workshop** — later, om een gemodelleerd corpus te valideren. **Gate**:
  schema valide + tests groen + modellering-fouten gefixt + de resterende open punten
  zijn *judgment* (niet *factual*). Anders eerst nog een desk-cyclus — je wilt geen
  experts laten valideren wat eigenlijk onze eigen modelleerfout is.

## Handoff-lus (kort)

- **Desk → workshop**: een rijp (of bewust-ruw, bij verkennen) corpus; de judgment-set
  untranslatables wordt de beslispunten; de scope-analyse komt uit `cross-law-diagram` +
  `corpus-status`.
- **Workshop → desk**: correctiepunten + bevestigde interpretaties gaan terug de cyclus
  in (implementeer modellering-fixes, verifieer claims tegen bronnen via de
  resolutie-tracker, documenteer bevestigde wetgevings-fouten).

Zie `references/routing.md` voor het volledige diagram en de gedeelde bruggen.
