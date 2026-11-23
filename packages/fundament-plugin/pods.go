package main

import (
	"context"
	"fmt"
	"regexp"
	"sort"
	"strings"

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

// ansi matches the colour codes of the server's log lines, which reach the
// termination message as they were written.
var ansi = regexp.MustCompile(`\x1b\[[0-9;]*m`)

// releasePodProblem looks at the pods of w's release and returns the first
// problem it finds, by pod name, or nil when every container is running or
// still on its way.
func releasePodProblem(ctx context.Context, c client.Reader, w *v1alpha1.Wetsproces) (*podProblem, error) {
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
		statuses := append(append([]corev1.ContainerStatus{}, pod.Status.InitContainerStatuses...), pod.Status.ContainerStatuses...)
		for _, s := range statuses {
			if p := containerProblem(s); p != nil {
				return p, nil
			}
		}
	}
	return nil, nil
}

// containerProblem is the problem of one container, or nil.
func containerProblem(s corev1.ContainerStatus) *podProblem {
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
		message = lastLine(t.Message)
		if message == "" {
			message = fmt.Sprintf("stopped with exit code %d", t.ExitCode)
		}
	}
	return &podProblem{Reason: waiting.Reason, Message: fmt.Sprintf("container %s: %s", s.Name, message)}
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
