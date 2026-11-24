# RegelHet, a Fundament plugin

A [Fundament](https://github.com/fundament-oss/fundament) plugin that offers
RegelHet (`packages/regelhet`) to the projects of a cluster. It
is installed once per cluster. After that a project member creates a
RegelHet in the console under **Procedures**, and each one runs one case
on an address of its own, with a database of its own. Several can run in one
namespace or spread over several. The model is that of Fundament's
CloudNativePG plugin, where a project makes its own databases.

The plugin is derived from the Haven+ plugin
([plugin-fundament](https://gitlab.com/commonground/haven/havenplus/plugin-fundament),
EUPL-1.2), and its console pages from those of Fundament's CloudNativePG plugin
(Apache-2.0).

## The RegelHet resource

A RegelHet is a namespaced object of kind `RegelHet`
(`regelhet.regelrecht.nl/v1alpha1`, CRD in `crds/`). The plugin installs the
CRD when it starts.

```yaml
apiVersion: regelhet.regelrecht.nl/v1alpha1
kind: RegelHet
metadata:
  name: parkeerkaart
  namespace: tnt-rr-voorbeeld--parkeerkaart
spec:
  manifest:
    repoUrl: https://code.digitaledienst.overheid.nl/regelrecht/regelhet-parkeerkaart
    ref: main
    path: bron.yaml
    # For a private repository: a Secret with the keys username and token.
    # Read once, by the Job that copies the case (below).
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
  snapshot:
    commit: 2a89a76fd0e08323cc1453a52506fc04db212eb9
    configMaps: [parkeerkaart-case-3f9c1a07b2-0]
```

| Field | What it is |
|---|---|
| `manifest.repoUrl` | HTTPS URL of the git repository with the case, without credentials |
| `manifest.ref` | branch, tag or full commit, `main` by default; a commit pins the case to one version |
| `manifest.path` | path of the source manifest in that repository, `bron.yaml` by default: a manifest repository such as `regelrecht/regelhet-parkeerkaart` has it at the root, and this repository has it at `packages/regelhet/bron.yaml`. The case file and the articles it runs are named there, so the wizard does not ask for them |
| `manifest.credentialsSecretRef` | a Secret in the same namespace with `username` and `token`, for a private repository. Only the snapshot Job reads it; annotated `regelhet.regelrecht.nl/delete-after-snapshot: "true"`, the plugin deletes it after the snapshot (below) |
| `version` | the regelrecht commit whose RegelHet image runs and whose chart installs it, the full 40-character hash. Empty follows main |
| `theme.accentColor` | an NLDD theme, such as `paars` or `donkergroen`; empty keeps the design system's own accent |
| `hostname` | a name under the base domain, or a domain of your own with a dot in it |
| `auth.mode` | `test` (the default: the test login and a clock the admin sets) or `stable` (a Keycloak of its own, and the real date) |
| `auth.testAccounts` | in stable mode, give that Keycloak the test accounts of the case, with one generated password |
| `fsc4b.enabled` | also offer the citizens' routes to an app through FSC voor Burgers, on the demo Stelsel with a test Activatiedienst that checks no identity. Test mode only: with `auth.mode: stable` the API server and the plugin refuse it. `status.fsc4bDiscoveryUrl` then names `https://<citizens' host>/fsc4b/discovery`. A `version` from before chart 0.5.0 ignores it |
| `parties.ownInstances` | give each external party that the executor asks something of by its own policy a release of its own, where its staff log in, linked with the executor's release (see [External parties on a release of their own](#external-parties-on-a-release-of-their-own)). Test mode only |

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

In stable mode a third address carries its Keycloak: `zorgtoeslag-login.<domain>`,
or `<first label>-login` beside a domain of your own.

Two RegelHet instances cannot share an address. The older one keeps it; the newer
one goes to `Failed` with the reason `HostnameInUse` and makes nothing until
its hostname changes or the other one is gone.

## What the plugin makes

For each RegelHet the plugin's controller first copies the case into the
namespace (the snapshot, below), and then makes two Flux objects there, all
owned by the RegelHet:

1. a `GitRepository` named `<name>-chart` for this repository, kept to
   `charts/regelhet` at `CHART_REF`;
2. a `HelmRelease` named `<name>` that installs that chart with values from the
   spec, and the case from the snapshot.

Flux installs the chart and keeps it running while the plugin pod is down. The
plugin writes the HelmRelease's `Ready` condition back into the status: phase
`Ready` once the release is ready, `Failed` once Flux has stalled on it or the
spec is refused, `Pending` before that.

### The snapshot of the case

The pods of a release do not fetch the case. At install, and after every change
of `manifest.repoUrl`, `ref` or `path`, the plugin runs a Job
`<name>-snapshot-<hash>` that fetches that one commit, with the token of
`credentialsSecretRef` if there is one, and packs only what the manifest needs:
the manifest itself, its `ui_spec`, and the YAML files under each of its
`directories`, by their path in the repository. The plugin reads the tar.gz from
the Job's log and stores it in ConfigMaps `<name>-case-<hash>-<n>`, at most
900 KiB each and 4.4 MiB in all. The chart unpacks those into `/case` with an
init container (`case.snapshot.configMaps`). So a pod that restarts needs no
network access to git and mounts no Secret, and runs the same commit every
time; `status.snapshot.commit` names it.

The Job uses `alpine/git` and `mikefarah/yq`, both pinned by digest, and runs
under the same restricted security context as the chart's pods. The plugin
deletes it once the copy is stored. While a new snapshot is taken, or when it
fails, the release keeps running the previous one; the old ConfigMaps go once
the release is ready on the new ones.

A failed Job turns the RegelHet `Failed` with the reason `SnapshotFailed`,
and the message says why in one line:

```
copying the case failed: the git server refused access to https://...: check the token in the Secret of credentialsSecretRef, which needs read access (...)
copying the case failed: the manifest bron.yaml is not in commit 2a89a76f... of main
copying the case failed: the case is 5123456 bytes packed, more than the 4608000 a snapshot holds; ...
```

The Job stays for 5 minutes with its reason, and then the plugin tries again,
so a corrected token takes effect without a change of the spec.

A `version` from before chart 0.8.0 has a chart without the snapshot. Its pods
clone the case at every start, as before, with the token. So the plugin never
deletes the token of a RegelHet with a `version`.

A branch moves on without a change of the spec. For a new snapshot of the same
`ref`, set or change the annotation `regelhet.regelrecht.nl/snapshot` on the
RegelHet, to any value; a commit in `ref` pins the case instead.

The copy is as readable as the namespace's ConfigMaps: whoever may read those,
the role `view` included, can read the case, which a Secret would not allow. For
a private repository that is the trade: no token in the namespace, the case
itself in a ConfigMap.

#### The token is for one copy

After a successful snapshot the plugin deletes the Secret of
`credentialsSecretRef`, but only when that Secret carries the annotation
`regelhet.regelrecht.nl/delete-after-snapshot: "true"`, and only the Secret it
read (by UID). The wizard sets that annotation on the Secret it makes. A Secret
you make yourself stays unless you add it. `status.token` says whether the
Secret is still there (`Present` or `Absent`).

So a new version of the case (another `ref`, `path` or a new snapshot
annotation) needs a new Secret with a token first. Without it the RegelHet
goes to `Failed` with the reason `TokenMissing` and the release keeps the copy
it has. Create the Secret under the same name, with the annotation if it may go
again:

```bash
kubectl -n <namespace> create secret generic <name>-repo \
  --from-literal=username=x-access-token --from-file=token=<file>
kubectl -n <namespace> annotate secret <name>-repo \
  regelhet.regelrecht.nl/delete-after-snapshot=true
```

The plugin looks for the Secret again every 15 seconds.

### A RegelHet that does not start

Helm waits for the pods of an install until its timeout, and says nothing
before that. The plugin does not wait for it: it watches the pods of each
release, and as soon as a container stops with an error or keeps stopping
(`CrashLoopBackOff`), an image cannot be pulled, or a Secret the pod needs is missing, it sets the
RegelHet to `Failed` with the reason `NotStarting`. The message names the
container and gives the last line it wrote, which the chart keeps in the
container status (`terminationMessagePolicy: FallbackToLogsOnError`).

The server itself gets some time first. It stops at start while its database,
or in stable mode its Keycloak, does not answer yet, and Kubernetes starts it
again; in a test of stable mode on `cluster-a` it restarted four times in its
first minute. So a crash of the server container is reported from its fifth
restart, or once its pod is 3 minutes old, whichever comes first. A failed
init container, an image that cannot be pulled and a missing Secret wait for
nothing that is still starting, and are reported at once. A manifest path that
is not in the repository therefore reads, after about two and a half minutes:

```
container regelhet: regelhet: the case: the manifest does not read: ...
```

A token the git server refuses, a missing path or a case that is too large
is reported by the snapshot instead, before any pod starts (above).

Helm goes on trying. Its timeout is 3 minutes instead of the default 5, so a
corrected spec, or a new RegelHet in place of the failed one, takes over
sooner. Once the pods run, the status follows the release again.

The snapshot Job checks that the manifest and the directories it names are in
the commit. Whether the case itself reads, the server says when it starts.

The chart then runs what it runs by default since PR #7: a CloudNativePG
`Cluster` as the database of the release, one instance for citizens and one for
employees, each on its own address. Its hosts are listeners on the cluster's
shared Gateway (below), so a release needs no public address of its own, and
two RegelHet instances take no more IP addresses than one.

### Deleting a RegelHet deletes its data

Deleting a RegelHet deletes its `HelmRelease` through the owner reference,
and helm-controller uninstalls the release. That removes the CloudNativePG
`Cluster`, which removes its PVCs, and with them every chronicle of the
RegelHet. There is no retention option and there are no backups. The detail
page in the console and the last step of the wizard say this.

In stable mode the Keycloak's own database goes the same way, with every
account in it. The Job of the chart makes the Secret `<name>-keycloak` with
the passwords outside Helm, so that a reinstall keeps them; nothing owns it.
Once the RegelHet is gone, the plugin deletes that Secret itself, if it
still carries the chart's labels for that release. It does this on the delete
event, without a finalizer: a RegelHet with a finalizer that outlives the
plugin could no longer be deleted. A delete the plugin misses, because its pod
was down, leaves the Secret behind.

The Secrets `<name>-burger-tls`, `<name>-backoffice-tls` and, in stable mode,
`<name>-keycloak-tls` stay: cert-manager made them and leaves them when the
listeners go. They hold certificates, no data of the RegelHet; delete them by
hand if you like.

### Version

Without a `version` the HelmRelease leaves `image` out of its values, so the
chart's own `image.ref` counts. That line names the latest build of main:
`.forgejo/workflows/regelhet-image.yml` sets it after every build, in a
commit of its own. Because the HelmRelease rebuilds the chart on every new
commit of its source (`reconcileStrategy: Revision`), a RegelHet without a
version rolls out each new build within the interval of its `GitRepository`.
With a `version` it runs `<DEFAULT_IMAGE>:<version>`, and the `GitRepository`
of the chart points at that same commit (`ref.commit`), so the image runs with
the chart it was built with. A commit from before the chart existed has no
`charts/regelhet`; the last such commit in this repository is `f619755c`.
Flux keeps trying such a source without giving up, so the plugin sets the
RegelHet to `Failed` with the reason `ChartNotInCommit` as soon as Flux
reports that the chart is not there.

The case comes from `manifest.ref`, not from the image. A case on `main` that
needs a newer server than the pinned image can stop the server at start; it
says why in its log.

## Organisaties

A municipality runs more than one RegelHet. An `Organisatie` puts several of
them behind one front with its own name, logo and login, so employees log in
once and citizens find every product in one place. Each RegelHet keeps its own
release, its own database and its own addresses, and works on its own as well.

```yaml
apiVersion: regelhet.regelrecht.nl/v1alpha1
kind: Organisatie
metadata:
  name: wilgendaal
  namespace: tnt-rr-voorbeeld--wilgendaal
spec:
  displayName: Gemeente Wilgendaal
  logo: https://wilgendaal.example.org/logo.svg   # optional
  theme:
    accentColor: groen
  hostname: wilgendaal
  version: ''                                      # as for a RegelHet
  auth:
    mode: test                                     # test only, for now
status:
  phase: Ready
  url: https://wilgendaal.cluster-a.fieldlab2026.nl/
  backofficeUrl: https://wilgendaal-medewerkers.cluster-a.fieldlab2026.nl/
  processen:
    - namespace: tnt-rr-voorbeeld--parkeerkaart
      name: parkeerkaart
      phase: Ready
```

A RegelHet joins it through its own spec. An empty `namespace` is the
namespace of the RegelHet:

```yaml
spec:
  organisatie:
    name: wilgendaal
    namespace: tnt-rr-voorbeeld--wilgendaal
```

For an Organisatie the plugin makes a `GitRepository` and a `HelmRelease`, as
for a RegelHet and with the same chart, with the block `voorkant` in the
values: the server in its role as a front, without a case and without a
database. The list of processes in that block holds every RegelHet that
names the organisatie, with its address on the cluster network
(`http://<name>-<side>.<namespace>.svc.cluster.local:8000`) and its public
one. The plugin watches RegelHet instances, so a process that joins or leaves
changes the list, and the chart restarts the front.

The HelmRelease of a RegelHet on the list gets the block `organisatie`: the
id of the front, its name, its two addresses and where its public key is
served on the cluster network. The RegelHet accepts what that front signs.

A RegelHet joins only an organisatie of its own Fundament project. In one
namespace that holds by itself; across namespaces both must carry the same
`fundament.io/project-id` label, which the plugin reads with `get` on the two
namespaces. Without the check any project could put a RegelHet behind the
front of another. The condition `OrganisatieGevonden` on the RegelHet says
how it went:

| Reason | What happens |
|---|---|
| `OrganisatieFound` | on the list; the release has the block `organisatie` |
| `OrganisatieNotFound` | the organisatie does not exist or is being deleted; the RegelHet runs on its own |
| `OrganisatieInOtherProject` | the namespaces are in different projects, or one is in none; the RegelHet runs on its own |
| `OrganisatieModeMismatch` | a stable RegelHet behind a front in test mode, whose test login lets anyone in as anyone; the RegelHet goes to `Failed` |

A RegelHet also runs on its own, with the reason `OrganisatieRefused`, when
the plugin refuses the organisatie itself (its spec, its address or its name).
The other way round, an organisatie lists no RegelHet the plugin refuses.
The plugin reads the project label when a RegelHet or an organisatie
changes; it does not watch namespaces, so a label that changes later counts
from the next change of either.

Deleting an organisatie deletes its release and leaves the RegelHet instances
running; their condition turns `False` and their releases lose the block. The
plugin then deletes the Secret `<name>-organisatie` with the signing key, which
the Job of the chart made outside Helm, as it does with the Keycloak Secret of
a RegelHet. The wizard of a stable RegelHet skips the step **Organisatie**:
an organisatie runs in test mode only for now.

The address check of a RegelHet covers organisaties as well: an organisatie
`wilgendaal` and a RegelHet `wilgendaal` cannot both have
`wilgendaal.<domain>`, and the newer one gets `HostnameInUse`. Within one
namespace a RegelHet and an Organisatie cannot share a name either, because
the name is that of their Helm release; the newer one gets `NameInUse`.

## The chart behind a shared Gateway

`charts/regelhet` reaches its hosts in one of three ways, chosen by
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

The chart can run a RegelHet in stable mode, and the plugin turns it on with
`spec.auth.mode: stable` (below). With `regelhet.login: oidc` each instance logs in through Keycloak, with
a client of its own: `regelhet` for citizens and `regelhet-backoffice` for
employees, as [Authentication & Roles](../../docs/src/content/docs/auth-and-roles.md#regelhet-roles)
describes. The chart sets `BASE_URL` to the instance's own host, so the
redirect URI each client must allow is `https://<host>/auth/callback`.
`regelhet.mode: stable` needs this login, the database and no instance with
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

The instances then log in at `https://<Keycloak host>/realms/regelhet` with
the client secrets from that Secret, without further values. The server reads
the discovery document of that host when it starts, so it needs a certificate
it trusts: Let's Encrypt staging is enough to try the routes, and the login
works on production. Until Keycloak and the databases answer, the server stops
at start and Kubernetes starts it again.

```yaml
regelhet:
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

### Stable mode from the plugin

A RegelHet with `auth.mode: stable` gets the values above: `regelhet.mode:
stable`, `regelhet.login: oidc` and `keycloak.enabled: true`, with
`keycloak.host` set to its `-login` address and `keycloak.testUsers` from
`auth.testAccounts`. PostgreSQL and an instance per side are the chart's
defaults already. The status names the login address (`loginUrl`) and the
Secret with the passwords (`keycloakSecret`, `<name>-keycloak`), by its name
only; the detail page in the console shows both, and how to read a key with
`kubectl`.

```yaml
apiVersion: regelhet.regelrecht.nl/v1alpha1
kind: RegelHet
metadata:
  name: zorgtoeslag
  namespace: tnt-rr-voorbeeld--zorgtoeslag
spec:
  manifest:
    repoUrl: https://code.digitaledienst.overheid.nl/regelrecht/regelhet-parkeerkaart
  hostname: zorgtoeslag
  auth:
    mode: stable
    testAccounts: true
```

The login needs a certificate the server trusts. On an installation with
`ACME: staging` the plugin therefore refuses stable mode: the RegelHet goes
to `Failed` with the reason `StableNeedsProductionCerts`, and nothing is made.
Test mode is unaffected by this.

For a Keycloak elsewhere, `oidc.issuerUrl` and `oidc.existingSecret` name it
and the Secret with the client secrets, in the release's namespace. The realm
is then also available as a `KeycloakRealmImport`, for a Keycloak that the
Keycloak operator runs. That operator only reads an import in the namespace of
its `Keycloak`; on `cluster-a` that is `keycloak-instances`, which Haven+
manages from its own repository, so there the import is rendered to a file and
applied by whoever manages that namespace:

```sh
helm template regelhet charts/regelhet --set domain=cluster-a.fieldlab2026.nl \
  --api-versions postgresql.cnpg.io/v1/Cluster \
  --set keycloak.realmImport.enabled=true \
  --set keycloak.realmImport.namespace=keycloak-instances \
  --set keycloak.realmImport.secretName=regelhet-realm \
  --set keycloak.testUsers=true \
  --show-only templates/keycloak-realm.yaml
```

The operator fills the client secrets and the test password in from the Secret
`secretName`, in the namespace of the import.

## The chart for an organisation

An organisation is one front over several RegelHet instances: one address for
citizens, one for employees, one login. The front is the same server in the
role `organisatie`, installed from the same chart with `voorkant.enabled`. Each
process stays a release of its own and, with `organisatie.id`, accepts what
the front signs. Both blocks are off by default, and a release that sets
neither renders as it did before.

With `voorkant.enabled` the chart:

- runs an instance per side, citizens on `host` and employees on `<first label
  of host>-medewerkers` beside it, so behind the shared Gateway both stay one
  label under the wildcard. Without `host` the base is `<release>.<domain>`,
  and `surfaces.*.host` overrides either;
- clones no case and makes no database, whatever `case` and `database` say;
  the sessions of the front live in memory, so it refuses stable mode for now;
- writes `voorkant.processen` into the ConfigMap `<release>-organisatie`, as the
  file the server reads at `REGELHET_PROCESSEN`. A checksum of the list on
  the pods restarts the front when a process registers or leaves;
- generates a P-256 key once, in the Secret `<release>-organisatie` under
  `private-key`, with the same Job that makes the Secret of Keycloak. A step
  with `alpine/openssl` writes the key to a volume in memory, because the image
  with `kubectl` has no openssl. The Secret stays behind after an uninstall,
  like the one of Keycloak.

```yaml
voorkant:
  enabled: true
  id: tnt-rr-voorbeeld--wilgendaal/wilgendaal    # <namespace>/<name> of the Organisatie
  naam: Gemeente Wilgendaal
  logo: https://wilgendaal.example.org/logo.svg  # optional
  processen:
    - id: tnt-rr-voorbeeld--parkeerkaart/parkeerkaart
      naam: parkeerkaart
      burger:
        intern: http://parkeerkaart-burger.tnt-rr-voorbeeld--parkeerkaart.svc.cluster.local:8000
        publiek: https://parkeerkaart.cluster-a.fieldlab2026.nl
      backoffice:
        intern: http://parkeerkaart-backoffice.tnt-rr-voorbeeld--parkeerkaart.svc.cluster.local:8000
        publiek: https://parkeerkaart-medewerkers.cluster-a.fieldlab2026.nl
```

A process of that organisation keeps its own hosts, case and database. The
chart gives both its instances the process id `<namespace>/<release>`, the
organisation's id, name and key address, and a link back: the citizens'
instance to `url`, the employees' instance to `medewerkersUrl`.

```yaml
organisatie:
  id: tnt-rr-voorbeeld--wilgendaal/wilgendaal
  naam: Gemeente Wilgendaal
  url: https://wilgendaal.cluster-a.fieldlab2026.nl
  medewerkersUrl: https://wilgendaal-medewerkers.cluster-a.fieldlab2026.nl
  sleutelUrl: http://wilgendaal-backoffice.tnt-rr-voorbeeld--wilgendaal.svc.cluster.local:8000/organisatie/v1/sleutel
```

`sleutelUrl` is for test mode. A process in stable mode fetches no key over the
network and needs `sleutelJwk`, the front's public key pinned in the values.
Both examples are complete in `charts/regelhet/ci/`, where CI renders them.

## Console

The plugin adds **Procedures** to the project sidebar, under the heading
**RegelHet** (the `displayName` of the definition), with pages of its own
under `console/`: a list, a detail page with the addresses and the status, and
a wizard that asks, one step at a time, for the case, the access and the login
(test or stable, and in test mode whether FSC voor Burgers is on), the version,
the accent color and the address, and shows what it will make before it makes
it. For a private repository the wizard makes the Secret with the token as well,
owned by the RegelHet so the two go together.

The console calls a RegelHet a procedure: the menu item, the page titles and
the text on the pages say "procedure", while the kind `RegelHet`, its API
group, its fields and the addresses keep their names.

Under **Organisaties**, next to **Procedures**, are the same three pages for an Organisatie: a list,
a detail page with the addresses, the RegelHet instances on its list and the status,
and a wizard for the name and logo, the accent color and the address. The
wizard of a RegelHet has a step **Organisatie** with three choices: none, a
new one (whose name, address, logo and color it asks for, and which it makes in
the namespace of the RegelHet before the RegelHet itself), or one of the
organisaties in the namespaces of the project. The detail page of a RegelHet
names its organisatie and whether it is on the list. Both wizards share the
step machine in `console/_shared.js`.

The version step offers the latest build or a commit typed in. It does not list
the available builds: the console's content security policy lets a page reach
only Fundament's own APIs, not the registry.

The pages are plain ES modules with the NLDD components the console serves
(0.15.0). Their logic outside the DOM is in `console/regelhets-body.js` and
`console/organisaties-body.js`, with tests in `console-test/`.

## Configuration

The install form holds defaults only; everything per RegelHet is in its
object. The plugin reads each key as the `FUNP_<name>` environment variable.

| Key | Required | Default | What it is |
|---|---|---|---|
| `DOMAIN` | yes | | the base domain, e.g. `cluster-a.fieldlab2026.nl` |
| `DEFAULT_IMAGE` | no (advanced) | `code.digitaledienst.overheid.nl/regelrecht/regelhet` | image repository for a RegelHet with a version |
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
| Start, install, upgrade | Checks that Flux is there, applies the two CRDs and the two roles below, then runs the controllers. A failed set-up is retried every 30 seconds. |
| Reconcile | Repairs the CRD and the roles and reports how many RegelHet instances are ready, every `FUNP_RECONCILE_INTERVAL` (default 5m). |
| Uninstall | Deletes the two roles. It keeps the CRDs and every object: deleting the RegelHet CRD would delete every RegelHet with its data. Flux keeps them running; without the plugin nobody reconciles a change. |

To remove everything after an uninstall, delete the objects first
(`kubectl delete regelhets,organisaties --all -n <namespace>`), then the
CRDs.

## Permissions

Fundament makes `spec.permissions.rbac` a ClusterRole, so no rule can be kept
to the namespaces of a project. The rules in `definition.yaml` say per group
why the plugin needs them. In short: create and get the plugin's own CRDs,
manage two ClusterRoles by name, manage RegelHet and Organisatie objects and
their status, manage Flux `GitRepository` and `HelmRelease` objects, read pods
and the log of a pod, get namespaces, manage Jobs and ConfigMaps, and create,
get and delete Secrets. Pods are read to report a RegelHet that does not
start. A namespace is read, with `get` only, for its project label when a
RegelHet names an organisatie in another namespace. Jobs, ConfigMaps and
`pods/log` are for the snapshot: the plugin creates, watches and deletes its
own Jobs, reads the log of a finished one, and creates, lists and deletes the
ConfigMaps that hold the copy; it lists ConfigMaps by label, past its cache,
and does not watch them. On Secrets the plugin has no `list` or `watch`, so it
sees only a Secret it asks for by name:

- `create` is for the wizard, which makes the Secret with the token of a
  private repository. A console page acts with the rights the user and the
  plugin share, so the plugin must hold every right a page uses.
- `get` and `delete` are for one Secret per deleted RegelHet: the plugin
  reads `<name>-keycloak` and deletes it when it carries the chart's labels
  for that release. And for the token Secret after a snapshot: the plugin
  reads it, and deletes it only with the annotation
  `regelhet.regelrecht.nl/delete-after-snapshot: "true"`. The names are not
  known in advance, so `resourceNames` cannot narrow this.

The pages never read a Secret.

The two ClusterRoles aggregate into `admin` and `view`, the roles Fundament
binds project admins and viewers to. Admins may create and delete
RegelHet instances and organisaties; viewers may read them.

## External parties on a release of their own

A case names its external parties, and its regelwerken say which of them the
executor asks something of by its own policy. For the parking card that is the
GGD, which examines the applicant; the court is asked nothing. With
`parties.ownInstances` each such party gets a release of its own, where its
staff log in and see what the executor asks of it. The two releases exchange
requests and answers as signed messages (the chart's `koppeling` values; the
server's side is described on the docs page `components/regelhet-koppeling`).

The plugin does not read the case. The server does, and names those parties on
`GET /koppeling/v1/partijen`. So the plugin works in two steps:

1. It installs the executor's release without a link.
2. Once that release is ready, it asks the Service of the employees' instance
   inside the cluster, writes the answer to `status.parties`, makes a
   HelmRelease `<name>-<party>` per party and applies the executor's release
   again with those parties as its peers.

A party's address is `<hostname>-<party>.<domain>`, where the party is its cell
with dashes: `ggd_wilgendaal` under the hostname `parkeerkaart` gives
`parkeerkaart-ggd-wilgendaal.<domain>`. The condition `PartiesLinked` says how
far this is and why not. A case whose regelwerken ask no party for anything
gets no extra release, and the condition says so.

While the release is ready the plugin asks again at every reconcile, and at
least every ten minutes, so a case that gains a party follows. A party the
server no longer names keeps its release and its data, and the condition says
so: the answer depends on the day the server runs on, which an admin sets in
test mode, and removing a release removes its database. Turning
`parties.ownInstances` off removes every party release. While the release is
not ready, or the server does not answer, what runs is left alone and the
plugin asks again after thirty seconds.

A party is left out, with the reason in the condition, when its name gives no
valid address or release name, when it comes to the same name as another
party or as a part of the executor's own release (`burger`, `backoffice`,
`medewerkers`, `login`, `keycloak`, `db`, `chart`), or when a release or a
RegelHet with that name or address exists already. The names come from the
case, so the plugin never applies to or deletes a release it did not make for
that party.

What this leans on:

- The plugin's pod must reach a Service in a project namespace. The cluster
  does not separate namespaces on the network today; where it does, this call
  needs an allowance.
- A build of the server from before this route answers 404, which the
  condition reports as a version that is too old.
- The link runs with keys made at start, which the server allows in test mode
  only, so `auth.mode: stable` refuses `parties.ownInstances`.

## Develop

The plugin needs Go 1.27.3 (`go.mod`). After a change to `api/v1alpha1`,
regenerate the deep copies and the CRD with controller-gen 0.20:

```sh
controller-gen object paths=./api/... crd:crdVersions=v1 output:crd:dir=./crds
go -C packages/fundament-plugin test ./...
node --test packages/fundament-plugin/console-test/*.test.js
docker build -t regelhet-fundament-plugin:dev packages/fundament-plugin
```

`TestChartValuesExistInChart` reads `charts/regelhet/values.yaml`, so the
tests run from a checkout of the whole repository.

## Publish

The image goes to the Forgejo registry under the version in `definition.yaml`.
Images there are public, so nothing secret goes into one. `functl plugin
publish` takes the image by digest and injects it into the definition:

```sh
docker buildx build --platform linux/amd64 --push \
  -t code.digitaledienst.overheid.nl/regelrecht/regelhet-plugin:v0.2.0 packages/fundament-plugin
docker buildx imagetools inspect code.digitaledienst.overheid.nl/regelrecht/regelhet-plugin:v0.2.0
FUNCTL_REGISTRY_URL=https://marketplace-registry-api.fundament-poc.nl \
  functl plugin publish packages/fundament-plugin/definition.yaml --create --submit \
  --image=code.digitaledienst.overheid.nl/regelrecht/regelhet-plugin@sha256:<digest>
```

`--submit` sends the version to review. The console offers only approved
versions.
