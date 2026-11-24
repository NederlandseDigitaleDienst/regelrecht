# Individuele inkomenstoeslag Groningen in RegelHet

Een tweede casus voor RegelHet, naast de gehandicaptenparkeerkaart: de
individuele inkomenstoeslag (artikel 36 Participatiewet) van de gemeente Groningen. De burger
vraagt de toeslag aan. De behandelaar legt de gegevens van de aanvrager vast. De beslisser
beoordeelt de aanvraag en neemt het besluit. Bezwaar en beroep lopen via de Awb, zoals bij de
kaart.

Dit is geen geldend recht en geen advies. De verordening en de beleidsregels van Groningen zijn
echt (dossier `dossiers/iit-groningen`). De rest is een eigen keuze in de modellering:
- de Participatiewet is een uittreksel per 2026-01-01;
- het uitvoeringsbeleid en de cel zijn fictief;
- de testprofielen zijn verzonnen.

## Starten

Bouw eerst de frontend, één keer, dezelfde als voor de parkeerkaart:

```bash
npm ci
npm run build -w frontend-regelhet
just regelhet-iit
```

Daarna draait de instantie op <http://localhost:7181>. De parkeerkaart kan tegelijk draaien op
7180 (`just regelhet`). In test mode staan de kronieken in het geheugen, dus na een herstart
is alles leeg. De testklok begint op 13 oktober 2026.

Burger en medewerker delen lokaal één inlogsessie. Open daarom de medewerkerskant
(`/backoffice`) in een privévenster of in een andere browser.

## Testprofielen

De burgerservicenummers komen uit de testreeks en horen bij niemand. De keuzelijst van de
gesimuleerde DigiD heeft er maar twee: Anna Voorbeeld en Bram Proef. Voor deze profielen vul je
op die pagina zelf de naam en het BSN in. Alle gegevens staan in
[testprofielen.json](testprofielen.json), in eurocent.

| BSN | Naam | Situatie | Verwacht |
|---|---|---|---|
| 999990044 | Henk Hoogeveen | Alleenstaand, al jaren bijstand tot de norm, volledig arbeidsongeschikt | Toegekend, € 545 |
| 999990056 | Sanne Meijer | Gehuwd, bijstand tot de gehuwdennorm, aangewezen op een participatieplaats | Toegekend, € 777 |
| 999990068 | Fatma Yilmaz | Alleenstaande ouder, bijstand, twaalf maanden ontheffing van de arbeidsverplichtingen | Toegekend, € 697 |
| 999990081 | Kees Dijkstra | Medisch urenbeperkt met vrijlating van inkomsten. In maart en april 2026 € 1.800 inkomen, geen bijstand: artikel 3 zegt nee, artikel 3a ja | Toegekend, € 545 |
| 999990093 | Lotte Bakker | 24 jaar, bijbaan van € 900, studeert met studiefinanciering | Afgewezen (beleidsregels art. 3) |
| 999990111 | Piet de Boer | Bijstand, maar geen grond uit de beleidsregels | De beslisser oordeelt zelf (beleidsregels art. 2) |
| 999990123 | Ruud Visser | Arbeidsongeschikt, maar maatregel op 1 maart 2026 | Afgewezen (beleidsregels art. 5) |
| 999990135 | Mieke Smit | Arbeidsongeschikt, maar € 9.000 vermogen | Afgewezen (vermogensgrens € 8.000) |
| 999990147 | Jan Mulder | Studeert én is arbeidsongeschikt | Geen uitkomst (W9), dus niet verleend |

Wat de behandelaar vastlegt, als het niet anders in de tabel staat:
- geboortedatum 2 maart 1975;
- niet de pensioengerechtigde leeftijd;
- alleenstaande;
- vermogen € 2.500;
- inkomen € 0 per maand;
- "Ontving algemene bijstand tot de norm" aan;
- geen afwijkende maanden;
- verder alles uit of leeg.

De afwijkingen per profiel:

| Profiel | Vul anders in |
|---|---|
| Henk | Volledig arbeidsongeschikt aan |
| Sanne | Leefvorm `gehuwden`, geboren 15-06-1982, vermogen € 4.000, aangewezen op een participatieplaats aan |
| Fatma | Leefvorm `alleenstaande ouder`, geboren 20-11-1988, vermogen € 9.000, ontheffing arbeidsverplichtingen aan, duur 12 |
| Kees | Geboren 30-08-1969, medisch urenbeperkt aan, met vrijlating aan, maanden die afweken: 2026-03-01 en 2026-04-01, elk inkomen € 1.800 en bijstand € 0 |
| Lotte | Geboren 10-05-2002, vermogen € 500, bijstand tot de norm uit, inkomen € 900, studeert met studiefinanciering aan |
| Piet | Geboren 12-01-1979 |
| Ruud | Geboren 04-04-1971, volledig arbeidsongeschikt aan, dag van de laatste maatregel 2026-03-01 |
| Mieke | Geboren 18-09-1966, volledig arbeidsongeschikt aan, vermogen € 9.000 |
| Jan | Geboren 27-02-1990, volledig arbeidsongeschikt aan, studeert met studiefinanciering aan |

