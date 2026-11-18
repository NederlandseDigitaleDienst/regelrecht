---
title: "Lawmaking Frontend"
description: "Visualizes the Dutch legislative process as a GitFlow diagram that plays step by step."
---

The lawmaking frontend visualizes the Dutch legislative process as an interactive flow diagram.

## Overview

- **Language**: TypeScript, Vue 3 / Vite
- **Location**: `frontend-lawmaking/`
- **Production URL**: `lawmaking.regelrecht.rijks.app`

## The GitFlow framing

The app reads the legislative process as a Git branching model. The Corpus
Juris, the law in force, is `main`. The wetgevingskalender, the proposals
formally in procedure, is `develop`. A ministry that drafts a wetsvoorstel
works in a fork, internal coordination happens on topic branches inside that
fork, and advisory bodies such as the Raad van State work in forks of their
own. Parliamentary debate is a pull-request review (in the extended view each amendment gets its own branch),
and the Koninklijk Besluit that brings a law into force is the merge from
`develop` into `main`. Meanwhile `main` keeps moving with other laws, which is
why the diagram shows rebases.

Every step carries two labels, a Git one (`git rebase main`, `merge fork →
develop`) and a legislative one (Raad van State, Eerste Kamer), so a reader who
knows either vocabulary can follow the other.

Three datasets use this model, chosen with the segmented control in the header:

- **Eenvoudig** - the basic path, from wetsvoorstel to publication
- **Uitgebreid** - the full process, with parallel advisory and consultation forks, amendment branches and a novelle
- **Wet open overheid** - the real history of the Woo (kamerstuk 33328 and novelle 35112), with phases and a year timeline

The data is static TypeScript in `src/data/` (`flowDataSimple.ts`,
`flowDataAdvanced.ts`, `flowDataWoo.ts`), typed by `src/data/types.ts`. There
is no backend.

## Toolbar

The diagram starts at step one and reveals later steps as you advance. The
header toolbar has, from left to right:

- **Step controls**: to the start, one step back, one step forward, to the end
- **Afspelen / Pauzeren**: autoplay, one step every 1.5 seconds, stopping at the end; it restarts from the beginning when pressed at the last step
- **Step counter**: current step and total
- **Zoom**: zoom in, zoom out, and reset to 100%
- **View selector**: the three datasets above; switching resets to step one

During playback the diagram scrolls along with the newest step. A legend under
the diagram names the branch colors.

## Detail sheet

Clicking a revealed node, or pressing Enter on it, opens a sheet on the right
with the Git label next to the legislative label, a description of the step,
and its type (commit, merge, review and so on). Clicking the node again, the
close button, Escape or clicking the empty canvas closes it. Opening a node
stops autoplay.

## Running locally

```bash
just dev lawmaking
```

This starts Vite on port 7500 (override with `LAWMAKING_PORT`), without
database or backend. Running `npm run dev` inside `frontend-lawmaking/` also
works; Vite then uses the port from `vite.config.ts`, 3000.

`npm test` runs `src/data/flow-data.test.ts` with Vitest, which fails when a connection or stage points at a stage or branch that does not exist.

## TypeScript setup

The app uses the setup that [Frontend Code](/guide/frontend-code) describes; `npm run typecheck` runs `vue-tsc --build`.

## Further reading

- [Deployment](/operations/deployment) - how the lawmaking site is deployed
