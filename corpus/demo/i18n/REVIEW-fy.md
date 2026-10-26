# Wat een Friestalige revisor moet nakijken

De Friese vertaling van de demo is gemaakt door taalmodellen, niet door een
vertaler. Het meeste is routine, maar een aantal keuzes is afgeleid in plaats
van opgezocht. Dit bestand is de lijst van plekken waar de vertalers zelf
zeiden dat ze het niet zeker wisten, gesorteerd op hoe erg het is als het fout
staat.

Het is dus geen volledige revisie-opdracht. Wie de hele vertaling nakijkt, komt
meer tegen; dit zijn de plekken waar al bekend is dat er iets te beslissen valt.

## 1. Zeker fout als niemand kijkt

**`sim.harmonise.metrics_label`** staat op `Mjittens`, voor het Nederlandse
"Maten" in de zin van statistische maatstaven. Vast staat dat het Nederlandse
woord hier fout is: "Maten" leest in het Fries als "maats, vrienden". Niet vast
staat of `Mjittens` de juiste vorm is; die is afgeleid van de stam `mjitte`,
zonder bron. Alternatieven: `Mjitten`, of een omschrijving als `Maatstêven`.

Dit is een `accessible-label`: alleen een schermlezer leest hem voor. Juist
daarom telt hij, want wie hem hoort heeft geen scherm om de fout mee te
corrigeren.

## 2. Juridische termen, waar een fout gezag suggereert

- **`delegation.type.partner`** "Vennoot" staat op `Feint`. De vertaler noemde
  dit zijn grootste twijfel: `Feint` betekent ook "knecht" of "vrijgezel" en kan
  in een zakelijke context verkeerd landen. Overweeg `Fennoat`, of "Vennoot"
  laten staan.
- **`delegation.type.guardianship`** "Voogdij" staat op `Aldehoedij`. Gangbare
  Friese term, maar weinig frequent; Friese overheidsteksten houden vaak
  "Voogdij" aan. (De persoon zelf heet elders `fâd`, en dat is wel gewoon.)
- **`delegation.type.executor`** "Executeur" staat op `Eksekuteur`. Friese
  spelling van een leenwoord; of dat in juridische stukken gebruikelijk is, is
  niet geverifieerd.
- **"Gegrond" en "ongegrond"** bij bezwaar staan op `grûne` en `ûngrûne`.
  Formeel correct, maar de ingeburgerde rechtsterm zou `terjochte` en
  `net terjochte` kunnen zijn. Er is bewust voor de letterlijke vorm gekozen om
  de koppeling met het Nederlandse bestuursrecht zichtbaar te houden.
- **`zaak.title`** "Zaaksysteem" staat op `Sakesysteem`. Door de vertaler
  gevormde samenstelling, geen gevestigde term.
- **`hanneljensbekwaam`** voor handelingsbekwaam: correct gevormd, niet
  geverifieerd tegen juridisch Fries.

## 3. De banner over de wettekst

`wet.dutch_only.body` staat op:

> De wetten yn dizze demo binne yn it Nederlânsk bekendmakke, en dy bekendmakke
> tekst is de tekst dy't jildt. In oersetting dêrfan is de wet net, en soe hjir
> de yndruk wekke fan wat dêr't nimmen him op beroppe kin. De tekst stiet der
> dêrom sa't er bekendmakke is. It model dat dêrop boud is, en alles
> deromhinne, is wol oerset.

De inhoudelijke claim is met opzet smal: hij gaat over *deze* wetten en de vorm
waarin ze bekendgemaakt zijn, niet over het Nederlands als enige mogelijke taal
van een wet. Er staat dus niets wat in strijd is met de status van het Fries
onder de Wet gebruik Friese taal. Dat deel hoort zo te blijven.

Wat wel schaven verdient is `soe hjir de yndruk wekke fan wat dêr't nimmen him
op beroppe kin`: een zware relatiefconstructie, en `him` is gegenderd waar het
Nederlands neutraal is. Voorstel van de vertaler: `soe hjir lykje op wat it net
is: in tekst dêr't men rjochten oan ûntliene kin`.

## 4. Systematische keuzes, die overal doorwerken

- **`nei alle gedachten`** voor "waarschijnlijk", 36 keer in `fy.yaml`.
  Idiomatisch Fries, maar drie woorden waar het Nederlands er één heeft. Korter
  alternatief: `wierskynlik`. Dit is na de apostrofkwestie de wijziging die de
  meeste regels raakt, dus beslis hem in één keer.
