---
name: regelrecht-stelselanalyse
description: Voert de iteratieve desk-review-&-completion-cyclus uit op een regelrecht-corpus — plannen, harvesten/migreren/uitbreiden, multi-agent valideren, bevindingen 4-weg classificeren (modellering-fout / wetgevings-fout / engine-limitatie / acceptabele untranslatable), synthese + heroverweging, documenteren (wetgevings-/stelselfouten-analyse, fixes-plan, engine-limitaties, diagrammen, corpus-status), verifiëren tegen externe bronnen (resolutie-tracker + bronnen-dossier), en een eindrapport per cyclus. Gebruik dit bij corpus-review op correctheid/coverage zonder live expert-sessie, het opstellen van een wetgevings-notitie-bron, schema-migratie, het harvesten van nieuwe wetten, of het aansturen van een autonome completion-loop. Casus-agnostisch; de regelrecht-methode is de vaste taal. Voor live-validatie met experts: zie regelrecht-audit-products.
allowed-tools: Read, Write, Edit, Grep, Glob, Bash, WebFetch, WebSearch, Agent, AskUserQuestion, ScheduleWakeup, Monitor
---

# Regelrecht stelselanalyse — desk-review & corpus-completion

De analytische desk-tegenhanger van `regelrecht-audit-products` (live-validatie met
experts). Deze skill drijft de **iteratieve cyclus** waarmee een machine-leesbaar
corpus stap voor stap correcter en completer wordt, en levert de bijbehorende
producten op.

Het verschil met een audit-doc: een audit-doc vraagt *"klopt ons stappenplan?"*. Deze
skill vraagt óók *"klopt de wet zelf, is hij uitvoerbaar, en hoe ver komt de engine?"*.
Zie `references/classification.md`.

Geen casus-inhoud in de skill; concrete inhoud komt uit het corpus en de externe
bronnen tijdens de cyclus.

## Routing & handoff

Dit is de desk-laag. Oordeels-/praktijkvragen (acceptabele untranslatables, formule-vs-
praktijk, scope-keuzes) routeren naar de workshop-skill **`regelrecht-audit-products`**;
feitelijke defecten blijven hier. De *judgment*-set untranslatables wordt de beslispunten
van een workshop; de scope-analyse daar komt uit de desk-producten `cross-law-diagram` +
`corpus-status`. Workshop-correctiepunten komen terug de cyclus in (fixes + verificatie).
Twijfel je waar te beginnen → **`regelrecht-traject`** (voordeur: de cyclus, en welke skill
wanneer).

## De cyclus

```
plan ─► (harvest / migreer / breid uit) ─► valideer (multi-agent, per as)
   ▲                                              │
   │                                              ▼
eindrapport ◄─ verifieer (bronnen + tracker) ◄─ classificeer (4-weg)
                                                  │
                                                  ▼
                                        synthese + heroverweging
                                                  │
                                                  ▼
                              documenteer (fouten / fixes / limitaties / diagrammen / status)
```

Eén cyclus pakt één **thema/scope** (niet alles tegelijk). Werk in micro-cycli.

## Step 0 — drift-check (verplicht, vóór elke YAML-edit)

Elke micro-cyclus die een regulation-YAML kán aanraken begint met de zustersskill
**`law-version-drift-check`**: spiegel elk `text:`-blok tegen de geldende wettekst op
wetten.overheid.nl bij de `valid_from` van de YAML. Géén bypass, geen "deze cyclus is
maar docs"-escape — de trigger is "ik ga zo een regulation-YAML bewerken".

Waarom een harde voorwaarde: drift mis-classificeert bevindingen over de hele 4-weg-as.
Een claim "de YAML zegt X maar de wet zegt Y" is alleen een **wetgevings-fout** als de
geldende wet écht Y zegt; was Y al gewijzigd door een niet-ingeharvest Stb., dan is
diezelfde bevinding in werkelijkheid een **modellering-fout (drift)**. Zonder een CLEANE
(of scope-gerestricteerde) drift-rapport mag geen YAML-edit gemaakt worden, en bevindingen
op artikelen met open DRIFT-tekst worden bij de synthese (stap 5) verworpen.

