# Napp: Nederlandse autoriteit politieke partijen

> **Proof of concept.** Fictieve gegevens. De Napp bestaat niet en de Wet op de
> politieke partijen is een wetsvoorstel; de wettekst hier is een uitvoerbare
> reconstructie, geen geldend recht. Aan deze omgeving kunnen geen rechten
> worden ontleend.

Het subsidieproces uit het wetsvoorstel
[Wet op de politieke partijen](https://www.tweedekamer.nl/kamerstukken/wetsvoorstellen/detail?qry=wetsvoorstel%3A36742)
(kamerstuk 36742), end-to-end uitgevoerd door de engine: aanvragen, besluiten,
betaalopdrachten, bezwaar en een openbaar register.

Zodra de definitieve wettekst er is, wordt het YAML-bestand vervangen zonder
dat de rest verandert.

## Waar het ligt

| Pad | Wat |
|---|---|
| `packages/poc-napp/` | Axum-orchestratielaag: aanvragen, besluiten (RFC-008 besluit-state), betaalopdrachten, openbaar register; SQLite; SSO Rijk via regelrecht-auth (OIDC) met demo-fallback |
| `packages/poc-napp/data/` | Partijregister-snapshot uit open data; zie [Het partijregister](#het-partijregister) |
| `packages/poc-napp/scripts/` | De generatoren achter die snapshot en de demowereld |
| `frontend-poc-napp/` | Drie gescheiden ingangen (Vue 3 + NLDD): publiek (landing + register), subsidieportaal (mock-eHerkenning), beoordelingsomgeving (incl. scenario-runner op de WASM-engine) |
| `corpus-poc/napp/law/` | De wetten: Wpp (twee subsidietracks + betaalopdracht-hook), regeling met bedragen (IoC), AWB-subset (procedure + hooks 3:46/6:7/6:8), Algemene termijnenwet (weekend-verlenging), Kieswet |
| `corpus-poc/napp/scenarios/` | 48 Gherkin-scenario's die vastleggen hoe de wet hoort te werken: vijf in `getoond/` die de beoordelingsomgeving laat zien, de randgevallen in `randgevallen/`. Alle 48 draaien als test, in Rust (`packages/poc-napp/tests/bdd`) en in JS (`frontend-poc-napp/tests`) |

De registry-entry staat in `pocs/registry.yaml` (`slug: napp`, `soort: proxy`).

## Draaien

Napp is een `proxy`-poc: hij draait als eigen ZAD-component en het portaal
stuurt `/napp/` ongewijzigd door. Daarom serveert de backend zichzelf onder
hetzelfde voorvoegsel (`NAPP_BASE_PATH`), en bouwt de frontend met
`POC_BASE=/napp/`. Los draaien werkt zonder beide, op `/`.

```bash
cd frontend-poc-napp && npm install && npm run dev   # vite op :5400
```

Ingangen lokaal: `/` (publiek + register), `/aanvrager/` (eHerkenning gemockt),
`/beoordelaar/` (SSO Rijk, of demo-login zonder OIDC-configuratie).

Omgevingsvariabelen (defaults uit `Dockerfile`):

| Variabele | Default | Betekenis |
|---|---|---|
| `NAPP_PORT` | `8000` | luisterpoort |
| `NAPP_BASE_PATH` | `/napp/` | het voorvoegsel waaronder napp zichzelf serveert |
| `DATABASE_URL` | `sqlite:/data/napp.db?mode=rwc` | SQLite op het persistente volume |
| `NAPP_LAW_DIR` | `/app/law` | wetscorpus |
| `NAPP_STATIC_DIR` | `/app/static` | frontend-assets |
| `OIDC_*` | (leeg) | SSO Rijk; zonder configuratie is de demo-login actief |

## Het partijregister

`data/partijregister.json` is een snapshot uit open data, gegenereerd door
`scripts/bouw_register.py`:

```bash
uv run packages/poc-napp/scripts/bouw_register.py
```

Zetelaantallen komen uit de echte verkiezingsuitslagen (Kiesraad: TK2025 en
GR2026 als CSV, PS2023 en AB2023 uit de officiële Resultaat-EML's) en
inwoneraantallen van het CBS; aanvragers declareren die niet zelf. De
KvK-koppeling is synthetisch: die koppeling (rechtspersoon naar geregistreerde
aanduiding) is precies wat de Napp bij registratie vastlegt en is geen open
data.

Het bestand staat bewust zonder witruimte op schijf, omdat de pre-commit-hook
bestanden boven 500 KB weigert. Herformatteer het niet.

## De demowereld

`scripts/seed_productie.py` vult een draaiende omgeving via de publieke API:
alle landelijke partijen (behalve de PVV) dienen hun jaaraanvraag in met
realistische ledentallen en neveninstellingen, ~65% van de decentrale partijen
en afdelingen doet hetzelfde, en een handvol verhaal-dossiers (afwijzing,
bezwaren, aangehouden betaling, open claim) maakt elke tab van de
beoordelaarsomgeving interessant.

```bash
uv run packages/poc-napp/scripts/seed_productie.py            # localhost:8400
uv run packages/poc-napp/scripts/seed_productie.py <base-url>
```

`scripts/seed_demo.sh` is de kleinere variant: drie partijen, genoeg om de
keten te zien. Het endpoint `POST /api/beheer/demo/reset` wist de dossiers en
zet de snapshot terug; de wereld eromheen bouw je met deze scripts opnieuw op.

## Hoe het werkt

1. Een partij dient een aanvraag in; de backend persisteert de besluit-state
   (stage `BEHANDELING`, conform RFC-008: de engine is stateless, de
   orchestratielaag bewaart de toestand).
2. De beoordelaar ziet de uitkomst die de wet berekent (recht, bedrag,
   motivering met artikelverwijzingen) en stelt het besluit vast. Bij
   toekenning vuurt artikel 16 (post_actions-hook) en ontstaat een
   betaalopdracht naar het (gemockte) betaalsysteem.
3. Bij bekendmaking berekent AWB 6:8 de bezwaartermijn; de Algemene
   termijnenwet verlengt een einddatum die in het weekend valt naar de
   eerstvolgende werkdag (feestdagen zijn als untranslatable gemarkeerd,
   RFC-012).
4. Het bekendgemaakte besluit verschijnt in het openbare register met
   statistieken.

### De aanvraag volgt de rechtspersoon

Conform de twee organisatiemodellen uit de Wpp (MvT bij art. 27). Een
**centraal** georganiseerde partij (één vereniging, één KvK) dient een
samengestelde jaaraanvraag in met al haar aanspraken als onderdelen:
landelijk plus elk decentraal orgaan/gebied (gemeenteraad, provinciale staten,
waterschap) waar de Kiesraad haar zetels toewees. Een **decentraal**
georganiseerde partij heeft afdelingen met eigen rechtspersoonlijkheid; elke
afdeling logt zelf in en vraagt alleen haar eigen gebied aan.

De wet rekent per onderdeel; de som, de specificatie en de ene betaalopdracht
aan de rechtspersoon zijn orchestratie. Onderdelen die al lopen of zijn
toegekend voor het subsidiejaar zijn geblokkeerd voor herhaalaanvraag; na een
afwijzing kan het opnieuw. Tijdens het invullen toont het formulier een
indicatieve uitkomst, live berekend door dezelfde wet-engine.

Welke partijen decentraal georganiseerd zijn volgt uit hun statuten en is geen
open data; de set in `bouw_register.py` is een demo-aanname.

De scenario-runner in de beoordelingsomgeving draait de vijf scenario's uit
`getoond/` live in de browser, op de naar WASM gecompileerde engine. Daar kan
een beoordelaar zelf zien hoe de wet werkt, en de gegevens aanpassen. De
randgevallen staan er niet bij; die bewaken als test dat de wet doet wat hij
moet doen.
