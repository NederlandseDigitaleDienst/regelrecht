Feature: Handelsregisterwet - Jaarrekening Deponeringsplicht
  Als rechtspersoon
  Wil ik weten of ik een deponeringsplicht heb en wanneer mijn jaarrekening uiterlijk gedeponeerd moet zijn
  Zodat ik op tijd aan artikel 2:394 BW voldoe

  Background:
    Given the calculation date is "2024-06-01"

  Scenario: BV met deponeringsplicht - deadline is twaalf maanden na boekjaareinde
    # Art. 2:394 lid 3: uiterlijk twaalf maanden na afloop van het boekjaar.
    Given parameter "kvk_nummer" is "12345678"
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/jaarrekening":
      | kvk_nummer | rechtsvorm | laatste_boekjaar_einde |
      | "12345678" | BV         | 2023-12-31             |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_deponeringsplicht, volgende_deadline" of "handelsregisterwet/jaarrekening"
    Then output "voldoet_aan_voorwaarden" is true
    And output "heeft_deponeringsplicht" is true
    And output "volgende_deadline" equals "2024-12-31"

  Scenario: Eenmanszaak heeft geen deponeringsplicht
    # Art. 2:394 lid 1: de deponeringsplicht geldt voor de rechtspersonen genoemd bij
    # artikel 360, niet voor een eenmanszaak.
    Given parameter "kvk_nummer" is "23456789"
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/jaarrekening":
      | kvk_nummer | rechtsvorm  | laatste_boekjaar_einde |
      | "23456789" | Eenmanszaak | 2023-12-31             |
    When I evaluate outputs "voldoet_aan_voorwaarden, heeft_deponeringsplicht" of "handelsregisterwet/jaarrekening"
    Then output "voldoet_aan_voorwaarden" is false
    And output "heeft_deponeringsplicht" is false

  Scenario: Ministeriële ontheffing verleend - vrijstelling lid 5
    # Art. 2:394 lid 5: de voorgaande leden gelden niet bij een ontheffing van Onze
    # Minister van Economische Zaken op grond van artikel 58, 101 of 210.
    Given parameter "kvk_nummer" is "34567890"
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/jaarrekening":
      | kvk_nummer | rechtsvorm | laatste_boekjaar_einde | ontheffing_verleend |
      | "34567890" | NV         | 2023-12-31             | true                |
    When I evaluate outputs "reden_vrijstelling" of "handelsregisterwet/jaarrekening"
    Then output "reden_vrijstelling" equals "Lid 5: ministeriële ontheffing verleend (art. 58/101/210 BW2)"

  Scenario: Toezending aan AFM - vrijstelling lid 8
    # Art. 2:394 lid 8: een vennootschap met effecten op een gereglementeerde markt
    # wordt geacht aan lid 1 te hebben voldaan als zij de jaarrekening op grond van
    # artikel 5:25o, eerste lid, Wft heeft toegezonden aan de AFM. De lid-5-grond
    # (ontheffing) gaat in de IF voor en moet dus bekend zijn (false); zonder die
    # kolom is de eerste tak onbekend en daarmee de hele reden.
    Given parameter "kvk_nummer" is "45678901"
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/jaarrekening":
      | kvk_nummer | rechtsvorm | laatste_boekjaar_einde | ontheffing_verleend | afm_toezending_gedaan |
      | "45678901" | NV         | 2023-12-31             | false               | true                  |
    When I evaluate outputs "reden_vrijstelling" of "handelsregisterwet/jaarrekening"
    Then output "reden_vrijstelling" equals "Lid 8: toezending aan AFM op grond van art. 5:25o Wft"

  Scenario: Geen ontheffings- of AFM-register - reden vrijstelling onbekend
    # Art. 2:394 lid 5/8: de demo heeft geen ontheffingen- of AFM-toezendingsregister,
    # dus zonder dat gegeven is de reden onbekend, niet standaard "geen vrijstelling".
    Given parameter "kvk_nummer" is "56789012"
    And the following "KVK" data with key "kvk_nummer" for law "handelsregisterwet/jaarrekening":
      | kvk_nummer | rechtsvorm | laatste_boekjaar_einde |
      | "56789012" | BV         | 2023-12-31             |
    When I evaluate outputs "reden_vrijstelling" of "handelsregisterwet/jaarrekening"
    Then output "reden_vrijstelling" is unknown
