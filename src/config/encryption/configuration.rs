//! Encryption section from [`Config`]. It is the `[...config.encryption]`
//! section in the configuration file

#![allow(unused)]
use super::Result;
use crate::config::{
    builder::{
        hoard::{AsymmetricEncryption, Encryption, SymmetricEncryption},
        GlobalConfig,
    },
    directories::PROJECT_DIRS,
};
use gpgme::Protocol;
use std::path::PathBuf;

/// Gpg configuration
#[derive(Debug, Clone)]
pub struct GpgConfig {
    /// User's home directory
    pub home_dir:    PathBuf,
    /// Output directory
    pub output_dir:  PathBuf,
    /// Symmetric encryption
    pub symmetric:   Option<SymmetricEncryption>,
    // /// Asymmetric encryption
    // pub asymmetric:  Option<AsymmetricEncryption>,
    /// ASCII armored or not
    pub armor:       bool,
    /// Default key
    pub default_key: Option<String>,
}

impl GpgConfig {
    /// Create a new instance of [`GpgConfig`]
    pub fn new(opts: &Encryption, global: &GlobalConfig) -> Result<Self> {
        println!("OPTS:: {:?}", opts);
        let init = gpgme::init();
        init.set_engine_home_dir(
            Protocol::OpenPgp,
            PROJECT_DIRS.home_dir().to_path_buf().display().to_string(),
        )?;

        // asymmetric:  if let Encryption::Asymmetric(asym) = opts {
        //     Some(asym.clone())
        // } else {
        //     None
        // },

        Ok(Self {
            home_dir:    PROJECT_DIRS.home_dir().to_path_buf(),
            output_dir:  PROJECT_DIRS.config_dir().join("gpg"),
            symmetric:   if let Encryption::Symmetric(sym) = opts {
                Some(sym.clone())
            } else {
                None
            },
            armor:       if let Encryption::Asymmetric(asym) = opts {
                asym.armor.unwrap_or(false)
            } else {
                false
            },
            default_key: if let Encryption::Asymmetric(asym) = opts {
                Some(asym.public_key.clone())
            } else {
                global.public_key.clone()
            },
        })
    }
}
