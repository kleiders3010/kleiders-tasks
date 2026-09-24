//! kt-engine: the generic local-first sync engine behind Kleiders Tasks (and future apps).
//!
//! Spec: `docs/DESIGN.md` (v2). This crate has no task-specific code (principle 7).

pub mod crypto;
pub mod envelope;
pub mod epochs;
pub mod ids;
pub mod identity;
pub mod invite;
pub mod limits;
pub mod replica;
pub mod sandbox;
pub mod storage;
pub mod wire;

pub use ids::{DeviceId, DocId, EpochId, GroupId};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("crypto: {0}")]
    Crypto(&'static str),
    #[error("encoding: {0}")]
    Encoding(String),
    #[error("invalid: {0}")]
    Invalid(String),
    #[error("rejected: {0}")]
    Rejected(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("loro: {0}")]
    Loro(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
