---
title: "RegelHet: dienstprofiel FSC voor Burgers"
description: "Welke verzoeken de regelhet-server via FSC voor Burgers aanneemt, wat erin gaat en eruit komt, en wie daarbij de burger is."
lang: nl
---

FSC voor Burgers legt niet vast welke verzoeken een dienst aanneemt. Dat doet het dienstprofiel (§2.2.1 en §2.2.2 van de werkversie). Deze pagina is het dienstprofiel van `regelhetBurgerApi`, de dienst waarmee de [regelhet-server](/components/regelhet) een app laat doen wat een burger in de browser doet: een aanvraag indienen en volgen wat ermee gebeurt. Daarnaast kan een app ophalen wat de uitvoerder al over de burger weet. Hoe een app een sessie opent en hoe het verkeer versleuteld is, staat op [FSC voor Burgers](/components/regelhet-fsc4b).

## De dienst

| | |
|---|---|
| Naam in de discovery-configuratie | `regelhetBurgerApi`, versie `v1` |
| Adres | `GET /fsc4b/discovery` noemt `<publiek adres>/fsc4b/burger` onder versie `v1`; een verzoek gaat naar dat adres met `/v1/` en het pad erachter |
| Sessie | `sessionApi` versie `v1`: `CreateSession` uit §4.3.4 |
| Scope | `nl.regelrecht.regelhet.demo` op het demo-stelsel; voor een echt stelsel is er nog geen |
| Specificatie | OpenAPI 3 op `GET /fsc4b/openapi.json` van dezelfde server |

De OpenAPI-specificatie is de bron voor de precieze vorm van elk verzoek. Deze pagina zegt wat de verzoeken betekenen.

## Verzoeken

Elk verzoek is een `POST` met het sessietoken in de header `Authorization` en als body een JWE onder de sessiesleutel. Het antwoord is een JWE onder dezelfde sleutel, met de HTTP-status ernaast. Alleen een `401` (geen lopende sessie onder het token, of een body die niet ontsleutelt) en een `500` bij het lezen van de sessie zijn gewone JSON. Hieronder staat steeds de ontsleutelde inhoud.

| Verzoek | Pad | Body | Antwoord |
|---|---|---|---|
| Aanvraag indienen | `indieningen` | `{ "velden": {…}, "artikel"?: "…", "op"?: "<id>", "idempotency_key"?: "…" }` | De vastgelegde indiening: `id`, `kind` (`AANVRAAG` voor een aanvraag), de ingevulde `fields` en de regelwerkversies waaronder ze ontstond |
| Dossier inzien | `dossier` | `{}` | `{ "grammen": [ … ], "stand": { … }, "verzoeken": [ … ], "vervolg": [ … ], "door_het_systeem": { … } }`: alles wat over deze burger is vastgelegd, en de stand van elke aanvraag |
| Besluit laten uitleggen | `waarom` | `{ "id": "<besluit>" }` | Waarom het besluit zo uitviel, zoals `GET /api/dossier/waarom/{id}` |
| Tijdlijn lezen | `tijdlijn` | `{ "id": "<vastlegging>" }` | De tijdlijn van de aanvraag waar de vastlegging bij hoort, binnen het dossier en met wat nog komt, zoals `GET /api/dossier/tijdlijn/{id}` |
| Casus lezen | `casus` | `{}` | De casus en de datum van vandaag |
| Formulieren lezen | `formulier` | `{}` | De formulieren met hun velden |
| Brongegevens ophalen | `gegevens` | `{}` | `{ "ondertekend": "<JWS>", "gegevens": { … } }`: wat de uitvoerder over deze burger weet, ondertekend met de organisatiesleutel |

De status van een aanvraag is geen apart verzoek. Een app leest die uit het dossier, onder `stand.antwoorden` met het `id` van de aanvraag als sleutel. `soort` is daar `open` zolang er geen besluit is en `besloten` met het `besluit` zodra een medewerker beslist; `in_de_plaats` en `vervallen` zeggen dat een besluit is herroepen. Volgens §2.2.1 mag een organisatie bij een trage afhandeling antwoorden met hoe de burger de status kan volgen. Dat is hier het dossier.

`velden` zijn de velden van het formulier dat `formulier` geeft. Zonder `artikel` en `op` is de indiening een aanvraag; met `op` hoort ze bij een eerdere vastlegging, zoals een bezwaar tegen een besluit. Bij een aanvraag vult de server het veld waarmee de burger inlogt zelf met het BSN uit de sessie, en een aanvraag die dat veld zelf invult, weigert hij met `422`. Met een `idempotency_key` mag een app een aanvraag opnieuw sturen: de server antwoordt dan met hetzelfde ontvangstbewijs en legt niets opnieuw vast. De sleutel zit binnen de versleuteling, zodat niemand hem eraf kan halen.

