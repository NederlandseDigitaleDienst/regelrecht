# Berichten: het template

Een bericht aan de burger volgt uit een **vastlegging** in de kroniek: een besluit, een indiening of een feit. Wat er in het bericht staat, komt uit die vastlegging en uit de wet. De tekst komt uit een register van het bestuursorgaan. Nergens staat een zin los van de gegevens: zo kan een besluit niet "positief beoordeeld" en "€ 0,00" zeggen terwijl de bijlage op een ander bedrag uitkomt.

Drie bestanden, elk met één taak:

| bestand | wat | van wie |
|---|---|---|
| `ui_spec.yaml` | het formulier: welke velden, wie ze vult, wat de burger ziet | het bestuursorgaan, per proces |
| `berichten.yaml` | **de berichtenspec**: welk bericht uit welke vastlegging volgt, en hoe het uit die vastlegging wordt opgebouwd | het bestuursorgaan, per proces |
| `berichtteksten.yaml` | **de tekstlaag**: de standaardteksten van de berichten, met sleutels | het bestuursorgaan (een register) |

Het manifest (`bron.yaml`) noemt de twee laatste onder `berichten` en `berichtteksten`. De server leest ze met `src/berichten.rs` van de poc-crate en geeft ze aan de schermen onder `berichten` in `GET /api/formulier`. Eén component toont elk bericht: `frontend-regelhet/src/gedeeld/Bericht.vue`, met de logica in `gedeeld/bericht.ts`.

## Het template

Elk bericht heeft dezelfde vorm: een berichtsoort met een binding, een wettelijk skelet van delen, een tekstlaag met sleutels, een status en een kanaal, en invarianten.

### 1. De berichtsoort en haar binding aan de vastlegging

```yaml
berichtsoorten:
  - id: beschikking
    naam: beschikking
    patroon: beschikking            # het wettelijke skelet, zie 2
    vastlegging:
      rol: besluit                  # besluit | indiening | vastlegging
      artikel: 'regelwerk#49'       # het artikel van de vastlegging
      feit: 'regelwerk#12#feit'     # alleen bij rol: vastlegging
      rechtskarakter: BESCHIKKING   # wat het artikel voortbrengt (produces)
      uitkomst: kaart_verstrekt     # de uitkomst die het besluit draagt
      stap: besluit                 # de stap in de kroniek
    sjabloon: beschikking_kaart     # het sjabloon in de tekstlaag, zie 3
```

Het scherm zoekt bij een vastlegging de berichtsoort met dezelfde rol en hetzelfde artikel, en bij een feit hetzelfde feit. Volgen uit de besluiten van één artikel twee berichtsoorten, zoals een buitenbehandelingstelling naast de beschikking, dan zegt `vastlegging.als` onder welke voorwaarde op de vastlegging (een `uitkomst` of `veld` met een toets) deze berichtsoort geldt; de eerste die past, telt. Een besluit zonder berichtsoort krijgt de delen die de Awb van elk besluit vraagt, gevuld uit de wet; een andere vastlegging zonder berichtsoort krijgt geen bericht.

### 2. De delen: het wettelijke skelet

Een patroon zegt welke delen de wet eist. De server weigert een berichtsoort waarin zo'n deel ontbreekt.

| patroon | delen die de wet eist |
|---|---|
| `beschikking` | dictum (Awb 1:3), grondslag (3:47), motivering (3:46), ondertekening (10:10), rechtsmiddelen (3:45) |
| `beslissing_op_bezwaar` | dictum (7:11), motivering (7:12), ondertekening (10:10), rechtsmiddelen (3:45) |
| `buitenbehandelingstelling` | dictum (4:5), motivering (3:46), ondertekening (10:10), rechtsmiddelen (3:45) |
| `ontvangstbevestiging` | dictum (2:17) |
| `herstelverzoek` | dictum (4:5), specificatie (4:5): wat ontbreekt en tot wanneer |
| `mededeling` | geen |

Per deel:

```yaml
    delen:
      - id: dictum
        plek: dictum               # de plek, en daarmee de volgorde
        vereist_door: 'Awb 1:3'    # het artikel dat het deel eist
        bron: {tekst: [Ons besluit]}
        als: toekenning            # alleen als deze sleutel waar is
        nldd: nldd-box             # het element van het design system
        titel: Uw kaart            # een kop, als het sjabloon er geen geeft
```

