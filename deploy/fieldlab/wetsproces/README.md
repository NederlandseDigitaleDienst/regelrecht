# wetsproces on Fundament (fieldlab 2026)

This directory runs the wetsproces server on the Fundament cluster of the
fieldlab: cluster `cluster-a` in organisation `fieldlab-a`, in namespace
`wetsproces` of project `rr-voorbeeld` (`RegelRecht voorbeeld`). The cluster is
shared with other teams, so everything here stays inside that one namespace.

What runs:

- the image `code.digitaledienst.overheid.nl/regelrecht/wetsproces:main`, which
  `.forgejo/workflows/wetsproces-image.yml` builds on every push to `main`
  that touches the code;
- the server in test mode, with the test login and both sides on one address
  (`WETSPROCES_SURFACE=all`);
- a PostgreSQL database from the CloudNativePG plugin for the chronicles;
- an HTTPRoute for `http://regelrecht-wetsproces.cluster-a.fieldlab2026.nl`
  on the shared gateway of Haven+.

## Before the first deploy

You need `functl` and `kubectl` on your PATH and a kubeconfig for the cluster
(step 4 of the fieldlab's getting-started page).

Create the namespace through Fundament, not with `kubectl create namespace`:
Fundament never adopts a namespace it did not create, and project members get
no rights on it.

```bash
functl namespace create --project=01a1ee7d-2850-78d1-9c02-90cccb3d5e1e wetsproces
kubectl get ns tnt-rr-voorbeeld--wetsproces
```

## Deploy

```bash
kubectl apply -k deploy/fieldlab/wetsproces
kubectl -n tnt-rr-voorbeeld--wetsproces get cluster.postgresql.cnpg.io wetsproces-db
kubectl -n tnt-rr-voorbeeld--wetsproces rollout status deploy/wetsproces
```

The database takes about a minute. Until its secret `wetsproces-db-app`
exists, the pod waits in `CreateContainerConfigError`; that resolves itself.

After a new image, pull it again:

```bash
kubectl -n tnt-rr-voorbeeld--wetsproces rollout restart deploy/wetsproces
```

## Things to know

- **HTTP only.** The shared gateway only accepts routes from other namespaces
  on its `http` listener. HTTPS needs a listener per hostname on that gateway,
  and Flux manages it; ask the platform team for one. The test login works
  over HTTP, the Keycloak login (stable mode) does not.
- **Test mode.** Anyone who reaches the address can log in as any test person
  and empty the setup. Do not put real data in it.
- **Public image.** Every image of the organisation can be pulled without
  login, so an image holds no secrets.
