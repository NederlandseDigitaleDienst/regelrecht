package main

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"sort"
	"strings"
	"time"

	apierrors "k8s.io/apimachinery/pkg/api/errors"
	apimeta "k8s.io/apimachinery/pkg/api/meta"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/log"
	"sigs.k8s.io/controller-runtime/pkg/reconcile"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

// An external party as a RegelHet of its own.
//
// A case names its external parties, and its regelwerken say which of them
// the executor asks something of by its own policy: for the parking card the
// GGD, which examines the applicant, and not the court. Such a party does
// work of its own in the process. It runs as a RegelHet of its own, which an
// admin installs like any other and which names, in spec.partij, its cell
// and the RegelHet of the executor. It keeps its own address, namespace,
// colour and title; the plugin links the two releases (charts/regelhet,
// values `koppeling`).
//
// The plugin does not read the case. The executor's server does, at start,
// and names those parties on partiesPath; the plugin asks the running
// release and keeps the answer in the executor's status (askedParties). A
// party is linked once its cell is in that list. The executor finds its
// parties as an Organisatie finds its RegelHet instances: the RegelHet
// instances of the same Fundament project that name it.

const (
	// partiesPath is where the server names the parties its policy asks
	// something of (packages/regelhet/src/link.rs, PARTIES_PATH).
	partiesPath = "/koppeling/v1/partijen"

	// partyLabel carried the cell of a party on a HelmRelease the plugin
	// made for it under the executor, before a party became a RegelHet of
	// its own (spec.parties.ownInstances, plugin v0.6.0). The plugin
	// deletes those releases (pruneOwnPartyReleases).
	partyLabel = "regelhet.regelrecht.nl/partij"

	// partiesRequeue is how soon the plugin asks again after the server did
	// not answer, and looks again at a party whose executor named no parties
	// yet.
	partiesRequeue = 30 * time.Second

	// partiesRefresh is how soon the plugin asks a server that answered
	// again, so a case that changed is followed without another event.
	partiesRefresh = 10 * time.Minute

	// serverPort is the port of the server's Service in the chart.
	serverPort = 8000

	reasonPartiesLinked  = "PartiesLinked"
	reasonPartiesUnknown = "PartiesUnknown"
	// reasonPartyPending: the executor of a party has not named its parties
	// yet; the party waits for it.
	reasonPartyPending = "PartyPending"
	// The reasons the plugin refuses a RegelHet with spec.partij. Failed.
	reasonExecutorNotFound  = "ExecutorNotFound"
	reasonExecutorElsewhere = "ExecutorInOtherProject"
	reasonExecutorNotUsable = "ExecutorNotUsable"
	reasonPartyNotAsked     = "PartyNotAsked"
	reasonPartyTaken        = "PartyTaken"
)

// askedParties is what the server answers on partiesPath.
type askedParties struct {
	Executor struct {
		Cell string `json:"cel"`
	} `json:"uitvoerder"`
	Parties []struct {
		Cell string `json:"cel"`
		Name string `json:"cel_naam"`
	} `json:"partijen"`
}

// PartiesReader asks the running release of a RegelHet which parties its
// policy asks something of.
type PartiesReader interface {
	Asked(ctx context.Context, w *v1alpha1.RegelHet) (askedParties, error)
}

// httpParties asks the Service of the employees' instance inside the
// cluster. It needs no certificate and no public name to exist yet, and the
// answer holds only what the case itself says.
type httpParties struct {
	client *http.Client
}

func newHTTPParties() *httpParties {
	return &httpParties{client: &http.Client{Timeout: 5 * time.Second}}
}

