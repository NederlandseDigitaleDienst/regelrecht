---
name: law-wetsgeschiedenis
description: >
  Answers, per article and lid, where the text of a Dutch law comes from and what
  the legislature said about it: which document (bill, nota van wijziging,
  amendment) produced each line, which amendments touched it and whether they
  replayed, how it read on a given date, and the explanatory passages (memorie
  van toelichting, nota naar aanleiding van het verslag, amendment explanations)
  hung on that article or lid — every answer with a citable Kamerstuk and a check
  against the official text. Fetches and replays a Kamerdossier on demand, also a
  bill still in parliament (by dossier number), and a law through time (by BWB
  id). Use proactively when drafting or justifying a regelwerk's
  machine_readable logic, when the user asks for "de bedoeling van de wetgever",
  "wetsgeschiedenis", "toelichting bij artikel", "waar komt deze tekst vandaan",
  "Kamerstukken", "amendement", "nota van wijziging", or how an article read on
  a date; and before law-mvt-research or law-letter-fidelity-audit, which both
  need the toelichting per article.
allowed-tools: Read, Bash, Grep, Glob
user-invocable: true
---

# Law wetsgeschiedenis — the history behind an article, citable

A regelwerk has to be justified: every condition in `machine_readable` traces to
the letter of the law, and where the letter leaves room, to what the legislature
said it meant. That second part usually means reading a whole Kamerdossier.
This skill answers it per article and per lid, with the document to cite.

It queries the timelines that the wetsgeschiedenis replay chain builds
(`packages/poc-wetsgeschiedenis/`). That chain fetches a Kamerdossier, replays
every nota van wijziging and every adopted amendment on the original bill, and
records for every line which document its text came from. It hangs each
explanatory passage on the article (and where it can, the lid) it discusses,
and compares the end result word for word with the official text: the bill as
the Tweede Kamer adopted it, the Staatsblad, and once in force, the BWB.

## The script

```bash
W=.claude/skills/law-wetsgeschiedenis/wetsgeschiedenis.py

python3 $W lijst                                   # timelines available
python3 $W stukken 35968                           # the documents, in order, with replay quality
python3 $W artikel 35968 2.6 --lid 2               # the text, each line with its source document
python3 $W herkomst 35968 5.6                      # amendments that touched it; its versions
python3 $W toelichting 35968 2.6 --lid 2           # what the explanatory documents say
python3 $W toelichting 36889 --doelwet 15          # a bill that amends a law: passages on that law's article 15
python3 $W artikel BWBR0007376 15 --op 2010-01-01  # a law through time: how it read on a date
python3 $W wet BWBR0007376 --op 2010-01-01         # the whole law on that date
python3 $W wet 35968 --na kst-35968-8 --herkomst   # the whole bill after a nota, each line with its origin
python3 $W vergelijk BWBR0007376 --van 2004-01-01 --tot 2010-01-01   # what changed in between, word by word
python3 $W vergelijk 35968 4.1 --van kst-35968-7 --tot kst-35968-8   # one article, across one nota
```

`<id>` is a Kamerdossier number for a bill (five digits), or a BWB id for a law
through time. A moment is the same as in the PoC's player:

- `--op JJJJ-MM-DD`: the state on that date (for a law through time, the BWB
  version in force that day);
- `--na kst-…` (or `versie-…`): right after that document;
- `--stap N`: right after replay step N, one onderdeel of a nota or amendment
  (`--stap 0` is the bill as filed; `stukken` and `herkomst` show the steps);
- none: the last state.

