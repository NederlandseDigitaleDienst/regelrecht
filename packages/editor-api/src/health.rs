//! Liveness, readiness and a graceful stop, for the orchestrator that runs the
//! editor.
//!
//! Two probes, two questions:
//!
//! - `/health` ([`live`]) answers whether the process still serves HTTP. It
//!   checks nothing else, so a database outage never gets the pod restarted:
//!   a restart would not bring the database back.
//! - `/health/ready` ([`ready`]) answers whether this pod should get new requests. It
//!   is ready from the moment the port opens (the corpus and the migrations
//!   finish before that) until shutdown starts. It does not check the
//!   database either: the corpus routes work without one, and a readiness
//!   probe that follows a shared dependency takes every pod out at once,
//!   turning a partial outage into a full one.
//!
//! The stop has two halves. [`watch_stop`] takes the signal: before the port
//! opens it exits the process straight away, since nothing is routed to it
//! yet; once serving it marks the pod not ready and hands the signal on.
//! [`drain_on`] then keeps serving for [`DRAIN_DELAY`] while the endpoint is
//! taken out of the routing, and only then lets the server stop accepting.
//! Requests in flight finish; `axum::serve` waits for them.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use tokio::sync::oneshot;

use crate::state::AppState;

/// How long a stopping pod keeps serving after it reports not ready.
///
/// Kubernetes sends SIGTERM and takes the pod out of the service endpoints at
/// the same time, and the ingress learns of the second a moment later.
/// Requests that arrive in between still reach this pod; closing the listener
/// straight away would refuse them. Five seconds stays well inside the
/// default grace period of thirty.
pub const DRAIN_DELAY: Duration = Duration::from_secs(5);

const STARTING: u8 = 0;
const SERVING: u8 = 1;
const DRAINING: u8 = 2;

/// Where the process is in its life: starting, serving or draining. It only
/// moves forward. Clones share the state.
#[derive(Clone, Debug, Default)]
pub struct Readiness {
    state: Arc<AtomicU8>,
}

/// What to do about a stop signal, decided by [`Readiness::on_stop`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopAction {
    /// The server never started: exit now.
    Exit,
    /// The server runs: drain it.
    Drain,
}

impl Readiness {
    /// The listener is bound: report ready from now on. Returns `false` when a
    /// stop came first, in which case the state stays as the stop left it.
    pub fn start_serving(&self) -> bool {
        self.state
            .compare_exchange(STARTING, SERVING, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// A stop signal arrived: report not ready from now on, and say whether
    /// there is a server to drain.
    pub fn on_stop(&self) -> StopAction {
        match self.state.swap(DRAINING, Ordering::AcqRel) {
            STARTING => StopAction::Exit,
            _ => StopAction::Drain,
        }
    }

    /// Whether the readiness probe answers 200.
    pub fn is_ready(&self) -> bool {
        self.state.load(Ordering::Acquire) == SERVING
    }
}

/// The liveness probe, `/health`: the process answers HTTP.
pub async fn live() -> &'static str {
    "OK"
}

/// The readiness probe, `/health/ready`: 200 while serving, 503 once a stop
/// signal has arrived.
pub async fn ready(State(state): State<AppState>) -> (StatusCode, &'static str) {
    if state.readiness.is_ready() {
        (StatusCode::OK, "OK")
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "draining")
    }
}

/// Which signal asked the process to stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopSignal {
    /// SIGTERM, what the orchestrator sends before it replaces the pod.
    Terminate,
    /// Ctrl-C in a terminal: a developer, not a rollout.
    Interrupt,
}

/// Install the handlers for SIGTERM and SIGINT, and return a future that
/// resolves on the first of the two.
///
/// Call it at the start of `main`, and pass the future to [`watch_stop`].
/// Without a handler the editor ignores SIGTERM altogether: in the container
/// it runs as PID 1, and the kernel drops a default-handled signal sent to
/// PID 1, so the pod sits out the whole grace period before it is killed.
///
/// # Errors
///
/// When a handler cannot be installed.
pub fn stop_signal() -> std::io::Result<impl Future<Output = StopSignal>> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = signal(SignalKind::terminate())?;
    let mut interrupt = signal(SignalKind::interrupt())?;
    Ok(async move {
        tokio::select! {
            _ = terminate.recv() => StopSignal::Terminate,
            _ = interrupt.recv() => StopSignal::Interrupt,
        }
    })
}

