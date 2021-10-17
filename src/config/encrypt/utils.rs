//! Utility functions used in the `encrypt` module

use anyhow::Result;
use std::{
    env, io,
    path::{Path, PathBuf},
};
use thiserror::Error;

use crate::config::builder::hoard::{Config as HoardConfig, Encryption};

use super::{
    fortress::{FORTRESS_GPG_ID, FORTRESS_PUB_KEY},
    Config, Context, Engine, ImplContext, Key,
};

/// Errors used throughout the `util` part of the `encrypt` module
#[derive(Debug, Error)]
pub enum Error {
    /// Path contains '.gpg-id' or '.public-keys'
    #[error("fortress already present: {0}")]
    ExistingFortress(PathBuf),
    /// IO Error
    #[error("there was an error reading directory")]
    ReadDir(#[from] io::Error),
    /// Directory has no parent directory
    #[error("failed to append suffix to file path, unknown parent")]
    NoParent,
    /// Unkown file name
    #[error("failed to append suffix to file path, unknown name")]
    UnknownName,
}

/// Default cryptography protocol.
const ENGINE: Engine = Engine::Gpg;

/// Max depth a symlink with traverse
const SYMLINK_MAX_DEPTH: u8 = 31_u8;

/// Check whether `GPG_TTY` is set
#[must_use]
pub fn has_gpg_tty() -> bool {
    env::var_os("GPG_TTY").map_or(false, |v| !v.is_empty())
}

/// Get TTY path for this process.
///
/// Returns `None` if not in a TTY.
/// Always returns `None` if OS is not Linux, FreeBSD, or `OpenBSD`
#[must_use]
pub fn get_tty() -> Option<PathBuf> {
    // None on unsupported platforms
    if cfg!(not(any(
        target_os = "linux",
        target_os = "freebsd",
        target_os = "openbsd",
    ))) {
        return None;
    }

    let path = PathBuf::from("/dev/stdin");
    resolve_symlink(&path, 0)
}

/// Resolve symlink to the final accessible path.
///
/// Returns `None` if the given link could not be read (and `depth` is 0).
///
/// # Panics
///
/// Panics if a depth of `SYMLINK_DEPTH_MAX` is reached to prevent infinite
/// recursion.
fn resolve_symlink(path: &Path, depth: u8) -> Option<PathBuf> {
    #[allow(clippy::if_then_panic, clippy::panic)]
    if depth >= SYMLINK_MAX_DEPTH {
        panic!("failed to resolve symlink because it is too deep, possible loop?");
    }

    // TODO:
    // Could add option for two path's as input, both as options. Run the recursive
    // function using only the second path and then if an error arises, return the
    // first path that was passed to the original calling function

    // Read symlink path, recursively find target
    match path.read_link() {
        Ok(path) => resolve_symlink(&path, depth + 1),
        Err(_) if depth == 0 => None,
        Err(_) => Some(path.into()),
    }
}

/// Append a suffix to the filename of a path.
///
/// Errors if the path parent or file name could not be determined.
pub fn append_file_name(path: &Path, suffix: &str) -> Result<PathBuf> {
    Ok(path.parent().ok_or(Error::NoParent)?.join(format!(
        "{}{}",
        path.file_name()
            .ok_or(Error::UnknownName)?
            .to_string_lossy(),
        suffix,
    )))
}

/// Consistent formatting of all fingerprints by converting them to uppercase
pub fn format_fingerprint<S: AsRef<str>>(fingerprint: S) -> String {
    fingerprint.as_ref().trim().to_uppercase()
}

/// Check whether two fingerprints match
pub fn fingerprints_equal<S: AsRef<str>, T: AsRef<str>>(a: S, b: T) -> bool {
    !a.as_ref().trim().is_empty()
        && a.as_ref().trim().to_uppercase() == b.as_ref().trim().to_uppercase()
}

/// Check if list of keys contains given fingerprint
pub fn keys_contain_fingerprint<S: AsRef<str>>(keys: &[Key], fingerprint: S) -> bool {
    keys.iter()
        .any(|key| fingerprints_equal(key.fingerprint(false), fingerprint.as_ref()))
}

/// Check whether the user has any private/secret key in their keychain
pub fn has_private_key(config: &Config) -> Result<bool> {
    Ok(!super::context(config)?.keys_private()?.is_empty())
}

/// Construct crypto config, respect fields found in the configuration file.
/// Converts [`hoard::config::builder::hoard::Config`] to
/// [`hoard::config::encrypt::Config`]
#[must_use]
pub fn config(hoardconf: &HoardConfig) -> Config {
    let mut config = Config::from(ENGINE);
    hoardconf.clone().encryption.map_or(config.clone(), |enc| {
        tracing::trace!("Constructing configuration from: {} encryption", enc.name());
        match enc {
            Encryption::Symmetric(ref _e) => config,
            Encryption::Asymmetric(ref e) => {
                config.armor = e.armor;
                config
            },
        }
    })
}

/// Construct crypto context
pub fn context(config: &HoardConfig) -> Result<Context, super::Error> {
    super::context(&self::config(config))
}

/// Check if the directory is a fortress already
pub fn check_existing_fortress(path: &Path) -> Result<(), Error> {
    if !path.is_dir()
        || path
            .read_dir()?
            .filter_map(Result::ok)
            .any(|e| e.file_name() == FORTRESS_PUB_KEY || e.file_name() == FORTRESS_GPG_ID)
    {
        return Err(Error::ExistingFortress(path.to_path_buf()));
    }
    Ok(())
}
