package main

import (
	"context"
	"errors"
	"os"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	corev1 "k8s.io/api/core/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/runtime"
	clientgoscheme "k8s.io/client-go/kubernetes/scheme"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/client/fake"
	"sigs.k8s.io/yaml"

	"github.com/fundament-oss/fundament/plugin-sdk/pluginruntime"
	pluginerrors "github.com/fundament-oss/fundament/plugin-sdk/pluginruntime/errors"
)

func TestPluginImplementsInterfaces(t *testing.T) {
	t.Parallel()
	plugin := NewWetsprocesPlugin()

	var _ pluginruntime.Plugin = plugin
	var _ pluginruntime.Installer = plugin
	var _ pluginruntime.Reconciler = plugin

	assert.NotNil(t, plugin)
}

func validConfig() pluginConfig {
	return pluginConfig{
		CaseRepoURL: forkURL,
		CaseRef:     "main",
		CasePath:    "packages/wetsproces/bron.yaml",
		Namespace:   testNamespace,
		Domain:      "cluster.example.org",
		Image:       "ghcr.io/nederlandsedigitaledienst/regelrecht-wetsproces:sha-fa89c17",
		ChartRef:    "main",
		ACME:        acmeProduction,
	}
}

const testNamespace = "tnt-voorbeeld--wetsproces"

func TestConfigValidate(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name    string
		mutate  func(*pluginConfig)
		wantErr string
	}{
		{name: "valid", mutate: func(*pluginConfig) {}},
		{name: "tag as case ref", mutate: func(c *pluginConfig) { c.CaseRef = "v1.2.0" }},
		{name: "branch with slash", mutate: func(c *pluginConfig) { c.CaseRef = "feat/casus-b" }},
		{name: "image by digest", mutate: func(c *pluginConfig) {
			c.Image = "ghcr.io/x/y@sha256:" + strings.Repeat("a", 64)
		}},
		{name: "staging", mutate: func(c *pluginConfig) { c.ACME = acmeStaging }},
		{name: "fixed gateway address", mutate: func(c *pluginConfig) { c.GatewayAddress = "194.135.48.156" }},
		{name: "ssh url", mutate: func(c *pluginConfig) { c.CaseRepoURL = "ssh://git@example.org/x/y.git" }, wantErr: "https://"},
		{name: "credentials in url", mutate: func(c *pluginConfig) { c.CaseRepoURL = "https://user:token@example.org/x/y.git" }, wantErr: "credentials"},
		{name: "ref as option", mutate: func(c *pluginConfig) { c.CaseRef = "--upload-pack=x" }, wantErr: "CASE_REF"},
		{name: "full ref", mutate: func(c *pluginConfig) { c.CaseRef = "refs/heads/main" }, wantErr: "CASE_REF"},
		{name: "ref with dots", mutate: func(c *pluginConfig) { c.CaseRef = "a..b" }, wantErr: "CASE_REF"},
		{name: "empty chart ref", mutate: func(c *pluginConfig) { c.ChartRef = "" }, wantErr: "CHART_REF"},
		{name: "absolute path", mutate: func(c *pluginConfig) { c.CasePath = "/etc/passwd" }, wantErr: "CASE_PATH"},
		{name: "path escapes repo", mutate: func(c *pluginConfig) { c.CasePath = "packages/../../x" }, wantErr: "CASE_PATH"},
		{name: "domain with scheme", mutate: func(c *pluginConfig) { c.Domain = "https://example.org" }, wantErr: "DOMAIN"},
		{name: "upper case domain", mutate: func(c *pluginConfig) { c.Domain = "Example.org" }, wantErr: "DOMAIN"},
		{name: "image without tag", mutate: func(c *pluginConfig) { c.Image = "ghcr.io/x/y" }, wantErr: "IMAGE"},
		{name: "image with space", mutate: func(c *pluginConfig) { c.Image = "ghcr.io/x/y:a b" }, wantErr: "IMAGE"},
		{name: "unknown acme", mutate: func(c *pluginConfig) { c.ACME = "prod" }, wantErr: "ACME"},
		{name: "gateway address no ip", mutate: func(c *pluginConfig) { c.GatewayAddress = "gw.example.org" }, wantErr: "GATEWAY_ADDRESS"},
		{name: "namespace without tenant prefix", mutate: func(c *pluginConfig) { c.Namespace = "wetsproces" }, wantErr: "NAMESPACE"},
		{name: "empty namespace", mutate: func(c *pluginConfig) { c.Namespace = "" }, wantErr: "NAMESPACE"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()
			cfg := validConfig()
			tt.mutate(&cfg)
			err := cfg.validate()
			if tt.wantErr == "" {
				assert.NoError(t, err)
				return
			}
			assert.ErrorContains(t, err, tt.wantErr)
		})
	}
}

