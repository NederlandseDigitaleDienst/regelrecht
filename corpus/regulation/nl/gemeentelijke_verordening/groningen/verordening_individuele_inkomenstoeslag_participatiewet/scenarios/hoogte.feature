Feature: Verzoek en hoogte (Verordening IIT Groningen, artikelen 2 en 4)
  Het verzoek gaat met het formulier van het college. De toeslag is een percentage van
  de gehuwdennorm: 27,18 voor een alleenstaande, 34,81 voor een alleenstaande ouder en
  38,78 voor gehuwden. Is een van de gehuwden uitgesloten op grond van artikel 11 of 13,
  eerste lid, Participatiewet, dan krijgt de ander het bedrag voor een alleenstaande of
  alleenstaande ouder.

  # Bron: toelichting bij de verordening, CVDR367471/3
  #   (dossiers/iit-groningen/bronnen/gemeente/tekst/CVDR367471_3-toelichting.txt)
  # URL: https://lokaleregelgeving.overheid.nl/CVDR367471/3
  #
  # Deze scenario's toetsen het percentage en niet het bedrag. De Participatiewet in het
  # corpus heeft alleen de normen van 2022, dus een bedrag voor 2023 of later zou
  # verkeerd zijn (bevinding M9, dossiers/iit-groningen/regelwerken-model.md).
  #
  # Niet als scenario opgenomen: "Voldoet één van hen niet aan deze voorwaarden, dan
  # bestaat voor beiden geen recht" (regel 32). Die voorwaarden zijn die van artikel 36
  # Participatiewet, en dat artikel staat nog niet in het corpus.

  Background:
    Given the calculation date is "2023-05-01"

  Scenario: Een mondeling verzoek voldoet niet aan artikel 2
    # Bron: toelichting, artikel 2 (regel 17): "Een mondeling verzoek kan hiermee dus niet
    # worden aangemerkt als een verzoek om individuele inkomenstoeslag".
    Given the following parameters:
      | verzoek_ingediend_middels_vastgesteld_formulier | false |
    When I evaluate "verzoek_voldoet_aan_artikel_2" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "verzoek_voldoet_aan_artikel_2" is false

  Scenario: Een verzoek met het formulier voldoet aan artikel 2
    # Bron: toelichting, artikel 2 (regel 17).
    Given the following parameters:
      | verzoek_ingediend_middels_vastgesteld_formulier | true |
    When I evaluate "verzoek_voldoet_aan_artikel_2" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "verzoek_voldoet_aan_artikel_2" is true

  Scenario: Een alleenstaande krijgt 27,18 procent van de gehuwdennorm
    # Bron: toelichting, artikel 4 (regel 28): "onderscheid ... tussen een alleenstaande,
    # een alleenstaande ouder en gehuwden"; het percentage uit de tekst van artikel 4.
    Given parameter "leefvorm_op_peildatum" is "alleenstaande"
    Given the following parameters:
      | echtgenoot_uitgesloten_ingevolge_artikel_11_of_13 | false |
    When I evaluate "toepasselijk_percentage" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "toepasselijk_percentage" equals 27.18

  Scenario: Een alleenstaande ouder krijgt 34,81 procent van de gehuwdennorm
    # Bron: toelichting, artikel 4 (regel 28): "onderscheid ... tussen een alleenstaande,
    # een alleenstaande ouder en gehuwden"; het percentage uit de tekst van artikel 4.
    Given parameter "leefvorm_op_peildatum" is "alleenstaande ouder"
    Given the following parameters:
      | echtgenoot_uitgesloten_ingevolge_artikel_11_of_13 | false |
    When I evaluate "toepasselijk_percentage" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "toepasselijk_percentage" equals 34.81

  Scenario: Gehuwden krijgen samen 38,78 procent van de gehuwdennorm
    # Bron: toelichting, artikel 4 (regel 28): "onderscheid ... tussen een alleenstaande,
    # een alleenstaande ouder en gehuwden"; het percentage uit de tekst van artikel 4.
    Given parameter "leefvorm_op_peildatum" is "gehuwden"
    Given the following parameters:
      | echtgenoot_uitgesloten_ingevolge_artikel_11_of_13 | false |
    When I evaluate "toepasselijk_percentage" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "toepasselijk_percentage" equals 38.78

  Scenario: Een uitgesloten echtgenoot: de ander krijgt het percentage van een alleenstaande
    # Bron: toelichting, artikel 4 onder "Gehuwden" (regel 33): "komt deze rechthebbende
    # partner in aanmerking voor een individuele inkomenstoeslag naar de hoogte die voor
    # hem als alleenstaande of alleenstaande ouder zou gelden."
    Given parameter "leefvorm_op_peildatum" is "gehuwden"
    Given the following parameters:
      | echtgenoot_uitgesloten_ingevolge_artikel_11_of_13 | true |
    Given the following parameters:
      | rechthebbende_echtgenoot_zou_alleenstaande_ouder_zijn | false |
    When I evaluate "toepasselijk_percentage" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "toepasselijk_percentage" equals 27.18

  Scenario: Een uitgesloten echtgenoot, met kinderen: het percentage van een alleenstaande ouder
    # Bron: toelichting, artikel 4 onder "Gehuwden" (regel 33).
    Given parameter "leefvorm_op_peildatum" is "gehuwden"
    Given the following parameters:
      | echtgenoot_uitgesloten_ingevolge_artikel_11_of_13 | true |
    Given the following parameters:
      | rechthebbende_echtgenoot_zou_alleenstaande_ouder_zijn | true |
    When I evaluate "toepasselijk_percentage" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "toepasselijk_percentage" equals 34.81
