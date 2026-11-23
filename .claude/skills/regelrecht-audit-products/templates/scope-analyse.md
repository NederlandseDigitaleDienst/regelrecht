# {Traject} — scope-analyse en wet-graph

*Te gebruiken om de scope te bekrachtigen aan het begin van de sessie. Open in een
viewer die `mermaid` en `[ ]`-checkboxes rendert.*

**Casus**: {korte omschrijving van het traject en de centrale beschikking/output}.
**Scope-manifest**: `{pad naar scope-manifest}` — {N} wetten.

---

## Samenvatting in één oogopslag

```mermaid
graph TD
    %% Laag A — grondslag
    {A1}[{Wet A1}<br/><i>WET</i>]:::wet

    %% Laag B — uitwerking
    {B1}[{Regeling B1}<br/><i>MIN. REG. / BELEIDSREGEL</i>]:::mr

    %% Laag C — lokaal/uitvoerend (orchestrator)
    {C1}[{Orchestrator C1}<br/><i>...</i>]:::lokaal

    %% Laag D — databronnen
    {D1}[{Databron D1}<br/><i>WET</i>]:::data

    %% Grondslagen (legal_basis)
    {C1} -.grondslag.-> {A1}
    {B1} -.grondslag.-> {A1}

    %% Source-calls (data-flow bij executie)
    {C1} ==source==> {B1}

    %% Override
    {Bx} -->|overrides {output}| {By}

    %% Impliciete data-dependencies
    {C1} -.data.-> {D1}

    classDef wet fill:#e8f4f8,stroke:#2a6ca8,stroke-width:2px
    classDef mr fill:#fff4e0,stroke:#c88a1e,stroke-width:2px
    classDef lokaal fill:#d8f0d8,stroke:#2a7d2a,stroke-width:3px
    classDef data fill:#f0e0ff,stroke:#7030a0,stroke-width:2px
```

**Legenda**
- `==source==>` executie-tijd data-call (formule A haalt waarde uit wet B)
- `-.grondslag.->` `legal_basis` (juridisch fundament, geen runtime-dep)
- `--overrides-->` B verandert de betekenis van A's output
- `-.data.->` impliciete parameter-dependency (databron levert data, geen formule)

---

## De wetten — tabel

| # | Law-id | Laag | Rol in keten | YAML |
|---|---|---|---|---|
| 1 | `{law_id}` | {A/B/C/D} | {grondslag / kern-berekening / orchestrator / databron} | `{pad}` |
| 2 | … | … | … | … |

---

## Runtime-afhankelijkheden (source-calls)

Wie roept wie aan bij executie van de centrale beschikking:

```
[#{n}] {Orchestrator}
  ├── source → #{m}  {wet/artikel}   ({welke output/waarde})
  └── source → #{k}  {wet/artikel}   ({welke output/waarde})
```

**Niet-source, wel data-provider** (via caller-parameters):
- #{x} {databron} → `{parameters}`

---

## Juridische afhankelijkheden (`legal_basis`)

```
{Orchestrator}   → {grondslag-wet art X}  +  {…}
{Regeling}       → {grondslag-wet art X}
```

Iedereen berust direct of transitief op **{grondslag-wet art X}** — de delegatie-basis.

---

## Override-relaties

| Override op | Output | Door | Nieuwe waarde |
|---|---|---|---|
| {wet art X waarde} | `{output}` | {andere regeling art Y} | **{nieuwe waarde}** |

**Werking + grondslag-nuance**: {werkt de override mechanisch correct, en klopt de
juridische grondslag-attributie? Markeer twijfel als beslispunt.}

---

## Clusters — de lagen

### Laag A — grondslag
{wet(ten); waarom (nog) niet zelf machine-leesbaar}

### Laag B — uitwerking
{regeling(en) waar de kern-berekening leeft}

### Laag C — lokale/uitvoerende laag
{de orchestrator; wat zijn eigen bijdrage smal/breed maakt}

### Laag D — databronnen
{wetten die alleen data leveren}

---

## Analyse — wat cruciaal is om te snappen

1. {één entry-point? waar leeft de kern-berekening? hoe smal is de eigen bijdrage van
   de orchestrator? welke databronnen zijn nog niet volledig gekoppeld? hoe ver
   reikt de override? hoe transitief is de grondslag?}

---

## Scope-beslispunten (voor sessie-blok "scope")

| # | Keuze | Opties | Implicatie |
|---|---|---|---|
| S1 | {scope volledig of ontbreekt er iets?} | compleet / aanvullen | {…} |
| S2 | {databronnen via source of via caller-parameter?} | source / caller | {…} |
| S3 | … | … | … |

- [ ] **S1** — {vraag}? Notitie: —
- [ ] **S2** — {vraag}? Notitie: —
- [ ] **S3** — {vraag}? Notitie: —
