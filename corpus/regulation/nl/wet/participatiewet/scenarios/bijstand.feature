Feature: Bijstandsaanvraag via Participatiewet
  Als burger zonder voldoende middelen
  Wil ik bijstand kunnen aanvragen bij mijn gemeente
  Zodat ik in mijn levensonderhoud kan voorzien

  # Keten: Art 43 → Art 21 (normbedrag + leeftijdscheck) + Art 8 (verlaging via open_terms)
  #
  # Art. 21: Normbedragen - €1.091,71 (alleenstaand) / €1.559,58 (gehuwd)
  #          Leeftijdscheck: >= 21 en niet pensioengerechtigde leeftijd
  # Art. 8:  Delegatie - gemeente stelt verlaging vast via open_terms
  # Art. 43: Vaststelling - uitkering = normbedrag - (normbedrag × verlaging%)
  #
  # Art. 11 (Rechthebbenden) is not yet machine_readable — nationality/middelen
  # checks are not part of these scenarios. See #384.
  #
  # Afstemmingsverordening Diemen (GM0384):
  #   Categorie 1: 5%   - niet tijdig registreren UWV
  #   Categorie 2: 30%  - niet meewerken plan van aanpak
  #   Categorie 3: 100% - niet naar vermogen werk zoeken

  Background:
    Given the calculation date is "2024-06-01"
    Given law "afstemmingsverordening_participatiewet_diemen" is loaded

  # === Toekenningsscenario's voor burger uit Diemen (GM0384) ===

  Scenario: Alleenstaande burger krijgt volledige bijstand
    Given the following parameters:
      | bsn                                        | "123456789" |
      | gemeente_code                              | GM0384      |
      | leeftijd                                   | 35          |
      | is_alleenstaande                           | true        |
      | heeft_kostendelende_medebewoners           | false       |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false       |
      | gedragscategorie                           | 0           |
    When I evaluate "heeft_recht_op_bijstand" of "participatiewet"
    Then the execution succeeds
    Then output "heeft_recht_op_bijstand" is true
    When I evaluate "uitkering_bedrag" of "participatiewet"
    Then output "uitkering_bedrag" equals 109171

  Scenario: Gehuwde burger krijgt volledige bijstand
    Given the following parameters:
      | bsn                                        | "123456789" |
      | gemeente_code                              | GM0384      |
      | leeftijd                                   | 42          |
      | is_alleenstaande                           | false       |
      | heeft_kostendelende_medebewoners           | false       |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false       |
      | gedragscategorie                           | 0           |
    When I evaluate "heeft_recht_op_bijstand" of "participatiewet"
    Then the execution succeeds
    Then output "heeft_recht_op_bijstand" is true
    When I evaluate "uitkering_bedrag" of "participatiewet"
    Then output "uitkering_bedrag" equals 155958

  Scenario: Burger met gedragscategorie 1 krijgt 5% verlaging (Diemen)
    # Categorie 1: niet tijdig geregistreerd bij UWV
    Given the following parameters:
      | bsn                                        | "123456789" |
      | gemeente_code                              | GM0384      |
      | leeftijd                                   | 28          |
      | is_alleenstaande                           | true        |
      | heeft_kostendelende_medebewoners           | false       |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false       |
      | gedragscategorie                           | 1           |
    When I evaluate "heeft_recht_op_bijstand" of "participatiewet"
    Then the execution succeeds
    Then output "heeft_recht_op_bijstand" is true
    When I evaluate "uitkering_bedrag" of "participatiewet"
    # NB: a 5% verlaging yields a sub-cent amount; the model applies no whole-cent
    # rounding (that would be an explicit ROUND op, RFC-023/024), so the exact
    # value stands.
    Then output "uitkering_bedrag" equals 103712.45

  Scenario: Burger met gedragscategorie 2 krijgt 30% verlaging (Diemen)
    # Categorie 2: niet meewerken aan plan van aanpak
    Given the following parameters:
      | bsn                                        | "123456789" |
      | gemeente_code                              | GM0384      |
      | leeftijd                                   | 45          |
      | is_alleenstaande                           | true        |
      | heeft_kostendelende_medebewoners           | false       |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false       |
      | gedragscategorie                           | 2           |
    When I evaluate "heeft_recht_op_bijstand" of "participatiewet"
    Then the execution succeeds
    Then output "heeft_recht_op_bijstand" is true
    When I evaluate "uitkering_bedrag" of "participatiewet"
    # NB: a 30% verlaging yields a sub-cent amount; no whole-cent rounding in the
    # model (would be an explicit ROUND op, RFC-023/024), so the exact value stands.
    Then output "uitkering_bedrag" equals 76419.70

  Scenario: Burger met gedragscategorie 3 krijgt 100% verlaging (Diemen)
    # Categorie 3: niet naar vermogen arbeid verkrijgen
    Given the following parameters:
      | bsn                                        | "123456789" |
      | gemeente_code                              | GM0384      |
      | leeftijd                                   | 30          |
      | is_alleenstaande                           | true        |
      | heeft_kostendelende_medebewoners           | false       |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false       |
      | gedragscategorie                           | 3           |
    When I evaluate "heeft_recht_op_bijstand" of "participatiewet"
    Then the execution succeeds
    Then output "heeft_recht_op_bijstand" is true
    When I evaluate "uitkering_bedrag" of "participatiewet"
    Then output "uitkering_bedrag" equals 0

  # === Afwijzingsscenario's ===

  Scenario: Burger jonger dan 21 krijgt geen bijstand
    # Art. 21 checks leeftijd >= 21. Under-21 fails that check,
    # which propagates to Art. 43 via heeft_recht_op_bijstand = false.
    Given the following parameters:
      | bsn                                        | "123456789" |
      | gemeente_code                              | GM0384      |
      | leeftijd                                   | 19          |
      | is_alleenstaande                           | true        |
      | heeft_kostendelende_medebewoners           | false       |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false       |
      | gedragscategorie                           | 0           |
    When I evaluate "heeft_recht_op_bijstand" of "participatiewet"
    Then output "heeft_recht_op_bijstand" is false
    When I evaluate "uitkering_bedrag" of "participatiewet"
    Then output "uitkering_bedrag" equals 0

  # === Wettelijke grenzen (uit negative_scenarios) ===

  Scenario: Bijstand op exact leeftijd 21 — grens wordt geaccepteerd
    # Art 21 checks leeftijd >= 21. Exactly 21 passes.
    Given the following parameters:
      | bsn                                        | "123456789" |
      | gemeente_code                              | GM0384      |
      | leeftijd                                   | 21          |
      | is_alleenstaande                           | true        |
      | heeft_kostendelende_medebewoners           | false       |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false       |
      | gedragscategorie                           | 0           |
    When I evaluate "heeft_recht_op_bijstand" of "participatiewet"
    Then output "heeft_recht_op_bijstand" is true
    When I evaluate "uitkering_bedrag" of "participatiewet"
    Then output "uitkering_bedrag" equals 109171

  Scenario: Bijstand op leeftijd 20 — grens wordt afgewezen
    # Art 21 checks leeftijd >= 21. Age 20 fails this check,
    # propagating heeft_recht_op_bijstand = false through Art 43.
    Given the following parameters:
      | bsn                                        | "123456789" |
      | gemeente_code                              | GM0384      |
      | leeftijd                                   | 20          |
      | is_alleenstaande                           | true        |
      | heeft_kostendelende_medebewoners           | false       |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false       |
      | gedragscategorie                           | 0           |
    When I evaluate "heeft_recht_op_bijstand" of "participatiewet"
    Then output "heeft_recht_op_bijstand" is false
    When I evaluate "uitkering_bedrag" of "participatiewet"
    Then output "uitkering_bedrag" equals 0

  # === Gemeente zonder afstemmingsverordening: volledige bijstand ===

  Scenario: Burger uit gemeente zonder verordening krijgt volledige bijstand
    # Gemeente GM9999 heeft geen afstemmingsverordening
    # Art. 18 lid 2: "verlaagt ... overeenkomstig de verordening"
    # Geen verordening = geen verlaging = volledige bijstand
    Given the following parameters:
      | bsn                                        | "123456789" |
      | gemeente_code                              | GM9999      |
      | leeftijd                                   | 35          |
      | is_alleenstaande                           | true        |
      | heeft_kostendelende_medebewoners           | false       |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false       |
      | gedragscategorie                           | 1           |
    When I evaluate "heeft_recht_op_bijstand" of "participatiewet"
    Then the execution succeeds
    Then output "heeft_recht_op_bijstand" is true
    When I evaluate "uitkering_bedrag" of "participatiewet"
    Then output "uitkering_bedrag" equals 109171

  # NB: Without gemeente_code, Art 8 open_terms delegation resolves the optional
  # verlaging as null (per #403) → 0 reduction → full bijstand.
  Scenario: Bijstand zonder gemeente_code valt terug op volledige bijstand
    Given the following parameters:
      | bsn                                        | "123456789" |
      | leeftijd                                   | 35          |
      | is_alleenstaande                           | true        |
      | heeft_kostendelende_medebewoners           | false       |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false       |
      | gedragscategorie                           | 0           |
    When I evaluate "heeft_recht_op_bijstand" of "participatiewet"
    Then the execution succeeds
    Then output "heeft_recht_op_bijstand" is true
    When I evaluate "uitkering_bedrag" of "participatiewet"
    Then output "uitkering_bedrag" equals 109171

  # NB: Art 18 lid 2 — verlaging happens "overeenkomstig de verordening".
  # No verordening = no legal basis for reduction = full bijstand, even with
  # gedragscategorie 3 (100% reduction in Diemen).
  Scenario: Bijstand zonder verordening — verlaging valt weg
    Given the following parameters:
      | bsn                                        | "123456789" |
      | gemeente_code                              | GM9999      |
      | leeftijd                                   | 35          |
      | is_alleenstaande                           | true        |
      | heeft_kostendelende_medebewoners           | false       |
      | heeft_pensioengerechtigde_leeftijd_bereikt | false       |
      | gedragscategorie                           | 3           |
    When I evaluate "heeft_recht_op_bijstand" of "participatiewet"
    Then the execution succeeds
    Then output "heeft_recht_op_bijstand" is true
    When I evaluate "uitkering_bedrag" of "participatiewet"
    Then output "uitkering_bedrag" equals 109171
