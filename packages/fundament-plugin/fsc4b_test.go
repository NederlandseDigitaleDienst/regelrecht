package main

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

func TestFsc4bIsOffUnlessTheSpecTurnsItOn(t *testing.T) {
	t.Parallel()
	w := newRegelHet("a", "a")
	assert.NotContains(t, chartValues(w, validConfig(), testSnapshot), "fsc4b", "an existing RegelHet keeps the chart's default")

	w.Spec.Fsc4b.Enabled = true
	assert.Equal(t, map[string]any{"enabled": true}, chartValues(w, validConfig(), testSnapshot)["fsc4b"])
}

func TestFsc4bIsRefusedInStableMode(t *testing.T) {
	t.Parallel()
	w := stable(newRegelHet("a", "a"), false)
	w.Spec.Fsc4b.Enabled = true
	err := validateSpec(w, "cluster.example.org")
	require.Error(t, err)
	assert.Contains(t, err.Error(), "test mode only")
}

func TestStatusNamesTheFsc4bDiscoveryWhenOn(t *testing.T) {
	t.Parallel()
	off := newRegelHet("a", "a")
	on := newRegelHet("b", "b")
	on.Spec.Fsc4b.Enabled = true
	refused := stable(newRegelHet("c", "c"), false)
	refused.Spec.Fsc4b.Enabled = true
	r := newReconciler(t, off, on, refused)

	assert.Empty(t, reconcileOnce(t, r, off).Status.Fsc4bDiscoveryURL)

	got := reconcileOnce(t, r, on)
	assert.Equal(t, "https://b.cluster.example.org/fsc4b/discovery", got.Status.Fsc4bDiscoveryURL)
	hr := fluxObject(t, r.Client, "HelmRelease", "b")
	enabled, _, _ := unstructured.NestedBool(hr.Object, "spec", "values", "fsc4b", "enabled")
	assert.True(t, enabled)

	got = reconcileOnce(t, r, refused)
	assert.Equal(t, v1alpha1.PhaseFailed, got.Status.Phase)
	assert.Contains(t, got.Status.Message, "test mode only")
	assert.Empty(t, got.Status.Fsc4bDiscoveryURL, "a refused spec names no discovery")
}