func (p *httpParties) Asked(ctx context.Context, w *v1alpha1.RegelHet) (askedParties, error) {
	var asked askedParties
	url := internalURL(w, "backoffice") + partiesPath
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return asked, fmt.Errorf("ask %s: %w", url, err)
	}
	resp, err := p.client.Do(req)
	if err != nil {
		return asked, fmt.Errorf("ask %s: %w", url, err)
	}
	defer resp.Body.Close() //nolint:errcheck // nothing to do about a failed close of a read body.
	if resp.StatusCode == http.StatusNotFound {
		return asked, fmt.Errorf("this build of the server does not name its parties (%s answers 404); choose a later version", partiesPath)
	}
	if resp.StatusCode != http.StatusOK {
		return asked, fmt.Errorf("ask %s: status %d", url, resp.StatusCode)
	}
	body, err := io.ReadAll(io.LimitReader(resp.Body, 1<<20))
	if err != nil {
		return asked, fmt.Errorf("read the answer of %s: %w", url, err)
	}
	if err := json.Unmarshal(body, &asked); err != nil {
		return asked, fmt.Errorf("the answer of %s is not the list of parties: %w", url, err)
	}
	return asked, nil
}

// executorKey is the RegelHet that the party w names as its executor; an
// empty namespace is w's own. w must have spec.partij.
func executorKey(w *v1alpha1.RegelHet) client.ObjectKey {
	ns := w.Spec.Partij.Uitvoerder.Namespace
	if ns == "" {
		ns = w.Namespace
	}
	return client.ObjectKey{Namespace: ns, Name: w.Spec.Partij.Uitvoerder.Name}
}

// isParty reports whether w runs as an external party of another RegelHet.
func isParty(w *v1alpha1.RegelHet) bool { return w.Spec.Partij != nil }

// partyHost is the one address of the party w: its hostname itself, as the
// citizens of an executor have theirs. A party has no citizens' side, so its
// staff take that address.
func partyHost(w *v1alpha1.RegelHet, domain string) string {
	return hostsOf(w, domain).burger
}

// linkURL is an address as a peer of the link names it: https, without a
// slash at the end.
func linkURL(host string) string { return "https://" + host }

// executorPeers are the parties of w as the executor's release names them.
func executorPeers(w *v1alpha1.RegelHet) []any {
	peers := make([]any, 0, len(w.Status.Parties))
	for _, p := range w.Status.Parties {
		peers = append(peers, map[string]any{"cel": p.Cell, "url": strings.TrimSuffix(p.URL, "/")})
	}
	return peers
}

// partyValues turns the chart values of the party w into those of one
// instance for the party's staff, on its hostname, linked with the employees' instance of its executor e.
func partyValues(values map[string]any, w, e *v1alpha1.RegelHet, cfg pluginConfig) map[string]any {
	values["host"] = partyHost(w, cfg.Domain)
	values["surfaces"] = map[string]any{"split": false}
	values["regelhet"] = map[string]any{"surface": "backoffice"}
	values["koppeling"] = map[string]any{
		"partij": w.Spec.Partij.Cel,
		"peers": []any{map[string]any{
			"cel": e.Status.ExecutorCell,
			"url": linkURL(hostsOf(e, cfg.Domain).backoffice),
		}},
	}
	if w.Spec.Theme.AccentColor == "" {
		// Another organization than the executor's, so another colour:
		// someone with both open tells them apart at a glance. The wizard
		// writes this colour into the spec; this covers a party made
		// without one.
		values["theme"].(map[string]any)["accentColor"] = partyAccent(e.Spec.Theme.AccentColor)
	}
	return values
}

// The accent colours a party gets when its spec names none (themes of the
// NLDD design system): red, or ruby red beside an executor that is red
// itself. The wizard offers the same (console/regelhets-body.js).
const (
	partyAccentColor    = "rood"
	partyAccentFallback = "robijnrood"
)

// partyAccent is the accent colour of a party beside an executor whose
// accent is executor: never the same one.
func partyAccent(executor string) string {
	if executor == partyAccentColor {
		return partyAccentFallback
	}
	return partyAccentColor
}

// partyLink is what the plugin decided about the spec.partij of a RegelHet.
type partyLink struct {
	// executor is the RegelHet the party is linked with; nil when it is not.
	executor *v1alpha1.RegelHet
	// condition is PartiesLinked. Its reason is reasonPartyPending while
	// the executor has not named its parties, and a refusal otherwise.
	condition metav1.Condition
}