## Kernprincipe: vier-weg-classificatie

Elke bevinding krijgt precies één label. Dit bepaalt waar hij landt en welke actie
volgt — verkeerd labelen = verkeerde actie. Zie `references/classification.md`:

1. **Modellering-fout** — onze YAML/feature wijkt af van de (correcte) wettekst →
   wij fixen het (`modellering-fixes-plan`).
2. **Wetgevings-fout** — de wet zelf is onjuist/achterhaald/onuitvoerbaar, niet door
   interpretatie te repareren → aanbevolen wetgevings-actie (`wetgevingsfouten-analyse`).
3. **Engine-limitatie** — wet + modellering kloppen, engine kan het (nog) niet →
   engine-issue (`engine-limitaties`).
4. **Acceptabele untranslatable** — open norm die bewust niet gemodelleerd wordt en
   dat ook niet hoeft → markeren, geen actie.

De grens tussen (2) en (4) is de subtielste en belangrijkste: een open norm is
*acceptabel untranslatable* bij een kenbare beslisser + toetsbaar kader, en een
*wetgevings-fout* als beslisser/kader/grond ontbreekt.

**Meta-check, altijd uitvoeren**: maken de features en de YAML dezelfde fout? Zo ja,
dan valideert de BDD-suite de YAML en niet de wet — groene tests bewijzen dan niets
over juridische correctheid. Het concrete wapen hiertegen is de techniek-skill
**`regelrecht-scenario-traces`**: keten-checkpoints (assert elke knoop op het kritieke
pad, niet alleen de endpoint) + golden-trace-snapshots maken elke ketenschakel los
toetsbaar tegen de wettekst i.p.v. alleen de endpoint.

## Werkstroom

0. **Drift-check (Step 0).** Vóór élke YAML-edit: draai de zustersskill
   `law-version-drift-check` per file. Geen CLEANE/scope-gerestricteerde drift-rapport →
   geen edit. Zie de sectie *Step 0* hierboven; bevindingen → resolutie-tracker (klasse 1).

1. **Plan de cyclus.** Bepaal thema/scope en deel op in micro-cycli
   (`templates/cyclus-plan.md`). Voor autonome uitvoering: schrijf een self-driving
   loop-prompt (`templates/loop-prompt.md`). Zie `references/cycle-workflow.md`.

2. **Doe het corpus-werk.** Harvest een nieuwe wet, migreer naar een nieuwe
   schemaversie, of breid MR-logica uit — elk met een rapport
   (`harvest`/`mr`/`schema-migratie`-sjablonen).

