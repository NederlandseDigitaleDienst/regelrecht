Feature: Regio en vakantieperioden volgens de Regeling vaststelling schoolvakanties 2025–2030
  De plaats van vestiging bepaalt de regio van een school (artikel 2). Artikel
  3 deelt de regio's in naar provincie en gemeente, en voor enkele gemeenten
  naar een voormalige gemeente of kern. Artikel 5 stelt de kerst- en
  meivakantie vast voor alle scholen buiten Caribisch Nederland, artikel 6 de
  zomervakantie per regio.

  Background:
    Given the calculation date is "2026-10-20"

  Scenario: Een gemeente in Noord-Holland ligt in regio Noord
    Given the following parameters:
      | gemeente          | Haarlem       |
      | provincie         | Noord-Holland |
      | deel_van_gemeente | null          |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then the execution succeeds
    Then output "regio" equals "Noord"

  Scenario: Zeewolde ligt in regio Midden, de rest van Flevoland in Noord
    Given the following parameters:
      | gemeente          | Zeewolde  |
      | provincie         | Flevoland |
      | deel_van_gemeente | null      |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Midden"

  Scenario: Almere ligt in regio Noord
    Given the following parameters:
      | gemeente          | Almere    |
      | provincie         | Flevoland |
      | deel_van_gemeente | null      |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Noord"

  Scenario: Eemnes ligt in regio Noord, de rest van Utrecht in Midden
    Given the following parameters:
      | gemeente          | Eemnes  |
      | provincie         | Utrecht |
      | deel_van_gemeente | null    |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Noord"

  Scenario: De voormalige gemeente Abcoude ligt in regio Noord
    Given the following parameters:
      | gemeente          | De Ronde Venen |
      | provincie         | Utrecht        |
      | deel_van_gemeente | Abcoude        |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Noord"

  Scenario: Amersfoort ligt in regio Midden
    Given the following parameters:
      | gemeente          | Amersfoort |
      | provincie         | Utrecht    |
      | deel_van_gemeente | null       |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Midden"

  Scenario: Hattem ligt als enige gemeente van Gelderland in regio Noord
    Given the following parameters:
      | gemeente          | Hattem     |
      | provincie         | Gelderland |
      | deel_van_gemeente | null       |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Noord"

  Scenario: Apeldoorn ligt in regio Midden
    Given the following parameters:
      | gemeente          | Apeldoorn  |
      | provincie         | Gelderland |
      | deel_van_gemeente | null       |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Midden"

  Scenario: Nijmegen ligt in regio Zuid
    Given the following parameters:
      | gemeente          | Nijmegen   |
      | provincie         | Gelderland |
      | deel_van_gemeente | null       |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Zuid"

  Scenario: De voormalige gemeente Didam in Montferland ligt in regio Zuid
    Given the following parameters:
      | gemeente          | Montferland |
      | provincie         | Gelderland  |
      | deel_van_gemeente | Didam       |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Zuid"

  Scenario: Montferland buiten Didam ligt in regio Midden
    Given the following parameters:
      | gemeente          | Montferland |
      | provincie         | Gelderland  |
      | deel_van_gemeente | null        |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Midden"

  Scenario: De kern Dussen in Altena ligt in regio Zuid
    Given the following parameters:
      | gemeente          | Altena        |
      | provincie         | Noord-Brabant |
      | deel_van_gemeente | Dussen        |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Zuid"

  Scenario: Altena buiten Aalburg, Hank en Dussen ligt in regio Midden
    Given the following parameters:
      | gemeente          | Altena        |
      | provincie         | Noord-Brabant |
      | deel_van_gemeente | null          |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Midden"

  Scenario: Een gemeente in Limburg ligt in regio Zuid
    Given the following parameters:
      | gemeente          | Maastricht |
      | provincie         | Limburg    |
      | deel_van_gemeente | null       |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Zuid"

  Scenario: Bonaire ligt in regio Caribisch Nederland
    Given the following parameters:
      | gemeente          | Bonaire |
      | provincie         | null    |
      | deel_van_gemeente | null    |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Caribisch Nederland"

  Scenario: West Betuwe noemt artikel 3 niet, dus ligt het in geen regio
    Given the following parameters:
      | gemeente          | West Betuwe |
      | provincie         | Gelderland  |
      | deel_van_gemeente | null        |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then the execution succeeds
    Then output "regio" is absent

  Scenario: Zonder plaats van vestiging behoort de school tot geen regio
    # De regeling zegt niet waar een school gevestigd is. In de cel van de
    # school koppelt het beleid van het hoofd de plaats van vestiging aan het
    # antwoord van de cel, uit haar kroniek (RFC-048, RFC-051); hier geeft
    # niemand dat antwoord.
    When I evaluate "regio_van_school" of "regeling_vaststelling_schoolvakanties_20252030"
    Then the execution succeeds
    Then output "regio_van_school" is absent

  Scenario: De kerstvakantie van het schooljaar 2026–2027 geldt voor elke regio
    Given the following parameters:
      | eerste_jaar_schooljaar | 2026 |
      | regio                  | Zuid |
    When I evaluate "kerstvakantie_eerste_dag" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "kerstvakantie_eerste_dag" equals "2026-12-19"
    When I evaluate "kerstvakantie_laatste_dag" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "kerstvakantie_laatste_dag" equals "2027-01-03"
    When I evaluate "meivakantie_eerste_dag" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "meivakantie_eerste_dag" equals "2027-04-24"
    When I evaluate "meivakantie_laatste_dag" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "meivakantie_laatste_dag" equals "2027-05-02"

  Scenario: Caribisch Nederland heeft geen kerst- en meivakantie uit artikel 5
    Given the following parameters:
      | eerste_jaar_schooljaar | 2026                |
      | regio                  | Caribisch Nederland |
    When I evaluate "kerstvakantie_eerste_dag" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "kerstvakantie_eerste_dag" is absent
    When I evaluate "meivakantie_laatste_dag" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "meivakantie_laatste_dag" is absent

  Scenario: De zomervakantie van 2027 verschilt per regio
    Given the following parameters:
      | jaar  | 2027  |
      | regio | Noord |
    When I evaluate "zomervakantie_eerste_dag" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "zomervakantie_eerste_dag" equals "2027-07-10"
    When I evaluate "zomervakantie_laatste_dag" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "zomervakantie_laatste_dag" equals "2027-08-22"

  Scenario: In regio Zuid loopt de zomervakantie van 2027 tot in september
    Given the following parameters:
      | jaar  | 2027  |
      | regio | Zuid  |
    When I evaluate "zomervakantie_eerste_dag" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "zomervakantie_eerste_dag" equals "2027-07-24"
    When I evaluate "zomervakantie_laatste_dag" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "zomervakantie_laatste_dag" equals "2027-09-05"

  Scenario: Voor een jaar buiten de regeling stelt zij geen zomervakantie vast
    Given the following parameters:
      | jaar  | 2031  |
      | regio | Noord |
    When I evaluate "zomervakantie_eerste_dag" of "regeling_vaststelling_schoolvakanties_20252030"
    Then the execution succeeds
    Then output "zomervakantie_eerste_dag" is absent

  Scenario: De voormalige gemeente Aalburg in Altena ligt in regio Zuid
    Given the following parameters:
      | gemeente          | Altena        |
      | provincie         | Noord-Brabant |
      | deel_van_gemeente | Aalburg       |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Zuid"

  Scenario: De kern Hank in Altena ligt in regio Zuid
    Given the following parameters:
      | gemeente          | Altena        |
      | provincie         | Noord-Brabant |
      | deel_van_gemeente | Hank          |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Zuid"

  Scenario: Neder-Betuwe ligt in regio Midden
    Given the following parameters:
      | gemeente          | Neder-Betuwe |
      | provincie         | Gelderland   |
      | deel_van_gemeente | null         |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Midden"

  Scenario: De voormalige gemeente Dodewaard in Neder-Betuwe ligt in regio Zuid
    Given the following parameters:
      | gemeente          | Neder-Betuwe |
      | provincie         | Gelderland   |
      | deel_van_gemeente | Dodewaard    |
    When I evaluate "regio" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "regio" equals "Zuid"

  Scenario: De grote vakantie van 2028 in Caribisch Nederland
    Given the following parameters:
      | jaar  | 2028                |
      | regio | Caribisch Nederland |
    When I evaluate "zomervakantie_eerste_dag" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "zomervakantie_eerste_dag" equals "2028-07-08"
    When I evaluate "zomervakantie_laatste_dag" of "regeling_vaststelling_schoolvakanties_20252030"
    Then output "zomervakantie_laatste_dag" equals "2028-08-20"
