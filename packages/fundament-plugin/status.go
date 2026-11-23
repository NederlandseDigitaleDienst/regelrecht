package main

import (
	"context"
	"fmt"
	"sync"

	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"sigs.k8s.io/controller-runtime/pkg/client"

	"github.com/fundament-oss/fundament/plugin-sdk/pluginruntime"
)

// fluxStatus is what the plugin reports: the Ready conditions of the chart
// source and of the HelmRelease.
type fluxStatus struct {
	sourceReady    bool
	sourceMessage  string
	releaseReady   bool
	releaseMessage string
	// releaseStalled is the HelmRelease's Stalled condition: Flux gave up
	// until the spec changes, for instance after the install retries ran out.
	releaseStalled bool
}

// pluginStatus maps the Flux state to a plugin phase. The phase follows the
// HelmRelease: running once the release is ready, degraded once Flux stalled
// on it, installing before that.
func (s fluxStatus) pluginStatus() pluginruntime.PluginStatus {
	msg := fmt.Sprintf("chart source: %s; release: %s", s.sourceMessage, s.releaseMessage)
	switch {
	case s.releaseReady:
		return pluginruntime.PluginStatus{Phase: pluginruntime.PhaseRunning, Message: msg}
	case s.releaseStalled:
		return pluginruntime.PluginStatus{Phase: pluginruntime.PhaseDegraded, Message: msg}
	default:
		return pluginruntime.PluginStatus{Phase: pluginruntime.PhaseInstalling, Message: msg}
	}
}

func readFluxStatus(ctx context.Context, c client.Client, namespace string) (fluxStatus, error) {
	var s fluxStatus

	src := &unstructured.Unstructured{}
	src.SetGroupVersionKind(gitRepositoryGVK)
	if err := c.Get(ctx, client.ObjectKey{Namespace: namespace, Name: sourceName}, src); err != nil {
		return fluxStatus{}, fmt.Errorf("get GitRepository: %w", err)
	}
	s.sourceReady, s.sourceMessage = condition(src, "Ready")

	hr := &unstructured.Unstructured{}
	hr.SetGroupVersionKind(helmReleaseGVK)
	if err := c.Get(ctx, client.ObjectKey{Namespace: namespace, Name: releaseName}, hr); err != nil {
		return fluxStatus{}, fmt.Errorf("get HelmRelease: %w", err)
	}
	s.releaseReady, s.releaseMessage = condition(hr, "Ready")
	s.releaseStalled, _ = condition(hr, "Stalled")
	return s, nil
}

// condition returns whether status.conditions has the type with status True,
// and that condition's message.
func condition(obj *unstructured.Unstructured, conditionType string) (bool, string) {
	conditions, _, _ := unstructured.NestedSlice(obj.Object, "status", "conditions")
	for _, c := range conditions {
		cond, ok := c.(map[string]any)
		if !ok || cond["type"] != conditionType {
			continue
		}
		msg, _ := cond["message"].(string)
		return cond["status"] == "True", msg
	}
	return false, "not reconciled yet"
}

// statusReporter sends a status to the host only when it changes, because the
// host logs every report. After a failed Reconcile, the Flux poll keeps the
// error on display: only the next good Reconcile clears it.
type statusReporter struct {
	mu              sync.Mutex
	last            pluginruntime.PluginStatus
	reported        bool
	reconcileFailed bool
}

// flux reports the Flux status, unless the last Reconcile failed.
func (r *statusReporter) flux(host pluginruntime.Host, s fluxStatus) {
	r.mu.Lock()
	defer r.mu.Unlock()
	if r.reconcileFailed {
		return
	}
	r.send(host, s.pluginStatus())
}

// reconciled reports the result of a Reconcile. err == nil clears an earlier
// failure.
func (r *statusReporter) reconciled(host pluginruntime.Host, status pluginruntime.PluginStatus, err error) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.reconcileFailed = err != nil
	r.send(host, status)
}

// direct reports a status that does not come from Flux or a Reconcile.
func (r *statusReporter) direct(host pluginruntime.Host, status pluginruntime.PluginStatus) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.send(host, status)
}

func (r *statusReporter) send(host pluginruntime.Host, status pluginruntime.PluginStatus) {
	if r.reported && r.last == status {
		return
	}
	r.last, r.reported = status, true
	host.ReportStatus(status)
}
