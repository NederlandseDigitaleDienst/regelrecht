{{/*
The hostname the app is served on; for an organisation's front the base of
its two hosts, by default <release>.<domain>.
*/}}
{{- define "regelhet.host" -}}
{{- if .Values.host -}}
{{- .Values.host -}}
{{- else if .Values.voorkant.enabled -}}
{{- printf "%s.%s" .Release.Name (required "domain is required" .Values.domain) -}}
{{- else -}}
{{- printf "regelhet.%s" (required "domain is required" .Values.domain) -}}
{{- end -}}
{{- end -}}

{{/*
"true" when the release keeps its chronicles in PostgreSQL: database.enabled,
except on an organisation's front, which keeps none.
*/}}
{{- define "regelhet.database" -}}
{{- if and .Values.database.enabled (not .Values.voorkant.enabled) -}}true{{- end -}}
{{- end -}}

{{/* The case path, refused when it climbs out of the clone. */}}
{{- define "regelhet.casePath" -}}
{{- if contains ".." .Values.case.path -}}
{{- fail "case.path must stay inside the repository" -}}
{{- end -}}
{{- .Values.case.path -}}
{{- end -}}

{{- define "regelhet.labels" -}}
app.kubernetes.io/name: regelhet
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end -}}

{{- define "regelhet.selectorLabels" -}}
app.kubernetes.io/name: regelhet
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{/* The name of the CNPG Cluster; CNPG names its app secret <cluster>-app. */}}
{{- define "regelhet.dbCluster" -}}
{{- printf "%s-db" .Release.Name -}}
{{- end -}}

{{/*
The instances of the server, as a JSON list: one with the surface of
regelhet.surface on the host of regelhet.host, or, with surfaces.split,
one per surface on a host of its own. Each item has `name` (the name of its
Deployment and Service), `surface`, `host` and `suffix` (empty for the single
instance, so its names stay what they were).

An organisation's front always has an instance per side, which share
nothing, by default on <host> and <first label of host>-medewerkers.
*/}}
{{- define "regelhet.instances" -}}
{{- $base := include "regelhet.host" . -}}
{{- $front := .Values.voorkant.enabled -}}
{{- if or .Values.surfaces.split $front -}}
{{- if not (or .Values.database.enabled $front) -}}
{{- fail "surfaces.split needs database.enabled: the two instances share their chronicles, and the store in memory cannot be shared" -}}
{{- end -}}
{{- $labels := splitList "." $base -}}
{{- $burger := .Values.surfaces.burger.host | default (ternary $base (printf "burger.%s" $base) $front) -}}
{{- $backoffice := .Values.surfaces.backoffice.host | default (ternary (printf "%s-medewerkers.%s" (first $labels) (rest $labels | join ".")) (printf "medewerkers.%s" $base) $front) -}}
{{- if eq $burger $backoffice -}}
{{- fail "surfaces.burger.host and surfaces.backoffice.host must differ: two HTTPS listeners on one host conflict" -}}
{{- end -}}
{{- list
  (dict "name" (printf "%s-burger" .Release.Name) "surface" "burger" "host" $burger "suffix" "-burger")
  (dict "name" (printf "%s-backoffice" .Release.Name) "surface" "backoffice" "host" $backoffice "suffix" "-backoffice")
  | toJson -}}
{{- else -}}
{{- list (dict "name" .Release.Name "surface" .Values.regelhet.surface "host" $base "suffix" "") | toJson -}}
{{- end -}}
{{- end -}}

