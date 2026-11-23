{{/* The hostname the app is served on. */}}
{{- define "wetsproces.host" -}}
{{- if .Values.host -}}
{{- .Values.host -}}
{{- else -}}
{{- printf "wetsproces.%s" (required "domain is required" .Values.domain) -}}
{{- end -}}
{{- end -}}

{{/* The case path, refused when it climbs out of the clone. */}}
{{- define "wetsproces.casePath" -}}
{{- if contains ".." .Values.case.path -}}
{{- fail "case.path must stay inside the repository" -}}
{{- end -}}
{{- .Values.case.path -}}
{{- end -}}

{{- define "wetsproces.labels" -}}
app.kubernetes.io/name: wetsproces
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end -}}

{{- define "wetsproces.selectorLabels" -}}
app.kubernetes.io/name: wetsproces
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{/* The name of the CNPG Cluster; CNPG names its app secret <cluster>-app. */}}
{{- define "wetsproces.dbCluster" -}}
{{- printf "%s-db" .Release.Name -}}
{{- end -}}

{{/*
The instances of the server, as a JSON list: one with the surface of
wetsproces.surface on the host of wetsproces.host, or, with surfaces.split,
one per surface on a host of its own. Each item has `name` (the name of its
Deployment and Service), `surface`, `host` and `suffix` (empty for the single
instance, so its names stay what they were).
*/}}
{{- define "wetsproces.instances" -}}
{{- $base := include "wetsproces.host" . -}}
{{- if .Values.surfaces.split -}}
{{- if not .Values.database.enabled -}}
{{- fail "surfaces.split needs database.enabled: the two instances share their chronicles, and the store in memory cannot be shared" -}}
{{- end -}}
{{- $burger := .Values.surfaces.burger.host | default (printf "burger.%s" $base) -}}
{{- $backoffice := .Values.surfaces.backoffice.host | default (printf "medewerkers.%s" $base) -}}
{{- if eq $burger $backoffice -}}
{{- fail "surfaces.burger.host and surfaces.backoffice.host must differ: two HTTPS listeners on one host conflict" -}}
{{- end -}}
{{- list
  (dict "name" (printf "%s-burger" .Release.Name) "surface" "burger" "host" $burger "suffix" "-burger")
  (dict "name" (printf "%s-backoffice" .Release.Name) "surface" "backoffice" "host" $backoffice "suffix" "-backoffice")
  | toJson -}}
{{- else -}}
{{- list (dict "name" .Release.Name "surface" .Values.wetsproces.surface "host" $base "suffix" "") | toJson -}}
{{- end -}}
{{- end -}}

