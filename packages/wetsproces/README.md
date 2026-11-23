# Wetsproces

Een proefopstelling van een wetsprocessysteem: een burger vraagt een gehandicaptenparkeerkaart aan bij de fictieve gemeente Wilgendaal, en elke stap daarna volgt uit de wet en het beleid. Alles draait op één uitvoerder, de cel van de gemeente, met de GGD en de rechtbank als externe partijen.

- Burgerkant: <https://wetsproces-burgers-fieldlab-regel-k4c.rig.prd1.gn2.quattro.rijksapps.nl>
- Medewerkerskant: <https://wetsproces-medewerkers-fieldlab-regel-k4c.rig.prd1.gn2.quattro.rijksapps.nl>

Beide draaien in test mode met verzonnen personen. Pull request #2051 is de werkbasis.

## Wat er is

| Kant | Tabbladen | Wie |
|---|---|---|
| Burger | geen; het dossier met "Nieuwe aanvraag" in de balk | een testpersoon, via de Gesimuleerde DigiD |
| Medewerker | Werkvoorraad, Kroniek | het testaccount van regelrecht |
| Admin | Beheerpaneel, Simulatie | een account met de rol `wetsproces-admin`; het testaccount van regelrecht heeft die |

De bouwstenen:

- **Eén uitvoerder.** De gemeente beslist; de GGD en de rechtbank zijn cellen met een eigen kroniek.
- **Beheerpaneel voor externe partijen.** De admin legt namens de GGD of de rechtbank vast wat die doen, zoals een advies of een uitspraak.
- **Schermen uit de ui_spec.** Welke velden er zijn, zegt de wet; `ui_spec.yaml` zegt hun volgorde, groep, naam en hulptekst, en welke het systeem invult.
- **Beleid stuurt stappen.** Bij een aanvraag vraagt het systeem zelf de GGD om onderzoek, en na een toekenning bestelt het zelf de kaart.
- **De materiële kant.** Na de toekenning volgen bestellen, ontvangen en uitreiken, en een kwijtgeraakte kaart krijgt een duplicaat.
- **Een tijdlijn per aanvraag.** De burger ziet haar onder elk stuk in Mijn dossier, de medewerker onderaan het behandelscherm. Ze toont wat er bij de aanvraag gebeurde, op volgorde, en daaronder de volgende stap met wie aan zet is. Elke regel opent het stuk zelf.

Elke beslissing toont vooraf een voorvertoning, met de volledige keten om uit te klappen. Na een vernietiging beslist de medewerker opnieuw.

## Loop de hele flow door

Gebruik twee browservensters: één voor de burger, één voor de medewerkerskant. Aan de medewerkerskant doet het testaccount van regelrecht het werk van de medewerker en van de admin; de inloggegevens krijgt u van de begeleider. U logt in via de toegangsmuur en Keycloak, en de app gaat daarna zelf door, zonder knop.

### Route 1: aanvraag tot uitgereikte kaart

Beginnen:

1. Admin, tabblad Simulatie: klik "Testomgeving leegmaken" en bevestig met "Leegmaken".
2. Burger: kies op de Gesimuleerde DigiD de testpersoon Anna Voorbeeld en klik "Inloggen als deze testpersoon".
3. Burger: klik rechtsboven op "Nieuwe aanvraag". Zet in de sheet "Ik vraag de kaart aan als bestuurder van de auto" aan, vul het adres in en klik "Dien aanvraag in".

Advies en besluit:

4. Medewerker, tabblad Werkvoorraad: open de aanvraag. Onder "Gevraagd aan anderen" staat het verzoek aan de GGD op "Loopt".
5. Admin, tabblad Beheerpaneel: klik onder GGD Wilgendaal op "Verzoek om geneeskundig onderzoek ontvangen". Het BSN staat er al.
6. Admin: vul "Datum onderzoek" in, zet "Langdurige loopbeperking" aan en klik "Vastleggen".
7. Medewerker, Werkvoorraad: open de aanvraag en kies bij "Geneeskundig onderzoek achterwege gelaten" Nee. De voorvertoning zegt "Zou toegewezen worden"; klik "Besluit nemen".

