package main

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"context"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	batchv1 "k8s.io/api/batch/v1"
	corev1 "k8s.io/api/core/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

// tarGz packs files as a snapshot holds them.
func tarGz(t *testing.T, files map[string]string) []byte {
	t.Helper()
	var buf bytes.Buffer
	gz := gzip.NewWriter(&buf)
	tw := tar.NewWriter(gz)
	for name, text := range files {
		require.NoError(t, tw.WriteHeader(&tar.Header{Name: name, Mode: 0o644, Size: int64(len(text)), Typeflag: tar.TypeReg}))
		_, err := tw.Write([]byte(text))
		require.NoError(t, err)
	}
	require.NoError(t, tw.Close())
	require.NoError(t, gz.Close())
	return buf.Bytes()
}

const kvkManifest = "repo_root: ../..\ndirectories:\n  casussen: case/casussen\n  cells/kvk: case/cellen/kvk\n"

// A case like the Napp: a login step, a register that issued user names
// with a KvK number, and a Chamber of Commerce cell with the names.
const stepCase = `cel: loket
inloggen:
  feit: wet_digitale_overheid#15#authenticatie_verricht
  sleutel: kvk_nummer
  veld: kvk_nummer_opgegeven
  inlogstap: {cel: dienst, toegang: r#4#toegang, dienst: urn:x}
andere_cellen:
  - cel: kvk
    begin:
      executogrammen:
        - feit: k#1#ingeschreven
          velden: {kvk_nummer: '90123456', naam: Vereniging Maasstad Vooruit}
        - feit: k#1#ingeschreven
          velden: {kvk_nummer: '90200000', naam: Vereniging Zonder Middel}
  - cel: dienst
    begin:
      executogrammen:
        - feit: r#2#machtiging
          velden: {gebruikersnaam: maasstad-vooruit, kvk_nummer: '90123456'}
`

func TestTheLoginOfACaseWithAStepGivesItsUsersAndTheirNumbers(t *testing.T) {
	t.Parallel()
	data := tarGz(t, map[string]string{
		"case/napp/bron.yaml":            kvkManifest,
		"case/casussen/napp.yaml":        stepCase,
		"case/cellen/kvk/reductie.yaml":  "x: 1\n",
		"case/casussen/sub/no-case.yaml": "cel: not-this-one\n",
	})
	login, err := readCaseLogin(data, "case/napp/bron.yaml")
	require.NoError(t, err)
	assert.Equal(t, "kvk_nummer", login.Key)
	assert.Equal(t, []caseApplicant{
		{Number: "90123456", Name: "Vereniging Maasstad Vooruit", Username: "maasstad-vooruit"},
	}, login.Applicants, "only who has a user name at the step can pass it")
}

func TestTheLoginOfACaseWithoutAStepNamesItsApplicantsByNumber(t *testing.T) {
	t.Parallel()
	c := `cel: gemeente
inloggen: {feit: f, sleutel: kvk_nummer, veld: kvk_nummer}
begin:
  executogrammen:
    - velden: {kvk_nummer: '99990011', handelsnaam: Bakkerij De Wilg}
    - velden: {kvk_nummer: 99990022}
`
	data := tarGz(t, map[string]string{"case/napp/bron.yaml": kvkManifest, "case/casussen/b.yaml": c})
	login, err := readCaseLogin(data, "case/napp/bron.yaml")
	require.NoError(t, err)
	assert.Equal(t, []caseApplicant{
		{Number: "99990011", Name: "Bakkerij De Wilg", Username: "kvk-99990011"},
		{Number: "99990022", Username: "kvk-99990022"},
	}, login.Applicants)
}

func TestACaseOnBsnHasNoLoginKey(t *testing.T) {
	t.Parallel()
	data := tarGz(t, map[string]string{"case/napp/bron.yaml": kvkManifest, "case/casussen/p.yaml": "cel: gemeente\n"})
	login, err := readCaseLogin(data, "case/napp/bron.yaml")
	require.NoError(t, err)
	assert.Equal(t, &caseLogin{}, login)
}

