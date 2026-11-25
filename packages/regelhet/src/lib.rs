//! A RegelHet: a server that runs one legal process on the cells of
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
//! - [`config`]: the mode (`test` or `stable`), the surface, the manifest,
//!   the port and the login, from the environment;
//! - [`theme`]: the accent colour of the screens, a theme of the NLDD
//!   design system;
//! - [`surface`]: which side of the process an instance serves, citizens or
//!   the back office, and what differs between them;
//! - [`access`]: the roles, and who a request comes from;
//! - [`login`]: the login through Keycloak or, in test mode, the test
//!   login, and the sessions;
//! - [`fsc4b`]: the citizen's routes for an app, after FSC voor Burgers,
//!   when configured;
//! - [`link`]: the exchange of records with the server of another cell of
//!   the case, when that cell runs on a server of its own;
//! - [`clock`]: the clock trait, with a clock the API can set and the real
//!   date in the Netherlands;
//! - [`store`]: the chronicle store trait, with the store in memory and the
//!   one in Postgres;
//! - [`dossier`]: which records are about one person;
//! - [`external`]: what the admin panel shows of an external party;
//! - [`timeline`]: what happened about one application, and what comes next;
//! - [`organisatie`]: the front of an organisation before several
//!   processes (`REGELHET_ROL=organisatie`), and the statements it signs
//!   for them;
//! - [`case`]: the one case, from the manifest;
//! - [`check`]: whether a case starts, for `regelrecht-regelhet check`;
//! - [`service`]: the steps, against a store and a clock;
//! - [`api`]: the HTTP API over the service;
//! - [`app`]: what the binary builds from a [`config::Config`].

pub mod access;
pub mod api;
pub mod app;
pub mod case;
pub mod check;
pub mod clock;
pub mod config;
pub mod dossier;
pub mod external;
pub mod fsc4b;
pub mod identity;
pub mod link;
pub mod login;
pub mod organisatie;
pub mod service;
pub mod store;
pub mod surface;
pub mod table;
pub mod theme;
pub mod timeline;
pub mod visibility;
pub mod vlam;
