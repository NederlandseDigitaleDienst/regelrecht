# Wetsproces Fundament plugin

A [Fundament](https://github.com/fundament-oss/fundament) plugin that runs the
wetsprocessysteem (`packages/wetsproces`) for one case on a Fundament cluster.
You pick the case in the install form: a public git repository, a branch or
tag, and the path of its source manifest (`bron.yaml`).

The plugin does little itself. It expects Flux on the cluster, as Haven+
installs it, and a namespace that Fundament made (console or functl), named in
`NAMESPACE`. A namespace made with kubectl stays invisible to Fundament, and
its project members get no rights in it, so the plugin never creates one. In
that namespace it creates two objects:

1. a Flux `GitRepository` named `wetsproces-chart` for this repository, kept to
   `charts/wetsproces`;
2. a `HelmRelease` named `wetsproces` that installs that chart with the install
   form as values.

Flux installs the chart and keeps the app running while the plugin pod is
down. The plugin reports the `Ready` condition of the `HelmRelease` as its
status: `running` once the release is ready, `degraded` once Flux has stalled
on it, `installing` before that.

The plugin is derived from the Haven+ plugin
([plugin-fundament](https://gitlab.com/commonground/haven/havenplus/plugin-fundament),
EUPL-1.2). The lifecycle, the status reporter and most of the tests come from
there.

## The chart

`charts/wetsproces` runs the server in test mode, as the fieldlab deployment
on ZAD does: an instance per side on one PostgreSQL database, `burger` for
citizens on `burger.wetsproces.<domain>` and `backoffice` for employees on
`medewerkers.wetsproces.<domain>`, both with the test login. An init container
clones the case repository, shallow, into an
`emptyDir` at `/case`, and `WETSPROCES_MANIFEST` points at
`/case/<CASE_PATH>`. The manifest's `repo_root` is resolved from there, so
`../..` in `packages/wetsproces/bron.yaml` is the root of the clone. The screens
come from the image (`/app/frontend`), not from the case.

The database is a CloudNativePG `Cluster` of the release, `wetsproces-db`, with
one instance and no backups. The CloudNativePG operator must run on the
cluster; on `cluster-a` it does, as a Fundament plugin, and without it the
chart refuses to render. helm-controller creates the `Cluster`, so the
release's namespace needs no extra rights. Both instances read the connection
URL from the secret `wetsproces-db-app` that CloudNativePG writes, and migrate
the schema `wetsproces` at start under an advisory lock. The chronicles survive
a restart; an uninstall removes the `Cluster` and its volume with them.

`database.enabled: false` with `surfaces.split: false` is the small setup: one
instance on `wetsproces.<domain>` for both sides, the chronicles in memory and
emptied by every restart. The chart refuses two instances on memory, because
each would keep its own chronicles. `dns.enabled` adds a `DNSEndpoint` for an
external-dns that reads them, such as the one on `cluster-a` for its zone
`cluster-a.fieldlab2026.nl`. It needs an address reserved for the project in
`dns.target` or `gateway.address`, because a record left on an ephemeral
Gateway address would point at whoever gets that address next; an external-dns
with upsert-only never removes a record, so remove them by hand after an
uninstall. The plugin does not pass `dns`, so this is for a release installed
by hand.

The values are described in `charts/wetsproces/values.yaml`. Without the
plugin, into a Fundament namespace:

```sh
helm upgrade --install wetsproces charts/wetsproces -n tnt-rr-voorbeeld--wetsproces \
  --set domain=194-135-48-156.sslip.io --set tls.acme=staging
```

### How the host is reached

Everything stays in the release's namespace. The chart creates:

- a Gateway API `Gateway` (class `istio`) with its own public address, an HTTP
  listener and an HTTPS listener per host, each with a certificate of its own;
- a namespaced cert-manager `Issuer` for Let's Encrypt, with the HTTP-01
  solver on the Gateway's HTTP listener, and the annotation
  `cert-manager.io/issuer` on the Gateway, from which cert-manager makes the
  `Certificate`;
- an `HTTPRoute` per instance from its HTTPS listener to its service, and one on the HTTP
  listener that redirects to HTTPS.

`tls.acme` picks the staging or the production environment of Let's Encrypt.
Production has a weekly rate limit, shared by everyone on the same domain, so
start on staging and switch once the Gateway answers.

Istio asks the LoadBalancer pool for the Gateway's address. On `cluster-a`
MetalLB assigns no address unless the Service names its pool, which
`gateway.serviceAnnotations` does. The addresses there are ephemeral: the cloud
controller allocates one when the Gateway's Service is created and releases it
when the Service goes, so an uninstall can cost the address the DNS name points
at. `gateway.address` asks for a fixed one, but the controller refuses an
address that is not allocated to the project, and it does so for a released
ephemeral one too. A stable address needs a static IP for the project.

Without a DNS record of your own, `<ip-with-dashes>.sslip.io` resolves to the
Gateway's address. The first install then runs with a placeholder domain until
the address is known; the certificate for that placeholder fails, and an
upgrade with the real domain fixes it.

## Configuration

The console shows these keys in the install form. The plugin reads each key as
the `FUNP_<name>` environment variable.

| Key | Required | Default | What it is |
|---|---|---|---|
| `CASE_REPO_URL` | no | `https://code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026.git` | HTTPS URL of the public repository with the case |
| `CASE_REF` | no | `main` | branch or tag of that repository |
| `CASE_PATH` | no | `packages/wetsproces/bron.yaml` | path of the manifest in that repository |
| `NAMESPACE` | yes | | the full name of an existing Fundament namespace, `tnt-<project>--<name>` |
| `DOMAIN` | yes | | citizens use `burger.wetsproces.<domain>` and employees `medewerkers.wetsproces.<domain>`, e.g. `194-135-48-156.sslip.io` |
| `IMAGE` | no (advanced) | `code.digitaledienst.overheid.nl/regelrecht/wetsproces:f619755c457d30babec9d7e200092e30794a79f7` | the wetsproces image |
| `CHART_REF` | no (advanced) | `main` | branch or tag of this repository that the chart is read from |
| `GATEWAY_ADDRESS` | no (advanced) | empty | a fixed public IP for the Gateway; must be a static IP of the project |
| `ACME` | no (advanced) | `production` | Let's Encrypt environment: `staging` or `production` |

`CASE_REPO_URL` has a default and is therefore not marked required: a
Fundament config schema refuses a key that is both.

The default `IMAGE` is a build of this repository's `main` by
`.forgejo/workflows/wetsproces-image.yml`, which tags each build with its
commit: here `f619755c`. The plugin pins it, so an installation runs the same
code until its `IMAGE` changes; the screens come from the image, and only the
case comes from `CASE_REF`.

The chart itself follows `main`: after each build, the same workflow sets
`image.ref` in `charts/wetsproces/values.yaml` to the new image, in a commit of
its own on `main`. A `HelmRelease` that reads the chart from `main` and leaves
`image` out of its values therefore rolls out every build, within the interval
of its `GitRepository`.

An install config for `cluster-a`, as a `PluginInstallation` takes it:

```yaml
config:
  NAMESPACE: tnt-rr-voorbeeld--wetsproces
  DOMAIN: 194-135-48-156.sslip.io
  CASE_REPO_URL: https://code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026.git
  CASE_REF: main
  CASE_PATH: packages/wetsproces/bron.yaml
```

## Lifecycle

| Event | What the plugin does |
|---|---|
| Install | Checks that the namespace exists, applies the `GitRepository` and the `HelmRelease`. A missing namespace is retried. |
| Reconcile | Applies both again and reports the release status, every `FUNP_RECONCILE_INTERVAL` (default 5m). |
| Uninstall | Deletes the `HelmRelease` and waits until helm-controller has uninstalled the release, then deletes the `GitRepository`. The namespace stays. |

A new `CASE_REF` changes the values of the `HelmRelease`, and the changed pod
template makes Kubernetes start a pod that clones that ref. A new commit on the
same branch does not: the pod keeps the clone it made at start until it
restarts.

## Permissions

The plugin may read namespaces and manage `GitRepository` and `HelmRelease`
objects. Fundament makes `spec.permissions.rbac` a ClusterRole, so the rules
cannot be kept to one namespace. It needs no internet access: Flux pulls the chart and
the cluster pulls the images. helm-controller installs the chart with its own
service account.

## Develop

The plugin needs Go 1.27.3 (`go.mod`).

```sh
go -C packages/fundament-plugin test ./...
docker build -t wetsproces-fundament-plugin:dev packages/fundament-plugin
```

`TestChartValuesExistInChart` reads `charts/wetsproces/values.yaml`, so the
tests run from a checkout of the whole repository.

## Publish

The image goes to the Forgejo registry under the version in
`definition.yaml`. Images there are public, so nothing secret goes into one.
`functl plugin publish` takes the image by digest and injects it into the
definition:

```sh
docker buildx build --platform linux/amd64 --push \
  -t code.digitaledienst.overheid.nl/regelrecht/wetsproces-plugin:v0.1.0 packages/fundament-plugin
docker buildx imagetools inspect code.digitaledienst.overheid.nl/regelrecht/wetsproces-plugin:v0.1.0
FUNCTL_REGISTRY_URL=https://marketplace-registry-api.fundament-poc.nl \
  functl plugin publish packages/fundament-plugin/definition.yaml --create --submit \
  --image=code.digitaledienst.overheid.nl/regelrecht/wetsproces-plugin@sha256:<digest>
```

`--submit` sends the version to review. The console offers only approved
versions.
