# Wat een Franstalige revisor moet nakijken

De Franse vertaling van de demo is gemaakt door taalmodellen, niet door een
vertaler. Het meeste is routine, maar een aantal keuzes is afgeleid in plaats
van opgezocht. Dit bestand is de lijst van plekken waar de vertalers zelf
zeiden dat ze het niet zeker wisten, gesorteerd op hoe erg het is als het fout
staat.

Het is dus geen volledige revisie-opdracht. Wie de hele vertaling nakijkt, komt
meer tegen; dit zijn de plekken waar al bekend is dat er iets te beslissen valt.

Een terugkerende moeilijkheid: het Nederlandse bestuursrecht heeft voor veel
begrippen geen Franse tegenhanger. De vertaling kiest dan soms de Franse term
die er het dichtst bij ligt en soms een letterlijke omschrijving. Waar dat
gebeurt staat het hieronder, want een Franse term zegt meer dan hij hier mag.

## 1. Zeker fout als niemand kijkt

- **Congruentie in geslacht en getal.** Vaste teksten en labels die woord voor
  woord worden samengesteld, buigen niet altijd mee.
  - De woorden onder `words:` in `glossary.fr.yaml` staan allemaal in de
    mannelijke enkelvoudsvorm (`actief: actif`, `toegestaan: autorisé`). De 26
    namen die nu samenstellen zijn nagelopen en kloppen; de overige 423 staan
    als hele naam onder `names:`. Een nieuwe veldnaam die samenstelt kan
    "Autorisation requis" opleveren, en geen controle ziet dat.
  - `zaak.lane.bekendmaking` staat op `Notifiés` (mannelijk, naar *dossiers*),
    terwijl de tags en statuswoorden op het bord vrouwelijk zijn (naar
    *demande*). Bekijk het bord als geheel.
  - `sheet.application.status.in_review.supporting` zegt `Vous serez informé.`,
    mannelijk. Er is geen korte neutrale vorm gevonden; `Nous vous tiendrons
    informé·e` of `Vous recevrez une réponse` zijn opties.
  - `sim.outcome.eligible` (`{eligible} sur {evaluated} éligibles`) en
    `sim.breakdown.*` behandelen het meervoud niet: bij één persoon staat er
    toch `éligibles`.
- **De ja/nee-tegels** (`outcome_phrasing.*.lead`, `.yes`, `.no` in `fr.yaml`,
  voor kieswet, kinderbijslag, vreemdelingenwet, terrassen, alcohol, HACCP,
  meldplicht, informatieplicht en jaarrekening). Het Nederlands zet een aanhef
  ("Voor de verkiezingen van {date} heb je") boven de uitspraak ("STEMRECHT" /
  "GEEN stemrecht"). Dat kan in het Frans niet: een ontkenning vraagt
  "ne ... pas", dus `vous avez` boven `PAS le droit de vote` is geen zin. De
  aanhef is daarom een kop zonder werkwoord geworden (`Pour les élections du
  {date}`) en de uitspraak een hele zin (`VOUS AVEZ LE DROIT DE VOTE` /
  `Vous n'avez PAS le droit de vote`). Niet vast: of die kop als overline goed
  leest, en of een hele zin in kapitalen bij "ja" te luid is. Bij kinderbijslag
  en vreemdelingenwet is van "Je hebt" een onderwerpkop gemaakt
  (`Allocations familiales`, `Séjour aux Pays-Bas`). Bekijk dit op het scherm.
- **`format.missing`** staat op `manque : {facts}`, zodat de zin `Il manque :
  …` wordt. Hetzelfde fragment landt ook na een `·` of `;`
  (`wet.lineage.computed_by_missing`, `wet.tile.outcome.missing`,
  `wet.tile.undecidable.body`) en leest daar als `· manque : …`. Grammaticaal,
  maar kortaf. Bekijk het op het scherm.

## 2. Juridische termen, waar een fout gezag suggereert

