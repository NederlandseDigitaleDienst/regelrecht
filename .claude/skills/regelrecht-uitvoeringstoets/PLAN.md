# Planningsdocument — Skill `regelrecht-uitvoeringstoets`

> Status: **plan / nog niet gebouwd**. Datum: 2026-06-16.
> Basis voor een toekomstige skill-PR (engine-repo `.claude/skills`). Niets hiervan is
> gepusht of geïmplementeerd; dit document is bedoeld om aan te scherpen.

## 1. In één zin

Een **workshop-format-skill** die uit een (semi-)gevalideerd corpus een geloofwaardige
**service-PoC** genereert (burger- + behandelaar-portaal, of een headless beslis-service),
en die PoC inzet als **validatie-instrument** om met uitvoeringsexperts inzichtelijk te
maken wat nodig is om een wet **rechtvaardig in de praktijk** te brengen — gezien vanuit de
betrokkene.

## 2. These

Een echt voelbare dienstverlening-demo maakt — beter dan een tekstanalyse — zichtbaar wat
een wet *betekent als dienst*: welke last bij de burger ligt, waar menselijk oordeel zit,
welke termijnen gelden, en waar de wet hard of onduidelijk is. Dat is precies het materiaal
dat uitvoeringsexperts, juristen/beleid en ontwerpers nodig hebben om de uitvoering
rechtvaardig in te richten. De PoC is dus **middel, geen doel** — maar moet zó goed zijn dat
het "echt had kunnen zijn", anders valideert het niet eerlijk.

## 3. Rol & plek in de skill-familie

Een **vierde, zelfstandige skill** die als **fase 2** in de validatiereis zit — ná de
logica-/scenario-validatie, niet in plaats daarvan:

```
regelrecht-stelselanalyse (desk)        → corpus correct & compleet
        ↓
regelrecht-audit-products (workshop A)  → logica + eerste scenario's/persona's gevalideerd, open vragen
        ↓
regelrecht-uitvoeringstoets (WORKSHOP B) → service-PoC + dienstverlening-validatie
        ↓
terug naar de desk / volgende corpus-ronde
```

- **Consumeert**: output van `regelrecht-stelselanalyse` (het corpus) en
  `regelrecht-scenario-traces` (gevalideerde persona's/scenario's + keten-checkpoints).
- **Levert aan**: de workshop-cyclus van `regelrecht-audit-products` (de PoC ís het
  sessie-materiaal voor workshop B) en de bevindingen terug de cyclus in.
- **Router**: `regelrecht-traject` moet deze fase-2-plek leren kennen (routeert
  service-/dienstverleningsvragen hierheen).

Woont in de engine-repo `.claude/skills` (zelfde plek/PR-lijn als de andere, met de
bestaande leak-guard).

## 4. Naam

`regelrecht-uitvoeringstoets` — sluit aan op de gevestigde overheidsterm "uitvoeringstoets".
**Let op:** dit is *niet* de formele uitvoeringstoets-procedure; de skill levert materiaal
dát een uitvoeringstoets-achtige validatie ondersteunt. Dat onderscheid expliciet benoemen
in de skill-omschrijving.

## 5. Eenheid van waarde

Primair het **validatie-inzicht** (wat de demo blootlegt over rechtvaardige uitvoering),
maar met een PoC die **production-credible** is. Een dun/clickable artefact volstaat
daarom niet.

## 6. Intellectuele kern — corpus-afleiding (signaal → service-spec → vorm)

Geen vaste archetype-lijst die je kiest; de skill **leidt af wat de YAML's impliceren**. Het
"archetype" is een *emergente samenvatting* van de gevonden dimensies, geen schakelaar. Dit
maakt het echt casus-agnostisch: een nieuwe wet hoeft niet in een voorgebakken hokje te passen.

