package main

import (
	"context"
	"fmt"
	"strings"
	"time"

	batchv1 "k8s.io/api/batch/v1"
	corev1 "k8s.io/api/core/v1"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	apimeta "k8s.io/apimachinery/pkg/api/meta"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/runtime"
	"k8s.io/apimachinery/pkg/runtime/schema"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/builder"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/controller/controllerutil"
	"sigs.k8s.io/controller-runtime/pkg/handler"
	"sigs.k8s.io/controller-runtime/pkg/log"
	"sigs.k8s.io/controller-runtime/pkg/predicate"
	"sigs.k8s.io/controller-runtime/pkg/reconcile"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

// retryRequeue is how soon a RegelHet refused for a hostname another one
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
	// instance one from before charts/regelhet existed.
	reasonChartNotInCommit = "ChartNotInCommit"
	// reasonStableNeedsProductionCerts: stable mode on an installation with
	// ACME staging, whose certificates the server does not trust.
	reasonStableNeedsProductionCerts = "StableNeedsProductionCerts"
	// reasonNotStarting: a container of the release keeps failing; the
	// message says which and why.
	reasonNotStarting = "NotStarting"
)

// RegelHetReconciler makes, for each RegelHet, the Flux objects that run
// it, in the namespace of the object and owned by it. Deleting a RegelHet
// therefore deletes its HelmRelease, and helm-controller uninstalls the
// release with everything in it, the database and its volume included. It
// writes what it finds back into the status.
type RegelHetReconciler struct {
	// Client reads the RegelHet objects and the Flux objects from the
	// manager's cache.
	Client client.Client
	// Uncached reads past the cache: one Secret by name, so the plugin needs
	// get on Secrets and no informer over every Secret of the cluster, and
	// the ConfigMaps of a snapshot by label, so it needs no watch on those.
	Uncached client.Reader
	// Logs reads the log of the snapshot Job, where it writes the copy of
	// the case (snapshot.go).
	Logs logReader
	// Namespaces reads one Namespace past the cache, for its project label,
	// when a RegelHet names an Organisatie in another namespace.
	Namespaces client.Reader
	Scheme     *runtime.Scheme
	Config     pluginConfig
	// Parties asks a running release which parties its policy asks
	// something of; the Service inside the cluster when nil.
	Parties PartiesReader
	// Now is the clock of the start-up grace of a pod; time.Now when nil.
	Now func() time.Time
}

func (r *RegelHetReconciler) now() time.Time {
	if r.Now != nil {
		return r.Now()
	}
	return time.Now()
}

// SetupWithManager registers the reconciler: it runs for every change of a
// RegelHet and of a GitRepository, HelmRelease or snapshot Job it owns.
func (r *RegelHetReconciler) SetupWithManager(mgr ctrl.Manager) error {
	owned := func(gvk schema.GroupVersionKind) *unstructured.Unstructured {
		obj := &unstructured.Unstructured{}
		obj.SetGroupVersionKind(gvk)
		return obj
	}
	err := ctrl.NewControllerManagedBy(mgr).
		Named("regelhet").
		For(&v1alpha1.RegelHet{}).
		Owns(owned(gitRepositoryGVK)).
		Owns(owned(helmReleaseGVK)).
		Owns(&batchv1.Job{}).
		// The pods of the release are Helm's, not the plugin's, so they are
		// watched by their labels rather than owned.
		Watches(&corev1.Pod{}, podToRegelHet()).
		// A RegelHet that names an organisatie joins it once it appears,
		// and runs on its own again once it goes.
		// Only on a change of its spec: its status changes with every phase
		// of every member, and resolveOrganisatie reads the spec alone.
		Watches(&v1alpha1.Organisatie{}, handler.EnqueueRequestsFromMapFunc(r.namingRegelHets),
			builder.WithPredicates(predicate.GenerationChangedPredicate{})).
		// A party and its executor look at each other: the executor links
		// the RegelHet instances that name it, and a party links once its
		// executor named the parties of its case (parties.go).
		Watches(&v1alpha1.RegelHet{}, handler.EnqueueRequestsFromMapFunc(r.linkedRegelHets)).
		Complete(r)
	if err != nil {
		return fmt.Errorf("register the RegelHet controller: %w", err)
	}
	return nil
}

