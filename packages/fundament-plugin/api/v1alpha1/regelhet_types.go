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
)

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

	// Path of the manifest inside the repository.
	// +kubebuilder:default="packages/regelhet/bron.yaml"
	// +kubebuilder:validation:Pattern=`^[^/\s]\S*$`
	// +optional
	Path string `json:"path,omitempty"`

	// CredentialsSecretRef names a Secret with the keys `username` and
	// `token`, for a private repository. The token needs read access only.
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

// RegelHetSpec is one RegelHet.
type RegelHetSpec struct {
	// Manifest is the case to run.
	Manifest Manifest `json:"manifest"`

	// Version is the regelrecht commit whose RegelHet image runs, the full
	// 40-character hash. Empty follows the latest build of main.
	// +kubebuilder:validation:Pattern=`^([0-9a-f]{40})?$`
	// +optional
	Version string `json:"version,omitempty"`

	// Theme is how the screens look.
	// +optional
	Theme Theme `json:"theme,omitempty"`

	// Hostname is the address: a name under the plugin's base domain
	// (`zorgtoeslag` gives zorgtoeslag.<domain> for citizens and
	// zorgtoeslag-medewerkers.<domain> for employees), or a domain name of
	// your own, with a dot in it (employees on medewerkers.<hostname>).
	// +kubebuilder:validation:MinLength=1
	// +kubebuilder:validation:MaxLength=253
	// +kubebuilder:validation:Pattern=`^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*$`
	Hostname string `json:"hostname"`
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

	// Message says why the phase is what it is.
	// +optional
	Message string `json:"message,omitempty"`

	// ObservedGeneration is the generation of the spec this status is about.
	// +optional
	ObservedGeneration int64 `json:"observedGeneration,omitempty"`

	// Conditions are Ready and ReleaseReady.
	// +listType=map
	// +listMapKey=type
	// +optional
	Conditions []metav1.Condition `json:"conditions,omitempty"`
}

// RegelHet is one RegelHet: a case from a git repository, run by
// the RegelHet server on its own address.
// +kubebuilder:object:root=true
// +kubebuilder:subresource:status
// +kubebuilder:resource:path=regelhets,singular=regelhet,scope=Namespaced
// +kubebuilder:validation:XValidation:rule="size(self.metadata.name) <= 40",message="the name is at most 40 characters: it names the Helm release and the objects of the chart"
// +kubebuilder:printcolumn:name="Phase",type=string,JSONPath=`.status.phase`
// +kubebuilder:printcolumn:name="URL",type=string,JSONPath=`.status.url`
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
