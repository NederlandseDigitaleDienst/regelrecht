Feature: AIO-aanvulling - Aanvullende inkomensvoorziening ouderen (art. 47a)
  Als AOW-gerechtigde met een laag inkomen
  Wil ik weten of ik recht heb op een AIO-aanvulling
  Zodat ik weet of mijn inkomen wordt aangevuld tot het sociaal minimum

  Background:
    Given the calculation date is "2026-03-01"

  Scenario: Alleenstaande met volledige AOW en geen ander inkomen heeft geen aanvulling nodig
    # Audit AUDIT_GETROUWHEID.md: sociaal_minimum citeerde art. 21 in plaats van art. 22
    # (de norm voor pensioengerechtigden), en totaal_inkomen telde de AOW-term op en
    # trok hem in dezelfde som weer af (algebraïsch gelijk aan inkomen/12, de AOW-term
    # deed niets). Met een volledige AOW-uitkering (138000, art. 22 leeftijdsbepaling
    # persona) boven de art. 22-norm voor alleenstaanden (121306) is er geen aanvulling.
    Given parameter "bsn" is "999993653"
    And the following "SVB" data with key "bsn" for law "algemene_ouderdomswet":
      | bsn         | woonperiodes |
      | "999993653" | 50           |
    And the following "RvIG" data with key "bsn" for law "wet_brp":
      | bsn         | geboortedatum | partnerschap_type | partner_bsn | kinderen_gegevens | ouder_adressen | land_verblijf | nationaliteit | adres | medebewoners | partner_geboortedatum |
      | "999993653" | 1958-02-15    | GEEN              | null        | []                | []             |               |               | null  | []           |                       |
    And the following "BELASTINGDIENST" data with key "bsn" for law "wet_inkomstenbelasting":
      | bsn         | loon_uit_dienstbetrekking | uitkeringen_en_pensioenen | winst_uit_onderneming | resultaat_overige_werkzaamheden | eigen_woning | reguliere_voordelen | vervreemdingsvoordelen | spaargeld | beleggingen | onroerend_goed | schulden | persoonsgebonden_aftrek | partner_loon_uit_dienstbetrekking | partner_uitkeringen_en_pensioenen | partner_winst_uit_onderneming | partner_resultaat_overige_werkzaamheden | partner_eigen_woning | partner_reguliere_voordelen | partner_vervreemdingsvoordelen | partner_spaargeld | partner_beleggingen | partner_onroerend_goed | partner_schulden | partner_buitenlands_inkomen | buitenlands_inkomen |
      | "999993653" | 0                         | 0                         | 0                     | 0                               | 0            | 0                   | 0                      | 0         | 0           | 0              | 0        | 0                       | 0                                 | 0                                 | 0                             | 0                                       | 0                    | 0                           | 0                              | 0                 | 0                   | 0                      | 0                | 0                           | 0                   |
    And the following "CBS" data with key "bsn" for law "wet_op_het_centraal_bureau_voor_de_statistiek":
      | bsn         | verwachting_65 |
      | "999993653" | 20.5           |
    And the following "UWV" data with key "bsn" for law "wet_structuur_uitvoeringsorganisatie_werk_en_inkomen":
      | bsn         | dienstverbandperiodes | uitkeringsperiodes |
      | "999993653" | []                    | []                 |
    When I evaluate outputs "voldoet_aan_voorwaarden, is_gerechtigd, sociaal_minimum, totaal_inkomen, aio_aanvulling" of "participatiewet/aio"
    Then output "voldoet_aan_voorwaarden" is true
    And output "is_gerechtigd" is true
    And output "sociaal_minimum" equals 121306
    And output "totaal_inkomen" equals 138000
    And output "aio_aanvulling" equals 0
