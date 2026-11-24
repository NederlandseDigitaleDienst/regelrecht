package main

import (
	"context"
	"fmt"
	"strings"
	"time"

	corev1 "k8s.io/api/core/v1"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	apimeta "k8s.io/apimachinery/pkg/api/meta"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/runtime"
	"k8s.io/apimachinery/pkg/runtime/schema"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/controller/controllerutil"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

// retryRequeue is how soon a Wetsproces refused for a hostname another one
// holds is looked at again: that one may be deleted without an event here.
const retryRequeue = 5 * time.Minute

// Reasons in the conditions.
const (
	reasonInvalidSpec    = "InvalidSpec"
	reasonHostnameInUse  = "HostnameInUse"
	reasonReleaseReady   = "ReleaseReady"
	reasonReleasePending = "ReleasePending"
	reasonReleaseStalled = "ReleaseStalled"
	// reasonChartNotInCommit: the commit in spec.version has no chart, for
	// instance one from before charts/wetsproces existed.
	reasonChartNotInCommit = "ChartNotInCommit"
	// reasonStableNeedsProductionCerts: stable mode on an installation with
	// ACME staging, whose certificates the server does not trust.
	reasonStableNeedsProductionCerts = "StableNeedsProductionCerts"
	// reasonNotStarting: a container of the release keeps failing; the
	// message says which and why.
	reasonNotStarting = "NotStarting"
)

// WetsprocesReconciler makes, for each Wetsproces, the Flux objects that run
// it, in the namespace of the object and owned by it. Deleting a Wetsproces
// therefore deletes its HelmRelease, and helm-controller uninstalls the
// release with everything in it, the database and its volume included. It
// writes what it finds back into the status.
type WetsprocesReconciler struct {
	// Client reads the Wetsproces objects and the Flux objects from the
	// manager's cache.
	Client client.Client
	// Secrets reads one Secret past the cache, so the plugin needs get on
	// Secrets and no informer over every Secret of the cluster (list, watch).
	Secrets client.Reader
	Scheme  *runtime.Scheme
	Config  pluginConfig
	// Now is the clock of the start-up grace of a pod; time.Now when nil.
	Now func() time.Time
}

func (r *WetsprocesReconciler) now() time.Time {
	if r.Now != nil {
		return r.Now()
	}
	return time.Now()
}

// SetupWithManager registers the reconciler: it runs for every change of a
// Wetsproces and of a GitRepository or HelmRelease it owns.
func (r *WetsprocesReconciler) SetupWithManager(mgr ctrl.Manager) error {
	owned := func(gvk schema.GroupVersionKind) *unstructured.Unstructured {
		obj := &unstructured.Unstructured{}
		obj.SetGroupVersionKind(gvk)
		return obj
	}
	err := ctrl.NewControllerManagedBy(mgr).
		Named("wetsproces").
		For(&v1alpha1.Wetsproces{}).
		Owns(owned(gitRepositoryGVK)).
		Owns(owned(helmReleaseGVK)).
		// The pods of the release are Helm's, not the plugin's, so they are
		// watched by their labels rather than owned.
		Watches(&corev1.Pod{}, podToWetsproces()).
		Complete(r)
	if err != nil {
		return fmt.Errorf("register the Wetsproces controller: %w", err)
	}
	return nil
}

// Reconcile brings one Wetsproces to its spec.
func (r *WetsprocesReconciler) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	var w v1alpha1.Wetsproces
	if err := r.Client.Get(ctx, req.NamespacedName, &w); err != nil {
		if !apierrors.IsNotFound(err) {
			return ctrl.Result{}, err
		}
		// Gone: the garbage collector removes what it owned, and the
		// plugin the one thing nobody owns, the Keycloak Secret.
		return ctrl.Result{}, r.deleteKeycloakSecret(ctx, req.NamespacedName)
	}
	if !w.DeletionTimestamp.IsZero() {
		return ctrl.Result{}, nil
	}
	before := w.DeepCopy()
	result, err := r.reconcile(ctx, &w)
	w.Status.ObservedGeneration = w.Generation
	if patchErr := r.Client.Status().Patch(ctx, &w, client.MergeFrom(before)); patchErr != nil && !apierrors.IsNotFound(patchErr) {
		return ctrl.Result{}, fmt.Errorf("write the status of %s: %w", req.NamespacedName, patchErr)
	}
	return result, err
}

