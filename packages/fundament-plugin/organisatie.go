package main

import (
	"context"
	"fmt"
	"sort"
	"time"

	corev1 "k8s.io/api/core/v1"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	apimeta "k8s.io/apimachinery/pkg/api/meta"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/runtime"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/handler"
	"sigs.k8s.io/controller-runtime/pkg/reconcile"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

// projectLabel is the label Fundament puts on the namespaces of a project. A
// RegelHet may join an Organisatie in another namespace only when both
// namespaces carry the same value: without the check, any project could put
// its RegelHet behind the front of another.
const projectLabel = "fundament.io/project-id"

// Reasons of the condition OrganisatieGevonden, and of a refusal.
const (
	reasonOrganisatieFound     = "OrganisatieFound"
	reasonOrganisatieNotFound  = "OrganisatieNotFound"
	reasonOrganisatieElsewhere = "OrganisatieInOtherProject"
	// reasonOrganisatieRefused: the plugin refuses the organisatie itself,
	// for its spec, its address or its name.
	reasonOrganisatieRefused = "OrganisatieRefused"
	// reasonOrganisatieModeMismatch: a stable RegelHet behind a front in
	// test mode, whose test login would let anyone in as anyone. Failed.
	reasonOrganisatieModeMismatch = "OrganisatieModeMismatch"
	// reasonNameInUse: a RegelHet and an Organisatie of one name in one
	// namespace would share their Helm release.
	reasonNameInUse = "NameInUse"
)

// organisatieSecretComponent is the component label, and the suffix of the
// name, of the Secret with the signing key of a front
// (charts/regelhet/templates/secrets-job.yaml).
const organisatieSecretComponent = "organisatie"

// sleutelPath is where the front serves its public key.
const sleutelPath = "/organisatie/v1/sleutel"

// OrganisatieReconciler makes, for each Organisatie, the Flux objects that run
// its front, like RegelHetReconciler does for a RegelHet, with the
// RegelHet instances that name it in the values of the release.
type OrganisatieReconciler struct {
	// Client reads the Organisatie and RegelHet objects and the Flux objects
	// from the manager's cache.
	Client client.Client
	// Namespaces reads one Namespace past the cache, for its project label:
	// get, without list or watch.
	Namespaces client.Reader
	// Secrets reads one Secret past the cache: the signing key of a front
	// that is gone.
	Secrets client.Reader
	Scheme  *runtime.Scheme
	Config  pluginConfig
	// Now is the clock of the start-up grace of a pod; time.Now when nil.
	Now func() time.Time
}

func (r *OrganisatieReconciler) now() time.Time {
	if r.Now != nil {
		return r.Now()
	}
	return time.Now()
}

// SetupWithManager registers the reconciler: it runs for every change of an
// Organisatie, of what it owns, of its pods, and of a RegelHet that names it
// now or named it before the change.
func (r *OrganisatieReconciler) SetupWithManager(mgr ctrl.Manager) error {
	gitRepository := &unstructured.Unstructured{}
	gitRepository.SetGroupVersionKind(gitRepositoryGVK)
	helmRelease := &unstructured.Unstructured{}
	helmRelease.SetGroupVersionKind(helmReleaseGVK)
	err := ctrl.NewControllerManagedBy(mgr).
		Named("organisatie").
		For(&v1alpha1.Organisatie{}).
		Owns(gitRepository).
		Owns(helmRelease).
		// An update maps both the old and the new object, so a RegelHet
		// that moves to another organisatie leaves the first list too.
		Watches(&v1alpha1.RegelHet{}, handler.EnqueueRequestsFromMapFunc(
			func(_ context.Context, obj client.Object) []reconcile.Request {
				w, ok := obj.(*v1alpha1.RegelHet)
				if !ok || w.Spec.Organisatie == nil {
					return nil
				}
				return []reconcile.Request{{NamespacedName: organisatieKey(w)}}
			})).
		Watches(&corev1.Pod{}, podToRegelHet()).
		Complete(r)
	if err != nil {
		return fmt.Errorf("register the Organisatie controller: %w", err)
	}
	return nil
}

// Reconcile brings one Organisatie to its spec. For a deleted one the garbage
// collector removes what it owned, and the RegelHet controller watches
// organisaties, so its members drop it. The plugin deletes the one thing
// nobody owns: the Secret <name>-organisatie with the signing key, which the
// Job of the chart made outside Helm. Without that, an organisatie made again
// under the same name would sign with the old key. Best effort, like the
// Keycloak Secret of a RegelHet.
func (r *OrganisatieReconciler) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	var o v1alpha1.Organisatie
	if err := r.Client.Get(ctx, req.NamespacedName, &o); err != nil {
		if !apierrors.IsNotFound(err) || r.Secrets == nil {
			return ctrl.Result{}, client.IgnoreNotFound(err)
		}
		return ctrl.Result{}, deleteChartSecret(ctx, r.Secrets, r.Client, req.NamespacedName, organisatieSecretComponent)
	}
	if !o.DeletionTimestamp.IsZero() {
		return ctrl.Result{}, nil
	}
	before := o.DeepCopy()
	result, err := r.reconcile(ctx, &o)
	o.Status.ObservedGeneration = o.Generation
	if patchErr := r.Client.Status().Patch(ctx, &o, client.MergeFrom(before)); patchErr != nil && !apierrors.IsNotFound(patchErr) {
		return ctrl.Result{}, fmt.Errorf("write the status of %s: %w", req.NamespacedName, patchErr)
	}
	return result, err
}