| Corpus-signaal | Wat het impliceert voor de dienst |
|---|---|
| `hooks` (AANVRAAG / BEHANDELING / BESLUIT / BEKENDMAKING / BEZWAAR) | welke **levenscyclus-fasen** bestaan → procesflow + schermen |
| `legal_character: BESCHIKKING` + `produces` | er is een **besluit** → motiveringsplicht, bekendmaking, bezwaar/beroep |
| `untranslatables` | waar **menselijk oordeel** zit → de mens-taken-wachtrij |
| caller-params (leaf-inputs), per leaf geclassificeerd naar *herkomst* | welke **gegevens** nodig zijn en *van wie* (burger / ander systeem / oordeel ambtenaar) → de last, en of een portaal zin heeft vs. vooraf-invullen |
| `source:` / `implements:` | **ketensamenwerking** → andere partijen/systemen, data-herkomst |
| `type_spec` (days / weeks / eurocent) | **termijnen & bedragen** in de dienst |
| verzoek-param + AANVRAAG-hook (aan/afwezig) | **initiatie**: burger-geïnitieerd vs. ambtshalve vs. melding |

Uit deze **service-spec** assembleert de skill de PoC: welke lenzen (burger / behandelaar /
headless), welke schermen, welke taken-wachtrij, welke wat-als-schakelaars (de open vragen).

**Afwezigheid is een bevinding.** Waar een signaal ontbreekt of dubbelzinnig is, zet de skill
dat op de agenda voor workshop B:
- geen BEZWAAR-hook → "is er echt geen bezwaarweg, of is het corpus incompleet?"
- een leaf-param die niemand kan leveren → last-/uitvoerbaarheidsvraag
- een untranslatable zonder duidelijke eigenaar → wie doet dit oordeel, en hoe?

De afleiding produceert dus meteen het validatie-materiaal.

## 7. Uitvoeringstoets-gereedheid — contract op het corpus

De skill definieert de **minimale metadata** die een corpus nodig heeft om zinvol te
genereren (bv. hooks aanwezig, legal_character gezet, caller-params met herkomst-hints,
type_spec-units). Ontbreekt die: **soepel bouwen met markeringen** (zie §9) en het gat op de
bevindingenlijst zetten — niet weigeren.

## 8. De vijf stappen van de skill

1. **Ingangscheck** — is de logica + ≥1 gevalideerd scenario/persona + open-vragen-register
   aanwezig? Zo niet: waarschuwen en markeren, niet weigeren.
2. **Afleiden** — service-spec uit corpus-signalen (§6) → tonen → bevestigen met de gebruiker.
3. **Genereren** — PoC uit de **referentie-template-repo** + corpus-config, gevuld met de
   gevalideerde persona's. Privacy-guards worden meegescaffold (§12).
4. **Instrumenteren** — per scherm: artikel-herkomst, open vragen zichtbaar, wat-als-
   schakelaars (een open beleidskeuze: variant A vs B), feedback-vangst, en eerlijkheid over
   de grenzen (§13).
5. **Faciliteren & oogsten** — sessie-draaiboek + facilitator-materiaal; verslag dat twee
   kanalen voedt (§11).

## 9. Ingangscontract — soepel met markeringen

Draait ook op semi-gevalideerd corpus, maar markeert overal expliciet wat nog niet
gevalideerd is. Die markeringen *zijn* validatie-materiaal. (Configureerbare strengheid is
een mogelijk groeipad, niet v1.)

## 10. Deelnemers van workshop B

- **Uitvoeringsexperts** (beslisambtenaren / behandelaars) — de kern.
- **Juristen / beleid** — voor de open beleidsvragen die de PoC blootlegt.
- **Ontwerpers / dienstverlening** — voor begrijpelijkheid, last, toegankelijkheid.
- **Groeipad (niet v1):** ervaringsdeskundigen / burgers — interessant, maar vraagt
  zorgvuldige opzet; eerst de PoC's testen met bovenstaande rollen.

## 11. Oogst — twee kanalen

- **Wets-/logica-bevindingen** → terug naar `regelrecht-stelselanalyse` (desk-cyclus).
- **Dienstverlening-bevindingen** → een apart **service-backlog** (last, begrijpelijkheid,
  toegankelijkheid, proces-/interactiekeuzes).

## 12. Verpakking — skill + referentie-template-repo

- De **skill** orkestreert (afleiden, genereren, instrumenteren, faciliteren) en bevat het
  sessie-format.
- Een **casus-agnostische referentie-template-repo** levert de échte architectuur (engine =
  rekenmeester, wet = procesflow, lenzen, RFC-013 receipts, append-only audit). Zo blijft het
  "had kunnen zijn"-niveau hoog én het onderhoud voorspelbaar; §14 vat de onderhoudbaarheids-
  eisen samen.