Om een demo voor te bereiden, legt dit script de gegevens van een profiel in één keer vast,
zoals de behandelaar dat zou doen:

```bash
packages/regelhet/iit-groningen/leg-testprofiel-vast.sh 999990044
```

## Demoroute

1. Admin, tabblad Simulatie: maak de testomgeving leeg.
2. Burger: kies op de gesimuleerde DigiD "zelf invullen", met naam en BSN van een profiel.
   Klik "Nieuwe aanvraag", vul het adres in en dien de aanvraag in.
3. Behandelaar, tabblad Werkvoorraad: klik "Gegevens voor iit vastgesteld vastleggen" en vul de
   gegevens van het profiel in.
4. Beslisser, tabblad Werkvoorraad: open de aanvraag. De voorvertoning zegt of de toeslag wordt
   verleend en hoe hoog die is. Bij Piet beantwoord je eerst "Oordeel van het college over
   uitzicht en inspanningen". Klik daarna "Besluit nemen".
5. Burger, Mijn dossier: het besluit, met het bedrag en de bezwaartermijn.
6. De twaalf maanden van artikel 36, derde lid: zet als admin de testklok een week vooruit en
   laat hetzelfde profiel opnieuw aanvragen. De voorvertoning wijst nu af.

## Hoe het in elkaar zit

| Bestand | Wat |
|---|---|
| [bron.yaml](bron.yaml) | het manifest: welke regelwerken en cellen meedraaien |
| [casussen/individuele_inkomenstoeslag.yaml](casussen/individuele_inkomenstoeslag.yaml) | de casus: cel, beslissend artikel, uitkomst, testklok |
| [ui_spec.yaml](ui_spec.yaml) | hoe de schermen de velden tonen |
| `corpus/regulation/nl/wet/participatiewet/` | uittreksel artikel 8, 21, 34 en 36 per 2026-01-01, met artikel 36 als aanvraag en besluit |
| `corpus/regulation/nl/uitvoeringsbeleid/beleid_iit_gemeente_groningen/` | fictief: welke gegevens de behandelaar vastlegt (art. 1), de 48 maanden en de normen per half jaar (art. 2), en hoe het college de verordening en de beleidsregels toepast (art. 3) |
| `corpus/cells/gemeente_groningen/` | de cel, met twee reducties: de vastgelegde gegevens, en de besluiten voor de twaalf maanden van lid 3 |
| `corpus/regulation/nl/{gemeentelijke_verordening,beleidsregel}/groningen/` | symbolische links naar de versies in het corpus. Het manifest kent alleen mappen, en de mappen in het corpus bevatten ook `.enrichment-result.yaml`, dat de lader als regelwerk probeert te lezen. |

## Bekende beperkingen

- **Twee vastleggingen op één dag.** Leg je de gegevens van dezelfde aanvrager twee keer op
  dezelfde dag vast, dan faalt de beoordeling: de reductie kan dan niet kiezen welke de laatste
  is. Maak de testomgeving leeg, of zet de klok een dag vooruit. De echte oplossing is een feit
  dat naar de aanvraag verwijst.
- **Niet ambtshalve.** Groningen kent ambtshalve toe (beleidsregels art. 5a), maar het
  RegelHet kent geen ambtshalve route (dossier, vertaling-aanvraagportaal.md, B1).
- **Twaalf maanden zonder bezwaar.** De reductie over verleende toeslagen telt alleen het besluit
  zelf. Een herroeping op bezwaar of een vernietiging door de rechter telt nog niet.
- **Normen tot 2026.** Het beleid kent de normen van juli 2022 tot en met juli 2026. Na 2026
  rekent het met die van juli 2026.
- **Teksten over de kaart.** Twee teksten in de schermen gaan nog over de kaart:
  `frontend-regelhet/src/burger/routes.ts` ("Hier vraagt u de kaart aan") en het beheerpaneel.
- **Leefvorm als tekst.** De behandelaar typt de leefvorm. Een tikfout geeft de norm en grens van
  een alleenstaande.