- **`trochsneed`** voor "gemiddeld", onverbogen gebruikt (`trochsneed leeftyd`),
  raakt zo'n twaalf sleutels. Sommige bronnen schrijven het aaneen
  (`trochsneedleeftyd`).
- **`bernedeiferbliuwtaslach`** voor kinderopvangtoeslag: correct gevormd en
  doorzichtig, maar 23 tekens. Staat in een tegelzin en een grafieklabel.
- **Apostrofsoort.** De vertaling gebruikt overal de rechte apostrof
  (`yn 'e moanne`, `euro's`), het Nederlandse origineel op plekken de
  krulapostrof. Eén van beide moet het worden.
- **`Eltse` naast `Elk`.** `Eltse organisaasje` maar `Elk artikel`;
  grammaticaal juist (de-woord tegenover het-woord), oogt inconsistent.
- **`itenfeilichheid`** voor voedselveiligheid. `fiedselfeilichheid` bestaat ook
  en staat dichter bij het Nederlands.
- **`Horeka` of `Horeca`**, en **`Slitery` of `Slytery`** voor slijterij. Friese
  spelling gekozen, maar Friese overheidsteksten schrijven vaak `horeca`.
- **`kofjesaak` of `koffiesaak`** voor het bedrijf van de persona.
- **`graaf`** als term uit de grafentheorie is onvertaald gelaten omdat hij in
  het Fries hetzelfde zou zijn. Dat is afgeleid uit het patroon van
  wetenschappelijke leenwoorden, niet opgezocht.
- **`regelingen`** als meervoud: de `-ing`-meervouden lopen tussen Fries en
  Nederlands soms uiteen.
- **`regelwurk`, meervoud `regelwurken`**, voor het Nederlandse "regelwerk":
  één regeling zoals die in YAML is vastgelegd, met een versie per datum. Het
  Nederlandse woord is zelf
  een nieuwe vakterm (RFC-044), en het Friese is er letter voor letter van
  afgeleid; geen vertaler heeft het gezien. Wat vaststaat: `wurk` is het Friese
  woord voor "werk", en `regel` is in beide talen hetzelfde. Wat niet vaststaat:
  of een Friese lezer de samenstelling als één begrip leest, of het meervoud
  `regelwurken` goed valt (het telbare gebruik is ook in het Nederlands nieuw),
  en of het een it-woord is. De vertaling behandelt het als it-woord (`it
  foarige regelwurk`, `gjin oar regelwurk`), naar het
  Nederlandse "het regelwerk". Het staat in het tabblad, in de dia over dat
  tabblad en in 24 sleutels in `fy.js` (zoek op `regelwurk`, ook met hoofdletter), dus beslis het in
  één keer.

- **`begjinstân` en `presintaasjemodus`** in `app.reset.body`. Beide
  samengesteld uit woorden die elders in de vertaling staan (`begjin`, `stân`,
  `presintaasje`), niet opgezocht. `presintaasjemodus` kan ook `wize fan
  presintearjen` zijn, als een samenstelling met `modus` in het Fries te
  technisch leest.

- **De scenarioweergave** (`scenario.value.not_stated`, `scenario.passed`,
  `scenario.summary.*`). `net opjûn` voor "niet opgegeven" en `Slagge` voor
  "Geslaagd" zijn afgeleid van woorden die elders in de vertaling staan
  (`opjûn`, `mislearre`), niet opgezocht.

- **De opgenomen rondleiding** (`walkthrough.*`, `recorder.*`). `rûnlieding`
  voor "rondleiding" is gekozen, niet opgezocht. Het pad is `/fy/runlieding`
  zonder kapje, omdat een `û` in een adres als `%C3%BB` op het scherm komt;
  past `rûnlieding` niet, dan moet het pad mee. Verder afgeleid en niet
  opgezocht: `Skoftsje` voor "Pauzeren" (een pauze nemen, niet een video
  stilzetten; `Stilsette` kan ook), `Ûndertitels`, `Útskreaune tekst` voor
  "uitgeschreven tekst" en `fersprekking` voor "verspreking". De
  opnameknoppen (`recorder.*`) ziet alleen wie de rondleiding opneemt.

## 5. Beelden die in het Fries misschien niet werken

