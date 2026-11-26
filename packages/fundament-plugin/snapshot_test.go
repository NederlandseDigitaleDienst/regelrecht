package main

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"context"
	"encoding/base64"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	batchv1 "k8s.io/api/batch/v1"
	corev1 "k8s.io/api/core/v1"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	apimeta "k8s.io/apimachinery/pkg/api/meta"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"sigs.k8s.io/controller-runtime/pkg/client"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

var testCommit = strings.Repeat("a", 40)

// snapshotLog is what snapshot.sh writes for data.
func snapshotLog(data []byte) []byte {
	var b bytes.Buffer
	fmt.Fprintf(&b, "regelhet-snapshot commit=%s bytes=%d\n", testCommit, len(data))
	enc := base64.StdEncoding.EncodeToString(data)
	for len(enc) > 76 {
		b.WriteString(enc[:76] + "\n")
		enc = enc[76:]
	}
	b.WriteString(enc + "\nregelhet-snapshot end\n")
	return b.Bytes()
}

func getJob(t *testing.T, c client.Client, w *v1alpha1.RegelHet) (*batchv1.Job, error) {
	t.Helper()
	var job batchv1.Job
	err := c.Get(context.Background(), client.ObjectKey{Namespace: w.Namespace, Name: snapshotJobName(w, snapshotSource(w))}, &job)
	return &job, err
}

// finishJob marks the snapshot Job of w done with the condition, and makes
// the pod it ran in.
func finishJob(t *testing.T, c client.Client, w *v1alpha1.RegelHet, condition batchv1.JobConditionType, pod *corev1.Pod) {
	t.Helper()
	job, err := getJob(t, c, w)
	require.NoError(t, err)
	job.Status.Conditions = append(job.Status.Conditions, batchv1.JobCondition{
		Type: condition, Status: corev1.ConditionTrue, LastTransitionTime: metav1.NewTime(testNow),
	})
	require.NoError(t, c.Status().Update(context.Background(), job))
	pod.Name, pod.Namespace = job.Name+"-x", w.Namespace
	pod.Labels = map[string]string{appLabel: appName, "batch.kubernetes.io/job-name": job.Name}
	require.NoError(t, c.Create(context.Background(), pod))
}

func helmRelease(t *testing.T, c client.Client, name string) (*unstructured.Unstructured, error) {
	t.Helper()
	obj := &unstructured.Unstructured{}
	obj.SetGroupVersionKind(helmReleaseGVK)
	err := c.Get(context.Background(), client.ObjectKey{Namespace: testNamespace, Name: name}, obj)
	return obj, err
}

func TestTheFirstReconcileTakesASnapshotAndInstallsNothingYet(t *testing.T) {
	t.Parallel()
	w := newRegelHet("a", "a")
	r := newBareReconciler(t, w)
	got := reconcileOnce(t, r, w)

	assert.Equal(t, v1alpha1.PhasePending, got.Status.Phase)
	assert.Equal(t, reasonSnapshotPending, apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionSnapshotReady).Reason)
	_, err := helmRelease(t, r.Client, "a")
	assert.True(t, apierrors.IsNotFound(err), "no release before the case is in the namespace")

	job, err := getJob(t, r.Client, w)
	require.NoError(t, err)
	assert.Equal(t, "RegelHet", job.OwnerReferences[0].Kind)
	pod := job.Spec.Template.Spec
	assert.False(t, *pod.AutomountServiceAccountToken)
	env := map[string]string{}
	for _, e := range pod.Containers[0].Env {
		env[e.Name] = e.Value
	}
	assert.Equal(t, "https://git.example.org/casus/a.git", env["CASE_REPO"])
	assert.Equal(t, "main", env["CASE_REF"])
	assert.Equal(t, "bron.yaml", env["CASE_PATH"])
	assert.NotContains(t, env, "GIT_TOKEN", "a public repository needs no token")
}

