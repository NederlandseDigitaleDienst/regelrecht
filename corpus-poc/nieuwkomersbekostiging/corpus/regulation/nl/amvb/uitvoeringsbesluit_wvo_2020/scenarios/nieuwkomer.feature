# Begripsbepaling "nieuwkomer" (art. 1.1) en de uitsluiting van de reguliere
# telling (art. 6.8, eerste lid, onderdeel d) van het Uitvoeringsbesluit WVO
# 2020, in de redactie van 2026 (eerste inschrijfdatum) en die van 2025
# (datum van vestiging in Nederland).
Feature: Nieuwkomer volgens het Uitvoeringsbesluit WVO 2020

  Background:
    Given law "uitvoeringsbesluit_wvo_2020" is loaded

  Scenario: Nieuwkomer die korter dan een jaar staat ingeschreven telt niet mee in de reguliere telling (2026)
    Given the calculation date is "2026-01-01"
    Given the following "personas" data with key "bsn":
      | bsn         | is_vreemdeling | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | internationaal_georienteerd |
      | "300000001" | true           | 2025-08-15                | 2025-09-01            | true                    | true                  | false                       |
      | "300000002" | true           | 2024-01-10                | 2024-03-01            | true                    | true                  | false                       |
    Given parameter "bsn" is "300000001"
    When I evaluate "is_nieuwkomer" of "uitvoeringsbesluit_wvo_2020"
    Then output "is_nieuwkomer" is true
    When I evaluate "korter_dan_een_jaar_ingeschreven" of "uitvoeringsbesluit_wvo_2020"
    Then output "korter_dan_een_jaar_ingeschreven" is true
    # Eerste inschrijving een jaar en tien maanden geleden: wel nieuwkomer, wel meetellen.
    Given parameter "bsn" is "300000002"
    When I evaluate "is_nieuwkomer" of "uitvoeringsbesluit_wvo_2020"
    Then output "is_nieuwkomer" is true
    When I evaluate "korter_dan_een_jaar_ingeschreven" of "uitvoeringsbesluit_wvo_2020"
    Then output "korter_dan_een_jaar_ingeschreven" is false

  Scenario: Leerling die nog niet is ingeschreven op de teldatum is geen nieuwkomer
    Given the calculation date is "2026-01-01"
    Given the following "personas" data with key "bsn":
      | bsn         | is_vreemdeling | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | internationaal_georienteerd |
      | "300000003" | true           | 2025-12-20                | 2026-02-01            | true                    | true                  | false                       |
    Given parameter "bsn" is "300000003"
    When I evaluate "is_nieuwkomer" of "uitvoeringsbesluit_wvo_2020"
    Then output "is_nieuwkomer" is false

  Scenario: In 2025 telt de verblijfsduur in Nederland, niet de eerste inschrijving
    Given the calculation date is "2025-10-01"
    Given the following "personas" data with key "bsn":
      | bsn         | is_vreemdeling | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | internationaal_georienteerd |
      | "300000004" | true           | 2025-02-01                | 2025-09-01            | true                    | true                  | false                       |
      | "300000005" | true           | 2023-06-01                | 2024-02-01            | true                    | true                  | false                       |
    # Acht maanden in Nederland: nieuwkomer, korter dan een jaar.
    Given parameter "bsn" is "300000004"
    When I evaluate "is_nieuwkomer" of "uitvoeringsbesluit_wvo_2020"
    Then output "is_nieuwkomer" is true
    When I evaluate "korter_dan_een_jaar_ingeschreven" of "uitvoeringsbesluit_wvo_2020"
    Then output "korter_dan_een_jaar_ingeschreven" is true
    # Twee jaar en vier maanden in Nederland: geen nieuwkomer, ondanks late inschrijving.
    Given parameter "bsn" is "300000005"
    When I evaluate "is_nieuwkomer" of "uitvoeringsbesluit_wvo_2020"
    Then output "is_nieuwkomer" is false
    When I evaluate "korter_dan_een_jaar_ingeschreven" of "uitvoeringsbesluit_wvo_2020"
    Then output "korter_dan_een_jaar_ingeschreven" is false
