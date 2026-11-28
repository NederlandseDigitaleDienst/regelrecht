# Regelrecht — zakkaart voor regelanalisten

*Eén A4: waar begin ik, welke skill, hoe loopt een traject. De methode is bekend verondersteld.*

## Bij twijfel: de voordeur

**Beschrijf wat je wilt, dan wijst `regelrecht-traject` de weg.** Skillnamen hoef je niet te
onthouden.

| je wilt… | skill |
|---|---|
| een wet machine-leesbaar maken | `law-interpret` |
| een corpus reviewen, fouten in de wet vinden | `regelrecht-stelselanalyse` |
| een interpretatiekeuze vastleggen, of erover uitspraak doen | `regelrecht-verantwoording` |
| een expert- of validatiesessie voorbereiden | `regelrecht-audit-products` |
| weten waar een bepaling vandaan komt | `law-wetsgeschiedenis` |

## De cyclus

```
 werkronde ──▶ uitspraak ──▶ verwerken ──▶ attestatie ──▶ volgende versie
     ▲                           │
     └──────── gecorrigeerd ─────┘
```

| moment | wie | wat |
|---|---|---|
| werkronde | model, analist | tekst wordt YAML; keuzes worden claims, vastgelegd in scenario's |
| uitspraak | de bevoegde | bekrachtigt, corrigeert, weerlegt, of laat met reden open; in de editor of in een sessie |
| verwerken | analist | het corpus volgt; een correctie wordt een nieuwe claim |
| attestatie | de bevoegde | versie H is productiewaardig: gedrag groen, toelichting compleet |

## Een claim — de toelichting per artikel

Een interpretatiekeuze, als notitie bij de wettekst: **lezing · grond · verworpen alternatief
met gevolg · wie moet erover spreken · voor welke regelwerkversie (hash).** Wie de claim
vastlegt, sluit haar niet zelf. De kroniek groeit alleen: een correctie is een nieuwe claim.

Een scenario legt de lezing vast als het **rood wordt onder het alternatief**. Toelichting
nodig hebben alleen artikelen waarin het model meer doet dan de letter: een open term,
aanname, untranslatable of override.

| toelichting | scenario | |
|---|---|---|
| ✓ | ✓ | sterkst |
| — | ✓ | alleen zonder interpretatieruimte |
| ✓ | — | alleen bij oordeel per geval |

## Soort bevinding

| label | actie |
|---|---|
| modellering-fout | desk fixt de YAML |
| wetgevings-fout | desk documenteert; normsteller is bevoegd |
| engine-limitatie | engine-issue |
| acceptabele untranslatable | claim met een bevoegde; rekent door als werkhypothese |

Een feitelijk defect lost de desk zelf op, een oordeel wacht op de bevoegde.

## Sessies

- **Verkennend**: vroeg, zonder voorwaarden. Domeinkennis ophalen.
- **Validerend**: schema valide, tests groen, modelfouten gefixt; wat overblijft is oordeel.

## Het bestandssysteem is de interface

Wat je in de repo achterlaat, ziet de editor: wet-YAML in `regulation/`, scenario's in
`scenarios/`, claims en uitspraken in `annotations/{law_id}/annotations.yaml`. Dat bestand is
**append-only**: nooit herbouwen.

## Autonomie & veiligheid

`/loop` is een lokale batch, `/schedule` een cloud-routine. Commits door routines gaan **alleen
naar private repos** (push-guard); wie bewust publiek wil pushen zet `ALLOW_PUBLIC_PUSH=1`.

---
Voordeur: `regelrecht-traject/SKILL.md` · claims: `regelrecht-verantwoording/SKILL.md`
