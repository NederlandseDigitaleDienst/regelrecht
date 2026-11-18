//! A wetsprocessysteem: a server that runs one legal process on the cells of
//! the regelrecht method, for every user at once.
//!
//! What it runs comes from one source manifest that ships with it
//! ([`case`]): one case, its cells and the regelwerken they run. The process
//! steps are those of the chronolexografie poc (`regelrecht_poc_chronolexografie::kern`),
//! used as a library; this crate adds no step of its own. Where the poc
//! keeps one visitor's game in the browser, this server loads the cells from
//! a [`store::ChronicleStore`] for every request, runs the step and writes
//! the new records back ([`service`]). What day it is comes from a
//! [`clock::Clock`].
//!
//! The parts:
//!
//! - [`config`]: the mode (`test` or `stable`), the manifest and the port,
//!   from the environment;
//! - [`clock`]: the clock trait, with a clock the API can set and the real
//!   date in the Netherlands;
//! - [`store`]: the chronicle store trait, with the store in memory;
//! - [`case`]: the one case, from the manifest;
//! - [`service`]: the steps, against a store and a clock;
//! - [`api`]: the HTTP API over the service;
//! - [`app`]: what the binary builds from a [`config::Config`].

pub mod api;
pub mod app;
pub mod case;
pub mod clock;
pub mod config;
pub mod service;
pub mod store;