func (r *OrganisatieReconciler) reconcile(ctx context.Context, o *v1alpha1.Organisatie) (ctrl.Result, error) {
	h := organisatieHosts(o, r.Config.Domain)
	o.Status.URL = "https://" + h.burger + "/"
	o.Status.BackofficeURL = "https://" + h.backoffice + "/"
	o.Status.LoginURL = ""
	s := statusOf(&o.Status.Phase, &o.Status.Message, &o.Status.Conditions)

	members, err := organisatieMembers(ctx, r.Client, r.Namespaces, r.Config, o)
	if err != nil {
		return ctrl.Result{}, err
	}
	o.Status.Processen = make([]v1alpha1.ProcessReference, 0, len(members))
	for _, w := range members {
		o.Status.Processen = append(o.Status.Processen, v1alpha1.ProcessReference{Namespace: w.Namespace, Name: w.Name, Phase: w.Status.Phase})
	}

	reason, message, err := organisatieRefusal(ctx, r.Client, r.Config.Domain, o)
	if err != nil {
		return ctrl.Result{}, err
	}
	if reason != "" {
		s.refuse(reason, message)
		if reason == reasonInvalidSpec {
			return ctrl.Result{}, nil
		}
		return ctrl.Result{RequeueAfter: retryRequeue}, nil
	}

	objects := []*unstructured.Unstructured{
		newGitRepository(o, o.Spec.Version, r.Config),
		newHelmRelease(o, voorkantValues(o, members, r.Config)),
	}
	return runRelease(ctx, releaseDeps{Client: r.Client, Scheme: r.Scheme, Now: r.now()}, o, o.Spec.Version, objects, s, nil)
}

// organisatieRefusal is why the plugin refuses o, or "": a spec it cannot
// run, or an address or a name another object has. No RegelHet joins a
// refused organisatie.
func organisatieRefusal(ctx context.Context, c client.Reader, domain string, o *v1alpha1.Organisatie) (reason, message string, err error) {
	if err := validateOrganisatie(o, domain); err != nil {
		return reasonInvalidSpec, err.Error(), nil
	}
	return addressTakenBy(ctx, c, domain, o, organisatieHosts(o, domain).all())
}

