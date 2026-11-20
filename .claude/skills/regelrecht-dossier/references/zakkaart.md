# Regelrecht — zakkaart voor regelanalisten

*Eén A4. Waar begin ik · welke skill · hoe loopt een casus. (Methode bekend verondersteld.)*

## Bij twijfel: de voordeur
**Beschrijf je casus → `regelrecht-dossier` (router) wijst de weg.** Je hoeft geen
skill-namen te onthouden — zeg wat je wilt, de router triageert en dispatcht.

## Drie deuren (op intentie)

| Je wilt… | Deur |
|---|---|
| nieuwe casus · waar begin ik · welke producten heb ik nodig | **router** — `regelrecht-dossier` |
| wet machine-leesbaar maken · corpus reviewen · fouten in de wet vinden | **desk** — `regelrecht-stelselanalyse` |
| expert-/validatiesessie voorbereiden | **workshop** — `regelrecht-audit-products` |

Achter elke deur: **waar komt deze tekst vandaan, wat bedoelde de wetgever, hoe luidde
het artikel op die datum?** → `law-wetsgeschiedenis`, per artikel en lid, met het
Kamerstuk om te citeren.

## De flow (gesloten lus)

```
 NIEUWE CASUS → [router] → feitelijk? ┐        ┌ oordeel/praktijk?
                                      ▼        ▼
   ┌──────────── DESK (machinekamer) ────────────┐  handoff  ┌──── WORKSHOP (live) ────┐
   │ plan → harvest → modelleer → valideer →      │ ◄───────► │ scope-analyse → audit-  │
   │ CLASSIFICEER (4-weg) → fix → documenteer →   │           │ doc → sessie → consensus│
   │ verifieer (bronnen/tracker) → eindrapport    │           │ + correctiepunten +     │
   └──────────────────────────────────────────────┘           │ testcases               │
        └─ correctiepunten + bevestigde interpretaties ◄────────┘ terug de desk in
```

## Router-taal: 4-weg-classificatie — waar gaat een bevinding heen?

| Label | Betekenis | Route |
|---|---|---|
| **modellering-fout** | onze YAML ≠ (correcte) wet | desk fixt de YAML |
| **wetgevings-fout** | de wet zelf fout/achterhaald/onuitvoerbaar | → wetgevings-notitie |
| **engine-limitatie** | wet + modellering kloppen, engine kan 't niet | → engine-issue |
| **untranslatable / praktijk** | open norm, oordeel nodig | → **workshop** |

> **Eén regel**: feitelijke defecten → naar binnen (desk); oordeels-/praktijkvragen → naar buiten (workshop).
> Open norm = *acceptabele untranslatable* bij kenbare beslisser + kader, anders *wetgevings-fout*.

Canonieke definities: `regelrecht-stelselanalyse/references/classification.md` — deze kaart is de spiekversie.

## Workshop: twee modi (gate is mode-afhankelijk)

- **Verkennend** — vroeg, géén gate: domeinkennis ontginnen op een ruwe scope-analyse.
- **Validerend** — gate: schema valide + tests groen + modellering-fouten gefixt + resterende punten zijn *judgment*.

## Beslis-kaart

| Situatie | Ga naar |
|---|---|
| Geen/ruw corpus | desk — harvest + modelleer |
| Modellering onzeker/ongevalideerd | desk — valideer + classificeer + fix |
| Open punt is **feitelijk** (YAML/wet/engine) | desk — fixes / wetgevings-analyse / engine-issues |
| Open punt is **oordeel/praktijk** | workshop — validerend |
| Domeinkennis/buy-in nodig die je mist | workshop — verkennend (mag vroeg) |
| Na een workshop | desk — implementeer + verifieer |
| Repetitief batchwerk / terugkerende controle | `/loop` (lokaal) · `/schedule` (cloud) |

## Autonomie & veiligheid

`/loop` = lokale batch (zichzelf afmakend) · `/schedule` = cloud-routine (onbeheerd, cron).
Commits door routines gaan **alleen naar private repos** (push-guard); bewust publiek
pushen: `ALLOW_PUBLIC_PUSH=1`.

---
Canonieke flow & details: `regelrecht-dossier/references/routing.md`
