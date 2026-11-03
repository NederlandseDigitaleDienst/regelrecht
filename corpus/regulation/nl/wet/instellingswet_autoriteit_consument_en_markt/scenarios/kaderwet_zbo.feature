Feature: De Kaderwet zbo haakt in op de besluiten van de ACM
  Kaderwet zbo artikel 22, eerste lid, geeft de minister de bevoegdheid elk
  besluit van een zelfstandig bestuursorgaan te vernietigen. De ACM is zo'n
  orgaan (Instellingswet ACM artikel 2). Artikel 10 van die wet wijkt af:
  lid 3 sluit artikel 22, eerste lid, uit voor besluiten op grond van een
  sectorwet of op het gebied van mededinging, en lid 1 beperkt de
  bevoegdheid voor de overige besluiten tot besluiten van algemene
  strekking.

  Background:
    Given the calculation date is "2026-01-01"
    Given law "kaderwet_zelfstandige_bestuursorganen" is loaded

  Scenario: Een boete op het gebied van mededinging: artikel 22 is uitgesloten
    Given the following parameters:
      | overtreding            | AWB_5_20    |
      | gebied_van_het_besluit | MEDEDINGING |
    When I evaluate "bestuurlijke_boete_kan_worden_opgelegd" of "instellingswet_autoriteit_consument_en_markt"
    Then the execution succeeds
    Then output "bestuurlijke_boete_kan_worden_opgelegd" is true
    Then output "motivering_vereist" is true
    # Kaderwet zbo artikel 22 draait niet; de Awb-hooks wel.
    Then output "minister_kan_besluit_vernietigen" has voided provenance

  Scenario: Een boete op een ander gebied: artikel 22 vuurt, lid 1 beperkt het
    Given the following parameters:
      | overtreding                     | AWB_5_20                   |
      | gebied_van_het_besluit          | CONSUMENTENBESCHERMING     |
      | besluit_van_algemene_strekking  | false                      |
    When I evaluate "bestuurlijke_boete_kan_worden_opgelegd" of "instellingswet_autoriteit_consument_en_markt"
    Then the execution succeeds
    Then output "minister_kan_besluit_vernietigen" is false
    Then output "minister_kan_besluit_vernietigen" has override provenance

  Scenario: Een besluit op grond van een sectorwet: artikel 22 is uitgesloten
    Given the following parameters:
      | overtreding            | ZELFSTANDIGE_LAST |
      | gebied_van_het_besluit | ENERGIE           |
      | op_grond_van_sectorwet | true              |
    When I evaluate "bestuurlijke_boete_kan_worden_opgelegd" of "instellingswet_autoriteit_consument_en_markt"
    Then the execution succeeds
    Then output "minister_kan_besluit_vernietigen" has voided provenance

  Scenario: Een besluit van algemene strekking op een ander gebied: de minister kan het vernietigen
    # Een boete is een beschikking en nooit van algemene strekking; dit
    # scenario laat alleen de andere tak van lid 1 zien.
    Given the following parameters:
      | overtreding                    | AWB_5_20               |
      | gebied_van_het_besluit         | CONSUMENTENBESCHERMING |
      | besluit_van_algemene_strekking | true                   |
    When I evaluate "bestuurlijke_boete_kan_worden_opgelegd" of "instellingswet_autoriteit_consument_en_markt"
    Then the execution succeeds
    Then output "minister_kan_besluit_vernietigen" is true
    Then output "minister_kan_besluit_vernietigen" has override provenance
