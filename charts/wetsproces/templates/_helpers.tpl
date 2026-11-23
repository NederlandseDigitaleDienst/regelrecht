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
