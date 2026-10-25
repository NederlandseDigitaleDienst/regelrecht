# Wetsgeschiedenis: de gegevens

De gegevens achter de poc `wetsgeschiedenis` (`frontend-poc-wetsgeschiedenis/`).
`gegevens/dossiers.json` noemt wat de keuzelijst toont; per dossier of wet staat
er een `<id>.json` met de tijdlijn: elke stap met de tekst zoals zij dan luidt,
per regel het stuk waar die tekst vandaan komt, de ijkpunten en de toelichting
per artikel.

Niets hiervan is met de hand geschreven. De bestanden zijn gemaakt door de
afspeelketen in het Wpp-dossier (`werkbank/generatoren/bouw-wetsgeschiedenis.py
--poc`), die alleen openbare bronnen leest:

- de Kamerstukken zoals officielebekendmakingen.nl ze serveert, en de
  stemmingen uit het Gegevensmagazijn van de Tweede Kamer;
- de Staatsbladen;
- de versies van een wet op wetten.overheid.nl.

Wijzig hier dus niets met de hand: bij de volgende export is het weg. Een fout
in de afspeling hoort in de generator te worden verbeterd.
