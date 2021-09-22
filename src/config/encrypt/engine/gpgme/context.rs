//! `gpgme` engine for encryption

use std::env;

use anyhow::Result;
use gpgme::{Context as GpgmeContext, PinentryMode, Protocol};
use thiserror::Error;

use crate::config::encrypt::{
    protocol,
    recipients::Recipients,
    types::{Plaintext, Sectext},
    utils, Config, Engine, ImplContext, Key,
};

use super::gpg;

// use std::sync::Once;
use once_cell::sync::OnceCell;
static ONCE: OnceCell<()> = OnceCell::new();

/// Errors used throughout the `context` part of the `gpgme` module
#[derive(Debug, Error)]
pub enum Error {
    /// `gpgme` context error
    #[error("gpgme context error")]
    Context(#[source] gpgme::Error),
}

/// Return `Context` from `gpgme` which this whole module is based off of
pub fn context(config: &Config) -> Result<Context> {
    ONCE.get_or_init(|| {
        let _span = tracing::trace_span!("setting gpgme context");
        if config.gpg_tty && !utils::has_gpg_tty() {
            if let Some(tty) = utils::get_tty() {
                tracing::trace!(?tty, "setting `GPG_TTY`");
                env::set_var("GPG_TTY", tty);
            }
        }
    });

    let mut context = gpgme::Context::from_protocol(Protocol::OpenPgp).map_err(Error::Context)?;

    if config.gpg_tty {
        context
            .set_pinentry_mode(PinentryMode::Loopback)
            .map_err(Error::Context)?;
    }

    tracing::trace!(armor = config.armor, "setting armor");
    context.set_armor(config.armor);

    Ok(Context::from(context))
}

/// `GPGME` context wrapper
#[derive(Debug)]
pub struct Context {
    /// `GPGME` encryption context
    context: GpgmeContext,
}

impl Context {
    /// Convert a `GpgmeContext` to `Context`
    #[must_use]
    pub fn from(context: GpgmeContext) -> Self {
        Self { context }
    }
}

impl ImplContext for Context {
    fn encrypt(&mut self, recipients: &Recipients, plaintext: Plaintext) -> Result<Sectext> {
        let fingerprints: Vec<String> = recipients
            .keys()
            .iter()
            .map(|key| key.fingerprint(false))
            .collect();
        let fingerprints: Vec<&str> = fingerprints.iter().map(String::as_str).collect();
        gpg::encrypt(&mut self.context, &fingerprints, &plaintext)
    }

    fn encrypt_symmetric(&mut self, plaintext: Plaintext) -> Result<Sectext> {
        gpg::encrypt_symmetric(&mut self.context, &plaintext)
    }

    fn decrypt(&mut self, sectext: Sectext) -> Result<Plaintext> {
        gpg::decrypt(&mut self.context, &sectext)
    }

    fn can_decrypt(&mut self, sectext: Sectext) -> Result<bool> {
        Ok(gpg::can_decrypt(&mut self.context, &sectext))
    }

    fn user_emails(&mut self) -> Result<Vec<String>> {
        gpg::user_emails(&mut self.context)
    }

    fn keys_public(&mut self) -> Result<Vec<Key>> {
        Ok(gpg::public_keys(&mut self.context)?
            .into_iter()
            .map(|key| {
                Key::Gpg(protocol::gpg::Key {
                    fingerprint: key.0,
                    user_ids:    key.1,
                })
            })
            .collect())
    }

    fn keys_private(&mut self) -> Result<Vec<Key>> {
        Ok(gpg::private_keys(&mut self.context)?
            .into_iter()
            .map(|key| {
                Key::Gpg(protocol::gpg::Key {
                    fingerprint: key.0,
                    user_ids:    key.1,
                })
            })
            .collect())
    }

    fn import_key(&mut self, key: &[u8]) -> Result<()> {
        gpg::import_key(&mut self.context, key)
    }

    fn export_key(&mut self, key: Key) -> Result<Vec<u8>> {
        gpg::export_key(&mut self.context, &key.fingerprint(false))
    }

    fn supports_engine(&self, engine: Engine) -> bool {
        engine == Engine::Gpg
    }
}
