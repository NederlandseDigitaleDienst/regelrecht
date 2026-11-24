package main

import (
	"fmt"
	"net/url"
	"regexp"
	"strings"

	"github.com/caarlos0/env/v11"
	"k8s.io/apimachinery/pkg/util/validation"
)

// forkURL is the repository the chart comes from, and the default case.
const forkURL = "https://code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026.git"

// The Let's Encrypt environments of the chart's Issuer (tls.acme).
const (
	acmeStaging    = "staging"
	acmeProduction = "production"
)

// pluginConfig holds the install-time configuration from spec.configSchema in
// definition.yaml, which the plugin-controller injects as FUNP_* env vars. It
// holds defaults only: every RegelHet is a RegelHet object of its
// own, with its case, version, colour and address in its spec.
//
// The Keycloak of stable mode will be one more default here, its URL, once
// the chart supports stable mode.
type pluginConfig struct {
	// Domain is the base domain: a RegelHet with hostname x is served on
	// x.<domain>, unless its hostname is a domain name of its own.
	Domain string `env:"FUNP_DOMAIN,required"`
	// DefaultImage is the image repository of the RegelHet server. A
	// RegelHet with a version runs <DefaultImage>:<version>.
	DefaultImage string `env:"FUNP_DEFAULT_IMAGE" envDefault:"code.digitaledienst.overheid.nl/regelrecht/regelhet"`
	// ChartRef is the branch or tag of the fork that the chart is read from.
	ChartRef string `env:"FUNP_CHART_REF" envDefault:"main"`
	// ACME is the Let's Encrypt environment of the certificates.
	ACME string `env:"FUNP_ACME" envDefault:"production"`
}

func loadConfig() (pluginConfig, error) {
	cfg, err := env.ParseAs[pluginConfig]()
	if err != nil {
		return pluginConfig{}, fmt.Errorf("parse config: %w", err)
	}
	if err := cfg.validate(); err != nil {
		return pluginConfig{}, err
	}
	return cfg, nil
}

// gitRefPattern accepts a branch or tag name. It refuses what git refuses in a
// ref name and a leading dash, which git clone would read as an option.
var gitRefPattern = regexp.MustCompile(`^[A-Za-z0-9._/][A-Za-z0-9._/-]*$`)

// imageRepositoryPattern accepts an image repository without a tag, a digest,
// whitespace or a scheme.
var imageRepositoryPattern = regexp.MustCompile(`^[a-z0-9]([a-z0-9._-]*[a-z0-9])?(:[0-9]+)?(/[a-z0-9._-]+)+$`)

func (c pluginConfig) validate() error {
	if err := validateRef("CHART_REF", c.ChartRef); err != nil {
		return err
	}
	if errs := validation.IsDNS1123Subdomain(c.Domain); len(errs) > 0 {
		return fmt.Errorf("DOMAIN %q is not a valid domain: %s", c.Domain, strings.Join(errs, "; "))
	}
	if !imageRepositoryPattern.MatchString(c.DefaultImage) {
		return fmt.Errorf("DEFAULT_IMAGE must be an image repository without a tag, got %q", c.DefaultImage)
	}
	if c.ACME != acmeStaging && c.ACME != acmeProduction {
		return fmt.Errorf("ACME must be %q or %q, got %q", acmeStaging, acmeProduction, c.ACME)
	}
	return nil
}

// validateHTTPS accepts an https:// URL without credentials in it: those
// belong in a Secret, not in a spec anyone with read access sees.
func validateHTTPS(key, raw string) error {
	u, err := url.Parse(raw)
	if err != nil {
		return fmt.Errorf("%s: %w", key, err)
	}
	if u.Scheme != "https" || u.Host == "" {
		return fmt.Errorf("%s must be an https:// URL, got %q", key, raw)
	}
	if u.User != nil {
		return fmt.Errorf("%s must not carry credentials: name a Secret in credentialsSecretRef", key)
	}
	return nil
}

func validateRef(key, ref string) error {
	if !gitRefPattern.MatchString(ref) || strings.Contains(ref, "..") || strings.HasSuffix(ref, ".lock") {
		return fmt.Errorf("%s must be a branch or tag name, got %q", key, ref)
	}
	if strings.HasPrefix(ref, "refs/") {
		return fmt.Errorf("%s must be a short branch or tag name such as main, not a full reference, got %q", key, ref)
	}
	return nil
}
