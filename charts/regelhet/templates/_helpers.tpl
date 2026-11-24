{{/* The hostname the app is served on. */}}
{{- define "regelhet.host" -}}
{{- if .Values.host -}}
{{- .Values.host -}}
{{- else -}}
{{- printf "regelhet.%s" (required "domain is required" .Values.domain) -}}
{{- end -}}
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
*/}}
{{- define "regelhet.instances" -}}
{{- $base := include "regelhet.host" . -}}
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
{{- list (dict "name" .Release.Name "surface" .Values.regelhet.surface "host" $base "suffix" "") | toJson -}}
{{- end -}}
{{- end -}}
