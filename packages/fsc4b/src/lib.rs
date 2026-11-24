//! The side of FSC voor Burgers that a server and an app share, after the
//! werkversie of the standard at <https://docs.fsc4b.nl/specs/protocol/>:
//!
//! - [`jose`]: the JOSE formats of §3, narrowed to what the protocol uses;
//! - [`wire`]: the fixed names and paths both sides must spell alike;
//! - [`client`] (feature `client`): what an app does, step by step, over
//!   HTTP.
//!
//! The server in `packages/regelhet` checks what this crate produces; the
//! app in `packages/regelhet-app` and the end-to-end tests of the server
//! run on the same client, so there is one implementation of each side.

#[cfg(feature = "client")]
pub mod client;
pub mod jose;
pub mod wire;
