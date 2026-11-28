# {Traject / endpoint} — casus-matrix (reverse-lookup index)

Vind een casus terug: van mensentaal naar scenario, keten-pad en uitkomst. De
tegenhanger van de persona-bibliotheek — daar staat de definitie, hier de index.

## Persona → scenario → pad → uitkomst

| Persona | Assen-coördinaat (kort) | Scenario('s) | Kritiek keten-pad | Endpoint |
|---|---|---|---|---|
| {persona-naam} | {as1=w, as2=w} | {scenario-id / titel} | {knoop → knoop → endpoint} | {waarde} |

## Index op as

Zoek-ingang per as-waarde: "waar zit een casus met {as = w}?"

| As = waarde | Persona's | Aantal scenario's |
|---|---|---|
| {as X = w} | {persona, persona} | {n} |

## Index op keten-pad

Welke scenario's lopen via welke schakel — zo zie je welke paden druk getest zijn en
welke kaal.

| Keten-knoop / tak | Geraakt door | Waar / onwaar gedekt? |
|---|---|---|
| {knoop} | {persona's} | waar ✓ / onwaar ✗ |

## Gaten

- **Ongedekte as-waarden**: {as = w} heeft geen enkele persona.
- **Ongedekte takken**: {knoop, onwaar-kant} nergens geraakt → kandidaat voor nieuwe
  persona/twin (zie `golden-trace-review`).
- **Onderscheid zonder as**: {bedoeld verschil dat geen as is} → bevinding, niet indexeerbaar.
