//! Provides an interface for encryption recipients

use anyhow::Result;

use super::Key;
use crate::config::encrypt::{self, prelude::*, utils, Config};

/// A list of recipients.
///
/// This list is used to define identities (as keys) to encrypt secrets for.
/// All keys should always use the same protocol.
///
/// In the future this may support recipients using multiple protocols.
#[derive(Clone, PartialEq, Debug)]
pub struct Recipients {
    /// Recipient keys
    keys: Vec<Key>,
}

impl Recipients {
    /// Construct recipients set from list of keys
    ///
    /// # Panics
    ///
    /// Panics if keys use multiple protocols.
    #[must_use]
    pub fn from(keys: Vec<Key>) -> Self {
        assert!(
            keys_same_protocol(&keys),
            "recipient keys must use same proto"
        );
        Self { keys }
    }

    /// Get recipient keys
    #[must_use]
    pub fn keys(&self) -> &[Key] {
        &self.keys
    }

    /// Add recipient
    ///
    /// # Panics
    ///
    /// Panics if new key uses different protocol
    pub fn add(&mut self, key: Key) {
        self.keys.push(key);
        assert!(
            keys_same_protocol(&self.keys),
            "added recipient key uses different proto"
        );
    }

    /// Remove the given key if existent.
    pub fn remove(&mut self, key: &Key) {
        self.keys.retain(|k| k != key);
    }

    /// Remove the given keys.
    ///
    /// Keys that are not found are ignored.
    pub fn remove_all(&mut self, keys: &[Key]) {
        self.keys.retain(|k| !keys.contains(k));
    }

    /// Check whether this recipient list has the given fingerprint.
    #[must_use]
    pub fn has_fingerprint(&self, fingerprint: &str) -> bool {
        self.keys
            .iter()
            .any(|k| utils::fingerprints_equal(k.fingerprint(false), fingerprint))
    }
}

/// Check whether the given recipients contain any key that we have a secret key
/// in our keychain for.
pub fn contains_own_secret_key(recipients: &Recipients) -> Result<bool> {
    let secrets = Recipients::from(encrypt::context(&Config::default())?.keys_private()?);
    Ok(recipients
        .keys()
        .iter()
        .any(|k| secrets.has_fingerprint(&k.fingerprint(false))))
}

/// Check if given keys all use same proto.
///
/// Succeeds if no key is given.
fn keys_same_protocol(keys: &[Key]) -> bool {
    if keys.len() < 2 {
        true
    } else {
        let protocol = keys[0].protocol();
        keys[1..].iter().all(|k| k.protocol() == protocol)
    }
}