func TestAPrivateRepositoryWaitsForItsTokenSecret(t *testing.T) {
	t.Parallel()
	w := newRegelHet("a", "a")
	w.Spec.Manifest.CredentialsSecretRef = &v1alpha1.SecretReference{Name: "a-repo"}
	r := newBareReconciler(t, w)
	got := reconcileOnce(t, r, w)
	assert.Equal(t, v1alpha1.PhasePending, got.Status.Phase, "the wizard makes the Secret just after the RegelHet")
	assert.Equal(t, reasonTokenMissing, apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionSnapshotReady).Reason)
	assert.Equal(t, v1alpha1.TokenAbsent, got.Status.Token)
	assert.Contains(t, got.Status.Message, "a-repo")
	_, err := getJob(t, r.Client, w)
	assert.True(t, apierrors.IsNotFound(err), "no Job without its Secret")

	require.NoError(t, r.Client.Create(context.Background(), &corev1.Secret{
		ObjectMeta: metav1.ObjectMeta{Name: "a-repo", Namespace: testNamespace},
		Data:       map[string][]byte{"username": []byte("x"), "token": []byte("t")},
	}))
	got = reconcileOnce(t, r, w)
	assert.Equal(t, v1alpha1.TokenPresent, got.Status.Token)
	job, err := getJob(t, r.Client, w)
	require.NoError(t, err)
	var token *corev1.EnvVar
	for i, e := range job.Spec.Template.Spec.Containers[0].Env {
		if e.Name == "GIT_TOKEN" {
			token = &job.Spec.Template.Spec.Containers[0].Env[i]
		}
	}
	require.NotNil(t, token)
	assert.Equal(t, "a-repo", token.ValueFrom.SecretKeyRef.Name)
}

func TestAFinishedJobBecomesConfigMapsAndTheRelease(t *testing.T) {
	t.Parallel()
	w := newRegelHet("a", "a")
	w.Spec.Manifest.CredentialsSecretRef = &v1alpha1.SecretReference{Name: "a-repo"}
	marked := &corev1.Secret{ObjectMeta: metav1.ObjectMeta{
		Name: "a-repo", Namespace: testNamespace, UID: "uid-secret",
		Annotations: map[string]string{v1alpha1.DeleteAfterSnapshotAnnotation: "true"},
	}}
	r := newBareReconciler(t, w, marked)
	// Larger than one ConfigMap holds.
	data := bytes.Repeat([]byte("0123456789"), snapshotPartBytes/10+5)
	r.Logs = func(_ context.Context, _, _, container string) ([]byte, error) {
		assert.Equal(t, "snapshot", container)
		return snapshotLog(data), nil
	}
	reconcileOnce(t, r, w)
	finishJob(t, r.Client, w, batchv1.JobComplete, &corev1.Pod{})
	got := reconcileOnce(t, r, w)

	require.NotNil(t, got.Status.Snapshot)
	assert.Equal(t, testCommit, got.Status.Snapshot.Commit)
	require.Len(t, got.Status.Snapshot.ConfigMaps, 2)
	var joined []byte
	for _, name := range got.Status.Snapshot.ConfigMaps {
		var cm corev1.ConfigMap
		require.NoError(t, r.Client.Get(context.Background(), client.ObjectKey{Namespace: testNamespace, Name: name}, &cm))
		assert.LessOrEqual(t, len(cm.BinaryData[snapshotKey]), snapshotPartBytes)
		assert.Equal(t, "a", cm.OwnerReferences[0].Name, "deleted with the RegelHet")
		joined = append(joined, cm.BinaryData[snapshotKey]...)
	}
	assert.Equal(t, data, joined, "the parts join into the tar.gz")

	hr, err := helmRelease(t, r.Client, "a")
	require.NoError(t, err)
	names, _, _ := unstructured.NestedStringSlice(hr.Object, "spec", "values", "case", "snapshot", "configMaps")
	assert.Equal(t, got.Status.Snapshot.ConfigMaps, names)

	_, err = getJob(t, r.Client, w)
	assert.True(t, apierrors.IsNotFound(err), "the Job goes once its copy is stored")
	err = r.Client.Get(context.Background(), client.ObjectKeyFromObject(marked), &corev1.Secret{})
	assert.True(t, apierrors.IsNotFound(err), "a marked token Secret goes after the snapshot")
	assert.Equal(t, v1alpha1.TokenAbsent, got.Status.Token)
	assert.True(t, apimeta.IsStatusConditionTrue(got.Status.Conditions, v1alpha1.ConditionSnapshotReady))

	// Later reconciles keep the snapshot and need no token.
	got = reconcileOnce(t, r, w)
	assert.Equal(t, testCommit, got.Status.Snapshot.Commit)
	assert.NotEqual(t, v1alpha1.PhaseFailed, got.Status.Phase)
}

