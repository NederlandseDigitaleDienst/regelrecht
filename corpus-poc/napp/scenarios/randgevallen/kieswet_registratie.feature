Feature: Registratie-eisen voor een aanduiding (Kieswet artikel G 1)
  Een aanduiding wordt geregistreerd voor een vereniging met volledige
  rechtsbevoegdheid die in het Handelsregister is ingeschreven en waarvan
  de statutaire naam overeenkomt met de aanduiding. De Napp gebruikt deze
  toets als advies bij claims in het partijregister.

  Scenario: Een ingeschreven vereniging met volledige rechtsbevoegdheid voldoet
    Given the calculation date is "2026-06-01"
    And the following facts:
      | ingeschreven_in_handelsregister | true                                        |
      | rechtsvorm                      | Vereniging met volledige rechtsbevoegdheid  |
      | naam_komt_overeen               | true                                        |
    When the registratie-eisen of Kieswet G 1 are evaluated
    Then the output "voldoet_aan_registratie_eisen" is true

  Scenario: Een stichting voldoet niet aan de rechtsvorm-eis
    Given the calculation date is "2026-06-01"
    And the following facts:
      | ingeschreven_in_handelsregister | true      |
      | rechtsvorm                      | Stichting |
      | naam_komt_overeen               | true      |
    When the registratie-eisen of Kieswet G 1 are evaluated
    Then the output "voldoet_eis_rechtsvorm" is false
    And the output "voldoet_aan_registratie_eisen" is false

  Scenario: Zonder inschrijving in het Handelsregister geen registratie
    Given the calculation date is "2026-06-01"
    And the following facts:
      | ingeschreven_in_handelsregister | false                                       |
      | rechtsvorm                      | Vereniging met volledige rechtsbevoegdheid  |
      | naam_komt_overeen               | true                                        |
    When the registratie-eisen of Kieswet G 1 are evaluated
    Then the output "voldoet_eis_inschrijving" is false
    And the output "voldoet_aan_registratie_eisen" is false

  Scenario: Een afwijkende statutaire naam voldoet niet
    Given the calculation date is "2026-06-01"
    And the following facts:
      | ingeschreven_in_handelsregister | true                                        |
      | rechtsvorm                      | Vereniging met volledige rechtsbevoegdheid  |
      | naam_komt_overeen               | false                                       |
    When the registratie-eisen of Kieswet G 1 are evaluated
    Then the output "voldoet_eis_naam" is false
    And the output "voldoet_aan_registratie_eisen" is false