/// Wait for `signal` in a task of its own, so it is acted on while `main` is
/// still starting up, and return the future that hands it to [`drain_on`].
///
/// Before [`Readiness::start_serving`] the task exits the process with status
/// 0: no request can be in flight, and a startup that is still loading the
/// corpus can take longer than the grace period. Once serving, it marks the
/// pod not ready and resolves the returned future.
pub fn watch_stop(
    signal: impl Future<Output = StopSignal> + Send + 'static,
    readiness: Readiness,
) -> impl Future<Output = StopSignal> {
    let (stop_tx, stop_rx) = oneshot::channel();
    tokio::spawn(async move {
        let signal = signal.await;
        match readiness.on_stop() {
            StopAction::Exit => {
                tracing::info!(?signal, "stop signal before the server started; exiting");
                std::process::exit(0);
            }
            StopAction::Drain => {
                // The receiver lives as long as the server; once it is gone
                // there is nothing left to tell.
                if stop_tx.send(signal).is_err() {
                    tracing::debug!("stop signal after the server had stopped");
                }
            }
        }
    });
    async move {
        match stop_rx.await {
            Ok(signal) => signal,
            // The task ended without a signal; that cannot happen short of a
            // panic in it, and is no reason to stop serving.
            Err(_) => std::future::pending().await,
        }
    }
}

/// Resolve when the server should stop accepting connections: after `signal`,
/// and for SIGTERM after a further `delay`.
///
/// Ctrl-C skips the delay; nobody routes traffic to a developer's terminal.
pub async fn drain_on(signal: impl Future<Output = StopSignal>, delay: Duration) {
    match signal.await {
        StopSignal::Terminate => {
            tracing::info!(
                delay_secs = delay.as_secs(),
                "SIGTERM received; reporting not ready, then draining connections"
            );
            tokio::time::sleep(delay).await;
        }
        StopSignal::Interrupt => {
            tracing::info!("interrupted; draining connections");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readiness_is_ready_only_while_serving() {
        let readiness = Readiness::default();
        let clone = readiness.clone();
        assert!(!readiness.is_ready(), "not ready before the port opens");

        assert!(readiness.start_serving());
        assert!(clone.is_ready(), "clones share one state");

        assert_eq!(clone.on_stop(), StopAction::Drain);
        assert!(!readiness.is_ready());
    }

    #[test]
    fn a_stop_during_startup_exits_and_keeps_the_server_from_starting() {
        let readiness = Readiness::default();

        assert_eq!(readiness.on_stop(), StopAction::Exit);

        assert!(!readiness.start_serving());
        assert!(!readiness.is_ready());
    }

    #[tokio::test]
    async fn a_stop_while_serving_reports_not_ready_before_it_reaches_the_drain() {
        let readiness = Readiness::default();
        readiness.start_serving();

        let signal = watch_stop(async { StopSignal::Terminate }, readiness.clone()).await;

        assert_eq!(signal, StopSignal::Terminate);
        assert!(!readiness.is_ready());
    }

    #[tokio::test(start_paused = true)]
    async fn sigterm_keeps_serving_for_the_delay() {
        let drain = tokio::spawn(drain_on(async { StopSignal::Terminate }, DRAIN_DELAY));

        tokio::time::sleep(DRAIN_DELAY - Duration::from_millis(1)).await;
        assert!(!drain.is_finished(), "the listener closed before the delay");

        tokio::time::sleep(Duration::from_millis(1)).await;
        drain.await.expect("drain task");
    }

    #[tokio::test(start_paused = true)]
    async fn ctrl_c_stops_without_the_delay() {
        let started = tokio::time::Instant::now();

        drain_on(async { StopSignal::Interrupt }, DRAIN_DELAY).await;

        assert_eq!(started.elapsed(), Duration::ZERO);
    }
}