func TestLoadConfigDefaults(t *testing.T) {
	// An empty value counts as unset, so a developer's .envrc cannot leak in.
	for _, key := range []string{"CASE_REPO_URL", "CASE_REF", "CASE_PATH", "IMAGE", "CHART_REF", "GATEWAY_ADDRESS", "ACME"} {
		t.Setenv("FUNP_"+key, "")
	}
	t.Setenv("FUNP_DOMAIN", "example.org")
	t.Setenv("FUNP_NAMESPACE", testNamespace)

	cfg, err := loadConfig()
	require.NoError(t, err)
	assert.Equal(t, forkURL, cfg.CaseRepoURL)
	assert.Equal(t, "main", cfg.CaseRef)
	assert.Equal(t, "packages/wetsproces/bron.yaml", cfg.CasePath)
	assert.Equal(t, "main", cfg.ChartRef)
	assert.Equal(t, acmeProduction, cfg.ACME)
	assert.Empty(t, cfg.GatewayAddress)
}

func TestLoadConfigRequiresDomainAndNamespace(t *testing.T) {
	for _, key := range []string{"FUNP_DOMAIN", "FUNP_NAMESPACE"} {
		t.Run(key, func(t *testing.T) {
			t.Setenv("FUNP_DOMAIN", "example.org")
			t.Setenv("FUNP_NAMESPACE", testNamespace)
			// t.Setenv restores the old value after the test; then unset it,
			// because an empty value does not count as missing for a required key.
			require.NoError(t, os.Unsetenv(key))

			_, err := loadConfig()
			assert.ErrorContains(t, err, key)
		})
	}
}

func newFakeClient(t *testing.T, objs ...client.Object) client.Client {
	t.Helper()
	scheme := runtime.NewScheme()
	require.NoError(t, clientgoscheme.AddToScheme(scheme))
	return fake.NewClientBuilder().WithScheme(scheme).WithObjects(objs...).Build()
}

func TestInstallNeedsAnExistingNamespace(t *testing.T) {
	t.Parallel()
	ctx := context.Background()
	c := newFakeClient(t)
	p := &WetsprocesPlugin{k8sClient: c, cfg: validConfig()}
	p.initOnce.Do(func() {})

	err := p.Install(ctx, nil)
	require.Error(t, err)
	assert.True(t, pluginerrors.IsTransient(err), "a namespace created later must let a retry succeed")
	assert.ErrorContains(t, err, "Fundament")

	var list corev1.NamespaceList
	require.NoError(t, c.List(ctx, &list))
	assert.Empty(t, list.Items, "the plugin must never create a namespace")

	ns := &corev1.Namespace{}
	ns.SetName(testNamespace)
	require.NoError(t, c.Create(ctx, ns))
	require.NoError(t, p.Install(ctx, nil))
	gone, err := objectGone(ctx, c, helmReleaseGVK, testNamespace, releaseName)
	require.NoError(t, err)
	assert.False(t, gone, "the HelmRelease goes into the configured namespace")
}

func TestNewGitRepository(t *testing.T) {
	t.Parallel()
	cfg := validConfig()
	cfg.ChartRef = "feat/fundament-plugin"
	src := newGitRepository(cfg)

	assert.Equal(t, gitRepositoryGVK, src.GroupVersionKind())
	assert.Equal(t, testNamespace, src.GetNamespace())

	url, _, _ := unstructured.NestedString(src.Object, "spec", "url")
	assert.Equal(t, forkURL, url, "the chart comes from the fork, whatever the case repository is")
	branch, _, _ := unstructured.NestedString(src.Object, "spec", "ref", "branch")
	assert.Equal(t, "feat/fundament-plugin", branch)
	ignore, _, _ := unstructured.NestedString(src.Object, "spec", "ignore")
	assert.Contains(t, ignore, "!/charts/wetsproces/", "the artifact must keep the chart")

	_, hasSecret, _ := unstructured.NestedFieldNoCopy(src.Object, "spec", "secretRef")
	assert.False(t, hasSecret, "the repository is public: no secret")
}