// pending reports whether the party waits for its executor.
func (l partyLink) pending() bool { return l.condition.Reason == reasonPartyPending }

// resolveExecutor decides whether the party w is linked with the executor it
// names. The executor must exist in the same Fundament project, be accepted
// by the plugin, run in test mode, be no party itself, and its case must ask
// the cell of w something, or have done so while w was linked; no RegelHet
// made earlier may run as that cell for it. w must have spec.partij.
func resolveExecutor(ctx context.Context, c client.Reader, namespaces client.Reader, cfg pluginConfig, w *v1alpha1.RegelHet) (partyLink, error) {
	key := executorKey(w)
	cond := func(status metav1.ConditionStatus, reason, message string) partyLink {
		return partyLink{condition: metav1.Condition{Type: v1alpha1.ConditionPartiesLinked, Status: status, Reason: reason, Message: message}}
	}
	var e v1alpha1.RegelHet
	if err := c.Get(ctx, key, &e); err != nil {
		if !apierrors.IsNotFound(err) {
			return partyLink{}, fmt.Errorf("read RegelHet %s: %w", key, err)
		}
		return cond(metav1.ConditionFalse, reasonExecutorNotFound, fmt.Sprintf("the executor, RegelHet %s, does not exist", key)), nil
	}
	if !e.DeletionTimestamp.IsZero() {
		return cond(metav1.ConditionFalse, reasonExecutorNotFound, fmt.Sprintf("the executor, RegelHet %s, is being deleted", key)), nil
	}
	var all v1alpha1.RegelHetList
	if err := c.List(ctx, &all); err != nil {
		return partyLink{}, fmt.Errorf("list regelhets: %w", err)
	}
	reason, message, err := partyRefusal(ctx, c, namespaces, cfg, &e, w, all.Items)
	if err != nil {
		return partyLink{}, err
	}
	if reason != "" {
		return cond(metav1.ConditionFalse, reason, message), nil
	}
	link := cond(metav1.ConditionTrue, reasonPartiesLinked, fmt.Sprintf("runs as %s for the executor, RegelHet %s", w.Spec.Partij.Cel, key))
	link.executor = &e
	return link, nil
}

// partyRefusal is why the executor e does not link the party p, or "". The
// reason reasonPartyPending says that e has not named its parties yet. all
// are the RegelHet instances on the cluster, to find another that runs as
// the same cell. The plugin decides the same from both sides: the executor
// links exactly the parties that are told they are linked.
func partyRefusal(ctx context.Context, c client.Reader, namespaces client.Reader, cfg pluginConfig, e, p *v1alpha1.RegelHet, all []v1alpha1.RegelHet) (reason, message string, err error) {
	reason, message, err = partyBasics(ctx, c, namespaces, cfg, e, p)
	if reason != "" || err != nil {
		return reason, message, err
	}
	cell := p.Spec.Partij.Cel
	switch {
	case e.Status.ExecutorCell == "":
		return reasonPartyPending, pendingMessage(e), nil
	case cell == e.Status.ExecutorCell:
		return reasonPartyNotAsked, fmt.Sprintf("%s is the cell of the executor, RegelHet %s, itself", cell, objectID(e)), nil
	case !askedOrLinked(e, p):
		return reasonPartyNotAsked, notAskedMessage(e, cell), nil
	}
	if linkedNow(e, p) {
		// A running link is not taken over: that would lose messages.
		return "", "", nil
	}
	for i := range all {
		o := &all[i]
		if o.UID == p.UID || !o.DeletionTimestamp.IsZero() || !isParty(o) || o.Spec.Partij.Cel != cell ||
			executorKey(o) != client.ObjectKeyFromObject(e) || !(linkedNow(e, o) || comesFirst(o, p)) {
			continue
		}
		// Only a competitor that would be linked itself counts.
		if r, _, err := partyBasics(ctx, c, namespaces, cfg, e, o); err != nil {
			return "", "", err
		} else if r != "" || !askedOrLinked(e, o) {
			continue
		}
		return reasonPartyTaken, fmt.Sprintf("RegelHet %s runs as %s for the executor, RegelHet %s, already", objectID(o), cell, objectID(e)), nil
	}
	return "", "", nil
}