// validateOrganisatie checks what the CRD's schema cannot.
func validateOrganisatie(o *v1alpha1.Organisatie, domain string) error {
	if o.Spec.Logo != "" {
		if err := validateHTTPS("logo", o.Spec.Logo); err != nil {
			return err
		}
	}
	if o.Spec.Auth.Mode != "" && o.Spec.Auth.Mode != v1alpha1.AuthTest {
		return fmt.Errorf("auth.mode %q: an organisatie runs in test mode only, for now", o.Spec.Auth.Mode)
	}
	return validateHosts(o.Spec.Hostname, organisatieHosts(o, domain))
}

// organisatieHosts are the addresses of the front: as for a RegelHet, and in
// test mode without a login host.
func organisatieHosts(o *v1alpha1.Organisatie, domain string) hosts {
	return hostsFor(o.Spec.Hostname, false, domain)
}

// organisatieMode is the auth mode of o, test when empty.
func organisatieMode(o *v1alpha1.Organisatie) string {
	if o.Spec.Auth.Mode == "" {
		return v1alpha1.AuthTest
	}
	return o.Spec.Auth.Mode
}

// regelhetMode is the auth mode of w, test when empty.
func regelhetMode(w *v1alpha1.RegelHet) string {
	if isStable(w) {
		return v1alpha1.AuthStable
	}
	return v1alpha1.AuthTest
}

// objectID is <namespace>/<name>, the id of a front or a process in the values
// and in what the front signs.
func objectID(obj client.Object) string { return obj.GetNamespace() + "/" + obj.GetName() }

// internalURL is the address of one side of a release on the cluster network:
// the Service <release>-<surface> of the chart.
func internalURL(obj client.Object, surface string) string {
	return fmt.Sprintf("http://%s-%s.%s.svc.cluster.local:%d", obj.GetName(), surface, obj.GetNamespace(), serverPort)
}

// organisatieKey is the Organisatie that w names; an empty namespace is w's
// own.
func organisatieKey(w *v1alpha1.RegelHet) client.ObjectKey {
	ns := w.Spec.Organisatie.Namespace
	if ns == "" {
		ns = w.Namespace
	}
	return client.ObjectKey{Namespace: ns, Name: w.Spec.Organisatie.Name}
}

// membership is what the plugin decided about the spec.organisatie of a
// RegelHet.
type membership struct {
	// organisatie is the front the RegelHet is on the list of; nil when it
	// runs on its own.
	organisatie *v1alpha1.Organisatie
	// condition is OrganisatieGevonden.
	condition metav1.Condition
	// refused is set when the RegelHet must not run at all, with the
	// reason in condition.
	refused bool
}

