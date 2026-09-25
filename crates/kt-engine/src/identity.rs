//! Device identity (D1).
//!
//! The device's secret keys live in a directory that is **excluded from backups**
//! (Android `noBackupFilesDir`; on desktop a separate `identity/` directory). A restored
//! or copied install therefore has no identity and creates a fresh one, which prevents
//! two live devices from sharing keys or a Loro PeerID.
//!
//! The desktop build must also hold an exclusive lock on the identity directory so two
//! instances can't run at once (see [`IdentityLock`]).

use std::fs::{self, File, OpenOptions};
use std::path::Path;

use crate::crypto::{DevicePublic, DeviceSecret};
use crate::ids::{peer_id_for, DeviceId};
use crate::{Error, Result};

const IDENTITY_FILE: &str = "device.key";
const LOCK_FILE: &str = "instance.lock";

pub struct DeviceIdentity {
    secret: DeviceSecret,
    public: DevicePublic,
}

impl DeviceIdentity {
    pub fn generate() -> Self {
        let secret = DeviceSecret::generate();
        let public = secret.public();
        DeviceIdentity { secret, public }
    }

    pub fn id(&self) -> DeviceId {
        DeviceId(self.public.sign)
    }

    pub fn public(&self) -> DevicePublic {
        self.public
    }

    pub fn peer_id(&self) -> u64 {
        peer_id_for(&self.id())
    }

    pub fn secret(&self) -> &DeviceSecret {
        &self.secret
    }

    pub fn sign(&self, msg: &[u8]) -> [u8; 64] {
        self.secret.sign(msg)
    }

    /// Load the identity from `dir`, or create one if none exists.
    pub fn load_or_create(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let path = dir.join(IDENTITY_FILE);
        if path.exists() {
            let bytes = fs::read(&path)?;
            let arr: [u8; 64] = bytes
                .as_slice()
                .try_into()
                .map_err(|_| Error::Invalid("identity file has wrong length".into()))?;
            let secret = DeviceSecret::from_bytes(&arr);
            let public = secret.public();
            Ok(DeviceIdentity { secret, public })
        } else {
            let id = DeviceIdentity::generate();
            crate::storage::write_atomic(&path, &id.secret.to_bytes())?;
            Ok(id)
        }
    }
}

/// Exclusive single-instance lock (D1). Held for the lifetime of the engine.
///
/// An OS file lock, not a marker file: the OS releases it when the process dies, so a
/// killed app (Android kills processes all the time) never locks itself out.
pub struct IdentityLock {
    _file: File,
}

impl IdentityLock {
    /// Acquire the lock. Fails if another running instance holds it.
    pub fn acquire(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let f = OpenOptions::new().read(true).write(true).create(true).truncate(false).open(dir.join(LOCK_FILE))?;
        match f.try_lock() {
            Ok(()) => Ok(IdentityLock { _file: f }),
            Err(std::fs::TryLockError::WouldBlock) => Err(Error::Rejected("another instance is already running".into())),
            Err(std::fs::TryLockError::Error(e)) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("kt-identity-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn load_or_create_is_stable() {
        let d = tmp("stable");
        let a = DeviceIdentity::load_or_create(&d).unwrap();
        let b = DeviceIdentity::load_or_create(&d).unwrap();
        assert_eq!(a.id(), b.id());
        assert_eq!(a.peer_id(), b.peer_id());
        let _ = fs::remove_dir_all(&d);
    }

    #[test]
    fn missing_identity_means_new_device() {
        // Simulates a restore where the identity dir was excluded from backup.
        let d1 = tmp("orig");
        let d2 = tmp("restored");
        let a = DeviceIdentity::load_or_create(&d1).unwrap();
        let b = DeviceIdentity::load_or_create(&d2).unwrap();
        assert_ne!(a.id(), b.id());
        assert_ne!(a.peer_id(), b.peer_id());
        let _ = fs::remove_dir_all(&d1);
        let _ = fs::remove_dir_all(&d2);
    }

    #[test]
    fn single_instance_lock() {
        let d = tmp("lock");
        let l1 = IdentityLock::acquire(&d).unwrap();
        assert!(IdentityLock::acquire(&d).is_err());
        drop(l1);
        assert!(IdentityLock::acquire(&d).is_ok());
        // A lock file left behind by a killed process doesn't lock the app out.
        fs::write(d.join(LOCK_FILE), b"12345
").unwrap();
        assert!(IdentityLock::acquire(&d).is_ok(), "stale lock file ignored");
        let _ = fs::remove_dir_all(&d);
    }
}
