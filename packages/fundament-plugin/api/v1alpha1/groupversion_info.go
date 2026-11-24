// Package v1alpha1 holds the API of the RegelHet plugin: the RegelHet
// resource, one RegelHet per object.
// +kubebuilder:object:generate=true
// +groupName=regelhet.regelrecht.nl
package v1alpha1

import (
	"k8s.io/apimachinery/pkg/runtime/schema"
	"sigs.k8s.io/controller-runtime/pkg/scheme"
)

var (
	// GroupVersion is the group and version of the RegelHet resource.
	GroupVersion = schema.GroupVersion{Group: "regelhet.regelrecht.nl", Version: "v1alpha1"}

	// SchemeBuilder adds the types of this group-version to a scheme. The
	// deprecation is about API packages pulling in controller-runtime; this
	// one is local to the plugin, which depends on it anyway.
	SchemeBuilder = &scheme.Builder{GroupVersion: GroupVersion} //nolint:staticcheck // SA1019: kubebuilder's scaffolded pattern

	// AddToScheme adds the types of this group-version to a scheme.
	AddToScheme = SchemeBuilder.AddToScheme
)
