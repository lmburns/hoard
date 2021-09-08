//! Plain text (`Plaintext`) and encrypted text (`Sectext`)

use secstr::SecVec;
use zeroize::Zeroize;

/// Wrap plain-text bytes
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Plaintext(SecVec<u8>);

impl Plaintext {
    /// Create a new instance of `Plaintext`
    #[must_use]
    pub fn empty() -> Self {
        vec![].into()
    }

    /// Get unsecure reference to inner data.
    ///
    /// # Warning
    ///
    /// Unsecure because we cannot guarantee that the referenced data isn't
    /// cloned. Use with care!
    ///
    /// The reference itself is safe to use and share. Data may be cloned from
    /// this reference though, when that happens we lose track of it and are
    /// unable to securely handle it in memory. You should clone `Plaintext`
    /// instead.
    #[must_use]
    pub fn unsecure_ref(&self) -> &[u8] {
        self.0.unsecure()
    }

    /// Get the plaintext as UTF8 string.
    ///
    /// # Warning
    ///
    /// Unsecure because we cannot guarantee that the referenced data isn't
    /// cloned. Use with care!
    ///
    /// The reference itself is safe to use and share. Data may be cloned from
    /// this reference though, when that happens we lose track of it and are
    /// unable to securely handle it in memory. You should clone `Plaintext`
    /// instead.
    pub fn unsecure_to_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.unsecure_ref())
    }

    /// Check whether this plaintext is empty.
    ///
    /// - Empty if 0 bytes
    /// - Empty if bytes parsed as UTF-8 has trimmed length of 0 characters
    ///   (ignored on encoding failure)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.unsecure_ref().is_empty()
            || std::str::from_utf8(self.unsecure_ref())
                .map(|s| s.trim().is_empty())
                .unwrap_or(false)
    }
}

impl From<String> for Plaintext {
    fn from(mut other: String) -> Plaintext {
        // Explicit zeroing of unsecure buffer required
        let into = Plaintext(other.as_bytes().into());
        other.zeroize();
        into
    }
}

impl From<Vec<u8>> for Plaintext {
    fn from(mut other: Vec<u8>) -> Plaintext {
        // Explicit zeroing of unsecure buffer required
        let into = Plaintext(other.clone().into());
        other.zeroize();
        into
    }
}

impl From<&str> for Plaintext {
    fn from(s: &str) -> Self {
        Self(s.as_bytes().into())
    }
}

/// Sectext.
///
/// Wraps Sectext bytes. This type is limited on purpose, to prevent
/// accidentally leaking the Sectext. Security properties are enforced by
/// `secstr::SecVec`.
#[derive(Debug)]
pub struct Sectext(SecVec<u8>);

impl Sectext {
    /// New empty Sectext.
    #[must_use]
    pub fn empty() -> Self {
        vec![].into()
    }

    /// Get unsecure reference to inner data.
    ///
    /// # Warning
    ///
    /// Unsecure because we cannot guarantee that the referenced data isn't
    /// cloned. Use with care!
    ///
    /// The reference itself is safe to use and share. Data may be cloned from
    /// this reference though, when that happens we lose track of it and are
    /// unable to securely handle it in memory. You should clone
    /// `Sectext` instead.
    pub(crate) fn unsecure_ref(&self) -> &[u8] {
        self.0.unsecure()
    }
}

impl From<Vec<u8>> for Sectext {
    fn from(mut other: Vec<u8>) -> Sectext {
        // Explicit zeroing of unsecure buffer required
        let into = Sectext(other.clone().into());
        other.zeroize();
        into
    }
}