func TestAnUnmarkedTokenSecretStays(t *testing.T) {
	t.Parallel()
	w := newRegelHet("a", "a")
	w.Spec.Manifest.CredentialsSecretRef = &v1alpha1.SecretReference{Name: "a-repo"}
	secret := &corev1.Secret{ObjectMeta: metav1.ObjectMeta{Name: "a-repo", Namespace: testNamespace}}
	r := newBareReconciler(t, w, secret)
	r.Logs = func(context.Context, string, string, string) ([]byte, error) { return snapshotLog([]byte("tar")), nil }
	reconcileOnce(t, r, w)
	finishJob(t, r.Client, w, batchv1.JobComplete, &corev1.Pod{})
	got := reconcileOnce(t, r, w)
	require.NotNil(t, got.Status.Snapshot)
	require.NoError(t, r.Client.Get(context.Background(), client.ObjectKeyFromObject(secret), &corev1.Secret{}))
	assert.Equal(t, v1alpha1.TokenPresent, got.Status.Token)
}

func TestAFailedJobSaysWhyAndIsRetriedLater(t *testing.T) {
	t.Parallel()
	w := newRegelHet("a", "a")
	r := newBareReconciler(t, w)
	reconcileOnce(t, r, w)
	finishJob(t, r.Client, w, batchv1.JobFailed, &corev1.Pod{Status: corev1.PodStatus{
		ContainerStatuses: []corev1.ContainerStatus{{Name: "snapshot", State: corev1.ContainerState{
			Terminated: &corev1.ContainerStateTerminated{ExitCode: 1, Message: "the manifest bron.yaml is not in commit x of main"},
		}}},
	}})
	got := reconcileOnce(t, r, w)
	assert.Equal(t, v1alpha1.PhaseFailed, got.Status.Phase)
	assert.Equal(t, reasonSnapshotFailed, apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionReady).Reason)
	assert.Contains(t, got.Status.Message, "the manifest bron.yaml is not in commit x of main")
	_, err := getJob(t, r.Client, w)
	require.NoError(t, err, "the failed Job stays a while, with its reason")

	r.Now = func() time.Time { return testNow.Add(snapshotRetry) }
	reconcileOnce(t, r, w)
	_, err = getJob(t, r.Client, w)
	assert.True(t, apierrors.IsNotFound(err), "after snapshotRetry the Job goes, and the next reconcile starts a new one")
}

func TestANewManifestKeepsTheOldSnapshotUntilTheReleaseRunsTheNew(t *testing.T) {
	t.Parallel()
	w := newRegelHet("a", "a")
	r := newReconciler(t, w)
	r.Logs = func(context.Context, string, string, string) ([]byte, error) { return snapshotLog([]byte("new")), nil }
	got := reconcileOnce(t, r, w)
	old := got.Status.Snapshot.ConfigMaps

	got.Spec.Manifest.Ref = "v2"
	require.NoError(t, r.Client.Update(context.Background(), got))
	w = got
	got = reconcileOnce(t, r, w)
	assert.Equal(t, old, got.Status.Snapshot.ConfigMaps, "the release runs the old snapshot while the new one is taken")
	assert.Equal(t, reasonSnapshotPending, apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionSnapshotReady).Reason)

	finishJob(t, r.Client, w, batchv1.JobComplete, &corev1.Pod{})
	got = reconcileOnce(t, r, w)
	current := got.Status.Snapshot.ConfigMaps
	assert.NotEqual(t, old, current)
	exists := func(name string) bool {
		err := r.Client.Get(context.Background(), client.ObjectKey{Namespace: testNamespace, Name: name}, &corev1.ConfigMap{})
		return err == nil
	}
	assert.True(t, exists(old[0]), "the old snapshot stays until the release runs the new one")

	hr, err := helmRelease(t, r.Client, "a")
	require.NoError(t, err)
	require.NoError(t, unstructured.SetNestedSlice(hr.Object, []any{map[string]any{"type": "Ready", "status": "True"}}, "status", "conditions"))
	require.NoError(t, unstructured.SetNestedField(hr.Object, hr.GetGeneration(), "status", "observedGeneration"))
	require.NoError(t, r.Client.Update(context.Background(), hr))
	reconcileOnce(t, r, w)
	assert.False(t, exists(old[0]), "the old snapshot goes once the release runs the new one")
	assert.True(t, exists(current[0]))
}

