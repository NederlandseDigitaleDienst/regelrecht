package main

import (
	"os"
	"reflect"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"github.com/fundament-oss/fundament/plugin-sdk/pluginruntime"
)

// loadSourceDefinition parses definition.yaml, the authored, image-free source
// manifest, with ParseSourceDefinition. Publishing injects the pushed image
// digest and validates the result with the stricter ParseDefinition.
func loadSourceDefinition(t *testing.T) pluginruntime.PluginDefinition {
	t.Helper()

	src, err := os.ReadFile("definition.yaml")
	require.NoError(t, err, "definition.yaml must exist")

	def, err := pluginruntime.ParseSourceDefinition(src)
	require.NoError(t, err, "definition.yaml must parse without error")

	return def
}

func TestDefinitionParses(t *testing.T) {
	t.Parallel()

	def := loadSourceDefinition(t)

	assert.Equal(t, pluginName, def.Metadata.Name)
}

// The schema in definition.yaml and the env tags on pluginConfig are one
// contract: a key the console prompts for must be one the binary reads, with
// the same default and the same required flag.
func TestConfigSchemaMatchesPluginConfig(t *testing.T) {
	t.Parallel()

	def := loadSourceDefinition(t)

	fields := map[string]reflect.StructField{}
	typ := reflect.TypeFor[pluginConfig]()
	for i := range typ.NumField() {
		f := typ.Field(i)
		name, _, _ := strings.Cut(f.Tag.Get("env"), ",")
		fields[strings.TrimPrefix(name, "FUNP_")] = f
	}

	require.Len(t, def.Spec.ConfigSchema, len(fields), "every pluginConfig field needs a configSchema entry")
	for _, entry := range def.Spec.ConfigSchema {
		f, ok := fields[entry.Name]
		require.True(t, ok, "configSchema key %s is not read by pluginConfig", entry.Name)
		assert.Equal(t, entry.Required, strings.Contains(f.Tag.Get("env"), ",required"), "required flag of %s", entry.Name)
		assert.Equal(t, entry.Default, f.Tag.Get("envDefault"), "default of %s", entry.Name)
	}
}

// menu, customComponents and the files under console/ are one contract that
// nothing checks at build time: every page the definition names must be
// embedded, and the menu must offer the CRD the pages are for.
func TestCustomComponentsAreEmbedded(t *testing.T) {
	t.Parallel()

	def := loadSourceDefinition(t)
	for _, kind := range []string{"RegelHet", "Organisatie"} {
		pages, ok := def.Spec.CustomComponents[kind]
		require.True(t, ok, kind)
		for view, file := range map[string]string{"list": pages.List, "detail": pages.Detail, "create": pages.Create} {
			require.NotEmpty(t, file, "the %s page of %s", view, kind)
			_, err := consoleFiles.ReadFile("console/" + file)
			require.NoError(t, err, "the %s page %s is not embedded", view, file)
		}
	}
	require.Len(t, def.Spec.Menu.Project, 2)
	assert.Equal(t, crdName, def.Spec.Menu.Project[0].CRD)
	assert.Equal(t, organisatieCRDName, def.Spec.Menu.Project[1].CRD)
	for _, crd := range []string{crdName, organisatieCRDName} {
		assert.Contains(t, def.Spec.UIHints, crd, "the status badge of %s", crd)
	}
}

// The RBAC of the plugin is what Digilab reviews. v0.5.0 adds the organisatie
// rules and get on namespaces, and nothing more: namespaces stay get only, and
// the organisaties carry the same verbs as the RegelHet instances.
func TestTheRBACOfOrganisaties(t *testing.T) {
	t.Parallel()

	def := loadSourceDefinition(t)
	verbs := map[string][]string{}
	for _, rule := range def.Spec.Permissions.RBAC {
		if len(rule.ResourceNames) > 0 {
			continue
		}
		for _, resource := range rule.Resources {
			verbs[resource] = append(verbs[resource], rule.Verbs...)
		}
	}
	for _, sub := range []string{"", "/status", "/finalizers"} {
		assert.ElementsMatch(t, verbs["regelhets"+sub], verbs["organisaties"+sub], "organisaties%s", sub)
	}
	assert.Equal(t, []string{"get"}, verbs["namespaces"], "the project check reads one namespace at a time")

	var crds []string
	for _, rule := range def.Spec.Permissions.RBAC {
		if len(rule.Resources) == 1 && rule.Resources[0] == "customresourcedefinitions" {
			crds = append(crds, rule.ResourceNames...)
		}
	}
	assert.Contains(t, crds, organisatieCRDName)
}

// v0.6.0 adds the snapshot of the case and nothing more: its own Jobs, the
// ConfigMaps of the copy without watch, and the log of a pod, read only.
func TestTheRBACOfTheSnapshot(t *testing.T) {
	t.Parallel()

	def := loadSourceDefinition(t)
	verbs := map[string][]string{}
	for _, rule := range def.Spec.Permissions.RBAC {
		for _, resource := range rule.Resources {
			verbs[resource] = append(verbs[resource], rule.Verbs...)
		}
	}
	assert.ElementsMatch(t, []string{"create", "get", "list", "watch", "delete"}, verbs["jobs"])
	assert.ElementsMatch(t, []string{"create", "get", "list", "delete"}, verbs["configmaps"])
	assert.Equal(t, []string{"get"}, verbs["pods/log"])
	assert.ElementsMatch(t, []string{"create", "get", "delete"}, verbs["secrets"], "no list or watch on Secrets")
	// v0.7.0 (spec.partij) asks for no permission beyond these.
	assert.Equal(t, "v0.7.0", def.Metadata.Version)
}
