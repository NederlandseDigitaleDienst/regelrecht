Feature: Langdurig laag inkomen (Verordening IIT Groningen, artikelen 1, 3 en 3a)
  Een persoon heeft een langdurig laag inkomen als zijn inkomen in elke maand van de
  referteperiode (twee jaar voor de peildatum) niet hoger is dan de toepasselijke
  bijstandsnorm. Artikel 3a laat eenmalig een onderbreking van ten hoogste drie
  aaneengesloten maanden toe, tot 140 procent van de norm, na twee jaar laag inkomen.

  # Bron: toelichting bij de verordening, CVDR367471/3
  #   (dossiers/iit-groningen/bronnen/gemeente/tekst/CVDR367471_3-toelichting.txt)
  # URL: https://lokaleregelgeving.overheid.nl/CVDR367471/3
  #
  # De bedragen zijn schematisch: een norm van 1.000 euro per maand en een inkomen van
  # 1.000 euro algemene bijstand. De verordening rekent per maand met de norm die de
  # aanroeper meegeeft (markering op artikel 3). Inkomen en norm staan per maand in
  # eurocent, de maand als eerste dag van die maand. Peildatum 1 mei 2023: de
  # referteperiode loopt van 1 mei 2021 tot en met april 2023.
  #
  # Niet als scenario opgenomen: de toelichting laat een "marginale overschrijding" van
  # minder dan ongeveer 5 euro per maand buiten beschouwing (regel 24). Die regel staat
  # niet in de tekst van artikel 3 en zit dus niet in het model (vraag U8).

  Background:
    Given the calculation date is "2023-05-01"
    Given parameter "datum_verzoek" is "2023-05-01"
    Given the following parameters:
      | artikel_3a_eerder_toegepast | false |
    Given parameter "toepasselijke_bijstandsnormen" is the collection:
      | maand      | norm   |
      | 2020-11-01 | 100000 |
      | 2020-12-01 | 100000 |
      | 2021-01-01 | 100000 |
      | 2021-02-01 | 100000 |
      | 2021-03-01 | 100000 |
      | 2021-04-01 | 100000 |
      | 2021-05-01 | 100000 |
      | 2021-06-01 | 100000 |
      | 2021-07-01 | 100000 |
      | 2021-08-01 | 100000 |
      | 2021-09-01 | 100000 |
      | 2021-10-01 | 100000 |
      | 2021-11-01 | 100000 |
      | 2021-12-01 | 100000 |
      | 2022-01-01 | 100000 |
      | 2022-02-01 | 100000 |
      | 2022-03-01 | 100000 |
      | 2022-04-01 | 100000 |
      | 2022-05-01 | 100000 |
      | 2022-06-01 | 100000 |
      | 2022-07-01 | 100000 |
      | 2022-08-01 | 100000 |
      | 2022-09-01 | 100000 |
      | 2022-10-01 | 100000 |
      | 2022-11-01 | 100000 |
      | 2022-12-01 | 100000 |
      | 2023-01-01 | 100000 |
      | 2023-02-01 | 100000 |
      | 2023-03-01 | 100000 |
      | 2023-04-01 | 100000 |

  # === Artikel 3: niet hoger dan de bijstandsnorm ===

  Scenario: Twee jaar op bijstandsniveau voldoet aan artikel 3
    # Bron: toelichting, artikel 3 onder "Laag inkomen" (regel 23): laag is niet hoger
    # dan honderd procent van de toepasselijke bijstandsnorm; artikel 1, referteperiode.
    Given parameter "maandgegevens" is the collection:
      | maand      | inkomen_artikel_32 | algemene_bijstand |
      | 2020-11-01 | 0                  | 100000            |
      | 2020-12-01 | 0                  | 100000            |
      | 2021-01-01 | 0                  | 100000            |
      | 2021-02-01 | 0                  | 100000            |
      | 2021-03-01 | 0                  | 100000            |
      | 2021-04-01 | 0                  | 100000            |
      | 2021-05-01 | 0                  | 100000            |
      | 2021-06-01 | 0                  | 100000            |
      | 2021-07-01 | 0                  | 100000            |
      | 2021-08-01 | 0                  | 100000            |
      | 2021-09-01 | 0                  | 100000            |
      | 2021-10-01 | 0                  | 100000            |
      | 2021-11-01 | 0                  | 100000            |
      | 2021-12-01 | 0                  | 100000            |
      | 2022-01-01 | 0                  | 100000            |
      | 2022-02-01 | 0                  | 100000            |
      | 2022-03-01 | 0                  | 100000            |
      | 2022-04-01 | 0                  | 100000            |
      | 2022-05-01 | 0                  | 100000            |
      | 2022-06-01 | 0                  | 100000            |
      | 2022-07-01 | 0                  | 100000            |
      | 2022-08-01 | 0                  | 100000            |
      | 2022-09-01 | 0                  | 100000            |
      | 2022-10-01 | 0                  | 100000            |
      | 2022-11-01 | 0                  | 100000            |
      | 2022-12-01 | 0                  | 100000            |
      | 2023-01-01 | 0                  | 100000            |
      | 2023-02-01 | 0                  | 100000            |
      | 2023-03-01 | 0                  | 100000            |
      | 2023-04-01 | 0                  | 100000            |
    When I evaluate "heeft_langdurig_laag_inkomen_artikel_3" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "heeft_langdurig_laag_inkomen_artikel_3" is true

  Scenario: Twee jaar op bijstandsniveau is een langdurig laag inkomen
    # Bron: toelichting, artikel 3 onder "Laag inkomen" (regel 23): laag is niet hoger
    # dan honderd procent van de toepasselijke bijstandsnorm; artikel 1, referteperiode.
    Given parameter "maandgegevens" is the collection:
      | maand      | inkomen_artikel_32 | algemene_bijstand |
      | 2020-11-01 | 0                  | 100000            |
      | 2020-12-01 | 0                  | 100000            |
      | 2021-01-01 | 0                  | 100000            |
      | 2021-02-01 | 0                  | 100000            |
      | 2021-03-01 | 0                  | 100000            |
      | 2021-04-01 | 0                  | 100000            |
      | 2021-05-01 | 0                  | 100000            |
      | 2021-06-01 | 0                  | 100000            |
      | 2021-07-01 | 0                  | 100000            |
      | 2021-08-01 | 0                  | 100000            |
      | 2021-09-01 | 0                  | 100000            |
      | 2021-10-01 | 0                  | 100000            |
      | 2021-11-01 | 0                  | 100000            |
      | 2021-12-01 | 0                  | 100000            |
      | 2022-01-01 | 0                  | 100000            |
      | 2022-02-01 | 0                  | 100000            |
      | 2022-03-01 | 0                  | 100000            |
      | 2022-04-01 | 0                  | 100000            |
      | 2022-05-01 | 0                  | 100000            |
      | 2022-06-01 | 0                  | 100000            |
      | 2022-07-01 | 0                  | 100000            |
      | 2022-08-01 | 0                  | 100000            |
      | 2022-09-01 | 0                  | 100000            |
      | 2022-10-01 | 0                  | 100000            |
      | 2022-11-01 | 0                  | 100000            |
      | 2022-12-01 | 0                  | 100000            |
      | 2023-01-01 | 0                  | 100000            |
      | 2023-02-01 | 0                  | 100000            |
      | 2023-03-01 | 0                  | 100000            |
      | 2023-04-01 | 0                  | 100000            |
    When I evaluate "heeft_langdurig_laag_inkomen" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "heeft_langdurig_laag_inkomen" is true

  Scenario: Een maand boven de norm doorbreekt artikel 3
    # Bron: tekst van artikel 3 ("gedurende de referteperiode ... niet hoger").
    Given parameter "maandgegevens" is the collection:
      | maand      | inkomen_artikel_32 | algemene_bijstand |
      | 2020-11-01 | 0                  | 100000            |
      | 2020-12-01 | 0                  | 100000            |
      | 2021-01-01 | 0                  | 100000            |
      | 2021-02-01 | 0                  | 100000            |
      | 2021-03-01 | 0                  | 100000            |
      | 2021-04-01 | 0                  | 100000            |
      | 2021-05-01 | 0                  | 100000            |
      | 2021-06-01 | 0                  | 100000            |
      | 2021-07-01 | 0                  | 100000            |
      | 2021-08-01 | 0                  | 100000            |
      | 2021-09-01 | 0                  | 100000            |
      | 2021-10-01 | 0                  | 100000            |
      | 2021-11-01 | 0                  | 100000            |
      | 2021-12-01 | 0                  | 100000            |
      | 2022-01-01 | 0                  | 100000            |
      | 2022-02-01 | 0                  | 100000            |
      | 2022-03-01 | 0                  | 100000            |
      | 2022-04-01 | 0                  | 100000            |
      | 2022-05-01 | 0                  | 100000            |
      | 2022-06-01 | 0                  | 100000            |
      | 2022-07-01 | 0                  | 100000            |
      | 2022-08-01 | 0                  | 100000            |
      | 2022-09-01 | 0                  | 100000            |
      | 2022-10-01 | 0                  | 100000            |
      | 2022-11-01 | 160000             | 0                 |
      | 2022-12-01 | 0                  | 100000            |
      | 2023-01-01 | 0                  | 100000            |
      | 2023-02-01 | 0                  | 100000            |
      | 2023-03-01 | 0                  | 100000            |
      | 2023-04-01 | 0                  | 100000            |
    When I evaluate "heeft_langdurig_laag_inkomen_artikel_3" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "heeft_langdurig_laag_inkomen_artikel_3" is false

  Scenario: Zonder inkomen voor elke maand van de referteperiode is er geen uitkomst
    # Bron: tekst van artikel 3; eigen keuze van het model (regelwerken-model.md):
    # een ontbrekende maand geeft geen uitkomst, geen "nee".
    Given parameter "maandgegevens" is the collection:
      | maand      | inkomen_artikel_32 | algemene_bijstand |
      | 2020-11-01 | 0                  | 100000            |
      | 2020-12-01 | 0                  | 100000            |
      | 2021-01-01 | 0                  | 100000            |
      | 2021-02-01 | 0                  | 100000            |
      | 2021-03-01 | 0                  | 100000            |
      | 2021-04-01 | 0                  | 100000            |
      | 2021-05-01 | 0                  | 100000            |
      | 2021-06-01 | 0                  | 100000            |
      | 2021-07-01 | 0                  | 100000            |
      | 2021-08-01 | 0                  | 100000            |
      | 2021-09-01 | 0                  | 100000            |
      | 2021-10-01 | 0                  | 100000            |
      | 2021-11-01 | 0                  | 100000            |
      | 2021-12-01 | 0                  | 100000            |
      | 2022-01-01 | 0                  | 100000            |
      | 2022-03-01 | 0                  | 100000            |
      | 2022-04-01 | 0                  | 100000            |
      | 2022-05-01 | 0                  | 100000            |
      | 2022-06-01 | 0                  | 100000            |
      | 2022-07-01 | 0                  | 100000            |
      | 2022-08-01 | 0                  | 100000            |
      | 2022-09-01 | 0                  | 100000            |
      | 2022-10-01 | 0                  | 100000            |
      | 2022-11-01 | 0                  | 100000            |
      | 2022-12-01 | 0                  | 100000            |
      | 2023-01-01 | 0                  | 100000            |
      | 2023-02-01 | 0                  | 100000            |
      | 2023-03-01 | 0                  | 100000            |
      | 2023-04-01 | 0                  | 100000            |
    When I evaluate "heeft_langdurig_laag_inkomen_artikel_3" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "heeft_langdurig_laag_inkomen_artikel_3" is absent

  # === Artikel 3a: tijdelijk hoger inkomen ===

  Scenario: Drie maanden werk tot 130 procent doorbreekt artikel 3
    # Bron: toelichting, artikel 3a (regel 26): "kan hij eenmalig gedurende enkele maanden
    # een hoger inkomen ontvangen, terwijl hij blijft voldoen aan de eis van een langdurig
    # laag inkomen." Van november 2020 tot november 2022 laag, daarna drie maanden hoger.
    Given parameter "maandgegevens" is the collection:
      | maand      | inkomen_artikel_32 | algemene_bijstand |
      | 2020-11-01 | 0                  | 100000            |
      | 2020-12-01 | 0                  | 100000            |
      | 2021-01-01 | 0                  | 100000            |
      | 2021-02-01 | 0                  | 100000            |
      | 2021-03-01 | 0                  | 100000            |
      | 2021-04-01 | 0                  | 100000            |
      | 2021-05-01 | 0                  | 100000            |
      | 2021-06-01 | 0                  | 100000            |
      | 2021-07-01 | 0                  | 100000            |
      | 2021-08-01 | 0                  | 100000            |
      | 2021-09-01 | 0                  | 100000            |
      | 2021-10-01 | 0                  | 100000            |
      | 2021-11-01 | 0                  | 100000            |
      | 2021-12-01 | 0                  | 100000            |
      | 2022-01-01 | 0                  | 100000            |
      | 2022-02-01 | 0                  | 100000            |
      | 2022-03-01 | 0                  | 100000            |
      | 2022-04-01 | 0                  | 100000            |
      | 2022-05-01 | 0                  | 100000            |
      | 2022-06-01 | 0                  | 100000            |
      | 2022-07-01 | 0                  | 100000            |
      | 2022-08-01 | 0                  | 100000            |
      | 2022-09-01 | 0                  | 100000            |
      | 2022-10-01 | 0                  | 100000            |
      | 2022-11-01 | 130000             | 0                 |
      | 2022-12-01 | 130000             | 0                 |
      | 2023-01-01 | 130000             | 0                 |
      | 2023-02-01 | 0                  | 100000            |
      | 2023-03-01 | 0                  | 100000            |
      | 2023-04-01 | 0                  | 100000            |
    When I evaluate "heeft_langdurig_laag_inkomen_artikel_3" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "heeft_langdurig_laag_inkomen_artikel_3" is false

  Scenario: Drie maanden werk tot 130 procent houdt het recht in stand
    # Bron: toelichting, artikel 3a (regel 26): "kan hij eenmalig gedurende enkele maanden
    # een hoger inkomen ontvangen, terwijl hij blijft voldoen aan de eis van een langdurig
    # laag inkomen." Van november 2020 tot november 2022 laag, daarna drie maanden hoger.
    Given parameter "maandgegevens" is the collection:
      | maand      | inkomen_artikel_32 | algemene_bijstand |
      | 2020-11-01 | 0                  | 100000            |
      | 2020-12-01 | 0                  | 100000            |
      | 2021-01-01 | 0                  | 100000            |
      | 2021-02-01 | 0                  | 100000            |
      | 2021-03-01 | 0                  | 100000            |
      | 2021-04-01 | 0                  | 100000            |
      | 2021-05-01 | 0                  | 100000            |
      | 2021-06-01 | 0                  | 100000            |
      | 2021-07-01 | 0                  | 100000            |
      | 2021-08-01 | 0                  | 100000            |
      | 2021-09-01 | 0                  | 100000            |
      | 2021-10-01 | 0                  | 100000            |
      | 2021-11-01 | 0                  | 100000            |
      | 2021-12-01 | 0                  | 100000            |
      | 2022-01-01 | 0                  | 100000            |
      | 2022-02-01 | 0                  | 100000            |
      | 2022-03-01 | 0                  | 100000            |
      | 2022-04-01 | 0                  | 100000            |
      | 2022-05-01 | 0                  | 100000            |
      | 2022-06-01 | 0                  | 100000            |
      | 2022-07-01 | 0                  | 100000            |
      | 2022-08-01 | 0                  | 100000            |
      | 2022-09-01 | 0                  | 100000            |
      | 2022-10-01 | 0                  | 100000            |
      | 2022-11-01 | 130000             | 0                 |
      | 2022-12-01 | 130000             | 0                 |
      | 2023-01-01 | 130000             | 0                 |
      | 2023-02-01 | 0                  | 100000            |
      | 2023-03-01 | 0                  | 100000            |
      | 2023-04-01 | 0                  | 100000            |
    When I evaluate "heeft_langdurig_laag_inkomen" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "heeft_langdurig_laag_inkomen" is true

  Scenario: Boven 140 procent telt de onderbreking wel
    # Bron: tekst van artikel 3a, eerste lid, onder a.
    Given parameter "maandgegevens" is the collection:
      | maand      | inkomen_artikel_32 | algemene_bijstand |
      | 2020-11-01 | 0                  | 100000            |
      | 2020-12-01 | 0                  | 100000            |
      | 2021-01-01 | 0                  | 100000            |
      | 2021-02-01 | 0                  | 100000            |
      | 2021-03-01 | 0                  | 100000            |
      | 2021-04-01 | 0                  | 100000            |
      | 2021-05-01 | 0                  | 100000            |
      | 2021-06-01 | 0                  | 100000            |
      | 2021-07-01 | 0                  | 100000            |
      | 2021-08-01 | 0                  | 100000            |
      | 2021-09-01 | 0                  | 100000            |
      | 2021-10-01 | 0                  | 100000            |
      | 2021-11-01 | 0                  | 100000            |
      | 2021-12-01 | 0                  | 100000            |
      | 2022-01-01 | 0                  | 100000            |
      | 2022-02-01 | 0                  | 100000            |
      | 2022-03-01 | 0                  | 100000            |
      | 2022-04-01 | 0                  | 100000            |
      | 2022-05-01 | 0                  | 100000            |
      | 2022-06-01 | 0                  | 100000            |
      | 2022-07-01 | 0                  | 100000            |
      | 2022-08-01 | 0                  | 100000            |
      | 2022-09-01 | 0                  | 100000            |
      | 2022-10-01 | 0                  | 100000            |
      | 2022-11-01 | 150000             | 0                 |
      | 2022-12-01 | 0                  | 100000            |
      | 2023-01-01 | 0                  | 100000            |
      | 2023-02-01 | 0                  | 100000            |
      | 2023-03-01 | 0                  | 100000            |
      | 2023-04-01 | 0                  | 100000            |
    When I evaluate "heeft_langdurig_laag_inkomen" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "heeft_langdurig_laag_inkomen" is false

  Scenario: Vier maanden is langer dan de onderbreking die artikel 3a toelaat
    # Bron: tekst van artikel 3a, eerste lid: "maximaal drie maanden".
    Given parameter "maandgegevens" is the collection:
      | maand      | inkomen_artikel_32 | algemene_bijstand |
      | 2020-11-01 | 0                  | 100000            |
      | 2020-12-01 | 0                  | 100000            |
      | 2021-01-01 | 0                  | 100000            |
      | 2021-02-01 | 0                  | 100000            |
      | 2021-03-01 | 0                  | 100000            |
      | 2021-04-01 | 0                  | 100000            |
      | 2021-05-01 | 0                  | 100000            |
      | 2021-06-01 | 0                  | 100000            |
      | 2021-07-01 | 0                  | 100000            |
      | 2021-08-01 | 0                  | 100000            |
      | 2021-09-01 | 0                  | 100000            |
      | 2021-10-01 | 0                  | 100000            |
      | 2021-11-01 | 0                  | 100000            |
      | 2021-12-01 | 0                  | 100000            |
      | 2022-01-01 | 0                  | 100000            |
      | 2022-02-01 | 0                  | 100000            |
      | 2022-03-01 | 0                  | 100000            |
      | 2022-04-01 | 0                  | 100000            |
      | 2022-05-01 | 0                  | 100000            |
      | 2022-06-01 | 0                  | 100000            |
      | 2022-07-01 | 0                  | 100000            |
      | 2022-08-01 | 0                  | 100000            |
      | 2022-09-01 | 0                  | 100000            |
      | 2022-10-01 | 0                  | 100000            |
      | 2022-11-01 | 130000             | 0                 |
      | 2022-12-01 | 130000             | 0                 |
      | 2023-01-01 | 130000             | 0                 |
      | 2023-02-01 | 130000             | 0                 |
      | 2023-03-01 | 0                  | 100000            |
      | 2023-04-01 | 0                  | 100000            |
    When I evaluate "heeft_langdurig_laag_inkomen" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "heeft_langdurig_laag_inkomen" is false

  Scenario: Twee losse maanden zijn geen aaneengesloten termijn
    # Bron: tekst van artikel 3a, eerste lid: "een aaneengesloten termijn".
    Given parameter "maandgegevens" is the collection:
      | maand      | inkomen_artikel_32 | algemene_bijstand |
      | 2020-11-01 | 0                  | 100000            |
      | 2020-12-01 | 0                  | 100000            |
      | 2021-01-01 | 0                  | 100000            |
      | 2021-02-01 | 0                  | 100000            |
      | 2021-03-01 | 0                  | 100000            |
      | 2021-04-01 | 0                  | 100000            |
      | 2021-05-01 | 0                  | 100000            |
      | 2021-06-01 | 0                  | 100000            |
      | 2021-07-01 | 0                  | 100000            |
      | 2021-08-01 | 0                  | 100000            |
      | 2021-09-01 | 0                  | 100000            |
      | 2021-10-01 | 0                  | 100000            |
      | 2021-11-01 | 0                  | 100000            |
      | 2021-12-01 | 0                  | 100000            |
      | 2022-01-01 | 0                  | 100000            |
      | 2022-02-01 | 0                  | 100000            |
      | 2022-03-01 | 0                  | 100000            |
      | 2022-04-01 | 0                  | 100000            |
      | 2022-05-01 | 0                  | 100000            |
      | 2022-06-01 | 0                  | 100000            |
      | 2022-07-01 | 0                  | 100000            |
      | 2022-08-01 | 0                  | 100000            |
      | 2022-09-01 | 0                  | 100000            |
      | 2022-10-01 | 0                  | 100000            |
      | 2022-11-01 | 130000             | 0                 |
      | 2022-12-01 | 0                  | 100000            |
      | 2023-01-01 | 130000             | 0                 |
      | 2023-02-01 | 0                  | 100000            |
      | 2023-03-01 | 0                  | 100000            |
      | 2023-04-01 | 0                  | 100000            |
    When I evaluate "heeft_langdurig_laag_inkomen" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "heeft_langdurig_laag_inkomen" is false

  Scenario: Artikel 3a geldt maar één keer
    # Bron: tekst van artikel 3a, tweede lid: "eenmalig van toepassing".
    Given the following parameters:
      | artikel_3a_eerder_toegepast | true |
    Given parameter "maandgegevens" is the collection:
      | maand      | inkomen_artikel_32 | algemene_bijstand |
      | 2020-11-01 | 0                  | 100000            |
      | 2020-12-01 | 0                  | 100000            |
      | 2021-01-01 | 0                  | 100000            |
      | 2021-02-01 | 0                  | 100000            |
      | 2021-03-01 | 0                  | 100000            |
      | 2021-04-01 | 0                  | 100000            |
      | 2021-05-01 | 0                  | 100000            |
      | 2021-06-01 | 0                  | 100000            |
      | 2021-07-01 | 0                  | 100000            |
      | 2021-08-01 | 0                  | 100000            |
      | 2021-09-01 | 0                  | 100000            |
      | 2021-10-01 | 0                  | 100000            |
      | 2021-11-01 | 0                  | 100000            |
      | 2021-12-01 | 0                  | 100000            |
      | 2022-01-01 | 0                  | 100000            |
      | 2022-02-01 | 0                  | 100000            |
      | 2022-03-01 | 0                  | 100000            |
      | 2022-04-01 | 0                  | 100000            |
      | 2022-05-01 | 0                  | 100000            |
      | 2022-06-01 | 0                  | 100000            |
      | 2022-07-01 | 0                  | 100000            |
      | 2022-08-01 | 0                  | 100000            |
      | 2022-09-01 | 0                  | 100000            |
      | 2022-10-01 | 0                  | 100000            |
      | 2022-11-01 | 130000             | 0                 |
      | 2022-12-01 | 0                  | 100000            |
      | 2023-01-01 | 0                  | 100000            |
      | 2023-02-01 | 0                  | 100000            |
      | 2023-03-01 | 0                  | 100000            |
      | 2023-04-01 | 0                  | 100000            |
    When I evaluate "heeft_langdurig_laag_inkomen" of "verordening_individuele_inkomenstoeslag_participatiewet_groningen"
    Then the execution succeeds
    Then output "heeft_langdurig_laag_inkomen" is false
