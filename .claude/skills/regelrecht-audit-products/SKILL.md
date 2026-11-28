---
name: regelrecht-audit-products
description: Bouwt de audit- en workshop-producten waarmee juridische experts een machine-leesbare (regelrecht-YAML) vertaling van wet- en regelgeving valideren in een live sessie. Gebruik dit bij het voorbereiden van een expert-workshop of validatie-sessie over een traject, het maken van een scope-/stelselanalyse, per-artikel audit-checklists, facilitator-materiaal, testcase-scenario's, of sessie-verslagen (intern + extern). Casus-agnostisch; de regelrecht-methode (YAML, formules, untranslatables, source/override, legal_basis, engine-trace) is de vaste taal. Voor analytische desk-review van een corpus zonder live sessie (wetgevings-/stelselfouten, coverage, multi-agent review): zie de zusterskill regelrecht-stelselanalyse.
allowed-tools: Read, Write, Edit, Grep, Glob, Bash, AskUserQuestion
---

# Regelrecht audit- & workshop-producten

Genereert de documenten waarmee een analist een machine-leesbare vertaling van
regelgeving (regelrecht-YAML) laat valideren door domein-experts — voorbereiding,
facilitatie, en nazorg van een **live validatie-sessie**.

> **Twee skills + een router.** Dit is de live-sessie-familie. De analytische
> **desk-review**-familie woont in **`regelrecht-stelselanalyse`**. Twijfel je waar te
> beginnen of hoe ze samenhangen → **`regelrecht-traject`** (voordeur).

## Routing & handoff

Feitelijke defecten routeren naar de desk (`regelrecht-stelselanalyse`); oordeels-/
praktijkvragen naar de workshop (hier). Twee workshop-modi: **verkennend** (vroeg, geen
gate — domeinkennis ontginnen op een ruwe scope-analyse) en **validerend** (gate: schema
valide + tests groen + modellering-fouten gefixt + resterende open punten zijn judgment).
Na de sessie gaan correctiepunten + bevestigde interpretaties terug de desk-cyclus in.
De cyclus en de routing: `regelrecht-traject`.

**Een sessie levert uitspraken op.** De agenda bestaat uit de claims die nog op een uitspraak
wachten. Elk antwoord landt als `replying`-notitie bij die claim, met wie sprak, het verdict
en de reden. Een verslag mag erbij, maar staat er niet voor in de plaats. Vorm:
`regelrecht-verantwoording`, stap 4.

Deze skill bevat **geen casus-inhoud**. Hij is generiek over trajecten; alleen de
regelrecht-*methode* ligt vast. Concrete casus-inhoud (welke wet, welke gronden,
welke bedragen) komt uit de aangeleverde casus-map of uit de analist tijdens het
opbouwen.

## Wanneer gebruiken

- "Ik ga een workshop / validatie-sessie voorbereiden over traject X"
- "Maak een scope-analyse / stelseloverzicht voor deze casus"
- "Maak audit-documenten voor de artikelen die we gaan doornemen"
- "Ik heb facilitator-materiaal nodig (draaiboek, spiekbriefje, jargon-uitleg)"
- "Maak testcase-scenario's voor een vervolgsessie"
- "Schrijf een verslag van de sessie (intern / voor de klant)"
- "Doe een analytische review van de machine-readable correctheid van dit corpus"

## Kernprincipe: hybride, modus per product

Niet alles wordt automatisch gegenereerd, en niet alles is handwerk. **Per product
de juiste modus** (zie `references/products.md` voor de volledige catalogus):

- **Auto-uit-YAML** (hoog): stelsel/scope-overzicht en de analytische review zijn
  grotendeels af te leiden uit het scope-manifest + de YAML's (legal_basis, source,
  overrides, outputs, untranslatables).
- **Semi-auto, analist legt focus** (midden): per-artikel audit-docs en het
  draaiboek — de skill extraheert structuur uit de YAML, maar **welke kernartikelen
  en welke focus** bepaal je samen met de analist. Vraag dit expliciet.
- **Mens-gedreven, skill structureert** (laag): verslagen en testcase-uitkomsten
  leunen op observaties/keuzes die alleen de analist heeft; de skill levert de
  structuur en stelt gericht vragen.

Forceer nooit volledige automatisering waar contextkennis van de analist nodig is.
Bij twijfel: gebruik `AskUserQuestion` om focus, sessie-doel, deelnemers of
beoordelings-paden op te halen voordat je schrijft.

## Werkstroom

1. **Oriënteer op de casus-map.** Verwacht een casus-map met een scope-manifest
   (welke wetten/artikelen in scope) + verwijzingen naar de machine-readable
   YAML's in het corpus. Lees `references/inputs.md` voor wat je nodig hebt en hoe
   je het detecteert. Ontbreekt het scope-manifest, help het dan eerst opstellen.

2. **Bepaal welke producten nodig zijn.** Vraag het, of leid het af uit de vraag.
   Zie de productcatalogus in `references/products.md`. Veel producten bouwen op
   elkaar voort (volgorde-afhankelijkheden staan daar).

3. **Bouw per product in de juiste modus.** Start vanuit het bijbehorende sjabloon
   in `templates/`. Vul uit de YAML wat automatisch kan; haal focus/keuzes op bij de
   analist waar nodig. Houd je aan de regelrecht-terminologie uit
   `references/method-glossary.md`.

4. **Schrijf naar de casus-map, niet in de skill.** Output gaat naar de
   werkomgeving van het traject (bijv. een `audit/`- of `docs/`-map van de casus).
   Wijzig nooit de sjablonen in deze skill met casus-inhoud.

5. **Commit/push alleen op verzoek.** Nooit ongevraagd pushen.

## Bestanden in deze skill

- `references/products.md` — productcatalogus: per product het doel, de input, de
  modus/automatiseringsgraad, hoe te bouwen, en de onderlinge volgorde.
- `references/method-glossary.md` — de regelrecht-concepten (YAML, engine, output,
  parameter, input, source, caller, action, definition, untranslatable, override,
  legal_basis) in vaste, generieke bewoordingen. De gedeelde taal van alle producten.
- `references/inputs.md` — wat de skill als input verwacht (casus-map, scope-manifest,
  corpus-YAML's) en hoe je dat detecteert of helpt opstellen.
- `references/facilitation-patterns.md` — herbruikbare werkvormen en facilitator-
  technieken (walk-through-protocol A/B/C, 1-2-4-all, time-boxing, risico's +
  back-pockets) die in draaiboek, brief en vervolgsessie terugkomen.
- `templates/` — generieke skeletten per product (zie catalogus voor de mapping).

## Belangrijke regels

- **Casus-agnostisch blijven.** Geen vaste wet-namen, bedragen, of casus-specifieke
  voorbeelden in de skill-bestanden. In *output* mag casus-inhoud uiteraard wel.
- **Vertrouwelijkheid.** Neem geen persoonsgegevens of vertrouwelijke casus-details op
  in producten die voor verspreiding bedoeld zijn (bijv. extern verslag, scenario's).
  Testcase-scenario's zijn fictief of sterk-gelijkend, nooit echte persoonsgegevens.
- **Twee verslag-varianten gescheiden houden.** Intern (eerlijk, reflectief, voor het
  eigen team) vs extern (hoogover, waarderend, voor de klant) — nooit vermengen.
- **Engine-uitkomsten zijn feiten, geen oordeel.** In het Claude-orakel en in
  scenario's: toon wat de YAML/engine zegt, laat het juridische oordeel bij de experts.
