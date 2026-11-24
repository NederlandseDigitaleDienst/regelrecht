package main

import (
	"context"
	"errors"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	apimeta "k8s.io/apimachinery/pkg/api/meta"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

// fakeParties answers as a running server would, and counts the questions.
type fakeParties struct {
	cells []string
	err   error
	asked int
}

func (f *fakeParties) Asked(context.Context, *v1alpha1.Wetsproces) (askedParties, error) {
	f.asked++
	var a askedParties
	if f.err != nil {
		return a, f.err
	}
	a.Executor.Cell = "gemeente_voorbeeld"
	for _, cell := range f.cells {
		a.Parties = append(a.Parties, struct {
			Cell string `json:"cel"`
			Name string `json:"cel_naam"`
		}{Cell: cell, Name: "GGD Voorbeeld"})
	}
	return a, nil
}

// withParties is a Wetsproces whose parties get a release of their own, on
// a reconciler whose running server names cells.
func withParties(t *testing.T, cells ...string) (*WetsprocesReconciler, *v1alpha1.Wetsproces, *fakeParties) {
	t.Helper()
	w := newWetsproces("a", "a")
	w.Spec.Parties.OwnInstances = true
	r := newReconciler(t, w)
	server := &fakeParties{cells: cells}
	r.Parties = server
	return r, w, server
}

func ready(t *testing.T, r *WetsprocesReconciler, name string) {
	t.Helper()
	setReleaseConditions(t, r.Client, name, map[string]any{"type": "Ready", "status": "True", "message": "Helm install succeeded"})
}

func peersOf(t *testing.T, hr *unstructured.Unstructured) []any {
	t.Helper()
	peers, _, err := unstructured.NestedSlice(hr.Object, "spec", "values", "koppeling", "peers")
	require.NoError(t, err)
	return peers
}

func TestPartiesStayInTheAdminPanelUnlessTheSpecAsks(t *testing.T) {
	t.Parallel()
	w := newWetsproces("a", "a")
	r := newReconciler(t, w)
	server := &fakeParties{cells: []string{"ggd_voorbeeld"}}
	r.Parties = server
	reconcileOnce(t, r, w)
	ready(t, r, "a")
	got := reconcileOnce(t, r, w)

	assert.Zero(t, server.asked, "a Wetsproces that does not ask is not looked into")
	assert.Empty(t, got.Status.Parties)
	assert.NotContains(t, chartValues(got, validConfig()), "koppeling")
	cond := apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionPartiesLinked)
	require.NotNil(t, cond)
	assert.Equal(t, reasonPartiesDisabled, cond.Reason)
}

func TestTheServerIsAskedOnceItsReleaseIsReady(t *testing.T) {
	t.Parallel()
	r, w, server := withParties(t, "ggd_voorbeeld")
	got := reconcileOnce(t, r, w)
	assert.Zero(t, server.asked)
	assert.Empty(t, got.Status.Parties)
	assert.False(t, apimeta.IsStatusConditionTrue(got.Status.Conditions, v1alpha1.ConditionPartiesLinked))
	assert.Empty(t, peersOf(t, fluxObject(t, r.Client, "HelmRelease", "a")), "the executor's release starts without a link")
}

func TestAPartyTheCaseAsksGetsALinkedReleaseOfItsOwn(t *testing.T) {
	t.Parallel()
	r, w, _ := withParties(t, "ggd_voorbeeld")
	reconcileOnce(t, r, w)
	ready(t, r, "a")
	got := reconcileOnce(t, r, w)

	require.Len(t, got.Status.Parties, 1)
	assert.Equal(t, v1alpha1.PartyStatus{Cell: "ggd_voorbeeld", Name: "GGD Voorbeeld", URL: "https://a-ggd-voorbeeld.cluster.example.org/"}, got.Status.Parties[0])
	assert.Equal(t, "gemeente_voorbeeld", got.Status.ExecutorCell)
	assert.True(t, apimeta.IsStatusConditionTrue(got.Status.Conditions, v1alpha1.ConditionPartiesLinked))

	// The executor's release now names the party as its peer.
	assert.Equal(t,
		[]any{map[string]any{"cel": "ggd_voorbeeld", "url": "https://a-ggd-voorbeeld.cluster.example.org"}},
		peersOf(t, fluxObject(t, r.Client, "HelmRelease", "a")))

	// The party's release: one instance for employees, the same case, and
	// the executor's employees' instance as its peer.
	party := fluxObject(t, r.Client, "HelmRelease", "a-ggd-voorbeeld")
	values, _, _ := unstructured.NestedMap(party.Object, "spec", "values")
	assert.Equal(t, "a-ggd-voorbeeld.cluster.example.org", values["host"])
	assert.Equal(t, map[string]any{"split": false}, values["surfaces"])
	assert.Equal(t, map[string]any{"surface": "backoffice"}, values["wetsproces"])
	assert.Equal(t, map[string]any{
		"partij": "ggd_voorbeeld",
		"peers":  []any{map[string]any{"cel": "gemeente_voorbeeld", "url": "https://a-medewerkers.cluster.example.org"}},
	}, values["koppeling"])
	main := fluxObject(t, r.Client, "HelmRelease", "a")
	mainCase, _, _ := unstructured.NestedMap(main.Object, "spec", "values", "case")
	assert.Equal(t, mainCase, values["case"])
	releaseName, _, _ := unstructured.NestedString(party.Object, "spec", "releaseName")
	assert.Equal(t, "a-ggd-voorbeeld", releaseName)
	require.Len(t, party.GetOwnerReferences(), 1, "deleting the Wetsproces deletes the party's release")
	assert.Equal(t, w.Name, party.GetOwnerReferences()[0].Name)
}

