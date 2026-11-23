# {Traject / endpoint} — persona-bibliotheek

De benoemde casussen voor {endpoint}. Persona = bron; scenario's zijn afgeleiden. Alle
niet-genoemde leafs staan op de baseline (zie onder).

## Casus-assen

| As | Soort | Waarden | Voedt tussen-norm |
|---|---|---|---|
| {as 1} | categorisch / vlag | {w1, w2, ...} | {knoop} |
| {as 2} | ... | ... | ... |

**Verboden combinaties** (onmogelijke casussen): {as X = a met as Y = b}, ...

## Baseline

Alle leafs op hun neutrale waarde. Niet-gezette leaf → runner neemt {standaardwaarde}.

```
{leaf} | {neutrale waarde}
...
```

## Persona's

### {persona-naam}

> {Casus in mensentaal, 1-2 zinnen.}

- **Assen-coördinaat**: {as 1 = w}, {as 2 = w}, ...
- **Leaf-deltas**:
  | leaf | waarde |
  |---|---|
  | {leaf} | {waarde} |
- **Keten-checkpoints (verwacht)**: {knoop = waarde}, ..., **endpoint = {waarde}**
- **Bedoeld onderscheid**: {wat deze persona test dat een buur-persona niet test}

*(herhaal per persona)*

## Twin-persona's

Paren die op precies één as-waarde verschillen — om een anders onzichtbaar onderscheid
falsifieerbaar te maken.

| Twin-paar | Verschilt op | Verwacht: endpoint flipt? | Knoop die hoort te flippen |
|---|---|---|---|
| {A} ↔ {B} | {as = w1 vs w2} | ja / nee | {knoop} |

**Onderscheid zonder as** *(indien van toepassing)*: {twee casussen met identieke invoer
maar bedoeld verschil} → bevinding, geen scenario. Route 4-weg via
`regelrecht-stelselanalyse`: {modellering-fout / wetgevings-fout / acceptabel}.

## Expressievorm

{Scenario Outline + Examples / custom `Given persona`-step + fixtures / fixtures-bestand}
— gekozen omdat {runner-reden}. Typering: {booleans/getallen via typerende stap;
identifiers als string}.
