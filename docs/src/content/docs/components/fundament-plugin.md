---
title: "RegelHet: the Fundament plugin"
description: "How the RegelHet plugin gives the projects of a Fundament cluster a console where members create a RegelHet per case, and how a change in the wizard reaches the running server."
---

The Fundament plugin lets project members of a cluster run a [RegelHet](/components/regelhet) per case from the Fundament console. The plugin does not run the server itself. It turns a form into a `RegelHet` object, copies the case from a git repository into the namespace, and has Flux install the Helm chart `charts/regelhet` for it. The code is in `packages/fundament-plugin/` (Go). Field-by-field detail and the stable mode are in `packages/fundament-plugin/README.md`; this page is the way in.

## Two levels

- **The installation, once per cluster.** A `PluginInstallation`, named by Fundament from the organization and the plugin name. Its form is `configSchema` in `packages/fundament-plugin/definition.yaml`. Each key reaches the plugin pod as an environment variable `FUNP_<KEY>`.
- **A procedure, per case.** An object of kind `RegelHet` (`regelhet.regelrecht.nl/v1alpha1`) in a namespace of the project. The wizard makes it; the same object made with `kubectl` or the Kubernetes API behaves the same. The console menu calls it a procedure. A second kind, `Organisatie`, puts one login and one front in front of several procedures; see [the organisation](/components/regelhet-organisatie).

### The install form

| Key | Kind | Default | Effect |
|---|---|---|---|
| `DOMAIN` | required | none | Base domain. A procedure with hostname `x` is served on `x.<domain>` for citizens and `x-medewerkers.<domain>` for employees. |
| `DEFAULT_IMAGE` | advanced | `code.digitaledienst.overheid.nl/regelrecht/regelhet` | Image repository, used with a procedure's `version`. |
| `CHART_REF` | advanced | `main` | Branch or tag of the repository the chart is read from. |
| `ACME` | advanced | `production` | Let's Encrypt `production` or `staging`. Stable mode works with production only. |

Go reads and validates these in `packages/fundament-plugin/config.go`. The test `TestConfigSchemaMatchesPluginConfig` in `definition_test.go` fails when the schema and the Go struct drift apart.

### The fields of a procedure

The type is `packages/fundament-plugin/api/v1alpha1/regelhet_types.go`; the CRD in `packages/fundament-plugin/crds/` is generated from it with `controller-gen`.

| Field | Meaning |
|---|---|
| `hostname` | Required. A name under the base domain, or a domain of your own with a dot in it. |
| `manifest.repoUrl` | https address of the repository with the source manifest, without credentials. |
| `manifest.ref` | Branch, tag or full commit. Default `main`. |
| `manifest.path` | Path of the manifest. Default `bron.yaml`. |
| `manifest.credentialsSecretRef` | Secret with `username` and `token` for a private repository. |
| `casus` | The case to run, by the name of its case file, when the manifest makes several. |
| `version` | Full 40-character regelrecht commit. Empty follows main. A version also takes the chart from that commit. |
| `theme.accentColor` | One of the NLDD theme colors. |
| `title` | The name the screens show for the procedure. |
| `auth.mode`, `auth.testAccounts` | `test` (the test login, default) or `stable` (a Keycloak of its own on `<name>-login`). |
| `fsc4b.enabled`, `fsc4b.origins` | [FSC voor Burgers](/components/regelhet-fsc4b) on the demo Stelsel, and the browser apps that may call it (at most 20). Test mode only. |
| `partij.cel`, `partij.uitvoerder` | Run as the server of one external party of another procedure's case, such as the GGD; see [a link between servers](/components/regelhet-koppeling). Test mode only, and not together with `fsc4b` or `organisatie`. |
| `fsc.outway`, `fsc.grants` | Reach linked procedures through this one's OpenFSC outway. No wizard field; set it with `kubectl`. |
| `organisatie` | The organisatie whose front lists this procedure. |

The status carries `phase` (`Pending`, `Ready` or `Failed`), the addresses (`url`, `backofficeUrl`, `loginUrl`, `fsc4bDiscoveryUrl`), `snapshot` (the commit the release runs), `token` (`Present` or `Absent`), and a `message` that says why the phase is what it is. An executor also lists the parties its case asks something of (`askedParties`) and the ones that run as a procedure of their own (`parties`), under the condition `PartiesLinked`; `OrganisatieGevonden` says the same for its organisatie.

## What `definition.yaml` decides

Everything the console shows of the plugin is in `packages/fundament-plugin/definition.yaml`: `configSchema` (the install form), `permissions.rbac` (what the plugin may do on the cluster; Digilab reviews it for every version), `menu.project` (the entries Procedures and Organisaties), `customComponents` (the HTML pages for list, detail and create), `allowedResources` (what those pages may do through the console) and `uiHints` (status labels: `Ready` is shown as Draait, `Failed` as Mislukt, anything else as Bezig).