De kaart:

8. Medewerker, Werkvoorraad, onder "Na de toekenning": de kaart is al besteld door het systeem. Open de regel en klik "Vastleggen" voor de ontvangst.
9. Medewerker: open de regel opnieuw en klik "Vastleggen" voor de uitreiking.
10. Burger, tabblad Mijn dossier: onder het besluit staat "Uw kaart is op … aan u uitgereikt."

### Route 2: bezwaar, beroep, vernietiging en nieuw besluit

Begin na stap 7 van route 1.

Bezwaar:

1. Burger, Mijn dossier: open "Besluit (beschikking)" en klik "Bezwaar indienen". Vul het formulier in en klik "Bezwaar indienen".
2. Medewerker, Werkvoorraad: open het bezwaar, kies Ja bij "Indiener bezwaarschrift is belanghebbende" en bij "Indiener bezwaarschrift niet in verzuim", en klik "Besluit nemen".

Beroep:

3. Burger: open de beslissing op bezwaar en klik "Beroep indienen". Vul het formulier in en klik "Beroep indienen".
4. Admin, Beheerpaneel: open onder Rechtbank Wilgendaal het beroep. Kies Nee bij "Rechtsgevolgen in stand" en bij "Uitspraak treedt in de plaats", Ja bij de rest, en klik "Besluit nemen".
5. Admin: klik "Afschrift toezenden".

Opnieuw beslissen:

6. Medewerker, Werkvoorraad: het bezwaar staat er weer met "Opnieuw beslissen". Open het, kies Ja bij beide vragen en klik "Besluit nemen".
7. Burger: open het bezwaar. Onder "Stand" staat het nieuwe besluit met "Geldt"; het eerdere besluit heet "Vernietigd".

### Route 3: kwijtgeraakte kaart

Begin na route 1, met een uitgereikte kaart.

1. Burger, Mijn dossier: klik "Kaart kwijt?" onder de kaart.
2. Burger: vul in "Kaart kwijt of gestolen" de datum en wat er gebeurde in, zet de verklaring aan en klik "Dien aanvraag in".
3. Medewerker, Werkvoorraad: open de nieuwe aanvraag en klik "Besluit nemen". Het systeem bestelt het duplicaat.
4. Medewerker, onder "Na de toekenning": leg de ontvangst vast en daarna de uitreiking.
5. Burger, Mijn dossier: bij de oude kaart staat "Deze kaart geldt niet meer"; "Kaart kwijt?" staat nu alleen bij het duplicaat.

## Lokaal draaien

Vanuit de root van de repository:

```bash
npm ci
npm run build -w frontend-wetsproces
just wetsproces
```

Dat start de server in test mode op <http://localhost:7180>, met beide kanten op één adres en de kronieken in het geheugen. Lokaal is er geen toegangsmuur of Keycloak: u kiest een account uit de lijst van de testlogin. `WETSPROCES_PORT` kiest een andere poort. Met PostgreSQL, en met elke kant apart: zie "Running it locally" in `docs/src/content/docs/components/wetsproces.md`.

## Verder bouwen

| Wat | Waar |
|---|---|
| De casus: cellen, verzoeken, stappen na de toekenning | `packages/wetsproces/casussen/gehandicaptenparkeerkaart.yaml` |
| Hoe de schermen velden tonen | `packages/wetsproces/ui_spec.yaml` |
| Welke regelwerken en cellen meedraaien | `packages/wetsproces/bron.yaml` |
| Het beleid van Wilgendaal | `packages/poc-chronolexografie/corpus/regulation/nl/uitvoeringsbeleid/` |
| Server en schermen | `packages/wetsproces/src/`, `frontend-wetsproces/src/` |

In de casus:

