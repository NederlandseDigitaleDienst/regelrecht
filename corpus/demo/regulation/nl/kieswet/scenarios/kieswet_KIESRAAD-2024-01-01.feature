# Converted from overig/kieswet_KIESRAAD-2024-01-01.feature by corpus/demo/tools/convert_features.mjs
# Absence semantics applied by corpus/demo/tools/apply_absent_semantics.mjs (RFC-036)
Feature: Bepalen kiesrecht Tweede Kamer
  Als burger
  Wil ik weten of ik stemrecht heb voor de Tweede Kamerverkiezingen
  Zodat ik weet of ik mag stemmen

  Background:
    Given the calculation date is "2025-03-15"
    And parameter "bsn" is "999993653"

  Scenario: Persoon met Nederlandse nationaliteit van 18+ mag stemmen
    Given the following "KIESRAAD" data with key "bsn" for law "kieswet":
      | bsn         | verkiezingsdatum |
      | "999993653" | 2025-10-29       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 2006-01-01    | null              | null        | []                | []             | NLD           | NEDERLANDS    | null  | []           |                       |
    And the following "JUSTID" data with key "bsn" for law "wetboek_van_strafrecht":
      | bsn         | stemrecht_uitsluitingen |
      | "999993653" | []                      |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "kieswet"
    Then output "voldoet_aan_voorwaarden" is true

  Scenario: Persoon zonder Nederlandse nationaliteit mag niet stemmen
    Given the following "KIESRAAD" data with key "bsn" for law "kieswet":
      | bsn         | verkiezingsdatum |
      | "999993653" | 2025-10-29       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1990-01-01    | null              | null        | []                | []             | NLD           | DUITS         | null  | []           |                       |
    And the following "JUSTID" data with key "bsn" for law "wetboek_van_strafrecht":
      | bsn         | stemrecht_uitsluitingen |
      | "999993653" | []                      |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "kieswet"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Persoon onder 18 mag niet stemmen
    Given the following "KIESRAAD" data with key "bsn" for law "kieswet":
      | bsn         | verkiezingsdatum |
      | "999993653" | 2025-10-29       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 2008-01-01    | null              | null        | []                | []             | NLD           | NEDERLANDS    | null  | []           |                       |
    And the following "JUSTID" data with key "bsn" for law "wetboek_van_strafrecht":
      | bsn         | stemrecht_uitsluitingen |
      | "999993653" | []                      |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "kieswet"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Persoon met uitsluiting kiesrecht mag niet stemmen
    Given the following "KIESRAAD" data with key "bsn" for law "kieswet":
      | bsn         | verkiezingsdatum |
      | "999993653" | 2025-10-29       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1990-01-01    | null              | null        | []                | []             | NLD           | NEDERLANDS    | null  | []           |                       |
    And the following "JUSTID" data with key "bsn" for law "wetboek_van_strafrecht":
      | bsn         | stemrecht_uitsluitingen                                                                                     |
      | "999993653" | [{"startdatum":"2023-01-01","einddatum":"2024-01-01"},{"startdatum":"2024-06-01","einddatum":"2025-12-01"}] |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "kieswet"
    Then output "voldoet_aan_voorwaarden" is false

  Scenario: Gedetineerde persoon mag wel stemmen
    Given the following "KIESRAAD" data with key "bsn" for law "kieswet":
      | bsn         | verkiezingsdatum |
      | "999993653" | 2025-10-29       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status     | inrichting_type |
      | "999993653" | INGESLOTEN | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1990-01-01    | null              | null        | []                | []             | NLD           | NEDERLANDS    | null  | []           |                       |
    And the following "JUSTID" data with key "bsn" for law "wetboek_van_strafrecht":
      | bsn         | stemrecht_uitsluitingen                                |
      | "999993653" | [{"startdatum":"2023-01-01","einddatum":"2024-01-01"}] |
    When I evaluate outputs "voldoet_aan_voorwaarden" of "kieswet"
    Then output "voldoet_aan_voorwaarden" is true

  # Art. B1 lid 1: werkelijke woonplaats in Aruba, Curacao of Sint Maarten sluit uit, en
  # lid 2 geeft geen van beide uitzonderingen op de uitzondering (geen tien jaar
  # ingezetenschap, geen Nederlandse openbare dienst): geen stemrecht.
  Scenario: Persoon woonachtig in Aruba zonder tien jaar ingezetenschap of openbare dienst mag niet stemmen
    Given the following "KIESRAAD" data with key "bsn" for law "kieswet":
      | bsn         | verkiezingsdatum |
      | "999993653" | 2025-10-29       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1990-01-01    | null              | null        | []                | []             | ARUBA         | NEDERLANDS    | null  | []           |                       |
    And the following "JUSTID" data with key "bsn" for law "wetboek_van_strafrecht":
      | bsn         | stemrecht_uitsluitingen |
      | "999993653" | []                      |
    Given the following parameters:
      | ingezetenschapsduur_jaren               | 3     |
      | werkzaam_in_nederlandse_openbare_dienst | false |
    When I evaluate outputs "heeft_stemrecht" of "kieswet"
    Then output "heeft_stemrecht" is false

  # Art. B1 lid 2 onder a: tien jaar ingezetenschap van Nederland heft de lid-1-uitzondering
  # op, ook al woont de persoon nu in Curacao: wel stemrecht.
  Scenario: Persoon woonachtig in Curacao met tien jaar ingezetenschap mag wel stemmen
    Given the following "KIESRAAD" data with key "bsn" for law "kieswet":
      | bsn         | verkiezingsdatum |
      | "999993653" | 2025-10-29       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1980-01-01    | null              | null        | []                | []             | CURACAO       | NEDERLANDS    | null  | []           |                       |
    And the following "JUSTID" data with key "bsn" for law "wetboek_van_strafrecht":
      | bsn         | stemrecht_uitsluitingen |
      | "999993653" | []                      |
    Given the following parameters:
      | ingezetenschapsduur_jaren               | 12    |
      | werkzaam_in_nederlandse_openbare_dienst | false |
    When I evaluate outputs "heeft_stemrecht" of "kieswet"
    Then output "heeft_stemrecht" is true

  # Art. B1 lid 2 onder b: werkzaam in Nederlandse openbare dienst in Sint Maarten heft de
  # lid-1-uitzondering op, ook zonder tien jaar ingezetenschap: wel stemrecht.
  Scenario: Persoon werkzaam in Nederlandse openbare dienst op Sint Maarten mag wel stemmen
    Given the following "KIESRAAD" data with key "bsn" for law "kieswet":
      | bsn         | verkiezingsdatum |
      | "999993653" | 2025-10-29       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1990-01-01    | null              | null        | []                | []             | SINT_MAARTEN  | NEDERLANDS    | null  | []           |                       |
    And the following "JUSTID" data with key "bsn" for law "wetboek_van_strafrecht":
      | bsn         | stemrecht_uitsluitingen |
      | "999993653" | []                      |
    Given the following parameters:
      | ingezetenschapsduur_jaren               | 2    |
      | werkzaam_in_nederlandse_openbare_dienst | true |
    When I evaluate outputs "heeft_stemrecht" of "kieswet"
    Then output "heeft_stemrecht" is true

  # Art. B1 lid 1: wie in Nederland woont valt niet onder de uitzondering; die vraag speelt
  # niet, dus stemrecht zoals in de gewone gevallen hierboven.
  Scenario: Persoon woonachtig in Nederland mag wel stemmen, ongeacht ingezetenschapsduur
    Given the following "KIESRAAD" data with key "bsn" for law "kieswet":
      | bsn         | verkiezingsdatum |
      | "999993653" | 2025-10-29       |
    And the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1990-01-01    | null              | null        | []                | []             | NLD           | NEDERLANDS    | null  | []           |                       |
    And the following "JUSTID" data with key "bsn" for law "wetboek_van_strafrecht":
      | bsn         | stemrecht_uitsluitingen |
      | "999993653" | []                      |
    When I evaluate outputs "heeft_stemrecht" of "kieswet"
    Then output "heeft_stemrecht" is true
