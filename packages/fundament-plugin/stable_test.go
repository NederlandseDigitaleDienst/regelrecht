package main

import (
	"context"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	corev1 "k8s.io/api/core/v1"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	apimeta "k8s.io/apimachinery/pkg/api/meta"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/types"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

const pinned = "0123456789abcdef0123456789abcdef01234567"

func stable(w *v1alpha1.Wetsproces, testAccounts bool) *v1alpha1.Wetsproces {
	w.Spec.Auth = v1alpha1.Auth{Mode: v1alpha1.AuthStable, TestAccounts: testAccounts}
	return w
}

func TestTestModeLeavesTheLoginToTheChartsDefaults(t *testing.T) {
	t.Parallel()
	for _, mode := range []string{"", v1alpha1.AuthTest} {
		w := newWetsproces("a", "a")
		w.Spec.Auth.Mode = mode
		values := chartValues(w, validConfig())
		assert.NotContains(t, values, "wetsproces", "mode %q", mode)
		assert.NotContains(t, values, "keycloak", "mode %q", mode)
		assert.Empty(t, hostsOf(w, "cluster.example.org").login)
	}
}

func TestStableModeTurnsOnTheChartsKeycloak(t *testing.T) {
	t.Parallel()
	w := stable(newWetsproces("zorgtoeslag", "zorgtoeslag"), true)
	values := chartValues(w, validConfig())
	assert.Equal(t, map[string]any{"mode": "stable", "login": "oidc"}, values["wetsproces"])
	assert.Equal(t, map[string]any{
		"enabled":   true,
		"host":      "zorgtoeslag-login.cluster.example.org",
		"testUsers": true,
	}, values["keycloak"])
	_, database := values["database"]
	assert.False(t, database, "PostgreSQL is the chart's default, which stable mode needs")

	own := stable(newWetsproces("zorg", "zorg.example.nl"), false)
	assert.Equal(t, "zorg-login.example.nl", hostsOf(own, "cluster.example.org").login)
}

func TestTheLoginHostCountsForTheAddressCheck(t *testing.T) {
	t.Parallel()
	login := hostsOf(stable(newWetsproces("a", "a"), false), "cluster.example.org")
	other := hostsOf(newWetsproces("x", "a-login"), "cluster.example.org")
	assert.True(t, login.overlaps(other))
	assert.False(t, hostsOf(newWetsproces("a", "a"), "cluster.example.org").overlaps(other))
}

func TestAVersionPinsTheChartToItsCommit(t *testing.T) {
	t.Parallel()
	w := newWetsproces("a", "a")
	ref, _, _ := unstructured.NestedStringMap(newGitRepository(w, validConfig()).Object, "spec", "ref")
	assert.Equal(t, map[string]string{"branch": "main"}, ref)

	w.Spec.Version = pinned
	ref, _, _ = unstructured.NestedStringMap(newGitRepository(w, validConfig()).Object, "spec", "ref")
	assert.Equal(t, map[string]string{"commit": pinned}, ref, "the chart comes from the commit the image was built from")
}

func TestStatusNamesTheLoginAndTheSecretInStableModeOnly(t *testing.T) {
	t.Parallel()
	test := newWetsproces("a", "a")
	sw := stable(newWetsproces("b", "b"), true)
	r := newReconciler(t, test, sw)
	r.Config.ACME = acmeProduction

	got := reconcileOnce(t, r, test)
	assert.Empty(t, got.Status.LoginURL)
	assert.Empty(t, got.Status.KeycloakSecret)

	got = reconcileOnce(t, r, sw)
	assert.Equal(t, "https://b-login.cluster.example.org/", got.Status.LoginURL)
	assert.Equal(t, "b-keycloak", got.Status.KeycloakSecret)
	hr := fluxObject(t, r.Client, "HelmRelease", "b")
	enabled, _, _ := unstructured.NestedBool(hr.Object, "spec", "values", "keycloak", "enabled")
	assert.True(t, enabled)
}

// A pinned commit from before the chart existed: Flux keeps retrying without
// stalling, and the plugin says at once why it will never work.
func TestACommitWithoutTheChartFails(t *testing.T) {
	t.Parallel()
	missing := map[string]any{
		"type": "Ready", "status": "False", "reason": "SourceNotReady",
		"message": "HelmChart 'ns/ns-a' is not ready: invalid chart reference: stat /tmp/source/charts/wetsproces: no such file or directory",
	}

	w := newWetsproces("a", "a")
	w.Spec.Version = pinned
	r := newReconciler(t, w)
	reconcileOnce(t, r, w)
	setReleaseConditions(t, r.Client, "a", missing)
	got := reconcileOnce(t, r, w)
	assert.Equal(t, v1alpha1.PhaseFailed, got.Status.Phase)
	ready := apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionReady)
	require.NotNil(t, ready)
	assert.Equal(t, reasonChartNotInCommit, ready.Reason)
	assert.Contains(t, got.Status.Message, pinned)

	// Without a version the source is main, whose chart a later commit can
	// bring back; the release stays pending.
	unpinned := newWetsproces("b", "b")
	r = newReconciler(t, unpinned)
	reconcileOnce(t, r, unpinned)
	setReleaseConditions(t, r.Client, "b", missing)
	got = reconcileOnce(t, r, unpinned)
	assert.Equal(t, v1alpha1.PhasePending, got.Status.Phase)
}

