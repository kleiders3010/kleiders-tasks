//! kt-net: transports for kt-engine (DESIGN §6–§7b).
//!
//! The engine is transport-agnostic: it produces sealed slots and consumes them. This crate
//! moves them: a capability-based [`mailbox::Mailbox`] interface, the Nostr backend, and the
//! [`sync::Syncer`] that drives a replica against mailboxes.

pub mod dht;
pub mod helper;
pub mod health;
pub mod helper_dir;
#[cfg(feature = "relay-server")]
pub mod helper_relay;
pub mod invite_net;
pub mod lan;
pub mod live;
pub mod mailbox;
pub mod manifest;
pub mod nostr;
pub mod p2p;
#[cfg(feature = "relay-server")]
pub mod relay_runner;
pub mod relay_tls;
pub mod relays;
pub mod settings;
pub mod slotstore;
pub mod sync;
pub mod testrelay;
pub mod update;