- **`baalje`** (balie), in "de andere kant van de balie". Het woord bestaat,
  maar de uitdrukking is een Nederlands beeld.
- **`boustrjitte`** voor "bouwstraat" (de CI/CD-pijplijn). Letterlijke calque
  van een Nederlandse metafoor die in het Fries niet bestaat.
- **`Oanfreegje`** als kop waar het Nederlands "Aanvragen" heeft. Dat kan in het
  Nederlands zowel werkwoord als meervoud zijn; hier is het als handeling
  gelezen. `Oanfragen` zou ook kunnen.

## 6. Wat bewust niet vertaald is

Eigennamen van organisaties (`Belastingdienst`, `Kamer van Koophandel`,
`Gemeente Rotterdam`), wetnamen, afkortingen (`BSN`, `KvK`, `AOW`, `Anw`,
`AIO`, `Bbz`, `WW`), de productnaam `RegelRecht` en het adres
`regelrecht.rijks.app`.

De wettekst zelf blijft ook Nederlands, in elke taal. Dat is een uitgelegde
keuze en geen omissie; zie de banner onder punt 3.

Woorden die in het Fries werkelijk hetzelfde zijn blijven staan: "Ja", "Nee",
"Titel", "Adres", "Totaal", "Seed", "Mediaan", "Partner", "Staffel", "Wetten",
"Portaal", "Alles". Daar is per sleutel naar gevraagd; het is geen luiheid.

## 7. Het dek van Claudia, nog niet nagelezen

Deze dia's kwamen later, met het ondernemersverhaal van Claudia, en zijn door
hetzelfde soort vertaler gemaakt. Niemand heeft ze nagelezen. Het hele blok
verdient een lezing; de woorden hieronder zijn nieuw in de demo en komen nergens
anders in de vertaling voor, dus er is ook geen eerdere keuze om op te leunen.

- **`slides.7`** (Wetten hingje gear), **`slides.9`** (De feroardering
  toetst), **`slides.15`** (In terras oanfreegje), **`slides.16`** (De gemeente
  beoardielet) en **`slides.17`** (Prekario folget út de fergunning) zijn
  nieuw. **`slides.14`** (In ûndernimmer, Claudia) is herschreven.
- **"Verordening"** staat op `feroardering`. Of dat de gangbare Friese vorm is
  voor een gemeentelijke verordening, is niet geverifieerd.
- **"Aanslag"** (de belastingaanslag, `slides.17.overline`) staat op
  `oanslach`. Het risico is dat het als "aanval" leest.
- **"Vrijgesteld"** staat op `frijsteld` en **"geheven"** op `heft wurdt`
  (`slides.17`). Belastingtaal; niet nagekeken tegen Friese
  overheidsteksten.
- **"Verleend"** staat op `ferliend`, en als bijvoeglijk naamwoord op
  `ferliene` ("it ferliene terras"). Controleer of beide vormen kloppen.
- **`profiles.claudia.portal_tab_label`** staat nu op `Myn oerheid`, gelijk
  aan dat van Merijn.

## 8. De opgenomen rondleiding

- **`walkthrough.restoring`** staat op `De demo wurdt weromset` ("De demo
  wordt teruggezet", terwijl de speler na het rondklikken de stand
  terugzet). Of `weromsette` hier het gangbare werkwoord is, is niet
  nagekeken.

## Hoe je een correctie doorvoert

De schermteksten staan in `frontend-demo/src/i18n/fy.js`, één sleutel per regel.
De inhoud van het presentatiedek, de portaalkoppen, de tegelzinnen en de
persona's staan in `corpus/demo/i18n/fy.yaml`, met het pad als sleutel.

Na een wijziging: `cd frontend-demo && npx vitest run`. Drie dingen worden
bewaakt, en ze falen met de sleutelnaam erbij:

- elke sleutel die het Nederlands heeft, bestaat ook in het Fries
- elke `{placeholder}` staat er nog
- een Fries woord houdt zijn diakriet, ook vooraan een zin (`Ôfwiisd`, niet
  `Ofwiisd`). Die laatste fout maakten alle vier de vertalers, en geen andere
  controle ziet hem: zo'n string verschilt immers van het Nederlands.

`MAX_IDENTICAL_SHARE` in `src/i18n/i18n.test.ts` bewaakt hoeveel sleutels nog
letterlijk het Nederlands zijn. Dat getal hoort omlaag te gaan als er vertaald
wordt, niet omhoog.
