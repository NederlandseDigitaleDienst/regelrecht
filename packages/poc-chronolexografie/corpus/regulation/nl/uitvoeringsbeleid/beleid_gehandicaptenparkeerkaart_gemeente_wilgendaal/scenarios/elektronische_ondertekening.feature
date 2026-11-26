Feature: Wanneer een elektronische aanvraag als ondertekend geldt
  Fictief beleid van de gemeente Wilgendaal, artikel 8: een aanvraag die de
  aanvrager elektronisch indient nadat hij zich met DigiD aanmeldde, geldt als
  ondertekend. Het systeem waarmee hij indient, voert dit artikel uit met het
  middel waarmee hij zich aanmeldde en zet de uitkomst in de aanvraag.

  Background:
    Given the calculation date is "2026-10-13"

  Scenario: Na een aanmelding met DigiD geldt de aanvraag als ondertekend
    Given the following parameters:
      | inlogmiddel | DigiD |
    When I evaluate "aanvraag_geldt_als_ondertekend" of "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "aanvraag_geldt_als_ondertekend" is true

  Scenario: Na een aanmelding met een ander middel geldt zij niet als ondertekend
    Given the following parameters:
      | inlogmiddel | FSC voor Burgers |
    When I evaluate "aanvraag_geldt_als_ondertekend" of "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "aanvraag_geldt_als_ondertekend" is false

  Scenario: Zonder aanmelding geldt zij niet als ondertekend
    Given the following parameters:
      | inlogmiddel | null |
    When I evaluate "aanvraag_geldt_als_ondertekend" of "beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "aanvraag_geldt_als_ondertekend" is false