- **Nederlandse instellingen met een Franse naam.** In `glossary.fr.yaml` staan
  `rekenkamer` op `Cour des comptes`, `raad_van_state` op `Conseil d'État` en
  `ombudsman` op `médiateur` (ook in `is_algemene_rekenkamer`,
  `is_raad_van_state`, `is_ombudsman` en
  `is_voor_het_leven_benoemde_ambtenaar_rvs_of_rekenkamer`). Het Engels deed
  hetzelfde ("Court of Audit"). Het risico: een Franse lezer houdt ze voor de
  Franse instellingen, en Frankrijk heeft er van elk een, met andere
  bevoegdheden (de Franse ombudsman heet bovendien `Défenseur des droits`).
  Alternatief: de Nederlandse naam met een Franse glos, `Raad van State
  (Conseil d'État néerlandais)`. In `scenarios.fr.yaml` zijn deze namen juist
  Nederlands gebleven, dus de twee bestanden spreken elkaar nu tegen.
- **Bezwaar** staat op `réclamation`, gegrond/ongegrond op `fondée / non
  fondée` (`fr.js` en glossary; `beroep` is `recours`). Het Franse equivalent
  van het Nederlandse bezwaar is de `recours administratif préalable`;
  `réclamation` is de term in belastingzaken. Gekozen om de knoppen kort te
  houden. Alternatief: `recours gracieux`.
- **Hardheidsclausule** staat op `clause d'équité` (`sheet.edit.hardship.*`,
  `sheet.application.hardship.*`, `sheet.corrections.hardship*`). Geen Franse
  tegenhanger. `Clause de sauvegarde` betekent iets anders; of België `clause
  de dureté` kent is niet nagekeken.
