Feature: In welke maanden het voorschot wordt uitbetaald
  Artikel 22. Een voorschot dat vóór het berekeningsjaar wordt verleend,
  gaat in twaalf termijnen vanaf december daarvoor (eerste lid). Een
  voorschot dat in de loop van het jaar wordt verleend, gaat in termijnen
  vanaf de maand van dagtekening tot en met november (tweede lid). Verleent
  de beschikking voor het eerst een voorschot, dan gaat het deel over de
  verstreken maanden in één bedrag (vierde lid). Na 31 oktober gaat het hele
  voorschot in één bedrag (vijfde lid). Een maand telt als het aantal
  maanden na januari van het berekeningsjaar.

  Background:
    Given the calculation date is "2025-06-30"

  Scenario: Verleend vóór het berekeningsjaar: twaalf termijnen vanaf december
    Given the following parameters:
      | dagtekening_voorschot   | 2024-11-15 |
      | berekeningsjaar         | 2025       |
      | maand                   | 2024-12-20 |
      | voor_het_eerst_verleend | true       |
    When I evaluate outputs "aantal_termijnen, termijn_in_maand, eerste_termijn, maanden_ineens, resterende_termijnen" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then the execution succeeds
    Then output "aantal_termijnen" equals 12
    Then output "termijn_in_maand" is true
    Then output "eerste_termijn" is true
    Then output "maanden_ineens" equals 0
    Then output "resterende_termijnen" equals 12

  Scenario: Verleend in januari: elf termijnen, tot en met november
    Given the following parameters:
      | dagtekening_voorschot   | 2025-01-06 |
      | berekeningsjaar         | 2025       |
      | maand                   | 2025-12-20 |
      | voor_het_eerst_verleend | true       |
    When I evaluate outputs "aantal_termijnen, termijn_in_maand, maanden_ineens" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then the execution succeeds
    Then output "aantal_termijnen" equals 11
    Then output "termijn_in_maand" is false
    Then output "maanden_ineens" equals 0

  Scenario: Voor het eerst verleend in maart: januari tot en met maart in één bedrag
    Given the following parameters:
      | dagtekening_voorschot   | 2025-03-14 |
      | berekeningsjaar         | 2025       |
      | maand                   | 2025-03-20 |
      | voor_het_eerst_verleend | true       |
    When I evaluate outputs "aantal_termijnen, eerste_termijn, maanden_ineens, resterende_termijnen" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then the execution succeeds
    Then output "aantal_termijnen" equals 9
    Then output "eerste_termijn" is true
    Then output "maanden_ineens" equals 3
    Then output "resterende_termijnen" equals 9

  Scenario: Een tweede voorschot in maart: geen deel in één bedrag
    Given the following parameters:
      | dagtekening_voorschot   | 2025-03-14 |
      | berekeningsjaar         | 2025       |
      | maand                   | 2025-04-20 |
      | voor_het_eerst_verleend | false      |
    When I evaluate outputs "maanden_ineens, termijn_in_maand, laatste_termijn, resterende_termijnen" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then the execution succeeds
    Then output "maanden_ineens" equals 0
    Then output "termijn_in_maand" is true
    Then output "laatste_termijn" is false
    Then output "resterende_termijnen" equals 8

  Scenario: Verleend op 31 oktober: nog twee termijnen, met een deel ineens
    Given the following parameters:
      | dagtekening_voorschot   | 2025-10-31 |
      | berekeningsjaar         | 2025       |
      | maand                   | 2025-10-31 |
      | voor_het_eerst_verleend | true       |
    When I evaluate outputs "aantal_termijnen, maanden_ineens, laatste_termijn" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then the execution succeeds
    Then output "aantal_termijnen" equals 2
    Then output "maanden_ineens" equals 10
    Then output "laatste_termijn" is false

  Scenario: Verleend na 31 oktober: het hele voorschot in één bedrag
    Given the following parameters:
      | dagtekening_voorschot   | 2025-11-03 |
      | berekeningsjaar         | 2025       |
      | maand                   | 2025-11-20 |
      | voor_het_eerst_verleend | true       |
    When I evaluate outputs "aantal_termijnen, eerste_termijn, laatste_termijn, maanden_ineens" of "algemene_wet_inkomensafhankelijke_regelingen"
    Then the execution succeeds
    Then output "aantal_termijnen" equals 1
    Then output "eerste_termijn" is true
    Then output "laatste_termijn" is true
    Then output "maanden_ineens" equals 12
