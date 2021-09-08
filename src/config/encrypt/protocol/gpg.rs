//! `gpg` protocol

use crate::config::encrypt;

/// Representation of a `gpg` key
#[derive(Debug, Clone)]
pub struct Key {
    /// `gpg` fingerprint
    pub fingerprint: String,
    /// User ID's as strings
    pub user_ids:    Vec<String>,
}

impl Key {
    /// Transform generic key into `Key` with `gpgme` aspects
    #[must_use]
    pub fn into_key(self) -> encrypt::Key {
        encrypt::Key::Gpg(self)
    }

    /// Display user id
    #[must_use]
    pub fn display_user(&self) -> String {
        self.user_ids.join("; ")
    }

    /// Display short or long fingerprint
    #[must_use]
    pub fn fingerprint(&self, short: bool) -> String {
        {
            if short {
                &self.fingerprint[self.fingerprint.len() - 16..]
            } else {
                &self.fingerprint
            }
        }
        .trim()
        .to_uppercase()
    }
}

impl PartialEq for Key {
    fn eq(&self, key: &Self) -> bool {
        self.fingerprint.trim().to_uppercase() == key.fingerprint.trim().to_uppercase()
    }
}
