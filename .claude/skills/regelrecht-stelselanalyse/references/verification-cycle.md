# Verificatie-cyclus — tracker, bronnen, status

Hoe je claims (eigen bevindingen én menselijke reviewer-comments) toetst tegen externe
bronnen en de status traceerbaar vastlegt.

## Resolutie-tracker

De leidende status-administratie per cyclus (`templates/resolutie-tracker.md`). Per
comment en per wetgevings-fout: scope, status, gevonden bron, en het voorlopige gevolg.

### Status-vocabulaire

| Status | Betekenis |
|---|---|
| `open` | Niets ondernomen deze cyclus |
| `in-progress` | Bron-onderzoek loopt of resolutie wordt geschreven |
| `verifieerd` | Externe bron bevestigt de claim (referentie in bron-kolom) |
| `weerlegd` | Externe bron weerlegt de claim |
| `gedeeltelijk` | Bron resolveert deel van de claim; rest blijft open |
| `onverifieerbaar` | Via beschikbare bronnen geen uitspraak mogelijk; menselijke/dossier-actie nodig |
| `out-of-scope` | Bewust niet behandeld deze cyclus |

### Scope-markering

Markeer elk item met de cyclus-scope (bijv. een korte code per thema), zodat duidelijk
is wat deze ronde wel/niet wordt opgepakt. Items buiten scope blijven `open`/`out-of-scope`
en wachten op een volgende cyclus.

## Bronnen-dossier

Externe bronnen die je ontgint (Staatsbladen, nota's van toelichting, vaststellings-
besluiten, rekenregels, nieuwsberichten van de uitvoerder) krijgen elk een eigen
extract-bestand + een `bronnen-INDEX.md` die comments terugkoppelt aan bronnen.

### Kamerstukken: eerst `law-wetsgeschiedenis`

Gaat de vraag over wat de wetgever met een artikel of lid bedoelde, of waar een tekst
vandaan komt, haal dan het Kamerdossier op met `law-wetsgeschiedenis` in plaats van de
stukken los te ontginnen. Die geeft de toelichting per artikel en lid en de herkomst per
regel, elk met het Kamerstuk en zijn adres, klaar voor het bronnen-dossier. Twee dingen
neem je uit het antwoord mee de tracker in:

- de regel *Controle*: wijkt de afgespeelde tekst af van de officiële, citeer dan de
  officiële tekst en niet de afspeling;
- de passages onder *Niet gekoppeld*: die noemen het artikel zonder eraan te hangen, en
  zijn vaak juist de algemene uitleg waar het artikelsgewijze deel naar verwijst.

### WebFetch-ontginning

- Haal officiële publicaties op via WebFetch (Staatsblad-HTML, wetten.overheid.nl,
  uitvoerder-publicaties). Voor PDF's: lokaal extraheren (bijv. `pdftotext`) als WebFetch
  de inhoud niet geeft.
- Per bron: noteer wat het is, de exacte vindplaats, en de **kern** die het oplost of
  onderbouwt (één regel). Zie het INDEX-sjabloon.
- Onderscheid wat Claude wél kan ophalen van wat **alleen mens/dossier** kan leveren
  (interne mandaat-stukken, niet-publieke nota's) → die worden `onverifieerbaar`.

## Comment-resolutie

Menselijke reviewers laten vaak inline `COMMENT`-blokken achter in de fouten-analyse die
een claim nuanceren of weerleggen. Verwerk die expliciet:
1. Verzamel alle comments en koppel ze aan de betreffende fout/sectie.
2. Geef elk een scope + status in de tracker.
3. Zoek de onderbouwende bron.
4. Werk de fouten-analyse bij (claim afzwakken/intrekken bij weerlegging — zichtbaar,
   zie heroverweging in `review-orchestration.md`).

## Koppeling met de classificatie

Verificatie kan een bevinding van categorie doen wisselen: een vermeende wetgevings-fout
die door een recent wijzigingsbesluit al is gerepareerd, wordt `weerlegd`/`verouderd` en
verdwijnt uit de wetgevings-analyse. Houd de vier-weg-classificatie en de tracker
synchroon.