// pendingMessage says that a party waits for its executor e to name the
// parties of its case, with what e itself says about asking its server.
func pendingMessage(e *v1alpha1.RegelHet) string {
	msg := fmt.Sprintf("waits until the executor, RegelHet %s, runs and names the parties of its case", objectID(e))
	if c := apimeta.FindStatusCondition(e.Status.Conditions, v1alpha1.ConditionPartiesLinked); c != nil && c.Status != metav1.ConditionTrue {
		msg += "; the executor says: " + c.Message
	}
	return msg
}

// partyBasics are the refusals of partyRefusal that do not depend on the
// other parties: the party itself, the project and the executor.
func partyBasics(ctx context.Context, c client.Reader, namespaces client.Reader, cfg pluginConfig, e, p *v1alpha1.RegelHet) (reason, message string, err error) {
	if reason, message, err := regelhetRefusal(ctx, c, cfg, p); reason != "" || err != nil {
		return reason, message, err
	}
	same, err := sameProject(ctx, namespaces, p.Namespace, e.Namespace)
	if err != nil {
		return "", "", err
	}
	if !same {
		return reasonExecutorElsewhere, fmt.Sprintf("the executor, RegelHet %s, is not in the Fundament project of namespace %s", objectID(e), p.Namespace), nil
	}
	if isParty(e) {
		return reasonExecutorNotUsable, fmt.Sprintf("RegelHet %s runs as a party itself, so it is no executor", objectID(e)), nil
	}
	if isStable(e) {
		return reasonExecutorNotUsable, fmt.Sprintf("the executor, RegelHet %s, runs in stable mode, which pins every key; a link runs in test mode only", objectID(e)), nil
	}
	if r, m, err := regelhetRefusal(ctx, c, cfg, e); err != nil {
		return "", "", err
	} else if r != "" {
		return reasonExecutorNotUsable, fmt.Sprintf("the plugin refuses the executor, RegelHet %s (%s: %s)", objectID(e), r, m), nil
	}
	return "", "", nil
}

// askedOrLinked reports whether the case of e asks the cell of p something,
// or e links p already. A party stays linked when the server stops naming
// it: the answer depends on the day the server runs on, which an admin sets
// in test mode, and a link that comes and goes with it would lose messages.
func askedOrLinked(e, p *v1alpha1.RegelHet) bool {
	for _, a := range e.Status.AskedParties {
		if a.Cell == p.Spec.Partij.Cel {
			return true
		}
	}
	return linkedNow(e, p)
}

// linkedNow reports whether e links p as its cell already.
func linkedNow(e, p *v1alpha1.RegelHet) bool {
	for _, l := range e.Status.Parties {
		if l.Cell == p.Spec.Partij.Cel && l.Procedure != nil && l.Procedure.Name == p.Name && l.Procedure.Namespace == p.Namespace {
			return true
		}
	}
	return false
}

// notAskedMessage says that the case of e asks cell nothing, and which
// parties it does ask something of.
func notAskedMessage(e *v1alpha1.RegelHet, cell string) string {
	cells := make([]string, 0, len(e.Status.AskedParties))
	for _, a := range e.Status.AskedParties {
		cells = append(cells, a.Cell)
	}
	if len(cells) == 0 {
		return fmt.Sprintf("the case of the executor, RegelHet %s, asks %s nothing; it asks no party for anything", objectID(e), cell)
	}
	return fmt.Sprintf("the case of the executor, RegelHet %s, asks %s nothing; the parties it asks something of are %s", objectID(e), cell, strings.Join(cells, ", "))
}

