package main

import (
	"context"
	"os"
	"testing"
	"time"

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
	"sigs.k8s.io/yaml"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

// The namespaces of the tests: two of one Fundament project, one of another.
const (
	orgNamespace   = "tnt-voorbeeld--wilgendaal"
	otherNamespace = "tnt-ander--project"
)

func projectNamespace(name, project string) *corev1.Namespace {
	return &corev1.Namespace{ObjectMeta: metav1.ObjectMeta{Name: name, Labels: map[string]string{projectLabel: project}}}
}

func testNamespaces() []client.Object {
	return []client.Object{
		projectNamespace(testNamespace, "project-voorbeeld"),
		projectNamespace(orgNamespace, "project-voorbeeld"),
		projectNamespace(otherNamespace, "project-ander"),
	}
}

func newOrganisatie(name, hostname string) *v1alpha1.Organisatie {
	return &v1alpha1.Organisatie{
		TypeMeta: metav1.TypeMeta{APIVersion: v1alpha1.GroupVersion.String(), Kind: "Organisatie"},
		ObjectMeta: metav1.ObjectMeta{
			Name:      name,
			Namespace: orgNamespace,
			UID:       types.UID("uid-org-" + name),
		},
		Spec: v1alpha1.OrganisatieSpec{DisplayName: "Gemeente Wilgendaal", Hostname: hostname},
	}
}

// inOrganisatie puts w on the list of o.
func inOrganisatie(w *v1alpha1.RegelHet, o *v1alpha1.Organisatie) *v1alpha1.RegelHet {
	w.Spec.Organisatie = &v1alpha1.OrganisatieReference{Name: o.Name, Namespace: o.Namespace}
	return w
}

// reconcilers are both controllers on one fake cluster.
func reconcilers(t *testing.T, objs ...client.Object) (*RegelHetReconciler, *OrganisatieReconciler) {
	t.Helper()
	w := newReconciler(t, append(testNamespaces(), objs...)...)
	o := &OrganisatieReconciler{Client: w.Client, Namespaces: w.Client, Secrets: w.Client, Scheme: w.Scheme, Config: w.Config, Now: func() time.Time { return testNow }}
	return w, o
}

func reconcileOrganisatie(t *testing.T, r *OrganisatieReconciler, o *v1alpha1.Organisatie) *v1alpha1.Organisatie {
	t.Helper()
	_, err := r.Reconcile(context.Background(), ctrl.Request{NamespacedName: client.ObjectKeyFromObject(o)})
	require.NoError(t, err)
	var got v1alpha1.Organisatie
	require.NoError(t, r.Client.Get(context.Background(), client.ObjectKeyFromObject(o), &got))
	return &got
}

func releaseValues(t *testing.T, c client.Client, namespace, name string) map[string]any {
	t.Helper()
	hr := &unstructured.Unstructured{}
	hr.SetGroupVersionKind(helmReleaseGVK)
	require.NoError(t, c.Get(context.Background(), client.ObjectKey{Namespace: namespace, Name: name}, hr))
	values, _, err := unstructured.NestedMap(hr.Object, "spec", "values")
	require.NoError(t, err)
	return values
}

func TestAnOrganisatieRunsAFrontOfItsOwn(t *testing.T) {
	t.Parallel()
	o := newOrganisatie("wilgendaal", "wilgendaal")
	o.Spec.Logo = "https://example.org/logo.svg"
	o.Spec.Theme.AccentColor = "groen"
	_, r := reconcilers(t, o)
	got := reconcileOrganisatie(t, r, o)

	assert.Equal(t, v1alpha1.PhasePending, got.Status.Phase)
	assert.Equal(t, "https://wilgendaal.cluster.example.org/", got.Status.URL)
	assert.Equal(t, "https://wilgendaal-medewerkers.cluster.example.org/", got.Status.BackofficeURL)
	assert.Empty(t, got.Status.LoginURL)
	assert.Empty(t, got.Status.Processen)

	for _, kind := range []string{"GitRepository", "HelmRelease"} {
		obj := &unstructured.Unstructured{}
		name := "wilgendaal"
		obj.SetGroupVersionKind(helmReleaseGVK)
		if kind == "GitRepository" {
			obj.SetGroupVersionKind(gitRepositoryGVK)
			name += "-chart"
		}
		require.NoError(t, r.Client.Get(context.Background(), client.ObjectKey{Namespace: orgNamespace, Name: name}, obj))
		refs := obj.GetOwnerReferences()
		require.Len(t, refs, 1, kind)
		assert.Equal(t, "Organisatie", refs[0].Kind)
		assert.Equal(t, o.UID, refs[0].UID)
	}

	values := releaseValues(t, r.Client, orgNamespace, "wilgendaal")
	assert.Equal(t, map[string]any{
		"enabled":   true,
		"id":        orgNamespace + "/wilgendaal",
		"naam":      "Gemeente Wilgendaal",
		"logo":      "https://example.org/logo.svg",
		"processen": []any{},
	}, values["voorkant"])
	assert.NotContains(t, values, "case", "the front clones no case")
	assert.Equal(t, map[string]any{"accentColor": "groen"}, values["theme"])
}

func TestARegelHetWithoutOrganisatieIsUnchanged(t *testing.T) {
	t.Parallel()
	w := newRegelHet("parkeerkaart", "parkeerkaart")
	r, _ := reconcilers(t, w)
	got := reconcileOnce(t, r, w)
	assert.Nil(t, apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionOrganisatieFound))
	values := releaseValues(t, r.Client, testNamespace, "parkeerkaart")
	assert.NotContains(t, values, "organisatie")
	assert.Equal(t, chartValues(w, validConfig(), statusSnapshot(got)), values)
}