func (r *WetsprocesReconciler) reconcile(ctx context.Context, w *v1alpha1.Wetsproces) (ctrl.Result, error) {
	h := hostsOf(w, r.Config.Domain)
	w.Status.URL = "https://" + h.burger + "/"
	w.Status.BackofficeURL = "https://" + h.backoffice + "/"
	w.Status.LoginURL, w.Status.KeycloakSecret = "", ""
	if h.login != "" {
		w.Status.LoginURL = "https://" + h.login + "/"
		w.Status.KeycloakSecret = keycloakSecret(w)
	}
	if err := validateSpec(w, r.Config.Domain); err != nil {
		refuse(w, reasonInvalidSpec, err.Error())
		return ctrl.Result{}, nil
	}
	if isStable(w) && r.Config.ACME != acmeProduction {
		// The server reads Keycloak's discovery document at start, over
		// HTTPS, and does not trust a Let's Encrypt staging certificate:
		// it would never start.
		refuse(w, reasonStableNeedsProductionCerts, fmt.Sprintf(
			"stable mode logs in through Keycloak over HTTPS, and this installation's certificates come from Let's Encrypt %s, which the server does not trust; set ACME to production, or use auth.mode test", r.Config.ACME))
		return ctrl.Result{}, nil
	}
	taken, err := r.hostTakenBy(ctx, w)
	if err != nil {
		return ctrl.Result{}, err
	}
	if taken != "" {
		refuse(w, reasonHostnameInUse, fmt.Sprintf("hostname %s gives an address Wetsproces %s has already", w.Spec.Hostname, taken))
		return ctrl.Result{RequeueAfter: retryRequeue}, nil
	}

	for _, obj := range []*unstructured.Unstructured{newGitRepository(w, r.Config), newHelmRelease(w, r.Config)} {
		if err := r.apply(ctx, w, obj); err != nil {
			return ctrl.Result{}, err
		}
	}
	released := r.releaseCondition(ctx, w)
	apimeta.SetStatusCondition(&w.Status.Conditions, released)
	problem, err := releasePodProblem(ctx, r.Client, w, r.now())
	if err != nil {
		return ctrl.Result{}, err
	}
	if problem != nil {
		// Said within seconds, where Helm would only give up after its
		// timeout. The release goes on trying; once the pods run, the next
		// event clears this.
		refuse(w, reasonNotStarting, fmt.Sprintf("%s (%s)", problem.Message, problem.Reason))
		return ctrl.Result{}, nil
	}

	ready := metav1.Condition{Type: v1alpha1.ConditionReady, Status: released.Status, Reason: released.Reason, Message: released.Message}
	apimeta.SetStatusCondition(&w.Status.Conditions, ready)
	w.Status.Message = released.Message
	switch released.Reason {
	case reasonReleaseReady:
		w.Status.Phase = v1alpha1.PhaseReady
	case reasonReleaseStalled, reasonChartNotInCommit:
		w.Status.Phase = v1alpha1.PhaseFailed
	default:
		w.Status.Phase = v1alpha1.PhasePending
	}
	// A change of the HelmRelease's status is an event of an owned object,
	// so nothing needs to be polled.
	return ctrl.Result{}, nil
}

// refuse marks w failed for reason, and makes nothing for it.
func refuse(w *v1alpha1.Wetsproces, reason, message string) {
	w.Status.Phase = v1alpha1.PhaseFailed
	w.Status.Message = message
	apimeta.SetStatusCondition(&w.Status.Conditions, metav1.Condition{
		Type: v1alpha1.ConditionReady, Status: metav1.ConditionFalse, Reason: reason, Message: message,
	})
}

// hostTakenBy names the Wetsproces that has an address of w already, or "".
// Of two with one address the older keeps it, and between two made at the same
// second the first by namespace and name, so the answer does not depend on
// which one is reconciled first.
func (r *WetsprocesReconciler) hostTakenBy(ctx context.Context, w *v1alpha1.Wetsproces) (string, error) {
	var all v1alpha1.WetsprocesList
	if err := r.Client.List(ctx, &all); err != nil {
		return "", fmt.Errorf("list wetsprocessen: %w", err)
	}
	mine := hostsOf(w, r.Config.Domain)
	for i := range all.Items {
		o := &all.Items[i]
		if o.UID == w.UID || !o.DeletionTimestamp.IsZero() || !mine.overlaps(hostsOf(o, r.Config.Domain)) {
			continue
		}
		if comesFirst(o, w) {
			return o.Namespace + "/" + o.Name, nil
		}
	}
	return "", nil
}

