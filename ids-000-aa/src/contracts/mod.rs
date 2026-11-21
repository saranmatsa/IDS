//! # ids-000-aa Sealed Trait Contracts
//!
//! Every sealed trait contract lives here. Each contract is sealed via
//! `pub trait X: private::Sealed {}` — downstream crates can call methods and
//! use as trait bounds, but cannot implement. Only ids-000-aa and its
//! designated implementation crates can `impl`.
//!
//! Adding one method to any contract here = edit exactly one file, then
//! implement in one dependent crate at a time. Zero downstream edits.

pub mod carrier;
pub mod transport;
pub mod store;
pub mod session;
pub mod routing;
pub mod discovery;
pub mod membership;
pub mod placement;
pub mod replication;
pub mod recovery;
pub mod consistency;
pub mod ordering;
pub mod security;
pub mod client;
pub mod fault;
pub mod broadcast;
pub mod quorum;
pub mod snapshot;
pub mod byzantine;
pub mod lease;
pub mod crdt;
pub mod erasure;
pub mod saga;
pub mod coordinator;
pub mod durability;
pub mod topology;
pub mod integration;
pub mod protocol;
pub mod wire;
pub mod framing;
pub mod sequencing;
pub mod acks;