// Two RegelHet instances of one project, each in a namespace of its own, behind
// the front of Wilgendaal in a third: each gets the block organisatie, and the
// front lists both, sorted, with their addresses.
func TestRegelHetsJoinAnOrganisatieAcrossNamespacesOfOneProject(t *testing.T) {
	t.Parallel()
	o := newOrganisatie("wilgendaal", "wilgendaal")
	parkeer := inOrganisatie(newRegelHet("parkeerkaart", "parkeerkaart"), o)
	iit := inOrganisatie(newRegelHet("iit", "iit-wilgendaal"), o)
	iit.Namespace = orgNamespace
	rw, ro := reconcilers(t, o, parkeer, iit)

	for _, w := range []*v1alpha1.RegelHet{parkeer, iit} {
		got := reconcileOnce(t, rw, w)
		found := apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionOrganisatieFound)
		require.NotNil(t, found, w.Name)
		assert.Equal(t, metav1.ConditionTrue, found.Status, w.Name)
		assert.Equal(t, map[string]any{
			"id":             orgNamespace + "/wilgendaal",
			"naam":           "Gemeente Wilgendaal",
			"url":            "https://wilgendaal.cluster.example.org",
			"medewerkersUrl": "https://wilgendaal-medewerkers.cluster.example.org",
			"sleutelUrl":     "http://wilgendaal-backoffice." + orgNamespace + ".svc.cluster.local:8000/organisatie/v1/sleutel",
		}, releaseValues(t, rw.Client, w.Namespace, w.Name)["organisatie"], w.Name)
	}

	got := reconcileOrganisatie(t, ro, o)
	assert.Equal(t, []v1alpha1.ProcessReference{
		{Namespace: testNamespace, Name: "parkeerkaart", Phase: v1alpha1.PhasePending},
		{Namespace: orgNamespace, Name: "iit", Phase: v1alpha1.PhasePending},
	}, got.Status.Processen)

	voorkant := releaseValues(t, ro.Client, orgNamespace, "wilgendaal")["voorkant"].(map[string]any)
	processen := voorkant["processen"].([]any)
	require.Len(t, processen, 2)
	assert.Equal(t, map[string]any{
		"id":   testNamespace + "/parkeerkaart",
		"naam": "parkeerkaart",
		"burger": map[string]any{
			"intern":  "http://parkeerkaart-burger." + testNamespace + ".svc.cluster.local:8000",
			"publiek": "https://parkeerkaart.cluster.example.org",
		},
		"backoffice": map[string]any{
			"intern":  "http://parkeerkaart-backoffice." + testNamespace + ".svc.cluster.local:8000",
			"publiek": "https://parkeerkaart-medewerkers.cluster.example.org",
		},
	}, processen[0])
	assert.Equal(t, orgNamespace+"/iit", processen[1].(map[string]any)["id"])
}