func comesFirst(a, b *v1alpha1.Wetsproces) bool {
	if !a.CreationTimestamp.Equal(&b.CreationTimestamp) {
		return a.CreationTimestamp.Before(&b.CreationTimestamp)
	}
	return a.Namespace+"/"+a.Name < b.Namespace+"/"+b.Name
}

// apply server-side applies obj, owned by w.
func (r *WetsprocesReconciler) apply(ctx context.Context, w *v1alpha1.Wetsproces, obj *unstructured.Unstructured) error {
	if err := controllerutil.SetControllerReference(w, obj, r.Scheme); err != nil {
		return fmt.Errorf("own %s %s: %w", obj.GetKind(), obj.GetName(), err)
	}
	//nolint:staticcheck // client.Apply with an unstructured object is the simplest SSA path here.
	if err := r.Client.Patch(ctx, obj, client.Apply, client.FieldOwner(fieldOwner), client.ForceOwnership); err != nil {
		return fmt.Errorf("apply %s %s/%s: %w", obj.GetKind(), obj.GetNamespace(), obj.GetName(), err)
	}
	return nil
}

// releaseCondition is ReleaseReady: the HelmRelease's Ready, with Stalled
// (Flux gave up until the spec changes) as a failure of its own.
func (r *WetsprocesReconciler) releaseCondition(ctx context.Context, w *v1alpha1.Wetsproces) metav1.Condition {
	c := metav1.Condition{Type: v1alpha1.ConditionReleaseReady, Status: metav1.ConditionFalse, Reason: reasonReleasePending}
	hr := &unstructured.Unstructured{}
	hr.SetGroupVersionKind(helmReleaseGVK)
	if err := r.Client.Get(ctx, client.ObjectKey{Namespace: w.Namespace, Name: w.Name}, hr); err != nil {
		c.Message = "the HelmRelease is not readable yet: " + err.Error()
		return c
	}
	ready, msg := condition(hr, "Ready")
	stalled, stalledMsg := condition(hr, "Stalled")
	c.Message = "Flux: " + msg
	switch {
	case ready:
		c.Status, c.Reason = metav1.ConditionTrue, reasonReleaseReady
	case stalled:
		c.Reason, c.Message = reasonReleaseStalled, "Flux gave up: "+stalledMsg
	case w.Spec.Version != "" && strings.Contains(msg, chartMissingMarker):
		// Flux retries this forever without stalling, while the commit
		// will never get a chart.
		c.Reason = reasonChartNotInCommit
		c.Message = fmt.Sprintf("commit %s has no %s, so it cannot run here; choose a later commit or leave version empty (Flux: %s)", w.Spec.Version, chartPath, msg)
	}
	return c
}

// chartMissingMarker is what helm-controller says on a HelmRelease whose
// source has no chart at the path: "HelmChart ... is not ready: invalid
// chart reference: stat .../charts/wetsproces: no such file or directory".
const chartMissingMarker = "invalid chart reference"

// deleteKeycloakSecret deletes the Secret the chart generated for the
// Keycloak of the Wetsproces key, after that Wetsproces is gone. The Job of
// the chart makes it outside Helm, so that a reinstall keeps its passwords,
// and nothing else removes it. Only a Secret with the chart's labels for this
// release is deleted, and only the one that was read (its UID): a Secret of
// the same name that someone made by hand stays.
//
// Best effort, without a finalizer: a Wetsproces with a finalizer that
// outlives the plugin can no longer be deleted. A delete the plugin misses,
// because its pod was down, leaves the Secret behind.
func (r *WetsprocesReconciler) deleteKeycloakSecret(ctx context.Context, key client.ObjectKey) error {
	if r.Secrets == nil {
		return nil
	}
	var secret corev1.Secret
	name := client.ObjectKey{Namespace: key.Namespace, Name: key.Name + "-keycloak"}
	if err := r.Secrets.Get(ctx, name, &secret); err != nil {
		return client.IgnoreNotFound(err)
	}
	l := secret.Labels
	if l[appLabel] != appName || l[instanceLabel] != key.Name || l[componentLabel] != "keycloak" {
		return nil
	}
	err := r.Client.Delete(ctx, &secret, client.Preconditions{UID: &secret.UID})
	if err := client.IgnoreNotFound(err); err != nil {
		return fmt.Errorf("delete the Keycloak Secret %s: %w", name, err)
	}
	return nil
}