// namingRegelHets are the RegelHet instances that name the Organisatie obj.
func (r *RegelHetReconciler) namingRegelHets(ctx context.Context, obj client.Object) []reconcile.Request {
	var all v1alpha1.RegelHetList
	if err := r.Client.List(ctx, &all); err != nil {
		log.FromContext(ctx).Error(err, "list the regelhets that name an organisatie", "organisatie", client.ObjectKeyFromObject(obj))
		return nil
	}
	var out []reconcile.Request
	for i := range all.Items {
		w := &all.Items[i]
		if w.Spec.Organisatie != nil && organisatieKey(w) == client.ObjectKeyFromObject(obj) {
			out = append(out, reconcile.Request{NamespacedName: client.ObjectKeyFromObject(w)})
		}
	}
	return out
}

// Reconcile brings one RegelHet to its spec.
func (r *RegelHetReconciler) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	var w v1alpha1.RegelHet
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

func (r *RegelHetReconciler) reconcile(ctx context.Context, w *v1alpha1.RegelHet) (ctrl.Result, error) {
	h := hostsOf(w, r.Config.Domain)
	w.Status.URL = "https://" + h.burger + "/"
	w.Status.BackofficeURL = "https://" + h.backoffice + "/"
	if isParty(w) {
		// A party has one instance, for its staff, on its hostname itself,
		// and no citizens' side.
		w.Status.URL = ""
		w.Status.BackofficeURL = "https://" + partyHost(w, r.Config.Domain) + "/"
	}
	w.Status.LoginURL, w.Status.KeycloakSecret = "", ""
	w.Status.Fsc4bDiscoveryURL = ""
	if h.login != "" {
		w.Status.LoginURL = "https://" + h.login + "/"
		w.Status.KeycloakSecret = keycloakSecret(w)
	}
	reason, message, err := regelhetRefusal(ctx, r.Client, r.Config, w)
	if err != nil {
		return ctrl.Result{}, err
	}
	if reason != "" {
		// A refused RegelHet is on no organisatie's list either.
		apimeta.RemoveStatusCondition(&w.Status.Conditions, v1alpha1.ConditionOrganisatieFound)
		refuse(w, reason, message)
		if reason == reasonHostnameInUse || reason == reasonNameInUse {
			// The other one may be deleted without an event here.
			return ctrl.Result{RequeueAfter: retryRequeue}, nil
		}
		return ctrl.Result{}, nil
	}
	// Only for a spec the plugin accepts: a refused one serves no discovery.
	if w.Spec.Fsc4b.Enabled {
		w.Status.Fsc4bDiscoveryURL = "https://" + h.burger + fsc4bDiscoveryPath
	}

	var organisatie *v1alpha1.Organisatie
	if w.Spec.Organisatie == nil {
		apimeta.RemoveStatusCondition(&w.Status.Conditions, v1alpha1.ConditionOrganisatieFound)
	} else {
		m, err := resolveOrganisatie(ctx, r.Client, r.Namespaces, r.Config.Domain, w)
		if err != nil {
			return ctrl.Result{}, err
		}
		apimeta.SetStatusCondition(&w.Status.Conditions, m.condition)
		if m.refused {
			refuse(w, m.condition.Reason, m.condition.Message)
			return ctrl.Result{}, nil
		}
		organisatie = m.organisatie
	}
	var executor *v1alpha1.RegelHet
	if isParty(w) {
		link, err := resolveExecutor(ctx, r.Client, r.Namespaces, r.Config, w)
		if err != nil {
			return ctrl.Result{}, err
		}
		apimeta.SetStatusCondition(&w.Status.Conditions, link.condition)
		if link.pending() {
			// Without the executor's cell there is no peer, and a release
			// with no peer would run every cell of the case on its own.
			c := link.condition
			apimeta.SetStatusCondition(&w.Status.Conditions, metav1.Condition{
				Type: v1alpha1.ConditionReady, Status: metav1.ConditionFalse, Reason: c.Reason, Message: c.Message,
			})
			w.Status.Phase, w.Status.Message = v1alpha1.PhasePending, c.Message
			return ctrl.Result{RequeueAfter: partiesRequeue}, nil
		}
		if link.executor == nil {
			refuse(w, link.condition.Reason, link.condition.Message)
			return ctrl.Result{RequeueAfter: retryRequeue}, nil
		}
		executor = link.executor
	}
	token, err := r.tokenState(ctx, w)
	if err != nil {
		return ctrl.Result{}, err
	}
	w.Status.Token = token
	snap, err := r.reconcileSnapshot(ctx, w)
	if err != nil {
		return ctrl.Result{}, err
	}
	apimeta.SetStatusCondition(&w.Status.Conditions, snap.condition)
	// The release runs the snapshot of the spec as it is now, or, while a
	// new one is taken or fails, the one it ran before.
	running := snap.current
	if running == nil {
		if running, err = r.runningSnapshot(ctx, w); err != nil {
			return ctrl.Result{}, err
		}
	}
	if running == nil {
		// Nothing to install yet: the first snapshot is on its way, or failed.
		c := snap.condition
		apimeta.SetStatusCondition(&w.Status.Conditions, metav1.Condition{
			Type: v1alpha1.ConditionReady, Status: metav1.ConditionFalse, Reason: c.Reason, Message: c.Message,
		})
		w.Status.Message = c.Message
		w.Status.Phase = v1alpha1.PhasePending
		if c.Reason != reasonSnapshotPending && c.Reason != reasonTokenMissing {
			w.Status.Phase = v1alpha1.PhaseFailed
		}
		return ctrl.Result{RequeueAfter: snap.requeue}, nil
	}
	w.Status.Snapshot = &v1alpha1.SnapshotStatus{Commit: running.commit, ConfigMaps: running.configMaps}

	// The values follow the status too (the peers of the parties), so the
	// release is built again after the parties changed them.
	release := func() *unstructured.Unstructured {
		values := chartValues(w, r.Config, running)
		if organisatie != nil {
			values["organisatie"] = organisatieValues(organisatie, r.Config)
		}
		if executor != nil {
			values = partyValues(values, w, executor, r.Config)
		}
		return newHelmRelease(w, values)
	}

	objects := []*unstructured.Unstructured{newGitRepository(w, w.Spec.Version, r.Config), release()}
	// The executor's server names the parties its case asks something of,
	// and the RegelHet instances that run as one of them are linked. When
	// that changes them, the executor's release is applied again with its
	// new peers. Only a server that did not name its parties yet is asked
	// again soon.
	parties := func(released metav1.Condition) (time.Duration, error) {
		peersChanged, requeue, err := r.reconcileParties(ctx, w, released.Status == metav1.ConditionTrue)
		if err != nil {
			return 0, err
		}
		if peersChanged {
			if err := r.apply(ctx, w, release()); err != nil {
				return 0, err
			}
		}
		// The old snapshot goes once the release of w runs the new one.
		if snap.current != nil && r.releaseRuns(ctx, w.Namespace, w.Name, snap.current) {
			if err := r.deleteOldSnapshots(ctx, w, snap.current); err != nil {
				return 0, err
			}
		}
		return minRequeue(requeue, snap.requeue), nil
	}
	result, err := runRelease(ctx, releaseDeps{Client: r.Client, Scheme: r.Scheme, Now: r.now()}, w, w.Spec.Version, objects, regelhetStatus(w), parties)
	if err == nil && snap.condition.Status != metav1.ConditionTrue && snap.condition.Reason != reasonSnapshotPending {
		// A new snapshot failed; the release goes on with the old one.
		refuse(w, snap.condition.Reason, fmt.Sprintf("%s; the regelhet still runs the copy of commit %s", snap.condition.Message, running.commit))
	}
	return result, err
}

