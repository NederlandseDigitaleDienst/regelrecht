package main

import (
	"context"
	"fmt"
	"sync"
	"time"

	batchv1 "k8s.io/api/batch/v1"
	corev1 "k8s.io/api/core/v1"
	apiextensionsv1 "k8s.io/apiextensions-apiserver/pkg/apis/apiextensions/v1"
	"k8s.io/apimachinery/pkg/apis/meta/v1/unstructured"
	"k8s.io/apimachinery/pkg/labels"
	"k8s.io/apimachinery/pkg/runtime"
	"k8s.io/apimachinery/pkg/runtime/schema"
	"k8s.io/client-go/kubernetes"
	clientgoscheme "k8s.io/client-go/kubernetes/scheme"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/cache"
	"sigs.k8s.io/controller-runtime/pkg/client"
	metricsserver "sigs.k8s.io/controller-runtime/pkg/metrics/server"

	"github.com/fundament-oss/fundament/plugin-sdk/pluginruntime"
	pluginerrors "github.com/fundament-oss/fundament/plugin-sdk/pluginruntime/errors"
	crhelper "github.com/fundament-oss/fundament/plugin-sdk/pluginruntime/helpers/controllerruntime"

	"code.digitaledienst.overheid.nl/regelrecht/fieldlab-2026/packages/fundament-plugin/api/v1alpha1"
)

// setUpRetryInterval is how long Start waits before it retries a failed
// set-up, such as a cluster without Flux yet.
const setUpRetryInterval = 30 * time.Second

// RegelHetPlugin offers RegelHet instances to the projects of a cluster. It is
// installed once per cluster and installs the RegelHet CRD; each RegelHet
// object is one RegelHet, which the plugin's controller runs with
// Flux: a GitRepository for the chart and a HelmRelease per object, in the
// object's namespace. Flux keeps them running while the plugin pod is down.
type RegelHetPlugin struct {
	initOnce sync.Once
	initErr  error
	cfg      pluginConfig
	scheme   *runtime.Scheme
	// kube is an uncached client, for the set-up and the uninstall.
	kube client.Client

	// mu serializes the set-up, Reconcile and Uninstall, which the runtime
	// may call at once; uninstalling stops the first two from putting back
	// what an uninstall removes.
	mu           sync.Mutex
	uninstalling bool
}

// NewRegelHetPlugin creates a new RegelHetPlugin.
func NewRegelHetPlugin() *RegelHetPlugin {
	return &RegelHetPlugin{}
}

func buildScheme() (*runtime.Scheme, error) {
	scheme := runtime.NewScheme()
	for _, add := range []func(*runtime.Scheme) error{
		clientgoscheme.AddToScheme,
		apiextensionsv1.AddToScheme,
		v1alpha1.AddToScheme,
	} {
		if err := add(scheme); err != nil {
			return nil, fmt.Errorf("build scheme: %w", err)
		}
	}
	return scheme, nil
}

// init loads the config and builds the client once. Start and the uninstall
// RPC can both arrive first, so both call it.
func (p *RegelHetPlugin) init() error {
	p.initOnce.Do(func() {
		cfg, err := loadConfig()
		if err != nil {
			p.initErr = pluginerrors.NewPermanent(err)
			return
		}
		scheme, err := buildScheme()
		if err != nil {
			p.initErr = pluginerrors.NewPermanent(err)
			return
		}
		kube, err := client.New(ctrl.GetConfigOrDie(), client.Options{Scheme: scheme})
		if err != nil {
			p.initErr = pluginerrors.NewPermanent(fmt.Errorf("create kubernetes client: %w", err))
			return
		}
		p.cfg, p.scheme, p.kube = cfg, scheme, kube
	})
	return p.initErr
}