// A RegelHet in another Fundament project cannot join the front: it runs on
// its own, and the front does not list it.
func TestAnOrganisatieOfAnotherProjectIsRefused(t *testing.T) {
	t.Parallel()
	o := newOrganisatie("wilgendaal", "wilgendaal")
	w := inOrganisatie(newRegelHet("indringer", "indringer"), o)
	w.Namespace = otherNamespace
	rw, ro := reconcilers(t, o, w)

	got := reconcileOnce(t, rw, w)
	found := apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionOrganisatieFound)
	require.NotNil(t, found)
	assert.Equal(t, metav1.ConditionFalse, found.Status)
	assert.Equal(t, reasonOrganisatieElsewhere, found.Reason)
	assert.NotEqual(t, v1alpha1.PhaseFailed, got.Status.Phase, "it runs on its own")
	assert.NotContains(t, releaseValues(t, rw.Client, otherNamespace, "indringer"), "organisatie")

	assert.Empty(t, reconcileOrganisatie(t, ro, o).Status.Processen)
}

func TestANamespaceWithoutProjectIsInNoProject(t *testing.T) {
	t.Parallel()
	_, ro := reconcilers(t, &corev1.Namespace{ObjectMeta: metav1.ObjectMeta{Name: "los"}})
	same, err := sameProject(context.Background(), ro.Namespaces, "los", orgNamespace)
	require.NoError(t, err)
	assert.False(t, same)
	same, err = sameProject(context.Background(), ro.Namespaces, "onbekend", "onbekend")
	require.NoError(t, err)
	assert.True(t, same, "one namespace is one project, read or not")
}

// An organisatie that is not there, or goes away, leaves the RegelHet
// running on its own.
func TestAMissingOrganisatieLeavesTheRegelHetRunning(t *testing.T) {
	t.Parallel()
	o := newOrganisatie("wilgendaal", "wilgendaal")
	w := inOrganisatie(newRegelHet("parkeerkaart", "parkeerkaart"), o)
	rw, _ := reconcilers(t, w)

	got := reconcileOnce(t, rw, w)
	found := apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionOrganisatieFound)
	require.NotNil(t, found)
	assert.Equal(t, reasonOrganisatieNotFound, found.Reason)
	assert.Equal(t, v1alpha1.PhasePending, got.Status.Phase)
	assert.NotContains(t, releaseValues(t, rw.Client, testNamespace, "parkeerkaart"), "organisatie")

	// It appears, and the watch on organisaties brings the RegelHet back.
	require.NoError(t, rw.Client.Create(context.Background(), o))
	assert.Equal(t, []ctrl.Request{{NamespacedName: client.ObjectKeyFromObject(w)}}, rw.namingRegelHets(context.Background(), o))
	got = reconcileOnce(t, rw, w)
	assert.True(t, apimeta.IsStatusConditionTrue(got.Status.Conditions, v1alpha1.ConditionOrganisatieFound))
	assert.Contains(t, releaseValues(t, rw.Client, testNamespace, "parkeerkaart"), "organisatie")

	// It goes, and the RegelHet runs on its own again.
	require.NoError(t, rw.Client.Delete(context.Background(), o))
	got = reconcileOnce(t, rw, w)
	assert.False(t, apimeta.IsStatusConditionTrue(got.Status.Conditions, v1alpha1.ConditionOrganisatieFound))
	assert.NotContains(t, releaseValues(t, rw.Client, testNamespace, "parkeerkaart"), "organisatie")

	// Leaving the organisatie drops the condition.
	got.Spec.Organisatie = nil
	require.NoError(t, rw.Client.Update(context.Background(), got))
	got = reconcileOnce(t, rw, got)
	assert.Nil(t, apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionOrganisatieFound))
}