// On an installation with Let's Encrypt staging, stable mode would never
// start: the server does not trust Keycloak's certificate. The plugin says so
// and makes nothing; test mode runs as before.
func TestStableModeNeedsProductionCertificates(t *testing.T) {
	t.Parallel()
	sw := stable(newWetsproces("b", "b"), false)
	tw := newWetsproces("a", "a")
	r := newReconciler(t, sw, tw)
	require.Equal(t, acmeStaging, r.Config.ACME)

	got := reconcileOnce(t, r, sw)
	assert.Equal(t, v1alpha1.PhaseFailed, got.Status.Phase)
	ready := apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionReady)
	require.NotNil(t, ready)
	assert.Equal(t, reasonStableNeedsProductionCerts, ready.Reason)
	assert.Contains(t, ready.Message, "staging")
	var list unstructured.UnstructuredList
	list.SetGroupVersionKind(helmReleaseGVK.GroupVersion().WithKind("HelmReleaseList"))
	require.NoError(t, r.Client.List(context.Background(), &list))
	assert.Empty(t, list.Items, "nothing is made for a refused Wetsproces")

	assert.Equal(t, v1alpha1.PhasePending, reconcileOnce(t, r, tw).Status.Phase, "test mode is unaffected")
}

// After a Wetsproces is deleted, the plugin deletes the Secret the chart
// generated for its Keycloak; a Secret of that name without the chart's
// labels for that release stays.
func TestDeletingAWetsprocesDeletesItsKeycloakSecret(t *testing.T) {
	t.Parallel()
	ctx := context.Background()
	secret := func(name, release string) *corev1.Secret {
		return &corev1.Secret{ObjectMeta: metav1.ObjectMeta{
			Name: name, Namespace: testNamespace, UID: types.UID("uid-" + name),
			Labels: map[string]string{appLabel: appName, instanceLabel: release, componentLabel: "keycloak"},
		}}
	}
	gone := secret("weg-keycloak", "weg")
	handMade := &corev1.Secret{ObjectMeta: metav1.ObjectMeta{Name: "zelf-keycloak", Namespace: testNamespace}}
	stays := stable(newWetsproces("blijft", "blijft"), false)
	staysSecret := secret("blijft-keycloak", "blijft")
	r := newReconciler(t, gone, handMade, stays, staysSecret)
	r.Config.ACME = acmeProduction

	for _, name := range []string{"weg", "zelf", "nooit-geweest"} {
		_, err := r.Reconcile(ctx, ctrl.Request{NamespacedName: client.ObjectKey{Namespace: testNamespace, Name: name}})
		require.NoError(t, err, name)
	}
	reconcileOnce(t, r, stays)

	var s corev1.Secret
	err := r.Client.Get(ctx, client.ObjectKeyFromObject(gone), &s)
	assert.True(t, apierrors.IsNotFound(err), "the Keycloak Secret of a deleted Wetsproces goes")
	require.NoError(t, r.Client.Get(ctx, client.ObjectKeyFromObject(handMade), &s), "a Secret without the chart's labels stays")
	require.NoError(t, r.Client.Get(ctx, client.ObjectKeyFromObject(staysSecret), &s), "the Secret of a Wetsproces that exists stays")
}