3. **Valideer, eventueel multi-agent.** Verdeel over review-assen (correctheid,
   untranslatables, coverage, source-refs, diagrammen, wetgevings-fouten). Bij
   parallel werk: één sub-agent per as via de `Agent`-tool. Zie
   `references/review-orchestration.md`.

   **Source-refs — verplichte, niet-overslaanbare integriteitsscan.** De `source-refs`-as
   is geen losse steekproef maar een volledig algoritme over het hele corpus:
   1. Bouw een map `regulation → set(action-outputs)` over het hele corpus (welke output
      produceert elke wet feitelijk).
   2. **MISPLACED** — een `source:`-blok onder `parameters:` (i.p.v. `input:`). De engine
      heeft twee aparte structs: `Parameter` (onder `parameters:`) kent **geen** `source`-veld,
      alleen `Input` (onder `input:`) wel. Een source onder `parameters:` wordt dus bij het
      parsen **stil weggegooid** — de binding lijkt echt maar vuurt nooit; scenario's die de
      waarde direct injecteren maskeren het. Detecteer per `source` of de omsluitende lijst
      `input:` is; zo niet → **MISPLACED** (**modellering-fout**). Dit is de meest
      voorkomende verborgen vorm.
   3. **DANGLING** — voor elke `source: { regulation, output }` onder `input:`: verifieer
      `output ∈ outputs[regulation]`. Zo niet → **DANGLING** (**modellering-fout**).
   4. **PLAIN-PARAM** — een `parameters:`-item waarvan de `description` "conceptueel" of
      "tijdelijk als directe parameter" bevat maar dat géén binding is. (Let op: woorden als
      "forward naar" op een leaf-parameter die een binding-mapping vóédt zijn legitiem — niet
      flaggen.)
   5. Rapporteer `clean / misplaced / dangling / plain-param`. Een corpus is pas
      **source-clean** als `misplaced = 0`, `dangling = 0` én `plain-param = 0`.

   **MISPLACED, DANGLING en PLAIN-PARAM zijn in de vier-weg-classificatie ALTIJD
   modellering-fout — nooit "engine-limitatie".** Het "de engine kan geen meerdere bindingen
   per artikel"-excuus is ongeldig (schema v0.5.2 ondersteunt dit); bind echt — onder
   `input:` — en bewijs met een BDD-scenario dat de bron-wet laadt en de leaf-inputs zet.

   Draai de scan reproduceerbaar met
   `python3 script/cross-law-integriteit.py <corpus-root>`
   (exit-code 0 = source-clean, 1 = bevindingen).

4. **Classificeer** elke bevinding 4-weg (zie boven). Voer de meta-check uit.

5. **Synthese + heroverweging.** Voeg parallelle reviews samen; markeer twijfel-claims;
   schrap weerlegde claims (doorgestreept + reden) en pas tellingen aan.

6. **Documenteer** vanuit de sjablonen: wetgevings-fouten, fixes-plan, engine-limitaties,
   cross-law-diagrammen, corpus-status, engine-tests.

7. **Verifieer met externe bronnen.** Voor Kamerstukken en de geschiedenis van een
   artikel eerst `law-wetsgeschiedenis`: toelichting per artikel en lid, herkomst per
   regel en de versies door de tijd, elk met het Kamerstuk en zijn adres. Ontgin de rest
   (Staatsbladen buiten een dossier, rekenregels, publicaties van de uitvoerder) via
   WebFetch; leg status vast in `resolutie-tracker.md`
   (vocabulaire: verifieerd / weerlegd / gedeeltelijk / onverifieerbaar / out-of-scope)
   en in het bronnen-dossier. Zie `references/verification-cycle.md`.

8. **Eindrapport** per cyclus (`templates/eindrapport.md`): doel vs resultaat, geleverd,
   discoveries, open punten voor de volgende cyclus.

9. **Schrijf naar het corpus/traject, niet in de skill. Commit/push alleen op verzoek.**

## Bestanden in deze skill

