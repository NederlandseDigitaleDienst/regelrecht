Feature: Wat het college met een verstrekte kaart nog te doen heeft
  Fictief beleid van de gemeente Wilgendaal. Het college vraagt elke kaart
  die het verstrekte aan bij de drukker (artikel 1), de drukker drukt en
  levert haar (artikel 2), en het college reikt haar uit (artikel 3). Elk
  artikel zegt hoeveel er daarvan nog te doen is. In een cel telt elke cel
  dat uit haar eigen kroniek (artikel 4 tot en met 6); hier, zonder cel,
  geeft het scenario de eigen telling als bevinding.

  Background:
    Given the calculation date is "2026-10-28"

  Scenario: Een verstrekte kaart die nog niet is aangevraagd
    Given the following findings:
      | aantal_verstrekte_gehandicaptenparkeerkaarten_volgens_eigen_telling | 1 |
      | aantal_drukopdrachten_gegeven_volgens_eigen_telling                 | 0 |
    When I evaluate "aantal_aan_te_vragen_kaarten" of "beleid_uitreiking_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "aantal_aan_te_vragen_kaarten" equals 1

  Scenario: Een herroepen verstrekking laat het aantal niet onder nul zakken
    # De drukopdracht blijft staan als het college de kaart na bezwaar
    # herroept; er is dan niets aan te vragen, niet min één.
    Given the following findings:
      | aantal_verstrekte_gehandicaptenparkeerkaarten_volgens_eigen_telling | 0 |
      | aantal_drukopdrachten_gegeven_volgens_eigen_telling                 | 1 |
    When I evaluate "aantal_aan_te_vragen_kaarten" of "beleid_uitreiking_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "aantal_aan_te_vragen_kaarten" equals 0

  Scenario: De drukker heeft een ontvangen drukopdracht nog te leveren
    Given the following findings:
      | aantal_drukopdrachten_ontvangen_volgens_eigen_telling | 1 |
      | aantal_kaarten_geleverd_volgens_eigen_telling         | 0 |
    When I evaluate "aantal_te_leveren_kaarten" of "beleid_uitreiking_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "aantal_te_leveren_kaarten" equals 1
    When I evaluate "aantal_gedrukte_kaarten" of "beleid_uitreiking_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "aantal_gedrukte_kaarten" equals 0

  Scenario: Een uitgereikte kaart telt mee, en er is niets meer uit te reiken
    Given the following findings:
      | aantal_kaarten_ontvangen_van_drukker_volgens_eigen_telling | 1 |
      | aantal_kaarten_uitgereikt_volgens_eigen_telling            | 1 |
    When I evaluate "aantal_uit_te_reiken_kaarten" of "beleid_uitreiking_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "aantal_uit_te_reiken_kaarten" equals 0
    When I evaluate "aantal_uitgereikte_kaarten" of "beleid_uitreiking_gehandicaptenparkeerkaart_gemeente_wilgendaal"
    Then the execution succeeds
    Then output "aantal_uitgereikte_kaarten" equals 1
