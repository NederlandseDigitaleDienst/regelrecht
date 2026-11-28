# {Traject}-expert-workshop — {artikel/onderwerp} — {datum} ({duur})

**Doel**: {wat valideren we, met wie, in hoeveel tijd}.

**Deelnemers**: —

**Te auditen**:
- YAML: `{pad}` — {artikel}
- Wettekst: {URL}
- Formules: `{pad naar formules-doc}`

**Afbakening**: {wat valt binnen scope, wat raken we alleen waar direct gedelegeerd,
wat is een aparte sessie}.

---

## Agenda

| Tijd | Blok |
|---|---|
| {0:00–0:15} | Kennismaking |
| {0:15–0:45} | Kennis-oogst |
| {0:45–1:15} | Scope + wet-graph |
| {1:15–1:25} | Pauze |
| {1:25–2:25} | Walk-through {artikel} |
| {2:25–2:50} | Beslispunten |
| {2:50–3:00} | Afronding |

*Dit draaiboek is voor de facilitator — niet bedoeld om tijdens de sessie op scherm te
tonen.*

---

## Deel 1 — Kennismaking + kennis-oogst

Rondje (naam + rol + 1 zin "recente case"). Daarna brown paper + sticky-notes
(3 kleuren: 🟨 kern · 🟩 wat loopt goed · 🟥 wat loopt stroef/vragen).
Zie `facilitation-patterns.md` → kennis-oogst.

## Deel 2 — Scope + wet-graph

Scope-/lagen-diagram op tafel. Dot-voting (🟢 kennen we / 🔴 blinde vlek). Loop de
scope-beslispunten S1..Sn uit de scope-analyse door.

## Pauze — **niet skippen**

## Deel 3 — Walk-through {artikel}

Protocol per output (zie `facilitation-patterns.md` → A/B/C). Splitview formules-doc
links, sessie-notities rechts.

**Time-box** (harde stops):
- Output 1 → {x} min
- Output 2 (de zware) → {x} min
- …
- Buffer → {x} min

### Per output — key-reminder

**Output 1 `{naam}`** ({simpel/zwaar}):
- {korte karakterisering; verwacht weerwoord; je comeback}

**Output 2 `{naam}`** (de zware):
- {sub-gronden snel langs; eventuele splitsings-/interpretatie-discussie via 1-2-4-all}
- {untranslatables: benoem factual vs judgment}

*(per output herhalen)*

## Deel 4 — Beslispunten ({tijd}) — werkvorm 1-2-4-all

Per punt max ~3 min (zie `facilitation-patterns.md`). Noteer minderheidsstandpunten.

| # | Punt | Verwacht + comeback |
|---|---|---|
| B1 | {punt in één zin} | {verwacht weerwoord → comeback} |
| B2 | … | … |
| … | … | … |

Markeer de zwaarste juridische punten — daar minimaal {x} min discussie, nooit skippen.
Als er geen conclusie komt → action-item met eigenaar.

## Deel 5 — Afronding

1 zin per persoon — *"wat neem ik mee?"*. Noteer elke zin.
Daarna: action-items-tabel (wie/wat/deadline) + vervolgsessie-datum.

| # | Wat | Wie | Deadline |
|---|---|---|---|
| 1 | | | |

---

## Claude-orakel — wanneer gebruiken

{Goede momenten (case-simulatie bij "case die dit breekt?", feitelijke check bij een
beslispunt). Rode vlag: als Claude gaat adviseren → "Blijf bij de YAML-lookup."
Zie `templates/claude-orakel-prompt.md`.}

## Risico's + back-pockets

Zie `facilitation-patterns.md`. Plan B als het stroef loopt: case-vertelling dóór de
wettekst heen.

## Na afloop

1. Deze file (alle vinkjes + notities) wordt het ruwe sessie-verslag.
2. Audit-docs `[ ]` → `[x]` invullen + afwijkingen noteren.
3. Per beslispunt waar *wijzigen* is gekozen: aparte commit met YAML-update + testen
   groen + formules opnieuw genereren.
4. Verslag intern + extern opstellen.