- **Een veld:** een veld is een parameter van het artikel. Zet in `ui_spec.yaml` zijn `label`, `group`, `help` en zo nodig `fill`.
- **Een feit:** een feit legt een artikel van het beleid vast. Een handeling na de toekenning staat onder `na_toekenning` in het casusbestand.
- **Een automatische stap:** zet `door_het_systeem` bij die handeling in `na_toekenning`, of het artikel onder `bij_aanvraag`. Het beleidsartikel heeft één uitvoer die zegt of het systeem handelt. Zie "Steps the system takes itself" in de docs-pagina hierboven.
- **Een tweede proces:** het duplicaat (BABW artikel 52) is het voorbeeld. Het staat als aanvraag onder `na_toekenning.aanvragen` in het casusbestand, met een eigen element in `ui_spec.yaml`. Zie "A lost card" in dezelfde docs-pagina.

Een indiening neemt alleen gegevens over van het besluit waarnaar zij verwijst als zij een veld van het type `decision` heeft (`packages/cel`).

Op fieldlab:

- **Een nieuw image:** een push naar de integratiebranch van #2051 bouwt het. Zet daarna per component `zad deployment update-image fieldlab --component <component> --image ghcr.io/nederlandsedigitaledienst/regelrecht-wetsproces:sha-<tag>` en kijk met `zad logs fieldlab --component <component>` of hij start.
- **Na een image dat de casus wijzigt:** maak fieldlab eerst leeg (Simulatie, "Testomgeving leegmaken"). Een stap van vóór de wijziging kan anders blijven hangen, zoals een bestelling op "wacht op het systeem".
- **Bijwerken met main:** de werkbasis volgt main met `git merge origin/main`.

Draai vóór een merge `just check`, de tests en `npm run typecheck -w frontend-wetsproces`. De schermen staan in `docs/src/content/docs/components/wetsproces-frontend.md`.

## Bekende open punten

Over de kaart:

- Opnieuw beslissen op bezwaar raakt in deze casus de toekenning niet: het bezwaar vraagt alleen of de indiener belanghebbende is en niet in verzuim. Een uitgereikte kaart blijft daardoor geldig.
- Een kaart die na een duplicaat terugkomt, heeft geen gevolg; de oude kaart blijft ongeldig.
- "Kaart kwijt?" verschijnt ook onder een verlopen kaart; de aanvraag wordt dan geweigerd. Gemeld door de bouwer, niet op fieldlab nagelopen.

Over het proces:

- De postkamer heeft geen eigen rol in Keycloak; elke behandelaar kan een ontvangst vastleggen.
- Afschriften aan een andere cel gaan met de hand.
- De grens op het aantal rondes automatische stappen is alleen met unit-tests gedekt.

Over de tijdlijn:

- De volgende stap noemt een termijn met het label van zijn veld ("Laatste dag voor bezwaar of beroep"), niet of het om bezwaar of beroep gaat: beide formulieren lezen hetzelfde veld, dus de kern kan dat niet onderscheiden.
- De termijn voor een nieuw besluit na een uitspraak staat niet als volgende stap in de tijdlijn. Het is een uitvoer van de uitspraak die geen regel als termijn aanmerkt.
- De medewerker ziet het beroep niet in de tijdlijn, wel de uitspraak via de ontvangst van het afschrift. De app voor FSC voor Burgers kent de tijdlijn nog niet.

Over de schermen en de techniek:

- De koppen van de NLDD-pagina's laten de inhoud eronder doorschemeren bij scrollen. Dat is het ontwerp van NLDD en zo gelaten.
- Eigen CSS: geen. Alles is gebouwd met NLDD-componenten.
- Draait alleen op fieldlab, nog niet in de deployment `regelrecht`.

Niet op fieldlab nagelopen: de melding voor een account zonder rol, en een scherm van de chronolexografie-poc met een schakelaar met hulptekst. Beide zijn alleen met tests gedekt.