- **`plek`** is een van `kop`, `inleiding`, `dictum`, `grondslag`, `motivering`, `onderbouwing`, `specificatie`, `mededeling`, `ondertekening`, `rechtsmiddelen`, `bijlage`. De plek bepaalt de **volgorde**: een bericht toont zijn delen altijd in deze volgorde, in welke volgorde de spec ze ook noemt.
- **`bron`** zegt waar de inhoud vandaan komt; meer dan één mag:
  - `tekst`: alinea's van het sjabloon, op kop (`''` voor de alinea's zonder kop) of op positie (een getal);
  - `regels`: velden van de vastlegging of van het stuk waarop zij antwoordt, met hun waarde;
  - `tabel`: een specificatie, met `rijen` (sleutels met een bedrag) en een `totaal`;
  - `bijlage`: een tabel in kolommen, zie hieronder;
  - `wet`: wat de wet geeft: `grondslag` (het artikel van het besluit), `keten` (de artikelen die het besluit toepaste), `rechtsmiddelen` (wat de wet tegen het besluit toelaat, met de laatste dag) of `ondertekening` (de afzender, onder mandaat met "namens deze", Awb 10:10).
- **`nldd`** is een van `nldd-rich-text`, `nldd-box`, `nldd-table`, `nldd-list`, `nldd-identity`, `nldd-text`. Een element dat het scherm niet kent, weigert de server.

Een deel dat de wet eist en leeg blijft, staat in het bericht als **"Dit onderdeel ontbreekt"**, met het artikel erbij.

**Een bijlage in kolommen.** Een deel met `bron.bijlage` toont rijen in kolommen, zoals een berekening per post. De rijen komen uit het model: een sleutel waarvan de waarde rijen zijn, of rijen die de spec uit sleutels bouwt (`velden`, eventueel onder `als`). Elke kop en elke cel komt uit de tekstlaag (`bijlagen.<labels>`), met de velden van een rij tussen accolades; de spec noemt alleen velden en tekstsleutels.

```yaml
      - id: berekening
        plek: bijlage
        bron:
          bijlage:
            labels: per_post          # de teksten in de tekstlaag, onder bijlagen
            rijen: posten             # een sleutel met de rijen uit het model
            groep: soort              # een blok per waarde van dit veld, met de tekst 'groep' als kop
            bedrag: bedrag            # het bedrag van een rij; zonder dit veld geen sommen
            bedragen: [per_eenheid, bedrag]   # velden die in eurocent staan
            kolommen:
              - {kop: kop_post, tekst: cel_post}
              - {kop: kop_aantal, tekst: cel_aantal, onder: cel_deel, onder_als: {veld: deel, groter_dan: 1}}
            totaal: modeltotaal       # de sleutel waarvan de waarde het totaal is
```

Een bijlage rekent niet. Elk bedrag per rij komt uit het model; de som per blok is de som van zijn rijen (met de tekst `samen`), en het totaal is de waarde van `totaal`. Wat moet gelden is een invariant: de rijen maken samen het totaal, en met `bedrag_is_totaal` op de bijlage is het totaal het bedrag van het besluit. Een bijlage staat in de brief na de ondertekening, en geprint op een eigen blad.

### 3. De tekstlaag en de sleutels

In de berichtenspec staat geen zin. De tekst staat in de tekstlaag, een register van het bestuursorgaan:

```yaml
afzender:
  organisatie: Het college van burgemeester en wethouders van de gemeente Wilgendaal
  in_mandaat: false
  namens: namens deze,          # bij mandaat (Awb 10:10)
  rollen: {beslisser: het college van burgemeester en wethouders}
zinnen:
  bezwaarzin: ', dus uiterlijk op {bezwaardag}'
sjablonen:
  - id: beschikking_kaart
    ondertekenaar: beslisser
    betreft_bij: {toekenning: U krijgt een kaart, afwijzing: U krijgt geen kaart}
    aanhef: Geachte heer of mevrouw,
    alineas:
      - tekst: Op {aanvraagdag} vroeg u een kaart aan.
      - {kop: Ons besluit, als: toekenning, tekst: 'Wij verstrekken u een kaart, geldig tot en met {geldig_tot}.'}
    slot: Met vriendelijke groet,
```

Een sleutel (`{aanvraagdag}`) of voorwaarde (`als: toekenning`) in de tekst krijgt in de berichtenspec zijn bron. Een sleutel zonder toets mag ook als voorwaarde dienen: de alinea staat dan als hij een waarde heeft.

```yaml
    sleutels:
      toekenning: {uitkomst: kaart_verstrekt, is: true}      # een voorwaarde
      aanvraagdag: {veld: dagtekening, vorm: datum}          # een waarde
      bedrag: {uitkomst: vastgesteld_bedrag, vorm: bedrag}
      totaal: {keten: subsidiebedrag_totaal}                 # een uitkomst uit de keten van het besluit
      beslisdag: {indiening: redelijke_beslistermijn_einddatum, vorm: datum}
      bezwaarzin: {zin: bezwaarzin}                          # een zin uit de tekstlaag
```

