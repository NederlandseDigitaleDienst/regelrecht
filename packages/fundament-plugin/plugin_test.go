package main

import (
	"context"
	"os"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	rbacv1 "k8s.io/api/rbac/v1"
	apimeta "k8s.io/apimachinery/pkg/api/meta"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/types"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/client/fake"
	"sigs.k8s.io/yaml"

	"github.com/fundament-oss/fundament/plugin-sdk/pluginruntime"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

const testNamespace = "tnt-voorbeeld--wetsproces"

func validConfig() pluginConfig {
	return pluginConfig{
		Domain:       "cluster.example.org",
		DefaultImage: "registry.example.org/regelrecht/wetsproces",
		ChartRef:     "main",
		ACME:         acmeStaging,
	}
}

func newWetsproces(name, hostname string) *v1alpha1.Wetsproces {
	return &v1alpha1.Wetsproces{
		TypeMeta: metav1.TypeMeta{APIVersion: v1alpha1.GroupVersion.String(), Kind: "Wetsproces"},
		ObjectMeta: metav1.ObjectMeta{
			Name:      name,
			Namespace: testNamespace,
			UID:       types.UID("uid-" + name),
		},
		Spec: v1alpha1.WetsprocesSpec{
			Manifest: v1alpha1.Manifest{RepoURL: "https://git.example.org/casus/a.git"},
			Hostname: hostname,
		},
	}
}

func TestPluginImplementsInterfaces(t *testing.T) {
	t.Parallel()
	var p any = NewWetsprocesPlugin()
	_, ok := p.(pluginruntime.Plugin)
	assert.True(t, ok)
	_, ok = p.(pluginruntime.Installer)
	assert.True(t, ok)
	_, ok = p.(pluginruntime.Reconciler)
	assert.True(t, ok)
	_, ok = p.(pluginruntime.ConsoleProvider)
	assert.True(t, ok, "without ConsoleProvider the console's iframes stay empty")
}

func TestConfigValidate(t *testing.T) {
	t.Parallel()
	require.NoError(t, validConfig().validate())
	for name, mutate := range map[string]func(*pluginConfig){
		"a domain that is no domain": func(c *pluginConfig) { c.Domain = "not a domain" },
		"an image with a tag":        func(c *pluginConfig) { c.DefaultImage = "registry.example.org/wetsproces:main" },
		"a chart ref with dots":      func(c *pluginConfig) { c.ChartRef = "a..b" },
		"an unknown ACME":            func(c *pluginConfig) { c.ACME = "test" },
	} {
		cfg := validConfig()
		mutate(&cfg)
		assert.Error(t, cfg.validate(), name)
	}
}

//nolint:paralleltest // sets process environment
func TestLoadConfigRequiresTheDomainOnly(t *testing.T) {
	t.Setenv("FUNP_DOMAIN", "")
	require.NoError(t, os.Unsetenv("FUNP_DOMAIN"))
	_, err := loadConfig()
	require.ErrorContains(t, err, "DOMAIN")

	t.Setenv("FUNP_DOMAIN", "cluster-a.example.org")
	cfg, err := loadConfig()
	require.NoError(t, err)
	assert.Equal(t, "code.digitaledienst.overheid.nl/regelrecht/wetsproces", cfg.DefaultImage)
	assert.Equal(t, acmeProduction, cfg.ACME)
}

func TestHostsStayOneLabelUnderTheDomain(t *testing.T) {
	t.Parallel()
	h := hostsOf(newWetsproces("zorgtoeslag", "zorgtoeslag"), "cluster.example.org")
	assert.Equal(t, hosts{burger: "zorgtoeslag.cluster.example.org", backoffice: "zorgtoeslag-medewerkers.cluster.example.org"}, h)

	own := hostsOf(newWetsproces("zorg", "zorg.example.nl"), "cluster.example.org")
	assert.Equal(t, hosts{burger: "zorg.example.nl", backoffice: "medewerkers.zorg.example.nl"}, own)

	assert.True(t, h.overlaps(hostsOf(newWetsproces("x", "zorgtoeslag-medewerkers"), "cluster.example.org").swapped()))
	assert.False(t, h.overlaps(own))
}

// swapped is a hosts whose backoffice is another's citizens' host, for the
// overlap test.
func (h hosts) swapped() hosts { return hosts{burger: h.backoffice, backoffice: h.burger} }

func TestValidateSpec(t *testing.T) {
	t.Parallel()
	require.NoError(t, validateSpec(newWetsproces("a", "a"), "cluster.example.org"))
	for name, mutate := range map[string]func(*v1alpha1.Wetsproces){
		"credentials in the URL": func(w *v1alpha1.Wetsproces) { w.Spec.Manifest.RepoURL = "https://u:p@git.example.org/a.git" },
		"plain http":             func(w *v1alpha1.Wetsproces) { w.Spec.Manifest.RepoURL = "http://git.example.org/a.git" },
		"a full ref":             func(w *v1alpha1.Wetsproces) { w.Spec.Manifest.Ref = "refs/heads/main" },
		"a path out of the repo": func(w *v1alpha1.Wetsproces) { w.Spec.Manifest.Path = "../etc/passwd" },
		"a label too long": func(w *v1alpha1.Wetsproces) {
			w.Spec.Hostname = "a234567890123456789012345678901234567890123456789012345"
		},
	} {
		w := newWetsproces("a", "a")
		mutate(w)
		assert.Error(t, validateSpec(w, "cluster.example.org"), name)
	}
}

func TestChartValuesFollowTheSpec(t *testing.T) {
	t.Parallel()
	w := newWetsproces("zorgtoeslag", "zorgtoeslag")
	values := chartValues(w, validConfig())
	_, pinned := values["image"]
	assert.False(t, pinned, "without a version the chart's own image.ref, the latest build of main, must count")
	assert.Equal(t, map[string]any{
		"repoUrl": "https://git.example.org/casus/a.git",
		"ref":     "main",
		"path":    "bron.yaml",
	}, values["case"])
	assert.Equal(t, "zorgtoeslag.cluster.example.org", values["host"])
	assert.Equal(t, map[string]any{
		"burger":     map[string]any{"host": "zorgtoeslag.cluster.example.org"},
		"backoffice": map[string]any{"host": "zorgtoeslag-medewerkers.cluster.example.org"},
	}, values["surfaces"])

	w.Spec.Version = "0123456789abcdef0123456789abcdef01234567"
	w.Spec.Theme.AccentColor = "paars"
	w.Spec.Manifest.CredentialsSecretRef = &v1alpha1.SecretReference{Name: "zorgtoeslag-repo"}
	values = chartValues(w, validConfig())
	assert.Equal(t, map[string]any{"ref": "registry.example.org/regelrecht/wetsproces:0123456789abcdef0123456789abcdef01234567"}, values["image"])
	assert.Equal(t, map[string]any{"accentColor": "paars"}, values["theme"])
	assert.Equal(t, "zorgtoeslag-repo", values["case"].(map[string]any)["credentialsSecret"])
}

// The chart is the other half of chartValues: every value the plugin sets must
// be a key of charts/wetsproces/values.yaml, or Helm drops it silently.
func TestChartValuesExistInChart(t *testing.T) {
	t.Parallel()
	raw, err := os.ReadFile("../../charts/wetsproces/values.yaml")
	require.NoError(t, err)
	var chart map[string]any
	require.NoError(t, yaml.Unmarshal(raw, &chart))

	var check func(path string, want, have map[string]any)
	check = func(path string, want, have map[string]any) {
		for key, value := range want {
			got, ok := have[key]
			require.True(t, ok, "values.yaml has no %s%s", path, key)
			if sub, isMap := value.(map[string]any); isMap {
				gotMap, ok := got.(map[string]any)
				require.True(t, ok, "%s%s is no map in values.yaml", path, key)
				check(path+key+".", sub, gotMap)
			}
		}
	}
	w := newWetsproces("a", "a")
	w.Spec.Version = "0123456789abcdef0123456789abcdef01234567"
	w.Spec.Manifest.CredentialsSecretRef = &v1alpha1.SecretReference{Name: "a-repo"}
	check("", chartValues(w, validConfig()), chart)
	w.Spec.Auth = v1alpha1.Auth{Mode: v1alpha1.AuthStable, TestAccounts: true}
	check("", chartValues(w, validConfig()), chart)
	w.Spec.Auth = v1alpha1.Auth{}
	w.Spec.Fsc4b.Enabled = true
	check("", chartValues(w, validConfig()), chart)
}

func newReconciler(t *testing.T, objs ...client.Object) *WetsprocesReconciler {
	t.Helper()
	scheme, err := buildScheme()
	require.NoError(t, err)
	c := fake.NewClientBuilder().
		WithScheme(scheme).
		WithObjects(objs...).
		WithStatusSubresource(&v1alpha1.Wetsproces{}).
		Build()
	return &WetsprocesReconciler{Client: c, Secrets: c, Scheme: scheme, Config: validConfig(), Now: func() time.Time { return testNow }}
}

// testNow is the clock of the tests; a pod from releasePod is ten seconds old.
var testNow = time.Date(2026, 11, 24, 9, 0, 0, 0, time.UTC)

func reconcileOnce(t *testing.T, r *WetsprocesReconciler, w *v1alpha1.Wetsproces) *v1alpha1.Wetsproces {
	t.Helper()
	_, err := r.Reconcile(context.Background(), ctrl.Request{NamespacedName: client.ObjectKeyFromObject(w)})
	require.NoError(t, err)
	var got v1alpha1.Wetsproces
	require.NoError(t, r.Client.Get(context.Background(), client.ObjectKeyFromObject(w), &got))
	return &got
}

func TestReconcileMakesFluxObjectsOwnedByTheWetsproces(t *testing.T) {
	t.Parallel()
	w := newWetsproces("zorgtoeslag", "zorgtoeslag")
	r := newReconciler(t, w)
	got := reconcileOnce(t, r, w)

	for _, want := range []struct {
		gvkKind string
		name    string
	}{{"GitRepository", "zorgtoeslag-chart"}, {"HelmRelease", "zorgtoeslag"}} {
		obj := fluxObject(t, r.Client, want.gvkKind, want.name)
		refs := obj.GetOwnerReferences()
		require.Len(t, refs, 1, want.name)
		assert.Equal(t, "Wetsproces", refs[0].Kind)
		assert.Equal(t, w.UID, refs[0].UID)
		require.NotNil(t, refs[0].Controller)
		assert.True(t, *refs[0].Controller, "the garbage collector deletes it with the Wetsproces")
		assert.Equal(t, "zorgtoeslag", obj.GetLabels()[nameLabel])
	}
	hr := fluxObject(t, r.Client, "HelmRelease", "zorgtoeslag")
	source, _, _ := unstructured.NestedString(hr.Object, "spec", "chart", "spec", "sourceRef", "name")
	assert.Equal(t, "zorgtoeslag-chart", source)

	assert.Equal(t, v1alpha1.PhasePending, got.Status.Phase)
	assert.Equal(t, "https://zorgtoeslag.cluster.example.org/", got.Status.URL)
	assert.Equal(t, "https://zorgtoeslag-medewerkers.cluster.example.org/", got.Status.BackofficeURL)
}

func fluxObject(t *testing.T, c client.Client, kind, name string) *unstructured.Unstructured {
	t.Helper()
	obj := &unstructured.Unstructured{}
	switch kind {
	case "GitRepository":
		obj.SetGroupVersionKind(gitRepositoryGVK)
	default:
		obj.SetGroupVersionKind(helmReleaseGVK)
	}
	require.NoError(t, c.Get(context.Background(), client.ObjectKey{Namespace: testNamespace, Name: name}, obj))
	return obj
}

func setReleaseConditions(t *testing.T, c client.Client, name string, conditions ...map[string]any) {
	t.Helper()
	hr := fluxObject(t, c, "HelmRelease", name)
	list := make([]any, 0, len(conditions))
	for _, cond := range conditions {
		list = append(list, cond)
	}
	require.NoError(t, unstructured.SetNestedSlice(hr.Object, list, "status", "conditions"))
	require.NoError(t, c.Update(context.Background(), hr))
}

func TestTheStatusFollowsTheRelease(t *testing.T) {
	t.Parallel()
	w := newWetsproces("a", "a")
	r := newReconciler(t, w)
	reconcileOnce(t, r, w)

	setReleaseConditions(t, r.Client, "a", map[string]any{"type": "Ready", "status": "True", "message": "Helm install succeeded"})
	got := reconcileOnce(t, r, w)
	assert.Equal(t, v1alpha1.PhaseReady, got.Status.Phase)
	assert.True(t, apimeta.IsStatusConditionTrue(got.Status.Conditions, v1alpha1.ConditionReady))
	assert.Contains(t, got.Status.Message, "Helm install succeeded")

	setReleaseConditions(t, r.Client, "a",
		map[string]any{"type": "Ready", "status": "False", "message": "install retries exhausted"},
		map[string]any{"type": "Stalled", "status": "True", "message": "install retries exhausted"})
	got = reconcileOnce(t, r, w)
	assert.Equal(t, v1alpha1.PhaseFailed, got.Status.Phase)
	assert.False(t, apimeta.IsStatusConditionTrue(got.Status.Conditions, v1alpha1.ConditionReady))
}

func TestAnInvalidSpecMakesNothing(t *testing.T) {
	t.Parallel()
	w := newWetsproces("a", "a")
	w.Spec.Manifest.Path = "../buiten"
	r := newReconciler(t, w)
	got := reconcileOnce(t, r, w)
	assert.Equal(t, v1alpha1.PhaseFailed, got.Status.Phase)
	assert.Equal(t, reasonInvalidSpec, apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionReady).Reason)

	var list unstructured.UnstructuredList
	list.SetGroupVersionKind(helmReleaseGVK.GroupVersion().WithKind("HelmReleaseList"))
	require.NoError(t, r.Client.List(context.Background(), &list))
	assert.Empty(t, list.Items)
}

