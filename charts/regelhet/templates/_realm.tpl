{{/*
The realm of the RegelHet server, as docs/src/content/docs/auth-and-roles.md
describes it, with a client per side an instance of this release serves: a
RealmRepresentation in YAML. The client secrets and the test password are
placeholders, ${REGELHET_<SIDE>_CLIENT_SECRET} and ${REGELHET_TEST_PASSWORD},
that Keycloak fills in from its environment at import, or the Keycloak
operator from a Secret.
*/}}
{{- define "regelhet.realm" -}}
{{- $clients := .Values.oidc.clients }}
{{- /* A client per side that an instance serves, on that instance's host. */}}
{{- $hosts := dict }}
{{- range include "regelhet.instances" . | fromJsonArray }}
{{- $_ := set $hosts (include "regelhet.clientKey" .surface) .host }}
{{- end }}
{{- $roleMapper := dict
      "name" "realm roles"
      "protocol" "openid-connect"
      "protocolMapper" "oidc-usermodel-realm-role-mapper"
      "config" (dict
        "claim.name" "realm_access.roles"
        "multivalued" "true"
        "jsonType.label" "String"
        "id.token.claim" "true"
        "access.token.claim" "true"
        "userinfo.token.claim" "true") }}
{{- $bsnMapper := dict
      "name" "bsn"
      "protocol" "openid-connect"
      "protocolMapper" "oidc-usermodel-attribute-mapper"
      "config" (dict
        "user.attribute" "bsn"
        "claim.name" "bsn"
        "jsonType.label" "String"
        "id.token.claim" "true"
        "access.token.claim" "true"
        "userinfo.token.claim" "true") }}
{{- /*
A case that logs in on another key than bsn (regelhet.subjectClaim, such as
kvk_nummer): its claim from the user attribute of that name, beside bsn.
*/}}
{{- $claim := .Values.regelhet.subjectClaim }}
{{- $claimMapper := dict
      "name" $claim
      "protocol" "openid-connect"
      "protocolMapper" "oidc-usermodel-attribute-mapper"
      "config" (dict
        "user.attribute" $claim
        "claim.name" $claim
        "jsonType.label" "String"
        "id.token.claim" "true"
        "access.token.claim" "true"
        "userinfo.token.claim" "true") }}
{{- /*
The realm's user profile: Keycloak's default attributes, and `bsn`, which only
an admin may change. Without it Keycloak drops the attribute of every user.
Display names are plain text: the operator would read `${...}` as a
placeholder.
*/}}
{{- $name := dict "length" (dict "max" 255) "person-name-prohibited-characters" (dict) }}
{{- $own := dict "view" (list "admin" "user") "edit" (list "admin" "user") }}
{{- $profile := dict
      "attributes" (list
        (dict "name" "username" "displayName" "Username" "multivalued" false "permissions" $own
          "validations" (dict "length" (dict "min" 3 "max" 255) "username-prohibited-characters" (dict) "up-username-not-idn-homograph" (dict)))
        (dict "name" "email" "displayName" "Email" "multivalued" false "permissions" $own
          "required" (dict "roles" (list "user"))
          "validations" (dict "email" (dict) "length" (dict "max" 255)))
        (dict "name" "firstName" "displayName" "First name" "multivalued" false "permissions" $own
          "required" (dict "roles" (list "user")) "validations" $name)
        (dict "name" "lastName" "displayName" "Last name" "multivalued" false "permissions" $own
          "required" (dict "roles" (list "user")) "validations" $name)
        (dict "name" "bsn" "displayName" "BSN" "multivalued" false
          "permissions" (dict "view" (list "admin" "user") "edit" (list "admin"))
          "validations" (dict "pattern" (dict "pattern" "^[0-9]{9}$" "error-message" "nine digits"))))
      "groups" (list) }}
{{- if and $claim (ne $claim "bsn") }}
{{- $validations := dict "length" (dict "max" 64) }}
{{- if eq $claim "kvk_nummer" }}
{{- $validations = dict "pattern" (dict "pattern" "^[0-9]{8}$" "error-message" "eight digits") }}
{{- end }}
{{- $_ := set $profile "attributes" (append $profile.attributes (dict "name" $claim "displayName" (ternary "KvK-nummer" $claim (eq $claim "kvk_nummer")) "multivalued" false
      "permissions" (dict "view" (list "admin" "user") "edit" (list "admin"))
      "validations" $validations)) }}
{{- end }}
realm: {{ .Values.keycloak.realm }}
enabled: true
sslRequired: external
registrationAllowed: false
# The login gate holds nothing; the other roles contain it, and each step
# of the employees' ladder contains the one below.
roles:
  realm:
    - name: regelhet-toegang
      description: Logging in at all
    - name: regelhet-burger
      description: A citizen
      composite: true
      composites:
        realm: [regelhet-toegang]
    - name: regelhet-lezer
      description: An employee who reads the executor's chronicle
      composite: true
      composites:
        realm: [regelhet-toegang]
    - name: regelhet-beslisser
      description: Also records facts, sends copies of decisions and takes decisions
      composite: true
      composites:
        realm: [regelhet-lezer]
    - name: regelhet-admin
      description: Also sets the test clock and enters what an external party did
      composite: true
      composites:
        realm: [regelhet-beslisser]