// A stable RegelHet behind a front in test mode would let anyone in as
// anyone: it is refused, and the front does not list it.
func TestAStableRegelHetCannotStandBehindATestFront(t *testing.T) {
	t.Parallel()
	o := newOrganisatie("wilgendaal", "wilgendaal")
	w := stable(inOrganisatie(newRegelHet("parkeerkaart", "parkeerkaart"), o), false)
	rw, ro := reconcilers(t, o, w)
	rw.Config.ACME = acmeProduction

	got := reconcileOnce(t, rw, w)
	assert.Equal(t, v1alpha1.PhaseFailed, got.Status.Phase)
	assert.Equal(t, reasonOrganisatieModeMismatch, apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionReady).Reason)
	assert.Empty(t, reconcileOrganisatie(t, ro, o).Status.Processen)
}

// An organisatie and a RegelHet share the address check, and in one
// namespace the name of their release.
func TestAnOrganisatieAndARegelHetDoNotShareAnAddressOrAName(t *testing.T) {
	t.Parallel()
	w := newRegelHet("wilgendaal", "wilgendaal")
	w.CreationTimestamp = metav1.NewTime(time.Unix(1000, 0))
	o := newOrganisatie("wilgendaal-org", "wilgendaal")
	o.CreationTimestamp = metav1.NewTime(time.Unix(2000, 0))
	same := newOrganisatie("parkeerkaart", "parkeerkaart-organisatie")
	same.Namespace = testNamespace
	same.CreationTimestamp = metav1.NewTime(time.Unix(3000, 0))
	kaart := newRegelHet("parkeerkaart", "parkeerkaart")
	kaart.CreationTimestamp = metav1.NewTime(time.Unix(1500, 0))
	_, ro := reconcilers(t, w, o, same, kaart)

	got := reconcileOrganisatie(t, ro, o)
	assert.Equal(t, v1alpha1.PhaseFailed, got.Status.Phase)
	ready := apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionReady)
	assert.Equal(t, reasonHostnameInUse, ready.Reason)
	assert.Contains(t, ready.Message, "RegelHet "+testNamespace+"/wilgendaal")

	got = reconcileOrganisatie(t, ro, same)
	assert.Equal(t, reasonNameInUse, apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionReady).Reason)
}

// The pods of an Organisatie map onto a RegelHet of its name too; the
// clean-up after a deleted RegelHet must leave its Secrets alone.
func TestTheKeycloakCleanUpLeavesAnOrganisatieAlone(t *testing.T) {
	t.Parallel()
	o := newOrganisatie("wilgendaal", "wilgendaal")
	secret := &corev1.Secret{ObjectMeta: metav1.ObjectMeta{
		Name: "wilgendaal-keycloak", Namespace: orgNamespace,
		Labels: map[string]string{appLabel: appName, instanceLabel: "wilgendaal", componentLabel: "keycloak"},
	}}
	rw, _ := reconcilers(t, o, secret)
	_, err := rw.Reconcile(context.Background(), ctrl.Request{NamespacedName: client.ObjectKeyFromObject(o)})
	require.NoError(t, err)
	require.NoError(t, rw.Client.Get(context.Background(), client.ObjectKeyFromObject(secret), &corev1.Secret{}))
}

func TestAnOrganisatieWithAnInvalidLogoMakesNothing(t *testing.T) {
	t.Parallel()
	o := newOrganisatie("wilgendaal", "wilgendaal")
	o.Spec.Logo = "https://u:p@example.org/logo.svg"
	_, ro := reconcilers(t, o)
	got := reconcileOrganisatie(t, ro, o)
	assert.Equal(t, v1alpha1.PhaseFailed, got.Status.Phase)
	assert.Equal(t, reasonInvalidSpec, apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionReady).Reason)
}

