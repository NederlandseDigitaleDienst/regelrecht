package main

import (
	"context"
	"fmt"

	corev1 "k8s.io/api/core/v1"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/runtime/schema"
	"sigs.k8s.io/controller-runtime/pkg/client"

	pluginerrors "github.com/fundament-oss/fundament/plugin-sdk/pluginruntime/errors"
)

const (
	pluginName = "wetsproces"
	fieldOwner = "wetsproces-plugin"

	// sourceName is the GitRepository with the chart; releaseName is the
	// HelmRelease and the Helm release it installs.
	sourceName  = "wetsproces-chart"
	releaseName = "wetsproces"
	chartPath   = "charts/wetsproces"

	fluxInterval = "10m"
)

var (
	gitRepositoryGVK = schema.GroupVersionKind{Group: "source.toolkit.fluxcd.io", Version: "v1", Kind: "GitRepository"}
	helmReleaseGVK   = schema.GroupVersionKind{Group: "helm.toolkit.fluxcd.io", Version: "v2", Kind: "HelmRelease"}

	managedByLabels = map[string]any{"app.kubernetes.io/managed-by": pluginName}
)

// requireNamespace checks that the configured namespace exists. Fundament
// creates namespaces, so that project members get their rights in it; the
// plugin only waits for one. A missing namespace is transient: it may be
// created after the install.
func requireNamespace(ctx context.Context, c client.Client, name string) error {
	var ns corev1.Namespace
	err := c.Get(ctx, client.ObjectKey{Name: name}, &ns)
	switch {
	case apierrors.IsNotFound(err):
		return pluginerrors.NewTransient(fmt.Errorf("namespace %s does not exist: create it in Fundament (console or functl) first", name))
	case err != nil:
		return pluginerrors.NewTransient(fmt.Errorf("get namespace %s: %w", name, err))
	}
	return nil
}

// newGitRepository builds the source of the chart: the fork at ChartRef. The
// ignore rules keep only the chart, so the artifact stays small in a large
// repository.
func newGitRepository(cfg pluginConfig) *unstructured.Unstructured {
	obj := &unstructured.Unstructured{Object: map[string]any{
		"metadata": map[string]any{
			"name":      sourceName,
			"namespace": cfg.Namespace,
			"labels":    managedByLabels,
		},
		"spec": map[string]any{
			"url":      forkURL,
			"ref":      map[string]any{"branch": cfg.ChartRef},
			"interval": fluxInterval,
			"ignore":   "/*\n!/" + chartPath + "/\n",
		},
	}}
	obj.SetGroupVersionKind(gitRepositoryGVK)
	return obj
}

// chartValues maps the plugin config onto the values of charts/wetsproces.
func chartValues(cfg pluginConfig) map[string]any {
	return map[string]any{
		"domain": cfg.Domain,
		"image":  map[string]any{"ref": cfg.Image},
		"case": map[string]any{
			"repoUrl": cfg.CaseRepoURL,
			"ref":     cfg.CaseRef,
			"path":    cfg.CasePath,
		},
		"gateway": map[string]any{"address": cfg.GatewayAddress},
		"tls":     map[string]any{"acme": cfg.ACME},
	}
}

// newHelmRelease builds the HelmRelease that installs the chart with the
// config as values. reconcileStrategy Revision rebuilds the chart on every new
// commit of the source, because the chart version does not change per commit.
func newHelmRelease(cfg pluginConfig) *unstructured.Unstructured {
	obj := &unstructured.Unstructured{Object: map[string]any{
		"metadata": map[string]any{
			"name":      releaseName,
			"namespace": cfg.Namespace,
			"labels":    managedByLabels,
		},
		"spec": map[string]any{
			"interval":    fluxInterval,
			"releaseName": releaseName,
			"chart": map[string]any{
				"spec": map[string]any{
					"chart":             "./" + chartPath,
					"reconcileStrategy": "Revision",
					"sourceRef": map[string]any{
						"kind": "GitRepository",
						"name": sourceName,
					},
				},
			},
			"install": map[string]any{"remediation": map[string]any{"retries": int64(3)}},
			"upgrade": map[string]any{"remediation": map[string]any{"retries": int64(3)}},
			"values":  chartValues(cfg),
		},
	}}
	obj.SetGroupVersionKind(helmReleaseGVK)
	return obj
}

// applyFluxObjects server-side applies the GitRepository and the HelmRelease.
// Applying on every start and reconcile puts back anything changed by hand.
func applyFluxObjects(ctx context.Context, c client.Client, cfg pluginConfig) error {
	for _, obj := range []*unstructured.Unstructured{newGitRepository(cfg), newHelmRelease(cfg)} {
		//nolint:staticcheck // client.Apply with an unstructured object is the simplest SSA path here.
		if err := c.Patch(ctx, obj, client.Apply, client.FieldOwner(fieldOwner), client.ForceOwnership); err != nil {
			return pluginerrors.NewTransient(fmt.Errorf("apply %s %s: %w", obj.GetKind(), obj.GetName(), err))
		}
	}
	return nil
}

// deleteObject deletes one object by kind and name; a missing object is no error.
func deleteObject(ctx context.Context, c client.Client, gvk schema.GroupVersionKind, namespace, name string) error {
	obj := &unstructured.Unstructured{}
	obj.SetGroupVersionKind(gvk)
	obj.SetNamespace(namespace)
	obj.SetName(name)
	if err := client.IgnoreNotFound(c.Delete(ctx, obj)); err != nil {
		return pluginerrors.NewTransient(fmt.Errorf("delete %s %s: %w", gvk.Kind, name, err))
	}
	return nil
}

// objectGone reports whether the object no longer exists.
func objectGone(ctx context.Context, c client.Client, gvk schema.GroupVersionKind, namespace, name string) (bool, error) {
	obj := &unstructured.Unstructured{}
	obj.SetGroupVersionKind(gvk)
	err := c.Get(ctx, client.ObjectKey{Namespace: namespace, Name: name}, obj)
	if apierrors.IsNotFound(err) {
		return true, nil
	}
	return false, err
}