- **Curatele, bewind en mentorschap.** `curatelle`, `administration de biens`
  (of `administration (protectrice)` in scenario's) en `Mentor`/`mentorat`
  (`delegation.type.*`, persona's, glossary, scenario's). Het Franse stelsel
  (tutelle, curatelle, sauvegarde de justice, habilitation familiale) loopt
  niet één op één. Vast: de Nederlandse curatele ligt dichter bij de Franse
  `tutelle` van meerderjarigen dan bij de lichtere `curatelle`; gekozen is voor
  herkenbaarheid boven gelijkwaardigheid. `administration de biens` en
  `mentorat` zijn letterlijk, geen Franse rechtsfiguren. Bewindvoerder is in
  `fr.js` `Administrateur des biens`; het Franse `mandataire judiciaire à la
  protection des majeurs` was te lang.
- **Faillissementscurator** staat in de persona's op `liquidateur judiciaire`
  (om verwarring met `curateur` uit curatele te vermijden) en in de scenario's
  op `curateur de faillite` (Belgisch, om het verband met de
  Faillissementswet-feature te houden). Kies één.
- **`administrateur`** is zowel bewindvoerder als bestuurder van een BV, NV of
  stichting. In de scenariotitels staat er steeds `d'une BV` bij, dus het is in
  context te onderscheiden. Alternatief voor bestuurder: `dirigeant`.
- **Schuldsanering**: in de persona's `règlement des dettes`, in de scenario's
  `assainissement de dettes`, schone lei `effacement des dettes`. Frankrijk
  kent `procédure de surendettement` en `rétablissement personnel`. Gelijktrekken.
  Ook: `boedel` als `masse (de faillite)` leest bij WSNP mogelijk vreemd, en
  "Sanietnamen" werd `noms des débiteurs`, wat de nuance "onder
  schuldsanering" verliest.
- **Bekwaamheid.** Wilsonbekwaam is `hors d'état d'exprimer sa volonté` (uit
  de Franse Code de la santé publique; lang, alternatief `incapable de
  discernement`, Zwitsers). Handelingsbekwaam is `juridiquement capable`.
  Handlichting is `émancipé (handlichting)`: het dichtstbijzijnde instituut,
  maar de Franse émancipation reikt verder dan venia aetatis.
- **Bestuursorgaan** staat overal op `autorité administrative`. Vast: dat is de
  Franse term. Niet vast: of het Awb-begrip (met B-organen en private partijen
  met openbaar gezag) daarmee overkomt. `(A-orgaan)` en `(B-orgaan)` zijn
  onvertaald.
- **Bekendmaken** van een besluit is `notifier` (`À notifier`, `Notifiés`). Het
  Franse bestuursrecht notificeert individuele besluiten; `publier` zou fout
  zijn.
- **Machtiging** is `mandat`, gevolmachtigde `mandataire`.
- **Belastingtermen.** `heffingskorting` is `crédit d'impôt` (strikt geldt de
  korting ook voor premies); `arbeidskorting` `crédit d'impôt sur les revenus
  du travail`; `toetsingsinkomen` `revenu de référence` (dichtbij, niet
  identiek); `aanmerkelijk belang` `participation substantielle`; box 1/2/3
  blijft `box`. De combinatiekorting heet in de glossary `crédit d'impôt de
  combinaison` en in de persona's `crédit d'impôt pour activité combinée lié au
  revenu`. Eigen woning is in `fr.js` `Revenus du logement dont vous êtes
  propriétaire` en in de glossary `Résidence principale`. Beide paren
  gelijktrekken.
- **Uitkering en toeslag.** Toeslag is `allocation`, uitkering meestal
  `prestation`, maar de WW-uitkering `allocation chômage` (de Franse vaste
  term), zodat `allocation` niet uniek is voor toeslagen. AOW is `Pension AOW`,
  ziekengeld `indemnités de maladie` (Frans: `indemnités journalières`),
  woonkostentoeslag `Supplément frais de logement`, bijstand `aide sociale`
  (alternatief `revenu de solidarité`), uitloopbijstand `aide sociale de
  cessation` (omschrijving).
- **Precario** staat op drie manieren: `taxe precario` in `fr.yaml` (zoals EN
  "precario tax"), `taxe d'occupation du domaine public (precariobelasting)` in
  de scenario's en `taxe d'occupation du domaine public` in de glossary.
  Frankrijk heft een `redevance`, geen belasting. Kies één vorm.
- **Vergunningen.** Algemeen `autorisation`; alcoholvergunning is in de
  glossary `licence d'alcool` en in de scenario's `licence Alcoholwet`. Het
  Franse begrip is `licence de débit de boissons`.
- **Tweede Kamer** is in de persona's `Seconde Chambre` en in de scenario's
  `Tweede Kamer`. Gelijktrekken.
- **Archief.** `selectielijst` is `liste de sélection` (Frans archiefjargon:
  `tableau de gestion`), overbrengen `transférer` (jargon: `verser`), openbaar
  `public` (jargon: `communicable`), terwijl de feature-titel van de Archiefwet
  wel `communicabilité` gebruikt.
- **Kleinere termen, alle open:** harde/zachte weigeringsgrond als `motif de
  refus impératif / facultatif` (vrij zeker); `slecht levensgedrag` als
  `mauvaise conduite` (Frans spreekt eerder van `moralité`);
  `verdagingstermijn` als `délai de prorogation`; `loonheffingennummer` als
  `numéro de retenue à la source`; `verblijfsobject` als `unité d'occupation`;
  `woonlandfactor` als `coefficient pays de résidence`; `functionaris` als
  `agent`; `maatschap` als `société civile d'avocats`; Gedeputeerde Staten
  blijft `GS` zonder omschrijving.

## 3. De banner over de wettekst

`wet.dutch_only.body` in `fr.js` zegt dat de wetten in deze demo in het
Nederlands zijn bekendgemaakt en dat die tekst geldt. De claim is met opzet
smal: hij gaat over *deze* teksten, niet over het Nederlands als enige
mogelijke taal van een wet. Dat deel hoort zo te blijven. Geen vertaler
twijfelde hier, maar het is de tekst waar een verschuiving in betekenis het
meest kost. Lees vooral `donnerait l'impression d'un texte dont personne ne
peut se prévaloir`: bedoeld is dat een vertaling zou lijken op een tekst
waaraan men rechten kan ontlenen, terwijl dat niet zo is.

## 4. Systematische keuzes, die overal doorwerken

- **`regelwerk` staat op `réglementation`**, meervoud `réglementations`, ook in
  het pad `/fr/reglementations`. Het Engels heeft er een nieuw woord voor
  gemaakt ("rulework"); het Frans heeft geen gevestigd woord, en
  `réglementation` betekent ook "het geheel van regels". In `slides.5.title`
  (`La loi comme réglementation`) gaat zo de woordspeling verloren. Het staat in
  25 sleutels in `fr.js` en drie in `fr.yaml`. Een wijziging verandert ook de
  route in `frontend-demo/src/router.ts`, dus beslis het in één keer.
- **`regeling` staat op `dispositif`** (portaal, zaaksysteem, simulatie), zoals
  in `dispositif d'aide`. Vast: het moet een ander woord blijven dan dat voor
  regelwerk. Niet vast: of `dispositif` past bij plichten en vergunningen
  (`zaak.portaal.subtitle.business`), waar `régime` of `obligation` beter kan.
- **Behandelaar** is `gestionnaire`: kort, want het staat in tags en
  historieregels, maar minder formeel dan `agent instructeur`.
- **Horeca** staat op drie manieren: `Café-restaurant(s)` in `fr.js`
  (`group.hospitality`, `sim.knobs.hospitality`), `établissement de
  restauration` in de scenario's en `hôtellerie-restauration` in de glossary.
  Horeca omvat ook hotels en cafés zonder eten. Franse vakterm: `CHR`. Kies één.
  Slijterij is `Caviste` (`group.off_licence`): een winkeltype, geen
  vergunningscategorie.
- **Typografie.** Overal een gewone spatie voor `: ; ? !`, geen Franse
  vaste spatie (U+00A0 of U+202F), en de rechte apostrof, zodat het Frans
  gelijk loopt met de andere woordenboeken. Een revisor kan Franse typografie
  door het hele bestand willen; dan breekt een regel ook niet meer voor het
  leesteken. Verder: `{pct}%` zonder spatie in `fr.js` (de getalopmaak zit niet
  in dat bestand), maar `45 %` met spatie in de scenario's; getallen in
  scenariotitels houden de punt (`1.5m`, `0.43`), zoals het Engels; `€ 36,10`
  werd `36,10 €` in `slides.17.bullets.1`; `« pourquoi »` en `« Surpeuplement »`
  met guillemets.
- **Wetsverwijzingen**: lid is `alinéa`, onder a `point a`, aanhef `phrase
  introductive`. Franse wetgeving nummert anders (I, 1°, a)).
