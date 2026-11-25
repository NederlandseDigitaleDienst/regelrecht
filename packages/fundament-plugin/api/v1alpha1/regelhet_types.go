package v1alpha1

import (
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

// Phases of a RegelHet, in status.phase.
const (
	// PhasePending: the plugin made the Flux objects and Flux has not
	// installed the release yet.
	PhasePending = "Pending"
	// PhaseReady: the release is installed and its pods are ready.
	PhaseReady = "Ready"
	// PhaseFailed: the spec is refused, or Flux gave up on the release.
	PhaseFailed = "Failed"
)

// Condition types in status.conditions.
const (
	// ConditionReady sums the others up: RegelHet runs.
	ConditionReady = "Ready"
	// ConditionReleaseReady mirrors Ready of the HelmRelease.
	ConditionReleaseReady = "ReleaseReady"
	// ConditionPartiesLinked says, for an executor, which RegelHet
	// instances run as its external parties, and for a party, whether it is
	// linked with its executor.
	ConditionPartiesLinked = "PartiesLinked"
	// ConditionSnapshotReady: the copy of the case for the current
	// spec.manifest is in the namespace.
	ConditionSnapshotReady = "SnapshotReady"
)

// Token states, in status.token.
const (
	// TokenPresent: the Secret in credentialsSecretRef is in the namespace.
	TokenPresent = "Present"
	// TokenAbsent: it is not, for instance because the plugin deleted it
	// after the snapshot. A new snapshot needs a new one.
	TokenAbsent = "Absent"
)

// SnapshotAnnotation, on a RegelHet, asks for a new snapshot of the same
// repoUrl, ref and path: any new value takes one, for instance to pick up
// the latest commit of a branch.
const SnapshotAnnotation = "regelhet.regelrecht.nl/snapshot"

// DeleteAfterSnapshotAnnotation, set to "true" on the Secret in
// credentialsSecretRef, lets the plugin delete that Secret once a snapshot
// of the case is in the namespace. The wizard sets it on the Secret it makes;
// the plugin never deletes a Secret without it.
const DeleteAfterSnapshotAnnotation = "regelhet.regelrecht.nl/delete-after-snapshot"

// SecretReference names a Secret in the namespace of the RegelHet.
type SecretReference struct {
	// Name of the Secret.
	// +kubebuilder:validation:MinLength=1
	// +kubebuilder:validation:MaxLength=253
	Name string `json:"name"`
}

// Manifest is where the case comes from: a git repository with a source
// manifest (bron.yaml). The case file and the articles it runs are in that
// repository; the manifest names them.
type Manifest struct {
	// RepoURL is the HTTPS URL of the git repository, without credentials.
	// +kubebuilder:validation:Pattern=`^https://[^@/\s]+/\S+$`
	RepoURL string `json:"repoUrl"`

	// Ref is the branch or tag.
	// +kubebuilder:default=main
	// +kubebuilder:validation:Pattern=`^[A-Za-z0-9._/][A-Za-z0-9._/-]*$`
	// +optional
	Ref string `json:"ref,omitempty"`

	// Path of the manifest inside the repository: bron.yaml at the root of a
	// manifest repository, packages/regelhet/bron.yaml in regelrecht itself.
	// +kubebuilder:default="bron.yaml"
	// +kubebuilder:validation:Pattern=`^[^/\s]\S*$`
	// +optional
	Path string `json:"path,omitempty"`

	// CredentialsSecretRef names a Secret with the keys `username` and
	// `token`, for a private repository. The token needs read access only.
	// Only the Job that copies the case reads it, at install and after a
	// change of the manifest; the pods of the release never do. With the
	// annotation regelhet.regelrecht.nl/delete-after-snapshot: "true" the
	// plugin deletes the Secret after that copy, and a later change of the
	// manifest needs a new one.
	// +optional
	CredentialsSecretRef *SecretReference `json:"credentialsSecretRef,omitempty"`
}

// Theme is how the screens look.
type Theme struct {
	// AccentColor is a theme of the NLDD design system. Empty keeps its own
	// accent.
	// +kubebuilder:validation:Enum="";bruin;donkerblauw;donkerbruin;donkergeel;donkergroen;geel;groen;hemelblauw;lichtblauw;mintgroen;mosgroen;oranje;paars;robijnrood;rood;roze;violet
	// +optional
	AccentColor string `json:"accentColor,omitempty"`
}

// Auth modes, in spec.auth.mode.
const (
	// AuthTest: the test login, where anyone picks a test person, and a clock
	// the admin sets.
	AuthTest = "test"
	// AuthStable: login through a Keycloak of this RegelHet only, and
	// the real date.
	AuthStable = "stable"
)

// Auth is how people log in.
type Auth struct {
	// Mode is test (the test login) or stable (a Keycloak of its own on
	// <hostname>-login, beside the other addresses).
	// +kubebuilder:validation:Enum=test;stable
	// +kubebuilder:default=test
	// +optional
	Mode string `json:"mode,omitempty"`

	// TestAccounts gives the Keycloak of stable mode the test accounts of the
	// case, all with one password the chart generates. For a test setup
	// only; without them, accounts are made in Keycloak itself.
	// +optional
	TestAccounts bool `json:"testAccounts,omitempty"`
}

// Fsc4b is FSC voor Burgers: the citizens' routes for an app besides the
// browser.
type Fsc4b struct {
	// Enabled offers the citizens' routes to an app that proves who the
	// citizen is with a credential, on the demo Stelsel with a test
	// Activatiedienst that checks no identity. Test mode only.
	// +optional
	Enabled bool `json:"enabled,omitempty"`

	// Origins are the browser apps that may call the channel from an
	// address of their own, such as the citizen's app: each an origin,
	// https://host or https://host:port, or http on localhost or 127.0.0.1
	// for an app run on a developer's machine. The server answers them with
	// CORS headers on /fsc4b/ (the chart's fsc4b.origins). Only with
	// enabled.
	// +optional
	// +kubebuilder:validation:MaxItems=20
	// +kubebuilder:validation:items:Pattern=`^(https://[a-z0-9]([a-z0-9.-]*[a-z0-9])?(:[0-9]{1,5})?|http://(localhost|127\.0\.0\.1)(:[0-9]{1,5})?)$`
	Origins []string `json:"origins,omitempty"`
}

// Partij makes a RegelHet the server of one external party of the case of
// another RegelHet, the executor: the GGD that examines an applicant for the
// municipality, say. Its staff log in on it and see what the executor asks
// of that party; the two servers exchange requests and answers as signed
// messages. The RegelHet keeps its own address, namespace, colour and title.
type Partij struct {
	// Cel is the party as the case of the executor names it, such as
	// ggd_wilgendaal. The executor's case must ask that party something by
	// its policy.
	// +kubebuilder:validation:MinLength=1
	// +kubebuilder:validation:MaxLength=100
	// +kubebuilder:validation:Pattern=`^[A-Za-z0-9_-]+$`
	Cel string `json:"cel"`

	// Uitvoerder is the RegelHet of the executor, in the same Fundament
	// project.
	Uitvoerder RegelHetReference `json:"uitvoerder"`
}

// Fsc reaches the RegelHet instances this one is linked with through
// OpenFSC instead of on their public address: a request goes to this
// RegelHet's own outway, which calls the other's inway over the mutual TLS
// of the FSC group, under a contract both sides signed. The plugin makes
// none of that: the FSCInstallation, the service and the contracts are made
// beside it, and their outcome is entered here.
type Fsc struct {
	// Outway is the address of this RegelHet's FSC outway inside the
	// cluster, such as http://fsc-outway-default.<namespace>, without a
	// slash at the end.
	// +kubebuilder:validation:MaxLength=253
	// +kubebuilder:validation:Pattern=`^https?://[A-Za-z0-9.-]+(:[0-9]+)?$`
	Outway string `json:"outway"`

	// Grants are, per cell this RegelHet is linked with, the grant under
	// which the outway may call that cell's service. A linked cell without
	// one is reached on its public address.
	// +optional
	// +listType=map
	// +listMapKey=cel
	// +kubebuilder:validation:MaxItems=20
	Grants []FscGrant `json:"grants,omitempty"`
}

// FscGrant is the FSC grant to call the service of one linked cell.
type FscGrant struct {
	// Cel is the linked cell as the case names it: for a party its
	// executor's cell, for an executor the cell of a party.
	// +kubebuilder:validation:MinLength=1
	// +kubebuilder:validation:MaxLength=100
	// +kubebuilder:validation:Pattern=`^[A-Za-z0-9_-]+$`
	Cel string `json:"cel"`

	// GrantHash is the hash of the service connection grant, as the
	// contract in OpenFSC holds it.
	// +kubebuilder:validation:MinLength=1
	// +kubebuilder:validation:MaxLength=200
	// +kubebuilder:validation:Pattern=`^[!-~]+$`
	GrantHash string `json:"grantHash"`
}

// RegelHetReference names a RegelHet.
type RegelHetReference struct {
	// Name of the RegelHet.
	// +kubebuilder:validation:MinLength=1
	// +kubebuilder:validation:MaxLength=40
	Name string `json:"name"`

	// Namespace of the RegelHet; empty is the namespace of the RegelHet
	// that names it. It must be a namespace of the same Fundament project.
	// +kubebuilder:validation:MaxLength=63
	// +optional
	Namespace string `json:"namespace,omitempty"`
}

// AskedParty is an external party that the case of an executor asks
// something of by its policy, as its running server names it.
type AskedParty struct {
	// Cell is the party as the case names it.
	Cell string `json:"cell"`
	// Name is what a screen calls it.
	// +optional
	Name string `json:"name,omitempty"`
}

// PartyStatus is an external party that runs on a RegelHet of its own,
// linked with this one.
type PartyStatus struct {
	// Cell is the party as the case names it.
	Cell string `json:"cell"`
	// Name is what a screen calls it.
	// +optional
	Name string `json:"name,omitempty"`
	// URL is where its staff log in.
	URL string `json:"url"`
	// Procedure is the RegelHet the party runs on.
	// +optional
	Procedure *RegelHetReference `json:"procedure,omitempty"`
	// Via is how this RegelHet reaches the party: `fsc` through its FSC
	// outway, `direct` on the party's public address.
	// +optional
	Via string `json:"via,omitempty"`
}

// RegelHetSpec is one RegelHet.
// +kubebuilder:validation:XValidation:rule="!(has(self.partij) && has(self.auth) && has(self.auth.mode) && self.auth.mode == 'stable')",message="partij links two servers with keys made at start: test mode only"
// +kubebuilder:validation:XValidation:rule="!(has(self.partij) && has(self.fsc4b) && has(self.fsc4b.enabled) && self.fsc4b.enabled)",message="partij runs a server for the staff of a party, without a citizens' side for fsc4b"
// +kubebuilder:validation:XValidation:rule="!(has(self.partij) && has(self.organisatie))",message="partij runs a server for the staff of a party, without a citizens' side to put behind the front of an organisatie"
// +kubebuilder:validation:XValidation:rule="!(has(self.fsc4b) && has(self.fsc4b.enabled) && self.fsc4b.enabled && has(self.auth) && has(self.auth.mode) && self.auth.mode == 'stable')",message="fsc4b.enabled runs on the demo Stelsel, whose test Activatiedienst checks no identity: test mode only"
// +kubebuilder:validation:XValidation:rule="!(has(self.fsc4b) && has(self.fsc4b.origins) && size(self.fsc4b.origins) > 0 && !(has(self.fsc4b.enabled) && self.fsc4b.enabled))",message="fsc4b.origins lets browser apps call FSC voor Burgers: only with fsc4b.enabled"
type RegelHetSpec struct {
	// Manifest is the case to run.
	Manifest Manifest `json:"manifest"`

	// Casus chooses the case to run from a manifest that makes several, by
	// the name of its case file: `bedrijfsparkeervergunning` for
	// casussen/bedrijfsparkeervergunning.yaml. So one manifest repository
	// holds several procedures, each its own RegelHet. Empty runs the only
	// case of the manifest; the server refuses to start when the manifest
	// makes several, or none by this name.
	// +kubebuilder:validation:MaxLength=100
	// +kubebuilder:validation:Pattern=`^([a-z0-9][a-z0-9_-]*)?$`
	// +optional
	Casus string `json:"casus,omitempty"`

	// Version is the regelrecht commit whose RegelHet image runs, the full
	// 40-character hash, and whose chart installs it. Empty follows main: the
	// chart on main and the latest build it names.
	// +kubebuilder:validation:Pattern=`^([0-9a-f]{40})?$`
	// +optional
	Version string `json:"version,omitempty"`

	// Theme is how the screens look.
	// +optional
	Theme Theme `json:"theme,omitempty"`

	// Title is the text in the bar at the top of the screens, at most 80
	// characters on one line, without spaces at either end. Empty carries
	// the title of the case (`titel` in the case file).
	// +kubebuilder:validation:MaxLength=80
	// +kubebuilder:validation:Pattern=`^(\S([^\x00-\x1f\x7f]*\S)?)?$`
	// +optional
	Title string `json:"title,omitempty"`

	// Auth is how people log in. Empty is the test login.
	// +optional
	Auth Auth `json:"auth,omitempty"`

	// Fsc4b is FSC voor Burgers. Empty is off.
	// +optional
	Fsc4b Fsc4b `json:"fsc4b,omitempty"`

	// Partij runs this RegelHet as the server of one external party of the
	// case of another RegelHet, the executor, linked with it. Such a
	// RegelHet has one address, for the party's staff: its hostname
	// itself. Empty runs it as an executor, whose external
	// parties either run as a RegelHet of their own that names it here or
	// are entered by an admin in the admin panel. Test mode only.
	// +optional
	Partij *Partij `json:"partij,omitempty"`

	// Fsc sends what this RegelHet has for a linked RegelHet through
	// OpenFSC. Empty reaches every linked RegelHet on its public address.
	// +optional
	Fsc *Fsc `json:"fsc,omitempty"`

	// Hostname is the address: a name under the plugin's base domain
	// (`zorgtoeslag` gives zorgtoeslag.<domain> for citizens and
	// zorgtoeslag-medewerkers.<domain> for employees), or a domain name of
	// your own, with a dot in it (employees on medewerkers.<hostname>).
	// +kubebuilder:validation:MinLength=1
	// +kubebuilder:validation:MaxLength=253
	// +kubebuilder:validation:Pattern=`^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*$`
	Hostname string `json:"hostname"`

	// Organisatie puts this RegelHet on the list of an Organisatie in the
	// same Fundament project, behind its front. The RegelHet keeps its own
	// addresses. Empty runs it on its own.
	// +optional
	Organisatie *OrganisatieReference `json:"organisatie,omitempty"`
}

// RegelHetStatus is what the plugin last saw.
type RegelHetStatus struct {
	// Phase is Pending, Ready or Failed.
	// +optional
	Phase string `json:"phase,omitempty"`

	// URL is where citizens use RegelHet.
	// +optional
	URL string `json:"url,omitempty"`

	// BackofficeURL is where employees do.
	// +optional
	BackofficeURL string `json:"backofficeUrl,omitempty"`

	// LoginURL is the Keycloak of stable mode; empty in test mode.
	// +optional
	LoginURL string `json:"loginUrl,omitempty"`

	// Fsc4bDiscoveryURL is the discovery configuration of FSC voor Burgers,
	// where an app finds the services; empty when it is off.
	// +optional
	Fsc4bDiscoveryURL string `json:"fsc4bDiscoveryUrl,omitempty"`

	// AskedParties are the external parties that the case asks something
	// of by its policy, as the running server last named them: the cells a
	// RegelHet with spec.partij may run as for this one. Empty for a
	// RegelHet that is itself a party.
	// +optional
	// +listType=map
	// +listMapKey=cell
	AskedParties []AskedParty `json:"askedParties,omitempty"`

	// Parties are the external parties that run on a RegelHet of their
	// own, linked with this one; empty when none does.
	// +optional
	// +listType=map
	// +listMapKey=cell
	Parties []PartyStatus `json:"parties,omitempty"`

	// ExecutorCell is the cell of the case the executor's release works
	// for, which a party's release names as its peer; set with
	// AskedParties.
	// +optional
	ExecutorCell string `json:"executorCell,omitempty"`

	// KeycloakSecret names the Secret in the namespace of this RegelHet
	// that the chart generates for stable mode: the Keycloak admin password
	// (user admin) under admin-password, and the password of the test
	// accounts under test-password. Only the name, never a value.
	// +optional
	KeycloakSecret string `json:"keycloakSecret,omitempty"`

	// Snapshot is the copy of the case that the release runs: the commit it
	// was taken from and the ConfigMaps that hold it.
	// +optional
	Snapshot *SnapshotStatus `json:"snapshot,omitempty"`

	// Token says whether the Secret in credentialsSecretRef is still in the
	// namespace: Present or Absent; empty without credentialsSecretRef.
	// +optional
	Token string `json:"token,omitempty"`

	// Message says why the phase is what it is.
	// +optional
	Message string `json:"message,omitempty"`

	// ObservedGeneration is the generation of the spec this status is about.
	// +optional
	ObservedGeneration int64 `json:"observedGeneration,omitempty"`

	// Conditions are Ready, SnapshotReady, ReleaseReady and PartiesLinked,
	// and OrganisatieGevonden with spec.organisatie.
	// +listType=map
	// +listMapKey=type
	// +optional
	Conditions []metav1.Condition `json:"conditions,omitempty"`
}

// SnapshotStatus is a copy of the case in the namespace of the RegelHet.
type SnapshotStatus struct {
	// Commit is the full hash of the commit of manifest.ref the copy was
	// taken from.
	// +optional
	Commit string `json:"commit,omitempty"`

	// ConfigMaps hold the copy, a tar.gz of the manifest and the files it
	// names, in parts of at most 900 KiB, in this order.
	// +optional
	ConfigMaps []string `json:"configMaps,omitempty"`
}

// RegelHet is one RegelHet: a case from a git repository, run by
// the RegelHet server on its own address.
// +kubebuilder:object:root=true
// +kubebuilder:subresource:status
// +kubebuilder:resource:path=regelhets,singular=regelhet,scope=Namespaced
// +kubebuilder:validation:XValidation:rule="size(self.metadata.name) <= 40",message="the name is at most 40 characters: it names the Helm release and the objects of the chart"
// +kubebuilder:printcolumn:name="Phase",type=string,JSONPath=`.status.phase`
// +kubebuilder:printcolumn:name="Auth",type=string,JSONPath=`.spec.auth.mode`
// +kubebuilder:printcolumn:name="URL",type=string,JSONPath=`.status.url`
// +kubebuilder:printcolumn:name="Commit",type=string,JSONPath=`.status.snapshot.commit`,priority=1
// +kubebuilder:printcolumn:name="Medewerkers",type=string,JSONPath=`.status.backofficeUrl`,priority=1
// +kubebuilder:printcolumn:name="Age",type=date,JSONPath=`.metadata.creationTimestamp`
type RegelHet struct {
	metav1.TypeMeta   `json:",inline"`
	metav1.ObjectMeta `json:"metadata,omitempty"`

	Spec   RegelHetSpec   `json:"spec"`
	Status RegelHetStatus `json:"status,omitempty"`
}

// RegelHetList is a list of RegelHet.
// +kubebuilder:object:root=true
type RegelHetList struct {
	metav1.TypeMeta `json:",inline"`
	metav1.ListMeta `json:"metadata,omitempty"`
	Items           []RegelHet `json:"items"`
}

func init() {
	SchemeBuilder.Register(&RegelHet{}, &RegelHetList{})
}
