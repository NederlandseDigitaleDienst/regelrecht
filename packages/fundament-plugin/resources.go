package main

import (
	"fmt"
	"strings"

	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/runtime/schema"
	"k8s.io/apimachinery/pkg/util/validation"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

const (
	pluginName = "wetsproces"
	fieldOwner = "wetsproces-plugin"

	chartPath    = "charts/wetsproces"
	fluxInterval = "10m"
	// releaseTimeout is how long helm-controller waits for the pods of an
	// install or upgrade, 5m by default. A wetsproces starts in under a
	// minute; a shorter wait lets a corrected spec apply sooner after a
	// failed one. The plugin reports a failing pod before this ends.
	releaseTimeout = "3m"

	// nameLabel carries the name of the Wetsproces on everything the plugin
	// makes for it.
	nameLabel      = "wetsproces.regelrecht.nl/name"
	managedByLabel = "app.kubernetes.io/managed-by"

	// The keys of a Secret in credentialsSecretRef.
	secretUsernameKey = "username"
	secretTokenKey    = "token"
)

var (
	gitRepositoryGVK = schema.GroupVersionKind{Group: "source.toolkit.fluxcd.io", Version: "v1", Kind: "GitRepository"}
	helmReleaseGVK   = schema.GroupVersionKind{Group: "helm.toolkit.fluxcd.io", Version: "v2", Kind: "HelmRelease"}
)

// sourceName is the GitRepository with the chart of the Wetsproces w. The
// HelmRelease, the Helm release and the chart's own objects (Deployments,
// Services, Gateway, database) are named after w itself, so two Wetsproces
// objects in one namespace never share an object.
func sourceName(w *v1alpha1.Wetsproces) string { return w.Name + "-chart" }

// hosts are the addresses of w: citizens on its hostname under the base
// domain, employees on <hostname>-medewerkers beside it. Both stay one label
// under the domain, where the cluster's wildcard record covers them;
// medewerkers.<hostname>.<domain> would not be. A hostname with a dot is a
// domain of its own, and employees get medewerkers.<hostname> under it.
//
// In stable mode a third address carries its Keycloak: <hostname>-login
// beside the citizens' host, which is what the chart makes of an empty
// keycloak.host behind the shared Gateway, one label under the wildcard.
type hosts struct {
	burger     string
	backoffice string
	// login is empty in test mode.
	login string
}

func hostsOf(w *v1alpha1.Wetsproces, domain string) hosts {
	name := w.Spec.Hostname
	var h hosts
	if strings.Contains(name, ".") {
		first, rest, _ := strings.Cut(name, ".")
		h = hosts{burger: name, backoffice: "medewerkers." + name, login: first + "-login." + rest}
	} else {
		h = hosts{burger: name + "." + domain, backoffice: name + "-medewerkers." + domain, login: name + "-login." + domain}
	}
	if !isStable(w) {
		h.login = ""
	}
	return h
}

// all is every address of h.
func (h hosts) all() []string {
	out := []string{h.burger, h.backoffice}
	if h.login != "" {
		out = append(out, h.login)
	}
	return out
}

// overlaps reports whether h and o share an address.
func (h hosts) overlaps(o hosts) bool {
	for _, a := range h.all() {
		for _, b := range o.all() {
			if a == b {
				return true
			}
		}
	}
	return false
}

// isStable reports whether w logs in through a Keycloak of its own.
func isStable(w *v1alpha1.Wetsproces) bool {
	return w.Spec.Auth.Mode == v1alpha1.AuthStable
}

// fsc4bDiscoveryPath is where the server serves the discovery configuration
// of FSC voor Burgers (packages/wetsproces/src/fsc4b).
const fsc4bDiscoveryPath = "/fsc4b/discovery"

// keycloakSecret is the Secret the chart generates for the Keycloak of w's
// release (charts/wetsproces, templates/keycloak.yaml).
func keycloakSecret(w *v1alpha1.Wetsproces) string { return w.Name + "-keycloak" }

// validateSpec checks what the CRD's schema cannot: the rules of a git ref,
// a path that stays in the repository, and a hostname that is a DNS name.
// The schema checks the rest when the object is written.
func validateSpec(w *v1alpha1.Wetsproces, domain string) error {
	m := w.Spec.Manifest
	if err := validateHTTPS("manifest.repoUrl", m.RepoURL); err != nil {
		return err
	}
	if err := validateRef("manifest.ref", refOrDefault(m.Ref)); err != nil {
		return err
	}
	path := pathOrDefault(m.Path)
	if strings.HasPrefix(path, "/") || strings.Contains(path, "..") {
		return fmt.Errorf("manifest.path must be a relative path inside the repository, got %q", path)
	}
	if w.Spec.Fsc4b.Enabled && isStable(w) {
		return fmt.Errorf("fsc4b.enabled runs on the demo Stelsel, whose test Activatiedienst checks no identity: test mode only")
	}
	h := hostsOf(w, domain)
	for _, name := range h.all() {
		if errs := validation.IsDNS1123Subdomain(name); len(errs) > 0 {
			return fmt.Errorf("hostname %q gives %s, which is no valid domain name: %s", w.Spec.Hostname, name, strings.Join(errs, "; "))
		}
		// The subdomain check counts the whole name; each label between the
		// dots has a limit of its own, which -medewerkers can push it past.
		for _, label := range strings.Split(name, ".") {
			if errs := validation.IsDNS1123Label(label); len(errs) > 0 {
				return fmt.Errorf("hostname %q gives %s, whose part %q is no valid label: %s", w.Spec.Hostname, name, label, strings.Join(errs, "; "))
			}
		}
	}
	return nil
}

// The CRD defaults ref and path; an object written before those defaults,
// or through a client that skips them, still gets them here.
func refOrDefault(ref string) string {
	if ref == "" {
		return "main"
	}
	return ref
}

func pathOrDefault(path string) string {
	if path == "" {
		return "bron.yaml"
	}
	return path
}

func objectLabels(w *v1alpha1.Wetsproces) map[string]any {
	return map[string]any{managedByLabel: fieldOwner, nameLabel: w.Name}
}

func newObject(gvk schema.GroupVersionKind, w *v1alpha1.Wetsproces, name string, spec map[string]any) *unstructured.Unstructured {
	obj := &unstructured.Unstructured{Object: map[string]any{
		"metadata": map[string]any{
			"name":      name,
			"namespace": w.Namespace,
			"labels":    objectLabels(w),
		},
		"spec": spec,
	}}
	obj.SetGroupVersionKind(gvk)
	return obj
}

// newGitRepository builds the source of the chart: the fork at ChartRef, or,
// for a Wetsproces with a version, at that very commit, so the image and the
// chart that runs it come from one commit. The ignore rules keep only the
// chart, so the artifact stays small in a large repository.
func newGitRepository(w *v1alpha1.Wetsproces, cfg pluginConfig) *unstructured.Unstructured {
	ref := map[string]any{"branch": cfg.ChartRef}
	if w.Spec.Version != "" {
		ref = map[string]any{"commit": w.Spec.Version}
	}
	return newObject(gitRepositoryGVK, w, sourceName(w), map[string]any{
		"url":      forkURL,
		"ref":      ref,
		"interval": fluxInterval,
		"ignore":   "/*\n!/" + chartPath + "/\n",
	})
}

// chartValues maps the spec of w onto the values of charts/wetsproces. The
// chart's own defaults stand for the rest: a CloudNativePG database of the
// release, an instance per side, and the cluster's shared Gateway.
//
// Without a version the values leave the image out, so the chart's own
// image.ref counts: the build of main that the chart on main names, which CI
// moves on after every build. With reconcileStrategy Revision, Flux then rolls
// out each new build of main.
func chartValues(w *v1alpha1.Wetsproces, cfg pluginConfig) map[string]any {
	m := w.Spec.Manifest
	caseValues := map[string]any{
		"repoUrl": m.RepoURL,
		"ref":     refOrDefault(m.Ref),
		"path":    pathOrDefault(m.Path),
	}
	if m.CredentialsSecretRef != nil {
		caseValues["credentialsSecret"] = m.CredentialsSecretRef.Name
	}
	h := hostsOf(w, cfg.Domain)
	values := map[string]any{
		"domain": cfg.Domain,
		"host":   h.burger,
		"surfaces": map[string]any{
			"burger":     map[string]any{"host": h.burger},
			"backoffice": map[string]any{"host": h.backoffice},
		},
		"case":  caseValues,
		"theme": map[string]any{"accentColor": w.Spec.Theme.AccentColor},
		"tls":   map[string]any{"acme": cfg.ACME},
	}
	if w.Spec.Version != "" {
		values["image"] = map[string]any{"ref": cfg.DefaultImage + ":" + w.Spec.Version}
	}
	if w.Spec.Fsc4b.Enabled {
		// The chart sets the demo Stelsel on the citizens' instance; its
		// keys are made in memory at start, so there is no Secret to name.
		values["fsc4b"] = map[string]any{"enabled": true}
	}
	if isStable(w) {
		// The chart's stable mode: PostgreSQL and an instance per side are
		// its defaults already; login through its own Keycloak, whose host
		// is set here so the status can name it.
		values["wetsproces"] = map[string]any{"mode": "stable", "login": "oidc"}
		values["keycloak"] = map[string]any{
			"enabled":   true,
			"host":      h.login,
			"testUsers": w.Spec.Auth.TestAccounts,
		}
	}
	return values
}

// newHelmRelease builds the HelmRelease that installs the chart for w.
// reconcileStrategy Revision rebuilds the chart on every new commit of the
// source, because the chart version does not change per commit.
func newHelmRelease(w *v1alpha1.Wetsproces, cfg pluginConfig) *unstructured.Unstructured {
	return newObject(helmReleaseGVK, w, w.Name, map[string]any{
		"interval":    fluxInterval,
		"timeout":     releaseTimeout,
		"releaseName": w.Name,
		"chart": map[string]any{
			"spec": map[string]any{
				"chart":             "./" + chartPath,
				"reconcileStrategy": "Revision",
				"sourceRef": map[string]any{
					"kind": "GitRepository",
					"name": sourceName(w),
				},
			},
		},
		"install": map[string]any{"remediation": map[string]any{"retries": int64(3)}},
		"upgrade": map[string]any{"remediation": map[string]any{"retries": int64(3)}},
		"values":  chartValues(w, cfg),
	})
}

// condition returns whether status.conditions has the type with status True,
// and that condition's message.
func condition(obj *unstructured.Unstructured, conditionType string) (bool, string) {
	conditions, _, _ := unstructured.NestedSlice(obj.Object, "status", "conditions")
	for _, c := range conditions {
		cond, ok := c.(map[string]any)
		if !ok || cond["type"] != conditionType {
			continue
		}
		msg, _ := cond["message"].(string)
		return cond["status"] == "True", msg
	}
	return false, "not reconciled yet"
}
