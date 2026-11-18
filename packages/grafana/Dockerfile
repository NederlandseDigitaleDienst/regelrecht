FROM grafana/grafana-oss:12.4.0@sha256:b0ae311af06228bcfd4a620504b653db80f5b91e94dc3dc2a5b7dab202bcde20

# Published as ghcr.io/<repo owner>/regelrecht-grafana; deploy.yml rebuilds it
# only on a push to main that changes something under packages/grafana/.
# There is no manual trigger, so a build skipped by a red CI waits for the next
# change in this directory.

# ZAD requires port 8000 for liveprobe
ENV GF_SERVER_HTTP_PORT=8000
ENV GF_SERVER_ROOT_URL=https://grafana.regelrecht.rijks.app
ENV GF_ANALYTICS_CHECK_FOR_UPDATES=false
ENV GF_ANALYTICS_CHECK_FOR_PLUGIN_UPDATES=false

# Disable anonymous access
ENV GF_AUTH_ANONYMOUS_ENABLED=false

# Security headers. Grafana renders its own, so this is the third place they
# are decided next to packages/auth/src/security_headers.rs (the Axum
# services) and deploy/nginx/security-headers.conf (docs and lawmaking).
# Grafana ships no setting for Referrer-Policy or Permissions-Policy, so this
# host sends neither; adding them would take a proxy in front of it.
#
# Defaults already cover part of it: x_content_type_options and
# x_xss_protection are on, and allow_embedding=false yields
# `X-Frame-Options: deny`. HSTS and CSP are off by default and are what this
# block turns on.
ENV GF_SECURITY_STRICT_TRANSPORT_SECURITY=true
ENV GF_SECURITY_STRICT_TRANSPORT_SECURITY_MAX_AGE_SECONDS=31536000
ENV GF_SECURITY_STRICT_TRANSPORT_SECURITY_SUBDOMAINS=true
ENV GF_SECURITY_STRICT_TRANSPORT_SECURITY_PRELOAD=true
ENV GF_SECURITY_CONTENT_SECURITY_POLICY=true
# Grafana's own template with `frame-ancestors 'none'` appended. Its
# `'unsafe-eval'` and `'unsafe-inline'` are upstream's and cannot be removed
# without breaking the UI; `'strict-dynamic'` plus the per-request $NONCE
# makes a modern browser ignore the `'unsafe-inline'` anyway. Anything beyond
# this — a `default-src`, a narrower `img-src` — has to be tried against a
# running Grafana first, because its panels load workers and images from
# places the template deliberately leaves open.
#
# The `\$` are escaped for the Dockerfile parser, not for Grafana: an
# unescaped `$NONCE` is expanded at build time and reaches Grafana empty,
# which drops the nonce that makes `'strict-dynamic'` work.
ENV GF_SECURITY_CONTENT_SECURITY_POLICY_TEMPLATE="script-src 'self' 'unsafe-eval' 'unsafe-inline' 'strict-dynamic' \$NONCE;object-src 'none';font-src 'self';style-src 'self' 'unsafe-inline' blob:;img-src * data:;base-uri 'self';connect-src 'self' grafana.com ws://\$ROOT_PATH wss://\$ROOT_PATH;manifest-src 'self';media-src 'none';form-action 'self';frame-ancestors 'none';"

# The entrypoint hides the local login form when OIDC is available. When the
# OIDC vars are missing, the local admin account (random password unless
# GF_SECURITY_ADMIN_PASSWORD is set) is the only way in. Do NOT expose
# publicly without OIDC.
# NOTE: GF_SECURITY_SECRET_KEY must be injected at deploy time via ZAD env_vars
# to keep sessions stable across pod restarts.

# OIDC is conditionally enabled in entrypoint.sh based on whether
# ZAD injects OIDC_CLIENT_ID, OIDC_CLIENT_SECRET, OIDC_URL, OIDC_REALM.
# These static OIDC settings apply when OIDC is enabled:
ENV GF_AUTH_GENERIC_OAUTH_NAME=Keycloak
ENV GF_AUTH_GENERIC_OAUTH_ALLOW_SIGN_UP=true
ENV GF_AUTH_GENERIC_OAUTH_SCOPES="openid email profile"
# Map Keycloak roles to Grafana roles — users in 'grafana-admin' group get Admin
ENV GF_AUTH_GENERIC_OAUTH_ROLE_ATTRIBUTE_PATH="contains(groups[*], 'grafana-admin') && 'Admin' || 'Viewer'"

# Enable unified alerting
ENV GF_UNIFIED_ALERTING_ENABLED=true
ENV GF_ALERTING_ENABLED=false

# Serve dashboards through the Kubernetes-style API.
ENV GF_FEATURE_TOGGLES_ENABLE=kubernetesDashboards

# Copy provisioning files and entrypoint (includes GF_SECURITY_ADMIN_PASSWORD default fix)
COPY provisioning/ /etc/grafana/provisioning/
COPY entrypoint.sh /entrypoint.sh

EXPOSE 8000

ENTRYPOINT ["/entrypoint.sh"]
