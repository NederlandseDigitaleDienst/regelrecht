package main

import (
	"fmt"
	"strings"

	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/runtime/schema"
	"k8s.io/apimachinery/pkg/util/validation"
	"sigs.k8s.io/controller-runtime/pkg/client"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

const (
	pluginName = "regelhet"
	fieldOwner = "regelhet-plugin"

	chartPath    = "charts/regelhet"
	fluxInterval = "10m"
	// releaseTimeout is how long helm-controller waits for the pods of an
	// install or upgrade, 5m by default. A RegelHet starts in under a
	// minute; a shorter wait lets a corrected spec apply sooner after a
	// failed one. The plugin reports a failing pod before this ends.
	releaseTimeout = "3m"

	// nameLabel carries the name of the RegelHet on everything the plugin
	// makes for it.
	nameLabel      = "regelhet.regelrecht.nl/name"
	managedByLabel = "app.kubernetes.io/managed-by"

	// The keys of a Secret in credentialsSecretRef.
	secretUsernameKey = "username"
	secretTokenKey    = "token"
)

var (
	gitRepositoryGVK = schema.GroupVersionKind{Group: "source.toolkit.fluxcd.io", Version: "v1", Kind: "GitRepository"}
	helmReleaseGVK   = schema.GroupVersionKind{Group: "helm.toolkit.fluxcd.io", Version: "v2", Kind: "HelmRelease"}
)

// sourceName is the GitRepository with the chart of a RegelHet or an
// Organisatie. The HelmRelease, the Helm release and the chart's own objects
// (Deployments, Services, Gateway, database) are named after the object
// itself, so two objects in one namespace never share one; a RegelHet and an
// Organisatie of the same name in one namespace are refused (NameInUse).
func sourceName(owner client.Object) string { return owner.GetName() + "-chart" }

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

func hostsOf(w *v1alpha1.RegelHet, domain string) hosts {
	return hostsFor(w.Spec.Hostname, isStable(w), domain)
}

// hostsFor is hostsOf for a hostname, which a RegelHet and an Organisatie
// turn into addresses the same way.
func hostsFor(name string, stable bool, domain string) hosts {
	var h hosts
	if strings.Contains(name, ".") {
		first, rest, _ := strings.Cut(name, ".")
		h = hosts{burger: name, backoffice: "medewerkers." + name, login: first + "-login." + rest}
	} else {
		h = hosts{burger: name + "." + domain, backoffice: name + "-medewerkers." + domain, login: name + "-login." + domain}
	}
	if !stable {
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
func isStable(w *v1alpha1.RegelHet) bool {
	return w.Spec.Auth.Mode == v1alpha1.AuthStable
}

// fsc4bDiscoveryPath is where the server serves the discovery configuration
// of FSC voor Burgers (packages/regelhet/src/fsc4b).
const fsc4bDiscoveryPath = "/fsc4b/discovery"

// keycloakSecret is the Secret the chart generates for the Keycloak of w's
// release (charts/regelhet, templates/keycloak.yaml).
func keycloakSecret(w *v1alpha1.RegelHet) string { return w.Name + "-keycloak" }

// validateSpec checks what the CRD's schema cannot: the rules of a git ref,
// a path that stays in the repository, and a hostname that is a DNS name.
// The schema checks the rest when the object is written.
func validateSpec(w *v1alpha1.RegelHet, domain string) error {
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
	if err := validatePartij(w); err != nil {
		return err
	}
	if w.Spec.Fsc4b.Enabled && isStable(w) {
		return fmt.Errorf("fsc4b.enabled runs on the demo Stelsel, whose test Activatiedienst checks no identity: test mode only")
	}
	return validateHosts(w.Spec.Hostname, hostsOf(w, domain))
}

// validatePartij checks what spec.partij asks of the rest of the spec. The
// CRD's schema checks the same for an object written through the API, apart
// from a party that names itself, which a CEL rule cannot see.
func validatePartij(w *v1alpha1.RegelHet) error {
	if !isParty(w) {
		return nil
	}
	switch {
	case isStable(w):
		return fmt.Errorf("partij links two servers with keys made at start: test mode only")
	case w.Spec.Fsc4b.Enabled:
		return fmt.Errorf("partij runs a server for the staff of a party, without a citizens' side for fsc4b")
	case w.Spec.Organisatie != nil:
		return fmt.Errorf("partij runs a server for the staff of a party, without a citizens' side to put behind the front of an organisatie")
	case executorKey(w) == client.ObjectKeyFromObject(w):
		return fmt.Errorf("partij.uitvoerder names this RegelHet itself; name the RegelHet of the executor")
	}
	return nil
}

// validateHosts checks that every address h gives the hostname is a domain
// name, label by label.
func validateHosts(hostname string, h hosts) error {
	for _, name := range h.all() {
		if errs := validation.IsDNS1123Subdomain(name); len(errs) > 0 {
			return fmt.Errorf("hostname %q gives %s, which is no valid domain name: %s", hostname, name, strings.Join(errs, "; "))
		}
		// The subdomain check counts the whole name; each label between the
		// dots has a limit of its own, which -medewerkers can push it past.
		for _, label := range strings.Split(name, ".") {
			if errs := validation.IsDNS1123Label(label); len(errs) > 0 {
				return fmt.Errorf("hostname %q gives %s, whose part %q is no valid label: %s", hostname, name, label, strings.Join(errs, "; "))
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

func objectLabels(owner client.Object) map[string]any {
	return map[string]any{managedByLabel: fieldOwner, nameLabel: owner.GetName()}
}

func newObject(gvk schema.GroupVersionKind, owner client.Object, name string, spec map[string]any) *unstructured.Unstructured {
	obj := &unstructured.Unstructured{Object: map[string]any{
		"metadata": map[string]any{
			"name":      name,
			"namespace": owner.GetNamespace(),
			"labels":    objectLabels(owner),
		},
		"spec": spec,
	}}
	obj.SetGroupVersionKind(gvk)
	return obj
}

// newGitRepository builds the source of the chart for owner, a RegelHet or
// an Organisatie: the fork at ChartRef, or, for one with a version, at that
// very commit, so the image and the chart that runs it come from one commit.
// The ignore rules keep only the chart, so the artifact stays small in a large
// repository.
func newGitRepository(owner client.Object, version string, cfg pluginConfig) *unstructured.Unstructured {
	ref := map[string]any{"branch": cfg.ChartRef}
	if version != "" {
		ref = map[string]any{"commit": version}
	}
	return newObject(gitRepositoryGVK, owner, sourceName(owner), map[string]any{
		"url":      forkURL,
		"ref":      ref,
		"interval": fluxInterval,
		"ignore":   "/*\n!/" + chartPath + "/\n",
	})
}

// toAny is names as the []any an unstructured object holds.
func toAny(names []string) []any {
	out := make([]any, len(names))
	for i, n := range names {
		out[i] = n
	}
	return out
}

// chartValues maps the spec of w and its snapshot onto the values of
// charts/regelhet. The
// chart's own defaults stand for the rest: a CloudNativePG database of the
// release, an instance per side, and the cluster's shared Gateway.
//
// Without a version the values leave the image out, so the chart's own
// image.ref counts: the build of main that the chart on main names, which CI
// moves on after every build. With reconcileStrategy Revision, Flux then rolls
// out each new build of main.
func chartValues(w *v1alpha1.RegelHet, cfg pluginConfig, snap *snapshot) map[string]any {
	// The case comes from the snapshot: the chart unpacks it and its pods
	// fetch nothing. The repository, the ref and the token stay in the
	// values for the chart of a version from before chart 0.8.0, which knows
	// no snapshot and clones at every start.
	m := w.Spec.Manifest
	caseValues := map[string]any{
		"repoUrl": m.RepoURL,
		"ref":     refOrDefault(m.Ref),
		"path":    pathOrDefault(m.Path),
	}
	if m.CredentialsSecretRef != nil {
		caseValues["credentialsSecret"] = m.CredentialsSecretRef.Name
	}
	if w.Spec.Casus != "" {
		caseValues["id"] = w.Spec.Casus
	}
	if snap != nil {
		caseValues["snapshot"] = map[string]any{"configMaps": toAny(snap.configMaps)}
	}
	h := hostsOf(w, cfg.Domain)
	values := baseValues(h, w.Spec.Theme.AccentColor, w.Spec.Version, cfg)
	values["case"] = caseValues
	if w.Spec.Title != "" {
		values["theme"].(map[string]any)["title"] = w.Spec.Title
	}
	if w.Spec.Fsc4b.Enabled {
		// The chart sets the demo Stelsel on the citizens' instance; its
		// keys are made in memory at start, so there is no Secret to name.
		values["fsc4b"] = map[string]any{"enabled": true}
	}
	if peers := executorPeers(w); len(peers) > 0 {
		// The RegelHet instances that run as its parties (parties.go): the
		// executor's release sends them what they are to receive, and no
		// admin enters their acts here.
		values["koppeling"] = map[string]any{"peers": peers}
	}
	if isStable(w) {
		// The chart's stable mode: PostgreSQL and an instance per side are
		// its defaults already; login through its own Keycloak, whose host
		// is set here so the status can name it.
		values["regelhet"] = map[string]any{"mode": "stable", "login": "oidc"}
		values["keycloak"] = map[string]any{
			"enabled":   true,
			"host":      h.login,
			"testUsers": w.Spec.Auth.TestAccounts,
		}
	}
	return values
}

// baseValues are the values a RegelHet and an Organisatie set alike: the
// addresses, the colour, the certificates and, with a version, the image.
func baseValues(h hosts, accentColor, version string, cfg pluginConfig) map[string]any {
	values := map[string]any{
		"domain": cfg.Domain,
		"host":   h.burger,
		"surfaces": map[string]any{
			"burger":     map[string]any{"host": h.burger},
			"backoffice": map[string]any{"host": h.backoffice},
		},
		"theme": map[string]any{"accentColor": accentColor},
		"tls":   map[string]any{"acme": cfg.ACME},
	}
	if version != "" {
		values["image"] = map[string]any{"ref": cfg.DefaultImage + ":" + version}
	}
	return values
}

// newHelmRelease builds the HelmRelease that installs the chart for owner, a
// RegelHet or an Organisatie, with values. reconcileStrategy Revision
// rebuilds the chart on every new commit of the source, because the chart
// version does not change per commit.
func newHelmRelease(owner client.Object, values map[string]any) *unstructured.Unstructured {
	return newObject(helmReleaseGVK, owner, owner.GetName(), map[string]any{
		"interval":    fluxInterval,
		"timeout":     releaseTimeout,
		"releaseName": owner.GetName(),
		"chart": map[string]any{
			"spec": map[string]any{
				"chart":             "./" + chartPath,
				"reconcileStrategy": "Revision",
				"sourceRef": map[string]any{
					"kind": "GitRepository",
					"name": sourceName(owner),
				},
			},
		},
		"install": map[string]any{"remediation": map[string]any{"retries": int64(3)}},
		"upgrade": map[string]any{"remediation": map[string]any{"retries": int64(3)}},
		"values":  values,
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