## Brongegevens

Met `gegevens` haalt een app op wat de uitvoerder al over de burger weet, zodat ze op het apparaat zelf kan uitrekenen wat een aanvraag zou opleveren. Het zijn de antwoorden van de cel van de uitvoerder (`answers` in haar `cel.yaml`) die het casusbestand onder `uitgeven` noemt. Zonder die lijst gaat er niets uit en is `antwoorden` leeg. Elk genoemd antwoord heeft alleen het inlogveld nodig; de server vult daar het BSN uit de sessie in. Een BSN dat de app meestuurt, leest hij niet.

`gegevens` is de inhoud van de JWS `ondertekend`, zodat een app die niet controleert haar toch kan lezen. Daarin staan:

- `sub`, `iss` en `iat`: het BSN, de OIN van de organisatie en het moment van ondertekenen;
- `bundel_hash`: de hash van de casus die `GET /fsc4b/regelwerken` geeft (zie hieronder);
- `peildatum`: de dag waarop de antwoorden zijn uitgerekend, volgens de klok van de server;
- `antwoorden`: per antwoord de `waarde`, de `parameters` waarmee het gevraagd is en de `herkomst`. Een `waarde` van `null` betekent dat de cel geen antwoord geeft, niet dat het antwoord nee is;
- `records`: de vastleggingen uit het dossier van de burger, per cel in de volgorde van de kroniek, om lokaal na te spelen;
- `regelwerken`: de regelwerkversies waaronder de antwoorden zijn uitgerekend;
- `bij_aanvraag`: de feiten die pas na een aanvraag komen, zoals het advies van de GGD, met de taak die erbij hoort.

De casus zelf staat zonder sessie op `GET /fsc4b/regelwerken`, als `{ "<pad>": "<tekst>" }`. Dat valt buiten deze dienst, want het is de casus en geen persoonsgegeven. De bundel zelf draagt geen handtekening. Zijn hash staat als `bundel_hash` in de ondertekende gegevens, zodat een app de bundel controleert voordat ze ermee rekent. De hash is `sha256:` gevolgd door de SHA-256 in kleine hexadecimale tekens over de aaneenschakeling, per pad in oplopende bytevolgorde, van: de lengte van het pad in bytes als decimaal getal, `:`, het pad, de lengte van de tekst in bytes, `:` en de tekst, alles in UTF-8 en zonder scheidingsteken. Voor `{ "a.yaml": "x" }` zijn dat de bytes `6:a.yaml1:x`. Een manifest kan meer casussen bevatten; de casus van de server is die welke `gegevens` onder `casus` noemt.

Ook het uiterlijk van de procedure staat zonder sessie op `GET /fsc4b/uiterlijk`: `{ "accentkleur", "titel", "organisatie" }`, hetzelfde antwoord als `GET /api/uiterlijk` aan iedereen op het web geeft. Een app toont de schermen daarmee in de kleur en onder de titel van de procedure. Meer van de configuratie staat er niet in.

## Wie de burger is

De sessie draagt het BSN uit de credential van de Activatiedienst. De server maakt daarvan een burger met alleen de rol `regelhet-burger`, en het BSN onder de claim `bsn`. Dat is dezelfde burger als na een login met DigiD, met hetzelfde dossier. Een sessie komt niet buiten `/fsc4b/burger/`.

Bij elke vastlegging houdt de server bij dat ze via dit kanaal binnenkwam (`recorded_via = fsc4b`), met als account `fsc4b:` en de vingerafdruk van de sleutel van de app.

## Ondertekening

Een aanvraag via FSC voor Burgers telt als niet ondertekend. Het beleid van de casus noemt alleen DigiD als middel waarmee een aanvraag ondertekend is, en de server geeft een kanaal geen handtekening die het beleid niet toekent ([wat de server invult](/components/regelhet#what-the-server-fills-in-an-application)). Of een credential van een Activatiedienst als ondertekening mag tellen, is een juridische vraag die dit profiel niet beantwoordt.

## Wat dit profiel niet regelt

`gegevens` is een eigen verzoek van dit profiel en geen afspraak uit de standaard. Dat de server brongegevens uitgaf, legt hij nog niet vast.

Het demo-stelsel heeft een test-Activatiedienst die iedereen een credential geeft voor elk BSN uit de testreeks. Intrekking van credentials, bescherming tegen het opnieuw afspelen van een verzoek binnen een sessie en de toelating van de dienst tot een echt stelsel ontbreken; de pagina [FSC voor Burgers](/components/regelhet-fsc4b#what-is-not-here) zegt per punt wat er is.