The pages are in `packages/fundament-plugin/console/`, embedded in the binary (`console.go`) and served by the plugin. They load the plugin SDK and the NLDD design system from the console (`_shared.js`) and navigate with `postMessage`. The console forbids inline scripts and styles, so all code is in separate `.js` files. The display name in the marketplace is separate from the version: publishing a new version does not change it.

## The wizard

The wizard is `packages/fundament-plugin/console/regelhets-create.js`. Its choices become the object in `buildRegelHet` in `regelhets-body.js`.

| Step | Asks | Ends up in |
|---|---|---|
| Casus | repository, ref, path, and the case when the manifest makes several | `manifest.*`, `casus` |
| Toegang | public or private with a token; test or stable; test accounts; FSC voor Burgers and the apps that may call it | `credentialsSecretRef`, `auth`, `fsc4b` |
| Versie | latest, or a fixed commit | `version` |
| Uiterlijk | accent color and title | `theme.accentColor`, `title` |
| Adres | hostname and namespace | `hostname`, name and namespace of the object |
| Rol | executor, or an external party of another procedure; skipped in stable mode | `partij` |
| Organisatie | none, a new one, or an existing one; skipped in stable mode and for a party | `organisatie` |
| Controleren | summary | nothing |

The object name is the first label of the hostname. Most fields at their default are left out; `manifest.ref` and `manifest.path` are always written. For a private repository the wizard then creates the Secret `<name>-repo`, owned by the procedure and annotated for deletion after the copy. If that fails it deletes the procedure again, so none is left waiting for a token.

## Adding a field, from wizard to app

A new field goes through six layers. `theme.accentColor` is the example.

1. **Type.** Add the field in `api/v1alpha1/regelhet_types.go` and run `controller-gen` (command in the README) to update the CRD in `crds/`.
2. **Wizard.** Add the question to a step in `console/regelhets-create.js` and set it in the object in `console/regelhets-body.js`.
3. **Reconcile.** Pass it on as a Helm value in `chartValues` in `resources.go`.
4. **Chart.** Give it a default in `charts/regelhet/values.yaml` and an entry in `values.schema.json`.
5. **Template.** Turn it into an environment variable in `charts/regelhet/templates/deployment.yaml`, here `REGELHET_ACCENT_COLOR`.
6. **Server.** Read the variable in `packages/regelhet/src/config.rs`.

`TestChartValuesExistInChart` in `plugin_test.go` fails when the plugin passes a value the chart does not have; Helm would otherwise drop it without a word. `TestFscValuesExistInTheChart`, `TestFsc4bOriginsReachTheChart` and `TestOrganisatieValuesExistInChart` guard the same for their own values. The values of a party go through `partyValues` in `parties.go`. A field that belongs to the installation only is shorter: `configSchema`, the struct in `config.go`, and use in the reconcile.

## What happens on the cluster

A procedure goes from the wizard to pods in this order: the `RegelHet` object, the plugin controller, a snapshot of the case, a Flux `GitRepository` and `HelmRelease`, then pods for citizens and employees (a party has one, for its staff), a CloudNativePG database and routes, and the status written back into the object.

- **The snapshot.** The plugin copies the case once, at creation and after a change of `manifest` (`snapshot.go`, `snapshot.sh`). A Job named `<name>-snapshot-<hash>` clones one commit with the token, checks that the manifest and the directories it names are in the repository, and writes a tar.gz of those files to its log. The plugin reads the log, stores the archive in ConfigMaps (parts of at most 900 KiB, five at most) and deletes the Job. The chart unpacks the ConfigMaps into `/case`, so the pods need no token and no network access to git.
- **A new snapshot on demand.** Any new value of the annotation `regelhet.regelrecht.nl/snapshot` takes one, for instance to pick up the latest commit of a branch. While a new copy is taken or fails, the release keeps running the old one; the old ConfigMaps go once the release runs the new ones.
- **The token.** If the Secret carries `regelhet.regelrecht.nl/delete-after-snapshot: "true"`, the plugin deletes it after the copy. A later new snapshot needs a new Secret with the same name.
- **GitRepository `<name>-chart`.** Fetches `charts/regelhet` from the fork, at `CHART_REF`. With a `version` the chart comes from that commit, so chart and image match.
- **HelmRelease `<name>`.** Installs the chart with the values from `chartValues`. Interval 10 minutes, timeout 3 minutes, three remediation retries on install and on upgrade.
- **Ownership.** Everything carries an owner reference to the `RegelHet`. Deleting the procedure lets Flux remove the release, the database included. The one exception is the Keycloak Secret of stable mode, which the plugin deletes itself.