De bronnen: `uitkomst` (van de vastlegging), `veld` (van de vastlegging of het stuk waarop zij antwoordt, het dichtstbijzijnde eerst; met `eigen: true` alleen van de vastlegging zelf, zoals de motivering van een beslissing op bezwaar, die niet die van het bestreden besluit is), `keten` (een uitkomst in de keten van het besluit, uit `waarom`), `indiening` (wat een indiening onder haar eigen artikel geeft), `record` (`recorded_on`, `id`, `kenmerk`), `label` (de naam van een veld in de UI-beschrijving), `afzender` (uit de tekstlaag) en `zin` (een zin uit de tekstlaag, die alleen staat als al haar sleutels een waarde hebben). Met een toets (`is`, `gelijk`, `groter_dan`, `gevuld`, `niet_gevuld`) wordt een sleutel een voorwaarde; `alle`, `een_van` en `niet` combineren voorwaarden. `vorm` zegt hoe een waarde wordt geschreven: `bedrag` (eurocent als euro), `datum`, `getal` of `tekst`.

**Het briefhoofd.** De tekstlaag geeft onder `briefhoofd` de tekst in het lint bovenaan (`lint`, anders de naam van de afzender), het `retouradres` boven het adresblok en de regels van de `voettekst`. Het lint is een neutrale balk met tekst: het design system levert geen beeldmerk voor een andere afzender dan de Rijksoverheid, en een logo van een echte organisatie komt niet in deze repository. Rechts staat de kenmerkkolom: datum, onze referentie, uw brief van (de dag van het stuk waarop het bericht antwoordt), betreft, het kanaal en de bijlage. Geprint is het bericht een brief op papier van 210 bij 297 mm met "Blad x van y" onderaan elk blad; een concept draagt een stempel.

### 4. Status en kanaal

```yaml
    status:
      bekendgemaakt: {veld: bekendmaking_datum, gevuld: true}
    kanaal: {vast: portaal}
    # of: kanaal: {sleutel: kanaal, waarden: {portaal: portaal, papier: papier}}
```

- **Status**: *concept* (een voorvertoning: niets vastgelegd), *vastgesteld* (de vastlegging staat in de kroniek), *bekendgemaakt* (de voorwaarde onder `status.bekendgemaakt` geldt).
- **Kanaal**: `email` (Awb 2:8), `portaal` met een kennisgeving (2:8 en 2:10) of `papier` (3:41). Een kanaal dat uit een sleutel moet komen en daar niet uit volgt, is een schending.

### 5. Invarianten

Wat tussen de delen moet gelden. Een schending is **zichtbaar, nooit stil**: de medewerker ziet haar boven het concept, de burger een melding dat het bericht niet in orde is. Een gat in de spec zelf, zoals een alinea die geen deel opneemt, ziet alleen de medewerker: het is geen fout van dit bericht.

Altijd, zonder dat de spec ze noemt:

- **elk deel dat de wet eist, is gevuld** of staat zichtbaar als "ontbreekt";
- **geen sleutel zonder bron**: een sleutel of voorwaarde die de spec niet bindt, weigert de server al bij het laden; komt hij toch voor, dan staat hij als `[sleutel: geen bron]` in het bericht;
- **geen sleutel zonder waarde**: een sleutel waarvoor de vastlegging geen waarde heeft, staat als `[sleutel ontbreekt]` in het bericht;
- **elke alinea van het sjabloon heeft een plek**: een alinea die geen deel opneemt, meldt de server bij de berichtsoort;
- **het rechtskarakter klopt**: een besluit is van het rechtskarakter dat de binding noemt.

Op te nemen per berichtsoort:

```yaml
    invarianten:
      # Het bedrag in het dictum is het totaal van de specificatie, en dat
      # totaal is de som van haar regels.
      - {soort: bedrag_is_totaal, bedrag: bedrag, specificatie: specificatie}
      # Precies één oordeel geldt, en het past bij de uitkomst.
      - soort: oordeel_past
        oordelen:
          toekenning: {sleutel: bedrag, groter_dan: 0}
          afwijzing: {een_van: [{sleutel: bedrag, gelijk: 0}, {sleutel: bedrag, niet_gevuld: true}]}
```

`bedrag_is_totaal` vangt een besluit van € 0,00 met een bijlage van € 322,72. `oordeel_past` vangt "positief beoordeeld" bij € 0,00, en een bericht dat twee oordelen tegelijk of geen enkel oordeel geeft.

