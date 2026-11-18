# Werkinstructie DUO: tabbladen van het Bestand Nieuwkomers en de
# aanvraagdeadline per peildatum (rekendatum = peildatum).
Feature: Bestand Nieuwkomers en aanvraagdeadlines (werkinstructie DUO)

  Background:
    Given law "duo_werkinstructie_bestand_nieuwkomers" is loaded

  Scenario: Peildatum 1 juli 2026 heeft een deadline van acht weken vanwege de zomervakantie
    Given the calculation date is "2026-07-01"
    When I evaluate "aanvraagdeadline" of "duo_werkinstructie_bestand_nieuwkomers"
    Then output "aanvraagdeadline" equals "2026-08-26"
    When I evaluate "bestand_beschikbaar_op" of "duo_werkinstructie_bestand_nieuwkomers"
    Then output "bestand_beschikbaar_op" equals "2026-07-08"

  Scenario: Peildatum 1 april 2026 heeft een deadline van vier weken
    Given the calculation date is "2026-04-01"
    When I evaluate "aanvraagdeadline" of "duo_werkinstructie_bestand_nieuwkomers"
    Then output "aanvraagdeadline" equals "2026-04-29"

  Scenario: Verblijfstitelcode 21 komt op tabblad 2 en het bestuur bepaalt de categorie
    Given the calculation date is "2026-01-01"
    Given the following "personas" data with key "bsn":
      | bsn         | heeft_bsn | verblijfstitel_code |
      | "400000001" | true      | 21                  |
      | "400000002" | true      | 26                  |
      | "400000003" | true      | null                |
    Given parameter "bsn" is "400000001"
    When I evaluate "tabblad" of "duo_werkinstructie_bestand_nieuwkomers"
    Then output "tabblad" equals 2
    Given parameter "bsn" is "400000002"
    When I evaluate "tabblad" of "duo_werkinstructie_bestand_nieuwkomers"
    Then output "tabblad" equals 1
    Given parameter "bsn" is "400000003"
    When I evaluate "tabblad" of "duo_werkinstructie_bestand_nieuwkomers"
    Then output "tabblad" equals 2

  Scenario: Leerling zonder bsn staat op tabblad 3 en het bestuur beoordeelt alle voorwaarden
    Given the calculation date is "2026-01-01"
    Given the following "personas" data with key "bsn":
      | bsn        | heeft_bsn | verblijfstitel_code |
      | ON00000004 | false     | null                |
    Given parameter "bsn" is "ON00000004"
    When I evaluate "tabblad" of "duo_werkinstructie_bestand_nieuwkomers"
    Then output "tabblad" equals 3
