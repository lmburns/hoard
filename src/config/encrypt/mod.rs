//! Encryption of [`Hoards`] module
pub mod engine;
pub mod fortress;
pub mod protocol;
pub mod recipients;
pub mod selection;
pub mod types;
pub mod utils;

pub use recipients::Recipients;
use std::{collections::HashMap, fmt, fs, path::Path};
pub use types::{Plaintext, Sectext};

use anyhow::Result;
use thiserror::Error;

/// Errors used within the encryption module file
#[derive(Debug, Error)]
pub enum Error {
    /// Anyhow context
    #[error("failed to obtain GPGME cryptography context")]
    Context(#[source] anyhow::Error),
    /// Unsupported protocol
    #[error("failed to built context, protocol not supportd: {:?}", _0)]
    Unsupported(Engine),
    /// Error when writing to file
    #[error("failed to write to file")]
    WriteFile(#[source] std::io::Error),
    /// Error when reading file
    #[error("failed to read from file")]
    ReadFile(#[source] std::io::Error),
    /// Unknown fingerprint
    #[error("fingerprint does not match public key in keychain")]
    UnknownFingerprint,
}

/// Prelude for common encryption traits
pub mod prelude {
    pub use super::{fortress::FortressRecipients, ImplContext};
}

/// Protocol of encryption (possible implementations of `age` and others)
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Engine {
    /// `GPG` implementation
    Gpg,
}

impl Engine {
    /// Display protocol as human readable
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Gpg => "GPG",
        }
    }
}

/// Encryption configuration
#[derive(Debug, Clone)]
pub struct Config {
    /// Engine/protocol that is used
    pub engine:  Engine,
    /// `TTY` usage option (i.e., `loopback` mode)
    pub gpg_tty: bool,
    /// `--armor` usage option
    pub armor:   bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            engine:  Engine::Gpg,
            gpg_tty: false,
            armor:   true,
        }
    }
}

impl Config {
    /// Create a new instance of `Config` from specified `Engine`
    #[must_use]
    pub fn from(engine: Engine) -> Self {
        Self {
            engine,
            ..Self::default()
        }
    }
}

/// Representation of `Engine` key
#[derive(Clone, PartialEq, Debug)]
#[non_exhaustive]
pub enum Key {
    /// `gpg` key
    Gpg(protocol::gpg::Key),
}

impl Key {
    /// Display user of `Key`
    #[must_use]
    pub fn user_id(&self) -> String {
        match self {
            Self::Gpg(key) => key.display_user(),
        }
    }

    /// Return protocol/engine of `Key`
    #[must_use]
    pub fn protocol(&self) -> Engine {
        match self {
            Self::Gpg(_) => Engine::Gpg,
        }
    }

    /// Return short fingerprint of `Key`
    #[must_use]
    pub fn fingerprint(&self, short: bool) -> String {
        match self {
            Self::Gpg(key) => key.fingerprint(short),
        }
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "[{}] {} - {}",
            self.protocol().name(),
            self.fingerprint(true),
            self.user_id()
        )
    }
}

/// Return context to run encryption commands
/// Possibility of implementing an engine that will use the `gpg` binary
/// directly, or a different protocol entirely (e.g., `age`)
pub fn context(config: &Config) -> Result<Context, Error> {
    match config.engine {
        Engine::Gpg => {
            return Ok(Context::from(Box::new(
                engine::gpgme::context::context(config).map_err(Error::Context)?,
            )));
        },
    }

    #[allow(unreachable_code)]
    Err(Error::Unsupported(config.engine))
}

/// Generic context wrapper for all backend engines
#[allow(missing_debug_implementations)]
pub struct Context {
    /// Inner context
    context: Box<dyn ImplContext>,
}

impl Context {
    /// Convert an `ImplContext` to `Context` (a context wrapper)
    #[must_use]
    pub fn from(context: Box<dyn ImplContext>) -> Self {
        Self { context }
    }
}

impl ImplContext for Context {
    /// Encrypt `Plaintext` with recipients
    fn encrypt(&mut self, recipients: &Recipients, plaintext: Plaintext) -> Result<Sectext> {
        self.context.encrypt(recipients, plaintext)
    }

    /// Decrypt `Sectext`
    fn decrypt(&mut self, sectext: Sectext) -> Result<Plaintext> {
        self.context.decrypt(sectext)
    }

    /// Check if text can be decrypted
    fn can_decrypt(&mut self, sectext: Sectext) -> Result<bool> {
        self.context.can_decrypt(sectext)
    }

    /// Return a vector of public keys
    fn keys_public(&mut self) -> Result<Vec<Key>> {
        self.context.keys_public()
    }