// Two wetsprocessen side by side: each gets its own release and source; one
// that wants an address another one has is refused, and the older keeps it.
func TestTwoWetsprocessenSideBySide(t *testing.T) {
	t.Parallel()
	older := newWetsproces("zorgtoeslag", "zorgtoeslag")
	older.CreationTimestamp = metav1.NewTime(time.Unix(1000, 0))
	other := newWetsproces("parkeerkaart", "parkeerkaart")
	other.CreationTimestamp = metav1.NewTime(time.Unix(2000, 0))
	clash := newWetsproces("tweede", "zorgtoeslag")
	clash.Namespace = "tnt-voorbeeld--ander"
	clash.CreationTimestamp = metav1.NewTime(time.Unix(3000, 0))
	r := newReconciler(t, older, other, clash)

	for _, w := range []*v1alpha1.Wetsproces{older, other} {
		got := reconcileOnce(t, r, w)
		assert.Equal(t, v1alpha1.PhasePending, got.Status.Phase, w.Name)
		fluxObject(t, r.Client, "HelmRelease", w.Name)
		fluxObject(t, r.Client, "GitRepository", w.Name+"-chart")
	}
	got := reconcileOnce(t, r, clash)
	assert.Equal(t, v1alpha1.PhaseFailed, got.Status.Phase)
	ready := apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionReady)
	assert.Equal(t, reasonHostnameInUse, ready.Reason)
	assert.Contains(t, ready.Message, testNamespace+"/zorgtoeslag")
}

