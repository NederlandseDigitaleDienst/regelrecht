---
id: lakmoesproef-besluiten-execution-trace
titel: Lakmoesproef besluiten met execution trace en burgerdossier
faseId: wat
disciplineId: service-design
prioriteit: ''
omvang: ''
categorie: bet
capability: ''
capaciteit: service designer, design engineer, software engineer, analist, UX-researcher
toelichting: |-
  Ontwerp en valideer met burgers en intermediairs een interactieontwerp voor
  een burgerdossier waarin burgers hun rechtspositie kunnen borgen. Het dossier
  combineert: chronolexografie (tijdgebonden feiten in cellen, synthese bij de
  burger) en Rules as Executed (gepubliceerde regel, herberekenbare execution
  trace). Proefcasus moet worden gekozen.

  **Probleem:** Huidige portalen tonen toestanden (bv. "uw voorschot is € 98")
  zonder herkomst, tijd of regel. Burgers kunnen daardoor niet vaststellen op
  welke feiten een besluit rustte, niet zien dat bronnen elkaar tegenspreken,
  niet controleren of de regel juist is toegepast en niet bewijzen wat zij zelf
  hebben gedaan. Beide position papers beschrijven een oplossing op
  modelniveau; geen van beide is bij burgers getest.

  **Doel:** Een interactieontwerp dat aantoonbaar zes vermogens ondersteunt en
  burgers niet misleidt bij onvolledigheid, gevalideerd in drie onderzoeksrondes
  op een werkende, synthetisch gevulde proefopstelling:

  - **V0** niet misleid worden bij ontbrekende cel, gemiste notificatie of
    niet-uitvoerbare regel
  - **V1** vaststellen op welke feiten een besluit rustte en uit welke bron
  - **V2** herkennen dat de uitkomst afwijkt van de gepubliceerde regel
    (herberekening van de trace)
  - **V3** herkennen dat twee bronnen tegenstrijdige feiten bevatten
  - **V4** de juiste handeling met de juiste termijn vinden
  - **V5** achteraf bewijzen wat men zelf heeft gedaan en wat de overheid heeft
    verklaard

  **Opgeleverd:** ontwerprichtlijnen burgerdossier, gevalideerd prototype, lijst
  open juridische vragen.

  **Buiten scope:** authenticatie, echte gegevens, echte besluiten,
  cryptografische attestatie, koppeling met productiesystemen, juridische
  invoering.
volgorde: 4000
onderzoek: open
bouw: ''
rfcs:
  - 8
  - 13
onderzoeksvragen:
  - vraag: >-
      Hoe geef je de execution trace weer zodat burgers hem voldoende begrijpen om
      hun rechtspositie te borgen, en welke laag doet dat werk: de uitleg in gewone
      taal of de trace zelf?
    paper: sec:explaining
  - vraag: >-
      Hoe stellen we vast dat een uitleg de beslissende voorwaarde juist benoemt,
      en niet alleen dat lezers hem begrijpelijk vinden?
    paper: sec:explaining
  - Is het onderscheid feit/afleiding begrijpelijk in de gekozen presentatie, en
    welke metafoor werkt als dat niet zo is?
  - Helpt het om per waarde te tonen waar hij vandaan komt, meegegeven bij de
    uitvoering, opgehaald bij een ander register, of gepubliceerd in de regeling
    zelf?
  - Is ordening per rechtsverhouding de juiste standaard, of per organisatie of
    per gebeurtenis?
  - Welke notificaties willen burgers?
  - Op welk punt willen burgers een mens erbij, en hoe geeft het dossier die
    overdracht (machtiging aan intermediair) vorm?
  - vraag: >-
      Wat doet een burger met een aantoonbaar verschil tussen zijn besluit en de
      herberekening, en waar loopt hij in de huidige procedure vast?
    paper: sec:runningcase
  - vraag: >-
      Heeft de burger recht op de trace bij ontvangst van het besluit, wat is de
      bewijsstatus van een verstrekkingsfeit, en wat krijgt hij te zien wanneer die
      trace ook waarden over een ander bevat?
    id: trace-bij-ontvangst
    paper: sec:traceaccess
    stories:
      - titel: De trace zit bij het besluit
        id: trace-bij-besluit
        verhaal: >-
          Als burger wil ik bij mijn besluit de execution trace kunnen inzien,
          zodat ik kan nagaan op welke feiten en welke regel het rustte.
        acceptatiecriteria:
          - Het besluit verwijst naar de trace waarmee het is berekend.
          - De trace is herberekenbaar met de gepubliceerde regel.
        voorstel: true
      - titel: Waarden over een ander blijven buiten beeld
        id: trace-waarden-over-ander
        verhaal: >-
          Als burger wil ik in mijn trace alleen de waarden zien die over mij
          gaan, zodat gegevens van een ander niet via mijn besluit bij mij
          terechtkomen.
        acceptatiecriteria:
          - Een waarde over een ander is in de trace vervangen door een verwijzing.
          - De herberekening blijft mogelijk zonder die waarde te tonen.
        voorstel: true
samenhangIds:
  - informatiepositie-burgers-ondernemers
  - lakmoesproef-valideren-en-vaststellen
  - controle-en-herstel
  - aansluiten-op-bronnen-chronolexografie
---
