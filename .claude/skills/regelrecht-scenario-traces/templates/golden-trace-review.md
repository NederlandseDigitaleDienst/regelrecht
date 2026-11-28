# {Traject / endpoint} — golden-trace & coverage-review — {datum}

**Scope**: {endpoint(s) / keten} · **Engine**: {versie} · **Scenario's**: {n}

## Snapshot-status

| Scenario / persona | Golden trace | Geverifieerd tegen | Laatste herijking |
|---|---|---|---|
| {persona} | vastgelegd / ontbreekt | wettekst / nota / expert / — | {datum} |

> Een trace is pas "goud" na verificatie tegen de bron. Niet-geverifieerde snapshots
> bevriezen alleen de huidige interpretatie — markeer ze als voorlopig.

## Diffs deze ronde

| Scenario | Knoop die verschoof | Oud → nieuw | Endpoint mee veranderd? | Oordeel |
|---|---|---|---|---|
| {persona} | {knoop} | {a → b} | ja / **nee** | verwacht (herijk) / **onverwacht (ketenfout?)** |

{Een diff met ongewijzigde endpoint maar verschoven tussen-knoop is het signaal waar deze
methode voor bestaat: mogelijk een stille ketenfout. Classificeer 4-weg via
`regelrecht-stelselanalyse`.}

## Branch-coverage

| Knoop / conditie | Waar | Onwaar | Status |
|---|---|---|---|
| {conditie} | ✓ | ✗ | half — kandidaat-persona: {beschrijving} |

**Dekking**: {x}/{y} takken beide-kanten gedekt.

## Caps & overgeslagen

{Expliciet: welke scenario's niet gesnapshot, welke takken bewust niet gedekt, welke
personas gesampled i.p.v. volledig. Geen stille truncatie.}

## Acties

- {nieuwe persona/twin om ongedekte tak X te raken}
- {onverwachte diff Y onderzoeken / classificeren}
