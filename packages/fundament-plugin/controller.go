package main

import (
	"context"
	"fmt"
	"time"

	apierrors "k8s.io/apimachinery/pkg/api/errors"
	corev1 "k8s.io/api/core/v1"
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
	Scheme *runtime.Scheme
	Config pluginConfig
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
		// Gone: the garbage collector removes what it owned.
		return ctrl.Result{}, client.IgnoreNotFound(err)
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
	if err := validateSpec(w, r.Config.Domain); err != nil {
		refuse(w, reasonInvalidSpec, err.Error())
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
	problem, err := releasePodProblem(ctx, r.Client, w)
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
	case reasonReleaseStalled:
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
	}
	return c
}