// Start sets up the CRD and the roles, then runs the controller until ctx is
// cancelled.
//
// Start must be idempotent: the container is restarted on upgrades, node
// evictions and crashes, so this runs again from scratch every time. The
// set-up is a server-side apply, and the controller reconciles every
// RegelHet again when it starts.
func (p *RegelHetPlugin) Start(ctx context.Context, host pluginruntime.Host) error {
	if err := p.init(); err != nil {
		host.ReportStatus(pluginruntime.PluginStatus{Phase: pluginruntime.PhaseFailed, Message: err.Error()})
		return err
	}
	host.ReportStatus(pluginruntime.PluginStatus{Phase: pluginruntime.PhaseInstalling, Message: "installing the CRDs"})
	// Retry here rather than return: the runtime ends the process on an
	// error from Start, and a pod in a crash loop reports nothing.
	for {
		err := p.setUpOnce(ctx)
		if err == nil {
			break
		}
		host.Logger().Error("set up", "error", err)
		host.ReportStatus(pluginruntime.PluginStatus{Phase: pluginruntime.PhaseDegraded, Message: err.Error()})
		select {
		case <-ctx.Done():
			return nil
		case <-time.After(setUpRetryInterval):
		}
	}

	mgr, err := p.newManager()
	if err != nil {
		host.ReportStatus(pluginruntime.PluginStatus{Phase: pluginruntime.PhaseFailed, Message: err.Error()})
		return pluginerrors.NewPermanent(err)
	}
	host.ReportReady()
	host.ReportStatus(pluginruntime.PluginStatus{Phase: pluginruntime.PhaseRunning, Message: "reconciling regelhets"})
	if err := mgr.Start(ctx); err != nil {
		return fmt.Errorf("controller manager stopped: %w", pluginerrors.NewTransient(err))
	}
	return nil
}

func (p *RegelHetPlugin) setUpOnce(ctx context.Context) error {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.uninstalling {
		return nil
	}
	return setUp(ctx, p.kube)
}

// newManager builds the controller manager with the RegelHet and the
// Organisatie controller. Its cache holds the objects of both kinds and only
// the Flux objects the plugin made, by their label.
func (p *RegelHetPlugin) newManager() (ctrl.Manager, error) {
	mine := cache.ByObject{Label: labels.SelectorFromSet(labels.Set{managedByLabel: fieldOwner})}
	byObject := map[client.Object]cache.ByObject{}
	for _, gvk := range []schema.GroupVersionKind{gitRepositoryGVK, helmReleaseGVK} {
		obj := &unstructured.Unstructured{}
		obj.SetGroupVersionKind(gvk)
		byObject[obj] = mine
	}
	// Only the pods of RegelHet releases, which the chart labels, and of
	// the snapshot Jobs, which the plugin labels the same.
	byObject[&corev1.Pod{}] = cache.ByObject{Label: labels.SelectorFromSet(labels.Set{appLabel: appName})}
	byObject[&batchv1.Job{}] = mine
	mgr, err := crhelper.SetupManager(p.scheme, &ctrl.Options{
		// The plugin host owns the lifecycle; no listeners of our own.
		Metrics:                metricsserver.Options{BindAddress: "0"},
		HealthProbeBindAddress: "0",
		Cache:                  cache.Options{ByObject: byObject},
		Client:                 client.Options{Cache: &client.CacheOptions{Unstructured: true}},
	})
	if err != nil {
		return nil, fmt.Errorf("set up the controller manager: %w", err)
	}
	clientset, err := kubernetes.NewForConfig(mgr.GetConfig())
	if err != nil {
		return nil, fmt.Errorf("create the client for pod logs: %w", err)
	}
	r := &RegelHetReconciler{
		Client:     mgr.GetClient(),
		Uncached:   mgr.GetAPIReader(),
		Namespaces: mgr.GetAPIReader(),
		Logs:       podLogs(clientset),
		Scheme:     p.scheme,
		Config:     p.cfg,
	}
	if err := r.SetupWithManager(mgr); err != nil {
		return nil, err
	}
	o := &OrganisatieReconciler{
		Client:     mgr.GetClient(),
		Namespaces: mgr.GetAPIReader(),
		Secrets:    mgr.GetAPIReader(),
		Scheme:     p.scheme,
		Config:     p.cfg,
	}
	if err := o.SetupWithManager(mgr); err != nil {
		return nil, err
	}
	return mgr, nil
}