// releaseRuns reports whether the HelmRelease name is ready on the values
// that name s: only then is an older snapshot no longer needed.
func (r *RegelHetReconciler) releaseRuns(ctx context.Context, namespace, name string, s *snapshot) bool {
	hr := &unstructured.Unstructured{}
	hr.SetGroupVersionKind(helmReleaseGVK)
	if err := r.Client.Get(ctx, client.ObjectKey{Namespace: namespace, Name: name}, hr); err != nil {
		return false
	}
	names, _, _ := unstructured.NestedSlice(hr.Object, "spec", "values", "case", "snapshot", "configMaps")
	if len(names) != len(s.configMaps) {
		return false
	}
	for i, n := range names {
		if n != s.configMaps[i] {
			return false
		}
	}
	observed, _, _ := unstructured.NestedInt64(hr.Object, "status", "observedGeneration")
	ready, _ := condition(hr, "Ready")
	return ready && observed == hr.GetGeneration()
}

// apply server-side applies obj, owned by w.
func (r *RegelHetReconciler) apply(ctx context.Context, w *v1alpha1.RegelHet, obj *unstructured.Unstructured) error {
	return applyOwned(ctx, r.Client, r.Scheme, w, obj)
}

// regelhetRefusal is why the plugin refuses w before it makes anything for
// it: a spec it cannot run, stable mode on staging certificates, or an
// address or a name another object has. Empty when it accepts w. An
// organisatie lists no RegelHet the plugin refuses.
func regelhetRefusal(ctx context.Context, c client.Reader, cfg pluginConfig, w *v1alpha1.RegelHet) (reason, message string, err error) {
	if err := validateSpec(w, cfg.Domain); err != nil {
		return reasonInvalidSpec, err.Error(), nil
	}
	if isStable(w) && cfg.ACME != acmeProduction {
		// The server reads Keycloak's discovery document at start, over
		// HTTPS, and does not trust a Let's Encrypt staging certificate:
		// it would never start.
		return reasonStableNeedsProductionCerts, fmt.Sprintf(
			"stable mode logs in through Keycloak over HTTPS, and this installation's certificates come from Let's Encrypt %s, which the server does not trust; set ACME to production, or use auth.mode test", cfg.ACME), nil
	}
	return addressTakenBy(ctx, c, cfg.Domain, w, regelhetAddresses(w, cfg.Domain))
}

