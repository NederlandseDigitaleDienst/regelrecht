package main

import (
	"context"
	"fmt"
	"regexp"
	"sort"
	"strings"
	"time"

	corev1 "k8s.io/api/core/v1"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/handler"
	"sigs.k8s.io/controller-runtime/pkg/reconcile"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

// The labels the chart puts on every pod of a release (charts/wetsproces,
// wetsproces.selectorLabels): the app, and the release, which carries the
// name of the Wetsproces.
const (
	appLabel      = "app.kubernetes.io/name"
	appName       = "wetsproces"
	instanceLabel = "app.kubernetes.io/instance"
	// componentLabel marks the part of a release, such as the Keycloak
	// Secret the chart's Job labels "keycloak".
	componentLabel = "app.kubernetes.io/component"
)

// podProblem is why the pods of a release do not start, as the kubelet
// reports it: a container that keeps stopping, an image it cannot pull, a
// Secret that is not there. Helm would wait for its install timeout before
// it says anything; the pods say it within seconds.
type podProblem struct {
	// Reason is the kubelet's reason, such as CrashLoopBackOff or
	// ImagePullBackOff.
	Reason string
	// Message names the container and says what went wrong: the last line
	// it wrote before it stopped, or the kubelet's message.
	Message string
}

// waitingProblems are the waiting reasons that do not pass on their own.
// ContainerCreating and PodInitializing are left out: they are a pod on its
// way up.
var waitingProblems = map[string]bool{
	"CrashLoopBackOff":           true,
	"ImagePullBackOff":           true,
	"ErrImagePull":               true,
	"InvalidImageName":           true,
	"CreateContainerConfigError": true,
	"CreateContainerError":       true,
}

// A wetsproces server stops at start while the databases, or in stable mode
// its Keycloak, do not answer yet, and Kubernetes starts it again: in the
// live test of stable mode it restarted four times in its first minute. So a
// crash of the server is reported once it has restarted this often, or once
// its pod is older than appStartupGrace; a missing manifest gets there in
// about two and a half minutes, before Helm's install timeout. An init
// container, an image that does not pull and a missing Secret wait for
// nothing that is still starting, so those are reported at once.
const (
	appRestartsBeforeFailed = 5
	appStartupGrace         = 3 * time.Minute
)

// ansi matches the colour codes of the server's log lines, which reach the
// termination message as they were written.
var ansi = regexp.MustCompile(`\x1b\[[0-9;]*m`)

// releasePodProblem looks at the pods of w's release and returns the first
// problem it finds, by pod name, or nil when every container is running or
// still on its way. now is the time the start-up grace is measured against.
func releasePodProblem(ctx context.Context, c client.Reader, w *v1alpha1.Wetsproces, now time.Time) (*podProblem, error) {
	var pods corev1.PodList
	if err := c.List(ctx, &pods, client.InNamespace(w.Namespace),
		client.MatchingLabels{appLabel: appName, instanceLabel: w.Name}); err != nil {
		return nil, fmt.Errorf("list the pods of %s/%s: %w", w.Namespace, w.Name, err)
	}
	sort.Slice(pods.Items, func(i, j int) bool { return pods.Items[i].Name < pods.Items[j].Name })
	for i := range pods.Items {
		pod := &pods.Items[i]
		if !pod.DeletionTimestamp.IsZero() {
			continue
		}
		for _, s := range pod.Status.InitContainerStatuses {
			if p := containerProblem(s); p != nil {
				return p, nil
			}
		}
		started := now.Sub(pod.CreationTimestamp.Time) >= appStartupGrace
		for _, s := range pod.Status.ContainerStatuses {
			p := containerProblem(s)
			if p == nil {
				continue
			}
			if isCrash(p) && s.RestartCount < appRestartsBeforeFailed && !started {
				// Possibly a server waiting for what starts beside it.
				continue
			}
			return p, nil
		}
	}
	return nil, nil
}

// isCrash reports whether p is a container that stopped, as opposed to one
// that could not be made or pulled.
func isCrash(p *podProblem) bool {
	return p.Reason == "CrashLoopBackOff" || p.Reason == "Error"
}

// containerProblem is the problem of one container, or nil.
func containerProblem(s corev1.ContainerStatus) *podProblem {
	// A failed init container, such as clone-case with a token the git
	// server refuses, spends most of its back-off as terminated with an
	// error (Init:Error) and only moments as waiting in CrashLoopBackOff.
	// Looking at waiting alone would say why only now and then, and
	// "Pending" in between.
	if t := s.State.Terminated; t != nil && t.ExitCode != 0 {
		reason := t.Reason
		if reason == "" {
			reason = "Error"
		}
		return &podProblem{Reason: reason, Message: fmt.Sprintf("container %s: %s", s.Name, terminationMessage(t))}
	}
	waiting := s.State.Waiting
	if waiting == nil || !waitingProblems[waiting.Reason] {
		return nil
	}
	message := waiting.Message
	// A container that stopped says why in its termination message, which
	// the chart fills from its last lines (FallbackToLogsOnError). That is
	// the line a person needs: "the manifest ... does not read", not
	// "back-off restarting failed container".
	if t := s.LastTerminationState.Terminated; t != nil {
		message = terminationMessage(t)
	}
	return &podProblem{Reason: waiting.Reason, Message: fmt.Sprintf("container %s: %s", s.Name, message)}
}

// terminationMessage is the last line a stopped container wrote, or its
// exit code when it wrote nothing.
func terminationMessage(t *corev1.ContainerStateTerminated) string {
	if message := lastLine(t.Message); message != "" {
		return message
	}
	return fmt.Sprintf("stopped with exit code %d", t.ExitCode)
}

// lastLine is the last line of text that is not empty, without colour codes.
func lastLine(text string) string {
	lines := strings.Split(ansi.ReplaceAllString(text, ""), "\n")
	for i := len(lines) - 1; i >= 0; i-- {
		if line := strings.TrimSpace(lines[i]); line != "" {
			return line
		}
	}
	return ""
}

// podToWetsproces maps a pod of a release to the Wetsproces of that release,
// which has the release's name, in the pod's namespace.
func podToWetsproces() handler.EventHandler {
	return handler.EnqueueRequestsFromMapFunc(func(_ context.Context, obj client.Object) []reconcile.Request {
		return podRequests(obj)
	})
}

// podRequests is the Wetsproces a pod belongs to, if it is a wetsproces pod.
func podRequests(obj client.Object) []reconcile.Request {
	name := obj.GetLabels()[instanceLabel]
	if name == "" || obj.GetLabels()[appLabel] != appName {
		return nil
	}
	return []reconcile.Request{{NamespacedName: client.ObjectKey{Namespace: obj.GetNamespace(), Name: name}}}
}