{{/*
What stable mode and the login through Keycloak need, refused at render time
so that a release never rolls out a server that stops at start. The server
checks the same; these guards only say it earlier.
*/}}
{{- define "wetsproces.checkLogin" -}}
{{- $w := .Values.wetsproces -}}
{{- if eq $w.mode "stable" -}}
{{- if not .Values.database.enabled -}}
{{- fail "wetsproces.mode stable keeps the chronicles in PostgreSQL: set database.enabled" -}}
{{- end -}}
{{- range include "wetsproces.instances" . | fromJsonArray -}}
{{- if eq .surface "all" -}}
{{- fail "wetsproces.mode stable serves each side on an instance of its own: set surfaces.split, or a wetsproces.surface other than all" -}}
{{- end -}}
{{- end -}}
{{- if ne $w.login "oidc" -}}
{{- fail "wetsproces.mode stable logs in through Keycloak only: set wetsproces.login oidc" -}}
{{- end -}}
{{- end -}}
{{- if eq $w.login "oidc" -}}
{{- if not (include "wetsproces.oidcIssuer" .) -}}
{{- fail "wetsproces.login oidc needs oidc.issuerUrl, https://<keycloak>/realms/<realm>, or keycloak.enabled" -}}
{{- end -}}
{{- if not (include "wetsproces.oidcSecret" .) -}}
{{- fail "wetsproces.login oidc needs oidc.existingSecret, the Secret with the client secrets, or keycloak.enabled" -}}
{{- end -}}
{{- if not (hasPrefix "wetsproces-" .Values.oidc.requiredRole) -}}
{{- fail "oidc.requiredRole must be a role of this server (wetsproces-...): the realm may be shared" -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{/*
Which entry of oidc.clients an instance logs in with: the back office its
own, `burger` and `all` the citizens' client, since both serve citizens.
*/}}
{{- define "wetsproces.clientKey" -}}
{{- ternary "backoffice" "burger" (eq . "backoffice") -}}
{{- end -}}

{{/* The Keycloak of this release: the name of its objects, and its host. */}}
{{- define "wetsproces.keycloakName" -}}
{{- printf "%s-keycloak" .Release.Name -}}
{{- end -}}

{{/*
Behind the shared Gateway (gateway.mode listenerSet or shared) a host must
stay one label under the cluster's wildcard, so the default there is
<first label of host>-login beside it, as the employees' host of the plugin is
<name>-medewerkers. With a Gateway of its own it is login.<host>, beside the
default hosts burger.<host> and medewerkers.<host>.
*/}}
{{- define "wetsproces.keycloakHost" -}}
{{- if .Values.keycloak.host -}}
{{- .Values.keycloak.host -}}
{{- else -}}
{{- $host := include "wetsproces.host" . -}}
{{- if ne .Values.gateway.mode "own" -}}
{{- $labels := splitList "." $host -}}
{{- printf "%s-login.%s" (first $labels) (rest $labels | join ".") -}}
{{- else -}}
{{- printf "login.%s" $host -}}
{{- end -}}
{{- end -}}
{{- end -}}

{{/* The issuer the instances log in at: oidc.issuerUrl, or this release's Keycloak. */}}
{{- define "wetsproces.oidcIssuer" -}}
{{- if .Values.oidc.issuerUrl -}}
{{- .Values.oidc.issuerUrl -}}
{{- else if .Values.keycloak.enabled -}}
{{- printf "https://%s/realms/%s" (include "wetsproces.keycloakHost" .) .Values.keycloak.realm -}}
{{- end -}}
{{- end -}}

{{/* The Secret with the client secrets: oidc.existingSecret, or the generated one. */}}
{{- define "wetsproces.oidcSecret" -}}
{{- if .Values.oidc.existingSecret -}}
{{- .Values.oidc.existingSecret -}}
{{- else if .Values.keycloak.enabled -}}
{{- include "wetsproces.keycloakName" . -}}
{{- end -}}
{{- end -}}

{{/*
Everything the Gateway serves, as a JSON list: the instances, each on port
8000, and with keycloak.enabled the Keycloak of this release on port 8080.
Each item has `name` (its Service and certificate), `host`, `suffix` (for the
listener) and `port`. Two items on one host are refused.
*/}}
{{- define "wetsproces.routes" -}}
{{- $routes := list -}}
{{- range include "wetsproces.instances" . | fromJsonArray -}}
{{- $routes = append $routes (merge (dict "port" 8000) .) -}}
{{- end -}}
{{- if .Values.keycloak.enabled -}}
{{- $host := include "wetsproces.keycloakHost" . -}}
{{- range $routes -}}
{{- if eq .host $host -}}
{{- fail "keycloak.host must differ from the hosts of the instances" -}}
{{- end -}}
{{- end -}}
{{- $routes = append $routes (dict "name" (include "wetsproces.keycloakName" .) "host" $host "suffix" "-keycloak" "port" 8080) -}}
{{- end -}}
{{- $routes | toJson -}}
{{- end -}}

{{/*
A CloudNativePG Cluster; call with (dict "root" $ "name" <name> "db" <values
with instances, storage and resources> "component" <label>). CNPG makes the
database `app`, owned by the user `app`, and writes its connection to the
secret <name>-app.
*/}}
{{- define "wetsproces.cnpgCluster" -}}
apiVersion: postgresql.cnpg.io/v1
kind: Cluster
metadata:
  name: {{ .name }}
  labels:
    {{- include "wetsproces.labels" .root | nindent 4 }}
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
instance of the server selects on wetsproces.selectorLabels alone, which
would take these pods in too.
*/}}
{{- define "wetsproces.keycloakSelectorLabels" -}}
app.kubernetes.io/name: wetsproces-keycloak
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}