// regelhetStatus points at the status of w.
func regelhetStatus(w *v1alpha1.RegelHet) status {
	return statusOf(&w.Status.Phase, &w.Status.Message, &w.Status.Conditions)
}

// refuse marks w failed for reason, and makes nothing for it.
func refuse(w *v1alpha1.RegelHet, reason, message string) {
	regelhetStatus(w).refuse(reason, message)
}

// applyOwned server-side applies obj, owned by owner.
func applyOwned(ctx context.Context, c client.Client, scheme *runtime.Scheme, owner client.Object, obj *unstructured.Unstructured) error {
	if err := controllerutil.SetControllerReference(owner, obj, scheme); err != nil {
		return fmt.Errorf("own %s %s: %w", obj.GetKind(), obj.GetName(), err)
	}
	//nolint:staticcheck // client.Apply with an unstructured object is the simplest SSA path here.
	if err := c.Patch(ctx, obj, client.Apply, client.FieldOwner(fieldOwner), client.ForceOwnership); err != nil {
		return fmt.Errorf("apply %s %s/%s: %w", obj.GetKind(), obj.GetNamespace(), obj.GetName(), err)
	}
	return nil
}

// releaseCondition is ReleaseReady: the HelmRelease's Ready, with Stalled
// (Flux gave up until the spec changes) as a failure of its own.
// owner is a RegelHet or an Organisatie, and version its spec.version.
func releaseCondition(ctx context.Context, reader client.Reader, owner client.Object, version string) metav1.Condition {
	c := metav1.Condition{Type: v1alpha1.ConditionReleaseReady, Status: metav1.ConditionFalse, Reason: reasonReleasePending}
	hr := &unstructured.Unstructured{}
	hr.SetGroupVersionKind(helmReleaseGVK)
	if err := reader.Get(ctx, client.ObjectKeyFromObject(owner), hr); err != nil {
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
	case version != "" && strings.Contains(msg, chartMissingMarker):
		// Flux retries this forever without stalling, while the commit
		// will never get a chart.
		c.Reason = reasonChartNotInCommit
		c.Message = fmt.Sprintf("commit %s has no %s, so it cannot run here; choose a later commit or leave version empty (Flux: %s)", version, chartPath, msg)
	}
	return c
}