func TestUserRolesAggregateIntoAdminAndViewAndGoOnUninstall(t *testing.T) {
	t.Parallel()
	ctx := context.Background()
	r := newReconciler(t)
	require.NoError(t, applyUserRoles(ctx, r.Client))
	require.NoError(t, applyUserRoles(ctx, r.Client), "a second apply must update, not fail")

	var admin rbacv1.ClusterRole
	require.NoError(t, r.Client.Get(ctx, client.ObjectKey{Name: "fundament-wetsproces-admin"}, &admin))
	assert.Equal(t, "true", admin.Labels["rbac.authorization.k8s.io/aggregate-to-admin"])
	assert.Contains(t, admin.Rules[0].Verbs, "create")
	var view rbacv1.ClusterRole
	require.NoError(t, r.Client.Get(ctx, client.ObjectKey{Name: "fundament-wetsproces-view"}, &view))
	assert.NotContains(t, view.Rules[0].Verbs, "create")

	require.NoError(t, deleteUserRoles(ctx, r.Client))
	require.NoError(t, deleteUserRoles(ctx, r.Client), "deleting twice is no error")
	var roles rbacv1.ClusterRoleList
	require.NoError(t, r.Client.List(ctx, &roles))
	assert.Empty(t, roles.Items)
}

func TestTheEmbeddedCRDIsTheOneTheDefinitionNames(t *testing.T) {
	t.Parallel()
	var crd struct {
		Metadata struct {
			Name string `json:"name"`
		} `json:"metadata"`
	}
	require.NoError(t, yaml.Unmarshal(wetsprocesCRD, &crd))
	assert.Equal(t, crdName, crd.Metadata.Name)
	assert.Equal(t, []string{crdName}, loadSourceDefinition(t).Spec.CRDs)
}

func TestSummary(t *testing.T) {
	t.Parallel()
	ready := v1alpha1.Wetsproces{Status: v1alpha1.WetsprocesStatus{Phase: v1alpha1.PhaseReady}}
	pending := v1alpha1.Wetsproces{}
	s := summary([]v1alpha1.Wetsproces{ready, pending})
	assert.Equal(t, pluginruntime.PhaseRunning, s.Phase)
	assert.Equal(t, "1 of 2 wetsprocessen ready", s.Message)
}