- **Scenariostappen (Gherkin)** hebben Franse sleutelwoorden en sjablonen in
  `frontend-demo/src/data/gherkinNl.ts`. De sleutelwoorden zijn de officiële van
  `# language: fr` (`Soit`, `Quand`, `Alors`, `Et`, `Mais`; `Fonctionnalité`,
  `Contexte`, `Scénario`); die staan vast. Twijfel zit in de sjablonen:
  "peildatum" is `date de référence`, "gesleuteld op" is `indexées par`,
  `vaut` dient voor gelijkheid van zowel tekst als getal, en een
  verzamelingsparameter leest als `est l'ensemble :`. Kijk of dat leest als
  natuurlijke Franse testtaal. Ook op de dia's (`slides.8.bullets.0`,
  `slides.9.bullets.0`) staat "Gegeven" als `Soit`.
- **Geslacht van personen.** Waar het Nederlands neutraal is, kiest het Frans
  een geslacht naar de voornaam in `profiles.yaml` (`Avocate et liquidatrice
  judiciaire`, `Étudiante allemande`, `L'entrepreneuse` in `slides.14.*`;
  `entrepreneure` kan ook). `zaak.portaal.heading.citizen` is omgebouwd tot
  `Quels sont les droits de {name} ?` om een gegenderd `a-t-il droit` te
  vermijden.
- **`heeft_*`-velden** zijn `A un/une …`, plichten `Est soumis à l'obligation
  de …`. Alternatief: `Dispose d'un …`.

## 5. Woordkeuze en register

- **De ondertitel** "Van wet naar digitale werking" staat op twee manieren:
  `De la loi à son exécution numérique` (`home.hero.subtitle`) en `de la loi à
  sa mise en œuvre numérique` (`slides.0.subtitle`, `slides.18.title`).
  "Werking" heeft geen schoon Frans woord. Kies één, eventueel iets
  pakkenders.