- **Privacy-guards meegescaffold:** privé-repo-vereiste, fail-closed pre-push hook
  (`check-remote-private.sh`-equivalent), fictieve data (fictieve test-BSN's), corpus-by-reference.

## 12b. Hoe de referentie-template eruitziet

**Kern-truc:** alles wat in een handgebouwde PoC hardcoded zou zitten, komt in de template uit
een **gegenereerde service-spec**. De app-code is 100% casus-agnostisch; per wet verschilt
alleen één gegenereerd configbestand + de redactionele B1-teksten. Het corpus blijft
by-reference.

**Drie lagen:**

| Laag | Inhoud | Per casus? |
|---|---|---|
| Referentie-architectuur (de app) | engine=rekenmeester, levenscyclus-statemachine, lenzen/views, componenten, dialoog-flow, auth-adapter, receipts, guards, testharnas | nee — nooit |
| Service-spec (`service.config.yaml`) | params/labels, lenzen, levenscyclus, keten, termijnen, mens-taken-bron, wat-als, open vragen | ja — **gegenereerd door de skill** |
| Corpus | de wet-YAML's | by-reference (`CORPUS_DIR`) |

**Repo-structuur (schets):**

```
regelrecht-service-template/
├── Cargo.toml                # engine gePIND op release (geen path-dep)
├── service.config.yaml       # ← gegenereerd: de enige casus-specifieke waarheid
├── backend/src/
│   ├── config.rs             # service-spec model
│   ├── engine.rs             # generiek: law-id/outputs/keten uit config
│   ├── lifecycle.rs          # fasen/termijnen/taken gedreven door hooks + config
│   ├── db.rs + migrations/   # sqlx migrate (geen CREATE IF NOT EXISTS)
│   ├── api.rs                # getypeerde DTO's (geen losse row→json)
│   ├── dialoog.rs            # meedenk-flow (generiek)
│   └── auth.rs               # IdP-adapter: mock ⇄ echte OIDC, pluggable
│   └── tests/                # round-trip · contract · lifecycle
├── frontend/src/
│   ├── views/                # generieke intake/dossier/werkvoorraad/detail
│   ├── components/           # stepper, live-trace, wet-uitleg, thread
│   └── service-spec.js       # laadt de config
├── scaffold/                 # privacy-guards die meegescaffold worden
└── docs/service-spec.schema.md
```

**Service-spec — NEUTRAAL voorbeeld** (de template bevat géén echte casus-waarden;
voorbeelden draaien tegen `regelrecht-corpus-dummy`):

```yaml
casus: <voorbeeld-casus>
orchestrator_law: <hoofdregeling-id>

lenzen: [proef, burger, behandelaar, meekijken]   # of [headless] bij ambtshalve/plugin
levenscyclus:                                       # uit hooks
  - { fase: AANVRAAG }
  - { fase: BESLUIT, produceert_beschikking: true } # uit legal_character
  - { fase: BEZWAAR }

formulier:                                          # uit caller-params + type_spec
  outputs: [<uitkomst-a>, <bedrag-b>]
  groepen:
    - kort: <groep>
      velden:
        - key: <leaf_param>
          type: euro
          herkomst: burger            # burger | ander_systeem | oordeel_ambtenaar
          label: "<B1-label — redactie, te reviewen>"
  gates: [...]                          # poortvragen
  escalaties: [...]                     # toon_als-regels

keten:                                  # uit source/implements
  - { knoop: <tussenresultaat>, artikel: <regeling>_<art> }

termijnen:                              # uit type_spec days/weeks
  herstel: { output: <termijn-output>, unit: weeks }

mens_taken_bron: { law: <regeling>, article: "<nr>" }   # uit untranslatables

wat_als:                                # uit het open-vragen-register
  - naam: "<open beleidskeuze>"
    varianten: [{ label: "Variant A (corpus)" }, { label: "Variant B (praktijk)" }]
open_vragen: [...]
herkomst_annotaties: true
```

**Casus-neutraliteit / leak-preventie (expliciet):**
- De template-repo én de skill-tekst bevatten **geen** casus-specifieke waarden en **geen
  domein-vocabulaire** (geen organisatie-, belasting- of keten-termen die naar één casus
  wijzen). Ook impliciete vingerafdrukken vermijden.
- Voorbeelden en de "template draait"-demo gebruiken **`regelrecht-corpus-dummy`**.
- Een **ingevulde** service-spec (voor een echte casus) wordt at-runtime gegenereerd en leeft
  **alléén bij de gegenereerde (privé) PoC**, nooit in de publieke template of skill.
- Dit document is zélf casus-agnostisch: geen organisatie-, repo-, belasting- of domeinnamen,
  en geen verwijzing naar een specifieke eerdere build. Concrete waarden (law-ids, param- en
  output-namen, bedragen) leven uitsluitend bij de gegenereerde (privé) PoC en de
  service-spec — nooit in dit plan, de template of de skill.

## 13. Eerlijkheid over de grenzen (ethisch, kern van "rechtvaardig")

Een gelikte demo mag geen schijnzekerheid wekken. De PoC toont altijd expliciet:
- fictieve data, geen productie, geen rechten te ontlenen;
- **corpus-correct ≠ beleids-correct** (de wat-als-schakelaars maken openstaande
  beleidskeuzes zichtbaar i.p.v. ze te verbergen);
- waar menselijk oordeel onmisbaar is (de untranslatables), niet weggeautomatiseerd.

## 14. Onderhoudbaarheids-eisen aan de template

De template moet deze eisen inbakken, zodat de skill onderhoudbaar genereert (een naïef
handgebouwde PoC schendt ze typisch):

1. **Formulier-metadata uit corpus/engine** i.p.v. hardcoded veldgroepen/labels/param-namen
   (tientallen hardcoded params → drift-risico).
2. **Getypeerd API-contract** (gedeelde types / OpenAPI) i.p.v. ongetypeerde row→json-respons.
3. **Regressienet als onderdeel van de template**: round-trip-tests op de conversies,
   keten-checkpoint-waarden, en een **corpus-contracttest** (faalt zodra app en corpus
   uiteenlopen).
4. **Beslis-/mapping-logica uit de backend richting corpus** waar mogelijk (in een
   handgebouwde PoC zit nog business-logica in de backend, bv. beslis-type-bepaling en
   input-mappings — die hoort idealiter in het corpus/de engine).
5. **Engine gepind op release** (niet path-dep naar lokale checkout) + corpus-versie in de
   receipt; **migratieframework** i.p.v. ad-hoc tabel-creatie; echte DB/sessiestore.
6. **Echte design-system-componenten** + geverifieerde toegankelijkheid (niet alleen
   look-alike CSS).

## 15. v1-scope

- Ondersteunt in v1 eerst het archetype **aanvraag→beschikking**, maar de motor is de
  afleiding (§6), niet het archetype — andere vormen emergeren naarmate meer wetten
  erdoorheen gaan.
- Headless beslis-service (plugin op een ander systeem) als strategisch interessantste tweede
  vorm om de afleiding te testen — als snel groeipad, niet noodzakelijk in v1.

## 16. Open punten / nog te beslissen

1. Precieze afbakening van het **uitvoeringstoets-gereedheid-contract** (§7) — welke
   metadata strikt vereist, welke optioneel.
2. **Herkomst-classificatie van caller-params** (burger / ander systeem / oordeel): hoe
   gecodeerd in het corpus? Nieuwe metadata, of af te leiden?
3. **Naamgeving & locatie referentie-template-repo** (publiek-leeg-template vs. privé).
4. Aansluiting op de bestaande **skills-PR (#785)**: zelfde PR of opvolger.
5. Of de skill ook de **wat-als-schakelaars** automatisch uit het open-vragen-register
   genereert, of dat de facilitator ze kiest.

## 17. Beslist (samenvatting)

- Naam: **`regelrecht-uitvoeringstoets`**.
- Rol: **fase-2 workshop-format**, ná logica-/scenario-validatie.
- Waarde: **validatie-inzicht**, met production-credible PoC.
- Kern: **corpus-afleiding** (signaal → service-spec → vorm), archetype emergent.
- Ingang: **soepel met markeringen**.
- Deelnemers: **uitvoeringsexperts + juristen/beleid + ontwerpers**; burgers = groeipad.
- Oogst: **twee kanalen** (desk + service-backlog).
- Verpakking: **skill + referentie-template-repo**.
