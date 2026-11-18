Feature: Geen aanspraak op zorgtoeslag is geen bedrag
  Wie niet aan de voorwaarden voldoet heeft geen aanspraak op zorgtoeslag
  (art. 2 lid 1 en art. 3 lid 1 van de Wet op de zorgtoeslag). Dat is een
  afwezig bedrag, geen aanspraak op nihil.

  Background:
    Given the calculation date is "2025-02-01"
    And parameter "bsn" is "999993653"

  Scenario: Alleenstaande met vermogen boven de grens heeft geen aanspraak op zorgtoeslag
    # Spaargeld van EUR 200.000, boven de grens van EUR 141.896 voor een alleenstaande.
    Given the following "DJI" data with key "bsn" for law "penitentiaire_beginselenwet":
      | bsn         | status | inrichting_type |
      | "999993653" | null   | null            |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 2005-01-01    | GEEN              | null        | []                | []             | NEDERLAND     |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 79547                     | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 20000000  | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "RVZ" data with key "bsn" for law "zvw":
      | bsn         | polis_status | registratie |
      | "999993653" | ACTIEF       | null        |
    When I evaluate outputs "voldoet_aan_voorwaarden, hoogte_toeslag" of "zorgtoeslagwet"
    Then output "voldoet_aan_voorwaarden" is false
    And output "hoogte_toeslag" is absent
