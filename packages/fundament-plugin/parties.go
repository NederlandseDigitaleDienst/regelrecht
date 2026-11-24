package main

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"

	apierrors "k8s.io/apimachinery/pkg/api/errors"
	apimeta "k8s.io/apimachinery/pkg/api/meta"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/util/validation"
	"sigs.k8s.io/controller-runtime/pkg/client"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

// An external party on a server of its own.
//
// A case names its external parties, and its regelwerken say which of them
// the executor asks something of by its own policy: for the parking card the
// GGD, which examines the applicant, and not the court. Such a party does
// work of its own in the process. With spec.parties.ownInstances the plugin
// gives each a release of its own, where its staff log in, and links it with
// the executor's release (charts/regelhet, values `koppeling`).
//
// The plugin does not read the case. The server does, at start, and names
// those parties on partiesPath; the plugin asks the running release. So the
// executor's release comes first, without a link, and the parties follow
// once it runs. Their names are kept in the status, which is what the next
// reconcile builds the releases from.

const (
	// partiesPath is where the server names the parties its policy asks
	// something of (packages/regelhet/src/link.rs, PARTIES_PATH).
	partiesPath = "/koppeling/v1/partijen"

	// partyLabel carries the cell of the party on its HelmRelease.
	partyLabel = "regelhet.regelrecht.nl/partij"

	// partiesRequeue is how soon the plugin asks again after the server did
	// not answer.
	partiesRequeue = 30 * time.Second

	// partiesRefresh is how soon the plugin asks a server that answered
	// again, so a case that changed is followed without another event.
	partiesRefresh = 10 * time.Minute

	// serverPort is the port of the server's Service in the chart.
	serverPort = 8000

	// helmNameMax is the longest name Helm takes for a release.
	helmNameMax = 53

	reasonPartiesLinked   = "PartiesLinked"
	reasonPartiesUnknown  = "PartiesUnknown"
	reasonPartiesDisabled = "PartiesNotAsked"
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

// partySlug is the cell of a party as a part of a name: ggd_wilgendaal
// gives ggd-wilgendaal.
func partySlug(cell string) string {
	return strings.ToLower(strings.ReplaceAll(cell, "_", "-"))
}

// partyRelease is the release of the party cell of w.
func partyRelease(w *v1alpha1.RegelHet, cell string) string {
	return w.Name + "-" + partySlug(cell)
}

// partyHost is the address of the party cell of w: <hostname>-<party> beside
// the citizens' host, one label under the domain as the employees' host is;
// under a hostname with a dot, <party>.<hostname>.
func partyHost(w *v1alpha1.RegelHet, domain, cell string) string {
	name := w.Spec.Hostname
	if strings.Contains(name, ".") {
		return partySlug(cell) + "." + name
	}
	return name + "-" + partySlug(cell) + "." + domain
}

// reservedSlugs are names a party cannot take, because the chart gives
// them to parts of the executor's own release: <name>-burger and
// <name>-backoffice are its instances, <name>-medewerkers and <name>-login
// its addresses, and the others its database, Keycloak and chart source.
var reservedSlugs = map[string]bool{
	"burger": true, "backoffice": true, "medewerkers": true, "login": true,
	"keycloak": true, "db": true, "chart": true,
}

// partyStatuses turns the server's answer into the parties of w, leaving out
// with the reason a party whose name gives no valid release or address, one
// that would take a name of the executor's own release, and one whose name
// comes to the same release as an earlier party's. The names come from the
// case, which whoever owns its repository writes.
func partyStatuses(w *v1alpha1.RegelHet, domain string, asked askedParties) ([]v1alpha1.PartyStatus, []string) {
	var parties []v1alpha1.PartyStatus
	var skipped []string
	seen := map[string]bool{}
	own := hostsOf(w, domain).all()
	for _, p := range asked.Parties {
		slug := partySlug(p.Cell)
		host := partyHost(w, domain, p.Cell)
		release := partyRelease(w, p.Cell)
		label, _, _ := strings.Cut(host, ".")
		switch {
		case seen[slug]:
			skipped = append(skipped, fmt.Sprintf("%s: comes to the same name as another party", p.Cell))
		case reservedSlugs[slug] || shareAnAddress(own, []string{host}):
			skipped = append(skipped, fmt.Sprintf("%s: %s is a name of the executor's own release", p.Cell, release))
		case len(validation.IsDNS1123Subdomain(host)) > 0 || len(validation.IsDNS1123Label(label)) > 0:
			skipped = append(skipped, fmt.Sprintf("%s: %s is no valid address", p.Cell, host))
		case len(release) > helmNameMax || len(validation.IsDNS1123Label(release)) > 0:
			skipped = append(skipped, fmt.Sprintf("%s: %s is no valid release name", p.Cell, release))
		default:
			seen[slug] = true
			parties = append(parties, v1alpha1.PartyStatus{Cell: p.Cell, Name: p.Name, URL: "https://" + host + "/"})
		}
	}
	return parties, skipped
}

// partyHosts are the addresses of the parties of w, from its status.
func partyHosts(w *v1alpha1.RegelHet) []string {
	out := make([]string, 0, len(w.Status.Parties))
	for _, p := range w.Status.Parties {
		host := strings.TrimSuffix(strings.TrimPrefix(p.URL, "https://"), "/")
		out = append(out, host)
	}
	return out
}

// linkURL is an address as a peer of the link names it, without the slash a
// status URL ends on.
func linkURL(statusURL string) string { return strings.TrimSuffix(statusURL, "/") }

// executorPeers are the parties of w as the executor's release names them.
func executorPeers(w *v1alpha1.RegelHet) []any {
	peers := make([]any, 0, len(w.Status.Parties))
	for _, p := range w.Status.Parties {
		peers = append(peers, map[string]any{"cel": p.Cell, "url": linkURL(p.URL)})
	}
	return peers
}

// partyValues are the chart values of the release of the party p of w: the
// same case, image and look as the executor's release, as one instance for
// employees, linked with the executor's employees' instance.
func partyValues(w *v1alpha1.RegelHet, cfg pluginConfig, p v1alpha1.PartyStatus) map[string]any {
	values := chartValues(w, cfg, statusSnapshot(w))
	host := strings.TrimSuffix(strings.TrimPrefix(p.URL, "https://"), "/")
	values["host"] = host
	values["surfaces"] = map[string]any{"split": false}
	values["regelhet"] = map[string]any{"surface": "backoffice"}
	values["koppeling"] = map[string]any{
		"partij": p.Cell,
		"peers": []any{map[string]any{
			"cel": w.Status.ExecutorCell,
			"url": "https://" + hostsOf(w, cfg.Domain).backoffice,
		}},
	}
	// Another organization than the executor's, so another colour: someone
	// with both open tells them apart at a glance.
	values["theme"] = map[string]any{"accentColor": partyAccent(w.Spec.Theme.AccentColor)}
	// The party has no citizens' side, so no app reaches it.
	delete(values, "fsc4b")
	return values
}

// The accent colours of a party's release (themes of the NLDD design
// system): red, or ruby red beside an executor that is red itself.
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

// newPartyRelease builds the HelmRelease of the party p of w, from the chart
// source of w itself.
func newPartyRelease(w *v1alpha1.RegelHet, cfg pluginConfig, p v1alpha1.PartyStatus) *unstructured.Unstructured {
	name := partyRelease(w, p.Cell)
	hr := newHelmRelease(w, partyValues(w, cfg, p))
	hr.SetName(name)
	labels := hr.GetLabels()
	labels[partyLabel] = partySlug(p.Cell)
	hr.SetLabels(labels)
	_ = unstructured.SetNestedField(hr.Object, name, "spec", "releaseName")
	return hr
}

// reconcileParties brings the releases of the parties of w to what its
// running server asks: it asks the server when the executor's release is
// ready, keeps the answer in the status and applies a release per party. It
// says whether the status changed what the executor's release must name,
// and how soon to look again.
//
// A party is only ever added here. One the server no longer names keeps
// its release, and the condition says so: the answer depends on the day the
// server runs on, which an admin sets in test mode, and removing a release
// removes its database. Turning the option off removes them all.
func (r *RegelHetReconciler) reconcileParties(ctx context.Context, w *v1alpha1.RegelHet, executorReady bool) (changed bool, requeue time.Duration, err error) {
	before := executorPeers(w)
	condition := metav1.Condition{Type: v1alpha1.ConditionPartiesLinked, Status: metav1.ConditionTrue, Reason: reasonPartiesLinked}
	switch {
	case !w.Spec.Parties.OwnInstances:
		w.Status.Parties, w.Status.ExecutorCell = nil, ""
		condition.Reason, condition.Message = reasonPartiesDisabled, "no party runs on a server of its own"
	case !executorReady:
		// Not asked yet, or the release is being upgraded: what the status
		// holds stays, so a running link is not torn down by a rollout.
		condition.Status, condition.Reason = metav1.ConditionFalse, reasonPartiesUnknown
		condition.Message = "the server names its parties once its release is ready"
	default:
		requeue = partiesRefresh
		asked, askErr := r.partiesReader().Asked(ctx, w)
		if askErr == nil && asked.Executor.Cell == "" {
			askErr = fmt.Errorf("the server's answer names no executor, so it is not the list of parties")
		}
		if askErr != nil {
			condition.Status, condition.Reason, condition.Message = metav1.ConditionFalse, reasonPartiesUnknown, askErr.Error()
			requeue = partiesRequeue
			break
		}
		named, notes := partyStatuses(w, r.Config.Domain, asked)
		named, taken, heldErr := r.withoutTakenNames(ctx, w, named)
		if heldErr != nil {
			return false, 0, heldErr
		}
		notes = append(notes, taken...)
		parties, kept := mergeParties(w.Status.Parties, named)
		w.Status.Parties, w.Status.ExecutorCell = parties, asked.Executor.Cell
		condition.Message = fmt.Sprintf("%d of the case's parties run on a server of their own", len(parties))
		if len(parties) == 0 {
			condition.Message = "the regelwerken of this case ask no party for anything, so none gets a server of its own"
		}
		if len(kept) > 0 {
			condition.Message += "; no longer asked but kept, with its data, until parties.ownInstances is turned off: " + strings.Join(kept, ", ")
		}
		if len(notes) > 0 {
			condition.Status, condition.Reason = metav1.ConditionFalse, reasonPartiesUnknown
			condition.Message += "; left out: " + strings.Join(notes, "; ")
		}
	}
	apimeta.SetStatusCondition(&w.Status.Conditions, condition)

	wanted := map[string]bool{}
	for _, p := range w.Status.Parties {
		wanted[partyRelease(w, p.Cell)] = true
		if err := r.apply(ctx, w, newPartyRelease(w, r.Config, p)); err != nil {
			return false, 0, err
		}
	}
	if err := r.pruneParties(ctx, w, wanted); err != nil {
		return false, 0, err
	}
	return fmt.Sprint(before) != fmt.Sprint(executorPeers(w)), requeue, nil
}

// mergeParties are the parties the status held with those the server names
// now: each named party as named, and after them the ones it no longer
// names, whose cells it returns too.
func mergeParties(held, named []v1alpha1.PartyStatus) (all []v1alpha1.PartyStatus, kept []string) {
	all = append(all, named...)
	for _, h := range held {
		still := false
		for _, n := range named {
			still = still || n.Cell == h.Cell
		}
		if !still {
			all = append(all, h)
			kept = append(kept, h.Cell)
		}
	}
	return all, kept
}

// withoutTakenNames leaves out of parties each one whose release name or
// address belongs to something else already: a HelmRelease of that name
// that is not this party's own, or an address another RegelHet has. The
// plugin applies with force, so without this check a party would take over
// the release of a RegelHet that happens to be named <name>-<party>.
func (r *RegelHetReconciler) withoutTakenNames(ctx context.Context, w *v1alpha1.RegelHet, parties []v1alpha1.PartyStatus) (free []v1alpha1.PartyStatus, notes []string, err error) {
	var all v1alpha1.RegelHetList
	if err := r.Client.List(ctx, &all); err != nil {
		return nil, nil, fmt.Errorf("list regelhets: %w", err)
	}
	for _, p := range parties {
		name := partyRelease(w, p.Cell)
		host := strings.TrimSuffix(strings.TrimPrefix(p.URL, "https://"), "/")
		hr := &unstructured.Unstructured{}
		hr.SetGroupVersionKind(helmReleaseGVK)
		getErr := r.Client.Get(ctx, client.ObjectKey{Namespace: w.Namespace, Name: name}, hr)
		if getErr != nil && !apierrors.IsNotFound(getErr) {
			return nil, nil, fmt.Errorf("read the HelmRelease %s/%s: %w", w.Namespace, name, getErr)
		}
		if getErr == nil && !isPartyReleaseOf(hr, w) {
			notes = append(notes, fmt.Sprintf("%s: a release named %s exists already and is not this party's", p.Cell, name))
			continue
		}
		holder := ""
		for i := range all.Items {
			o := &all.Items[i]
			if o.UID == w.UID || !o.DeletionTimestamp.IsZero() {
				continue
			}
			if o.Namespace == w.Namespace && o.Name == name {
				holder = o.Namespace + "/" + o.Name
			}
			if shareAnAddress([]string{host}, append(hostsOf(o, r.Config.Domain).all(), partyHosts(o)...)) {
				holder = o.Namespace + "/" + o.Name
			}
		}
		if holder != "" {
			notes = append(notes, fmt.Sprintf("%s: RegelHet %s has the name or address %s already", p.Cell, holder, host))
			continue
		}
		free = append(free, p)
	}
	return free, notes, nil
}

// isPartyReleaseOf reports whether hr is a party release the plugin made
// for w: controlled by w and carrying the party label.
func isPartyReleaseOf(hr *unstructured.Unstructured, w *v1alpha1.RegelHet) bool {
	_, isParty := hr.GetLabels()[partyLabel]
	return isParty && metav1.IsControlledBy(hr, w)
}

// pruneParties deletes the party releases of w that are not in wanted.
// helm-controller then uninstalls each, its database included. Only a
// release the plugin made for w as a party's is ever deleted.
func (r *RegelHetReconciler) pruneParties(ctx context.Context, w *v1alpha1.RegelHet, wanted map[string]bool) error {
	list := &unstructured.UnstructuredList{}
	list.SetGroupVersionKind(helmReleaseGVK)
	err := r.Client.List(ctx, list, client.InNamespace(w.Namespace),
		client.MatchingLabels{nameLabel: w.Name}, client.HasLabels{partyLabel})
	if err != nil {
		return fmt.Errorf("list the party releases of %s/%s: %w", w.Namespace, w.Name, err)
	}
	for i := range list.Items {
		hr := &list.Items[i]
		if wanted[hr.GetName()] || !isPartyReleaseOf(hr, w) {
			continue
		}
		if err := client.IgnoreNotFound(r.Client.Delete(ctx, hr)); err != nil {
			return fmt.Errorf("delete the party release %s/%s: %w", hr.GetNamespace(), hr.GetName(), err)
		}
	}
	return nil
}

func (r *RegelHetReconciler) partiesReader() PartiesReader {
	if r.Parties != nil {
		return r.Parties
	}
	return newHTTPParties()
}