func TestTheSnapshotAnnotationTakesANewSnapshot(t *testing.T) {
	t.Parallel()
	w := newRegelHet("a", "a")
	before := snapshotSource(w)
	w.Annotations = map[string]string{v1alpha1.SnapshotAnnotation: "2026-11-24"}
	assert.NotEqual(t, before, snapshotSource(w))
	assert.LessOrEqual(t, len(snapshotJobName(&v1alpha1.RegelHet{ObjectMeta: metav1.ObjectMeta{Name: strings.Repeat("a", 40)}}, before)), 63)
}

func TestParseSnapshotLogRefusesAnIncompleteCopy(t *testing.T) {
	t.Parallel()
	full := snapshotLog([]byte("tar.gz"))
	commit, data, err := parseSnapshotLog(full)
	require.NoError(t, err)
	assert.Equal(t, testCommit, commit)
	assert.Equal(t, []byte("tar.gz"), data)

	_, _, err = parseSnapshotLog(bytes.TrimSuffix(full, []byte("regelhet-snapshot end\n")))
	assert.Error(t, err, "a log without its end line")
	_, _, err = parseSnapshotLog(bytes.Replace(full, []byte("bytes=6"), []byte("bytes=7"), 1))
	assert.Error(t, err, "a copy of another size than the header says")
}

// The script itself, against a repository on disk, where git and yq are
// installed; CI has no yq, so there it is skipped and the live test covers it.
func TestSnapshotScriptPacksWhatTheManifestNames(t *testing.T) {
	t.Parallel()
	listed := packSnapshot(t, map[string]string{
		"case/bron.yaml":        "repo_root: ..\nui_spec: ui.yaml\ndirectories:\n  casussen: c\n",
		"ui.yaml":               "{}\n",
		"c/a.yaml":              "a: 1\n",
		"c/beëindiging.yaml":    "b: 1\n",
		"c/notes.md":            "not read by the server\n",
		"elsewhere/secret.yaml": "not named\n",
	})
	assert.Equal(t, []string{"c/a.yaml", "c/beëindiging.yaml", "case/bron.yaml", "ui.yaml"}, listed)
}

// A UI spec in layers (a list under ui_spec) and the message spec and text
// layer beside it travel with the snapshot, as the server reads them
// (packages/corpus/src/source_manifest.rs).
func TestSnapshotScriptPacksTheLayersAndTheMessageSpec(t *testing.T) {
	t.Parallel()
	manifest := "repo_root: ..\nui_spec: [ui/generiek.yaml, ui/eigen.yaml]\n" +
		"berichten: ui/berichten.yaml\nberichtteksten: ui/teksten.yaml\ndirectories:\n  casussen: c\n"
	listed := packSnapshot(t, map[string]string{
		"case/bron.yaml":    manifest,
		"ui/generiek.yaml":  "{}\n",
		"ui/eigen.yaml":     "{}\n",
		"ui/berichten.yaml": "{}\n",
		"ui/teksten.yaml":   "{}\n",
		"ui/other.yaml":     "not named\n",
		"c/a.yaml":          "a: 1\n",
	})
	assert.Equal(t, []string{"c/a.yaml", "case/bron.yaml", "ui/berichten.yaml", "ui/eigen.yaml", "ui/generiek.yaml", "ui/teksten.yaml"}, listed)
}