// linkedParties are the parties of the executor w: the RegelHet instances
// that name it in spec.partij and that partyRefusal accepts, by cell, and a
// note for each one it does not.
func linkedParties(ctx context.Context, c client.Reader, namespaces client.Reader, cfg pluginConfig, w *v1alpha1.RegelHet) (parties []v1alpha1.PartyStatus, notes []string, err error) {
	var all v1alpha1.RegelHetList
	if err := c.List(ctx, &all); err != nil {
		return nil, nil, fmt.Errorf("list regelhets: %w", err)
	}
	for i := range all.Items {
		p := &all.Items[i]
		if !isParty(p) || !p.DeletionTimestamp.IsZero() || executorKey(p) != client.ObjectKeyFromObject(w) {
			continue
		}
		reason, message, err := partyRefusal(ctx, c, namespaces, cfg, w, p, all.Items)
		if err != nil {
			return nil, nil, err
		}
		if reason != "" {
			notes = append(notes, fmt.Sprintf("RegelHet %s: %s", objectID(p), message))
			continue
		}
		parties = append(parties, v1alpha1.PartyStatus{
			Cell:      p.Spec.Partij.Cel,
			Name:      cellName(w, p.Spec.Partij.Cel),
			URL:       linkURL(partyHost(p, cfg.Domain)) + "/",
			Procedure: &v1alpha1.RegelHetReference{Name: p.Name, Namespace: p.Namespace},
		})
	}
	sort.Slice(parties, func(i, j int) bool { return parties[i].Cell < parties[j].Cell })
	sort.Strings(notes)
	return parties, notes, nil
}

// cellName is what the case of w calls cell, as its server named it, or as
// the status held it before.
func cellName(w *v1alpha1.RegelHet, cell string) string {
	for _, a := range w.Status.AskedParties {
		if a.Cell == cell {
			return a.Name
		}
	}
	for _, p := range w.Status.Parties {
		if p.Cell == cell {
			return p.Name
		}
	}
	return ""
}

// reconcileParties brings the parties of the executor w up to date: it asks
// the server which parties the case asks something of when the release is
// ready, keeps the answer in the status, and links the RegelHet instances
// that run as one of them. It says whether that changed what the executor's
// release must name, and how soon to look again.
//
// Of a RegelHet that is itself a party, it clears what belongs to an
// executor. Of every RegelHet, it deletes the party releases that plugin
// versions before spec.partij made.
func (r *RegelHetReconciler) reconcileParties(ctx context.Context, w *v1alpha1.RegelHet, executorReady bool) (changed bool, requeue time.Duration, err error) {
	before := executorPeers(w)
	if err := r.pruneOwnPartyReleases(ctx, w); err != nil {
		return false, 0, err
	}
	if isParty(w) {
		w.Status.AskedParties, w.Status.Parties, w.Status.ExecutorCell = nil, nil, ""
		return len(before) > 0, 0, nil
	}
	condition := metav1.Condition{Type: v1alpha1.ConditionPartiesLinked, Status: metav1.ConditionTrue, Reason: reasonPartiesLinked}
	var asking string
	if executorReady {
		requeue = partiesRefresh
		asked, askErr := r.partiesReader().Asked(ctx, w)
		if askErr == nil && asked.Executor.Cell == "" {
			askErr = fmt.Errorf("the server's answer names no executor, so it is not the list of parties")
		}
		if askErr != nil {
			asking, requeue = askErr.Error(), partiesRequeue
		} else {
			w.Status.ExecutorCell = asked.Executor.Cell
			w.Status.AskedParties = make([]v1alpha1.AskedParty, 0, len(asked.Parties))
			for _, p := range asked.Parties {
				w.Status.AskedParties = append(w.Status.AskedParties, v1alpha1.AskedParty{Cell: p.Cell, Name: p.Name})
			}
		}
	} else if w.Status.ExecutorCell == "" {
		// Not asked yet. Once asked, what the status holds stays while the
		// release is upgraded, so a running link is not torn down by a
		// rollout.
		asking = "the server names the parties of its case once its release is ready"
	}

	parties, notes, err := linkedParties(ctx, r.Client, r.Namespaces, r.Config, w)
	if err != nil {
		return false, 0, err
	}
	w.Status.Parties = parties
	switch {
	case len(parties) > 0:
		cells := make([]string, 0, len(parties))
		for _, p := range parties {
			cells = append(cells, fmt.Sprintf("%s on RegelHet %s/%s", p.Cell, p.Procedure.Namespace, p.Procedure.Name))
		}
		condition.Message = "linked with " + strings.Join(cells, ", ")
	case w.Status.ExecutorCell != "" && len(w.Status.AskedParties) == 0:
		condition.Message = "the regelwerken of this case ask no party for anything"
	default:
		condition.Message = "no RegelHet runs as a party of this one"
	}
	if asking != "" {
		condition.Status, condition.Reason = metav1.ConditionFalse, reasonPartiesUnknown
		condition.Message += "; " + asking
		if len(parties) == 0 && len(notes) == 0 {
			// Nothing to link, and nothing wrong: a build of the server that
			// does not name its parties runs fine on its own.
			condition.Status = metav1.ConditionUnknown
		}
	}
	if len(notes) > 0 {
		condition.Status, condition.Reason = metav1.ConditionFalse, reasonPartiesUnknown
		condition.Message += "; not linked: " + strings.Join(notes, "; ")
	}
	apimeta.SetStatusCondition(&w.Status.Conditions, condition)
	return fmt.Sprint(before) != fmt.Sprint(executorPeers(w)), requeue, nil
}

