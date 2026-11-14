---
title: "Notes and Annotations"
description: "Stand-off notes that anchor to legal text by quote rather than position, so they survive renumbering and minor edits, and how their authority is derived."
---

People need to write things about a law: a comment explaining a term, a link from a sentence to the rule that executes it, a flag on an ambiguous norm. The hard part is that law text changes. Articles get renumbered, sentences get reworded, and a note pinned to "article 2, character 140" breaks the moment an article is inserted above it.

RegelRecht keeps notes **stand-off**: stored separately from the law, never embedded in it, so the verbatim source text stays untouched. A note anchors to the text by quoting it, following the W3C Web Annotation Data Model.

## Anchoring by quote

A note targets text through a `TextQuoteSelector`: the exact quote, plus a little prefix and suffix for context. From a real note on the Wet op de zorgtoeslag (`corpus/annotations/wet_op_de_zorgtoeslag/annotations.yaml`):

```yaml
- type: Annotation
  motivation: linking
  creator: Dienst Toeslagen
  target:
    source: regelrecht://wet_op_de_zorgtoeslag
    selector:
      type: TextQuoteSelector
      exact: zorgtoeslag
      prefix: 'heeft de verzekerde aanspraak op een '
      suffix: ' ter grootte van dat verschil'
  body:
    type: SpecificResource
    source: regelrecht://wet_op_de_zorgtoeslag/hoogte_zorgtoeslag
    purpose: linking
```

Because the anchor is the content, not a line number, the note follows its text. Insert a new article 1a and renumber the target to 4a, and the note still resolves to 4a. The resolver (`packages/engine/src/annotation/`) reports one of three outcomes (`MatchStatus`):

- **Found**: exactly one location matches.
- **Ambiguous**: the quote occurs more than once with no context to separate the hits (the common word "verzekerde" three times in a sentence). Adding a prefix or suffix disambiguates.
- **Orphaned**: the text is gone. The note is never discarded; it is marked orphaned so a human can re-anchor it.

When wording drifts slightly (a Staatsblad amendment swaps a few words), exact matching fails but **fuzzy matching** recovers it: normalized Levenshtein similarity above a threshold (currently 0.7) resolves as a fuzzy match with a confidence below 1.0; a wholesale rewrite falls below the threshold and orphans rather than anchoring to the wrong place. A note may also include an optional article hint to try first; an outdated hint falls back to a full search. The behavior is pinned by `bdd/conformance/notes.feature`.

## What a note says

The `motivation` field is the kind of note. The W3C model defines thirteen motivation values (Web Annotation Data Model, §3.3.5); four matter for law work, and three of those are in use in the corpus today:

- **linking** ties a sentence to the `machine_readable` element that executes it, making the chain from text to logic auditable.
- **commenting** is a plain explanation for readers.
- **questioning** flags an open or ambiguous norm. These draw their term from a controlled vocabulary (`corpus/annotations/_vocabulary/ambiguity.yaml`) and move through a small workflow: an open question becomes `resolved` once the implementing regulation is found and modelled.

The fourth, **tagging** (classification), is available but not yet used in the corpus.

## Storage, federation, and authority

Notes live in sidecar YAML keyed by the law's `$id`, not its file path (`corpus/annotations/{law_id}/annotations.yaml`). In a source repository the sidecar sits at the repository root, in `annotations/{law_id}/annotations.yaml`, even when the source's regulations live in a subdirectory such as `regulation/nl` ([RFC-018](/rfcs/rfc-018)). They follow the same [federated](./federated-corpus) model as the corpus: any organization can keep notes on a law in its own repository, and the editor's write path is append-only so parallel edits do not clobber each other.

By design, a note's **authority is derived at display time**, not declared. This is not built yet. The resolver would compare the note's `creator` against the article's [competent authority](./competent-authority):

- **authoritative** when the creator is the competent body itself (Dienst Toeslagen on its own act).
- **advisory** when the creator is another government organization.
- **generated** when tooling produced it.
- **personal** when an individual wrote it.

The same note text means something different depending on who wrote it, and the derived level records which case applies.

The engine exposes resolution to the browser through the WASM bindings `resolveNote` and `resolveNotes` (`packages/engine/src/wasm.rs`), so the editor anchors notes against the live text on screen.

## Further reading

- [Competent Authority](./competent-authority) - the basis for a note's derived authority
- [Federated Corpus](./federated-corpus) - how notes are distributed
- [RFC-005: Stand-off Notes](/rfcs/rfc-005) and [RFC-018: Note Infrastructure](/rfcs/rfc-018) - full specifications
