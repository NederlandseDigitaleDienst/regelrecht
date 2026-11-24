package main

import (
	"context"
	"fmt"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	corev1 "k8s.io/api/core/v1"
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

const testNamespace = "tnt-voorbeeld--regelhet"

func validConfig() pluginConfig {
	return pluginConfig{
		Domain:       "cluster.example.org",
		DefaultImage: "registry.example.org/regelrecht/regelhet",
		ChartRef:     "main",
		ACME:         acmeStaging,
	}
}

func newRegelHet(name, hostname string) *v1alpha1.RegelHet {
	return &v1alpha1.RegelHet{
		TypeMeta: metav1.TypeMeta{APIVersion: v1alpha1.GroupVersion.String(), Kind: "RegelHet"},
		ObjectMeta: metav1.ObjectMeta{
			Name:      name,
			Namespace: testNamespace,
			UID:       types.UID("uid-" + name),
		},
		Spec: v1alpha1.RegelHetSpec{
			Manifest: v1alpha1.Manifest{RepoURL: "https://git.example.org/casus/a.git"},
			Hostname: hostname,
		},
	}
}

func TestPluginImplementsInterfaces(t *testing.T) {
	t.Parallel()
	var p any = NewRegelHetPlugin()
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
		"an image with a tag":        func(c *pluginConfig) { c.DefaultImage = "registry.example.org/regelhet:main" },
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
	assert.Equal(t, "code.digitaledienst.overheid.nl/regelrecht/regelhet", cfg.DefaultImage)
	assert.Equal(t, acmeProduction, cfg.ACME)
}

func TestHostsStayOneLabelUnderTheDomain(t *testing.T) {
	t.Parallel()
	h := hostsOf(newRegelHet("zorgtoeslag", "zorgtoeslag"), "cluster.example.org")
	assert.Equal(t, hosts{burger: "zorgtoeslag.cluster.example.org", backoffice: "zorgtoeslag-medewerkers.cluster.example.org"}, h)

	own := hostsOf(newRegelHet("zorg", "zorg.example.nl"), "cluster.example.org")
	assert.Equal(t, hosts{burger: "zorg.example.nl", backoffice: "medewerkers.zorg.example.nl"}, own)

	assert.True(t, h.overlaps(hostsOf(newRegelHet("x", "zorgtoeslag-medewerkers"), "cluster.example.org").swapped()))
	assert.False(t, h.overlaps(own))
}

// swapped is a hosts whose backoffice is another's citizens' host, for the
// overlap test.
func (h hosts) swapped() hosts { return hosts{burger: h.backoffice, backoffice: h.burger} }

func TestValidateSpec(t *testing.T) {
	t.Parallel()
	require.NoError(t, validateSpec(newRegelHet("a", "a"), "cluster.example.org"))
	for name, mutate := range map[string]func(*v1alpha1.RegelHet){
		"credentials in the URL": func(w *v1alpha1.RegelHet) { w.Spec.Manifest.RepoURL = "https://u:p@git.example.org/a.git" },
		"plain http":             func(w *v1alpha1.RegelHet) { w.Spec.Manifest.RepoURL = "http://git.example.org/a.git" },
		"a full ref":             func(w *v1alpha1.RegelHet) { w.Spec.Manifest.Ref = "refs/heads/main" },
		"a path out of the repo": func(w *v1alpha1.RegelHet) { w.Spec.Manifest.Path = "../etc/passwd" },
		"a label too long": func(w *v1alpha1.RegelHet) {
			w.Spec.Hostname = "a234567890123456789012345678901234567890123456789012345"
		},
	} {
		w := newRegelHet("a", "a")
		mutate(w)
		assert.Error(t, validateSpec(w, "cluster.example.org"), name)
	}
}

func TestChartValuesFollowTheSpec(t *testing.T) {
	t.Parallel()
	w := newRegelHet("zorgtoeslag", "zorgtoeslag")
	values := chartValues(w, validConfig(), testSnapshot)
	_, pinned := values["image"]
	assert.False(t, pinned, "without a version the chart's own image.ref, the latest build of main, must count")
	assert.Equal(t, map[string]any{
		"repoUrl":  "https://git.example.org/casus/a.git",
		"ref":      "main",
		"path":     "bron.yaml",
		"snapshot": map[string]any{"configMaps": []any{"zorgtoeslag-case-0123456789-0"}},
	}, values["case"], "the pods get the snapshot")
	assert.Equal(t, "zorgtoeslag.cluster.example.org", values["host"])
	assert.Equal(t, map[string]any{
		"burger":     map[string]any{"host": "zorgtoeslag.cluster.example.org"},
		"backoffice": map[string]any{"host": "zorgtoeslag-medewerkers.cluster.example.org"},
	}, values["surfaces"])

	w.Spec.Version = "0123456789abcdef0123456789abcdef01234567"
	w.Spec.Theme.AccentColor = "paars"
	w.Spec.Manifest.CredentialsSecretRef = &v1alpha1.SecretReference{Name: "zorgtoeslag-repo"}
	values = chartValues(w, validConfig(), testSnapshot)
	assert.Equal(t, map[string]any{"ref": "registry.example.org/regelrecht/regelhet:0123456789abcdef0123456789abcdef01234567"}, values["image"])
	assert.Equal(t, map[string]any{"accentColor": "paars"}, values["theme"])
	// Named for the chart of an older version only: with a snapshot the
	// chart mounts no Secret (CI checks the render).
	assert.Equal(t, "zorgtoeslag-repo", values["case"].(map[string]any)["credentialsSecret"])
}

// The chart is the other half of chartValues: every value the plugin sets must
// be a key of charts/regelhet/values.yaml, or Helm drops it silently.
func TestChartValuesExistInChart(t *testing.T) {
	t.Parallel()
	raw, err := os.ReadFile("../../charts/regelhet/values.yaml")
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
	w := newRegelHet("a", "a")
	w.Spec.Version = "0123456789abcdef0123456789abcdef01234567"
	w.Spec.Manifest.CredentialsSecretRef = &v1alpha1.SecretReference{Name: "a-repo"}
	check("", chartValues(w, validConfig(), testSnapshot), chart)
	w.Spec.Auth = v1alpha1.Auth{Mode: v1alpha1.AuthStable, TestAccounts: true}
	check("", chartValues(w, validConfig(), testSnapshot), chart)
	w.Spec.Auth = v1alpha1.Auth{}
	w.Spec.Fsc4b.Enabled = true
	check("", chartValues(w, validConfig(), testSnapshot), chart)
	// With a party on a release of its own: both releases' values.
	w.Spec.Parties.OwnInstances = true
	party := v1alpha1.PartyStatus{Cell: "ggd_voorbeeld", URL: "https://a-ggd-voorbeeld.cluster.example.org/"}
	w.Status.Parties, w.Status.ExecutorCell = []v1alpha1.PartyStatus{party}, "gemeente_voorbeeld"
	check("", chartValues(w, validConfig(), testSnapshot), chart)
	check("", partyValues(w, validConfig(), party), chart)
}

// testSnapshot is a snapshot for chartValues.
var testSnapshot = &snapshot{commit: strings.Repeat("c", 40), configMaps: []string{"zorgtoeslag-case-0123456789-0"}}

// newReconciler is a reconciler on a fake cluster with objs, where each
// RegelHet among them has its snapshot already, so a reconcile goes on to
// the release.
func newReconciler(t *testing.T, objs ...client.Object) *RegelHetReconciler {
	t.Helper()
	for _, obj := range objs {
		if w, ok := obj.(*v1alpha1.RegelHet); ok {
			objs = append(objs, snapshotConfigMap(w, snapshotSource(w), "0123456789", 0, 1))
		}
	}
	return newBareReconciler(t, objs...)
}

// snapshotConfigMap is part i of n of a snapshot of w as storeSnapshot makes it.
func snapshotConfigMap(w *v1alpha1.RegelHet, source, hash string, i, n int) *corev1.ConfigMap {
	return &corev1.ConfigMap{
		ObjectMeta: metav1.ObjectMeta{
			Name:      fmt.Sprintf("%s-case-%s-%d", w.Name, hash, i),
			Namespace: w.Namespace,
			Labels: map[string]string{
				managedByLabel: fieldOwner, nameLabel: w.Name, componentLabel: snapshotComponent, sourceLabel: source,
			},
			Annotations: map[string]string{commitAnnotation: strings.Repeat("c", 40), partsAnnotation: fmt.Sprint(n)},
		},
		BinaryData: map[string][]byte{snapshotKey: []byte("x")},
	}
}

// newBareReconciler is a reconciler on a fake cluster with objs only.
func newBareReconciler(t *testing.T, objs ...client.Object) *RegelHetReconciler {
	t.Helper()
	scheme, err := buildScheme()
	require.NoError(t, err)
	c := fake.NewClientBuilder().
		WithScheme(scheme).
		WithObjects(objs...).
		WithStatusSubresource(&v1alpha1.RegelHet{}, &v1alpha1.Organisatie{}).
		Build()
	return &RegelHetReconciler{Client: c, Uncached: c, Namespaces: c, Scheme: scheme, Config: validConfig(), Now: func() time.Time { return testNow }}
}

// testNow is the clock of the tests; a pod from releasePod is ten seconds old.
var testNow = time.Date(2026, 11, 24, 9, 0, 0, 0, time.UTC)

func reconcileOnce(t *testing.T, r *RegelHetReconciler, w *v1alpha1.RegelHet) *v1alpha1.RegelHet {
	t.Helper()
	_, err := r.Reconcile(context.Background(), ctrl.Request{NamespacedName: client.ObjectKeyFromObject(w)})
	require.NoError(t, err)
	var got v1alpha1.RegelHet
	require.NoError(t, r.Client.Get(context.Background(), client.ObjectKeyFromObject(w), &got))
	return &got
}

func TestReconcileMakesFluxObjectsOwnedByTheRegelHet(t *testing.T) {
	t.Parallel()
	w := newRegelHet("zorgtoeslag", "zorgtoeslag")
	r := newReconciler(t, w)
	got := reconcileOnce(t, r, w)

	for _, want := range []struct {
		gvkKind string
		name    string
	}{{"GitRepository", "zorgtoeslag-chart"}, {"HelmRelease", "zorgtoeslag"}} {
		obj := fluxObject(t, r.Client, want.gvkKind, want.name)
		refs := obj.GetOwnerReferences()
		require.Len(t, refs, 1, want.name)
		assert.Equal(t, "RegelHet", refs[0].Kind)
		assert.Equal(t, w.UID, refs[0].UID)
		require.NotNil(t, refs[0].Controller)
		assert.True(t, *refs[0].Controller, "the garbage collector deletes it with the RegelHet")
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
	w := newRegelHet("a", "a")
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
	w := newRegelHet("a", "a")
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

// Two RegelHet instances side by side: each gets its own release and source; one
// that wants an address another one has is refused, and the older keeps it.
func TestTwoRegelHetSideBySide(t *testing.T) {
	t.Parallel()
	older := newRegelHet("zorgtoeslag", "zorgtoeslag")
	older.CreationTimestamp = metav1.NewTime(time.Unix(1000, 0))
	other := newRegelHet("parkeerkaart", "parkeerkaart")
	other.CreationTimestamp = metav1.NewTime(time.Unix(2000, 0))
	clash := newRegelHet("tweede", "zorgtoeslag")
	clash.Namespace = "tnt-voorbeeld--ander"
	clash.CreationTimestamp = metav1.NewTime(time.Unix(3000, 0))
	r := newReconciler(t, older, other, clash)

	for _, w := range []*v1alpha1.RegelHet{older, other} {
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
	require.NoError(t, r.Client.Get(ctx, client.ObjectKey{Name: "fundament-regelhet-admin"}, &admin))
	assert.Equal(t, "true", admin.Labels["rbac.authorization.k8s.io/aggregate-to-admin"])
	assert.Contains(t, admin.Rules[0].Verbs, "create")
	var view rbacv1.ClusterRole
	require.NoError(t, r.Client.Get(ctx, client.ObjectKey{Name: "fundament-regelhet-view"}, &view))
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
	names := make([]string, 0, len(pluginCRDs))
	for _, c := range pluginCRDs {
		require.NoError(t, yaml.Unmarshal(c.yaml, &crd))
		assert.Equal(t, c.name, crd.Metadata.Name)
		names = append(names, c.name)
	}
	assert.Equal(t, names, loadSourceDefinition(t).Spec.CRDs)
}

func TestSummary(t *testing.T) {
	t.Parallel()
	ready := v1alpha1.RegelHet{Status: v1alpha1.RegelHetStatus{Phase: v1alpha1.PhaseReady}}
	pending := v1alpha1.RegelHet{}
	s := summary([]v1alpha1.RegelHet{ready, pending})
	assert.Equal(t, pluginruntime.PhaseRunning, s.Phase)
	assert.Equal(t, "1 of 2 RegelHet instances ready", s.Message)
}