- **`slides.4.lines.1`, `slides.18.lines.1`** "We maken beleid door te doen":
  `Nous construisons les politiques par la pratique`. Vrij; de spreektaal van
  "door te doen" gaat verloren.
- **`slides.3.lines.*`** "Wat als": `Et si / On partait de la loi / On la
  rendait …`. "Et si" vraagt de imparfait; `on` houdt het luchtig.
- **`slides.0.footer`** "Nederlandse Digitale Dienst" is `Service numérique
  néerlandais`, omdat het Engels de naam ook vertaalde. Alternatief: de
  Nederlandse naam laten staan.
- **`slides.14.bullets.*`**: "APV, Alcoholwet, Warenwet" en "NVWA en RVO" zijn
  omschreven (`l'autorité alimentaire et l'agence des entreprises`), zoals het
  Engels; de afkortingen zijn weg.
- **`bouwstraat`** is `chaîne d'intégration`; **`sim.seed`** is `Graine`
  (`Seed` begrijpen technische lezers ook); **`NL Wallet`** is mannelijk, naar
  `portefeuille`.
- **Scenariotitels om te lezen:** `Tweeverdieners met hoger inkomen` (`Un
  couple à deux revenus plus élevés`, dubbelzinnig); `Over de jaargrens`
  (`Au passage de l'année`); `voldoende dosis` bij splijtstoffen (`une dose …
  conformes`; het Nederlands is zelf vreemd); `adviesplicht ICT-projecten`
  (`obligation d'avis`, of `de consultation`); `kostendelersnorm` (`norme de
  partage des frais`).

## 6. De opgenomen rondleiding

`walkthrough.fr.yaml` levert Franse ondertitels en dia's. `fr.js` heeft twee
regels voor de speler: `walkthrough.dutch_voice` (ondertitels, dia's en demo
vertaald) en `walkthrough.dutch_only` (alleen de bediening vertaald). Welke er
in het Frans verschijnt, beslist de code. Controleer dat de getoonde regel
waar is. Het pad is `/fr/visite`.

## 7. Wat bewust niet vertaald is

Eigennamen van organisaties (`Belastingdienst`, `Toeslagen`, `CJIB`, `Kamer van
Koophandel`), wetnamen (Awb, met één keer een glos in `zaak.publish.body`;
Participatiewet, Wmo 2015, Jeugdwet), afkortingen (`BSN`, `AOW`, `WW`, `BAG`),
rechtsvormen (BV, NV, Stichting, Vereniging), de productnaam `RegelRecht` en
het adres `regelrecht.rijks.app`. Ook `untranslatable` in een scenariotitel: dat
is een schematerm. Uitzondering op de eigennamen: de instellingen in de
glossary (punt 2).

De wettekst zelf blijft ook Nederlands, in elke taal. Dat is een uitgelegde
keuze en geen omissie; zie de banner onder punt 3.

## Hoe je een correctie doorvoert

| Wat | Bestand |
| --- | --- |
| Schermteksten | `frontend-demo/src/i18n/fr.js`, één sleutel per regel |
| Dek, portaalkoppen, tegelzinnen, persona's | `corpus/demo/i18n/fr.yaml`, pad als sleutel |
| Scenariotitels | `corpus/demo/i18n/scenarios.fr.yaml`, Nederlandse titel als sleutel |
| Veldnamen | `corpus/demo/i18n/glossary.fr.yaml` (`names:` en `words:`) |
| Rondleiding | `corpus/demo/i18n/walkthrough.fr.yaml` |

`glossary.generated.js` en `scenarioTitles.generated.js` maakt
`scripts/copy-demo-corpus.mjs` opnieuw bij `npm run dev` en `npm run build`;
bewerk ze niet met de hand.

Na een wijziging: `cd frontend-demo && npx vitest run`. Dat bewaakt dat elke
sleutel die het Nederlands heeft ook in het Frans bestaat en dat elke
`{placeholder}` er nog staat, en het faalt met de sleutelnaam erbij.
`MAX_IDENTICAL_SHARE` in `src/i18n/i18n.test.ts` bewaakt per taal hoeveel
sleutels nog letterlijk het Nederlands zijn. Dat getal hoort omlaag te gaan
als er vertaald wordt, niet omhoog.