    /// Return a vector of private keys
    fn keys_private(&mut self) -> Result<Vec<Key>> {
        self.context.keys_private()
    }

    /// Import a key to keychain
    fn import_key(&mut self, key: &[u8]) -> Result<()> {
        self.context.import_key(key)
    }

    /// Export a key from keychain
    fn export_key(&mut self, key: Key) -> Result<Vec<u8>> {
        self.context.export_key(key)
    }

    /// Check if context supports protocol
    fn supports_engine(&self, engine: Engine) -> bool {
        self.context.supports_engine(engine)
    }
}

/// Defines a generic encryption context
pub trait ImplContext {
    /// Encrypt `Plaintext` for recipients
    fn encrypt(&mut self, recipients: &Recipients, plaintext: Plaintext) -> Result<Sectext>;

    /// Encrypt `Plaintext` and write it to the file
    fn encrypt_file(
        &mut self,
        recipients: &Recipients,
        plaintext: Plaintext,
        path: &Path,
    ) -> Result<()> {
        fs::write(path, self.encrypt(recipients, plaintext)?.unsecure_ref())
            .map_err(|err| Error::WriteFile(err).into())
    }

    /// Decrypt `Sectext`
    fn decrypt(&mut self, sectext: Sectext) -> Result<Plaintext>;

    /// Decrypt encrypted text from file
    fn decrypt_file(&mut self, path: &Path) -> Result<Plaintext> {
        self.decrypt(fs::read(path).map_err(Error::ReadFile)?.into())
    }

    /// Check whether encrypted text (`Sectext`) can be decrypted
    fn can_decrypt(&mut self, sectext: Sectext) -> Result<bool>;

    /// Check whether we can decrypt Sectext from file.
    fn can_decrypt_file(&mut self, path: &Path) -> Result<bool> {
        self.can_decrypt(fs::read(path).map_err(Error::ReadFile)?.into())
    }

    /// Obtain all public keys from keychain
    fn keys_public(&mut self) -> Result<Vec<Key>>;

    /// Obtain all private keys from keychain
    fn keys_private(&mut self) -> Result<Vec<Key>>;

    /// Obtain a public key from keychain for fingerprint
    fn get_public_key(&mut self, fingerprint: &str) -> Result<Key> {
        self.keys_public()?
            .into_iter()
            .find(|key| utils::fingerprints_equal(key.fingerprint(false), fingerprint))
            .ok_or_else(|| Error::UnknownFingerprint.into())
    }

    /// Find public keys from keychain for fingerprints
    ///
    /// Skips fingerprints if no key is found for it
    fn find_public_keys(&mut self, fingerprints: &[&str]) -> Result<Vec<Key>> {
        let keys = self.keys_public()?;
        Ok(fingerprints
            .iter()
            .filter_map(|fingerprint| {
                keys.iter()
                    .find(|key| utils::fingerprints_equal(key.fingerprint(false), fingerprint))
                    .cloned()
            })
            .collect())
    }

    /// Import the given key from bytes into keychain.
    fn import_key(&mut self, key: &[u8]) -> Result<()>;

    /// Import the given key from a file into keychain.
    fn import_key_file(&mut self, path: &Path) -> Result<()> {
        self.import_key(&fs::read(path).map_err(Error::ReadFile)?)
    }

    /// Export the given key from the keychain as bytes.
    fn export_key(&mut self, key: Key) -> Result<Vec<u8>>;

    /// Export the given key from the keychain to a file.
    fn export_key_file(&mut self, key: Key, path: &Path) -> Result<()> {
        fs::write(path, self.export_key(key)?).map_err(|err| Error::WriteFile(err).into())
    }

    /// Check whether this context supports the given protocol.
    fn supports_engine(&self, engine: Engine) -> bool;
}

/// A pool of engine/protocol contexts.
///
/// Makes using multiple contexts easy, by caching contexts by protocol type and
/// initializing them on demand.
#[allow(missing_debug_implementations)]
pub struct ContextPool {
    /// All loaded contexts.
    contexts: HashMap<Engine, Context>,
}

impl ContextPool {
    /// Create new empty pool
    #[must_use]
    pub fn empty() -> Self {
        Self {
            contexts: HashMap::new(),
        }
    }

    /// Get mutable context for given engine.
    ///
    /// This will initialize the context if no context is loaded for the given
    /// engine yet. This may error..
    pub fn get_mut<'a>(&'a mut self, config: &'a Config) -> Result<&'a mut Context> {
        Ok(self
            .contexts
            .entry(config.engine)
            .or_insert(context(config)?))
    }
}
