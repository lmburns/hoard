//! Direct interface to `GPGME`.
//!
//! This provides the most basic and bare functions to interface with the
//! `GPGME` engine.

use anyhow::Result;
use gpgme::{Context, EncryptFlags, Key};
use thiserror::Error;
use zeroize::Zeroize;

use crate::config::encrypt::types::{Plaintext, Sectext};

/// `GPGME` errors
#[derive(Debug, Error)]
pub(crate) enum Error {
    /// `GPGME` encryption error
    #[error("failed to encrypt plaintext")]
    Encrypt(#[source] gpgme::Error),
    /// `GPGME` decryption error
    #[error("failed to decrypt sectext")]
    Decrypt(#[source] gpgme::Error),
    /// `GPGME` import error
    #[error("failed to import key")]
    Import(#[source] anyhow::Error),
    /// `GPGME` export error
    #[error("failed to export key")]
    Export(#[source] anyhow::Error),
    /// `GPGME` unkown fingerprint
    #[error("fingerprint does not match public key in keychain")]
    UnknownFingerprint(#[source] gpgme::Error),
}

/// Encrypt `Plaintext` for the given recipients.
///
/// - `context`: `GPGME` context
/// - `recipients`: list of recipient fingerprints to encrypt for
/// - `plaintext`: `Plaintext` to encrypt
///
/// # Panics
///
/// Panics if list of recipients is empty.
pub(crate) fn encrypt(
    context: &mut Context,
    recipients: &[&str],
    plaintext: &Plaintext,
) -> Result<Sectext> {
    assert!(
        !recipients.is_empty(),
        "attempting to encrypt secret for empty list of recipients"
    );

    let mut sectext = vec![];
    let keys = fingerprints_to_keys(context, recipients)?;
    context
        .encrypt_with_flags(
            keys.iter(),
            plaintext.unsecure_ref(),
            &mut sectext,
            EncryptFlags::ALWAYS_TRUST,
        )
        .map_err(Error::Encrypt)?;
    Ok(Sectext::from(sectext))
}

// TODO: Flags here do not allow for encryption with a password with no
// interaction of the file Only idea I have is to tar the directory and encrypt
// it

/// Encrypt `Plaintext` symmetrically for the given recipients.
///
/// - `context`: `GPGME` context
/// - `recipients`: list of recipient fingerprints to encrypt for
/// - `plaintext`: `Plaintext` to encrypt
pub(crate) fn encrypt_symmetric(context: &mut Context, plaintext: &Plaintext) -> Result<Sectext> {
    let mut sectext = vec![];
    context
        .encrypt_symmetric_with_flags(
            plaintext.unsecure_ref(),
            &mut sectext,
            EncryptFlags::SYMMETRIC,
        )
        .map_err(Error::Encrypt)?;
    Ok(Sectext::from(sectext))
}

/// Decrypt sectext.
///
/// - `context`: GPGME context
/// - `sectext`: encrypted text to decrypt
pub(crate) fn decrypt(context: &mut Context, sectext: &Sectext) -> Result<Plaintext> {
    let mut plaintext = vec![];
    context
        .decrypt(sectext.unsecure_ref(), &mut plaintext)
        .map_err(Error::Decrypt)?;
    Ok(Plaintext::from(plaintext))
}

/// Check whether we can decrypt sectext.
///
/// This checks whether whether we own the secret key to decrypt the given
/// sectext. Assumes `true` if GPGME returns an error different than
/// `NO_SECKEY`.
///
/// - `context`: `GPGME` context
/// - `sectext`: `Sectext` to check
// To check this, actual decryption is attempted, see this if this can be
// improved: https://stackoverflow.com/q/64633736/1000145
pub(crate) fn can_decrypt(context: &mut Context, sectext: &Sectext) -> bool {
    // Try to decrypt, explicit zeroing of unsecure buffer required
    let mut plaintext = vec![];
    let result = context.decrypt(sectext.unsecure_ref(), &mut plaintext);
    plaintext.zeroize();

    match result {
        Err(err) if gpgme::error::Error::NO_SECKEY.code() == err.code() => false,
        Ok(_) | Err(_) => true,
    }
}

/// Get all public keys from keychain.
///
/// - `context`: GPGME context
pub(crate) fn public_keys(context: &mut Context) -> Result<Vec<KeyId>> {
    Ok(context
        .keys()?
        .into_iter()
        .filter_map(Result::ok)
        .filter(Key::can_encrypt)
        .map(Into::into)
        .collect())
}

/// Get all private/secret keys from keychain.
///
/// - `context`: `GPGME` context
pub(crate) fn private_keys(context: &mut Context) -> Result<Vec<KeyId>> {
    Ok(context
        .secret_keys()?
        .into_iter()
        .filter_map(Result::ok)
        .filter(Key::can_encrypt)
        .map(Into::into)
        .collect())
}

/// Access emails within the keychain
pub(crate) fn user_emails(context: &mut Context) -> Result<Vec<String>> {
    let mut emails = vec![];
    context
        .secret_keys()?
        .into_iter()
        .filter_map(Result::ok)
        .filter(Key::can_encrypt)
        .map(|key| {
            key.user_ids()
                .map(|k| {
                    if let Ok(email) = k.email() {
                        if !email.trim().is_empty() {
                            emails.push(email.to_owned());
                        }
                    }
                })
                .collect::<Vec<_>>()
        })
        .for_each(drop);

    Ok(emails)
}

/// Import given key from bytes into keychain.
///
/// - `context`: GPGME context
///
/// # Panics
///
/// Panics if the provides key does not look like a public key.
pub fn import_key(context: &mut Context, key: &[u8]) -> Result<()> {
    // Assert we're importing a public key
    let key_str = std::str::from_utf8(key).expect("exported key is invalid UTF-8");
    assert!(
        !key_str.contains("PRIVATE KEY"),
        "imported key contains PRIVATE KEY, blocked to prevent accidentally leaked secret key"
    );
    assert!(
        key_str.contains("PUBLIC KEY"),
        "imported key must contain PUBLIC KEY, blocked to prevent accidentally leaked secret key"
    );

    // Import the key
    context
        .import(key)
        .map(|_| ())
        .map_err(|err| Error::Import(err.into()).into())
}

/// Export the given key as bytes.
///
/// # Panics
///
/// Panics if the received key does not look like a public key. This should
/// never happen unless the gpg binary backend is broken.
pub fn export_key(context: &mut Context, fingerprint: &str) -> Result<Vec<u8>> {
    // Find the GPGME key to export
    let key = context
        .get_key(fingerprint)
        .map_err(|err| Error::Export(Error::UnknownFingerprint(err).into()))?;

    // Export key to memoy with armor enabled
    let mut data: Vec<u8> = vec![];
    let armor = context.armor();
    context.set_armor(true);
    context.export_keys(&[key], gpgme::ExportMode::empty(), &mut data)?;
    context.set_armor(armor);

    // Assert we're exporting a public key
    let data_str = std::str::from_utf8(&data).expect("exported key is invalid UTF-8");
    assert!(
        !data_str.contains("PRIVATE KEY"),
        "exported key contains PRIVATE KEY, blocked to prevent accidentally leaking secret key"
    );
    assert!(
        data_str.contains("PUBLIC KEY"),
        "exported key must contain PUBLIC KEY, blocked to prevent accidentally leaking secret key"
    );

    Ok(data)
}

/// A key identifier with a fingerprint and user IDs.
#[derive(Clone, Debug)]
pub struct KeyId(pub String, pub Vec<String>);

impl From<Key> for KeyId {
    fn from(key: Key) -> Self {
        Self(
            key.fingerprint()
                .expect("GPGME key does not have fingerprint")
                .to_owned(),
            key.user_ids()
                .map(|user| {
                    let mut parts = vec![];
                    if let Ok(name) = user.name() {
                        if !name.trim().is_empty() {
                            parts.push(name.into());
                        }
                    }
                    if let Ok(comment) = user.comment() {
                        if !comment.trim().is_empty() {
                            parts.push(format!("({})", comment));
                        }
                    }
                    if let Ok(email) = user.email() {
                        if !email.trim().is_empty() {
                            parts.push(format!("<{}>", email));
                        }
                    }
                    parts.join(" ")
                })
                .collect(),
        )
    }
}

/// Transform fingerprints into GPGME keys.
///
/// Errors if a fingerprint does not match a public key.
fn fingerprints_to_keys(context: &mut Context, fingerprints: &[&str]) -> Result<Vec<Key>> {
    let mut keys = vec![];
    for fp in fingerprints {
        keys.push(
            context
                .get_key(fp.to_owned())
                .map_err(Error::UnknownFingerprint)?,
        );
    }
    Ok(keys)
}
