package main

import (
	"fmt"
	"net"
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
// definition.yaml, which the plugin-controller injects as FUNP_* env vars.
type pluginConfig struct {
	// CaseRepoURL is the public repository with the case. HTTPS only: the
	// repository is public and needs no credential.
	CaseRepoURL string `env:"FUNP_CASE_REPO_URL" envDefault:"https://code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026.git"`
	// CaseRef is the branch or tag of the case repository.
	CaseRef string `env:"FUNP_CASE_REF" envDefault:"main"`
	// CasePath is the path of the source manifest (bron.yaml) in the case
	// repository.
	CasePath string `env:"FUNP_CASE_PATH" envDefault:"packages/wetsproces/bron.yaml"`
	// Namespace is the existing Fundament namespace (tnt-<project>--<name>)
	// the app goes into. Fundament creates namespaces; the plugin never does.
	Namespace string `env:"FUNP_NAMESPACE,required"`
	// Domain is the base domain; the app is served on wetsproces.<domain>.
	Domain string `env:"FUNP_DOMAIN,required"`
	// Image is the wetsproces image.
	Image string `env:"FUNP_IMAGE" envDefault:"code.digitaledienst.overheid.nl/regelrecht/wetsproces:f619755c457d30babec9d7e200092e30794a79f7"`
	// ChartRef is the branch or tag of the fork that the chart is read from.
	ChartRef string `env:"FUNP_CHART_REF" envDefault:"main"`
	// GatewayAddress is a fixed public address for the Gateway; empty lets
	// the LoadBalancer pool pick one.
	GatewayAddress string `env:"FUNP_GATEWAY_ADDRESS"`
	// ACME is the Let's Encrypt environment of the certificate.
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

// imagePattern accepts an image reference with a tag or a digest, without
// whitespace or a scheme.
var imagePattern = regexp.MustCompile(`^[a-z0-9]([a-z0-9._-]*[a-z0-9])?(:[0-9]+)?(/[a-z0-9._-]+)+(:[A-Za-z0-9._-]+)?(@sha256:[a-f0-9]{64})?$`)

func (c pluginConfig) validate() error {
	if err := validatePublicHTTPS("CASE_REPO_URL", c.CaseRepoURL); err != nil {
		return err
	}
	if err := validateRef("CASE_REF", c.CaseRef); err != nil {
		return err
	}
	if err := validateRef("CHART_REF", c.ChartRef); err != nil {
		return err
	}
	if c.CasePath == "" || strings.HasPrefix(c.CasePath, "/") || strings.Contains(c.CasePath, "..") {
		return fmt.Errorf("CASE_PATH must be a relative path inside the repository, got %q", c.CasePath)
	}
	if errs := validation.IsDNS1123Subdomain(c.Domain); len(errs) > 0 {
		return fmt.Errorf("DOMAIN %q is not a valid domain: %s", c.Domain, strings.Join(errs, "; "))
	}
	if !imagePattern.MatchString(c.Image) || !strings.ContainsAny(c.Image, ":@") {
		return fmt.Errorf("IMAGE must be an image reference with a tag or digest, got %q", c.Image)
	}
	if errs := validation.IsDNS1123Label(c.Namespace); len(errs) > 0 || !strings.HasPrefix(c.Namespace, "tnt-") {
		return fmt.Errorf("NAMESPACE must be the full name of a Fundament namespace, tnt-<project>--<name>, got %q", c.Namespace)
	}
	if c.GatewayAddress != "" && net.ParseIP(c.GatewayAddress) == nil {
		return fmt.Errorf("GATEWAY_ADDRESS must be an IP address or empty, got %q", c.GatewayAddress)
	}
	if c.ACME != acmeStaging && c.ACME != acmeProduction {
		return fmt.Errorf("ACME must be %q or %q, got %q", acmeStaging, acmeProduction, c.ACME)
	}
	return nil
}

func validatePublicHTTPS(key, raw string) error {
	u, err := url.Parse(raw)
	if err != nil {
		return fmt.Errorf("%s: %w", key, err)
	}
	if u.Scheme != "https" || u.Host == "" {
		return fmt.Errorf("%s must be an https:// URL, got %q", key, raw)
	}
	if u.User != nil {
		return fmt.Errorf("%s must not carry credentials: the repository must be public", key)
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