func TestNewHelmReleaseCarriesTheConfig(t *testing.T) {
	t.Parallel()
	cfg := validConfig()
	cfg.CaseRepoURL = "https://git.example.org/casus/b.git"
	cfg.CaseRef = "v2"
	cfg.CasePath = "bron.yaml"
	cfg.ACME = acmeStaging
	cfg.GatewayAddress = "194.135.48.156"
	hr := newHelmRelease(cfg)

	assert.Equal(t, helmReleaseGVK, hr.GroupVersionKind())
	assert.Equal(t, testNamespace, hr.GetNamespace())
	assert.Equal(t, releaseName, hr.GetName())

	chart, _, _ := unstructured.NestedString(hr.Object, "spec", "chart", "spec", "chart")
	assert.Equal(t, "./charts/wetsproces", chart)
	strategy, _, _ := unstructured.NestedString(hr.Object, "spec", "chart", "spec", "reconcileStrategy")
	assert.Equal(t, "Revision", strategy)
	ref, _, _ := unstructured.NestedStringMap(hr.Object, "spec", "chart", "spec", "sourceRef")
	assert.Equal(t, map[string]string{"kind": "GitRepository", "name": sourceName}, ref)

	values, _, _ := unstructured.NestedMap(hr.Object, "spec", "values")
	assert.Equal(t, map[string]any{
		"domain": "cluster.example.org",
		"image":  map[string]any{"ref": cfg.Image},
		"case": map[string]any{
			"repoUrl": "https://git.example.org/casus/b.git",
			"ref":     "v2",
			"path":    "bron.yaml",
		},
		"gateway": map[string]any{"address": "194.135.48.156"},
		"tls":     map[string]any{"acme": acmeStaging},
	}, values)
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
	check("", chartValues(validConfig()), chart)
}

func withCondition(obj *unstructured.Unstructured, conditions ...map[string]any) *unstructured.Unstructured {
	list := make([]any, 0, len(conditions))
	for _, c := range conditions {
		list = append(list, c)
	}
	_ = unstructured.SetNestedSlice(obj.Object, list, "status", "conditions")
	return obj
}

func ready(status, message string) map[string]any {
	return map[string]any{"type": "Ready", "status": status, "message": message}
}

func TestCondition(t *testing.T) {
	t.Parallel()

	ok, msg := condition(&unstructured.Unstructured{Object: map[string]any{}}, "Ready")
	assert.False(t, ok)
	assert.Equal(t, "not reconciled yet", msg)

	ok, msg = condition(withCondition(&unstructured.Unstructured{Object: map[string]any{}}, ready("True", "Helm install succeeded")), "Ready")
	assert.True(t, ok)
	assert.Equal(t, "Helm install succeeded", msg)

	ok, _ = condition(withCondition(&unstructured.Unstructured{Object: map[string]any{}}, ready("False", "clone failed")), "Ready")
	assert.False(t, ok)
}

func TestPluginStatusFollowsTheRelease(t *testing.T) {
	t.Parallel()

	running := fluxStatus{sourceReady: true, sourceMessage: "stored artifact", releaseReady: true, releaseMessage: "Helm install succeeded"}.pluginStatus()
	assert.Equal(t, pluginruntime.PhaseRunning, running.Phase)
	assert.Contains(t, running.Message, "Helm install succeeded")

	installing := fluxStatus{sourceMessage: "cloning", releaseMessage: "not reconciled yet"}.pluginStatus()
	assert.Equal(t, pluginruntime.PhaseInstalling, installing.Phase)

	stalled := fluxStatus{sourceReady: true, releaseStalled: true, releaseMessage: "install retries exhausted"}.pluginStatus()
	assert.Equal(t, pluginruntime.PhaseDegraded, stalled.Phase)
}

func fluxObjects(releaseConditions ...map[string]any) []client.Object {
	cfg := validConfig()
	src := withCondition(newGitRepository(cfg), ready("True", "stored artifact"))
	hr := withCondition(newHelmRelease(cfg), releaseConditions...)
	return []client.Object{src, hr}
}

func TestReadFluxStatus(t *testing.T) {
	t.Parallel()
	c := fake.NewClientBuilder().WithObjects(fluxObjects(
		ready("False", "install retries exhausted"),
		map[string]any{"type": "Stalled", "status": "True", "message": "retries exhausted"},
	)...).Build()

	s, err := readFluxStatus(context.Background(), c, testNamespace)
	require.NoError(t, err)
	assert.True(t, s.sourceReady)
	assert.False(t, s.releaseReady)
	assert.True(t, s.releaseStalled)
	assert.Equal(t, pluginruntime.PhaseDegraded, s.pluginStatus().Phase)
}

func TestApplyFluxObjectsIsIdempotent(t *testing.T) {
	t.Parallel()
	ctx := context.Background()
	c := newFakeClient(t)
	cfg := validConfig()

	require.NoError(t, applyFluxObjects(ctx, c, cfg))
	cfg.CaseRef = "v2"
	require.NoError(t, applyFluxObjects(ctx, c, cfg), "a second apply with a new value must update, not fail")

	hr := &unstructured.Unstructured{}
	hr.SetGroupVersionKind(helmReleaseGVK)
	require.NoError(t, c.Get(ctx, client.ObjectKey{Namespace: testNamespace, Name: releaseName}, hr))
	ref, _, _ := unstructured.NestedString(hr.Object, "spec", "values", "case", "ref")
	assert.Equal(t, "v2", ref)
}

