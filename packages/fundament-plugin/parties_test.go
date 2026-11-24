package main

import (
	"context"
	"errors"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	apimeta "k8s.io/apimachinery/pkg/api/meta"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/controller/controllerutil"
	"sigs.k8s.io/controller-runtime/pkg/reconcile"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

// fakeParties answers as a running server would, and counts the questions.
type fakeParties struct {
	cells []string
	err   error
	asked int
}

func (f *fakeParties) Asked(context.Context, *v1alpha1.RegelHet) (askedParties, error) {
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

// asParty makes w run as cell for the executor e.
func asParty(w *v1alpha1.RegelHet, cell string, e *v1alpha1.RegelHet) *v1alpha1.RegelHet {
	ref := v1alpha1.RegelHetReference{Name: e.Name}
	if e.Namespace != w.Namespace {
		ref.Namespace = e.Namespace
	}
	w.Spec.Partij = &v1alpha1.Partij{Cel: cell, Uitvoerder: ref}
	return w
}

// executorWith is the executor "a" on a fake cluster with others, whose
// running server names cells.
func executorWith(t *testing.T, cells []string, others ...client.Object) (*RegelHetReconciler, *v1alpha1.RegelHet, *fakeParties) {
	t.Helper()
	e := newRegelHet("a", "a")
	r := newReconciler(t, append(append(testNamespaces(), e), others...)...)
	server := &fakeParties{cells: cells}
	r.Parties = server
	return r, e, server
}

// runs installs the executor and lets its server answer once.
func runs(t *testing.T, r *RegelHetReconciler, e *v1alpha1.RegelHet) *v1alpha1.RegelHet {
	t.Helper()
	reconcileOnce(t, r, e)
	ready(t, r, e.Name)
	return reconcileOnce(t, r, e)
}

func ready(t *testing.T, r *RegelHetReconciler, name string) {
	t.Helper()
	setReleaseConditions(t, r.Client, name, map[string]any{"type": "Ready", "status": "True", "message": "Helm install succeeded"})
}

func peersOf(t *testing.T, hr *unstructured.Unstructured) []any {
	t.Helper()
	peers, _, err := unstructured.NestedSlice(hr.Object, "spec", "values", "koppeling", "peers")
	require.NoError(t, err)
	return peers
}

func partiesLinked(t *testing.T, w *v1alpha1.RegelHet) *metav1.Condition {
	t.Helper()
	cond := apimeta.FindStatusCondition(w.Status.Conditions, v1alpha1.ConditionPartiesLinked)
	require.NotNil(t, cond)
	return cond
}

func TestTheExecutorsServerIsAskedOnceItsReleaseIsReady(t *testing.T) {
	t.Parallel()
	r, e, server := executorWith(t, []string{"ggd_voorbeeld"})
	got := reconcileOnce(t, r, e)
	assert.Zero(t, server.asked)
	assert.Empty(t, got.Status.AskedParties)
	assert.Empty(t, peersOf(t, fluxObject(t, r.Client, "HelmRelease", "a")))

	ready(t, r, "a")
	got = reconcileOnce(t, r, e)
	assert.Equal(t, 1, server.asked)
	assert.Equal(t, "gemeente_voorbeeld", got.Status.ExecutorCell)
	assert.Equal(t, []v1alpha1.AskedParty{{Cell: "ggd_voorbeeld", Name: "GGD Voorbeeld"}}, got.Status.AskedParties)
	assert.Empty(t, got.Status.Parties)
	cond := partiesLinked(t, got)
	assert.Equal(t, metav1.ConditionTrue, cond.Status)
	assert.Equal(t, "no RegelHet runs as a party of this one", cond.Message)
	assert.NotContains(t, chartValues(got, validConfig(), testSnapshot), "koppeling")
}

func TestACaseThatAsksNoPartySaysSo(t *testing.T) {
	t.Parallel()
	r, e, _ := executorWith(t, nil)
	got := runs(t, r, e)
	assert.Empty(t, got.Status.AskedParties)
	assert.Contains(t, partiesLinked(t, got).Message, "ask no party for anything")
}

func TestAPartyIsARegelHetOfItsOwnLinkedWithItsExecutor(t *testing.T) {
	t.Parallel()
	p := newRegelHet("ggd", "ggd")
	p.Spec.Title = "GGD Voorbeeld"
	p.Spec.Theme.AccentColor = "rood"
	p.Spec.Casus = "geneeskundig_advies"
	r, e, _ := executorWith(t, []string{"ggd_voorbeeld"}, asParty(p, "ggd_voorbeeld", newRegelHet("a", "a")))
	runs(t, r, e)

	party := reconcileOnce(t, r, p)
	assert.Equal(t, v1alpha1.PhasePending, party.Status.Phase, "its release is installing")
	assert.Empty(t, party.Status.URL, "a party has no citizens' side")
	assert.Equal(t, "https://ggd.cluster.example.org/", party.Status.BackofficeURL)
	cond := partiesLinked(t, party)
	assert.Equal(t, metav1.ConditionTrue, cond.Status)
	assert.Contains(t, cond.Message, "runs as ggd_voorbeeld for the executor")
	assert.Empty(t, party.Status.AskedParties, "a party asks nothing of its own server")

	// The party's own release: one instance for its staff, its own case,
	// title and colour, and the executor's employees' instance as its peer.
	values := releaseValues(t, r.Client, testNamespace, "ggd")
	assert.Equal(t, "ggd.cluster.example.org", values["host"])
	assert.Equal(t, map[string]any{"split": false}, values["surfaces"])
	assert.Equal(t, map[string]any{"surface": "backoffice"}, values["regelhet"])
	assert.Equal(t, map[string]any{
		"partij": "ggd_voorbeeld",
		"peers":  []any{map[string]any{"cel": "gemeente_voorbeeld", "url": "https://a-medewerkers.cluster.example.org"}},
	}, values["koppeling"])
	assert.Equal(t, map[string]any{"accentColor": "rood", "title": "GGD Voorbeeld"}, values["theme"])
	assert.Equal(t, "geneeskundig_advies", values["case"].(map[string]any)["id"])

	// The executor finds it and names it as its peer.
	got := reconcileOnce(t, r, e)
	assert.Equal(t, []v1alpha1.PartyStatus{{
		Cell: "ggd_voorbeeld", Name: "GGD Voorbeeld", URL: "https://ggd.cluster.example.org/",
		Procedure: &v1alpha1.RegelHetReference{Name: "ggd", Namespace: testNamespace},
	}}, got.Status.Parties)
	assert.True(t, apimeta.IsStatusConditionTrue(got.Status.Conditions, v1alpha1.ConditionPartiesLinked))
	assert.Equal(t,
		[]any{map[string]any{"cel": "ggd_voorbeeld", "url": "https://ggd.cluster.example.org"}},
		peersOf(t, fluxObject(t, r.Client, "HelmRelease", "a")))
}

func TestAPartyWaitsForItsExecutorToNameItsParties(t *testing.T) {
	t.Parallel()
	p := asParty(newRegelHet("ggd", "ggd"), "ggd_voorbeeld", newRegelHet("a", "a"))
	r, e, _ := executorWith(t, []string{"ggd_voorbeeld"}, p)
	reconcileOnce(t, r, e)

	result, err := r.Reconcile(context.Background(), ctrl.Request{NamespacedName: client.ObjectKeyFromObject(p)})
	require.NoError(t, err)
	assert.Equal(t, partiesRequeue, result.RequeueAfter)
	var party v1alpha1.RegelHet
	require.NoError(t, r.Client.Get(context.Background(), client.ObjectKeyFromObject(p), &party))
	assert.Equal(t, v1alpha1.PhasePending, party.Status.Phase)
	assert.Contains(t, party.Status.Message, "waits until the executor")
	hr := &unstructured.Unstructured{}
	hr.SetGroupVersionKind(helmReleaseGVK)
	err = r.Client.Get(context.Background(), client.ObjectKey{Namespace: testNamespace, Name: "ggd"}, hr)
	assert.True(t, apierrors.IsNotFound(err), "without a peer the release would run every cell itself: %v", err)
}

// refusedParty reconciles the party p next to the executor "a" whose server
// names cells, and returns it.
func refusedParty(t *testing.T, cells []string, p *v1alpha1.RegelHet, others ...client.Object) (*RegelHetReconciler, *v1alpha1.RegelHet, *v1alpha1.RegelHet) {
	t.Helper()
	r, e, _ := executorWith(t, cells, append([]client.Object{p}, others...)...)
	runs(t, r, e)
	got := reconcileOnce(t, r, p)
	assert.Equal(t, v1alpha1.PhaseFailed, got.Status.Phase)
	return r, e, got
}

func TestAPartyTheCaseDoesNotAskIsRefused(t *testing.T) {
	t.Parallel()
	p := asParty(newRegelHet("rechter", "rechter"), "rechtbank", newRegelHet("a", "a"))
	r, e, got := refusedParty(t, []string{"ggd_voorbeeld"}, p)
	assert.Equal(t, reasonPartyNotAsked, partiesLinked(t, got).Reason)
	assert.Contains(t, got.Status.Message, "asks rechtbank nothing; the parties it asks something of are ggd_voorbeeld")

	// The executor links nothing, and says why.
	exec := reconcileOnce(t, r, e)
	assert.Empty(t, exec.Status.Parties)
	cond := partiesLinked(t, exec)
	assert.Equal(t, metav1.ConditionFalse, cond.Status)
	assert.Contains(t, cond.Message, "not linked: RegelHet "+testNamespace+"/rechter")
}

func TestTheExecutorsOwnCellIsNoParty(t *testing.T) {
	t.Parallel()
	p := asParty(newRegelHet("ggd", "ggd"), "gemeente_voorbeeld", newRegelHet("a", "a"))
	_, _, got := refusedParty(t, []string{"ggd_voorbeeld"}, p)
	assert.Contains(t, got.Status.Message, "cell of the executor")
}

func TestAPartyNeedsAnExecutorThatExists(t *testing.T) {
	t.Parallel()
	p := asParty(newRegelHet("ggd", "ggd"), "ggd_voorbeeld", newRegelHet("weg", "weg"))
	_, _, got := refusedParty(t, nil, p)
	assert.Equal(t, reasonExecutorNotFound, partiesLinked(t, got).Reason)
	assert.Contains(t, got.Status.Message, "does not exist")
}

func TestAPartyLinksAcrossNamespacesOfOneProjectOnly(t *testing.T) {
	t.Parallel()
	same := newRegelHet("ggd", "ggd")
	same.Namespace = orgNamespace
	r, e, _ := executorWith(t, []string{"ggd_voorbeeld"}, asParty(same, "ggd_voorbeeld", newRegelHet("a", "a")))
	runs(t, r, e)
	got := reconcileOnce(t, r, same)
	assert.True(t, apimeta.IsStatusConditionTrue(got.Status.Conditions, v1alpha1.ConditionPartiesLinked))
	exec := reconcileOnce(t, r, e)
	require.Len(t, exec.Status.Parties, 1)
	assert.Equal(t, orgNamespace, exec.Status.Parties[0].Procedure.Namespace)

	other := newRegelHet("ggd", "ggd-elders")
	other.Namespace = otherNamespace
	_, _, refused := refusedParty(t, []string{"ggd_voorbeeld"}, asParty(other, "ggd_voorbeeld", newRegelHet("a", "a")))
	assert.Equal(t, reasonExecutorElsewhere, partiesLinked(t, refused).Reason)
}

func TestOneCellRunsOnOneRegelHet(t *testing.T) {
	t.Parallel()
	first := asParty(newRegelHet("ggd", "ggd"), "ggd_voorbeeld", newRegelHet("a", "a"))
	second := asParty(newRegelHet("ggd2", "ggd2"), "ggd_voorbeeld", newRegelHet("a", "a"))
	second.CreationTimestamp = metav1.NewTime(testNow)
	r, e, _ := executorWith(t, []string{"ggd_voorbeeld"}, first, second)
	runs(t, r, e)
	assert.True(t, apimeta.IsStatusConditionTrue(reconcileOnce(t, r, first).Status.Conditions, v1alpha1.ConditionPartiesLinked))
	late := reconcileOnce(t, r, second)
	assert.Equal(t, reasonPartyTaken, partiesLinked(t, late).Reason)
	assert.Contains(t, late.Status.Message, "RegelHet "+testNamespace+"/ggd runs as ggd_voorbeeld")

	exec := reconcileOnce(t, r, e)
	require.Len(t, exec.Status.Parties, 1)
	assert.Equal(t, "ggd", exec.Status.Parties[0].Procedure.Name)
}

func TestAStableExecutorLinksNoParty(t *testing.T) {
	t.Parallel()
	e := stable(newRegelHet("a", "a"), false)
	p := asParty(newRegelHet("ggd", "ggd"), "ggd_voorbeeld", e)
	r := newReconciler(t, append(testNamespaces(), e, p)...)
	r.Config.ACME = acmeProduction
	got := reconcileOnce(t, r, p)
	assert.Equal(t, reasonExecutorNotUsable, partiesLinked(t, got).Reason)
	assert.Contains(t, got.Status.Message, "stable mode")
}

func TestAPartyCannotBeAnExecutor(t *testing.T) {
	t.Parallel()
	exec := newRegelHet("a", "a")
	middle := asParty(newRegelHet("ggd", "ggd"), "ggd_voorbeeld", exec)
	p := asParty(newRegelHet("lab", "lab"), "lab", middle)
	_, _, got := refusedParty(t, []string{"ggd_voorbeeld"}, p, middle)
	assert.Equal(t, reasonExecutorNotUsable, partiesLinked(t, got).Reason)
	assert.Contains(t, got.Status.Message, "runs as a party itself")
}

func TestASpecWithPartijIsATestServerForStaffOnly(t *testing.T) {
	t.Parallel()
	e := newRegelHet("a", "a")
	cases := map[string]func(w *v1alpha1.RegelHet){
		"test mode only":             func(w *v1alpha1.RegelHet) { stable(w, false) },
		"fsc4b":                      func(w *v1alpha1.RegelHet) { w.Spec.Fsc4b.Enabled = true },
		"organisatie":                func(w *v1alpha1.RegelHet) { w.Spec.Organisatie = &v1alpha1.OrganisatieReference{Name: "o"} },
		"names this RegelHet itself": func(w *v1alpha1.RegelHet) { w.Spec.Partij.Uitvoerder.Name = w.Name },
	}
	for want, change := range cases {
		w := asParty(newRegelHet("ggd", "ggd"), "ggd_voorbeeld", e)
		change(w)
		err := validateSpec(w, "cluster.example.org")
		require.Error(t, err, want)
		assert.Contains(t, err.Error(), want)
	}
	require.NoError(t, validateSpec(asParty(newRegelHet("ggd", "ggd"), "ggd_voorbeeld", e), "cluster.example.org"))
}

func TestAPartyNoLongerAskedStaysLinked(t *testing.T) {
	t.Parallel()
	p := asParty(newRegelHet("ggd", "ggd"), "ggd_voorbeeld", newRegelHet("a", "a"))
	r, e, server := executorWith(t, []string{"ggd_voorbeeld"}, p)
	runs(t, r, e)
	reconcileOnce(t, r, p)
	require.Len(t, reconcileOnce(t, r, e).Status.Parties, 1)

	// The server names the GGD no longer, as when an admin sets the test
	// clock outside the policy's validity: the link stays.
	server.cells = nil
	exec := reconcileOnce(t, r, e)
	assert.Empty(t, exec.Status.AskedParties)
	require.Len(t, exec.Status.Parties, 1)
	assert.Equal(t, "GGD Voorbeeld", exec.Status.Parties[0].Name, "the name stays with the link")
	assert.True(t, apimeta.IsStatusConditionTrue(reconcileOnce(t, r, p).Status.Conditions, v1alpha1.ConditionPartiesLinked))
}

func TestADeletedPartyLeavesThePeersOfItsExecutor(t *testing.T) {
	t.Parallel()
	p := asParty(newRegelHet("ggd", "ggd"), "ggd_voorbeeld", newRegelHet("a", "a"))
	r, e, _ := executorWith(t, []string{"ggd_voorbeeld"}, p)
	runs(t, r, e)
	reconcileOnce(t, r, p)
	reconcileOnce(t, r, e)
	require.Len(t, peersOf(t, fluxObject(t, r.Client, "HelmRelease", "a")), 1)

	require.NoError(t, r.Client.Delete(context.Background(), p))
	got := reconcileOnce(t, r, e)
	assert.Empty(t, got.Status.Parties)
	assert.Empty(t, peersOf(t, fluxObject(t, r.Client, "HelmRelease", "a")))
}

func TestAServerThatDoesNotAnswerKeepsTheLinkAndIsAskedAgain(t *testing.T) {
	t.Parallel()
	p := asParty(newRegelHet("ggd", "ggd"), "ggd_voorbeeld", newRegelHet("a", "a"))
	r, e, server := executorWith(t, []string{"ggd_voorbeeld"}, p)
	runs(t, r, e)
	reconcileOnce(t, r, p)
	reconcileOnce(t, r, e)

	server.err = errors.New("connection refused")
	result, err := r.Reconcile(context.Background(), ctrl.Request{NamespacedName: client.ObjectKeyFromObject(e)})
	require.NoError(t, err)
	assert.Equal(t, partiesRequeue, result.RequeueAfter)
	var got v1alpha1.RegelHet
	require.NoError(t, r.Client.Get(context.Background(), client.ObjectKeyFromObject(e), &got))
	assert.Len(t, got.Status.Parties, 1, "a running link is not torn down because one question failed")
	assert.Equal(t, "gemeente_voorbeeld", got.Status.ExecutorCell)
	cond := partiesLinked(t, &got)
	assert.Equal(t, metav1.ConditionFalse, cond.Status)
	assert.Contains(t, cond.Message, "connection refused")
}

func TestAServerWithoutTheRouteIsNoProblemWithoutParties(t *testing.T) {
	t.Parallel()
	r, e, server := executorWith(t, nil)
	server.err = errors.New("this build of the server does not name its parties")
	got := runs(t, r, e)
	cond := partiesLinked(t, got)
	assert.Equal(t, metav1.ConditionUnknown, cond.Status, "a pinned old build runs fine on its own")
	assert.Equal(t, v1alpha1.PhaseReady, got.Status.Phase)
}

func TestAnAnswerWithoutAnExecutorIsNoAnswer(t *testing.T) {
	t.Parallel()
	r, e, _ := executorWith(t, []string{"ggd_voorbeeld"})
	runs(t, r, e)
	r.Parties = emptyAnswer{}
	got := reconcileOnce(t, r, e)
	assert.Equal(t, "gemeente_voorbeeld", got.Status.ExecutorCell, "what the status held stays")
	assert.False(t, apimeta.IsStatusConditionTrue(got.Status.Conditions, v1alpha1.ConditionPartiesLinked))
}

type emptyAnswer struct{}

func (emptyAnswer) Asked(context.Context, *v1alpha1.RegelHet) (askedParties, error) {
	return askedParties{}, nil
}

// oldPartyRelease is a HelmRelease as plugin versions with
// spec.parties.ownInstances made it for a party of w.
func oldPartyRelease(t *testing.T, r *RegelHetReconciler, w *v1alpha1.RegelHet, name string, controlled bool) {
	t.Helper()
	hr := newObject(helmReleaseGVK, w, name, map[string]any{"releaseName": name})
	labels := hr.GetLabels()
	labels[partyLabel] = "ggd-voorbeeld"
	hr.SetLabels(labels)
	if controlled {
		var current v1alpha1.RegelHet
		require.NoError(t, r.Client.Get(context.Background(), client.ObjectKeyFromObject(w), &current))
		require.NoError(t, controllerutil.SetControllerReference(&current, hr, r.Scheme))
	}
	require.NoError(t, r.Client.Create(context.Background(), hr))
}

func TestTheReleasesOfOwnInstancesGoAfterTheUpgrade(t *testing.T) {
	t.Parallel()
	r, e, _ := executorWith(t, []string{"ggd_voorbeeld"})
	reconcileOnce(t, r, e)
	oldPartyRelease(t, r, e, "a-ggd-voorbeeld", true)
	// One with the label that the plugin did not make for this RegelHet.
	oldPartyRelease(t, r, e, "a-iemand-anders", false)

	reconcileOnce(t, r, e)
	hr := &unstructured.Unstructured{}
	hr.SetGroupVersionKind(helmReleaseGVK)
	err := r.Client.Get(context.Background(), client.ObjectKey{Namespace: testNamespace, Name: "a-ggd-voorbeeld"}, hr)
	assert.True(t, apierrors.IsNotFound(err), "got %v", err)
	fluxObject(t, r.Client, "HelmRelease", "a-iemand-anders")
	fluxObject(t, r.Client, "HelmRelease", "a")
}

func TestAChangeOfAPartyOrAnExecutorReachesTheOther(t *testing.T) {
	t.Parallel()
	e := newRegelHet("a", "a")
	p := asParty(newRegelHet("ggd", "ggd"), "ggd_voorbeeld", e)
	other := newRegelHet("b", "b")
	r := newReconciler(t, e, p, other)
	key := func(w *v1alpha1.RegelHet) reconcile.Request {
		return reconcile.Request{NamespacedName: client.ObjectKeyFromObject(w)}
	}
	assert.Equal(t, []reconcile.Request{key(e)}, r.linkedRegelHets(context.Background(), p))
	assert.Equal(t, []reconcile.Request{key(p)}, r.linkedRegelHets(context.Background(), e))
	assert.Empty(t, r.linkedRegelHets(context.Background(), other))
}

func TestAPartyWithoutAColourOfItsOwnIsToldApartFromItsExecutor(t *testing.T) {
	t.Parallel()
	assert.Equal(t, "rood", partyAccent(""))
	assert.Equal(t, "rood", partyAccent("paars"))
	assert.Equal(t, "robijnrood", partyAccent("rood"))

	e := newRegelHet("a", "a")
	e.Spec.Theme.AccentColor = "rood"
	p := asParty(newRegelHet("ggd", "ggd"), "ggd_voorbeeld", e)
	r, exec, _ := executorWith(t, []string{"ggd_voorbeeld"}, p)
	exec.Spec.Theme.AccentColor = "rood"
	require.NoError(t, r.Client.Update(context.Background(), exec))
	runs(t, r, exec)
	reconcileOnce(t, r, p)
	assert.Equal(t, "robijnrood", releaseValues(t, r.Client, testNamespace, "ggd")["theme"].(map[string]any)["accentColor"])
}

func TestALinkedPartyIsNotTakenOverByAnOlderOne(t *testing.T) {
	t.Parallel()
	// The older one names another executor at first, so the younger links.
	older := asParty(newRegelHet("ggd-oud", "ggd-oud"), "ggd_voorbeeld", newRegelHet("elders", "elders"))
	younger := asParty(newRegelHet("ggd", "ggd"), "ggd_voorbeeld", newRegelHet("a", "a"))
	younger.CreationTimestamp = metav1.NewTime(testNow)
	r, e, _ := executorWith(t, []string{"ggd_voorbeeld"}, older, younger)
	runs(t, r, e)
	reconcileOnce(t, r, younger)
	require.Len(t, reconcileOnce(t, r, e).Status.Parties, 1)

	// Now the older one names a as well: the running link stays.
	var current v1alpha1.RegelHet
	require.NoError(t, r.Client.Get(context.Background(), client.ObjectKeyFromObject(older), &current))
	current.Spec.Partij.Uitvoerder.Name = "a"
	require.NoError(t, r.Client.Update(context.Background(), &current))
	assert.Equal(t, reasonPartyTaken, partiesLinked(t, reconcileOnce(t, r, older)).Reason)
	assert.True(t, apimeta.IsStatusConditionTrue(reconcileOnce(t, r, younger).Status.Conditions, v1alpha1.ConditionPartiesLinked))
	exec := reconcileOnce(t, r, e)
	require.Len(t, exec.Status.Parties, 1)
	assert.Equal(t, "ggd", exec.Status.Parties[0].Procedure.Name)
}

func TestAPartyBehindAnExecutorThatDoesNotAnswerSaysWhy(t *testing.T) {
	t.Parallel()
	p := asParty(newRegelHet("ggd", "ggd"), "ggd_voorbeeld", newRegelHet("a", "a"))
	r, e, server := executorWith(t, nil, p)
	server.err = errors.New("this build of the server does not name its parties")
	runs(t, r, e)
	got := reconcileOnce(t, r, p)
	assert.Equal(t, v1alpha1.PhasePending, got.Status.Phase)
	assert.Contains(t, got.Status.Message, "does not name its parties")
}
