---
title: "Private-repo trajects"
description: "How a traject in the editor is linked to its own (private) GitHub repository instead of the central corpus repository, and which token writes to it."
---

A *traject* (a working context in the editor, with its own members and branch) can be linked to **its own GitHub repository** instead of the central `MinBZK/regelrecht-corpus`. This suits organizations and teams that want to keep their regulations in a private repository while keeping the editor and its commit attribution.

What you need to do depends on your role: participant, traject owner, or operator of the RegelRecht deployment. It also depends on which of two write modes the deployment runs in.

## The two write modes

The editor writes to a traject repository with one of two kinds of token.

- **Service token.** A fine-grained GitHub Personal Access Token (PAT) per repository, configured by the operator as an environment variable (or as an entry in the `corpus-auth.yaml` file that `CORPUS_AUTH_FILE` points to). Every participant writes through that one token. This is the default.
- **Personal GitHub account.** Each user links their own GitHub account in the editor (Settings, under "Koppelingen", button "Koppelen") and writes with that. This needs a GitHub OAuth App on the deployment and is switched on for the whole environment, either with `GITHUB_USER_TOKEN_REQUIRED=true` or with the switch "Met eigen GitHub-account schrijven" in the editor settings (the feature flag `github.user_oauth`, off by default).

A configured service token always wins. The personal token is only used for a repository that has no service token. In personal mode a traject on a repository without a service token therefore needs no operator at all: a user who has push rights on the repository and has linked their account can create the traject and save to it. Without a linked account, or with an expired link, a save returns HTTP 428 and the editor sends the user through the linking flow.

Neither token is stored in the database. The service token lives only in the editor's runtime environment. The personal token is sealed (encrypted and authenticated with `GITHUB_TOKEN_ENC_KEY`) into an HttpOnly cookie that lasts for the browser session and is bound to the editor account, so the browser holds it but page scripts cannot read it.

## Who does what

With a service token:

| Role | Action |
|---|---|
| Repository owner (or admin) | Creates a GitHub PAT with write access to the target repository |
| Operator | Configures the PAT as an environment variable on the editor deployment (once per repository) |
| Traject owner | Creates the traject in the editor with the repository fields filled in |
| Participants | Log in through SSO; saves are attributed to their own name |

In personal mode the first two rows fall away for repositories without a service token, and every participant links their own GitHub account instead.

## Step 1: Prepare the GitHub repository

The target repository must meet two conditions before it can be linked:

1. **The base branch already exists**, with at least one commit. When the traject is created, the editor creates its own branch (`traject/<slug>-<id>`) from that base on GitHub. If the base does not exist yet (for example an empty `main`), validation fails. If needed, push an empty README commit to `main` to initialize it.
2. **The token has push rights** on the repository: the PAT account for a service token, or the user's own account in personal mode.

The repository can be public or private. Creating a traject always requires a token with push rights, also for a public repository. Afterwards, without a service token and outside personal mode, reads of a public repository go out unauthenticated, with GitHub's lower rate limits. Writes always need a token.

## Step 2: Create a GitHub PAT (service-token mode)