// packSnapshot commits files to a repository on disk, runs snapshot.sh on
// its case/bron.yaml and lists what the tar.gz holds, in order. Skipped
// without yq or git.
func packSnapshot(t *testing.T, files map[string]string) []string {
	t.Helper()
	yq, err := exec.LookPath("yq")
	if err != nil {
		t.Skip("yq is not installed")
	}
	if _, err := exec.LookPath("git"); err != nil {
		t.Skip("git is not installed")
	}
	dir := t.TempDir()
	repo := filepath.Join(dir, "repo")
	for name, text := range files {
		require.NoError(t, os.MkdirAll(filepath.Dir(filepath.Join(repo, name)), 0o755))
		require.NoError(t, os.WriteFile(filepath.Join(repo, name), []byte(text), 0o600))
	}
	for _, args := range [][]string{{"init", "-q", "-b", "main"}, {"add", "."}, {"-c", "user.name=t", "-c", "user.email=t@example.org", "commit", "-q", "-m", "x"}} {
		require.NoError(t, exec.Command("git", append([]string{"-C", repo}, args...)...).Run())
	}
	work := filepath.Join(dir, "work")
	require.NoError(t, os.Mkdir(work, 0o755))
	cmd := exec.Command("sh", "-c", snapshotScript)
	cmd.Env = append(os.Environ(), "YQ="+yq, "WORK="+work, "TERMINATION_LOG="+filepath.Join(dir, "term"),
		"CASE_REPO=file://"+repo, "CASE_REF=main", "CASE_PATH=case/bron.yaml", "MAX_BYTES=100000")
	out, err := cmd.Output()
	require.NoError(t, err)
	_, data, err := parseSnapshotLog(out)
	require.NoError(t, err)
	gz, err := gzip.NewReader(bytes.NewReader(data))
	require.NoError(t, err)
	var listed []string
	for tr := tar.NewReader(gz); ; {
		h, err := tr.Next()
		if err == io.EOF {
			break
		}
		require.NoError(t, err)
		listed = append(listed, h.Name)
	}
	return listed
}

func TestAPinnedVersionKeepsItsToken(t *testing.T) {
	t.Parallel()
	w := newRegelHet("a", "a")
	w.Spec.Version = strings.Repeat("0", 40)
	w.Spec.Manifest.CredentialsSecretRef = &v1alpha1.SecretReference{Name: "a-repo"}
	marked := &corev1.Secret{ObjectMeta: metav1.ObjectMeta{
		Name: "a-repo", Namespace: testNamespace,
		Annotations: map[string]string{v1alpha1.DeleteAfterSnapshotAnnotation: "true"},
	}}
	r := newBareReconciler(t, w, marked)
	r.Logs = func(context.Context, string, string, string) ([]byte, error) { return snapshotLog([]byte("tar")), nil }
	reconcileOnce(t, r, w)
	finishJob(t, r.Client, w, batchv1.JobComplete, &corev1.Pod{})
	got := reconcileOnce(t, r, w)
	require.NotNil(t, got.Status.Snapshot)
	require.NoError(t, r.Client.Get(context.Background(), client.ObjectKeyFromObject(marked), &corev1.Secret{}),
		"the chart of an older commit clones with the token at every start")
}

func TestACopyThatDoesNotStoreKeepsItsJobAWhile(t *testing.T) {
	t.Parallel()
	w := newRegelHet("a", "a")
	r := newBareReconciler(t, w)
	r.Logs = func(context.Context, string, string, string) ([]byte, error) { return []byte("half a log"), nil }
	reconcileOnce(t, r, w)
	finishJob(t, r.Client, w, batchv1.JobComplete, &corev1.Pod{})
	got := reconcileOnce(t, r, w)
	assert.Equal(t, v1alpha1.PhaseFailed, got.Status.Phase)
	_, err := getJob(t, r.Client, w)
	require.NoError(t, err, "deleting it at once would start the next Job at once")
}

func TestAJobOfAnEarlierManifestGoes(t *testing.T) {
	t.Parallel()
	w := newRegelHet("a", "a")
	r := newBareReconciler(t, w)
	reconcileOnce(t, r, w)
	first, err := getJob(t, r.Client, w)
	require.NoError(t, err)

	var got v1alpha1.RegelHet
	require.NoError(t, r.Client.Get(context.Background(), client.ObjectKeyFromObject(w), &got))
	got.Spec.Manifest.Ref = "v2"
	require.NoError(t, r.Client.Update(context.Background(), &got))
	reconcileOnce(t, r, &got)
	err = r.Client.Get(context.Background(), client.ObjectKeyFromObject(first), &batchv1.Job{})
	assert.True(t, apierrors.IsNotFound(err), "its log holds a copy of the case")
	_, err = getJob(t, r.Client, &got)
	assert.NoError(t, err)
}
