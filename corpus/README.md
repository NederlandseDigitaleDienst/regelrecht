# Corpus

Internal test regulations for validating the regelrecht engine.

## Purpose

These regulations are maintained by us to create isolated, controlled scenarios for testing the engine. They are **not** the public corpus — that lives in a separate repository.

## Structure

```
corpus/
├── regulation/
│   └── nl/
│       ├── wet/                          # Formal laws
│       │   └── <law_id>/
│       │       ├── <valid_from>.yaml     # One file per version
│       │       └── scenarios/            # BDD scenarios validating this law
│       ├── ministeriele_regeling/        # Ministerial regulations
│       └── gemeentelijke_verordening/    # Municipal ordinances
├── cells/
│   ├── <cell>/
│   │   ├── cel.yaml                      # Who a cell is and which answers it gives
│   │   └── reductions/<id>/              # Its reductions (REDUCTIE, RFC-048): no regulation,
│   │       └── <valid_from>.yaml         # the cell gives their outputs as named answers
│   └── reductions/<id>/                  # Reductions of no cell, such as that of the Awb:
│       └── <valid_from>.yaml             # every cell applies them to its own chronicle
├── annotations/
│   ├── <law_id>/annotations.yaml         # Note sidecars (RFC-018)
│   └── _vocabulary/                      # Shared ambiguity tag vocabulary
└── README.md
```
