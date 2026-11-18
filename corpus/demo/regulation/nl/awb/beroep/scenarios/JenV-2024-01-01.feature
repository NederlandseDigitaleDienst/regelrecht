Feature: Algemene wet bestuursrecht - Beroepmogelijkheid en Termijnen
  Als belanghebbende
  Wil ik weten of ik beroep kan instellen tegen een besluit
  Zodat ik weet welke termijn en rechter daarbij horen

  Background:
    Given the calculation date is "2024-06-01"

  Scenario: Beroep mogelijk na afwijzende beslissing op bezwaar
    Given parameter "bsn" is "999700001"
    And the following "JenV" data with key "bsn" for law "awb/beroep":
      | bsn         | zaak                                                | wet                                                                                                                                                                             | adres | jurisdictie                                                                          | gebeurtenissen              |
      | "999700001" | {"id":"zaak-1","status":"DECIDED","approved":false} | {"decision_type":"TOEKENNING","legal_character":"BESCHIKKING","voorbereidingsprocedure":"REGULIER","beroepstermijn_weken":null,"name":"participatiewet","competent_court":null} | null  | {"gemeente":"Den Haag","arrondissement":"Den Haag","rechtbank":"RECHTBANK_DEN_HAAG"} | [{"event_type":"Objected"}] |
    When I evaluate outputs "beroep_mogelijk, reden_niet_mogelijk, beroepstermijn" of "awb/beroep"
    Then output "beroep_mogelijk" is true
    And output "reden_niet_mogelijk" is absent
    # art. 6:7 Awb: geen beroepstermijn_weken bij de wet, dus de wettelijke standaardtermijn
    And output "beroepstermijn" equals 6

  Scenario: Geen beroep mogelijk zonder afwijzende beslissing op bezwaar
    Given parameter "bsn" is "999700002"
    And the following "JenV" data with key "bsn" for law "awb/beroep":
      | bsn         | zaak                                                 | wet                                                                                                                                                                             | adres | jurisdictie                                                                          | gebeurtenissen              |
      | "999700002" | {"id":"zaak-2","status":"IN_REVIEW","approved":null} | {"decision_type":"TOEKENNING","legal_character":"BESCHIKKING","voorbereidingsprocedure":"REGULIER","beroepstermijn_weken":null,"name":"participatiewet","competent_court":null} | null  | {"gemeente":"Den Haag","arrondissement":"Den Haag","rechtbank":"RECHTBANK_DEN_HAAG"} | [{"event_type":"Objected"}] |
    When I evaluate outputs "beroep_mogelijk, reden_niet_mogelijk" of "awb/beroep"
    Then output "beroep_mogelijk" is false
    And output "reden_niet_mogelijk" equals "er is nog geen afwijzende beslissing op bezwaar"

  Scenario: Direct beroep bij uitgebreide voorbereidingsprocedure
    Given parameter "bsn" is "999700003"
    And the following "JenV" data with key "bsn" for law "awb/beroep":
      | bsn         | zaak                                                 | wet                                                                                                                                                                               | adres | jurisdictie                                                                          | gebeurtenissen |
      | "999700003" | {"id":"zaak-3","status":"IN_REVIEW","approved":null} | {"decision_type":"TOEKENNING","legal_character":"BESCHIKKING","voorbereidingsprocedure":"UITGEBREID","beroepstermijn_weken":null,"name":"participatiewet","competent_court":null} | null  | {"gemeente":"Den Haag","arrondissement":"Den Haag","rechtbank":"RECHTBANK_DEN_HAAG"} | []             |
    When I evaluate outputs "beroep_mogelijk, direct_beroep, reden_direct_beroep" of "awb/beroep"
    Then output "beroep_mogelijk" is true
    And output "direct_beroep" is true
    And output "reden_direct_beroep" equals "besluit is voorbereid met uitgebreide procedure"

  Scenario: Vreemdelingenzaak loopt via de rechtbank als categorie, niet als vaste naam
    Given parameter "bsn" is "999700004"
    And the following "JenV" data with key "bsn" for law "awb/beroep":
      | bsn         | zaak                                                | wet                                                                                                                                                                              | adres                  | jurisdictie                                                                                         | gebeurtenissen              |
      | "999700004" | {"id":"zaak-4","status":"DECIDED","approved":false} | {"decision_type":"TOEKENNING","legal_character":"BESCHIKKING","voorbereidingsprocedure":"REGULIER","beroepstermijn_weken":null,"name":"vreemdelingenwet","competent_court":null} | {"gemeente":"Utrecht"} | {"gemeente":"Utrecht","arrondissement":"Midden-Nederland","rechtbank":"RECHTBANK_MIDDEN_NEDERLAND"} | [{"event_type":"Objected"}] |
    When I evaluate outputs "type_rechter, bevoegde_rechtbank" of "awb/beroep"
    # art. 8:6 Awb: type_rechter is de categorie, geen concrete rechtbanknaam (audit "Awb beroep JenV")
    Then output "type_rechter" equals "RECHTBANK"
    # art. 8:7 lid 1 Awb: de concrete rechtbank volgt uit de jurisdictie van het adres
    And output "bevoegde_rechtbank" equals "RECHTBANK_MIDDEN_NEDERLAND"
