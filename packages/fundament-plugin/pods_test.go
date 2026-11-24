package main

import (
	"context"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
	corev1 "k8s.io/api/core/v1"
	apimeta "k8s.io/apimachinery/pkg/api/meta"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

// The last lines the server writes when the manifest is not there, colour
// codes and all, as FallbackToLogsOnError hands them over.
const manifestMissing = "\x1b[2m2026-11-23T16:01:02Z\x1b[0m \x1b[31mERROR\x1b[0m wetsproces stopped\n" +
	"wetsproces: the case: the manifest does not read: /case/bron.yaml: No such file or directory\n"

// crashing is a container in a crash loop that has outlasted the start-up
// grace: appRestartsBeforeFailed restarts.
func crashing(name, message string) corev1.ContainerStatus {
	return corev1.ContainerStatus{
		Name:         name,
		RestartCount: appRestartsBeforeFailed,
		State: corev1.ContainerState{Waiting: &corev1.ContainerStateWaiting{
			Reason: "CrashLoopBackOff", Message: "back-off 40s restarting failed container",
		}},
		LastTerminationState: corev1.ContainerState{Terminated: &corev1.ContainerStateTerminated{ExitCode: 1, Message: message}},
	}
}

func TestContainerProblemSaysWhyInTheServersWords(t *testing.T) {
	t.Parallel()
	p := containerProblem(crashing("wetsproces", manifestMissing))
	require.NotNil(t, p)
	assert.Equal(t, "CrashLoopBackOff", p.Reason)
	assert.Equal(t, "container wetsproces: wetsproces: the case: the manifest does not read: /case/bron.yaml: No such file or directory", p.Message)

	noMessage := containerProblem(crashing("clone-case", ""))
	require.NotNil(t, noMessage)
	assert.Equal(t, "container clone-case: stopped with exit code 1", noMessage.Message)

	pull := containerProblem(corev1.ContainerStatus{Name: "wetsproces", State: corev1.ContainerState{Waiting: &corev1.ContainerStateWaiting{
		Reason: "ImagePullBackOff", Message: `Back-off pulling image "registry.example.org/wetsproces:0123"`,
	}}})
	require.NotNil(t, pull)
	assert.Contains(t, pull.Message, "Back-off pulling image")

	for _, reason := range []string{"ContainerCreating", "PodInitializing"} {
		s := corev1.ContainerStatus{State: corev1.ContainerState{Waiting: &corev1.ContainerStateWaiting{Reason: reason}}}
		assert.Nil(t, containerProblem(s), "%s is a pod on its way up", reason)
	}
	assert.Nil(t, containerProblem(corev1.ContainerStatus{State: corev1.ContainerState{Running: &corev1.ContainerStateRunning{}}}))
}

// Between two attempts a failed init container is mostly terminated, not
// waiting: the reason has to show then too. A container that ended well is
// no problem.
func TestContainerProblemSeesAnInitContainerBetweenAttempts(t *testing.T) {
	t.Parallel()
	failed := corev1.ContainerStatus{Name: "clone-case", State: corev1.ContainerState{Terminated: &corev1.ContainerStateTerminated{
		ExitCode: 128, Reason: "Error",
		Message: "remote: invalid credentials\nfatal: Authentication failed for 'https://git.example.org/case/'\n",
	}}}
	p := containerProblem(failed)
	require.NotNil(t, p)
	assert.Equal(t, "Error", p.Reason)
	assert.Equal(t, "container clone-case: fatal: Authentication failed for 'https://git.example.org/case/'", p.Message)

	done := corev1.ContainerStatus{Name: "clone-case", State: corev1.ContainerState{Terminated: &corev1.ContainerStateTerminated{
		ExitCode: 0, Reason: "Completed",
	}}}
	assert.Nil(t, containerProblem(done))
}

func releasePod(name, release string, statuses ...corev1.ContainerStatus) *corev1.Pod {
	return &corev1.Pod{
		ObjectMeta: metav1.ObjectMeta{
			Name: name, Namespace: testNamespace,
			Labels:            map[string]string{appLabel: appName, instanceLabel: release},
			CreationTimestamp: metav1.NewTime(testNow.Add(-10 * time.Second)),
		},
		Status: corev1.PodStatus{ContainerStatuses: statuses},
	}
}

// A manifest that is not there fails the Wetsproces at once with the
// server's reason, while Helm is still waiting; once the pods run, it is
// Ready.
func TestAFailingPodFailsTheWetsprocesBeforeHelmGivesUp(t *testing.T) {
	t.Parallel()
	w := newWetsproces("plugintest", "plugintest")
	pod := releasePod("plugintest-burger-1", "plugintest", crashing("wetsproces", manifestMissing))
	other := releasePod("ander-burger-1", "ander", crashing("wetsproces", "something else"))
	r := newReconciler(t, w, pod, other)

	got := reconcileOnce(t, r, w)
	assert.Equal(t, v1alpha1.PhaseFailed, got.Status.Phase)
	ready := apimeta.FindStatusCondition(got.Status.Conditions, v1alpha1.ConditionReady)
	assert.Equal(t, reasonNotStarting, ready.Reason)
	assert.Contains(t, got.Status.Message, "does not read")
	assert.NotContains(t, got.Status.Message, "something else", "only the pods of this release count")
	fluxObject(t, r.Client, "HelmRelease", "plugintest")

	pod.Status.ContainerStatuses = []corev1.ContainerStatus{{Name: "wetsproces", Ready: true, State: corev1.ContainerState{Running: &corev1.ContainerStateRunning{}}}}
	require.NoError(t, r.Client.Status().Update(context.Background(), pod))
	setReleaseConditions(t, r.Client, "plugintest", map[string]any{"type": "Ready", "status": "True", "message": "Helm install succeeded"})
	got = reconcileOnce(t, r, w)
	assert.Equal(t, v1alpha1.PhaseReady, got.Status.Phase)
}

func TestAPodMapsToTheWetsprocesOfItsRelease(t *testing.T) {
	t.Parallel()
	pod := releasePod("plugintest-burger-1", "plugintest")
	mapped := podRequests(pod)
	require.Len(t, mapped, 1)
	assert.Equal(t, testNamespace+"/plugintest", mapped[0].String())

	foreign := releasePod("x", "plugintest")
	foreign.Labels[appLabel] = "iets-anders"
	assert.Empty(t, podRequests(foreign))
}

func TestTheReleaseWaitsThreeMinutes(t *testing.T) {
	t.Parallel()
	hr := newHelmRelease(newWetsproces("a", "a"), validConfig())
	timeout, _, _ := unstructured.NestedString(hr.Object, "spec", "timeout")
	assert.Equal(t, "3m", timeout)
}

// A server that restarts while what it needs is still starting, as in
// stable mode while Keycloak comes up, is not reported; the same crash is,
// from its fifth restart, or once the pod is three minutes old.
func TestACrashWaitsForTheStartUpGrace(t *testing.T) {
	t.Parallel()
	ctx := context.Background()
	w := newWetsproces("a", "a")
	young := crashing("wetsproces", "wetsproces: the login: OIDC discovery failed: connection refused")
	young.RestartCount = appRestartsBeforeFailed - 1
	pod := releasePod("a-burger-1", "a", young)
	r := newReconciler(t, w, pod)

	p, err := releasePodProblem(ctx, r.Client, w, testNow)
	require.NoError(t, err)
	assert.Nil(t, p, "four restarts in its first seconds is a server waiting for Keycloak")

	p, err = releasePodProblem(ctx, r.Client, w, testNow.Add(appStartupGrace))
	require.NoError(t, err)
	require.NotNil(t, p, "after the grace the same crash is reported")
	assert.Contains(t, p.Message, "OIDC discovery failed")

	young.RestartCount = appRestartsBeforeFailed
	pod.Status.ContainerStatuses = []corev1.ContainerStatus{young}
	require.NoError(t, r.Client.Status().Update(ctx, pod))
	p, err = releasePodProblem(ctx, r.Client, w, testNow)
	require.NoError(t, err)
	assert.NotNil(t, p, "the fifth restart is reported however young the pod")
}

// What waits for nothing that is starting is reported at once: an init
// container that fails, and an image that does not pull.
func TestNoGraceForAFailedInitContainerOrImage(t *testing.T) {
	t.Parallel()
	ctx := context.Background()
	w := newWetsproces("a", "a")
	clone := releasePod("a-burger-1", "a")
	clone.Status.InitContainerStatuses = []corev1.ContainerStatus{{Name: "clone-case", State: corev1.ContainerState{
		Terminated: &corev1.ContainerStateTerminated{ExitCode: 128, Reason: "Error", Message: "fatal: Authentication failed\n"},
	}}}
	pull := releasePod("b-burger-1", "b", corev1.ContainerStatus{Name: "wetsproces", State: corev1.ContainerState{
		Waiting: &corev1.ContainerStateWaiting{Reason: "ImagePullBackOff", Message: "Back-off pulling image"},
	}})
	r := newReconciler(t, w, clone, pull)

	p, err := releasePodProblem(ctx, r.Client, w, testNow)
	require.NoError(t, err)
	require.NotNil(t, p)
	assert.Equal(t, "container clone-case: fatal: Authentication failed", p.Message)

	p, err = releasePodProblem(ctx, r.Client, newWetsproces("b", "b"), testNow)
	require.NoError(t, err)
	require.NotNil(t, p)
	assert.Equal(t, "ImagePullBackOff", p.Reason)
}