// resolveOrganisatie decides whether w joins the Organisatie it names. One
// that does not exist, is being deleted or is in another Fundament project
// leaves w running on its own, with the condition False; a RegelHet whose
// auth mode differs from the front's is refused. w must name one.
func resolveOrganisatie(ctx context.Context, c client.Reader, namespaces client.Reader, domain string, w *v1alpha1.RegelHet) (membership, error) {
	key := organisatieKey(w)
	cond := func(status metav1.ConditionStatus, reason, message string) metav1.Condition {
		return metav1.Condition{Type: v1alpha1.ConditionOrganisatieFound, Status: status, Reason: reason, Message: message}
	}
	var o v1alpha1.Organisatie
	if err := c.Get(ctx, key, &o); err != nil {
		if !apierrors.IsNotFound(err) {
			return membership{}, fmt.Errorf("read Organisatie %s: %w", key, err)
		}
		return membership{condition: cond(metav1.ConditionFalse, reasonOrganisatieNotFound,
			fmt.Sprintf("Organisatie %s does not exist; the regelhet runs on its own", key))}, nil
	}
	if !o.DeletionTimestamp.IsZero() {
		return membership{condition: cond(metav1.ConditionFalse, reasonOrganisatieNotFound,
			fmt.Sprintf("Organisatie %s is being deleted; the regelhet runs on its own", key))}, nil
	}
	same, err := sameProject(ctx, namespaces, w.Namespace, key.Namespace)
	if err != nil {
		return membership{}, err
	}
	if !same {
		return membership{condition: cond(metav1.ConditionFalse, reasonOrganisatieElsewhere,
			fmt.Sprintf("Organisatie %s is not in the Fundament project of namespace %s; the regelhet runs on its own", key, w.Namespace))}, nil
	}
	refused, why, err := organisatieRefusal(ctx, c, domain, &o)
	if err != nil {
		return membership{}, err
	}
	if refused != "" {
		return membership{condition: cond(metav1.ConditionFalse, reasonOrganisatieRefused,
			fmt.Sprintf("the plugin refuses Organisatie %s (%s: %s); the regelhet runs on its own", key, refused, why))}, nil
	}
	if mine, theirs := regelhetMode(w), organisatieMode(&o); mine != theirs {
		return membership{refused: true, condition: cond(metav1.ConditionFalse, reasonOrganisatieModeMismatch,
			fmt.Sprintf("the regelhet runs in %s mode and Organisatie %s in %s mode; a front in test mode lets anyone in as anyone, so a stable regelhet cannot stand behind it", mine, key, theirs))}, nil
	}
	return membership{organisatie: &o, condition: cond(metav1.ConditionTrue, reasonOrganisatieFound,
		fmt.Sprintf("on the list of Organisatie %s", key))}, nil
}

// sameProject reports whether namespaces a and b belong to one Fundament
// project. One namespace is one project; two need the same project label, and
// a namespace without it is in no project at all.
func sameProject(ctx context.Context, namespaces client.Reader, a, b string) (bool, error) {
	if a == b {
		return true, nil
	}
	project := func(name string) (string, error) {
		var ns corev1.Namespace
		if err := namespaces.Get(ctx, client.ObjectKey{Name: name}, &ns); err != nil {
			if apierrors.IsNotFound(err) {
				return "", nil
			}
			return "", fmt.Errorf("read namespace %s: %w", name, err)
		}
		return ns.Labels[projectLabel], nil
	}
	pa, err := project(a)
	if err != nil {
		return false, err
	}
	pb, err := project(b)
	if err != nil {
		return false, err
	}
	return pa != "" && pa == pb, nil
}

// organisatieMembers are the RegelHet instances on the list of o, by namespace and
// name: those that name it, in any namespace, that the plugin does not refuse
// and that resolveOrganisatie lets join it. A refused RegelHet has no release,
// or another has its address, so the front would send people to the wrong place.
func organisatieMembers(ctx context.Context, c client.Reader, namespaces client.Reader, cfg pluginConfig, o *v1alpha1.Organisatie) ([]*v1alpha1.RegelHet, error) {
	var all v1alpha1.RegelHetList
	if err := c.List(ctx, &all); err != nil {
		return nil, fmt.Errorf("list regelhets: %w", err)
	}
	var members []*v1alpha1.RegelHet
	for i := range all.Items {
		w := &all.Items[i]
		if w.Spec.Organisatie == nil || !w.DeletionTimestamp.IsZero() || organisatieKey(w) != client.ObjectKeyFromObject(o) {
			continue
		}
		reason, _, err := regelhetRefusal(ctx, c, cfg, w)
		if err != nil {
			return nil, err
		}
		if reason != "" {
			continue
		}
		m, err := resolveOrganisatie(ctx, c, namespaces, cfg.Domain, w)
		if err != nil {
			return nil, err
		}
		if m.organisatie != nil && m.organisatie.UID == o.UID {
			members = append(members, w)
		}
	}
	sort.Slice(members, func(i, j int) bool { return objectID(members[i]) < objectID(members[j]) })
	return members, nil
}

