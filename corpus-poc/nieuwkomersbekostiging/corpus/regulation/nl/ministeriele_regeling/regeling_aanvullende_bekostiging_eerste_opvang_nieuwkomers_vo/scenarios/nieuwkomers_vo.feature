# Scenario's voor de Regeling aanvullende bekostiging eerste opvang
# nieuwkomers vo. De rekendatum is de peildatum. Vanaf 2026 knoopt de
# begripsbepaling "nieuwkomer" (Uitvoeringsbesluit WVO 2020, art. 1.1) aan
# bij de eerste inschrijfdatum; tot en met 2025 bij de datum van vestiging
# in Nederland. Bedragen 2026: EUR 3.909,08 (eerste categorie), EUR 1.426,33
# (tweede categorie), EUR 21.843,34 (voorbereidingskosten); 2025:
# EUR 3.750,86 / EUR 1.373,99 / EUR 21.041,75.
Feature: Aanvullende bekostiging eerste opvang nieuwkomers vo

  Background:
    Given law "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo" is loaded
    Given law "uitvoeringsbesluit_wvo_2020" is loaded

  Scenario: Veertienjarige vreemdeling, eerste inschrijving 1 september 2025, is nieuwkomer eerste categorie
    Given the calculation date is "2026-01-01"
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | sector | internationaal_georienteerd | school_id |
      | "200000001" | 2011-05-14    | true           | true      | 2025-08-15                | 2025-09-01            | true                    | true                  | vo     | false                       | 01AB      |
    Given parameter "bsn" is "200000001"
    When I evaluate "is_nieuwkomer" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "is_nieuwkomer" is true
    When I evaluate "categorie_nieuwkomer_vo" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "categorie_nieuwkomer_vo" equals "EERSTE"
    When I evaluate "bedrag_kwartaal" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "bedrag_kwartaal" equals 390908
    When I evaluate "aanvraag_vereist" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "aanvraag_vereist" is false
    When I evaluate "vaststelling_vanaf" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "vaststelling_vanaf" equals "2026-02-01"

  Scenario: Eerste inschrijving 1 maart 2024 is tweede categorie op 1 januari 2026 en geen nieuwkomer meer op 1 april 2026
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | sector | internationaal_georienteerd | school_id |
      | "200000002" | 2010-02-20    | true           | true      | 2024-01-10                | 2024-03-01            | true                    | true                  | vo     | false                       | 01AB      |
    Given parameter "bsn" is "200000002"
    Given the calculation date is "2026-01-01"
    When I evaluate "is_nieuwkomer" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "is_nieuwkomer" is true
    When I evaluate "categorie_nieuwkomer_vo" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "categorie_nieuwkomer_vo" equals "TWEEDE"
    When I evaluate "bedrag_kwartaal" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "bedrag_kwartaal" equals 142633
    # Op 1 april 2026 is de eerste inschrijving twee jaar en een maand geleden.
    Given the calculation date is "2026-04-01"
    When I evaluate "is_nieuwkomer" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "is_nieuwkomer" is false
    When I evaluate "categorie_nieuwkomer_vo" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "categorie_nieuwkomer_vo" equals "GEEN"
    When I evaluate "bedrag_kwartaal" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "bedrag_kwartaal" equals 0

  Scenario: Leerling met de Nederlandse nationaliteit is geen nieuwkomer
    Given the calculation date is "2026-01-01"
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | sector | internationaal_georienteerd | school_id |
      | "200000003" | 2011-09-03    | false          | true      | 2025-08-01                | 2025-09-01            | true                    | true                  | vo     | false                       | 01AB      |
    Given parameter "bsn" is "200000003"
    When I evaluate "is_nieuwkomer" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "is_nieuwkomer" is false
    When I evaluate "categorie_nieuwkomer_vo" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "categorie_nieuwkomer_vo" equals "GEEN"
    When I evaluate "bedrag_kwartaal" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "bedrag_kwartaal" equals 0

  Scenario: Leerling in internationaal georienteerd voortgezet onderwijs is geen nieuwkomer
    Given the calculation date is "2026-01-01"
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | sector | internationaal_georienteerd | school_id |
      | "200000004" | 2011-01-30    | true           | true      | 2025-07-20                | 2025-09-01            | true                    | true                  | vo     | true                        | 02CD      |
    Given parameter "bsn" is "200000004"
    When I evaluate "is_nieuwkomer" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "is_nieuwkomer" is false
    When I evaluate "categorie_nieuwkomer_vo" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "categorie_nieuwkomer_vo" equals "GEEN"

  Scenario: Eerste inschrijving in het basisonderwijs telt door na de overstap naar het vo
    Given the calculation date is "2026-01-01"
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | sector | internationaal_georienteerd | school_id |
      | "200000005" | 2013-04-12    | true           | true      | 2024-05-01                | 2024-06-01            | true                    | true                  | vo     | false                       | 01AB      |
    Given parameter "bsn" is "200000005"
    When I evaluate "is_nieuwkomer" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "is_nieuwkomer" is true
    When I evaluate "categorie_nieuwkomer_vo" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "categorie_nieuwkomer_vo" equals "TWEEDE"
    When I evaluate "bedrag_kwartaal" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "bedrag_kwartaal" equals 142633

  Scenario: School met twaalf nieuwkomers die voor het eerst eerste opvang organiseert krijgt de voorbereidingskosten
    Given the calculation date is "2026-01-01"
    Given the following "scholen" data with key "school_id":
      | school_id | schoolsoort | aantal_nieuwkomers_peildatum | eerste_keer_eerste_opvang |
      | 01AB      | vo          | 12                           | true                      |
    Given parameter "school_id" is "01AB"
    When I evaluate "voorbereidingskosten" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "voorbereidingskosten" equals 2184334
    When I evaluate "uiterste_ontvangstdatum_aanvraag_voorbereidingskosten" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "uiterste_ontvangstdatum_aanvraag_voorbereidingskosten" equals "2026-01-15"
    When I evaluate "aanvraag_vereist_voorbereidingskosten" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "aanvraag_vereist_voorbereidingskosten" is true

  Scenario: School met negen nieuwkomers of met eerdere eerste opvang krijgt geen voorbereidingskosten
    Given the calculation date is "2026-04-01"
    Given the following "scholen" data with key "school_id":
      | school_id | schoolsoort | aantal_nieuwkomers_peildatum | eerste_keer_eerste_opvang |
      | 03EF      | vo          | 9                            | true                      |
      | 04GH      | vo          | 25                           | false                     |
    Given parameter "school_id" is "03EF"
    When I evaluate "voorbereidingskosten" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "voorbereidingskosten" equals 0
    Given parameter "school_id" is "04GH"
    When I evaluate "voorbereidingskosten" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "voorbereidingskosten" equals 0

  Scenario: Peildatum 2025 rekent met de datum van vestiging in Nederland (redactie 2025)
    Given the calculation date is "2025-07-01"
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | sector | internationaal_georienteerd | school_id |
      | "200000011" | 2010-11-05    | true           | true      | 2024-05-01                | 2024-06-15            | true                    | true                  | vo     | false                       | 01AB      |
      | "200000012" | 2010-08-22    | true           | true      | 2023-09-01                | 2023-10-15            | true                    | true                  | vo     | false                       | 01AB      |
    # Op 1 oktober 2023 nog niet in Nederland: eerste categorie.
    Given parameter "bsn" is "200000011"
    When I evaluate "categorie_nieuwkomer_vo" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "categorie_nieuwkomer_vo" equals "EERSTE"
    When I evaluate "bedrag_kwartaal" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "bedrag_kwartaal" equals 375086
    # Op 1 oktober 2023 al in Nederland, op de peildatum korter dan twee jaar: tweede categorie.
    Given parameter "bsn" is "200000012"
    When I evaluate "categorie_nieuwkomer_vo" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "categorie_nieuwkomer_vo" equals "TWEEDE"
    When I evaluate "bedrag_kwartaal" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "bedrag_kwartaal" equals 137399

  Scenario: Late eerste inschrijving: geen nieuwkomer onder de redactie 2025, wel onder de redactie 2026
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | sector | internationaal_georienteerd | school_id |
      | "200000013" | 2010-03-17    | true           | true      | 2023-06-01                | 2024-02-01            | true                    | true                  | vo     | false                       | 01AB      |
    Given parameter "bsn" is "200000013"
    # 1 juli 2025: twee jaar en een maand in Nederland, dus geen nieuwkomer (verblijfsduur).
    Given the calculation date is "2025-07-01"
    When I evaluate "is_nieuwkomer" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "is_nieuwkomer" is false
    When I evaluate "categorie_nieuwkomer_vo" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "categorie_nieuwkomer_vo" equals "GEEN"
    # 1 januari 2026: eerste inschrijving een jaar en elf maanden geleden, dus nieuwkomer (inschrijving).
    Given the calculation date is "2026-01-01"
    When I evaluate "is_nieuwkomer" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "is_nieuwkomer" is true
    When I evaluate "categorie_nieuwkomer_vo" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "categorie_nieuwkomer_vo" equals "TWEEDE"
    When I evaluate "bedrag_kwartaal" of "regeling_aanvullende_bekostiging_eerste_opvang_nieuwkomers_vo"
    Then output "bedrag_kwartaal" equals 142633
