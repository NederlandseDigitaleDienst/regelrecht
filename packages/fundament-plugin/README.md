# RegelRecht Wetsproces, a Fundament plugin

A [Fundament](https://github.com/fundament-oss/fundament) plugin that offers
the wetsprocessysteem (`packages/wetsproces`) to the projects of a cluster. It
is installed once per cluster. After that a project member creates a
wetsproces in the console under **Wetsprocessen**, and each one runs one case
on an address of its own, with a database of its own. Several can run in one
namespace or spread over several. The model is that of Fundament's
CloudNativePG plugin, where a project makes its own databases.

The plugin is derived from the Haven+ plugin
([plugin-fundament](https://gitlab.com/commonground/haven/havenplus/plugin-fundament),
EUPL-1.2), and its console pages from those of Fundament's CloudNativePG plugin
(Apache-2.0).

## The Wetsproces resource

A wetsproces is a namespaced object of kind `Wetsproces`
(`wetsproces.regelrecht.nl/v1alpha1`, CRD in `crds/`). The plugin installs the
CRD when it starts.

```yaml
apiVersion: wetsproces.regelrecht.nl/v1alpha1
kind: Wetsproces
metadata:
  name: parkeerkaart
  namespace: tnt-rr-voorbeeld--parkeerkaart
spec:
  manifest:
    repoUrl: https://code.digitaledienst.overheid.nl/regelrecht/wetsproces-parkeerkaart
    ref: main
    path: bron.yaml
    # For a private repository: a Secret with the keys username and token.
    # credentialsSecretRef:
    #   name: parkeerkaart-repo
  # Empty runs the latest build of main; a full commit hash pins one.
  version: ''
  theme:
    accentColor: donkergroen
  hostname: parkeerkaart
status:
  phase: Ready
  url: https://parkeerkaart.cluster-a.fieldlab2026.nl/
  backofficeUrl: https://parkeerkaart-medewerkers.cluster-a.fieldlab2026.nl/
```

| Field | What it is |
|---|---|
| `manifest.repoUrl` | HTTPS URL of the git repository with the case, without credentials |
| `manifest.ref` | branch, tag or full commit, `main` by default; a commit pins the case to one version |
| `manifest.path` | path of the source manifest in that repository, `bron.yaml` by default: a manifest repository such as `regelrecht/wetsproces-parkeerkaart` has it at the root, and this repository has it at `packages/wetsproces/bron.yaml`. The case file and the articles it runs are named there, so the wizard does not ask for them |
| `manifest.credentialsSecretRef` | a Secret in the same namespace with `username` and `token`, for a private repository |
| `version` | the regelrecht commit whose wetsproces image runs, the full 40-character hash. Empty follows the latest build of main |
| `theme.accentColor` | an NLDD theme, such as `paars` or `donkergroen`; empty keeps the design system's own accent |
| `hostname` | a name under the base domain, or a domain of your own with a dot in it |

The name of the object is at most 40 characters, because it names the Helm
release and every object of the chart.

### Addresses

A hostname `zorgtoeslag` gives two addresses: citizens use
`zorgtoeslag.<domain>` and employees `zorgtoeslag-medewerkers.<domain>`. Both
stay one label under the base domain, because the wildcard record of
`cluster-a` (`*.cluster-a.fieldlab2026.nl`) covers one label only, and
`medewerkers.zorgtoeslag.cluster-a.fieldlab2026.nl` would not resolve. A
hostname with a dot in it is a domain of its own; employees then get
`medewerkers.<hostname>`, and you make the DNS records for both yourself.

Two wetsprocessen cannot share an address. The older one keeps it; the newer
one goes to `Failed` with the reason `HostnameInUse` and makes nothing until
its hostname changes or the other one is gone.

## What the plugin makes

For each Wetsproces the plugin's controller makes two Flux objects in the
namespace of the object, both owned by it:

1. a `GitRepository` named `<name>-chart` for this repository, kept to
   `charts/wetsproces` at `CHART_REF`;
2. a `HelmRelease` named `<name>` that installs that chart with values from the
   spec.

Flux installs the chart and keeps it running while the plugin pod is down. The
plugin writes the HelmRelease's `Ready` condition back into the status: phase
`Ready` once the release is ready, `Failed` once Flux has stalled on it or the
spec is refused, `Pending` before that.

### A wetsproces that does not start

Helm waits for the pods of an install until its timeout, and says nothing
before that. The plugin does not wait for it: it watches the pods of each
release, and as soon as a container keeps stopping (`CrashLoopBackOff`), an
image cannot be pulled, or a Secret the pod needs is missing, it sets the
Wetsproces to `Failed` with the reason `NotStarting`. The message names the
container and gives the last line it wrote, which the chart keeps in the
container status (`terminationMessagePolicy: FallbackToLogsOnError`). A
manifest path that is not in the repository therefore reads, within a minute:

```
container wetsproces: wetsproces: the case: the manifest does not read: ...
```

Helm goes on trying. Its timeout is 3 minutes instead of the default 5, so a
corrected spec, or a new Wetsproces in place of the failed one, takes over
sooner. Once the pods run, the status follows the release again.

The plugin does not check the path before it makes the release. That would
need git and internet access in the plugin pod, and read access to the Secret
of a private repository, for one kind of mistake; the pods report every kind.

The chart then runs what it runs by default since PR #7: a CloudNativePG
`Cluster` as the database of the release, one instance for citizens and one for
employees, each on its own address. Its hosts are listeners on the cluster's
shared Gateway (below), so a release needs no public address of its own, and
two wetsprocessen take no more IP addresses than one.

### Deleting a wetsproces deletes its data

Deleting a Wetsproces deletes its `HelmRelease` through the owner reference,
and helm-controller uninstalls the release. That removes the CloudNativePG
`Cluster`, which removes its PVCs, and with them every chronicle of the
wetsproces. There is no retention option and there are no backups. The detail
page in the console and the last step of the wizard say this.

One Secret stays behind: `<name>-letsencrypt-<acme>-account`, the key of the
ACME account, which cert-manager makes for the release's Issuer without an
owner. It holds no data of the wetsproces; delete it by hand if you like.

### Version

Without a `version` the HelmRelease leaves `image` out of its values, so the
chart's own `image.ref` counts. That line names the latest build of main:
`.forgejo/workflows/wetsproces-image.yml` sets it after every build, in a
commit of its own. Because the HelmRelease rebuilds the chart on every new
commit of its source (`reconcileStrategy: Revision`), a wetsproces without a
version rolls out each new build within the interval of its `GitRepository`.
With a `version` it runs `<DEFAULT_IMAGE>:<version>` and stays there.

The case comes from `manifest.ref`, not from the image. A case on `main` that
needs a newer server than the pinned image can stop the server at start; it
says why in its log.

## The chart behind a shared Gateway

`charts/wetsproces` reaches its hosts in one of three ways, chosen by
`gateway.mode`. The default, `listenerSet`, is the one the plugin uses on
`cluster-a` and `cluster-b`:

- a `ListenerSet` in the release's namespace adds an HTTPS listener per host to
  the cluster's shared Gateway (`public-gateway` in `istio-gateway`, the
  address the wildcard record points at). The annotation
  `cert-manager.io/cluster-issuer` makes cert-manager issue each certificate
  with the cluster's ClusterIssuer, `letsencrypt-<tls.acme>`;
- an `HTTPRoute` per instance attaches to its listener in the `ListenerSet`,
  and one on the shared Gateway's HTTP listener redirects to HTTPS;
- the release makes no Gateway, Issuer or `TLSRoute` of its own. A host can be
  claimed once per cluster; a listener on a name someone else holds shows
  `Conflicted` (`kubectl get listenerset <release> -o yaml`).

`gateway.mode: shared` is for a shared Gateway that takes no ListenerSets yet,
such as `cluster-c` for now:

- the cluster's shared Gateway has a TLS listener on port 443 that passes TLS
  through unopened. A `TLSRoute` of the release sends its hosts to the
  release's own Gateway;
- the release's own Gateway terminates TLS with a Let's Encrypt certificate per
  host and routes each host to its instance. Its Service is a ClusterIP, through
  the annotation `networking.istio.io/service-type` on the Gateway. Istio reads
  that annotation from the Gateway's own metadata; under `spec.infrastructure`
  it only reaches the Service and is ignored, which leaves a LoadBalancer with
  a public address;
- the shared Gateway's HTTP listener on port 80 carries the ACME HTTP-01
  challenge (the Issuer's solver names it) and an `HTTPRoute` that redirects
  everything else to HTTPS.

`gateway.mode: own` is the setup from before: a Gateway with a public address
of its own and an HTTP listener of its own, with `dns.*` for the records.

## Stable mode and the login through Keycloak

The chart can run a wetsproces in stable mode; the plugin does not offer it
yet. With `wetsproces.login: oidc` each instance logs in through Keycloak, with
a client of its own: `wetsproces` for citizens and `wetsproces-backoffice` for
employees, as [Authentication & Roles](../../docs/src/content/docs/auth-and-roles.md#wetsproces-roles)
describes. The chart sets `BASE_URL` to the instance's own host, so the
redirect URI each client must allow is `https://<host>/auth/callback`.
`wetsproces.mode: stable` needs this login, the database and no instance with
the surface `all`. The chart refuses a release without them, so a server that
would stop at start is never rolled out. Test mode stays the default.

`keycloak.enabled` adds a Keycloak to the release, in the same namespace, so
the login needs nothing outside it:

- a Deployment of one pod with the image `quay.io/keycloak/keycloak` pinned by
  digest, rolled out with `Recreate` because Keycloak migrates its database at
  start;
- a CloudNativePG `Cluster` of its own, `<release>-keycloak-db`;
- a host of its own, reached like the others: behind the shared Gateway
  (`listenerSet` or `shared`) `<name>-login.<domain>` beside `<name>.<domain>`,
  one label under the wildcard, and with `gateway.mode: own` `login.<host>`;
  `keycloak.host` sets another;
- the realm in a ConfigMap, which Keycloak imports at its first start
  (`start --import-realm`). A realm that exists is left alone, so a change to
  the realm reaches a new database only;
- a Job, run by Helm before every install and upgrade, that generates the
  Secret `<release>-keycloak` in the cluster: the admin password
  (`admin-password`, for the user `admin`), the client secrets and, with
  `keycloak.testUsers`, the test password. It never changes a key that exists,
  and the Secret stays behind after an uninstall. No secret is in the values or
  an image.

The instances then log in at `https://<Keycloak host>/realms/wetsproces` with
the client secrets from that Secret, without further values. The server reads
the discovery document of that host when it starts, so it needs a certificate
it trusts: Let's Encrypt staging is enough to try the routes, and the login
works on production. Until Keycloak and the databases answer, the server stops
at start and Kubernetes starts it again.

```yaml
wetsproces:
  mode: stable
  login: oidc
keycloak:
  enabled: true
  testUsers: true   # Anna Voorbeeld, Bram Proef, beslisser, admin
```

The test password is read from the cluster:

```sh
kubectl -n <namespace> get secret <release>-keycloak \
  -o jsonpath='{.data.test-password}' | base64 -d
```

For a Keycloak elsewhere, `oidc.issuerUrl` and `oidc.existingSecret` name it
and the Secret with the client secrets, in the release's namespace. The realm
is then also available as a `KeycloakRealmImport`, for a Keycloak that the
Keycloak operator runs. That operator only reads an import in the namespace of
its `Keycloak`; on `cluster-a` that is `keycloak-instances`, which Haven+
manages from its own repository, so there the import is rendered to a file and
applied by whoever manages that namespace:

```sh
helm template wetsproces charts/wetsproces --set domain=cluster-a.fieldlab2026.nl \
  --api-versions postgresql.cnpg.io/v1/Cluster \
  --set keycloak.realmImport.enabled=true \
  --set keycloak.realmImport.namespace=keycloak-instances \
  --set keycloak.realmImport.secretName=wetsproces-realm \
  --set keycloak.testUsers=true \
  --show-only templates/keycloak-realm.yaml
```

The operator fills the client secrets and the test password in from the Secret
`secretName`, in the namespace of the import.

## Console

The plugin adds **Wetsprocessen** to the project sidebar, under the heading
**RegelRecht Wetsproces** (the `displayName` of the definition), with pages of its own
under `console/`: a list, a detail page with the addresses and the status, and
a wizard that asks, one step at a time, for the case, the access, the version,
the accent color and the address, and shows what it will make before it makes
it. For a private repository the wizard makes the Secret with the token as well,
owned by the Wetsproces so the two go together.

The version step offers the latest build or a commit typed in. It does not list
the available builds: the console's content security policy lets a page reach
only Fundament's own APIs, not the registry.

The pages are plain ES modules with the NLDD components the console serves
(0.15.0). Their logic outside the DOM is in `console/wetsprocessen-body.js`,
with tests in `console-test/`.

## Configuration

The install form holds defaults only; everything per wetsproces is in its
object. The plugin reads each key as the `FUNP_<name>` environment variable.

| Key | Required | Default | What it is |
|---|---|---|---|
| `DOMAIN` | yes | | the base domain, e.g. `cluster-a.fieldlab2026.nl` |
| `DEFAULT_IMAGE` | no (advanced) | `code.digitaledienst.overheid.nl/regelrecht/wetsproces` | image repository for a wetsproces with a version |
| `CHART_REF` | no (advanced) | `main` | branch or tag of this repository that the chart is read from |
| `ACME` | no (advanced) | `production` | Let's Encrypt environment: `staging` or `production` |

The chart supports stable mode with a Keycloak of its own (see
[Stable mode](#stable-mode-and-the-login-through-keycloak)); the wizard does
not offer it yet.

v0.1 took the case, the namespace and the image in the install form. Those keys
are gone, and the config schema is closed, so an installation of v0.1 is
reinstalled rather than upgraded. The `GitRepository` and `HelmRelease` that
v0.1 made stay in their namespace until they are deleted by hand.

## Lifecycle

| Event | What the plugin does |
|---|---|
| Start, install, upgrade | Checks that Flux is there, applies the CRD and the two roles below, then runs the controller. A failed set-up is retried every 30 seconds. |
| Reconcile | Repairs the CRD and the roles and reports how many wetsprocessen are ready, every `FUNP_RECONCILE_INTERVAL` (default 5m). |
| Uninstall | Deletes the two roles. It keeps the CRD and every Wetsproces: deleting the CRD would delete every wetsproces with its data. Flux keeps them running; without the plugin nobody reconciles a change. |

To remove everything after an uninstall, delete the Wetsproces objects first
(`kubectl delete wetsprocessen --all -n <namespace>`), then the CRD.

## Permissions

Fundament makes `spec.permissions.rbac` a ClusterRole, so no rule can be kept
to the namespaces of a project. The rules in `definition.yaml` say per group
why the plugin needs them. In short: create and get the plugin's own CRD,
manage two ClusterRoles by name, manage Wetsproces objects and their status,
manage Flux `GitRepository` and `HelmRelease` objects, read pods, and create
Secrets. Pods are read to report a wetsproces that does not start. Secrets are
for the wizard: a console page acts with the rights the user and the
plugin share, so the plugin must hold every right a page uses. Neither the
plugin nor its pages read a Secret.

The two ClusterRoles aggregate into `admin` and `view`, the roles Fundament
binds project admins and viewers to. Admins may create and delete
wetsprocessen; viewers may read them.

## Develop

The plugin needs Go 1.27.3 (`go.mod`). After a change to `api/v1alpha1`,
regenerate the deep copies and the CRD with controller-gen 0.20:

```sh
controller-gen object paths=./api/... crd:crdVersions=v1 output:crd:dir=./crds
go -C packages/fundament-plugin test ./...
node --test packages/fundament-plugin/console-test/wetsprocessen-body.test.js
docker build -t wetsproces-fundament-plugin:dev packages/fundament-plugin
```

`TestChartValuesExistInChart` reads `charts/wetsproces/values.yaml`, so the
tests run from a checkout of the whole repository.

## Publish

The image goes to the Forgejo registry under the version in `definition.yaml`.
Images there are public, so nothing secret goes into one. `functl plugin
publish` takes the image by digest and injects it into the definition:

```sh
docker buildx build --platform linux/amd64 --push \
  -t code.digitaledienst.overheid.nl/regelrecht/wetsproces-plugin:v0.2.0 packages/fundament-plugin
docker buildx imagetools inspect code.digitaledienst.overheid.nl/regelrecht/wetsproces-plugin:v0.2.0
FUNCTL_REGISTRY_URL=https://marketplace-registry-api.fundament-poc.nl \
  functl plugin publish packages/fundament-plugin/definition.yaml --create --submit \
  --image=code.digitaledienst.overheid.nl/regelrecht/wetsproces-plugin@sha256:<digest>
```

`--submit` sends the version to review. The console offers only approved
versions.