// voorkantValues maps o and its members onto the values of charts/regelhet
// for a front: the block voorkant, with each process at its address on the
// cluster network and its public one. With voorkant.enabled the chart clones
// no case and makes no database, so neither is set here.
func voorkantValues(o *v1alpha1.Organisatie, members []*v1alpha1.RegelHet, cfg pluginConfig) map[string]any {
	processen := make([]any, 0, len(members))
	for _, w := range members {
		h := hostsOf(w, cfg.Domain)
		processen = append(processen, map[string]any{
			"id":   objectID(w),
			"naam": w.Name,
			"burger": map[string]any{
				"intern":  internalURL(w, "burger"),
				"publiek": "https://" + h.burger,
			},
			"backoffice": map[string]any{
				"intern":  internalURL(w, "backoffice"),
				"publiek": "https://" + h.backoffice,
			},
		})
	}
	values := baseValues(organisatieHosts(o, cfg.Domain), o.Spec.Theme.AccentColor, o.Spec.Version, cfg)
	values["voorkant"] = map[string]any{
		"enabled":   true,
		"id":        objectID(o),
		"naam":      o.Spec.DisplayName,
		"logo":      o.Spec.Logo,
		"processen": processen,
	}
	return values
}

// organisatieValues is the block organisatie of the values of a process on the
// list of o: who signs what it accepts, and where the front is.
func organisatieValues(o *v1alpha1.Organisatie, cfg pluginConfig) map[string]any {
	h := organisatieHosts(o, cfg.Domain)
	return map[string]any{
		"id":             objectID(o),
		"naam":           o.Spec.DisplayName,
		"url":            "https://" + h.burger,
		"medewerkersUrl": "https://" + h.backoffice,
		// Both sides of the front sign with one key; the employees' side
		// serves it as well as the citizens'.
		"sleutelUrl": internalURL(o, "backoffice") + sleutelPath,
	}
}

// claimant is an object that holds addresses and a Helm release name: a
// RegelHet or an Organisatie.
type claimant struct {
	kind string
	obj  client.Object
	// addresses are its hosts.
	addresses []string
}

func (c claimant) String() string { return c.kind + " " + objectID(c.obj) }

// claimants are every RegelHet and Organisatie on the cluster that is not
// being deleted.
func claimants(ctx context.Context, c client.Reader, domain string) ([]claimant, error) {
	var ws v1alpha1.RegelHetList
	if err := c.List(ctx, &ws); err != nil {
		return nil, fmt.Errorf("list regelhets: %w", err)
	}
	var os v1alpha1.OrganisatieList
	if err := c.List(ctx, &os); err != nil {
		return nil, fmt.Errorf("list organisaties: %w", err)
	}
	out := make([]claimant, 0, len(ws.Items)+len(os.Items))
	for i := range ws.Items {
		w := &ws.Items[i]
		if w.DeletionTimestamp.IsZero() {
			out = append(out, claimant{kind: "RegelHet", obj: w, addresses: regelhetAddresses(w, domain)})
		}
	}
	for i := range os.Items {
		o := &os.Items[i]
		if o.DeletionTimestamp.IsZero() {
			out = append(out, claimant{kind: "Organisatie", obj: o, addresses: organisatieHosts(o, domain).all()})
		}
	}
	return out, nil
}

// regelhetAddresses are the hosts of w. A party (parties.go) serves only on
// its hostname, but claims the employees' address too, as any RegelHet does.
func regelhetAddresses(w *v1alpha1.RegelHet, domain string) []string {
	return hostsOf(w, domain).all()
}

// addressTakenBy returns a reason and a message when another RegelHet or
// Organisatie has an address of obj (whose addresses are mine) or, in the same
// namespace, its name, and "" when none does. Of two the older keeps it, and
// between two made at the same second the first by namespace and name, so the
// answer does not depend on which one is reconciled first.
func addressTakenBy(ctx context.Context, c client.Reader, domain string, obj client.Object, mine []string) (reason, message string, err error) {
	all, err := claimants(ctx, c, domain)
	if err != nil {
		return "", "", err
	}
	for _, o := range all {
		if o.obj.GetUID() == obj.GetUID() || !comesFirst(o.obj, obj) {
			continue
		}
		if shareAnAddress(mine, o.addresses) {
			return reasonHostnameInUse, fmt.Sprintf("the hostname gives an address %s has already", o), nil
		}
		if o.obj.GetNamespace() == obj.GetNamespace() && o.obj.GetName() == obj.GetName() {
			return reasonNameInUse, fmt.Sprintf("%s has this name in this namespace already, and the two would share a Helm release", o), nil
		}
	}
	return "", "", nil
}

