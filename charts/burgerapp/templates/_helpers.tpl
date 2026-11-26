{{- define "burgerapp.labels" -}}
app.kubernetes.io/name: burgerapp
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end -}}

{{- define "burgerapp.selectorLabels" -}}
app.kubernetes.io/name: burgerapp
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{/* The host the app is served on. */}}
{{- define "burgerapp.host" -}}
{{- if .Values.host -}}
{{ .Values.host }}
{{- else -}}
{{ printf "burgerapp.%s" (required "set domain or host" .Values.domain) }}
{{- end -}}
{{- end -}}

{{/*
The list as the app reads it (src/burgeromgeving/fsc4b/config.ts), checked
here as far as a template can: an id and a name, and an https address.
*/}}
{{- define "burgerapp.regelhets" -}}
{{- $lijst := list -}}
{{- range $i, $r := .Values.regelhets -}}
{{- $id := required (printf "regelhets[%d].id" $i) $r.id -}}
{{- $naam := required (printf "regelhets[%d].naam" $i) $r.naam -}}
{{- $adres := required (printf "regelhets[%d].adres" $i) $r.adres -}}
{{- if not (hasPrefix "https://" $adres) -}}
{{- fail (printf "regelhets[%d].adres is no https address: %s" $i $adres) -}}
{{- end -}}
{{- $entry := dict "id" $id "naam" $naam "adres" (trimSuffix "/" $adres) -}}
{{- with $r.oin }}{{ $_ := set $entry "oin" (toString .) }}{{ end -}}
{{- $lijst = append $lijst $entry -}}
{{- end -}}
{{- dict "regelhets" $lijst | toPrettyJson -}}
{{- end -}}

{{/* The origins of the RegelHets, for the CSP's connect-src. */}}
{{- define "burgerapp.origins" -}}
{{- $origins := list -}}
{{- range .Values.regelhets -}}
{{- $u := urlParse .adres -}}
{{- $origins = append $origins (printf "%s://%s" $u.scheme $u.host) -}}
{{- end -}}
{{- $origins | uniq | join " " -}}
{{- end -}}