Generate a **fine-grained PAT** at [github.com/settings/personal-access-tokens/new](https://github.com/settings/personal-access-tokens/new):

- **Resource owner**: the owner of the target repository (a personal account or an organization)
- **Repository access**: "Only select repositories", and pick exactly the target repository. Do not choose "All repositories"; it keeps the damage of a leak small.
- **Repository permissions**:
  - `Contents`: **Read and write**, for commits and for creating the traject branch
  - `Metadata`: **Read** (required and selected automatically)
- **Expiration**: choose a period that fits your security policy. When the PAT expires, the editor refuses reads and writes and the operator has to replace the environment variable.

The editor commits directly to the traject branch through the GitHub Contents API and opens no pull request, so the `Pull requests` permission is not needed.

Store the PAT value somewhere safe right away; GitHub shows it only once.

Classic PATs work too, but they grant far too much (every repository the account can reach). Fine-grained is strongly recommended.

## Step 3: The operator configures the environment variable

Give your operator three things:

1. The `owner/repo` of the target repository (for example `acme/regelrecht-lokaal`)
2. The PAT value
3. A short description of its purpose, for the audit trail

The operator sets the PAT as an environment variable on the editor deployment, named:

```
CORPUS_AUTH_<OWNER>_<REPO>_TOKEN
```

where `<OWNER>_<REPO>` is a **deterministic slug** of the coordinates: lowercased, every run of non-alphanumeric characters becomes a single `-` (leading and trailing dashes are dropped), and the result is uppercased with dashes turned into underscores. Examples:

| owner/repo | Environment variable |
|---|---|
| `MinBZK/regelrecht-corpus` (the central repository) | `CORPUS_AUTH_MINBZK_CENTRAL_TOKEN` |
| `acme/regels` | `CORPUS_AUTH_ACME_REGELS_TOKEN` |
| `acme/regelrecht-private-test` | `CORPUS_AUTH_ACME_REGELRECHT_PRIVATE_TEST_TOKEN` |

The central writable repository (`MinBZK/regelrecht-corpus`) does **not** use the derived slug but the fixed auth ref `minbzk-central`, so its variable is `CORPUS_AUTH_MINBZK_CENTRAL_TOKEN`, not `CORPUS_AUTH_MINBZK_REGELRECHT_CORPUS_TOKEN`. A user repository whose name would slug to `minbzk-central` is refused. Writable sources resolve their token strictly, without falling back to the shared `CORPUS_GIT_TOKEN`: a deployment that only sets `CORPUS_GIT_TOKEN` cannot push to the central repository. When no token is found, the editor log names the variable it looked for.

How to set an environment variable depends on the platform (ZAD, Kubernetes, docker-compose). After the change the editor pod has to restart to pick up the new variable. **One variable per repository**: when several trajects point at the same repository, one configuration covers them all.

If you create the traject before the variable exists, and the deployment is not in personal mode, the error message shows the exact variable name the editor expects. Pass that name to your operator verbatim.

## Step 4: Create the traject

In the editor:

1. Choose **"Nieuw traject…"** in the traject menu.
2. Fill in **Naam** (name) and, optionally, **Beschrijving** (description).
3. Turn on the switch **"Eigen GitHub-repo (i.p.v. standaard MinBZK-repo)"**.
4. Fill in **Repo owner**, **Repo** and **Base branch** (prefilled with `main`). Set **Subpath** if the YAML files live in a subdirectory; leave it empty for the repository root. The subpath applies to rulework versions and scenarios. Notes do not follow it: RFC-018 keeps them at the repository root, in `annotations/{law_id}/annotations.yaml` outside the subpath, and that is where the editor reads and saves them.
5. Click **"Maak traject aan"**.

Older editor versions saved notes under the subpath instead, in `{subpath}/annotations/{law_id}/annotations.yaml`. When a traject repository has no notes file at the root for a law but does have one at that old location, the editor shows the old file. The next save of a note on that law appends to it and writes the result to the repository root. The old file stays where it is; remove it by hand once the root file exists.

Before anything is stored, the editor runs a **preflight check** against the GitHub API:

- Does the repository exist, and can the token see it?
- Does the token have push rights?
- Does the base branch exist?

It then creates the traject branch on GitHub. If any of these steps fails you get a specific error (see [error messages](#error-messages-and-what-they-mean) below), no row is written to the database, and you can retry straight after fixing the cause. If the database write fails after the branch was created, the branch stays behind on GitHub; the editor logs it but does not remove it.

## Changing the subpath later

If the regulation moves to a different place in the repository over time, for example because working folders were added next to the corpus, the traject owner changes the subpath under **Instellingen → Algemeen → Subpath**. Leaving it empty means the repository root.

The subpath is the root of everything the editor reads and writes on this repository: laws, scenarios, annotations (`annotations/<law>/annotations.yaml`) and documents (`documents/<traject-ref>/`). After a change the editor looks for them under the new path. Existing annotations and documents do not move along by themselves; you move them yourself on the traject branch. Whatever lies outside the subpath is left alone by the editor; it just no longer counts as regulation.

Only the traject owner can do this, and only on an own repository: trajects that write to the central corpus keep the fixed path `regulation/nl`.

## Changing the repository later

When the repository moves, for example to another GitHub organization, the traject can follow it under **Instellingen → Algemeen → Repo**: enter the new owner and repository name, separated by a slash, and choose **Repo wijzigen**. Only someone with the `editor-admin` role can do this; being the traject owner is not enough, because the repository decides where the platform's tokens are used. An `editor-admin` who is not a member of the traject cannot open its settings, but can make the same change through `PUT /api/trajects/{id}/repo`.

Before anything is stored, the editor runs the same checks as when creating a traject, against the new repository: there must be a token, it must have push rights, and the base branch must exist. The service token is looked up under the name derived from the new coordinates, so configure `CORPUS_AUTH_<NEW_OWNER>_<NEW_REPO>_TOKEN` first. Without one, personal mode checks the GitHub account of the person making the change, and every member who saves later needs push rights on the new repository too. The traject branch is created on the new repository when it is missing there; after a GitHub transfer it already exists. Branch, base branch and subpath stay as they are.

Trajects on the central corpus keep their repository.

## How commit attribution works

Every save in a traject produces a commit on the traject branch. Who it is attributed to depends on the token:

- **Service token**: author and committer are both set to your name and email from the SSO session (Keycloak). GitHub still records the token's account as the pusher.
- **Personal token**: the editor sets no author or committer, so GitHub attributes the commit to your linked GitHub account.

For the display on GitHub: if the email from your SSO session is a verified address on your GitHub account, GitHub shows your avatar and name on the commit and links to your profile. If it is not, GitHub shows the email as plain text, without a profile link, but still with the correct name.

The editor refuses saves when Keycloak's `email_verified` claim is not `true`. This prevents a user with an unverified email claim from committing under that name. If you get a 403 for this, log in again (which refreshes the claim) or ask your administrator to check your account settings.

## Error messages and what they mean

When creating a traject fails, the editor shows a specific message. The messages are in Dutch; the most important ones:

| Status | Message | What to do |
|---|---|---|
| 503 | "deze repo is nog niet door je beheerder geconfigureerd (verwacht env var X)" | No service token and not in personal mode. Ask the operator to configure the variable named in the message |
| 428 | "Koppel je GitHub-account …" or "Je GitHub-koppeling is verlopen …" | Personal mode: link (or relink) your GitHub account |
| 502 | "het token van je beheerder wordt door GitHub geweigerd" | The token is expired or invalid; ask the operator to replace it |
| 403 | "het geconfigureerde token heeft geen schrijftoegang tot deze repo" | The PAT lacks `Contents: write`, or its account is not a collaborator on the repository |
| 404 | "repo X bestaat niet of het token kan 'm niet zien" | A typo in owner or repo, or the fine-grained PAT is not linked to this repository |
| 404 | "branch 'X' bestaat niet op owner/repo" | The base branch does not exist yet; initialize it with at least one commit |
| 400 | "repo_owner / repo_name mogen alleen letters, cijfers, en de tekens '-', '_' en '.' bevatten" | Owner or repository name contains invalid characters (the subpath has a similar rule, and may not contain `..`) |
| 400 | "base_branch bevat tekens die niet zijn toegestaan in een git branch-naam" | The base branch is not a valid git branch name |
| 400 | "… moeten alle drie worden meegegeven …" | The own-repository switch is on but owner, repository or base branch is empty |
| 502 | "kon de traject-branch 'X' niet aanmaken op owner/repo …" | The traject branch could not be created; nothing was stored |
| 502 | "onverwacht antwoord van GitHub bij repo-validatie" | A temporary GitHub problem; try again |
| 503 | "kon GitHub niet bereiken om de repo te valideren" | GitHub could not be reached; try again |

Saves can fail while editing as well. Most messages are the same; for a 403 about a verified email address (*geverifieerd e-mailadres*), logging in again usually fixes it.

## Limitations

- **A service token is per repository, not per user.** All participants in a traject commit through the same token, under their own name. GitHub's push events always show the token's account as the pusher, which is not enough for a strict per-user audit; use the editor API's audit logs or the commit history of the traject branch for that. Personal mode does not have this limitation.
- **Replacing an expired service token is operator work.** The editor refuses reads and writes once GitHub rejects the PAT, and the operator has to replace it. Plan for this. An expired personal link is fixed by the user relinking.
- **Service-token mode is not self-service.** Every new repository needs an operator to configure a variable. Personal mode removes that step, at the cost of every user needing their own GitHub account with push rights.
- **No tokens in the database.** A deliberate design choice: a bug, breach or insider with database access cannot exfiltrate tokens.

## Working next to a clone

A dossier corpus is often written outside the editor, for example by an agent in a local clone. The editor can open such a corpus and edit it without the two getting in each other's way. RFC-057 has the design.

The rule is two branches, never one shared branch:

- The **working branch** (for example `session-3`) belongs to the clone. The editor never writes to it.
- The **traject branch** (`traject/<name>-<id>`) belongs to the editor. Every save is one commit on it.

### Loading

Under "Nieuw traject", switch on the own repository, fill in owner, repository and the subpath if there is one, and press **Controleer corpus**. The check creates nothing. It fills the list of branches and reports:

- how many ruleworks sit under the subpath, and which of their versions (one dated file each) will not open in the editor, with the reason;
- which ruleworks another source also serves (in the traject, the repository wins);
- whether notes sidecars sit in `annotations/` at the repository root while the editor looks for them under the subpath. Move them in that case, or they stay invisible.

Then pick the working branch and create the traject.

### Staying level

While a traject is open in the editor, it checks every three minutes, and as soon as the tab comes back into view, whether the working branch has anything new. If it does, the editor brings it in on its own. When the traject has no commits of its own, the traject branch is simply moved forward. Otherwise GitHub merges the two with an ordinary git merge, which settles everything that does not touch the same lines, also within one file: you edit article 3 in the editor, the agent edits article 7, and both land.

After a sync the editor reloads the open article, unless it holds unsaved edits. Those stay. If the merge touched the same file, the save reports that the file changed in the meantime instead of replacing the edits. The **Bijwerken** button in the library does the same on request.

Only when both sides change the same lines does GitHub refuse the merge, and both branches stay as they were. Git cannot settle that without discarding one side, so the editor does not try. The notice names the files, and the editor tries again only once one of the branches moves. Resolve it in the clone:

```bash
git fetch origin
git checkout <working-branch>
git merge origin/traject/<name>-<id>
# resolve the conflicts, commit, and publish the working branch
```

### Back to the clone

**Voorstel naar werkbranch** opens a pull request from the traject branch to the working branch. In the clone, `git merge origin/traject/<name>-<id>` does the same. A routine that works well: start every session in the clone with `git pull` and a merge of the traject branch.

### Comments survive a save

On a save the editor rewrites only the changed part of an article. YAML comments, line folding and key order elsewhere in the file stay byte for byte as they were, so a save is a small diff that merges cleanly with work from the clone. Only for a document shape the editor cannot patch does it fall back to writing the whole file again.
