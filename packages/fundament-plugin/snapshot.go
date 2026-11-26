package main

import (
	"bufio"
	"bytes"
	"context"
	"crypto/sha256"
	_ "embed"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"regexp"
	"sort"
	"strconv"
	"strings"
	"time"

	batchv1 "k8s.io/api/batch/v1"
	corev1 "k8s.io/api/core/v1"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	"k8s.io/apimachinery/pkg/api/resource"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/controller/controllerutil"
	ctrllog "sigs.k8s.io/controller-runtime/pkg/log"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

// The snapshot: the plugin copies the case into the namespace once, at
// install and after a change of the manifest, so the pods of the release
// never fetch it themselves and need no token. A Job fetches the commit and
// writes a tar.gz of the manifest and the files it names to its log
// (snapshot.sh); the plugin reads the log and stores the tar.gz in
// ConfigMaps, which the chart unpacks into /case.

//go:embed snapshot.sh
var snapshotScript string

const (
	// The images of the Job: git with a shell and tar, and yq to read the
	// manifest. The git image is the chart's gitImage.
	snapshotGitImage = "alpine/git:2.54.0@sha256:6a1fdb962814ca11508ca8d1bab94375b5b7ac4e4a9a40981c29fc32c02f095d"
	snapshotYqImage  = "mikefarah/yq:4.47.1@sha256:b9285dd3b0bea3c34d0c54415dd48d767dabd9644d489bd6e253660847b58419"

	// snapshotPartBytes is the most one ConfigMap holds of the tar.gz: a
	// ConfigMap holds 1 MiB at most, its metadata included.
	snapshotPartBytes = 900 * 1024
	// snapshotMaxBytes is the most a snapshot holds, five ConfigMaps. The
	// Job writes it to its log in base64, and the kubelet rotates a log at
	// 10 MiB by default, so the whole of it must stay well under that.
	snapshotMaxBytes = 5 * snapshotPartBytes

	// snapshotRetry is how long a failed Job stays, with its reason in the
	// status, before the plugin tries again: a token may be fixed meanwhile.
	snapshotRetry = 5 * time.Minute
	// snapshotDeadline ends a Job that hangs, such as on a server that does
	// not answer.
	snapshotDeadline = int64(600)
	// tokenWait is how soon a RegelHet whose token Secret is not there yet
	// is looked at again: the wizard makes the Secret after the RegelHet,
	// and the plugin does not watch Secrets.
	tokenWait = 15 * time.Second

	// The component label of the snapshot pods and ConfigMaps.
	snapshotComponent = "case-snapshot"
	// sourceLabel ties a ConfigMap to the manifest it was taken of
	// (snapshotSource).
	sourceLabel = "regelhet.regelrecht.nl/snapshot-source"
	// The annotations on each ConfigMap of a snapshot.
	commitAnnotation = "regelhet.regelrecht.nl/commit"
	partsAnnotation  = "regelhet.regelrecht.nl/parts"
	// loginAnnotation holds the login of the case, as JSON (caseLogin).
	loginAnnotation = "regelhet.regelrecht.nl/login"
	// snapshotKey is the key of the tar.gz part in each ConfigMap.
	snapshotKey = "part"

	// The lines around the tar.gz in the log of the Job (snapshot.sh).
	snapshotHeader = "regelhet-snapshot "
	snapshotEnd    = "regelhet-snapshot end"

	reasonSnapshotPending = "SnapshotPending"
	reasonSnapshotReady   = "SnapshotReady"
	reasonSnapshotFailed  = "SnapshotFailed"
	reasonTokenMissing    = "TokenMissing"
)

// logReader returns the log of one container of a pod.
type logReader func(ctx context.Context, namespace, pod, container string) ([]byte, error)

// snapshotSource is a short hash of what a snapshot is taken of: the
// repository, the ref, the path and the annotation that asks for a new one.
// A change of any of these takes a new snapshot.
func snapshotSource(w *v1alpha1.RegelHet) string {
	m := w.Spec.Manifest
	sum := sha256.Sum256([]byte(strings.Join([]string{
		m.RepoURL, refOrDefault(m.Ref), pathOrDefault(m.Path), w.Annotations[v1alpha1.SnapshotAnnotation],
	}, "\n")))
	return hex.EncodeToString(sum[:])[:10]
}

// snapshotJobName is the Job for source: at most 40 + 20 characters, under
// the 63 of the job-name label.
func snapshotJobName(w *v1alpha1.RegelHet, source string) string {
	return w.Name + "-snapshot-" + source
}

// snapshot is one snapshot in the namespace.
type snapshot struct {
	commit     string
	configMaps []string
	// login is the login of the case in it, for the Keycloak of stable
	// mode (caselogin.go); nil when it did not read.
	login *caseLogin
}

// snapshotState is what reconcileSnapshot found and did.
type snapshotState struct {
	// current is the snapshot of the spec as it is now, or nil while it is
	// being taken or failed.
	current *snapshot
	// condition is SnapshotReady.
	condition metav1.Condition
	// requeue is when to look again, or 0.
	requeue time.Duration
}

// statusSnapshot is the snapshot the status names, which reconcile sets to
// the one the release runs before it applies a release; nil without one.
func statusSnapshot(w *v1alpha1.RegelHet) *snapshot {
	s := w.Status.Snapshot
	if s == nil || len(s.ConfigMaps) == 0 {
		return nil
	}
	return &snapshot{commit: s.Commit, configMaps: s.ConfigMaps}
}

// withParts is annotations with the number of parts.
func withParts(annotations map[string]string, parts int) map[string]string {
	out := map[string]string{partsAnnotation: strconv.Itoa(parts)}
	for k, v := range annotations {
		out[k] = v
	}
	return out
}

// loginOf is the login of the case a ConfigMap of a snapshot names, or nil.
func loginOf(annotations map[string]string) *caseLogin {
	raw, ok := annotations[loginAnnotation]
	if !ok {
		return nil
	}
	var l caseLogin
	if err := json.Unmarshal([]byte(raw), &l); err != nil {
		return nil
	}
	return &l
}

// runningSnapshot is the snapshot the status names, with the login of its
// case from its ConfigMaps: the one the release ran before a new one.
func (r *RegelHetReconciler) runningSnapshot(ctx context.Context, w *v1alpha1.RegelHet) (*snapshot, error) {
	s := statusSnapshot(w)
	if s == nil {
		return nil, nil
	}
	all, err := r.snapshotsOf(ctx, w)
	if err != nil {
		return nil, err
	}
	for _, o := range all {
		if strings.Join(o.configMaps, ",") == strings.Join(s.configMaps, ",") {
			s.login = o.login
		}
	}
	return s, nil
}

// minRequeue is the sooner of two requeues, where 0 is none.
func minRequeue(a, b time.Duration) time.Duration {
	if a == 0 || (b != 0 && b < a) {
		return b
	}
	return a
}

// newSnapshotJob builds the Job that takes the snapshot of source.
func newSnapshotJob(w *v1alpha1.RegelHet, source string) *batchv1.Job {
	m := w.Spec.Manifest
	labels := map[string]string{
		managedByLabel: fieldOwner,
		nameLabel:      w.Name,
		appLabel:       appName,
		componentLabel: snapshotComponent,
		sourceLabel:    source,
	}
	env := []corev1.EnvVar{
		{Name: "CASE_REPO", Value: m.RepoURL},
		{Name: "CASE_REF", Value: refOrDefault(m.Ref)},
		{Name: "CASE_PATH", Value: pathOrDefault(m.Path)},
		{Name: "MAX_BYTES", Value: strconv.Itoa(snapshotMaxBytes)},
		{Name: "HOME", Value: "/tmp"},
		{Name: "GIT_TERMINAL_PROMPT", Value: "0"},
	}
	if ref := m.CredentialsSecretRef; ref != nil {
		for _, kv := range [][2]string{{"GIT_USERNAME", secretUsernameKey}, {"GIT_TOKEN", secretTokenKey}} {
			env = append(env, corev1.EnvVar{Name: kv[0], ValueFrom: &corev1.EnvVarSource{
				SecretKeyRef: &corev1.SecretKeySelector{LocalObjectReference: corev1.LocalObjectReference{Name: ref.Name}, Key: kv[1]},
			}})
		}
	}
	restricted := &corev1.SecurityContext{
		AllowPrivilegeEscalation: new(false),
		ReadOnlyRootFilesystem:   new(true),
		Capabilities:             &corev1.Capabilities{Drop: []corev1.Capability{"ALL"}},
	}
	resources := corev1.ResourceRequirements{
		Requests: corev1.ResourceList{corev1.ResourceCPU: resource.MustParse("50m"), corev1.ResourceMemory: resource.MustParse("64Mi")},
		Limits:   corev1.ResourceList{corev1.ResourceMemory: resource.MustParse("512Mi")},
	}
	emptyDir := func(name, size string) corev1.Volume {
		limit := resource.MustParse(size)
		return corev1.Volume{Name: name, VolumeSource: corev1.VolumeSource{EmptyDir: &corev1.EmptyDirVolumeSource{SizeLimit: &limit}}}
	}
	return &batchv1.Job{
		ObjectMeta: metav1.ObjectMeta{Name: snapshotJobName(w, source), Namespace: w.Namespace, Labels: labels},
		Spec: batchv1.JobSpec{
			// The plugin retries itself, after snapshotRetry, so that the
			// reason of a failure stays readable in between.
			BackoffLimit:          new(int32(0)),
			ActiveDeadlineSeconds: new(snapshotDeadline),
			Template: corev1.PodTemplateSpec{
				ObjectMeta: metav1.ObjectMeta{Labels: labels},
				Spec: corev1.PodSpec{
					RestartPolicy:                corev1.RestartPolicyNever,
					AutomountServiceAccountToken: new(false),
					SecurityContext: &corev1.PodSecurityContext{
						RunAsNonRoot:   new(true),
						RunAsUser:      new(int64(100)),
						RunAsGroup:     new(int64(101)),
						FSGroup:        new(int64(101)),
						SeccompProfile: &corev1.SeccompProfile{Type: corev1.SeccompProfileTypeRuntimeDefault},
					},
					InitContainers: []corev1.Container{{
						Name:            "yq",
						Image:           snapshotYqImage,
						Command:         []string{"cp", "/usr/bin/yq", "/tools/yq"},
						SecurityContext: restricted,
						Resources:       resources,
						VolumeMounts:    []corev1.VolumeMount{{Name: "tools", MountPath: "/tools"}},
					}},
					Containers: []corev1.Container{{
						Name:                     "snapshot",
						Image:                    snapshotGitImage,
						Command:                  []string{"sh", "-c", snapshotScript},
						Env:                      env,
						TerminationMessagePolicy: corev1.TerminationMessageFallbackToLogsOnError,
						SecurityContext:          restricted,
						Resources:                resources,
						VolumeMounts: []corev1.VolumeMount{
							{Name: "tools", MountPath: "/tools", ReadOnly: true},
							{Name: "work", MountPath: "/work"},
							{Name: "tmp", MountPath: "/tmp"},
						},
					}},
					Volumes: []corev1.Volume{emptyDir("tools", "128Mi"), emptyDir("work", "2Gi"), emptyDir("tmp", "64Mi")},
				},
			},
		},
	}
}

// reconcileSnapshot makes sure the namespace holds the snapshot of the spec
// as it is now: it finds it, or runs the Job that takes it and stores what
// the Job wrote.
func (r *RegelHetReconciler) reconcileSnapshot(ctx context.Context, w *v1alpha1.RegelHet) (snapshotState, error) {
	source := snapshotSource(w)
	pending := func(reason, message string) snapshotState {
		return snapshotState{condition: metav1.Condition{
			Type: v1alpha1.ConditionSnapshotReady, Status: metav1.ConditionFalse, Reason: reason, Message: message,
		}}
	}
	if err := r.deleteOtherJobs(ctx, w, source); err != nil {
		return snapshotState{}, err
	}
	all, err := r.snapshotsOf(ctx, w)
	if err != nil {
		return snapshotState{}, err
	}
	if s := all[source]; s != nil {
		state := snapshotState{current: s, condition: metav1.Condition{
			Type: v1alpha1.ConditionSnapshotReady, Status: metav1.ConditionTrue, Reason: reasonSnapshotReady,
			Message: "a copy of commit " + s.commit,
		}}
		// After a crash between storing the copy and cleaning up: the token
		// and the Job that wrote this snapshot may still be there.
		if err := r.deleteToken(ctx, w); err != nil {
			return snapshotState{}, err
		}
		return state, r.deleteJob(ctx, w.Namespace, snapshotJobName(w, source))
	}

	var job batchv1.Job
	err = r.Client.Get(ctx, client.ObjectKey{Namespace: w.Namespace, Name: snapshotJobName(w, source)}, &job)
	if apierrors.IsNotFound(err) {
		// Pending, not failed, while there is no snapshot yet: the wizard
		// makes the Secret just after the RegelHet.
		if ref := w.Spec.Manifest.CredentialsSecretRef; ref != nil && w.Status.Token != v1alpha1.TokenPresent {
			state := pending(reasonTokenMissing, fmt.Sprintf(
				"the Secret %s with the token is not in the namespace; a new copy of the case needs one (the plugin deletes a marked Secret after each copy)", ref.Name))
			state.requeue = tokenWait
			return state, nil
		}
		job := newSnapshotJob(w, source)
		if err := controllerutil.SetControllerReference(w, job, r.Scheme); err != nil {
			return snapshotState{}, fmt.Errorf("own Job %s: %w", job.Name, err)
		}
		if err := r.Client.Create(ctx, job); err != nil && !apierrors.IsAlreadyExists(err) {
			return snapshotState{}, fmt.Errorf("create Job %s/%s: %w", job.Namespace, job.Name, err)
		}
		return pending(reasonSnapshotPending, "copying the case from "+w.Spec.Manifest.RepoURL), nil
	}
	if err != nil {
		return snapshotState{}, fmt.Errorf("read Job %s: %w", snapshotJobName(w, source), err)
	}

	switch {
	case jobCondition(&job, batchv1.JobComplete) != nil:
		s, err := r.storeSnapshot(ctx, w, &job, source)
		if err != nil {
			// The Job stays until snapshotRetry has passed, as a failed one
			// does: deleting it at once would start the next at once.
			state := pending(reasonSnapshotFailed, err.Error())
			done := jobCondition(&job, batchv1.JobComplete)
			if wait := snapshotRetry - r.now().Sub(done.LastTransitionTime.Time); wait > 0 {
				state.requeue = wait
				return state, nil
			}
			return state, r.deleteJob(ctx, w.Namespace, job.Name)
		}
		if err := r.deleteToken(ctx, w); err != nil {
			return snapshotState{}, err
		}
		state := snapshotState{current: s, condition: metav1.Condition{
			Type: v1alpha1.ConditionSnapshotReady, Status: metav1.ConditionTrue, Reason: reasonSnapshotReady,
			Message: "a copy of commit " + s.commit,
		}}
		return state, r.deleteJob(ctx, w.Namespace, job.Name)
	case jobCondition(&job, batchv1.JobFailed) != nil:
		failed := jobCondition(&job, batchv1.JobFailed)
		state := pending(reasonSnapshotFailed, "copying the case failed: "+r.jobFailure(ctx, &job, failed))
		if wait := snapshotRetry - r.now().Sub(failed.LastTransitionTime.Time); wait > 0 {
			state.requeue = wait
			return state, nil
		}
		// Long enough: the next reconcile, which deleting the owned Job
		// sets off, starts a new one.
		return state, r.deleteJob(ctx, w.Namespace, job.Name)
	default:
		return pending(reasonSnapshotPending, "copying the case from "+w.Spec.Manifest.RepoURL), nil
	}
}

func jobCondition(job *batchv1.Job, t batchv1.JobConditionType) *batchv1.JobCondition {
	for i := range job.Status.Conditions {
		c := &job.Status.Conditions[i]
		if c.Type == t && c.Status == corev1.ConditionTrue {
			return c
		}
	}
	return nil
}

// jobPod is the pod of the Job, or nil.
func (r *RegelHetReconciler) jobPod(ctx context.Context, job *batchv1.Job) (*corev1.Pod, error) {
	var pods corev1.PodList
	if err := r.Client.List(ctx, &pods, client.InNamespace(job.Namespace),
		client.MatchingLabels{appLabel: appName, "batch.kubernetes.io/job-name": job.Name}); err != nil {
		return nil, fmt.Errorf("list the pods of Job %s: %w", job.Name, err)
	}
	if len(pods.Items) == 0 {
		return nil, nil
	}
	return &pods.Items[0], nil
}

// jobFailure says why the Job failed: the line snapshot.sh wrote, as the
// status of its pod has it, or else what the Job says.
func (r *RegelHetReconciler) jobFailure(ctx context.Context, job *batchv1.Job, failed *batchv1.JobCondition) string {
	if pod, err := r.jobPod(ctx, job); err == nil && pod != nil {
		statuses := append(append([]corev1.ContainerStatus{}, pod.Status.InitContainerStatuses...), pod.Status.ContainerStatuses...)
		for _, s := range statuses {
			if p := containerProblem(s); p != nil {
				return p.Message
			}
		}
	}
	return fmt.Sprintf("%s: %s", failed.Reason, failed.Message)
}

// storeSnapshot reads what the Job wrote and stores it in ConfigMaps.
func (r *RegelHetReconciler) storeSnapshot(ctx context.Context, w *v1alpha1.RegelHet, job *batchv1.Job, source string) (*snapshot, error) {
	pod, err := r.jobPod(ctx, job)
	if err != nil {
		return nil, err
	}
	if pod == nil || r.Logs == nil {
		return nil, errors.New("the pod of the finished Job is gone, so its copy cannot be read")
	}
	log, err := r.Logs(ctx, pod.Namespace, pod.Name, "snapshot")
	if err != nil {
		return nil, fmt.Errorf("read the log of %s: %w", pod.Name, err)
	}
	commit, data, err := parseSnapshotLog(log)
	if err != nil {
		return nil, err
	}
	annotations := map[string]string{commitAnnotation: commit}
	login, err := readCaseLogin(data, pathOrDefault(w.Spec.Manifest.Path))
	if err != nil {
		// The release runs without it; only the Keycloak of stable mode
		// misses the case's own login key and its test applicants.
		ctrllog.FromContext(ctx).Info("the login of the case does not read from the snapshot", "error", err.Error())
	} else if raw, err := json.Marshal(login); err == nil {
		annotations[loginAnnotation] = string(raw)
	}
	sum := sha256.Sum256(data)
	prefix := w.Name + "-case-" + hex.EncodeToString(sum[:])[:10]
	parts := splitParts(data)
	s := &snapshot{commit: commit, login: login}
	for i, part := range parts {
		cm := &corev1.ConfigMap{
			ObjectMeta: metav1.ObjectMeta{
				Name:      fmt.Sprintf("%s-%d", prefix, i),
				Namespace: w.Namespace,
				Labels: map[string]string{
					managedByLabel: fieldOwner, nameLabel: w.Name, componentLabel: snapshotComponent, sourceLabel: source,
				},
				Annotations: withParts(annotations, len(parts)),
			},
			Immutable:  new(true),
			BinaryData: map[string][]byte{snapshotKey: part},
		}
		if err := controllerutil.SetControllerReference(w, cm, r.Scheme); err != nil {
			return nil, fmt.Errorf("own ConfigMap %s: %w", cm.Name, err)
		}
		if err := r.Client.Create(ctx, cm); err != nil && !apierrors.IsAlreadyExists(err) {
			return nil, fmt.Errorf("create ConfigMap %s/%s: %w", cm.Namespace, cm.Name, err)
		}
		s.configMaps = append(s.configMaps, cm.Name)
	}
	return s, nil
}

// parseSnapshotLog reads the commit and the tar.gz from the log of the Job.
func parseSnapshotLog(log []byte) (string, []byte, error) {
	sc := bufio.NewScanner(bytes.NewReader(log))
	sc.Buffer(make([]byte, 64*1024), 1024*1024)
	var commit string
	size := -1
	var encoded strings.Builder
	inside, done := false, false
	for sc.Scan() {
		line := strings.TrimSpace(sc.Text())
		switch {
		case line == snapshotEnd:
			done = true
		case strings.HasPrefix(line, snapshotHeader):
			for _, field := range strings.Fields(strings.TrimPrefix(line, snapshotHeader)) {
				k, v, _ := strings.Cut(field, "=")
				switch k {
				case "commit":
					commit = v
				case "bytes":
					size, _ = strconv.Atoi(v)
				}
			}
			inside = true
		case inside && !done:
			encoded.WriteString(line)
		}
	}
	if err := sc.Err(); err != nil {
		return "", nil, fmt.Errorf("read the log of the Job: %w", err)
	}
	if !inside || !done || !commitPattern.MatchString(commit) {
		return "", nil, errors.New("the log of the Job holds no complete copy of the case")
	}
	data, err := base64.StdEncoding.DecodeString(encoded.String())
	if err != nil {
		return "", nil, fmt.Errorf("the copy in the log of the Job does not decode: %w", err)
	}
	if len(data) != size {
		return "", nil, fmt.Errorf("the log of the Job holds %d bytes of a copy of %d", len(data), size)
	}
	if len(data) > snapshotMaxBytes {
		return "", nil, fmt.Errorf("the case is %d bytes packed, more than the %d a snapshot holds", len(data), snapshotMaxBytes)
	}
	return commit, data, nil
}

// commitPattern is a full commit hash, SHA-1 or SHA-256.
var commitPattern = regexp.MustCompile(`^([0-9a-f]{40}|[0-9a-f]{64})$`)

func splitParts(data []byte) [][]byte {
	var parts [][]byte
	for len(data) > snapshotPartBytes {
		parts = append(parts, data[:snapshotPartBytes])
		data = data[snapshotPartBytes:]
	}
	return append(parts, data)
}

// snapshotsOf are the complete snapshots of w in the namespace, by source.
// One with a part missing, such as after a crash between two creates, does
// not count; the next Job writes a new one, and deleteOldSnapshots removes
// the parts once the release runs that.
func (r *RegelHetReconciler) snapshotsOf(ctx context.Context, w *v1alpha1.RegelHet) (map[string]*snapshot, error) {
	list, err := r.snapshotConfigMaps(ctx, w)
	if err != nil {
		return nil, err
	}
	type group struct {
		names  []string
		login  *caseLogin
		commit string
		parts  int
	}
	groups := map[string]*group{}
	for _, cm := range list {
		key := cm.Labels[sourceLabel] + "/" + strings.TrimRight(cm.Name, "0123456789")
		g := groups[key]
		if g == nil {
			parts, _ := strconv.Atoi(cm.Annotations[partsAnnotation])
			g = &group{commit: cm.Annotations[commitAnnotation], parts: parts, login: loginOf(cm.Annotations)}
			groups[key] = g
		}
		g.names = append(g.names, cm.Name)
	}
	out := map[string]*snapshot{}
	for key, g := range groups {
		if g.parts == 0 || len(g.names) != g.parts {
			continue
		}
		sort.Slice(g.names, func(i, j int) bool { return partIndex(g.names[i]) < partIndex(g.names[j]) })
		source, _, _ := strings.Cut(key, "/")
		out[source] = &snapshot{commit: g.commit, configMaps: g.names, login: g.login}
	}
	return out, nil
}

func partIndex(name string) int {
	i, _ := strconv.Atoi(name[strings.LastIndex(name, "-")+1:])
	return i
}

// snapshotConfigMaps lists the ConfigMaps of w's snapshots, past the cache:
// the plugin keeps no informer over ConfigMaps.
func (r *RegelHetReconciler) snapshotConfigMaps(ctx context.Context, w *v1alpha1.RegelHet) ([]corev1.ConfigMap, error) {
	var list corev1.ConfigMapList
	if err := r.Uncached.List(ctx, &list, client.InNamespace(w.Namespace),
		client.MatchingLabels{nameLabel: w.Name, componentLabel: snapshotComponent}); err != nil {
		return nil, fmt.Errorf("list the snapshots of %s/%s: %w", w.Namespace, w.Name, err)
	}
	return list.Items, nil
}

// deleteOldSnapshots deletes every snapshot of w but keep, once the release
// runs keep: before that, a restarted pod of the old release still needs the
// old one.
func (r *RegelHetReconciler) deleteOldSnapshots(ctx context.Context, w *v1alpha1.RegelHet, keep *snapshot) error {
	list, err := r.snapshotConfigMaps(ctx, w)
	if err != nil {
		return err
	}
	kept := map[string]bool{}
	for _, name := range keep.configMaps {
		kept[name] = true
	}
	for i := range list {
		cm := &list[i]
		if kept[cm.Name] {
			continue
		}
		if err := r.Client.Delete(ctx, cm); client.IgnoreNotFound(err) != nil {
			return fmt.Errorf("delete the old snapshot %s: %w", cm.Name, err)
		}
	}
	return nil
}

// deleteOtherJobs deletes the snapshot Jobs of w for another source than
// source: a manifest that changed again while its Job ran leaves that Job,
// and its log holds a copy of the case.
func (r *RegelHetReconciler) deleteOtherJobs(ctx context.Context, w *v1alpha1.RegelHet, source string) error {
	var jobs batchv1.JobList
	if err := r.Client.List(ctx, &jobs, client.InNamespace(w.Namespace),
		client.MatchingLabels{nameLabel: w.Name, componentLabel: snapshotComponent}); err != nil {
		return fmt.Errorf("list the snapshot Jobs of %s: %w", w.Name, err)
	}
	for i := range jobs.Items {
		if jobs.Items[i].Labels[sourceLabel] == source {
			continue
		}
		if err := r.deleteJob(ctx, w.Namespace, jobs.Items[i].Name); err != nil {
			return err
		}
	}
	return nil
}

func (r *RegelHetReconciler) deleteJob(ctx context.Context, namespace, name string) error {
	job := &batchv1.Job{ObjectMeta: metav1.ObjectMeta{Namespace: namespace, Name: name}}
	err := r.Client.Delete(ctx, job, client.PropagationPolicy(metav1.DeletePropagationBackground))
	if client.IgnoreNotFound(err) != nil {
		return fmt.Errorf("delete Job %s/%s: %w", namespace, name, err)
	}
	return nil
}

// tokenState is status.token: whether the Secret of credentialsSecretRef is
// in the namespace.
func (r *RegelHetReconciler) tokenState(ctx context.Context, w *v1alpha1.RegelHet) (string, error) {
	ref := w.Spec.Manifest.CredentialsSecretRef
	if ref == nil || r.Uncached == nil {
		return "", nil
	}
	var secret corev1.Secret
	err := r.Uncached.Get(ctx, client.ObjectKey{Namespace: w.Namespace, Name: ref.Name}, &secret)
	switch {
	case apierrors.IsNotFound(err):
		return v1alpha1.TokenAbsent, nil
	case err != nil:
		return "", fmt.Errorf("read Secret %s: %w", ref.Name, err)
	}
	return v1alpha1.TokenPresent, nil
}

// deleteToken deletes the Secret of credentialsSecretRef after a snapshot,
// only when it carries DeleteAfterSnapshotAnnotation "true", and only the
// one that was read (its UID). Never for a RegelHet with a version: the
// chart of a commit from before chart 0.8.0 knows no snapshot and clones
// with the token at every start.
func (r *RegelHetReconciler) deleteToken(ctx context.Context, w *v1alpha1.RegelHet) error {
	ref := w.Spec.Manifest.CredentialsSecretRef
	if ref == nil || r.Uncached == nil || w.Spec.Version != "" {
		return nil
	}
	var secret corev1.Secret
	if err := r.Uncached.Get(ctx, client.ObjectKey{Namespace: w.Namespace, Name: ref.Name}, &secret); err != nil {
		return client.IgnoreNotFound(err)
	}
	if secret.Annotations[v1alpha1.DeleteAfterSnapshotAnnotation] != "true" {
		return nil
	}
	err := r.Client.Delete(ctx, &secret, client.Preconditions{UID: &secret.UID})
	if client.IgnoreNotFound(err) != nil {
		return fmt.Errorf("delete the token Secret %s: %w", ref.Name, err)
	}
	w.Status.Token = v1alpha1.TokenAbsent
	return nil
}