// podLogs reads the log of a container through the API server, which needs
// get on pods/log.
func podLogs(clientset kubernetes.Interface) logReader {
	return func(ctx context.Context, namespace, pod, container string) ([]byte, error) {
		raw, err := clientset.CoreV1().Pods(namespace).GetLogs(pod, &corev1.PodLogOptions{Container: container}).DoRaw(ctx)
		if err != nil {
			return nil, fmt.Errorf("read the log of %s/%s: %w", namespace, pod, err)
		}
		return raw, nil
	}
}

// Shutdown performs graceful cleanup. It must not uninstall anything: shutdown
// happens on every restart, not only on uninstall.
func (p *RegelHetPlugin) Shutdown(_ context.Context) error {
	return nil
}

// Install sets up the CRD and the roles; Start does the same, so an install
// that arrives first is no different.
func (p *RegelHetPlugin) Install(ctx context.Context, _ pluginruntime.Host) error {
	if err := p.init(); err != nil {
		return err
	}
	if err := p.setUpOnce(ctx); err != nil {
		return pluginerrors.NewTransient(err)
	}
	return nil
}

// Upgrade applies the CRD and the roles of the new version.
func (p *RegelHetPlugin) Upgrade(ctx context.Context, host pluginruntime.Host) error {
	return p.Install(ctx, host)
}

// Uninstall removes the roles that let project members make RegelHet instances.
// It keeps the CRD and every RegelHet: deleting the CRD would delete them
// all and, through their owner references, every running RegelHet
// with its data. Flux keeps them running; nobody reconciles a change until the
// plugin is installed again. Removing them is a deliberate step (README).
func (p *RegelHetPlugin) Uninstall(ctx context.Context, _ pluginruntime.Host) error {
	if err := p.init(); err != nil {
		return err
	}
	p.mu.Lock()
	defer p.mu.Unlock()
	p.uninstalling = true
	if err := deleteUserRoles(ctx, p.kube); err != nil {
		return pluginerrors.NewTransient(err)
	}
	return nil
}

// Reconcile repairs the CRD and the roles, and reports how many RegelHet instances
// run, every FUNP_RECONCILE_INTERVAL (default 5m). The controller reconciles
// each RegelHet on its own events.
func (p *RegelHetPlugin) Reconcile(ctx context.Context, host pluginruntime.Host) error {
	if p.kube == nil {
		return nil
	}
	if err := p.setUpOnce(ctx); err != nil {
		host.ReportStatus(pluginruntime.PluginStatus{Phase: pluginruntime.PhaseDegraded, Message: err.Error()})
		return fmt.Errorf("reconcile: %w", pluginerrors.NewTransient(err))
	}
	var all v1alpha1.RegelHetList
	if err := p.kube.List(ctx, &all); err != nil {
		return fmt.Errorf("reconcile: list regelhets: %w", pluginerrors.NewTransient(err))
	}
	host.ReportStatus(summary(all.Items))
	return nil
}

// summary is the plugin status for the RegelHet instances on the cluster. A
// failing one does not make the plugin degraded: it is that object's spec or
// release, and its own status says which.
func summary(items []v1alpha1.RegelHet) pluginruntime.PluginStatus {
	ready := 0
	for i := range items {
		if items[i].Status.Phase == v1alpha1.PhaseReady {
			ready++
		}
	}
	return pluginruntime.PluginStatus{
		Phase:   pluginruntime.PhaseRunning,
		Message: fmt.Sprintf("%d of %d RegelHet instances ready", ready, len(items)),
	}
}