func TestStableModeOnAKvkCaseSetsTheClaimAndTheTestApplicants(t *testing.T) {
	t.Parallel()
	w := stable(newRegelHet("napp", "napp"), true)
	snap := &snapshot{commit: testCommit, configMaps: []string{"napp-case-1-0"}, login: &caseLogin{
		Key:        "kvk_nummer",
		Applicants: []caseApplicant{{Number: "90123456", Name: "Vereniging Maasstad Vooruit", Username: "maasstad-vooruit"}},
	}}
	values := chartValues(w, validConfig(), snap)
	assert.Equal(t, "kvk_nummer", values["regelhet"].(map[string]any)["subjectClaim"])
	assert.Equal(t, []any{map[string]any{"username": "maasstad-vooruit", "number": "90123456", "name": "Vereniging Maasstad Vooruit"}},
		values["keycloak"].(map[string]any)["testApplicants"])

	// Without test accounts no applicants; in test mode the server reads
	// the key from the case itself.
	w.Spec.Auth.TestAccounts = false
	assert.NotContains(t, chartValues(w, validConfig(), snap)["keycloak"], "testApplicants")
	w.Spec.Auth = v1alpha1.Auth{}
	assert.NotContains(t, chartValues(w, validConfig(), snap), "regelhet")
}

// The login of the case goes from the snapshot into the stable release's
// values, and stays there on a later reconcile, from the ConfigMaps.
func TestTheStableReleaseOfAKvkCaseGetsItsClaimFromTheSnapshot(t *testing.T) {
	t.Parallel()
	w := stable(newRegelHet("napp", "napp"), true)
	w.Spec.Manifest.Path = "case/napp/bron.yaml"
	r := newBareReconciler(t, w)
	r.Config.ACME = acmeProduction
	data := tarGz(t, map[string]string{"case/napp/bron.yaml": kvkManifest, "case/casussen/napp.yaml": stepCase})
	r.Logs = func(context.Context, string, string, string) ([]byte, error) { return snapshotLog(data), nil }
	reconcileOnce(t, r, w)
	finishJob(t, r.Client, w, batchv1.JobComplete, &corev1.Pod{})
	for range 2 {
		got := reconcileOnce(t, r, w)
		require.NotNil(t, got.Status.Snapshot)
		hr, err := helmRelease(t, r.Client, "napp")
		require.NoError(t, err)
		claim, _, _ := unstructured.NestedString(hr.Object, "spec", "values", "regelhet", "subjectClaim")
		assert.Equal(t, "kvk_nummer", claim)
		users, _, _ := unstructured.NestedSlice(hr.Object, "spec", "values", "keycloak", "testApplicants")
		assert.Len(t, users, 1)
	}
}

// A number keeps its text as written, as the server reads it: an unquoted
// leading zero is no octal number. A name loses what Keycloak refuses in a
// first name, and a value the chart's schema would refuse is left out.
func TestApplicantsKeepTheNumberAsWrittenAndANameKeycloakTakes(t *testing.T) {
	t.Parallel()
	c := `cel: gemeente
inloggen: {feit: f, sleutel: kvk_nummer, veld: kvk_nummer}
begin:
  executogrammen:
    - velden: {kvk_nummer: 01234567, handelsnaam: "Bakker & Zn. (Wilgendaal) ${x}"}
    - velden: {kvk_nummer: '1234 5678', handelsnaam: Spaties}
    - velden: {kvk_nummer: 99990022}
`
	data := tarGz(t, map[string]string{"case/napp/bron.yaml": kvkManifest, "case/casussen/b.yaml": c})
	login, err := readCaseLogin(data, "case/napp/bron.yaml")
	require.NoError(t, err)
	assert.Equal(t, []caseApplicant{
		{Number: "01234567", Name: "Bakker Zn. Wilgendaal x", Username: "kvk-01234567"},
		{Number: "99990022", Username: "kvk-99990022"},
	}, login.Applicants)
}