func TestACaseThatAsksNoPartyGetsNone(t *testing.T) {
	t.Parallel()
	r, w, server := withParties(t)
	reconcileOnce(t, r, w)
	ready(t, r, "a")
	got := reconcileOnce(t, r, w)

	assert.Equal(t, 1, server.asked)
	assert.Empty(t, got.Status.Parties)
	cond := apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionPartiesLinked)
	require.NotNil(t, cond)
	assert.Contains(t, cond.Message, "ask no party")
	assert.Empty(t, peersOf(t, fluxObject(t, r.Client, "HelmRelease", "a")))
}

func TestAPartyNoLongerAskedKeepsItsReleaseUntilTheOptionGoesOff(t *testing.T) {
	t.Parallel()
	r, w, server := withParties(t, "ggd_voorbeeld")
	reconcileOnce(t, r, w)
	ready(t, r, "a")
	reconcileOnce(t, r, w)
	fluxObject(t, r.Client, "HelmRelease", "a-ggd-voorbeeld")

	// The server names the GGD no longer, as when an admin sets the test
	// clock outside the policy's validity: its release and data stay.
	server.cells = nil
	got := reconcileOnce(t, r, w)
	require.Len(t, got.Status.Parties, 1)
	cond := apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionPartiesLinked)
	require.NotNil(t, cond)
	assert.Contains(t, cond.Message, "no longer asked but kept")
	fluxObject(t, r.Client, "HelmRelease", "a-ggd-voorbeeld")
	assert.Len(t, peersOf(t, fluxObject(t, r.Client, "HelmRelease", "a")), 1)

	// Turning the option off removes it.
	var current v1alpha1.Wetsproces
	require.NoError(t, r.Client.Get(context.Background(), client.ObjectKeyFromObject(w), &current))
	current.Spec.Parties.OwnInstances = false
	require.NoError(t, r.Client.Update(context.Background(), &current))
	got = reconcileOnce(t, r, w)
	assert.Empty(t, got.Status.Parties)
	hr := &unstructured.Unstructured{}
	hr.SetGroupVersionKind(helmReleaseGVK)
	err := r.Client.Get(context.Background(), client.ObjectKey{Namespace: testNamespace, Name: "a-ggd-voorbeeld"}, hr)
	assert.True(t, apierrors.IsNotFound(err), "got %v", err)
	assert.Empty(t, peersOf(t, fluxObject(t, r.Client, "HelmRelease", "a")))
}

func TestAPartyNeverTakesTheReleaseOfAnotherWetsproces(t *testing.T) {
	t.Parallel()
	// A Wetsproces whose own name is what the party's release would be.
	other := newWetsproces("a-ggd-voorbeeld", "elders")
	w := newWetsproces("a", "a")
	w.Spec.Parties.OwnInstances = true
	r := newReconciler(t, other, w)
	server := &fakeParties{cells: []string{"ggd_voorbeeld"}}
	r.Parties = server
	reconcileOnce(t, r, other)
	theirs := fluxObject(t, r.Client, "HelmRelease", "a-ggd-voorbeeld")
	reconcileOnce(t, r, w)
	ready(t, r, "a")
	got := reconcileOnce(t, r, w)

	assert.Empty(t, got.Status.Parties)
	cond := apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionPartiesLinked)
	require.NotNil(t, cond)
	assert.Contains(t, cond.Message, "left out")
	after := fluxObject(t, r.Client, "HelmRelease", "a-ggd-voorbeeld")
	assert.Equal(t, theirs.GetOwnerReferences(), after.GetOwnerReferences())
	assert.Equal(t, theirs.Object["spec"], after.Object["spec"])

	// Turning the option off prunes nothing of theirs either.
	var current v1alpha1.Wetsproces
	require.NoError(t, r.Client.Get(context.Background(), client.ObjectKeyFromObject(w), &current))
	current.Spec.Parties.OwnInstances = false
	require.NoError(t, r.Client.Update(context.Background(), &current))
	reconcileOnce(t, r, w)
	fluxObject(t, r.Client, "HelmRelease", "a-ggd-voorbeeld")
}