func comesFirst(a, b client.Object) bool {
	ta, tb := a.GetCreationTimestamp(), b.GetCreationTimestamp()
	if !ta.Equal(&tb) {
		return ta.Before(&tb)
	}
	return objectID(a) < objectID(b)
}

// status points at the phase, message and conditions of the status of a
// RegelHet or an Organisatie, which the two settle the same way.
type status struct {
	phase      *string
	message    *string
	conditions *[]metav1.Condition
}

func statusOf(phase, message *string, conditions *[]metav1.Condition) status {
	return status{phase: phase, message: message, conditions: conditions}
}

// refuse marks the object failed for reason.
func (s status) refuse(reason, message string) {
	*s.phase = v1alpha1.PhaseFailed
	*s.message = message
	apimeta.SetStatusCondition(s.conditions, metav1.Condition{
		Type: v1alpha1.ConditionReady, Status: metav1.ConditionFalse, Reason: reason, Message: message,
	})
}

// releaseDeps is what runRelease needs of a reconciler.
type releaseDeps struct {
	Client client.Client
	Scheme *runtime.Scheme
	Now    time.Time
}

// runRelease applies objects, owned by owner, and settles the status from the
// HelmRelease and the pods of the release: Ready once the release is, Failed
// once Flux gave up or a container keeps failing, Pending before.
//
// afterRelease, when not nil, runs once the release condition is known, and
// says how soon to look again (0 is on events only): the parties of a
// RegelHet.
func runRelease(ctx context.Context, d releaseDeps, owner client.Object, version string, objects []*unstructured.Unstructured, s status,
	afterRelease func(released metav1.Condition) (time.Duration, error)) (ctrl.Result, error) {
	for _, obj := range objects {
		if err := applyOwned(ctx, d.Client, d.Scheme, owner, obj); err != nil {
			return ctrl.Result{}, err
		}
	}
	released := releaseCondition(ctx, d.Client, owner, version)
	apimeta.SetStatusCondition(s.conditions, released)
	var requeue time.Duration
	if afterRelease != nil {
		var err error
		if requeue, err = afterRelease(released); err != nil {
			return ctrl.Result{}, err
		}
	}
	problem, err := releasePodProblem(ctx, d.Client, owner, d.Now)
	if err != nil {
		return ctrl.Result{}, err
	}
	if problem != nil {
		// Said within seconds, where Helm would only give up after its
		// timeout. The release goes on trying; once the pods run, the next
		// event clears this.
		s.refuse(reasonNotStarting, fmt.Sprintf("%s (%s)", problem.Message, problem.Reason))
		return ctrl.Result{RequeueAfter: requeue}, nil
	}

	ready := metav1.Condition{Type: v1alpha1.ConditionReady, Status: released.Status, Reason: released.Reason, Message: released.Message}
	apimeta.SetStatusCondition(s.conditions, ready)
	*s.message = released.Message
	switch released.Reason {
	case reasonReleaseReady:
		*s.phase = v1alpha1.PhaseReady
	case reasonReleaseStalled, reasonChartNotInCommit:
		*s.phase = v1alpha1.PhaseFailed
	default:
		*s.phase = v1alpha1.PhasePending
	}
	// A change of the HelmRelease's status is an event of an owned object,
	// so nothing needs to be polled, unless afterRelease asked for it.
	return ctrl.Result{RequeueAfter: requeue}, nil
}