func TestUninstallRemovesTheFluxObjectsAndKeepsTheNamespace(t *testing.T) {
	t.Parallel()
	ctx := context.Background()
	ns := &corev1.Namespace{}
	ns.SetName(testNamespace)
	c := newFakeClient(t, append(fluxObjects(ready("True", "ok")), ns)...)
	p := &WetsprocesPlugin{k8sClient: c, cfg: validConfig(), deletePollInterval: time.Millisecond}
	p.initOnce.Do(func() {})

	require.NoError(t, p.Uninstall(ctx, nil))

	for _, obj := range []struct {
		gvk  string
		gone func() (bool, error)
	}{
		{"HelmRelease", func() (bool, error) { return objectGone(ctx, c, helmReleaseGVK, testNamespace, releaseName) }},
		{"GitRepository", func() (bool, error) { return objectGone(ctx, c, gitRepositoryGVK, testNamespace, sourceName) }},
	} {
		gone, err := obj.gone()
		require.NoError(t, err)
		assert.True(t, gone, "%s must be gone", obj.gvk)
	}
	require.NoError(t, c.Get(ctx, client.ObjectKey{Name: testNamespace}, &corev1.Namespace{}), "the namespace is Fundament's and stays")

	// A second uninstall, after a crash halfway, must succeed too.
	require.NoError(t, p.Uninstall(ctx, nil))
}

// recordingHost records each status that the plugin reports. The embedded
// Host is nil: a call to another method fails the test.
type recordingHost struct {
	pluginruntime.Host
	mu       sync.Mutex
	statuses []pluginruntime.PluginStatus
}

func (h *recordingHost) ReportStatus(s pluginruntime.PluginStatus) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.statuses = append(h.statuses, s)
}

func (h *recordingHost) ReportReady() {}

func (h *recordingHost) all() []pluginruntime.PluginStatus {
	h.mu.Lock()
	defer h.mu.Unlock()
	return append([]pluginruntime.PluginStatus(nil), h.statuses...)
}

func TestStatusReporterSendsOnlyChanges(t *testing.T) {
	t.Parallel()
	host := &recordingHost{}
	var r statusReporter

	s := fluxStatus{sourceReady: true, sourceMessage: "ok", releaseMessage: "installing"}
	r.flux(host, s)
	r.flux(host, s)
	s.releaseReady, s.releaseMessage = true, "Helm install succeeded"
	r.flux(host, s)

	got := host.all()
	require.Len(t, got, 2)
	assert.Equal(t, pluginruntime.PhaseRunning, got[1].Phase)
}

func TestStatusReporterKeepsReconcileError(t *testing.T) {
	t.Parallel()
	host := &recordingHost{}
	var r statusReporter
	healthy := fluxStatus{sourceReady: true, releaseReady: true, releaseMessage: "ok"}

	err := pluginerrors.NewTransient(errors.New("apply HelmRelease wetsproces: conflict"))
	r.reconciled(host, errorStatus(err), err)
	r.flux(host, healthy)
	got := host.all()
	require.Len(t, got, 1, "the Flux poll must not hide a failed Reconcile")
	assert.Equal(t, pluginruntime.PhaseDegraded, got[0].Phase)

	r.reconciled(host, healthy.pluginStatus(), nil)
	got = host.all()
	require.Len(t, got, 2)
	assert.Equal(t, pluginruntime.PhaseRunning, got[1].Phase)
}

func TestWatchFluxReportsReleaseChanges(t *testing.T) {
	t.Parallel()
	objs := fluxObjects(ready("False", "installing"))
	c := fake.NewClientBuilder().WithObjects(objs...).Build()
	p := &WetsprocesPlugin{k8sClient: c, cfg: validConfig(), pollInterval: 10 * time.Millisecond}
	host := &recordingHost{}

	ctx, cancel := context.WithCancel(t.Context())
	done := make(chan struct{})
	go func() {
		p.watchFlux(ctx, host)
		close(done)
	}()

	hasPhase := func(phase pluginruntime.PluginPhase) func() bool {
		return func() bool {
			for _, s := range host.all() {
				if s.Phase == phase {
					return true
				}
			}
			return false
		}
	}
	require.Eventually(t, hasPhase(pluginruntime.PhaseInstalling), time.Second, 5*time.Millisecond)

	hr := objs[1].(*unstructured.Unstructured)
	withCondition(hr, ready("True", "Helm install succeeded"))
	require.NoError(t, c.Update(ctx, hr))
	require.Eventually(t, hasPhase(pluginruntime.PhaseRunning), time.Second, 5*time.Millisecond)

	cancel()
	<-done
}
