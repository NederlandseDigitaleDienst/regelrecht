package main

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

// throughFsc gives w an FSC outway and a grant for each of cells.
func throughFsc(w *v1alpha1.RegelHet, outway string, cells ...string) *v1alpha1.RegelHet {
	w.Spec.Fsc = &v1alpha1.Fsc{Outway: outway}
	for _, cell := range cells {
		w.Spec.Fsc.Grants = append(w.Spec.Fsc.Grants, v1alpha1.FscGrant{Cel: cell, GrantHash: "$1$3$naar-" + cell})
	}
	return w
}

func TestWithAGrantBothSidesReachEachOtherThroughTheirOwnOutway(t *testing.T) {
	t.Parallel()
	p := throughFsc(newRegelHet("ggd", "ggd"), "http://fsc-outway-default.ns-ggd", "gemeente_voorbeeld")
	r, e, _ := executorWith(t, []string{"ggd_voorbeeld"}, asParty(p, "ggd_voorbeeld", newRegelHet("a", "a")))
	throughFsc(e, "http://fsc-outway-default.ns-gemeente/", "ggd_voorbeeld")
	require.NoError(t, r.Client.Update(t.Context(), e))
	runs(t, r, e)

	// The party's release names its own outway and the grant for the
	// executor's cell, not the executor's public address.
	party := reconcileOnce(t, r, p)
	assert.Equal(t, []any{map[string]any{
		"cel": "gemeente_voorbeeld", "url": "http://fsc-outway-default.ns-ggd", "grantHash": "$1$3$naar-gemeente_voorbeeld",
		"voorkant": "https://a-medewerkers.cluster.example.org",
	}}, releaseValues(t, r.Client, testNamespace, "ggd")["koppeling"].(map[string]any)["peers"])
	cond := partiesLinked(t, party)
	assert.Equal(t, metav1.ConditionTrue, cond.Status)
	assert.Contains(t, cond.Message, "through its FSC outway")

	// And so does the executor's, for the party.
	got := reconcileOnce(t, r, e)
	assert.Equal(t, []any{map[string]any{
		"cel": "ggd_voorbeeld", "url": "http://fsc-outway-default.ns-gemeente", "grantHash": "$1$3$naar-ggd_voorbeeld",
		"voorkant": "https://ggd.cluster.example.org",
	}}, peersOf(t, fluxObject(t, r.Client, "HelmRelease", "a")))
	assert.Equal(t, "fsc", got.Status.Parties[0].Via)
	assert.Contains(t, partiesLinked(t, got).Message, "through FSC")
}

func TestACellWithoutAGrantIsReachedOnItsPublicAddress(t *testing.T) {
	t.Parallel()
	// An outway, but a grant for another cell only.
	p := throughFsc(newRegelHet("ggd", "ggd"), "http://fsc-outway-default.ns-ggd", "een_andere_cel")
	r, e, _ := executorWith(t, []string{"ggd_voorbeeld"}, asParty(p, "ggd_voorbeeld", newRegelHet("a", "a")))
	runs(t, r, e)

	party := reconcileOnce(t, r, p)
	assert.Equal(t, []any{map[string]any{"cel": "gemeente_voorbeeld", "url": "https://a-medewerkers.cluster.example.org"}},
		releaseValues(t, r.Client, testNamespace, "ggd")["koppeling"].(map[string]any)["peers"])
	assert.NotContains(t, partiesLinked(t, party).Message, "FSC")
	got := reconcileOnce(t, r, e)
	assert.Equal(t, "direct", got.Status.Parties[0].Via)
}

func TestFscValuesExistInTheChart(t *testing.T) {
	t.Parallel()
	e := throughFsc(newRegelHet("a", "a"), "http://fsc-outway-default.ns", "ggd_voorbeeld")
	peer := linkPeer(e, "ggd_voorbeeld", "https://ggd.cluster.example.org")
	// The chart's schema takes exactly these keys for a peer.
	assert.ElementsMatch(t, []string{"cel", "url", "grantHash", "voorkant"}, keysOf(peer))
}

func keysOf(m map[string]any) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	return keys
}
