Feature: Verstrekking aan de rechtspersoon en uitbetaling (Wpp artikel 27)
  De subsidie wordt uitbetaald op een rekening op naam van de
  rechtspersoon. Alleen het tekenbevoegd bestuur (in deze uitvoering: een
  volledige eHerkenning-machtiging) mag de rekening opgeven of wijzigen.
  Zonder bekende rekening wordt de uitbetaling aangehouden.

  Scenario: Het tekenbevoegd bestuur geeft een rekening op naam op
    Given the calculation date is "2026-06-01"
    And the following facts:
      | rekening_op_naam_van_rechtspersoon | true |
      | eherkenning_volledige_machtiging   | true |
      | rekening_bekend                    | true |
    When the rekening-regels of artikel 27 are evaluated
    Then the output "mag_rekening_wijzigen" is true
    And the output "rekening_aanvaardbaar" is true
    And the output "uitbetaling_aangehouden" is false

  Scenario: Een afdelingsvolmacht mag de rekening niet wijzigen
    Given the calculation date is "2026-06-01"
    And the following facts:
      | rekening_op_naam_van_rechtspersoon | true  |
      | eherkenning_volledige_machtiging   | false |
      | rekening_bekend                    | true  |
    When the rekening-regels of artikel 27 are evaluated
    Then the output "mag_rekening_wijzigen" is false

  Scenario: Een rekening op andermans naam is niet aanvaardbaar
    Given the calculation date is "2026-06-01"
    And the following facts:
      | rekening_op_naam_van_rechtspersoon | false |
      | eherkenning_volledige_machtiging   | true  |
      | rekening_bekend                    | true  |
    When the rekening-regels of artikel 27 are evaluated
    Then the output "rekening_aanvaardbaar" is false

  Scenario: Zonder bekende rekening wordt de uitbetaling aangehouden
    Given the calculation date is "2026-06-01"
    And the following facts:
      | rekening_op_naam_van_rechtspersoon | false |
      | eherkenning_volledige_machtiging   | false |
      | rekening_bekend                    | false |
    When the rekening-regels of artikel 27 are evaluated
    Then the output "uitbetaling_aangehouden" is true