func TestPartyNamesThatClashAreLeftOut(t *testing.T) {
	t.Parallel()
	w := newWetsproces("a", "a")
	var asked askedParties
	for _, cell := range []string{"ggd_x", "ggd-x", "GGD_X", "medewerkers", "backoffice", "db", "ggd_x"} {
		asked.Parties = append(asked.Parties, struct {
			Cell string `json:"cel"`
			Name string `json:"cel_naam"`
		}{Cell: cell})
	}
	parties, skipped := partyStatuses(w, "cluster.example.org", asked)
	require.Len(t, parties, 1, "one release per name, and none on a name of the executor's release")
	assert.Equal(t, "ggd_x", parties[0].Cell)
	assert.Len(t, skipped, 6)
}

func TestAnAnswerWithoutAnExecutorIsNoAnswer(t *testing.T) {
	t.Parallel()
	r, w, _ := withParties(t, "ggd_voorbeeld")
	reconcileOnce(t, r, w)
	ready(t, r, "a")
	reconcileOnce(t, r, w)
	r.Parties = emptyAnswer{}
	got := reconcileOnce(t, r, w)
	assert.Len(t, got.Status.Parties, 1)
	assert.False(t, apimeta.IsStatusConditionTrue(got.Status.Conditions, v1alpha1.ConditionPartiesLinked))
}

type emptyAnswer struct{}

func (emptyAnswer) Asked(context.Context, *v1alpha1.Wetsproces) (askedParties, error) {
	return askedParties{}, nil
}

func TestAServerThatDoesNotAnswerKeepsWhatRunsAndIsAskedAgain(t *testing.T) {
	t.Parallel()
	r, w, server := withParties(t, "ggd_voorbeeld")
	reconcileOnce(t, r, w)
	ready(t, r, "a")
	reconcileOnce(t, r, w)

	server.err = errors.New("connection refused")
	result, err := r.Reconcile(context.Background(), ctrl.Request{NamespacedName: client.ObjectKeyFromObject(w)})
	require.NoError(t, err)
	assert.Equal(t, partiesRequeue, result.RequeueAfter)
	var got v1alpha1.Wetsproces
	require.NoError(t, r.Client.Get(context.Background(), client.ObjectKeyFromObject(w), &got))
	assert.Len(t, got.Status.Parties, 1, "a running link is not torn down because one question failed")
	cond := apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionPartiesLinked)
	require.NotNil(t, cond)
	assert.Contains(t, cond.Message, "connection refused")
	fluxObject(t, r.Client, "HelmRelease", "a-ggd-voorbeeld")
}

func TestPartiesOfTheirOwnAreRefusedInStableMode(t *testing.T) {
	t.Parallel()
	w := stable(newWetsproces("a", "a"), false)
	w.Spec.Parties.OwnInstances = true
	err := validateSpec(w, "cluster.example.org")
	require.Error(t, err)
	assert.Contains(t, err.Error(), "test mode only")
}

func TestAPartysAddressCountsForTheAddressCheck(t *testing.T) {
	t.Parallel()
	r, w, _ := withParties(t, "ggd_voorbeeld")
	reconcileOnce(t, r, w)
	ready(t, r, "a")
	reconcileOnce(t, r, w)

	// A later Wetsproces that wants the party's address is refused.
	late := newWetsproces("b", "a-ggd-voorbeeld")
	late.CreationTimestamp.Time = testNow
	require.NoError(t, r.Client.Create(context.Background(), late))
	got := reconcileOnce(t, r, late)
	assert.Equal(t, v1alpha1.PhaseFailed, got.Status.Phase)
	assert.Contains(t, got.Status.Message, "has already")
}

func TestPartyNamesFollowTheHostname(t *testing.T) {
	t.Parallel()
	w := newWetsproces("zorg", "zorg")
	assert.Equal(t, "zorg-ggd-voorbeeld.cluster.example.org", partyHost(w, "cluster.example.org", "ggd_voorbeeld"))
	own := newWetsproces("zorg-example-nl", "zorg.example.nl")
	assert.Equal(t, "ggd-voorbeeld.zorg.example.nl", partyHost(own, "cluster.example.org", "ggd_voorbeeld"))

	// A name too long for a release is left out, with the reason.
	var asked askedParties
	asked.Parties = append(asked.Parties, struct {
		Cell string `json:"cel"`
		Name string `json:"cel_naam"`
	}{Cell: "een_partij_met_een_naam_die_veel_te_lang_is_voor_een_release"})
	parties, skipped := partyStatuses(w, "cluster.example.org", asked)
	assert.Empty(t, parties)
	require.Len(t, skipped, 1)
	assert.Contains(t, skipped[0], "no valid")
}