// pruneOwnPartyReleases deletes the HelmReleases that plugin versions before
// spec.partij made under w for its parties (spec.parties.ownInstances).
// helm-controller then uninstalls each, its database included. Only a
// release controlled by w that carries the party label is deleted.
func (r *RegelHetReconciler) pruneOwnPartyReleases(ctx context.Context, w *v1alpha1.RegelHet) error {
	list := &unstructured.UnstructuredList{}
	list.SetGroupVersionKind(helmReleaseGVK)
	err := r.Client.List(ctx, list, client.InNamespace(w.Namespace),
		client.MatchingLabels{nameLabel: w.Name}, client.HasLabels{partyLabel})
	if err != nil {
		return fmt.Errorf("list the party releases of %s/%s: %w", w.Namespace, w.Name, err)
	}
	for i := range list.Items {
		hr := &list.Items[i]
		if !metav1.IsControlledBy(hr, w) {
			continue
		}
		if err := client.IgnoreNotFound(r.Client.Delete(ctx, hr)); err != nil {
			return fmt.Errorf("delete the party release %s/%s: %w", hr.GetNamespace(), hr.GetName(), err)
		}
		log.FromContext(ctx).Info("deleted a party release of before spec.partij; the party runs as a RegelHet of its own now",
			"release", client.ObjectKeyFromObject(hr))
	}
	return nil
}

func (r *RegelHetReconciler) partiesReader() PartiesReader {
	if r.Parties != nil {
		return r.Parties
	}
	return newHTTPParties()
}

// linkedRegelHets are the RegelHet instances a change of obj concerns as a
// party or an executor: the executor obj names, and the parties that name
// obj. An update maps both the old and the new object, so a party that
// moves to another executor leaves the first one's list too.
func (r *RegelHetReconciler) linkedRegelHets(ctx context.Context, obj client.Object) []reconcile.Request {
	w, ok := obj.(*v1alpha1.RegelHet)
	if !ok {
		return nil
	}
	var out []reconcile.Request
	if isParty(w) {
		out = append(out, reconcile.Request{NamespacedName: executorKey(w)})
	}
	var all v1alpha1.RegelHetList
	if err := r.Client.List(ctx, &all); err != nil {
		log.FromContext(ctx).Error(err, "list the regelhets that run as a party", "executor", client.ObjectKeyFromObject(obj))
		return out
	}
	for i := range all.Items {
		p := &all.Items[i]
		if isParty(p) && executorKey(p) == client.ObjectKeyFromObject(w) {
			out = append(out, reconcile.Request{NamespacedName: client.ObjectKeyFromObject(p)})
		}
	}
	return out
}
