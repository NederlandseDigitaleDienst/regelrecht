Feature: Geen kaart voordat het geneeskundig onderzoek heeft plaatsgehad
  Artikel 2, eerste lid: een gehandicaptenparkeerkaart wordt niet afgegeven
  alvorens een geneeskundig onderzoek heeft plaatsgehad. Dat stelt de
  verstrekkende instantie vast op het rapport van de GGD of de aangewezen
  deskundige (artikel 3, eerste lid); in een cel van de GGD haalt haar
  reductie het uit de adviezen die zij vastlegde, en het beleid van het
  college koppelt het begrip aan dat antwoord (RFC-048). Of eerder een kaart is
  verstrekt, zegt de verstrekkende instantie (lid 2, onder a en b). Het tweede lid laat het
  onderzoek achterwege bij wie eerder een kaart kreeg, als de verstrekkende
  instantie weet dat hij nog steeds voldoet (onder a) of de keurende
  instantie daarover oordeelt (onder b). Of het college die bevoegdheid
  gebruikt, en de oordelen onder a en b, geeft artikel 49 van het BABW mee.
  Onder c en het derde lid zijn niet gemodelleerd.

  Background:
    Given the calculation date is "2026-10-20"

  Scenario: Na het onderzoek mag de kaart worden afgegeven
    Given the following parameters:
      | bsn_aanvrager                                                | 999990020  |
      | dagtekening_aanvraag                                         | 2026-10-12 |
      | datum_afgifte                                                | 2026-10-20 |
      | geneeskundig_onderzoek_achterwege_gelaten                    | null       |
      | bekend_dat_aanvrager_nog_steeds_voldoet                      | null       |
      | keurende_instantie_oordeelt_dat_aanvrager_nog_steeds_voldoet | null       |
    Given the following findings:
      | geneeskundig_onderzoek_heeft_plaatsgehad_volgens_college             | true  |
    When I evaluate "gehandicaptenparkeerkaart_mag_worden_afgegeven" of "regeling_gehandicaptenparkeerkaart"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_mag_worden_afgegeven" is true

  Scenario: Zonder vaststelling over het onderzoek mag de kaart niet worden afgegeven
    # Eigen keuze: zonder antwoord en zonder vaststelling was er geen
    # onderzoek. Artikel 49 van het BABW weigert de kaart dan.
    Given the following parameters:
      | bsn_aanvrager                                                | 999990020  |
      | dagtekening_aanvraag                                         | 2026-10-12 |
      | datum_afgifte                                                | 2026-10-20 |
      | geneeskundig_onderzoek_achterwege_gelaten                    | null       |
      | bekend_dat_aanvrager_nog_steeds_voldoet                      | null       |
      | keurende_instantie_oordeelt_dat_aanvrager_nog_steeds_voldoet | null       |
    When I evaluate "gehandicaptenparkeerkaart_mag_worden_afgegeven" of "regeling_gehandicaptenparkeerkaart"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_mag_worden_afgegeven" is false

  Scenario: Stelt het college vast dat er geen onderzoek was, dan mag de kaart niet worden afgegeven
    # Eigen keuze: bijvoorbeeld omdat de aanvrager niet verscheen. Artikel 49
    # van het BABW weigert de kaart dan.
    Given the following parameters:
      | bsn_aanvrager                                                | 999990020  |
      | dagtekening_aanvraag                                         | 2026-10-12 |
      | datum_afgifte                                                | 2026-10-20 |
      | geneeskundig_onderzoek_achterwege_gelaten                    | null       |
      | bekend_dat_aanvrager_nog_steeds_voldoet                      | null       |
      | keurende_instantie_oordeelt_dat_aanvrager_nog_steeds_voldoet | null       |
    Given the following findings:
      | geneeskundig_onderzoek_heeft_plaatsgehad_volgens_college             | false |
    When I evaluate "gehandicaptenparkeerkaart_mag_worden_afgegeven" of "regeling_gehandicaptenparkeerkaart"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_mag_worden_afgegeven" is false

  Scenario: Onder a blijft het onderzoek achterwege bij wie eerder een kaart kreeg
    Given the following parameters:
      | bsn_aanvrager                                                | 999990020  |
      | dagtekening_aanvraag                                         | 2026-10-12 |
      | datum_afgifte                                                | 2026-10-20 |
      | geneeskundig_onderzoek_achterwege_gelaten                    | true       |
      | bekend_dat_aanvrager_nog_steeds_voldoet                      | true       |
      | keurende_instantie_oordeelt_dat_aanvrager_nog_steeds_voldoet | null       |
    Given the following findings:
      | aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt_volgens_college | true  |
    When I evaluate "gehandicaptenparkeerkaart_mag_worden_afgegeven" of "regeling_gehandicaptenparkeerkaart"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_mag_worden_afgegeven" is true

  Scenario: Onder b blijft het onderzoek achterwege op het oordeel van de keurende instantie
    Given the following parameters:
      | bsn_aanvrager                                                | 999990020  |
      | dagtekening_aanvraag                                         | 2026-10-12 |
      | datum_afgifte                                                | 2026-10-20 |
      | geneeskundig_onderzoek_achterwege_gelaten                    | true       |
      | bekend_dat_aanvrager_nog_steeds_voldoet                      | false      |
      | keurende_instantie_oordeelt_dat_aanvrager_nog_steeds_voldoet | true       |
    Given the following findings:
      | aanvrager_eerder_gehandicaptenparkeerkaart_verstrekt_volgens_college | true  |
    When I evaluate "gehandicaptenparkeerkaart_mag_worden_afgegeven" of "regeling_gehandicaptenparkeerkaart"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_mag_worden_afgegeven" is true

  Scenario: Zonder grond onder a of b geldt het eerste lid
    # Het college laat het onderzoek achterwege, maar weet niet dat de
    # aanvrager nog steeds voldoet, en de keurende instantie oordeelt er niet
    # over. Het onderzoek moet dan plaatsvinden, ook al kreeg de aanvrager
    # eerder een kaart; de engine vraagt daar dan niet meer naar.
    Given the following parameters:
      | bsn_aanvrager                                                | 999990020  |
      | dagtekening_aanvraag                                         | 2026-10-12 |
      | datum_afgifte                                                | 2026-10-20 |
      | geneeskundig_onderzoek_achterwege_gelaten                    | true       |
      | bekend_dat_aanvrager_nog_steeds_voldoet                      | false      |
      | keurende_instantie_oordeelt_dat_aanvrager_nog_steeds_voldoet | false      |
    Given the following findings:
      | geneeskundig_onderzoek_heeft_plaatsgehad_volgens_college             | true  |
    When I evaluate "gehandicaptenparkeerkaart_mag_worden_afgegeven" of "regeling_gehandicaptenparkeerkaart"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_mag_worden_afgegeven" is true

  Scenario: Wie het onderzoek niet achterwege laat, krijgt zonder onderzoek geen kaart, ook bij een eerdere kaart
    Given the following parameters:
      | bsn_aanvrager                                                | 999990020  |
      | dagtekening_aanvraag                                         | 2026-10-12 |
      | datum_afgifte                                                | 2026-10-20 |
      | geneeskundig_onderzoek_achterwege_gelaten                    | false      |
      | bekend_dat_aanvrager_nog_steeds_voldoet                      | true       |
      | keurende_instantie_oordeelt_dat_aanvrager_nog_steeds_voldoet | true       |
    When I evaluate "gehandicaptenparkeerkaart_mag_worden_afgegeven" of "regeling_gehandicaptenparkeerkaart"
    Then the execution succeeds
    Then output "gehandicaptenparkeerkaart_mag_worden_afgegeven" is false
