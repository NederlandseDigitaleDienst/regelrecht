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
	pluginName = "regelhet"
	fieldOwner = "regelhet-plugin"

	chartPath    = "charts/regelhet"
	fluxInterval = "10m"

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

// sourceName is the GitRepository with the chart of the RegelHet w. The
// HelmRelease, the Helm release and the chart's own objects (Deployments,
// Services, Gateway, database) are named after w itself, so two RegelHet
// objects in one namespace never share an object.
func sourceName(w *v1alpha1.RegelHet) string { return w.Name + "-chart" }

// hosts are the addresses of w: citizens on its hostname under the base
// domain, employees on <hostname>-medewerkers beside it. Both stay one label
// under the domain, where the cluster's wildcard record covers them;
// medewerkers.<hostname>.<domain> would not be. A hostname with a dot is a
// domain of its own, and employees get medewerkers.<hostname> under it.
type hosts struct {
	burger     string
	backoffice string
}

func hostsOf(w *v1alpha1.RegelHet, domain string) hosts {
	h := w.Spec.Hostname
	if strings.Contains(h, ".") {
		return hosts{burger: h, backoffice: "medewerkers." + h}
	}
	return hosts{burger: h + "." + domain, backoffice: h + "-medewerkers." + domain}
}

// overlaps reports whether h and o share an address.
func (h hosts) overlaps(o hosts) bool {
	return h.burger == o.burger || h.burger == o.backoffice ||
		h.backoffice == o.burger || h.backoffice == o.backoffice
}

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
	h := hostsOf(w, domain)
	for _, name := range []string{h.burger, h.backoffice} {
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
		return "packages/regelhet/bron.yaml"
	}
	return path
}

func objectLabels(w *v1alpha1.RegelHet) map[string]any {
	return map[string]any{managedByLabel: fieldOwner, nameLabel: w.Name}
}

func newObject(gvk schema.GroupVersionKind, w *v1alpha1.RegelHet, name string, spec map[string]any) *unstructured.Unstructured {
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

// newGitRepository builds the source of the chart: the fork at ChartRef. The
// ignore rules keep only the chart, so the artifact stays small in a large
// repository.
func newGitRepository(w *v1alpha1.RegelHet, cfg pluginConfig) *unstructured.Unstructured {
	return newObject(gitRepositoryGVK, w, sourceName(w), map[string]any{
		"url":      forkURL,
		"ref":      map[string]any{"branch": cfg.ChartRef},
		"interval": fluxInterval,
		"ignore":   "/*\n!/" + chartPath + "/\n",
	})
}

// chartValues maps the spec of w onto the values of charts/regelhet. The
// chart's own defaults stand for the rest: a CloudNativePG database of the
// release, an instance per side, and the cluster's shared Gateway.
//
// Without a version the values leave the image out, so the chart's own
// image.ref counts: the build of main that the chart on main names, which CI
// moves on after every build. With reconcileStrategy Revision, Flux then rolls
// out each new build of main.
func chartValues(w *v1alpha1.RegelHet, cfg pluginConfig) map[string]any {
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
		"case": caseValues,
		"theme":  map[string]any{"accentColor": w.Spec.Theme.AccentColor},
		"tls":    map[string]any{"acme": cfg.ACME},
	}
	if w.Spec.Version != "" {
		values["image"] = map[string]any{"ref": cfg.DefaultImage + ":" + w.Spec.Version}
	}
	return values
}

// newHelmRelease builds the HelmRelease that installs the chart for w.
// reconcileStrategy Revision rebuilds the chart on every new commit of the
// source, because the chart version does not change per commit.
func newHelmRelease(w *v1alpha1.RegelHet, cfg pluginConfig) *unstructured.Unstructured {
	return newObject(helmReleaseGVK, w, w.Name, map[string]any{
		"interval":    fluxInterval,
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
