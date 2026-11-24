---
title: "Wetsproces: dienstprofiel FSC voor Burgers"
description: "Welke verzoeken de wetsproces-server via FSC voor Burgers aanneemt, wat erin gaat en eruit komt, en wie daarbij de burger is."
lang: nl
---

FSC voor Burgers legt niet vast welke verzoeken een dienst aanneemt. Dat doet het dienstprofiel (§2.2.1 en §2.2.2 van de werkversie). Deze pagina is het dienstprofiel van `wetsprocesBurgerApi`, de dienst waarmee de [wetsproces-server](/components/wetsproces) een app laat doen wat een burger in de browser doet: een aanvraag indienen en volgen wat ermee gebeurt. Hoe een app een sessie opent en hoe het verkeer versleuteld is, staat op [FSC voor Burgers](/components/wetsproces-fsc4b).

## De dienst

| | |
|---|---|
| Naam in de discovery-configuratie | `wetsprocesBurgerApi`, versie `v1` |
| Adres | `GET /fsc4b/discovery` noemt `<publiek adres>/fsc4b/burger` onder versie `v1`; een verzoek gaat naar dat adres met `/v1/` en het pad erachter |
| Sessie | `sessionApi` versie `v1`: `CreateSession` uit §4.3.4 |
| Scope | `nl.regelrecht.wetsproces.demo` op het demo-stelsel; voor een echt stelsel is er nog geen |
| Specificatie | OpenAPI 3 op `GET /fsc4b/openapi.json` van dezelfde server |

De OpenAPI-specificatie is de bron voor de precieze vorm van elk verzoek. Deze pagina zegt wat de verzoeken betekenen.

## Verzoeken

Elk verzoek is een `POST` met het sessietoken in de header `Authorization` en als body een JWE onder de sessiesleutel. Het antwoord is een JWE onder dezelfde sleutel, met de HTTP-status ernaast. Alleen een `401` (geen lopende sessie onder het token, of een body die niet ontsleutelt) en een `500` bij het lezen van de sessie zijn gewone JSON. Hieronder staat steeds de ontsleutelde inhoud.

| Verzoek | Pad | Body | Antwoord |
|---|---|---|---|
| Aanvraag indienen | `indieningen` | `{ "velden": {…}, "artikel"?: "…", "op"?: "<id>", "idempotency_key"?: "…" }` | De vastgelegde indiening: `id`, `kind` (`AANVRAAG` voor een aanvraag), de ingevulde `fields` en de regelwerkversies waaronder ze ontstond |
| Dossier inzien | `dossier` | `{}` | `{ "grammen": [ … ], "stand": { … }, "verzoeken": [ … ], "vervolg": [ … ], "door_het_systeem": { … } }`: alles wat over deze burger is vastgelegd, en de stand van elke aanvraag |
| Besluit laten uitleggen | `waarom` | `{ "id": "<besluit>" }` | Waarom het besluit zo uitviel, zoals `GET /api/dossier/waarom/{id}` |
| Casus lezen | `casus` | `{}` | De casus en de datum van vandaag |
| Formulieren lezen | `formulier` | `{}` | De formulieren met hun velden |

De status van een aanvraag is geen apart verzoek. Een app leest die uit het dossier, onder `stand.antwoorden` met het `id` van de aanvraag als sleutel. `soort` is daar `open` zolang er geen besluit is en `besloten` met het `besluit` zodra een medewerker beslist; `in_de_plaats` en `vervallen` zeggen dat een besluit is herroepen. Volgens §2.2.1 mag een organisatie bij een trage afhandeling antwoorden met hoe de burger de status kan volgen. Dat is hier het dossier.

`velden` zijn de velden van het formulier dat `formulier` geeft. Zonder `artikel` en `op` is de indiening een aanvraag; met `op` hoort ze bij een eerdere vastlegging, zoals een bezwaar tegen een besluit. Bij een aanvraag vult de server het veld waarmee de burger inlogt zelf met het BSN uit de sessie, en een aanvraag die dat veld zelf invult, weigert hij met `422`. Met een `idempotency_key` mag een app een aanvraag opnieuw sturen: de server antwoordt dan met hetzelfde ontvangstbewijs en legt niets opnieuw vast. De sleutel zit binnen de versleuteling, zodat niemand hem eraf kan halen.

## Wie de burger is

De sessie draagt het BSN uit de credential van de Activatiedienst. De server maakt daarvan een burger met alleen de rol `wetsproces-burger`, en het BSN onder de claim `bsn`. Dat is dezelfde burger als na een login met DigiD, met hetzelfde dossier. Een sessie komt niet buiten `/fsc4b/burger/`.

Bij elke vastlegging houdt de server bij dat ze via dit kanaal binnenkwam (`recorded_via = fsc4b`), met als account `fsc4b:` en de vingerafdruk van de sleutel van de app.

## Ondertekening

Een aanvraag via FSC voor Burgers telt als niet ondertekend. Het beleid van de casus noemt alleen DigiD als middel waarmee een aanvraag ondertekend is, en de server geeft een kanaal geen handtekening die het beleid niet toekent ([wat de server invult](/components/wetsproces#what-the-server-fills-in-an-application)). Of een credential van een Activatiedienst als ondertekening mag tellen, is een juridische vraag die dit profiel niet beantwoordt.

## Wat dit profiel niet regelt

Het demo-stelsel heeft een test-Activatiedienst die iedereen een credential geeft voor elk BSN uit de testreeks. Intrekking van credentials, bescherming tegen het opnieuw afspelen van een verzoek binnen een sessie en de toelating van de dienst tot een echt stelsel ontbreken; de pagina [FSC voor Burgers](/components/wetsproces-fsc4b#what-is-not-here) zegt per punt wat er is.