`vergelijk` takes two such moments as `--van` and `--tot`, written as a date, a
document id, or `stap:N`; without `--tot` it compares with the last state. Every
answer names the moment it shows ("stand op 2023-07-07, na Kamerstuk 35968,
nr. 8"), so two documents on the same day are told apart.

An amendment takes effect at the vote, not when it is filed: `--na` on an
amendment shows the text without it, `--op <date of the vote>` with it. Add
`--json` (before the command) for structured output.

How to read an answer:

- Under each line, `artikel` prints its origin as a chain, oldest first:
  `nr. 2 → nr. 8, onderdeel C → nr. 11, onderdeel C (huidige formulering)`. The
  last link wrote the wording as it stands; the earlier ones are where the line
  came from before that.
- `herkomst` lists the versions of the article, each with the document that made
  that change: the nota onderdeel or amendment, at a vote the adopted
  amendments, and for the very first version the bill. An article inserted
  later begins with the nota or amendment that inserted it.
- `wet` prints the whole law at the moment, and before the text how many
  articles equal the official text and which do not. For a law through time
  the text *is* the BWB version, taken over rather than replayed, and it says so.
- `vergelijk` lists per article what changed: `~` a changed line with removed
  words as `[-…-]` and added ones as `{+…+}`, `-` a line gone, `+` a new line,
  with the documents that made the change. Articles are followed by identity,
  so a renumbered article shows as one change with its old number.
- *Let op* lines say what the text alone does not: the law was repealed, or a
  pending bill touches this article (with the command to fetch it).

Three timelines ship in `corpus-poc/wetsgeschiedenis/gegevens/`. Everything else
is fetched:

```bash
python3 $W haal 36889                    # a bill, also one still in parliament
python3 $W haal BWBR0007376 36889        # a dossier, in the context of the law it amends
python3 $W haal BWBR0007376 --keten      # the law through time (the youngest versions; --alles for all)
python3 $W haal BWBR0007376 --opbouw     # rebuild that, after fetching dossiers, to hang their toelichting on it
```

Fetched timelines land in `~/.cache/regelrecht/wetsgeschiedenis/` (override with
`WETSGESCHIEDENIS_CACHE`) and take precedence over the bundled ones;
`WETSGESCHIEDENIS_GEGEVENS` adds more directories of timelines, searched first.
A fetch is stopped after `WETSGESCHIEDENIS_TIMEOUT` seconds (default 1800).
Fetching needs `curl`, `pyyaml` and network access to wetten.overheid.nl,
repository.overheid.nl, zoek.officielebekendmakingen.nl,
repository.officiele-overheidspublicaties.nl and gegevensmagazijn.tweedekamer.nl.
A dossier takes a minute or so; a law through time about twenty seconds.

## Workflow: justifying a regelwerk article

1. **Find the law and the moment.** Take `bwb_id` and `valid_from` from the
   regelwerk. For a bill that is not law yet, take its Kamerdossier number.
2. **Get the history of the law.** `haal <BWB> --keten`, then
   `herkomst <BWB> <article> --op <valid_from>`. It lists every version of the
   article and the Kamerdossier behind each change, and says which dossiers are
   not fetched yet, with the command to fetch them.
3. **Fetch the dossier that wrote the text you model**, then `haal <BWB> --opbouw`.
   For a provision that has not changed since the law was made, that is the
   dossier of the law itself.
4. **Read per article and lid**: `artikel` for the text and where each line came
   from, `toelichting` for what was said about it, `herkomst` for how it got
   there. Ask about the lid you model, not the whole article, when the
   regelwerk models a single lid.
5. **Between two versions of a regelwerk**, run
   `vergelijk <BWB> --van <old valid_from> --tot <new valid_from>`. The articles
   it lists are the ones the new regelwerk version has to follow; the ones it
   does not list should not have changed in the regelwerk either.
6. **Cite** in the regelwerk's justification the Kamerstuk the script names, with
   its URL, e.g. *Kamerstuk 35968, nr. 3* for the memorie van toelichting. The
   script gives dossier and number; for the formal citation (*Kamerstukken II
   2021/22, 35968, nr. 3*, chamber and vergaderjaar) take them from the document
   at the URL.

## Rules

- **The letter leads.** A toelichting explains the text; it is not a norm. Never
  model a condition that exists only in the toelichting; flag it for the
  `law-letter-fidelity-audit` buckets instead (letter versus toelichting).
- **Read the check before you cite the text.** Every `artikel` and `herkomst`
  answer starts with *Controle*: whether the replayed article equals the
  official text. On *wijkt af* the replay went wrong somewhere for this
  article; cite the official text the answer prints, never the replayed text,
  and use `herkomst` only as a pointer to the documents, not as proof of what
  the article said in between.
- **"Niet afgespeeld" is a gap, not an absence.** A nota onderdeel or amendment
  that did not replay may still have changed the article. Read the instruction
  (`opdracht`) the answer prints, and the document itself.
- **Read the unlinked passages.** `toelichting` lists, under *Niet gekoppeld*,
  passages the chain could not hang on an article but that name this one.
  They are often the general part of the memorie van toelichting, where the
  intent of a whole chapter is explained. Skipping them misses exactly the
  explanation the article-by-article part refers back to.
- **Use the toelichting of the right moment.** The memorie van toelichting
  explains the bill as filed; a later nota van wijziging or amendment may have
  changed the article it describes. Check with `herkomst` that the text the
  passage discusses is still the text you model. In a law through time, a
  transition's toelichting explains that change, not the article as a whole.
- **A lid is followed by its number.** `herkomst --lid 2` shows whatever was lid 2
  at each moment; when a lid lapses and the next one moves up, that appears as
  a new version. Check with `herkomst` without `--lid` before citing a lid's
  history. Toelichting per lid does follow the passage's own numbering.
- **Passages are cut by the chain, not by the author.** A passage can run into
  the next heading of the document; read the heading the answer prints and
  stop where the subject changes.
- **Report what you could not get.** A dossier that would not fetch or replay
  is a finding for the user, with the error the script gave, not a reason to
  fall back silently on web search.

## Limits

- The chain is a proof of concept. Its replay quality differs per dossier; the
  first lines of `stukken` say how many changes replayed and how many articles
  match the official text. Treat a dossier with many mismatches as an index to
  the documents, not as a reconstruction.
- A law through time carries toelichting only for transitions whose dossier has
  been fetched (step 3 above). A bill still in parliament is not a transition
  yet; query it by its own dossier number.
- A law through time starts at the oldest version that was fetched (`--keten`
  fetches the youngest forty, `--keten --alles` all that wetten.overheid.nl
  has). That first text is cited as the BWB text of that date, not to an act:
  what made it is not replayed. A date before it is refused, not answered with
  a newer text.
- Dossiers from before 1995 exist only as scans; the chain reads them but does
  not replay them, and only in the context of the law they made
  (`haal <BWB> <dossier>`).
- The chain finds the Kamerstukken through the Tweede Kamer's Gegevensmagazijn.
  For a dossier it does not list, the fetch fails with that reason.

## Related skills

- `law-mvt-research` searches the MvT for worked examples to turn into
  scenarios. Use this skill first to find which documents discuss the article.
- `law-letter-fidelity-audit` separates the letter from the toelichting; this
  skill supplies the toelichting per lid, with its source.
- `law-version-drift-check` compares a regelwerk with the text in force on its
  `valid_from`; `artikel <BWB> <nr> --op <valid_from>` gives the same text with
  its history.

The query script is `.claude/skills/law-wetsgeschiedenis/wetsgeschiedenis.py`,
with its tests in `.claude/skills/law-wetsgeschiedenis/wetsgeschiedenis.test.py`.
The chain behind it is `packages/poc-wetsgeschiedenis/werker.py` and
`packages/poc-wetsgeschiedenis/afspeler/`.
