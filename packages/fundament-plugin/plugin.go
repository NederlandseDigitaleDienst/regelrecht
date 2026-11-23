package main

import (
	"context"
	"fmt"
	"sync"
	"time"

	"k8s.io/apimachinery/pkg/runtime"
	clientgoscheme "k8s.io/client-go/kubernetes/scheme"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"

	"github.com/fundament-oss/fundament/plugin-sdk/pluginruntime"
	pluginerrors "github.com/fundament-oss/fundament/plugin-sdk/pluginruntime/errors"
)

const (
	statusPollInterval = 15 * time.Second
	uninstallTimeout   = 5 * time.Minute
)

// WetsprocesPlugin runs the wetsprocessysteem for one case. It is a thin
// layer over Flux, which the cluster already has: in an existing Fundament
// namespace it creates a GitRepository for the chart and a HelmRelease with the
// install config as values. Flux installs the chart, and keeps it running when the plugin pod is
// down.
type WetsprocesPlugin struct {
	initOnce  sync.Once
	initErr   error
	cfg       pluginConfig
	k8sClient client.Client
	status    statusReporter
	// pollInterval is how often the plugin reads the Flux status.
	pollInterval time.Duration
	// deletePollInterval is how often Uninstall checks that an object is gone.
	deletePollInterval time.Duration
}

// NewWetsprocesPlugin creates a new WetsprocesPlugin.
func NewWetsprocesPlugin() *WetsprocesPlugin {
	return &WetsprocesPlugin{
		pollInterval:       statusPollInterval,
		deletePollInterval: 2 * time.Second,
	}
}

// init loads the config and builds the Kubernetes client once. Start and the
// uninstall RPC can both arrive first, so both call it.
func (p *WetsprocesPlugin) init() error {
	p.initOnce.Do(func() {
		cfg, err := loadConfig()
		if err != nil {
			p.initErr = pluginerrors.NewPermanent(err)
			return
		}
		p.cfg = cfg

		scheme := runtime.NewScheme()
		if err := clientgoscheme.AddToScheme(scheme); err != nil {
			p.initErr = pluginerrors.NewPermanent(fmt.Errorf("add client-go types to scheme: %w", err))
			return
		}
		k8sClient, err := client.New(ctrl.GetConfigOrDie(), client.Options{Scheme: scheme})
		if err != nil {
			p.initErr = pluginerrors.NewPermanent(fmt.Errorf("create kubernetes client: %w", err))
			return
		}
		p.k8sClient = k8sClient
	})
	return p.initErr
}

// Start installs the Flux objects, then reports the Flux status until ctx is
// cancelled.
//
// Start must be idempotent: the container is restarted on upgrades, node
// evictions and crashes, so this runs again from scratch every time. Every step
// it takes is create-if-missing or a server-side apply.
func (p *WetsprocesPlugin) Start(ctx context.Context, host pluginruntime.Host) error {
	if err := p.init(); err != nil {
		p.status.direct(host, pluginruntime.PluginStatus{Phase: pluginruntime.PhaseFailed, Message: err.Error()})
		return err
	}
	host.ReportReady()

	p.status.direct(host, pluginruntime.PluginStatus{Phase: pluginruntime.PhaseInstalling, Message: "applying the GitRepository and the HelmRelease"})
	if err := p.Install(ctx, host); err != nil {
		p.status.direct(host, errorStatus(err))
		return fmt.Errorf("install wetsproces: %w", err)
	}

	p.watchFlux(ctx, host)
	return nil
}

// watchFlux reports the Flux status every pollInterval until ctx ends.
func (p *WetsprocesPlugin) watchFlux(ctx context.Context, host pluginruntime.Host) {
	ticker := time.NewTicker(p.pollInterval)
	defer ticker.Stop()
	for {
		if s, err := readFluxStatus(ctx, p.k8sClient, p.cfg.Namespace); err == nil {
			p.status.flux(host, s)
		}
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
		}
	}
}

// Shutdown performs graceful cleanup. It must not uninstall anything: shutdown
// happens on every restart, not only on uninstall.
func (p *WetsprocesPlugin) Shutdown(_ context.Context) error {
	return nil
}

// Install checks that the namespace exists and applies the GitRepository and
// the HelmRelease.
func (p *WetsprocesPlugin) Install(ctx context.Context, _ pluginruntime.Host) error {
	if err := p.init(); err != nil {
		return err
	}
	if err := requireNamespace(ctx, p.k8sClient, p.cfg.Namespace); err != nil {
		return err
	}
	return applyFluxObjects(ctx, p.k8sClient, p.cfg)
}

// Uninstall deletes the HelmRelease and waits until helm-controller has
// uninstalled the release, then deletes the GitRepository. The namespace is
// Fundament's and stays.
func (p *WetsprocesPlugin) Uninstall(ctx context.Context, _ pluginruntime.Host) error {
	if err := p.init(); err != nil {
		return err
	}
	ctx, cancel := context.WithTimeout(ctx, uninstallTimeout)
	defer cancel()

	ns := p.cfg.Namespace
	if err := deleteObject(ctx, p.k8sClient, helmReleaseGVK, ns, releaseName); err != nil {
		return err
	}
	// helm-controller uninstalls the Helm release while it handles the
	// HelmRelease's finalizer; the source goes only after that.
	if err := p.waitGone(ctx, func(ctx context.Context) (bool, error) {
		return objectGone(ctx, p.k8sClient, helmReleaseGVK, ns, releaseName)
	}); err != nil {
		return err
	}
	return deleteObject(ctx, p.k8sClient, gitRepositoryGVK, ns, sourceName)
}

func (p *WetsprocesPlugin) waitGone(ctx context.Context, gone func(context.Context) (bool, error)) error {
	ticker := time.NewTicker(p.deletePollInterval)
	defer ticker.Stop()
	for {
		ok, err := gone(ctx)
		if err == nil && ok {
			return nil
		}
		select {
		case <-ctx.Done():
			return pluginerrors.NewTransient(fmt.Errorf("wait for HelmRelease deletion: %w", ctx.Err()))
		case <-ticker.C:
		}
	}
}

// Upgrade re-runs the install: a server-side apply of the same objects.
func (p *WetsprocesPlugin) Upgrade(ctx context.Context, host pluginruntime.Host) error {
	return p.Install(ctx, host)
}

// Reconcile re-applies the namespace and the Flux objects and reports the Flux
// status every FUNP_RECONCILE_INTERVAL (default 5m).
func (p *WetsprocesPlugin) Reconcile(ctx context.Context, host pluginruntime.Host) error {
	if p.k8sClient == nil {
		return nil
	}
	if err := p.Install(ctx, host); err != nil {
		p.reconcileFailed(host, err)
		return fmt.Errorf("reconcile: %w", err)
	}
	s, err := readFluxStatus(ctx, p.k8sClient, p.cfg.Namespace)
	if err != nil {
		err = pluginerrors.NewTransient(err)
		p.reconcileFailed(host, err)
		return fmt.Errorf("reconcile: %w", err)
	}
	p.status.reconciled(host, s.pluginStatus(), nil)
	return nil
}

func (p *WetsprocesPlugin) reconcileFailed(host pluginruntime.Host, err error) {
	p.status.reconciled(host, errorStatus(err), err)
}

// errorStatus reports a permanent error as failed and anything else as
// degraded.
func errorStatus(err error) pluginruntime.PluginStatus {
	phase := pluginruntime.PhaseDegraded
	if pluginerrors.IsPermanent(err) {
		phase = pluginruntime.PhaseFailed
	}
	return pluginruntime.PluginStatus{Phase: phase, Message: err.Error()}
}