// chartMissingMarker is what helm-controller says on a HelmRelease whose
// source has no chart at the path: "HelmChart ... is not ready: invalid
// chart reference: stat .../charts/regelhet: no such file or directory".
const chartMissingMarker = "invalid chart reference"

// deleteKeycloakSecret deletes the Secret the chart generated for the
// Keycloak of the RegelHet key, after that RegelHet is gone. The Job of
// the chart makes it outside Helm, so that a reinstall keeps its passwords,
// and nothing else removes it. Only a Secret with the chart's labels for this
// release is deleted, and only the one that was read (its UID): a Secret of
// the same name that someone made by hand stays.
//
// Best effort, without a finalizer: a RegelHet with a finalizer that
// outlives the plugin can no longer be deleted. A delete the plugin misses,
// because its pod was down, leaves the Secret behind.
func (r *RegelHetReconciler) deleteKeycloakSecret(ctx context.Context, key client.ObjectKey) error {
	if r.Uncached == nil {
		return nil
	}
	// The pods of an Organisatie of this name map here too; its release is
	// not ours to clean up after.
	var o v1alpha1.Organisatie
	if err := r.Client.Get(ctx, key, &o); err == nil {
		return nil
	} else if !apierrors.IsNotFound(err) {
		return fmt.Errorf("read Organisatie %s: %w", key, err)
	}
	return deleteChartSecret(ctx, r.Uncached, r.Client, key, "keycloak")
}

// deleteChartSecret deletes the Secret <release>-<component> that the Job of
// the chart generated for the release key, once its object is gone. Only a
// Secret with the chart's labels for this release and component is deleted,
// and only the one that was read (its UID).
func deleteChartSecret(ctx context.Context, secrets client.Reader, c client.Client, key client.ObjectKey, component string) error {
	var secret corev1.Secret
	name := client.ObjectKey{Namespace: key.Namespace, Name: key.Name + "-" + component}
	if err := secrets.Get(ctx, name, &secret); err != nil {
		return client.IgnoreNotFound(err)
	}
	l := secret.Labels
	if (l[appLabel] != secretAppName && l[appLabel] != appName) || l[instanceLabel] != key.Name || l[componentLabel] != component {
		return nil
	}
	err := c.Delete(ctx, &secret, client.Preconditions{UID: &secret.UID})
	if err := client.IgnoreNotFound(err); err != nil {
		return fmt.Errorf("delete the Secret %s: %w", name, err)
	}
	return nil
}

// shareAnAddress reports whether a and b have an address in common.
func shareAnAddress(a, b []string) bool {
	for _, x := range a {
		for _, y := range b {
			if x == y {
				return true
			}
		}
	}
	return false
}
