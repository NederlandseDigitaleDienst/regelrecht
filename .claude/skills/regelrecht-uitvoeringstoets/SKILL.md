---
name: regelrecht-uitvoeringstoets
description: Genereert uit een (semi-)gevalideerd regelrecht-corpus een geloofwaardige service-PoC (burger- + behandelaar-portaal, of een headless beslis-service) en zet die in als validatie-instrument met uitvoeringsexperts — om tastbaar te maken wat nodig is om een wet rechtvaardig in de praktijk te brengen, gezien vanuit de betrokkene. Gebruik dit als workshop-fase ná de logica-/scenariovalidatie (audit-products) om de dienstverlening-kant te valideren: welke service impliceert de wet, waar zit menselijk oordeel, welke last ligt bij de burger, welke termijnen, waar is de wet hard of onduidelijk. Casus-agnostisch; de regelrecht-methode (hooks, legal_character, untranslatables, source/implements, type_spec, receipts) is de vaste taal. Voor de logica-/scenariovalidatie zelf: zie de zusterskill regelrecht-audit-products.
allowed-tools: Read, Write, Edit, Grep, Glob, Bash, AskUserQuestion
---

# Regelrecht uitvoeringstoets — dienstverlening-validatie via een service-PoC

Genereert uit een corpus een geloofwaardige **service-PoC** en zet die in als
**validatie-instrument**: niet om een app op te leveren, maar om met uitvoeringsexperts
inzichtelijk te maken wat nodig is om de wet **rechtvaardig in de praktijk** te brengen.
De PoC is het middel; het **validatie-inzicht** is de uitkomst — maar de PoC moet zó goed
zijn dat het "echt had kunnen zijn", anders valideert het niet eerlijk.

> **Niet de formele uitvoeringstoets.** De naam sluit aan op de overheidsterm, maar deze
> skill levert *materiaal dat een uitvoeringstoets-achtige validatie ondersteunt* — geen
> formele toets-procedure.

> **De familie + router.** Dit is **workshop-fase 2 (dienstverlening)**, ná de logica-/
> scenariovalidatie. Logica valideren = `regelrecht-audit-products`. Desk-review/corpus-
> completion = `regelrecht-stelselanalyse`. Casussen/persona's/traces = `regelrecht-
> scenario-traces`. Twijfel je waar te beginnen → `regelrecht-traject` (voordeur).

## Geen casus-inhoud

Deze skill is **casus-agnostisch**. Hij bevat geen organisatie-, persoons-, functie-,
of domeinnamen, geen concrete law-ids/param-namen/bedragen, en geen onderbouwingen uit een
specifieke casus. Concrete waarden leven **uitsluitend** in het (privé) corpus en in de
at-runtime gegenereerde service-spec van de (privé) PoC — nooit in deze skill of de
referentie-template. Zie `references/definition-of-clean.md` (de leak-discipline) en houd je
eraan bij élk gegenereerd of toegevoegd bestand.

## Wanneer gebruiken

- "De logica/scenario's zijn gevalideerd — laten we de **dienstverlening** valideren."
- "Maak een service-PoC waarmee uitvoeringsexperts kunnen ervaren wat de wet voor betrokkenen betekent."
- "Wat voor service impliceert deze wet — een zaaksysteem, een melding, een ambtshalve flow, of een headless beslis-service?"
- "Maak workshop-materiaal (draaiboek) voor een dienstverlening-validatiesessie."
- "Oogst de bevindingen van zo'n sessie en route ze terug de cyclus in."

## Routing & handoff

- **Instroom**: een corpus op minstens "logica-/scenario-gevalideerd of semi-gevalideerd"
  niveau, plus ≥1 gevalideerd scenario/persona (uit `regelrecht-scenario-traces`) en het
  open-vragen-register (uit `regelrecht-audit-products`/`-stelselanalyse`).
- **Uitstroom (twee kanalen)**: wets-/logica-bevindingen → terug naar de desk
  (`regelrecht-stelselanalyse`); dienstverlening-bevindingen (last, begrijpelijkheid,
  toegankelijkheid, proces-/interactiekeuzes) → een apart service-backlog.

## De methode — vijf stappen

1. **Ingangscheck** — is de logica + ≥1 gevalideerd scenario/persona + open-vragen-register
   aanwezig? Soepel: bij gaten **waarschuwen en markeren**, niet weigeren. De markeringen
   zíjn validatie-materiaal. Het gereedheid-contract staat in `references/service-spec.md`.
2. **Afleiden** — leid de **service-spec** af uit corpus-signalen (zie de afleidingstabel in
   `references/service-spec.md`): hooks → levenscyclus; legal_character/produces → besluit
   + bezwaar/beroep; untranslatables → mens-taken; caller-params + herkomst → de last en of
   een portaal zinvol is; source/implements → ketensamenwerking; type_spec → termijnen.
   Het **archetype** (aanvraag→beschikking, melding, ambtshalve, headless beslis-service) is
   een *emergente samenvatting* van de gevonden dimensies, geen keuze vooraf. Toon → bevestig.
3. **Genereren** — bouw de PoC uit de **referentie-template-repo** (de casus-agnostische
   architectuur) + de gegenereerde service-spec, gevuld met de gevalideerde persona's. Scaffold
   de privacy-guards mee (privé-repo-vereiste, fail-closed pre-push, fictieve data). Zie
   `references/service-spec.md` voor het contract en de verpakking.
4. **Instrumenteren** — maak de PoC tot validatie-instrument: per scherm artikel-herkomst,
   open vragen zichtbaar, **wat-als-schakelaars** (een open beleidskeuze: variant A vs B),
   feedback-vangst, en **eerlijkheid over de grenzen** (fictieve data; corpus-correct ≠
   beleids-correct; menselijk oordeel niet wegautomatiseren).
5. **Faciliteren & oogsten** — draai de sessie met `templates/workshop-draaiboek.md`
   (deelnemers: uitvoeringsexperts + juristen/beleid + ontwerpers/dienstverlening; burgers/
   ervaringsdeskundigen = bewust groeipad, niet standaard). Leg de uitkomst vast met
   `templates/bevindingen-verslag.md` en route ze via de twee kanalen.

## Afwezigheid is een bevinding

Waar een corpus-signaal ontbreekt of dubbelzinnig is, genereer je geen aanname maar een
**bevinding**: geen bezwaar-hook → "is er echt geen bezwaarweg, of is het corpus incompleet?";
een param die niemand kan leveren → last-/uitvoerbaarheidsvraag; een untranslatable zonder
eigenaar → wie doet dit oordeel, en hoe? De afleiding levert zo meteen de agenda voor de sessie.

## Verpakking

- **Deze skill** orkestreert (afleiden, genereren, instrumenteren, faciliteren).
- De **referentie-template-repo** levert de échte, casus-agnostische architectuur (engine =
  rekenmeester, wet = procesflow, lenzen, receipts, append-only audit). Casus-specifieke
  waarden komen uit de gegenereerde service-spec, niet uit de template. Zie
  `references/service-spec.md`.

## Ontwerp & rationale

De volledige ontwerpredenering (waarom fase-2, de eenheid-van-waarde, de afleiding als kern,
de onderhoudbaarheids-eisen aan de template) staat in `PLAN.md` naast dit bestand.