An organisatie adds a release of its own. An external party is a procedure of its own with `spec.partij`: the executor finds it, lists it in `status.parties` and names it as a peer of the link in its release. Both are described in the README.

## Failure reasons

A refused or failing procedure shows `Failed` or `Pending` in `status.phase` and the reason in the `Ready` condition and `status.message`.

| Reason | Phase | When |
|---|---|---|
| `InvalidSpec` | Failed | A field is wrong. |
| `HostnameInUse` | Failed | Another procedure or organisatie already has the address. The older wins; at the same second, the first by namespace and name. |
| `NameInUse` | Failed | A procedure and an organisatie with one name in one namespace would share a Helm release. |
| `StableNeedsProductionCerts` | Failed | Stable mode on an installation with `ACME` staging. |
| `TokenMissing` | Pending | A private repository whose token Secret is not in the namespace. Checked again every 15 seconds. |
| `SnapshotPending` | Pending | The copy of the case is being taken. |
| `SnapshotFailed` | Failed | The Job failed or its result could not be stored. With an earlier copy the release keeps running it, and the message names its commit. The plugin tries again after 5 minutes, so a corrected token is picked up. |
| `ChartNotInCommit` | Failed | `version` names a commit older than the chart. |
| `ReleaseStalled` | Failed | Flux gave up on the HelmRelease. |
| `NotStarting` | Failed | A container keeps failing: wrong image, missing Secret, failed init container, or a server that crashes. A crashing server counts after 5 restarts or when its pod is 3 minutes old; the rest at once. The message names the container and its last log line. |
| `OrganisatieNotFound`, `OrganisatieInOtherProject`, `OrganisatieRefused` | in the condition `OrganisatieGevonden` | The named organisatie does not exist, lies in another Fundament project, or is itself refused. The procedure runs on its own. |
| `OrganisatieModeMismatch` | Failed | The organisatie runs the test login while the procedure is in stable mode. |
| `PartyPending` | Pending | A party waits for its executor to run. |
| `ExecutorNotFound`, `ExecutorInOtherProject`, `ExecutorNotUsable` | Failed | The procedure a party names as its executor does not exist, lies in another project, or cannot be one: it is a party itself, runs in stable mode, or is refused. |
| `PartyNotAsked`, `PartyTaken` | Failed | The executor's case asks nothing of the party's cell, or another procedure already runs as that party. |

The definitions are in `controller.go`, `organisatie.go`, `parties.go`, `pods.go` and `snapshot.go`.

## Releasing a new version

1. Raise `metadata.version` in `definition.yaml` and merge to main.
2. Push the tag `regelhet-plugin-vX.Y.Z` on that commit. The workflow `.forgejo/workflows/fundament-plugin-image.yml` builds the image `regelhet-plugin` and pushes it as `:vX.Y.Z` and `:<commit>`. It fails when the tag differs from `metadata.version` or the version already exists in the registry. Started by hand on main it builds a test image tagged with the commit only.
3. Publish with the digest of the image (a first publication adds `--create`):
   ```
   FUNCTL_REGISTRY_URL=https://marketplace-registry-api.fundament-poc.nl \
     functl plugin publish packages/fundament-plugin/definition.yaml \
     --submit --image=code.digitaledienst.overheid.nl/regelrecht/regelhet-plugin@sha256:<digest>
   ```
   Publishing is a manual step because it needs a Fundament API key.
4. Digilab reviews the version. The console offers approved versions only.
5. To upgrade, recreate the PluginInstallation: the version it points at cannot be changed. Delete the installation and create it again with the same settings. Procedures and their data stay.

## Fundament documentation

- [Plugins for developers](https://docs.fundament-poc.nl/docs/developer/plugins): PluginInstallation, what the controller creates, permissions, removal.
- [Writing a plugin](https://docs.fundament-poc.nl/docs/developer/plugins/writing-a-plugin): the structure of `definition.yaml` and the rules for `configSchema`.
- [Console integration](https://docs.fundament-poc.nl/docs/developer/plugins/console-integration): the iframe, `postMessage` and the proxy to the Kubernetes API.
- [Custom pages](https://docs.fundament-poc.nl/docs/developer/plugins/custom-ui): file layout, the SDK and the NLDD version.
- [Testing and publishing locally](https://docs.fundament-poc.nl/docs/developer/plugins/testing-plugins-locally): a k3d sandbox with a registry of its own, where a draft or pending version installs without review when the sandbox's own organization published it.
- [Plugins for users](https://docs.fundament-poc.nl/docs/user/plugins) and [functl](https://docs.fundament-poc.nl/docs/user/functl).
