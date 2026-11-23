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

// The default case is the fork the chart comes from.
func TestDefaultCaseIsTheFork(t *testing.T) {
	t.Parallel()

	f, ok := reflect.TypeFor[pluginConfig]().FieldByName("CaseRepoURL")
	require.True(t, ok)
	assert.Equal(t, forkURL, f.Tag.Get("envDefault"))
}
