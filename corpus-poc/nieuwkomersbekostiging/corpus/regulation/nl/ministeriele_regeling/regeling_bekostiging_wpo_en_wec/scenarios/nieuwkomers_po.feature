# Scenario's nieuwkomersbekostiging primair onderwijs (artikel 34 tot en met 36
# Definitieve regeling bekostiging WPO en WEC).
#
# Referentie: de zes casussen uit de OCW/DUO-visual over de eerste opvang.
# Alle datums uit die visual zijn hier één jaar opgeschoven (2024 -> 2025): het
# corpus kent de regelingen 2025 en 2026, en de engine weigert een peildatum
# waarop nog geen versie geldt (2024). De mechaniek is jaar-onafhankelijk; de
# bedragen volgen de versie die op de peildatum geldt (2025 resp. 2026).
#
# Interpretatiekeuzes die de verwachte uitkomsten bepalen:
#   (a) De aftrek voor de periode tussen datum vestiging en de vierde verjaardag
#       (artikel 34, vierde lid) gaat af van het einde van het totale recht: de
#       eerste vier kwartalen zijn artikel 34 (jaar 1), de rest artikel 35
#       (jaar 2). Casus C3 krijgt zo "een jaar en twee kwartalen", conform de
#       visual. Naar de letter zou de aftrek ook van de eerste twaalf maanden
#       kunnen gaan (dan jaar 1 = 2 kwartalen, jaar 2 = 4); open vraag.
#   (b) DUO rondt de aftrek naar beneden af op hele kwartalen
#       (DUO-werkinstructie): 7 maanden -> 2 kwartalen, 5 maanden -> 1.
#   (c) Verstreken kwartalen = hele maanden tussen eerste inschrijving en de
#       peildatum, gedeeld door drie en naar beneden afgerond. Telt op de
#       peildatum: eerste inschrijving <= peildatum en verstreken < bekostigbaar.
#   (d) Categorie op basis van de verblijfstitelcode (tiende lid). Bij code 21,
#       33, 34, 98, bij inschrijving op onderwijsnummer of bij een onbekende code
#       beslist het bevoegd gezag; zonder oordeel telt de leerling niet mee.
#   (e) Leerlingen op een speciale school voor basisonderwijs vallen onder
#       artikel 36: één tarief, vier kwartalen, dezelfde drempel van vier.
#   (f) Per leerling wordt 25% van het jaarbedrag afgerond op hele eurocenten;
#       op schoolniveau wordt de formule van het negende lid in één keer
#       berekend en daarna afgerond. Die twee kunnen een cent verschillen.
#   (g) Artikel 34, twaalfde lid (in Nederland geboren met een van de ouders
#       afgeleide status): "geboren in Nederland" is afgeleid als datum van
#       vestiging in Nederland <= geboortedatum, want voor een hier geboren kind
#       is de BRP-datum van vestiging de geboortedatum. Categorie GEEN.
#
# De persona-tabellen bevatten alleen de velden die de regeling leest;
# sector, school_id en in_telling_1_februari staan in data/personas.yaml.
Feature: Nieuwkomersbekostiging primair onderwijs

  Background:
    Given the calculation date is "2025-07-01"
    Given law "regeling_bekostiging_wpo_en_wec" is loaded

  # ------------------------------------------------------------- casus C1
  Scenario: C1 asielzoeker, gevestigd na de vierde verjaardag, acht kwartalen
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | verblijfstitel_code | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | schoolsoort | internationaal_georienteerd | oordeel_bevoegd_gezag |
      | "200000001" | 2017-03-10    | true           | 26                  | true      | 2025-02-01                | 2025-05-01            | true                    | true                  | basisschool | false                       | null                  |
    Given parameter "bsn" is "200000001"
    When I evaluate "categorie_nieuwkomer" of "regeling_bekostiging_wpo_en_wec"
    Then output "categorie_nieuwkomer" equals "ASIELZOEKER"
    When I evaluate "aftrek_maanden_voor_vierde_verjaardag" of "regeling_bekostiging_wpo_en_wec"
    Then output "aftrek_maanden_voor_vierde_verjaardag" equals 0
    When I evaluate "aftrek_kwartalen" of "regeling_bekostiging_wpo_en_wec"
    Then output "aftrek_kwartalen" equals 0
    When I evaluate "bekostigbare_kwartalen" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigbare_kwartalen" equals 8
    # Eerste peildatum na inschrijving: jaar 1, tarief 2025.
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is true
    When I evaluate "kwartalen_verstreken" of "regeling_bekostiging_wpo_en_wec"
    Then output "kwartalen_verstreken" equals 0
    When I evaluate "bekostigingsjaar" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigingsjaar" equals 1
    When I evaluate "bedrag_kwartaal" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_kwartaal" equals 361722
    When I evaluate "aanvraag_vereist" of "regeling_bekostiging_wpo_en_wec"
    Then output "aanvraag_vereist" is true
    # Achtste kwartaal: jaar 2 (artikel 35), tarief 2026.
    Given the calculation date is "2027-04-01"
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is true
    When I evaluate "kwartalen_verstreken" of "regeling_bekostiging_wpo_en_wec"
    Then output "kwartalen_verstreken" equals 7
    When I evaluate "bekostigingsjaar" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigingsjaar" equals 2
    When I evaluate "bedrag_kwartaal" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_kwartaal" equals 57027
    # Negende peildatum: het recht is op.
    Given the calculation date is "2027-07-01"
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is false
    When I evaluate "bekostigingsjaar" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigingsjaar" equals 0
    When I evaluate "bedrag_kwartaal" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_kwartaal" equals 0
    # Peildatum vóór de eerste inschrijving.
    Given the calculation date is "2025-04-01"
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is false
    When I evaluate "kwartalen_verstreken" of "regeling_bekostiging_wpo_en_wec"
    Then output "kwartalen_verstreken" equals 0

  # ------------------------------------------------------------- casus C2
  Scenario: C2 overige vreemdeling, vier kwartalen
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | verblijfstitel_code | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | schoolsoort | internationaal_georienteerd | oordeel_bevoegd_gezag |
      | "200000002" | 2014-06-20    | true           | 22                  | true      | 2025-11-15                | 2026-02-15            | true                    | true                  | basisschool | false                       | null                  |
    Given parameter "bsn" is "200000002"
    Given the calculation date is "2026-04-01"
    When I evaluate "categorie_nieuwkomer" of "regeling_bekostiging_wpo_en_wec"
    Then output "categorie_nieuwkomer" equals "OVERIGE_VREEMDELING"
    When I evaluate "bekostigbare_kwartalen" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigbare_kwartalen" equals 4
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is true
    When I evaluate "bekostigingsjaar" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigingsjaar" equals 1
    When I evaluate "bedrag_kwartaal" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_kwartaal" equals 117809
    Given the calculation date is "2027-01-01"
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is true
    When I evaluate "kwartalen_verstreken" of "regeling_bekostiging_wpo_en_wec"
    Then output "kwartalen_verstreken" equals 3
    Given the calculation date is "2027-04-01"
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is false

  # ------------------------------------------------------------- casus C3
  Scenario: C3 asielzoeker gevestigd zeven maanden voor de vierde verjaardag, zes kwartalen
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | verblijfstitel_code | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | schoolsoort | internationaal_georienteerd | oordeel_bevoegd_gezag |
      | "200000003" | 2021-09-01    | true           | 26                  | true      | 2025-02-01                | 2025-09-01            | true                    | true                  | basisschool | false                       | null                  |
    Given parameter "bsn" is "200000003"
    Given the calculation date is "2025-10-01"
    When I evaluate "aftrek_maanden_voor_vierde_verjaardag" of "regeling_bekostiging_wpo_en_wec"
    Then output "aftrek_maanden_voor_vierde_verjaardag" equals 7
    When I evaluate "aftrek_kwartalen" of "regeling_bekostiging_wpo_en_wec"
    Then output "aftrek_kwartalen" equals 2
    When I evaluate "bekostigbare_kwartalen" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigbare_kwartalen" equals 6
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is true
    When I evaluate "bekostigingsjaar" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigingsjaar" equals 1
    # Zesde kwartaal: de aftrek gaat van het einde af, dus dit is jaar 2.
    Given the calculation date is "2027-01-01"
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is true
    When I evaluate "kwartalen_verstreken" of "regeling_bekostiging_wpo_en_wec"
    Then output "kwartalen_verstreken" equals 5
    When I evaluate "bekostigingsjaar" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigingsjaar" equals 2
    When I evaluate "bedrag_kwartaal" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_kwartaal" equals 57027
    Given the calculation date is "2027-04-01"
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is false

  # ------------------------------------------------------------- casus C4
  Scenario: C4 overige vreemdeling met een kwartaal aftrek, inschrijving op de peildatum
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | verblijfstitel_code | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | schoolsoort | internationaal_georienteerd | oordeel_bevoegd_gezag |
      | "200000004" | 2021-07-01    | true           | 22                  | true      | 2025-02-01                | 2025-07-01            | true                    | true                  | basisschool | false                       | null                  |
    Given parameter "bsn" is "200000004"
    When I evaluate "aftrek_maanden_voor_vierde_verjaardag" of "regeling_bekostiging_wpo_en_wec"
    Then output "aftrek_maanden_voor_vierde_verjaardag" equals 5
    When I evaluate "aftrek_kwartalen" of "regeling_bekostiging_wpo_en_wec"
    Then output "aftrek_kwartalen" equals 1
    When I evaluate "bekostigbare_kwartalen" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigbare_kwartalen" equals 3
    # Inschrijvingsdag is de peildatum: telt mee.
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is true
    When I evaluate "bedrag_kwartaal" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_kwartaal" equals 112378
    Given the calculation date is "2026-01-01"
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is true
    Given the calculation date is "2026-04-01"
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is false

  # ------------------------------------------------------------- casus C5
  Scenario: C5 asielzoeker met twee kwartalen aftrek, zes kwartalen
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | verblijfstitel_code | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | schoolsoort | internationaal_georienteerd | oordeel_bevoegd_gezag |
      | "200000005" | 2021-08-15    | true           | 26                  | true      | 2025-02-01                | 2025-11-01            | true                    | true                  | basisschool | false                       | null                  |
    Given parameter "bsn" is "200000005"
    Given the calculation date is "2026-01-01"
    When I evaluate "aftrek_maanden_voor_vierde_verjaardag" of "regeling_bekostiging_wpo_en_wec"
    Then output "aftrek_maanden_voor_vierde_verjaardag" equals 6
    When I evaluate "aftrek_kwartalen" of "regeling_bekostiging_wpo_en_wec"
    Then output "aftrek_kwartalen" equals 2
    When I evaluate "bekostigbare_kwartalen" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigbare_kwartalen" equals 6
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is true
    When I evaluate "bedrag_kwartaal" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_kwartaal" equals 379204
    Given the calculation date is "2027-04-01"
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is true
    When I evaluate "bekostigingsjaar" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigingsjaar" equals 2
    Given the calculation date is "2027-07-01"
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is false

  # ------------------------------------------------------------- casus C6
  Scenario: C6 overige vreemdeling met twee kwartalen aftrek, twee kwartalen
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | verblijfstitel_code | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | schoolsoort | internationaal_georienteerd | oordeel_bevoegd_gezag |
      | "200000006" | 2021-10-01    | true           | 22                  | true      | 2025-02-01                | 2025-11-01            | true                    | true                  | basisschool | false                       | null                  |
    Given parameter "bsn" is "200000006"
    Given the calculation date is "2026-01-01"
    When I evaluate "aftrek_maanden_voor_vierde_verjaardag" of "regeling_bekostiging_wpo_en_wec"
    Then output "aftrek_maanden_voor_vierde_verjaardag" equals 8
    When I evaluate "aftrek_kwartalen" of "regeling_bekostiging_wpo_en_wec"
    Then output "aftrek_kwartalen" equals 2
    When I evaluate "bekostigbare_kwartalen" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigbare_kwartalen" equals 2
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is true
    Given the calculation date is "2026-04-01"
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is true
    Given the calculation date is "2026-07-01"
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is false

  # ------------------------------------------------------------- categorie
  Scenario: Nederlandse leerling valt buiten de regeling
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | verblijfstitel_code | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | schoolsoort | internationaal_georienteerd | oordeel_bevoegd_gezag |
      | "200000010" | 2017-03-10    | false          | null                | true      | 2017-03-10                | 2021-03-10            | true                    | true                  | basisschool | false                       | null                  |
    Given parameter "bsn" is "200000010"
    When I evaluate "categorie_nieuwkomer" of "regeling_bekostiging_wpo_en_wec"
    Then output "categorie_nieuwkomer" equals "GEEN"
    When I evaluate "bekostigbare_kwartalen" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigbare_kwartalen" equals 0
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is false
    When I evaluate "bedrag_kwartaal" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_kwartaal" equals 0

  Scenario: Code 21 zonder oordeel van het bevoegd gezag telt nog niet mee
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | verblijfstitel_code | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | schoolsoort | internationaal_georienteerd | oordeel_bevoegd_gezag |
      | "200000011" | 2017-03-10    | true           | 21                  | true      | 2025-02-01                | 2025-05-01            | true                    | true                  | basisschool | false                       | null                  |
    Given parameter "bsn" is "200000011"
    When I evaluate "categorie_nieuwkomer" of "regeling_bekostiging_wpo_en_wec"
    Then output "categorie_nieuwkomer" equals "BESTUUR_BEOORDEELT"
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is false
    When I evaluate "bekostigingsjaar" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigingsjaar" equals 0

  Scenario: Code 21 met oordeel asielzoeker volgt het oordeel
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | verblijfstitel_code | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | schoolsoort | internationaal_georienteerd | oordeel_bevoegd_gezag |
      | "200000012" | 2017-03-10    | true           | 21                  | true      | 2025-02-01                | 2025-05-01            | true                    | true                  | basisschool | false                       | ASIELZOEKER           |
    Given parameter "bsn" is "200000012"
    When I evaluate "categorie_nieuwkomer" of "regeling_bekostiging_wpo_en_wec"
    Then output "categorie_nieuwkomer" equals "ASIELZOEKER"
    When I evaluate "bekostigbare_kwartalen" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigbare_kwartalen" equals 8
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is true

  Scenario: Inschrijving op onderwijsnummer met oordeel overige vreemdeling
    Given the following "personas" data with key "bsn":
      | bsn       | geboortedatum | is_vreemdeling | verblijfstitel_code | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | schoolsoort | internationaal_georienteerd | oordeel_bevoegd_gezag |
      | ON0000013 | 2017-03-10    | true           | null                | false     | 2025-02-01                | 2025-05-01            | true                    | true                  | basisschool | false                       | OVERIGE_VREEMDELING   |
    Given parameter "bsn" is "ON0000013"
    When I evaluate "categorie_nieuwkomer" of "regeling_bekostiging_wpo_en_wec"
    Then output "categorie_nieuwkomer" equals "OVERIGE_VREEMDELING"
    When I evaluate "bekostigbare_kwartalen" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigbare_kwartalen" equals 4

  Scenario: Internationaal georiënteerd basisonderwijs is uitgesloten
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | verblijfstitel_code | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | schoolsoort | internationaal_georienteerd | oordeel_bevoegd_gezag |
      | "200000014" | 2017-03-10    | true           | 28                  | true      | 2025-02-01                | 2025-05-01            | true                    | true                  | basisschool | true                        | null                  |
    Given parameter "bsn" is "200000014"
    When I evaluate "categorie_nieuwkomer" of "regeling_bekostiging_wpo_en_wec"
    Then output "categorie_nieuwkomer" equals "GEEN"

  Scenario: Een code buiten de tabel van het tiende lid geeft geen categorie
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | verblijfstitel_code | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | schoolsoort | internationaal_georienteerd | oordeel_bevoegd_gezag |
      | "200000016" | 2017-03-10    | true           | 47                  | true      | 2025-02-01                | 2025-05-01            | true                    | true                  | basisschool | false                       | null                  |
    Given parameter "bsn" is "200000016"
    When I evaluate "categorie_nieuwkomer" of "regeling_bekostiging_wpo_en_wec"
    Then output "categorie_nieuwkomer" equals "GEEN"

  Scenario: Een leerling in het voortgezet onderwijs valt buiten deze regeling
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | verblijfstitel_code | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | schoolsoort | internationaal_georienteerd | oordeel_bevoegd_gezag |
      | "200000017" | 2011-03-10    | true           | 26                  | true      | 2025-02-01                | 2025-05-01            | true                    | true                  | vo          | false                       | null                  |
    Given parameter "bsn" is "200000017"
    When I evaluate "categorie_nieuwkomer" of "regeling_bekostiging_wpo_en_wec"
    Then output "categorie_nieuwkomer" equals "GEEN"
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is false

  Scenario: In Nederland geboren leerling met afgeleide status is uitgesloten (twaalfde lid)
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | verblijfstitel_code | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | schoolsoort | internationaal_georienteerd | oordeel_bevoegd_gezag |
      | "200000018" | 2022-04-01    | true           | 26                  | true      | 2022-04-01                | 2026-04-01            | true                    | true                  | basisschool | false                       | null                  |
    Given parameter "bsn" is "200000018"
    Given the calculation date is "2026-07-01"
    When I evaluate "geboren_in_nederland" of "regeling_bekostiging_wpo_en_wec"
    Then output "geboren_in_nederland" is true
    When I evaluate "categorie_nieuwkomer" of "regeling_bekostiging_wpo_en_wec"
    Then output "categorie_nieuwkomer" equals "GEEN"
    When I evaluate "bekostigbare_kwartalen" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigbare_kwartalen" equals 0
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is false

  Scenario: Speciale school voor basisonderwijs valt onder artikel 36
    Given the following "personas" data with key "bsn":
      | bsn         | geboortedatum | is_vreemdeling | verblijfstitel_code | heeft_bsn | datum_vestiging_nederland | eerste_inschrijfdatum | woonachtig_in_nederland | werkelijk_schoolgaand | schoolsoort | internationaal_georienteerd | oordeel_bevoegd_gezag |
      | "200000015" | 2015-03-10    | true           | 26                  | true      | 2025-02-01                | 2025-05-01            | true                    | true                  | sbo         | false                       | null                  |
    Given parameter "bsn" is "200000015"
    Given the calculation date is "2026-04-01"
    When I evaluate "valt_onder_artikel_36" of "regeling_bekostiging_wpo_en_wec"
    Then output "valt_onder_artikel_36" is true
    When I evaluate "categorie_nieuwkomer" of "regeling_bekostiging_wpo_en_wec"
    Then output "categorie_nieuwkomer" equals "ASIELZOEKER"
    # Eén tarief en maximaal vier kwartalen, ook voor een asielzoeker.
    When I evaluate "bekostigbare_kwartalen" of "regeling_bekostiging_wpo_en_wec"
    Then output "bekostigbare_kwartalen" equals 4
    When I evaluate "telt_op_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "telt_op_peildatum" is true
    When I evaluate "bedrag_kwartaal" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_kwartaal" equals 117809

  # ------------------------------------------------------------- schoolniveau
  Scenario: School onder de drempel van vier krijgt niets
    Given the following "scholen" data with key "school_id":
      | school_id | schoolsoort | aantal_asielzoekers_peildatum | aantal_overige_vreemdelingen_peildatum | aantal_asielzoekers_telling_1_februari | aantal_tweedejaars_asielzoekers_peildatum | eerste_keer_eerste_opvang |
      | S1        | basisschool | 3                             | 0                                      | 0                                      | 0                                         | true                      |
    Given parameter "school_id" is "S1"
    Given the calculation date is "2026-04-01"
    When I evaluate "voldoet_aan_drempel" of "regeling_bekostiging_wpo_en_wec"
    Then output "voldoet_aan_drempel" is false
    When I evaluate "bedrag_eerste_opvang_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_eerste_opvang_peildatum" equals 0
    When I evaluate "toeslag_eerste_keer" of "regeling_bekostiging_wpo_en_wec"
    Then output "toeslag_eerste_keer" equals 0

  Scenario: Reguliere peildatum, formule negende lid, tarieven 2026
    Given the following "scholen" data with key "school_id":
      | school_id | schoolsoort | aantal_asielzoekers_peildatum | aantal_overige_vreemdelingen_peildatum | aantal_asielzoekers_telling_1_februari | aantal_tweedejaars_asielzoekers_peildatum | eerste_keer_eerste_opvang |
      | S2        | basisschool | 2                             | 2                                      | 0                                      | 3                                         | true                      |
    Given parameter "school_id" is "S2"
    Given the calculation date is "2026-04-01"
    When I evaluate "voldoet_aan_drempel" of "regeling_bekostiging_wpo_en_wec"
    Then output "voldoet_aan_drempel" is true
    # 2 x 15.168,16 x 25% + 2 x 4.712,34 x 25% = 7.584,08 + 2.356,17
    When I evaluate "bedrag_eerste_opvang_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_eerste_opvang_peildatum" equals 994025
    When I evaluate "toeslag_eerste_keer" of "regeling_bekostiging_wpo_en_wec"
    Then output "toeslag_eerste_keer" equals 1817457
    # 3 x 2.281,08 x 25%
    When I evaluate "bedrag_tweede_jaar_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_tweede_jaar_peildatum" equals 171081

  Scenario: Reguliere peildatum met tarieven 2025
    Given the following "scholen" data with key "school_id":
      | school_id | schoolsoort | aantal_asielzoekers_peildatum | aantal_overige_vreemdelingen_peildatum | aantal_asielzoekers_telling_1_februari | aantal_tweedejaars_asielzoekers_peildatum | eerste_keer_eerste_opvang |
      | S2        | basisschool | 2                             | 2                                      | 0                                      | 3                                         | false                     |
    Given parameter "school_id" is "S2"
    Given the calculation date is "2025-04-01"
    # 2 x 14.468,89 x 25% + 2 x 4.495,10 x 25% = 7.234,445 + 2.247,55, afgerond
    When I evaluate "bedrag_eerste_opvang_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_eerste_opvang_peildatum" equals 948200
    When I evaluate "toeslag_eerste_keer" of "regeling_bekostiging_wpo_en_wec"
    Then output "toeslag_eerste_keer" equals 0
    # 3 x 2.175,92 x 25%
    When I evaluate "bedrag_tweede_jaar_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_tweede_jaar_peildatum" equals 163194

  Scenario: Peildatum 1 januari met meer asielzoekers dan in de 1-februari-telling
    Given the following "scholen" data with key "school_id":
      | school_id | schoolsoort | aantal_asielzoekers_peildatum | aantal_overige_vreemdelingen_peildatum | aantal_asielzoekers_telling_1_februari | aantal_tweedejaars_asielzoekers_peildatum | eerste_keer_eerste_opvang |
      | S3        | basisschool | 5                             | 1                                      | 3                                      | 0                                         | false                     |
    Given parameter "school_id" is "S3"
    Given the calculation date is "2026-01-01"
    # (5 - 3) x 15.168,16 x 25% + (3 + 1) x 4.712,34 x 25% = 7.584,08 + 4.712,34
    When I evaluate "bedrag_eerste_opvang_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_eerste_opvang_peildatum" equals 1229642

  Scenario: Peildatum 1 januari met niet meer asielzoekers dan in de 1-februari-telling
    Given the following "scholen" data with key "school_id":
      | school_id | schoolsoort | aantal_asielzoekers_peildatum | aantal_overige_vreemdelingen_peildatum | aantal_asielzoekers_telling_1_februari | aantal_tweedejaars_asielzoekers_peildatum | eerste_keer_eerste_opvang |
      | S4        | basisschool | 2                             | 2                                      | 3                                      | 0                                         | false                     |
    Given parameter "school_id" is "S4"
    Given the calculation date is "2026-01-01"
    # (2 + 2) x 4.712,34 x 25% = 4.712,34
    When I evaluate "bedrag_eerste_opvang_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_eerste_opvang_peildatum" equals 471234

  Scenario: Speciale school voor basisonderwijs, één tarief en geen tweede jaar
    Given the following "scholen" data with key "school_id":
      | school_id | schoolsoort | aantal_asielzoekers_peildatum | aantal_overige_vreemdelingen_peildatum | aantal_asielzoekers_telling_1_februari | aantal_tweedejaars_asielzoekers_peildatum | eerste_keer_eerste_opvang |
      | S5        | sbo         | 1                             | 3                                      | 0                                      | 0                                         | true                      |
    Given parameter "school_id" is "S5"
    Given the calculation date is "2026-04-01"
    When I evaluate "voldoet_aan_drempel" of "regeling_bekostiging_wpo_en_wec"
    Then output "voldoet_aan_drempel" is true
    # 4 x 4.712,34 x 25%
    When I evaluate "bedrag_eerste_opvang_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_eerste_opvang_peildatum" equals 471234
    When I evaluate "toeslag_eerste_keer" of "regeling_bekostiging_wpo_en_wec"
    Then output "toeslag_eerste_keer" equals 1817457
    When I evaluate "bedrag_tweede_jaar_peildatum" of "regeling_bekostiging_wpo_en_wec"
    Then output "bedrag_tweede_jaar_peildatum" equals 0
