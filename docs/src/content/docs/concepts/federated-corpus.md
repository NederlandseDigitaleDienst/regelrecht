---
title: "Federated Corpus"
description: "How each authority can maintain its own regulations in its own repository, discovered through a registry."
---

Dutch legislation is produced by many authorities: Parliament, ministers, 342 municipalities, 12 provinces, 21 water boards. Maintaining all regulations in a single repository does not match this reality.

The federated corpus model lets each authority maintain their own regulations in their own Git repository. The [corpus library](/components/corpus) discovers and loads laws from all sources through a registry, and hands them to the engine.

## The registry

A `corpus-registry.yaml` file lists all regulation sources. An illustrative example, with a municipal source next to the central corpus (the registry checked into this repository points at a different branch and priority):

```yaml
schema_version: "1.0"
sources:
  - id: minbzk-central
    name: "MinBZK Central Corpus"
    type: github
    github:
      owner: MinBZK
      repo: regelrecht-corpus
      branch: main
      path: regulation/nl
    scopes: []
    priority: 1

  - id: amsterdam
    name: "Gemeente Amsterdam"
    type: github
    github:
      owner: gemeente-amsterdam
      repo: regelrecht-amsterdam
      branch: main
      path: regulation/nl
    scopes:
      - type: gemeente_code
        value: "GM0363"
    priority: 10
    auth_ref: amsterdam
```

Each source declares:
- **Where** to find the regulations (a GitHub repository or local directory)
- **Scope** (jurisdiction claims, like `gemeente_code: GM0363` for Amsterdam)
- **Priority** (lower number = higher priority, used when the same law appears in multiple sources)

## How it works

The corpus library merges laws from all registered sources into a single corpus at load time. When the same law appears in more than one source, the source with the lowest priority value wins (`SourceMap` in `packages/corpus/src/source_map.rs`); two sources with equal priority and the same `$id` are an error. Scopes do not filter which laws load or apply.

Scopes are claims, and the loader checks them against what a source delivers. It warns when a scoped source provides a regulation for a different municipality or water board, when it provides a national regulation (a decentral source can fill in national law but not replace it), and when it provides a regulation whose `regulatory_layer` it does not recognize. The check covers every version a source supplies, including one from a source that loses on priority. It reads the regulation body, so a GitHub source that is only enumerated by path, without fetching the files, is not checked. The check warns and does not refuse.

The `implements` mechanism from [Inversion of Control](./inversion-of-control) works across repositories. Diemen's afstemmingsverordening (in Diemen's repo) can implement open terms from the Participatiewet (in the central repo). The engine does not care which repository a file came from.

## Local overrides

Developers can add a `corpus-registry.local.yaml` (gitignored) at the project root to add personal sources during development. It extends the main registry without affecting the shared configuration.

A common use case is wiring a sibling checkout of a private corpus repository into the editor without copying files or committing paths:

```yaml
schema_version: "1.0"
sources:
  - id: my-private-corpus
    name: "Private corpus (local checkout)"
    type: local
    local:
      path: ../regelrecht-private/corpus/regulation/nl
    scopes: []
    priority: 0
```

Paths in `local.path` resolve relative to the project root; absolute paths also work.

After adding sources or changing files in an external source, re-run `node frontend/scripts/copy-laws.js` (or restart the frontend so the `predev` hook fires) to refresh `frontend/public/data/`.

## Authentication

Credentials for private repositories are stored separately from the manifest. The registry uses `auth_ref` identifiers that map to environment variables (`CORPUS_AUTH_{ID}_TOKEN`) or a `corpus-auth.yaml` file. No secrets in the manifest itself.

## Editor integration

An authority can work on its ruleworks in the web editor without a development environment. The editor works inside a traject, which has its own branch, and every save is a commit on that branch through the GitHub Contents API. It does not open a pull request.

A traject can point at the authority's own repository, private if need be, instead of the central corpus. Which token writes there, a service token per repository or the user's own GitHub account, is described on [Private-repo trajects](/operations/private-repo-trajects).

## What is not built

The checked-in `corpus-registry.yaml` lists one source, the central corpus; further sources come from a local registry during development. No repository of another authority is registered. Scopes are claims the loader warns about; they do not decide which ruleworks apply to a case. That is decided by the execution itself, for example by the `gemeente_code` with which [Inversion of Control](./inversion-of-control) selects a municipal ordinance.

## Further reading

- [Inversion of Control](./inversion-of-control) - how `implements` works across repositories
- [Competent Authority](./competent-authority) - who decides, and what crossing an organizational boundary means for execution
- [RFC-010: Federated Corpus](/rfcs/rfc-010) - the full design specification