components:
  org.keycloak.userprofile.UserProfileProvider:
    - providerId: declarative-user-profile
      subComponents: {}
      config:
        kc.user.profile.config:
          - {{ toJson $profile | quote }}
clients:
  {{- range $side, $host := $hosts }}
  {{- $client := get $clients $side }}
  - clientId: {{ $client.clientId }}
    name: {{ ternary "RegelHet, citizens" "RegelHet, back office" (eq $side "burger") }}
    enabled: true
    publicClient: false
    clientAuthenticatorType: client-secret
    secret: {{ printf "${REGELHET_%s_CLIENT_SECRET}" (upper $side) | quote }}
    standardFlowEnabled: true
    directAccessGrantsEnabled: false
    implicitFlowEnabled: false
    serviceAccountsEnabled: false
    redirectUris:
      - https://{{ $host }}/auth/callback
    webOrigins:
      - https://{{ $host }}
    attributes:
      pkce.code.challenge.method: S256
      post.logout.redirect.uris: https://{{ $host }}
    protocolMappers:
      - {{- toYaml $roleMapper | nindent 12 }}
      {{- if eq $side "burger" }}
      - {{- toYaml $bsnMapper | nindent 12 }}
      {{- if and $claim (ne $claim "bsn") }}
      - {{- toYaml $claimMapper | nindent 12 }}
      {{- end }}
      {{- end }}
  {{- end }}
{{- if .Values.keycloak.testUsers }}
{{- /* The citizens of the test login (TEST_CITIZENS in packages/fsc4b). */}}
users:
  {{- $citizens := list
        (dict "username" "anna.voorbeeld" "first" "Anna" "last" "Voorbeeld" "bsn" "999990032")
        (dict "username" "bram.proef" "first" "Bram" "last" "Proef" "bsn" "999990020") }}
  {{- if and $claim (ne $claim "bsn") .Values.keycloak.testApplicants }}
  {{- /* The applicants of the case, under its own key (the plugin reads them from its begin data). */}}
  {{- range .Values.keycloak.testApplicants }}
  - username: {{ .username | quote }}
    enabled: true
    email: {{ printf "%s@example.org" .username | quote }}
    emailVerified: true
    firstName: {{ default .username .name | quote }}
    {{- /* Keycloak's name is first and last name together; the number is in its own attribute. */}}
    lastName: "Test"
    attributes:
      {{ $claim }}: [{{ .number | quote }}]
    realmRoles: [regelhet-burger]
    credentials:
      - type: password
        value: "${REGELHET_TEST_PASSWORD}"
        temporary: false
  {{- end }}
  {{- else }}
  {{- range $citizens }}
  - username: {{ .username }}
    enabled: true
    email: {{ .username }}@example.org
    emailVerified: true
    firstName: {{ .first }}
    lastName: {{ .last }}
    attributes:
      bsn: [{{ .bsn | quote }}]
    realmRoles: [regelhet-burger]
    credentials:
      - type: password
        value: "${REGELHET_TEST_PASSWORD}"
        temporary: false
  {{- end }}
  {{- end }}
  {{- range $role := list "beslisser" "admin" }}
  - username: {{ $role }}
    enabled: true
    email: {{ $role }}@example.org
    emailVerified: true
    firstName: Test
    lastName: {{ $role | title }}
    realmRoles: [regelhet-{{ $role }}]
    credentials:
      - type: password
        value: "${REGELHET_TEST_PASSWORD}"
        temporary: false
  {{- end }}
{{- end }}
{{- end -}}

{{/* The sides the realm has a client for, comma-separated. */}}
{{- define "regelhet.realmSides" -}}
{{- $sides := list -}}
{{- range include "regelhet.instances" . | fromJsonArray -}}
{{- $sides = append $sides (include "regelhet.clientKey" .surface) -}}
{{- end -}}
{{- $sides | uniq | sortAlpha | join "," -}}
{{- end -}}
