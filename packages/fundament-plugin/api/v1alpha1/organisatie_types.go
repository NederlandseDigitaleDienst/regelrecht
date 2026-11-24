package v1alpha1

import (
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
)

// ConditionOrganisatieFound, on a Wetsproces with spec.organisatie: True once
// the organisatie exists in the same Fundament project and the wetsproces is
// on its list. False leaves the wetsproces running on its own.
const ConditionOrganisatieFound = "OrganisatieGevonden"

// OrganisatieReference names the Organisatie a Wetsproces belongs to.
type OrganisatieReference struct {
	// Name of the Organisatie.
	// +kubebuilder:validation:MinLength=1
	// +kubebuilder:validation:MaxLength=40
	Name string `json:"name"`

	// Namespace of the Organisatie; empty is the namespace of the
	// Wetsproces. It must be a namespace of the same Fundament project.
	// +kubebuilder:validation:MaxLength=63
	// +optional
	Namespace string `json:"namespace,omitempty"`
}

// OrganisatieAuth is how people log in at the front of an organisatie.
type OrganisatieAuth struct {
	// Mode is test: the test login, where anyone picks a test person of the
	// wetsprocessen of the organisatie. Stable mode comes later.
	// +kubebuilder:validation:Enum=test
	// +kubebuilder:default=test
	// +optional
	Mode string `json:"mode,omitempty"`
}

// OrganisatieSpec is one front for several wetsprocessen.
type OrganisatieSpec struct {
	// DisplayName is the name people see, such as Gemeente Wilgendaal.
	// +kubebuilder:validation:MinLength=1
	// +kubebuilder:validation:MaxLength=100
	DisplayName string `json:"displayName"`

	// Logo is the https URL of the logo; empty shows the name only.
	// +kubebuilder:validation:MaxLength=2048
	// +kubebuilder:validation:Pattern=`^(https://[^@/\s]+(/\S*)?)?$`
	// +optional
	Logo string `json:"logo,omitempty"`

	// Theme is how the screens look.
	// +optional
	Theme Theme `json:"theme,omitempty"`

	// Hostname is the address, as for a Wetsproces: citizens on
	// <hostname>.<domain>, employees on <hostname>-medewerkers.<domain>, or a
	// domain of your own with a dot in it.
	// +kubebuilder:validation:MinLength=1
	// +kubebuilder:validation:MaxLength=253
	// +kubebuilder:validation:Pattern=`^[a-z0-9]([a-z0-9-]*[a-z0-9])?(\.[a-z0-9]([a-z0-9-]*[a-z0-9])?)*$`
	Hostname string `json:"hostname"`

	// Version is the regelrecht commit whose image runs the front, as for a
	// Wetsproces. Empty follows main.
	// +kubebuilder:validation:Pattern=`^([0-9a-f]{40})?$`
	// +optional
	Version string `json:"version,omitempty"`

	// Auth is how people log in. Empty is the test login.
	// +optional
	Auth OrganisatieAuth `json:"auth,omitempty"`
}

// ProcessReference is a Wetsproces on the list of an Organisatie.
type ProcessReference struct {
	Namespace string `json:"namespace"`
	Name      string `json:"name"`
	// Phase is the phase of that Wetsproces.
	// +optional
	Phase string `json:"phase,omitempty"`
}

// OrganisatieStatus is what the plugin last saw.
type OrganisatieStatus struct {
	// Phase is Pending, Ready or Failed.
	// +optional
	Phase string `json:"phase,omitempty"`

	// URL is where citizens use the front.
	// +optional
	URL string `json:"url,omitempty"`

	// BackofficeURL is where employees do.
	// +optional
	BackofficeURL string `json:"backofficeUrl,omitempty"`

	// LoginURL is the Keycloak of stable mode; empty in test mode.
	// +optional
	LoginURL string `json:"loginUrl,omitempty"`

	// Processen are the wetsprocessen on the list of the front, by namespace
	// and name.
	// +listType=atomic
	// +optional
	Processen []ProcessReference `json:"processen,omitempty"`

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

// Organisatie is one front, with its own name, logo and login, for the
// wetsprocessen that name it in spec.organisatie. Each wetsproces keeps its
// own release and database.
// +kubebuilder:object:root=true
// +kubebuilder:subresource:status
// +kubebuilder:resource:path=organisaties,singular=organisatie,scope=Namespaced
// +kubebuilder:validation:XValidation:rule="size(self.metadata.name) <= 40",message="the name is at most 40 characters: it names the Helm release and the objects of the chart"
// +kubebuilder:printcolumn:name="Phase",type=string,JSONPath=`.status.phase`
// +kubebuilder:printcolumn:name="Name",type=string,JSONPath=`.spec.displayName`
// +kubebuilder:printcolumn:name="URL",type=string,JSONPath=`.status.url`
// +kubebuilder:printcolumn:name="Medewerkers",type=string,JSONPath=`.status.backofficeUrl`,priority=1
// +kubebuilder:printcolumn:name="Age",type=date,JSONPath=`.metadata.creationTimestamp`
type Organisatie struct {
	metav1.TypeMeta   `json:",inline"`
	metav1.ObjectMeta `json:"metadata,omitempty"`

	Spec   OrganisatieSpec   `json:"spec"`
	Status OrganisatieStatus `json:"status,omitempty"`
}

// OrganisatieList is a list of Organisatie.
// +kubebuilder:object:root=true
type OrganisatieList struct {
	metav1.TypeMeta `json:",inline"`
	metav1.ListMeta `json:"metadata,omitempty"`
	Items           []Organisatie `json:"items"`
}

func init() {
	SchemeBuilder.Register(&Organisatie{}, &OrganisatieList{})
}
