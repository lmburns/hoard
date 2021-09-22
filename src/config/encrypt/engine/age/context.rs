//! Provides `age` binary context adapter.

use super::Config;
use anyhow::Result;
use thiserror::Error;

use super::raw;
use crate::{
    crypto::{proto, IsContext, Key, Proto},
    Ciphertext, Plaintext,
};

/// GPGME crypto context.
pub struct Context {
    /// `age` crytpo context.
    config: Config,
}

impl Context {
    /// Construct context from `age` `Config`.
    fn from(config: Config) -> Self {
        Self { config }
    }
}

impl IsContext for Context {
    fn encrypt(&mut self, plaintext: Plaintext) -> Result<Ciphertext> {
        raw::encrypt(config.recipient, config.recipients_file, plaintext)
    }

    fn decrypt(&mut self, ciphertext: Ciphertext) -> Result<Plaintext> {
        raw::decrypt(&mut self.context, ciphertext)
    }

    fn can_decrypt(&mut self, ciphertext: Ciphertext) -> Result<bool> {
        raw::can_decrypt(&mut self.context, ciphertext)
    }

    fn keys_public(&mut self) -> Result<Vec<Key>> {
        Ok(raw::public_keys(&mut self.context)?
            .into_iter()
            .map(|key| {
                Key::Gpg(proto::gpg::Key {
                    fingerprint: key.0,
                    user_ids:    key.1,
                })
            })
            .collect())
    }

    fn keys_private(&mut self) -> Result<Vec<Key>> {
        Ok(raw::private_keys(&mut self.context)?
            .into_iter()
            .map(|key| {
                Key::Gpg(proto::gpg::Key {
                    fingerprint: key.0,
                    user_ids:    key.1,
                })
            })
            .collect())
    }

    fn import_key(&mut self, key: &[u8]) -> Result<()> {
        raw::import_key(&mut self.context, key)
    }

    fn export_key(&mut self, key: Key) -> Result<Vec<u8>> {
        raw::export_key(&mut self.context, &key.fingerprint(false))
    }

    fn supports_proto(&self, proto: Proto) -> bool {
        proto == Proto::Gpg
    }
}

/// GPGME context error.
#[derive(Debug, Error)]
pub enum Err {
    #[error("failed to obtain GPGME cryptography context")]
    Context(#[source] gpgme::Error),
}
