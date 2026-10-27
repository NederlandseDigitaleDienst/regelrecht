---
id: referentie-casus-i
titel: Referentie casus I
faseId: wat
disciplineId: techniek
prioriteit: hoog
omvang: ''
categorie: bet
capability: ''
capaciteit: ''
toelichting: |-
  Dit werkpakket bouwt een verticale referentie-implementatie: één complete
  casus die de keten van wet tot werkend aanvraagsysteem laat zien. Het corpus
  modelleert de rekenregels van één regeling volgens het schema; de vraag is
  of die regels in de praktijk ook een systeem kunnen dragen.

  Om die rekenregels heen ontbreken nog technische constructen die een
  aanvraagsysteem nodig heeft. Er moet een herbruikbare frontend op te bouwen
  zijn. En het beoordelen van de codering moet patronen en gereedschap
  opleveren waar een volgende casus mee verder kan.

  Wat onderweg wordt aangepast, doen we zo generiek als op dat moment kan.
  Deze eerste casus beslist niet wat de generieke vorm is: hij levert een
  werkende specifieke implementatie en de signalen over wat generiek moet
  worden. Die signalen landen in de Specificaties-reeks.

  Pas als deze ene casus volledig door de keten loopt, is aangetoond dat het
  ecosysteem als geheel werkt; de overige capabilities (zoals simuleren,
  publiceren, analyseren en verifiëren) volgen in "Referentie casus II".

  **Voorlopige Definition of Done** (werkhypothese bij de vraag wanneer de
  integratie "volledig" is):

  - Een werkend aanvraagsysteem voor de casus (proces en rechtsgevolg) waar
    een aanvraag doorheen kan lopen die leidt tot een rechtsgevolg met
    traceability.
  - Datakoppelingen: het moet mogelijk zijn om data te koppelen aan een
    implementatie van de regels in het systeem.
  - Uit de manier waarop de analist de werking van deze regeling heeft
    beoordeeld, zijn de patronen en het gereedschap gedestilleerd die zich
    laten opwerken tot generieke tooling of een generiek proces.

  **Voorlopige toedeling van capabilities:** naast ontwikkelen en
  implementeren horen ook data-integratie en validatie 0.1 bij deze eerste
  casus.
volgorde: 2000
onderzoeksvragen:
  - vraag: >-
      Technisch: In het corpus worden de rekenregels uit de regelgeving
      gemodelleerd, volgens het schema. Welke technische constructen daarbuiten
      ontbreken nog om daarop in de praktijk een aanvraagsysteem te bouwen?
      Daarbij hoort het koppelen van data of representatieve overgangsdata, en
      optioneel raakvlakken met andere systemen.
    id: rc1-technische-constructen
    stories:
      - titel: Een aanvraag loopt van invoer tot rechtsgevolg door de engine
        id: rc1-aanvraag-tot-rechtsgevolg
        verhaal: >-
          Als analist wil ik een aanvraag voor de casus van begin tot eind door
          het systeem laten lopen, zodat ik zie welke constructen buiten de
          rekenregels nog ontbreken voordat er een rechtsgevolg uit komt.
        acceptatiecriteria:
          - Een ingevulde aanvraag levert een besluit op met een execution trace.
          - Elke stap die niet uit het regelwerk komt, is als ontbrekend construct genoteerd.
        voorstel: true
      - titel: Representatieve overgangsdata koppelen aan de regels
        id: rc1-overgangsdata-koppelen
        verhaal: >-
          Als analist wil ik een set representatieve gegevens aan de regels
          koppelen, zodat de aanvraag met echte situaties te beproeven is
          zonder persoonsgegevens.
        acceptatiecriteria:
          - De koppeling is beschreven in het regelwerk, niet in losse code.
          - De set dekt de gevallen die de regeling onderscheidt.
        voorstel: true
  - vraag: >-
      Frontend: Kan een herbruikbare frontend worden vormgegeven op basis van
      regelwerken, en welke generieke componenten
      zijn daarvoor nodig?
    id: rc1-herbruikbare-frontend
    stories:
      - titel: Een aanvraagscherm dat uit het regelwerk volgt
        id: rc1-aanvraagscherm-uit-regelwerk
        verhaal: >-
          Als aanvrager wil ik alleen de vragen zien die voor mijn situatie
          gelden, zodat ik niet hoef te raden welke velden ik moet invullen.
        acceptatiecriteria:
          - De volgorde en de zichtbaarheid van de vragen komen uit het regelwerk.
          - Een wijziging in het regelwerk verandert het scherm zonder handwerk.
        voorstel: true
      - titel: Een generiek component per soort invoer
        id: rc1-generiek-invoercomponent
        verhaal: >-
          Als ontwikkelaar wil ik per soort invoer (bedrag, datum, keuze) één
          herbruikbaar component, zodat een volgende casus het scherm niet
          opnieuw bouwt.
        acceptatiecriteria:
          - De componenten zijn gedocumenteerd met het regelwerkveld dat ze bedienen.
          - De casus gebruikt geen component dat alleen voor deze regeling werkt.
        voorstel: true
  - vraag: >-
      Methode: Wat zijn patronen en tooling in het analyseren en valideren van de
      regelwerken die herbruikbaar zijn voor volgende casussen, en waar
      houdt gereedschap op en begint menselijk oordeel?
    id: rc1-patronen-en-tooling
  - vraag: >-
      Generiek: Welke verbeteringen aan bestaande onderdelen van het ecosysteem,
      zoals het schema en de editor, komen uit deze casus naar boven, en hoe
      leggen we ze vast zodat een volgende casus ze niet opnieuw hoeft te
      ontdekken?
    id: rc1-verbeteringen-ecosysteem
  - vraag: >-
      Generiek: Welke elementen van de referentiecasus zijn herbruikbaar en
      abstraheerbaar voor "Referentie casus II"?
    id: rc1-herbruikbaar-voor-ii
bouw: deels
belegging:
  stand: opgepakt
  sinds: '2026-09-17'
rfcs:
  - 6
samenhangIds:
  - aansluiten-op-bronnen-chronolexografie
  - vaststelling-van-specificaties
  - specificaties-ii-enrichment-overhaul-landen
  - specificaties-iii-gaten-vinden-met-de-enricher
---