**references/**
- `classification.md` — vier-weg-classificatie + onderscheid met de audit-doc + de
  beslisregel open-norm-vs-wetgevingsfout. Het conceptuele hart.
- `review-orchestration.md` — multi-agent review: assen, sub-agent-verdeling, synthese,
  heroverweging, telling, de features-vs-YAML meta-check.
- `defect-taxonomy.md` — categorieën wetgevings-fouten, ernst-niveaus, per-fout format.
- `verification-cycle.md` — resolutie-tracker, status-vocabulaire, scope-markering,
  bronnen-dossier + INDEX, WebFetch-ontginning.
- `cycle-workflow.md` — de cyclus-motor: planning, loop-prompts, harvest, schema-migratie,
  eindrapport, micro-cycli.
- `check-keten.md` — overzicht van alle checks op wets-zuiverheid & cross-law, met per
  check het *orakel* (waartegen) en of het een CI-gate of methodologisch is.
- `script/cross-law-integriteit.py` (repo-script, buiten de skill) — herbruikbaar script voor de source-refs-integriteitsscan:
  bouwt `regulation → outputs`, detecteert MISPLACED/DANGLING/PLAIN-PARAM source-bindingen
  én IMPL-DANGLING (`implements` naar een niet-gedeclareerde open_term) / IMPL-NO-DATE
  (implementing-regeling zonder `valid_from`), en print de telling (exit 1 bij bevindingen).
  Corpus-agnostisch; draait ook als CI-gate (`cross-law-integrity` job) en als preflight
  in de `Valideer`-stap.

**templates/** — `cyclus-plan`, `loop-prompt`, `scheduled-routine`, `schema-migratie`,
`harvest-rapport`, `mr-uitbreiding-rapport`, `validatie-review`, `synthese`,
`wetgevingsfouten-analyse`, `modellering-fixes-plan`, `engine-limitaties`,
`resolutie-tracker`, `bronnen-INDEX`, `cross-law-diagram`, `corpus-status`,
`engine-tests`, `eindrapport`.

Autonome uitvoering (`/loop` self-paced voor een afgebakende batch, `/loop <interval>`
voor polling, `/schedule`-routine voor onbeheerd terugkerend werk): zie
`references/cycle-workflow.md` en de sjablonen `loop-prompt` + `scheduled-routine`.

**scripts/** — `assert-private-repo.sh`: fail-closed guard die alleen doorlaat als de
push-target een **private** GitHub-repo is (autonome routines mogen committen/pushen, maar
alleen naar private repos). Draai als preflight vóór elke push; PUBLIC/INTERNAL → geweigerd.
`test-assert-private-repo.sh` borgt de invarianten van die guard met gestubde `git`/`gh`
(gitlab/enterprise/bitbucket → block, github-private → pass, onbereikbaar → block).

**Zustersskills** (apart geïnstalleerd, geen bestand in deze skill):
- `law-version-drift-check` — **Step 0** van elke micro-cyclus: spiegelt YAML-tekst tegen
  de geldende wettekst vóór elke edit (zie de sectie *Step 0*). Bevindingen → resolutie-
  tracker als 4-weg klasse 1 (modellering-fout), fixes → `modellering-fixes-plan`.
- `regelrecht-audit-products` — live expert-validatie (workshop-laag); ontvangt de
  judgment-untranslatables en scope-analyse uit deze desk-laag.
- `regelrecht-scenario-traces` — techniek-skill voor keten-checkpoints + golden-trace-
  snapshots (zie de *Meta-check*): maakt elke ketenschakel los toetsbaar tegen de wettekst,
  niet alleen de endpoint.
- `regelrecht-traject` — voordeur: de cyclus, en welke skill wanneer.
- `law-wetsgeschiedenis` — de wetsgeschiedenis per artikel en lid, uit de afgespeelde
  Kamerstukken: waar een regel vandaan komt, welke nota's en amendementen haar raakten,
  hoe zij op een datum luidde, en wat de toelichting erover zegt. Gebruik in stap 7
  (bronnen), in Step 0 (`vergelijk` tussen twee `valid_from`-datums noemt de gewijzigde
  artikelen met de wet die ze wijzigde) en bij de 4-weg: komt vreemde tekst uit een laat
  amendement of een nota, dan is dat bewijs voor een wetgevings-fout, geen vermoeden.

## Belangrijke regels

- **Classificeer vóór je documenteert.** Het label bepaalt het product en de actie.
- **Wetgevings-fouten zijn een formeel product** (bron voor een wetgevings-notitie):
  feitelijk, met wettekst-citaat, implicatie en concrete reparatie — geen losse meningen.
- **Bewaar de redeneerketen bij heroverweging**: geschrapte claims blijven zichtbaar
  (doorgestreept + reden), tellingen aangepast — geen stille verwijdering.
- **Scheid corpus- en engine-issues**: een engine-limitatie is geen wetgevings- of
  modellering-fout; track 'm apart zodat het corpus niet onterecht "fout" lijkt.
- **Casus-agnostisch blijven**; niets pushen zonder toestemming.