### 6. Verzenden en doorsturen

Een bericht dat uitgaat is een **vastlegging**, geen weergave die steeds opnieuw wordt opgebouwd. De spec zegt onder `verzending` welk feit dat is:

```yaml
verzending:
  feit: 'beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#13#bericht_verzonden'
  doorsturen: 'beleid_gehandicaptenparkeerkaart_gemeente_wilgendaal#13#bericht_doorgestuurd'
  velden:
    bsn_aanvrager: {veld: bsn_aanvrager}   # over wie het gaat: zo staat het in haar dossier
```

- **Verzenden** legt het feit vast in de kroniek van de afzender, met de berichtsoort, de vastlegging waar het bij hoort, het kanaal, de dag en de **bevroren inhoud**: het bericht zoals de burger het leest, als tekst, met een SHA-256-hash ervan (`verzendvelden` en `bevries` in `gedeeld/bericht.ts`). Een latere wijziging van de spec, de tekstlaag of de vastlegging verandert een verzonden bericht niet: wie het verzonden bericht toont, leest het uit het feit en bouwt het niet opnieuw op. Een nieuw bericht vervangt het vorige met een verwijzing (`vervangt`); niets wordt overschreven.
- De **dag van verzending** is de dag van bekendmaking (Awb 3:41): een feit, geen invoerveld. De burger ziet het bericht onder "Mijn berichten" in haar dossier, uit dat feit; de medewerker ziet boven het besluit de verzonden versie, met de dag en de hash.
- **Doorsturen** is een feit van hetzelfde artikel, met dezelfde parameters als de ontvangst bij de andere cel (zoals de rechtbank). De kern levert het af zoals elk feit tussen cellen: de ontvanger legt de ontvangst in haar eigen kroniek vast. Van twee feiten met dezelfde parameters is het eerstgenoemde in `requires` het verzonden feit en het latere de ontvangst; zo legt de cel van de zaak een feit vast dat zij doorstuurt, en nooit de ontvangst ervan.

## Een nieuwe berichtsoort toevoegen

1. **Zoek de vastlegging.** Welk besluit, welke indiening of welk feit staat in de kroniek als het bericht moet uitgaan? Heeft de wet er geen feit voor, dan hoort dat feit eerst in het regelwerk (zoals artikel 12 van het fictieve beleid voor het herstelverzoek van Wilgendaal).
2. **Kies het patroon.** Is het een besluit (`beschikking`, `beslissing_op_bezwaar`, `buitenbehandelingstelling`), een ontvangstbevestiging, een herstelverzoek of een andere mededeling? Het patroon zegt welke delen er moeten zijn.
3. **Schrijf of kies het sjabloon** in de tekstlaag: alinea's met een kop, sleutels tussen accolades, voorwaarden onder `als`. Is de tekstlaag een register van een ander, dan blijft zij zoals zij is; een ontbrekende zin komt dan als voorstel terug bij de eigenaar.
4. **Bind elke sleutel en voorwaarde** in `sleutels` aan een bron. Een sleutel zonder bron weigert de server.
5. **Leg de delen op hun plek**: elke alinea van het sjabloon in een deel, elk deel dat het patroon eist met `vereist_door`, en per deel het NLDD-element.
6. **Neem de invarianten op** die voor dit bericht gelden: een bedrag en zijn specificatie, een oordeel en de uitkomst.
7. **Bewijs het.** Een integratietest laat de vastlegging ontstaan en toetst dat zij precies één berichtsoort heeft en dat elke sleutel een waarde vindt (zie `tests/berichten.rs`); de schermtoets (`gedeeld/bericht.test.ts`) toetst de opbouw en de invarianten, aan beide kanten: een bericht dat klopt, en een dat een schending geeft.

## De parkeerkaart van Wilgendaal

`berichten.yaml` heeft vier berichtsoorten op dit template: de **beschikking** (toekenning en afwijzing op één template, met de kaart als specificatie en de gegevens op de kaart als kleine bijlage in kolommen), de **ontvangstbevestiging** van de aanvraag, het **herstelverzoek** (Awb 4:5, uit artikel 12 van het fictieve beleid) en de **beslissing op bezwaar** (niet-ontvankelijk, ongegrond en gegrond op één template). De teksten staan in `berichtteksten.yaml`, met het briefhoofd van de gemeente. Verzenden en doorsturen gaan langs artikel 13 van het fictieve beleid: het college legt het verzonden bericht vast, en de rechtbank de ontvangst van een doorgestuurd bericht.