// Like TestChartValuesExistInChart, for the blocks voorkant and organisatie.
func TestOrganisatieValuesExistInChart(t *testing.T) {
	t.Parallel()
	raw, err := os.ReadFile("../../charts/regelhet/values.yaml")
	require.NoError(t, err)
	var chart map[string]any
	require.NoError(t, yaml.Unmarshal(raw, &chart))

	o := newOrganisatie("wilgendaal", "wilgendaal")
	o.Spec.Version = pinned
	w := newRegelHet("parkeerkaart", "parkeerkaart")
	voorkant := voorkantValues(o, []*v1alpha1.RegelHet{w}, validConfig())
	proces := chartValues(w, validConfig(), testSnapshot)
	proces["organisatie"] = organisatieValues(o, validConfig())
	for _, values := range []map[string]any{voorkant, proces} {
		for key, value := range values {
			have, ok := chart[key]
			require.True(t, ok, "values.yaml has no %s", key)
			sub, isMap := value.(map[string]any)
			if !isMap {
				continue
			}
			haveMap, ok := have.(map[string]any)
			require.True(t, ok, "%s is no map in values.yaml", key)
			for k := range sub {
				assert.Contains(t, haveMap, k, "values.yaml has no %s.%s", key, k)
			}
		}
	}
}

// A RegelHet the plugin refuses, here for an address another has, is on no
// list: the front would send people to the other one.
func TestARefusedRegelHetIsOnNoList(t *testing.T) {
	t.Parallel()
	o := newOrganisatie("wilgendaal", "wilgendaal")
	older := newRegelHet("parkeerkaart", "parkeerkaart")
	older.CreationTimestamp = metav1.NewTime(time.Unix(1000, 0))
	clash := inOrganisatie(newRegelHet("tweede", "parkeerkaart"), o)
	clash.CreationTimestamp = metav1.NewTime(time.Unix(2000, 0))
	rw, ro := reconcilers(t, o, older, clash)

	got := reconcileOnce(t, rw, clash)
	assert.Equal(t, v1alpha1.PhaseFailed, got.Status.Phase)
	assert.Nil(t, apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionOrganisatieFound),
		"a refused regelhet says nothing about its organisatie")
	assert.Empty(t, reconcileOrganisatie(t, ro, o).Status.Processen)
}

// An organisatie the plugin refuses takes no RegelHet: with a name another
// release has, its key would be served by that other release.
func TestARefusedOrganisatieTakesNoRegelHet(t *testing.T) {
	t.Parallel()
	older := newRegelHet("wilgendaal", "wilgendaal-proces")
	older.Namespace = orgNamespace
	older.CreationTimestamp = metav1.NewTime(time.Unix(1000, 0))
	o := newOrganisatie("wilgendaal", "wilgendaal")
	o.CreationTimestamp = metav1.NewTime(time.Unix(2000, 0))
	w := inOrganisatie(newRegelHet("parkeerkaart", "parkeerkaart"), o)
	rw, _ := reconcilers(t, older, o, w)

	got := reconcileOnce(t, rw, w)
	found := apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionOrganisatieFound)
	require.NotNil(t, found)
	assert.Equal(t, reasonOrganisatieRefused, found.Reason)
	assert.NotEqual(t, v1alpha1.PhaseFailed, got.Status.Phase, "it runs on its own")
	assert.NotContains(t, releaseValues(t, rw.Client, testNamespace, "parkeerkaart"), "organisatie")
}

// Once an organisatie is gone, the Secret with its signing key goes too, and
// only that one.
func TestADeletedOrganisatieTakesItsKeyAlong(t *testing.T) {
	t.Parallel()
	secret := func(name, instance, component string) *corev1.Secret {
		return &corev1.Secret{ObjectMeta: metav1.ObjectMeta{
			Name: name, Namespace: orgNamespace,
			Labels: map[string]string{appLabel: appName, instanceLabel: instance, componentLabel: component},
		}}
	}
	key := secret("wilgendaal-organisatie", "wilgendaal", "organisatie")
	byHand := secret("ander-organisatie", "iets-anders", "organisatie")
	_, ro := reconcilers(t, key, byHand)
	for _, name := range []string{"wilgendaal", "ander"} {
		_, err := ro.Reconcile(context.Background(), ctrl.Request{NamespacedName: client.ObjectKey{Namespace: orgNamespace, Name: name}})
		require.NoError(t, err)
	}
	err := ro.Client.Get(context.Background(), client.ObjectKeyFromObject(key), &corev1.Secret{})
	assert.True(t, apierrors.IsNotFound(err), "the key of the deleted organisatie is gone")
	require.NoError(t, ro.Client.Get(context.Background(), client.ObjectKeyFromObject(byHand), &corev1.Secret{}),
		"a Secret without the labels of that release stays")
}
