//! [`GpgConfig`] generation and `gpg` operations

pub mod configuration;

use thiserror::Error;

/// Result alias for [`GpgConfig`] module
pub type Result<T> = std::result::Result<T, Error>;

/// Errors found throughout the [`GpgConfig`] module
#[derive(Debug, Error)]
pub enum Error {
    /// Error with gpgme
    #[error("gpgme error: {0}")]
    Gpgme(#[from] gpgme::Error),
}