{{/*
What stable mode and the login through Keycloak need, refused at render time
so that a release never rolls out a server that stops at start. The server
checks the same; these guards only say it earlier.
*/}}
{{- define "regelhet.checkLogin" -}}
{{- $w := .Values.regelhet -}}
{{- include "regelhet.checkOrganisatie" . -}}
{{- if eq $w.mode "stable" -}}
{{- if not (include "regelhet.database" .) -}}
{{- fail "regelhet.mode stable keeps the chronicles in PostgreSQL: set database.enabled" -}}
{{- end -}}
{{- range include "regelhet.instances" . | fromJsonArray -}}
{{- if eq .surface "all" -}}
{{- fail "regelhet.mode stable serves each side on an instance of its own: set surfaces.split, or a regelhet.surface other than all" -}}
{{- end -}}
{{- end -}}
{{- if ne $w.login "oidc" -}}
{{- fail "regelhet.mode stable logs in through Keycloak only: set regelhet.login oidc" -}}
{{- end -}}
{{- end -}}
{{- if .Values.fsc4b.enabled -}}
{{- if ne $w.mode "test" -}}
{{- fail "fsc4b.enabled opens FSC voor Burgers on the demo Stelsel, with a test Activatiedienst that checks no identity: test mode only" -}}
{{- end -}}
{{- if not (include "regelhet.citizenHost" .) -}}
{{- fail "fsc4b.enabled is for citizens: with one instance, set a regelhet.surface other than backoffice" -}}
{{- end -}}
{{- end -}}
{{- if eq $w.login "oidc" -}}
{{- if not (include "regelhet.oidcIssuer" .) -}}
{{- fail "regelhet.login oidc needs oidc.issuerUrl, https://<keycloak>/realms/<realm>, or keycloak.enabled" -}}
{{- end -}}
{{- if not (include "regelhet.oidcSecret" .) -}}
{{- fail "regelhet.login oidc needs oidc.existingSecret, the Secret with the client secrets, or keycloak.enabled" -}}
{{- end -}}
{{- if not (hasPrefix "regelhet-" .Values.oidc.requiredRole) -}}
{{- fail "oidc.requiredRole must be a role of this server (regelhet-...): the realm may be shared" -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{/*
What a link needs, refused at render time as the server would refuse it at
start: a release for an external party is one instance for employees, and
stable mode pins every key.
*/}}
{{- define "regelhet.checkLink" -}}
{{- $k := .Values.koppeling -}}
{{- if $k.partij -}}
{{- if or .Values.surfaces.split (ne .Values.regelhet.surface "backoffice") -}}
{{- fail "koppeling.partij makes this the release of an external party, whose staff work in a back office: set surfaces.split false and regelhet.surface backoffice" -}}
{{- end -}}
{{- end -}}
{{- if and $k.peers (eq .Values.regelhet.mode "stable") -}}
{{- if not $k.keySecret -}}
{{- fail "a link in stable mode signs with a key of its own: set koppeling.keySecret" -}}
{{- end -}}
{{- range $k.peers -}}
{{- if or (not .sleutel) (not (hasPrefix "https://" .url)) -}}
{{- fail (printf "a link in stable mode needs an https url and the public key (sleutel) of every peer: %s" .cel) -}}
{{- end -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{/*
Which entry of oidc.clients an instance logs in with: the back office its
own, `burger` and `all` the citizens' client, since both serve citizens.
*/}}
{{- define "regelhet.clientKey" -}}
{{- ternary "backoffice" "burger" (eq . "backoffice") -}}
{{- end -}}

{{/* The Keycloak of this release: the name of its objects, and its host. */}}
{{- define "regelhet.keycloakName" -}}
{{- printf "%s-keycloak" .Release.Name -}}
{{- end -}}

{{/*
Behind the shared Gateway (gateway.mode listenerSet or shared) a host must
stay one label under the cluster's wildcard, so the default there is
<first label of host>-login beside it, as the employees' host of the plugin is
<name>-medewerkers. With a Gateway of its own it is login.<host>, beside the
default hosts burger.<host> and medewerkers.<host>.
*/}}
{{- define "regelhet.keycloakHost" -}}
{{- if .Values.keycloak.host -}}
{{- .Values.keycloak.host -}}
{{- else -}}
{{- $host := include "regelhet.host" . -}}
{{- if ne .Values.gateway.mode "own" -}}
{{- $labels := splitList "." $host -}}
{{- printf "%s-login.%s" (first $labels) (rest $labels | join ".") -}}
{{- else -}}
{{- printf "login.%s" $host -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{/* The issuer the instances log in at: oidc.issuerUrl, or this release's Keycloak. */}}
{{- define "regelhet.oidcIssuer" -}}
{{- if .Values.oidc.issuerUrl -}}
{{- .Values.oidc.issuerUrl -}}
{{- else if .Values.keycloak.enabled -}}
{{- printf "https://%s/realms/%s" (include "regelhet.keycloakHost" .) .Values.keycloak.realm -}}
{{- end -}}
{{- end -}}

{{/* The Secret with the client secrets: oidc.existingSecret, or the generated one. */}}
{{- define "regelhet.oidcSecret" -}}
{{- if .Values.oidc.existingSecret -}}
{{- .Values.oidc.existingSecret -}}
{{- else if .Values.keycloak.enabled -}}
{{- include "regelhet.keycloakName" . -}}
{{- end -}}
{{- end -}}

{{/*
Everything the Gateway serves, as a JSON list: the instances, each on port
8000, and with keycloak.enabled the Keycloak of this release on port 8080.
Each item has `name` (its Service and certificate), `host`, `suffix` (for the
listener) and `port`. Two items on one host are refused.
*/}}
{{- define "regelhet.routes" -}}
{{- $routes := list -}}
{{- range include "regelhet.instances" . | fromJsonArray -}}
{{- $routes = append $routes (merge (dict "port" 8000) .) -}}
{{- end -}}
{{- if .Values.keycloak.enabled -}}
{{- $host := include "regelhet.keycloakHost" . -}}
{{- range $routes -}}
{{- if eq .host $host -}}
{{- fail "keycloak.host must differ from the hosts of the instances" -}}
{{- end -}}
{{- end -}}
{{- $routes = append $routes (dict "name" (include "regelhet.keycloakName" .) "host" $host "suffix" "-keycloak" "port" 8080) -}}
{{- end -}}
{{- $routes | toJson -}}
{{- end -}}

{{/*
A CloudNativePG Cluster; call with (dict "root" $ "name" <name> "db" <values
with instances, storage and resources> "component" <label>). CNPG makes the
database `app`, owned by the user `app`, and writes its connection to the
secret <name>-app.
*/}}
{{- define "regelhet.cnpgCluster" -}}
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
metadata:
  name: {{ .name }}
  labels:
    {{- include "regelhet.labels" .root | nindent 4 }}
    app.kubernetes.io/component: {{ .component }}
spec:
  instances: {{ .db.instances }}
  # With one instance, CloudNativePG's PodDisruptionBudget allows no
  # disruption at all, so it blocks draining the node, and on a cluster with
  # one node that blocks maintenance for every tenant. The platform's own
  # one-instance databases switch it off for the same reason.
  enablePDB: {{ gt (int .db.instances) 1 }}
  {{- with .root.Values.database.imageCatalog }}
  {{- if .name }}
  imageCatalogRef:
    apiGroup: postgresql.cnpg.io
    kind: {{ .kind }}
    name: {{ .name }}
    major: {{ .major }}
  {{- end }}
  {{- end }}
  storage:
    size: {{ .db.storage.size }}
    {{- with .db.storage.storageClass }}
    storageClass: {{ . }}
    {{- end }}
  resources:
    {{- toYaml .db.resources | nindent 4 }}
{{- end -}}

{{/*
The selector of the Keycloak pods. It has a name of its own: the single
instance of the server selects on regelhet.selectorLabels alone, which
would take these pods in too.
*/}}
{{- define "regelhet.keycloakSelectorLabels" -}}
app.kubernetes.io/name: regelhet-keycloak
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{/*
The host of the instance that serves citizens, where FSC voor Burgers runs;
empty when the only instance is the back office.
*/}}
{{- define "regelhet.citizenHost" -}}
{{- range include "regelhet.instances" . | fromJsonArray -}}
{{- if ne .surface "backoffice" -}}
{{- .host -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{/*
What an organisation's front (voorkant) and a process of an organisation
(organisatie) need, refused at render time like the login above.
*/}}
{{- define "regelhet.checkOrganisatie" -}}
{{- $v := .Values.voorkant -}}
{{- $o := .Values.organisatie -}}
{{- if $v.enabled -}}
{{- if $o.id -}}
{{- fail "voorkant.enabled and organisatie.id exclude each other: a release is an organisation's front or a process" -}}
{{- end -}}
{{- if not $v.id -}}
{{- fail "voorkant.enabled needs voorkant.id, <namespace>/<name> of the Organisatie" -}}
{{- end -}}
{{- if not $v.naam -}}
{{- fail "voorkant.enabled needs voorkant.naam, the name the front shows" -}}
{{- end -}}
{{- if ne .Values.regelhet.mode "test" -}}
{{- fail "voorkant.enabled runs in test mode only for now: the front keeps its sessions in memory" -}}
{{- end -}}
{{- if .Values.fsc4b.enabled -}}
{{- fail "fsc4b.enabled is for a process: an organisation's front serves no FSC voor Burgers" -}}
{{- end -}}
{{- if or .Values.koppeling.partij .Values.koppeling.peers .Values.koppeling.keySecret -}}
{{- fail "koppeling links the processes of a case: an organisation's front has no cell, so set it on the processes" -}}
{{- end -}}
{{- $ids := list -}}
{{- range $v.processen -}}
{{- if has .id $ids -}}
{{- fail (printf "voorkant.processen names %s twice" .id) -}}
{{- end -}}
{{- $ids = append $ids .id -}}
{{- end -}}
{{- end -}}
{{- if $o.id -}}
{{- if not (and $o.url $o.medewerkersUrl) -}}
{{- fail "organisatie.id needs organisatie.url and organisatie.medewerkersUrl, for the link back" -}}
{{- end -}}
{{- if eq .Values.regelhet.mode "stable" -}}
{{- if not $o.sleutelJwk -}}
{{- fail "organisatie.id in stable mode needs organisatie.sleutelJwk: a stable process fetches no key over the network" -}}
{{- end -}}
{{- else if not (or $o.sleutelUrl $o.sleutelJwk) -}}
{{- fail "organisatie.id needs organisatie.sleutelUrl or organisatie.sleutelJwk, the key of the front" -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{/* The ConfigMap with the processes and the Secret with the key of a front. */}}
{{- define "regelhet.organisatieName" -}}
{{- printf "%s-organisatie" .Release.Name -}}
{{- end -}}

{{/*
The name of the Job that generates the Secrets. With Keycloak it keeps the
name it had when it made only the Secret of Keycloak, so an upgrade replaces
its objects instead of leaving the old ones behind.
*/}}
{{- define "regelhet.secretsJobName" -}}
{{- if .Values.keycloak.enabled -}}
{{- printf "%s-secrets" (include "regelhet.keycloakName" .) -}}
{{- else -}}
{{- printf "%s-secrets" .Release.Name -}}
{{- end -}}
{{- end -}}
