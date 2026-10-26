//! The PoC portal: one hostname, several proof-of-concepts, each behind its own
//! password.
//!
//! `poc.regelrecht.rijks.app` is a single ZAD component. It cannot be several,
//! because the deployment publishes on `component.subdomain` and every
//! component therefore gets its own hostname — so the routing between the PoCs
//! happens here, inside one container, rather than at the ingress.
//!
//! Two kinds of PoC live behind it. A `statisch` one is built into this image
//! and served from disk. A `proxy` one runs as its own (unpublished) component
//! and is forwarded to in-cluster, the way `editor-api` already forwards to
//! `pipelineapi`. A static PoC may also load a source while it runs, a
//! corpus the portal fetches from a branch behind the same password
//! ([`bron`]).

pub mod app;
pub mod bron;
pub mod config;
pub mod gate;
pub mod pagina;
pub mod proxy;
pub mod registry;

/// The register, baked into the binary. It describes the PoCs this image was
/// built with, so reading it from disk at runtime would only allow the two to
/// disagree.
pub const REGISTRY_YAML: &str = include_str!("../../../pocs/registry.yaml");
