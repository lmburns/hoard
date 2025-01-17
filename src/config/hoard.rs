//! This module contains processed versions of builder
//! [`Hoard`](crate::config::builder::hoard::Hoard)s. See documentation for
//! builder `Hoard`s for more details.

#![allow(unused)]
use crate::{
    checkers::history::last_paths::HoardPaths,
    config::{
        builder::{
            hoard::{Config, Encryption, SymmetricEncryption},
            GlobalConfig,
        },
        encrypt::{
            fortress::{
                append_sec_suffix, build_fortress, is_secret_file, is_special_file, rm_sec_suffix,
                Fortress, Secret,
            },
            prelude::*,
            types::Plaintext,
            utils::context,
            Context, Recipients,
        },
    },
    hoard_error, hoard_warn,
    utils::{
        contains_upperchar, create_temp_ignore, delete_file, osstr_to_bytes, recursively_set_perms,
        write_temp_ignore,
    },
};

use colored::Colorize;
use crossbeam_channel as channel;
use ignore::{overrides::OverrideBuilder, WalkBuilder, WalkState};
use once_cell::sync::{Lazy, OnceCell};
use rayon::prelude::*;
use regex::bytes::RegexBuilder;
use std::{
    borrow::Cow,
    collections::HashMap,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
};
use thiserror::Error;

static FORTRESS_INITIALIZATION: OnceCell<Result<(Fortress, Recipients), Error>> = OnceCell::new();

/// Errors that can happen while backing up or restoring a hoard.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Error)]
pub enum Error {
    /// Error while copying a file.
    #[error("failed to copy {src} to {dest}: {error}")]
    CopyFile {
        /// The path of the source file.
        src:   PathBuf,
        /// The path of the destination file.
        dest:  PathBuf,
        /// The I/O error that occurred.
        #[source]
        error: io::Error,
    },
    /// Error while creating a directory.
    #[error("failed to create {path}: {error}")]
    CreateDir {
        /// The path of the directory to create.
        path:  PathBuf,
        /// The error that occurred while creating.
        #[source]
        error: io::Error,
    },
    /// Error while reading a directory or an item in a directory.
    #[error("cannot read {path}: {error}")]
    ReadItem {
        /// The path of the file or directory to read.
        path:  PathBuf,
        /// The error that occurred while reading.
        #[source]
        error: io::Error,
    },
    /// Both the source and destination exist but are not both directories or
    /// both files.
    #[error(
        "both source (\"{src}\") and destination (\"{dest}\") exist but are not both files or \
         both directories"
    )]
    TypeMismatch {
        /// Source path/
        src:  PathBuf,
        /// Destination path.
        dest: PathBuf,
    },
    /// Unable to add Walker.exclude patterns to OverrideBuilder
    #[error("failed to parse excluded patterns: {0}")]
    ExcludeError(String),
    /// Unable to build OverrideBuilder
    #[error("failed to build OverrideBuilder: {0}")]
    OverrideBuildError(String),
    /// Failure to parse glob pattern
    #[error("failed to parse glob pattern: {0}")]
    GlobError(String),
    /// Failure to parse regex pattern
    #[error("failed to parse regex pattern: {0}")]
    RegexError(String),
    /// Failure to parse ignore pattern
    #[error("failed to parse ignore pattern: {0}")]
    IgnorePattern(String),
    /// Anyhow context
    #[error("failed to obtain GPGME cryptography context")]
    Context(#[source] anyhow::Error),
    /// OnceCell access context
    #[error("failed to access oncecell contents")]
    OnceCellAccess,
    /// Anyhow context
    #[error("failed to deconstruct fortress configuration")]
    DeconstructingFortress,
    /// Failure to normalize path for encryption
    #[error("failed append encryption suffix to path")]
    AppendingSuffix(#[source] anyhow::Error),
    /// Failure to remove secret suffix of file
    #[error("failed remove encryption suffix to path")]
    RemovingSuffix(#[source] anyhow::Error),
    /// Encryption failure
    #[error("failed to encrypt file")]
    Encrypt(#[source] anyhow::Error),
    /// Decryption failure
    #[error("failed to dencrypt file")]
    Decrypt(#[source] anyhow::Error),
    /// Error writing to a file
    #[error("failed to write decrypted data to file")]
    Write(#[source] std::io::Error),
}

/// A single path to hoard, with configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct Pile {
    /// Optional configuration for this path.
    pub config: Option<Config>,
    /// The path to hoard.
    ///
    /// The path is optional because it will almost always be set by processing
    /// a configuration file and it is possible that none of the environment
    /// combinations match.
    pub path:   Option<PathBuf>,
}

impl Pile {
    /// Helper function for copying files and directories.
    ///
    /// The returned [`PilePaths`] has items inserted as (src, dest).
    ///
    /// # Errors
    ///
    /// Various sorts of I/O errors as the different [`Error`] variants.
    #[allow(clippy::too_many_lines)]
    fn copy(
        &self,
        src: &Path,
        dest: &Path,
        global: &GlobalConfig,
        restore: bool,
    ) -> Result<(), Error> {
        let _span = tracing::trace_span!(
            "copy",
            source = ?src,
            destination = ?dest
        )
        .entered();

        let threads = num_cpus::get();
        let config = self.config.clone().unwrap_or_default();
        // tracing::trace!("Walker Config: {:#?}", config.walker.clone());

        let pattern = if config.walker.regex {
            config.walker.pattern
        } else {
            let builder = globset::GlobBuilder::new(&config.walker.pattern);
            builder
                .build()
                .map_err(|e| Error::GlobError(e.to_string()))?
                .regex()
                .to_owned()
        };

        let sensitive = config.walker.case_sensitive || contains_upperchar(&pattern);

        let compiled_patt = RegexBuilder::new(&pattern)
            .case_insensitive(!sensitive)
            .build()
            .map_err(|e| Error::RegexError(e.to_string()))?;

        let pattern = Arc::new(compiled_patt);

        let mut override_builder = OverrideBuilder::new(src);
        for ext in config
            .walker
            .exclude
            .iter()
            .map(|v| String::from("!") + v.as_str())
            .collect::<Vec<String>>()
        {
            override_builder
                .add(ext.as_str())
                .map_err(|e| Error::ExcludeError(e.to_string()))?;
        }

        let mut builder = WalkBuilder::new(src);
        builder
            .threads(threads)
            .follow_links(config.walker.follow_links)
            .hidden(config.walker.hidden)
            .max_depth(config.walker.max_depth)
            .ignore(false)
            .git_global(false)
            .git_ignore(false)
            .git_exclude(false)
            .parents(false)
            .overrides(
                override_builder
                    .build()
                    .map_err(|e| Error::OverrideBuildError(e.to_string()))?,
            );

        if let Some(ref ignore) = global.ignores {
            let tmp =
                create_temp_ignore(&move |file: &mut fs::File| write_temp_ignore(ignore, file));
            let res = builder.add_ignore(&tmp);
            match res {
                Some(ignore::Error::Partial(_)) | None => (),
                Some(err) => {
                    hoard_error!("{}", Error::IgnorePattern(err.to_string()));
                },
            }
            delete_file(tmp);
        }

        let walker = builder.build_parallel();
        let (tx, rx) = channel::unbounded::<ignore::DirEntry>();

        thread::spawn(|| {
            walker.run(move || {
                let tx = tx.clone();
                let pattern = Arc::clone(&pattern);

                Box::new(move |res| {
                    let entry = match res {
                        Ok(d) => d,
                        Err(e) => {
                            hoard_warn!("{}", &e);
                            return WalkState::Continue;
                        },
                    };

                    let entry_path = entry.path();

                    // Verify a file name is actually present
                    let entry_fname: Cow<OsStr> = match entry_path.file_name() {
                        Some(f) => Cow::Borrowed(f),
                        _ => unreachable!("Invalid file reached"),
                    };

                    // Filter out patterns that don't match
                    if !pattern.is_match(&osstr_to_bytes(entry_fname.as_ref())) {
                        return WalkState::Continue;
                    }

                    if tx.send(entry).is_err() {
                        tracing::trace!("WalkBuilder sent quit");
                        return WalkState::Quit;
                    }
                    WalkState::Continue
                })
            });
        });

        while let Ok(mod_src) = rx.recv() {
            tracing::trace!("Walker source: {:?}", mod_src);
            let src_path = mod_src.path();

            // Reverses path and grabs n components away from base dir
            let mod_dest = &mut PathBuf::from(dest);
            src_path
                .iter()
                .rev()
                .clone()
                .collect::<Vec<_>>()
                .drain(..mod_src.depth())
                .collect::<Vec<_>>()
                .iter()
                .rev()
                .for_each(|comp| mod_dest.push(comp));

            if let Some(file_type) = mod_src.file_type() {
                if file_type.is_dir() {
                    let _span = tracing::trace_span!("is_directory").entered();
                } else if file_type.is_file() {
                    let _span = tracing::trace_span!("is_file").entered();
                    if let Some(parent) = mod_dest.parent() {
                        if is_special_file(&mod_src) && restore {
                            continue;
                        }
                        tracing::trace!(
                            destination = src_path.to_string_lossy().as_ref(),
                            "ensuring parent directories for destination",
                        );
                        fs::create_dir_all(parent).map_err(|err| Error::CreateDir {
                            path:  mod_dest.clone(),
                            error: err,
                        })?;
                    }

                    if let Some(enc) = self.config.clone().and_then(|conf| conf.encryption) {
                        FORTRESS_INITIALIZATION.get_or_init(|| {
                            tracing::trace!("running fortress initialization");
                            build_fortress(
                                if restore { src } else { dest },
                                &self.config.clone().unwrap_or_default(),
                                global,
                            )
                            .map_err(|err| Error::Context(err.into()))
                        });

                        // TODO: use or remove fortress
                        let (_fortress, recipients) = FORTRESS_INITIALIZATION
                            .get()
                            .ok_or(Error::OnceCellAccess)?
                            .as_ref()
                            .map_err(|e| Error::Context(e.into()))?;

                        // If file is '.gpg-id' or the directory '.public-keys', do
                        // not do anything extra
                        if is_special_file(&mod_src) {
                            if !restore {
                                fs::copy(src_path.to_owned(), &mod_dest).map_err(|err| {
                                    Error::CopyFile {
                                        src:   src_path.to_owned(),
                                        dest:  mod_dest.clone(),
                                        error: err,
                                    }
                                })?;
                            }
                            continue;
                        }

                        if restore {
                            let norm = rm_sec_suffix(&mod_dest).map_err(Error::RemovingSuffix)?;
                            println!("norm :: {:#?}", norm);

                            match enc {
                                Encryption::Symmetric(e) => match e {
                                    SymmetricEncryption::Password(pass) => {
                                        println!("pass: {:#?}", pass);
                                        let plaintext =
                                            context(&self.config.clone().unwrap_or_default())
                                                .map_err(|err| Error::Context(err.into()))?
                                                .decrypt_file(src_path)
                                                .map_err(Error::Decrypt)?;
                                        fs::write(&norm, &plaintext.unsecure_ref())
                                            .map_err(Error::Write)?;
                                    },
                                    SymmetricEncryption::PasswordCmd(pass_cmd) => {
                                        println!("pass cmd: {:#?}", pass_cmd);
                                    },
                                },
                                Encryption::Asymmetric(e) => {
                                    println!("asym: {:#?}", e.public_key);
                                    let plaintext =
                                        context(&self.config.clone().unwrap_or_default())
                                            .map_err(|err| Error::Context(err.into()))?
                                            .decrypt_file(src_path)
                                            .map_err(Error::Decrypt)?;
                                    fs::write(&norm, &plaintext.unsecure_ref())
                                        .map_err(Error::Write)?;
                                },
                            }
                        } else {
                            let norm =
                                append_sec_suffix(&mod_dest).map_err(Error::AppendingSuffix)?;
                            let plaintext =
                                Plaintext::from(fs::read(&src_path).map_err(|err| {
                                    Error::ReadItem {
                                        path:  src_path.to_path_buf(),
                                        error: err,
                                    }
                                })?);

                            match enc {
                                Encryption::Symmetric(e) => match e {
                                    SymmetricEncryption::Password(pass) => {
                                        println!("pass: {:#?}", pass);
                                        context(&self.config.clone().unwrap_or_default())
                                            .map_err(|err| Error::Context(err.into()))?
                                            .encrypt_file_symmetric(plaintext, &norm)
                                            .map_err(Error::Encrypt)?;
                                    },
                                    SymmetricEncryption::PasswordCmd(pass_cmd) => {
                                        println!("pass cmd: {:#?}", pass_cmd);
                                    },
                                },
                                Encryption::Asymmetric(e) => {
                                    println!("asym: {:#?}", e.public_key);
                                    context(&self.config.clone().unwrap_or_default())
                                        .map_err(|err| Error::Context(err.into()))?
                                        .encrypt_file(recipients, plaintext, &norm)
                                        .map_err(Error::Encrypt)?;
                                },
                            }
                        }
                    } else {
                        // Copy all files as is
                        fs::copy(src_path.to_owned(), &mod_dest).map_err(|err| {
                            Error::CopyFile {
                                src:   src_path.to_owned(),
                                dest:  mod_dest.clone(),
                                error: err,
                            }
                        })?;
                        tracing::debug!(
                            source = src_path.to_string_lossy().as_ref(),
                            destination = mod_dest.to_string_lossy().as_ref(),
                            "copying",
                        );
                    }
                } else {
                    tracing::warn!(
                        source = src_path.to_string_lossy().as_ref(),
                        "source is not a file or directory",
                    );
                }
            } else {
                tracing::warn!(
                    source = mod_src.path().to_string_lossy().as_ref(),
                    "source does not have a file type",
                );
            }
        }
        Ok(())
    }

    /// Backs up files to the pile directory.
    ///
    /// `prefix` is the root directory for this pile. This should generally be
    /// `$HOARD_ROOT/$HOARD_NAME/($PILE_NAME)`.
    ///
    /// # Errors
    ///
    /// Various sorts of I/O errors as the different [`enum@Error`] variants.
    pub fn backup(&self, prefix: &Path, global: &GlobalConfig) -> Result<(), Error> {
        if let Some(path) = &self.path {
            let _span = tracing::debug_span!(
                "backup_pile",
                path = path.to_string_lossy().as_ref(),
                prefix = prefix.to_string_lossy().as_ref()
            )
            .entered();

            // if let Some(conf) = &self.config {
            //     if let Some(enc) = &conf.encryption {
            //         println!("ENCRYPT: {:#?}", enc);
            //     }
            // }

            Self::copy(self, path, prefix, global, false)?;
        } else {
            tracing::warn!("pile has no associated path -- perhaps no environment matched?");
        }

        Ok(())
    }

    /// Restores files from the hoard into the filesystem.
    ///
    /// # Errors
    ///
    /// Various sorts of I/O errors as the different [`enum@Error`] variants.
    pub fn restore(&self, prefix: &Path, global: &GlobalConfig) -> Result<(), Error> {
        // // let plain = context.decrypt_file(&path.join("aaa.txt")).expect("err
        // // decrypting"); fs::write(&path.join("aaa.txt"),
        // // plain.unsecure_ref()).expect("error writing");
        if let Some(path) = &self.path {
            let _span = tracing::debug_span!(
                "restore_pile",
                path = path.to_string_lossy().as_ref(),
                prefix = prefix.to_string_lossy().as_ref()
            )
            .entered();

            Self::copy(self, prefix, path, global, true)?;
        } else {
            tracing::warn!("pile has no associated path -- perhaps no environment matched");
        }

        Ok(())
    }
}

/// A collection of multiple related [`Pile`]s.
#[derive(Clone, Debug, PartialEq)]
pub struct MultipleEntries {
    /// The named [`Pile`]s in the hoard.
    pub piles: HashMap<String, Pile>,
}

impl MultipleEntries {
    /// Back up all of the contained [`Pile`]s.
    ///
    /// # Errors
    ///
    /// See [`Pile::backup`].
    pub fn backup(&self, prefix: &Path, global: &GlobalConfig) -> Result<(), Error> {
        for (name, entry) in &self.piles {
            let _span = tracing::info_span!(
                "backup_multi_pile",
                pile = %name
            )
            .entered();

            let sub_prefix = prefix.join(name);
            entry.backup(&sub_prefix, global)?;
        }

        Ok(())
    }

    /// Restore all of the contained [`Pile`]s.
    ///
    /// # Errors
    ///
    /// See [`Pile::restore`].
    pub fn restore(&self, prefix: &Path, global: &GlobalConfig) -> Result<(), Error> {
        for (name, entry) in &self.piles {
            let _span = tracing::info_span!(
                "restore_multi_pile",
                pile = %name
            )
            .entered();

            let sub_prefix = prefix.join(name);
            entry.restore(&sub_prefix, global)?;
        }

        Ok(())
    }
}

/// A configured hoard. May contain one or more [`Pile`]s.
#[derive(Clone, Debug, PartialEq)]
#[allow(variant_size_differences)]
pub enum Hoard {
    /// A single anonymous [`Pile`].
    Anonymous(Pile),
    /// Multiple named [`Pile`]s.
    Named(MultipleEntries),
}

impl Hoard {
    /// Back up this [`Hoard`].
    ///
    /// # Errors
    ///
    /// See [`Pile::backup`].
    pub fn backup(&self, prefix: &Path, global: &GlobalConfig) -> Result<(), Error> {
        let _span =
            tracing::trace_span!("backup_hoard", prefix = prefix.to_string_lossy().as_ref())
                .entered();

        match self {
            Hoard::Anonymous(single) => single.backup(prefix, global),
            Hoard::Named(multiple) => multiple.backup(prefix, global),
        }
    }

    /// Restore this [`Hoard`].
    ///
    /// # Errors
    ///
    /// See [`Pile::restore`].
    pub fn restore(&self, prefix: &Path, global: &GlobalConfig) -> Result<(), Error> {
        let _span =
            tracing::trace_span!("restore_hoard", prefix = prefix.to_string_lossy().as_ref(),)
                .entered();

        match self {
            Hoard::Anonymous(single) => single.restore(prefix, global),
            Hoard::Named(multiple) => multiple.restore(prefix, global),
        }
    }

    /// Returns a [`HoardPaths`] based on this `Hoard`.
    #[must_use]
    pub fn get_paths(&self) -> HoardPaths {
        match self {
            Hoard::Anonymous(pile) => pile.path.clone().into(),
            Hoard::Named(piles) => piles
                .piles
                .iter()
                .filter_map(|(key, val)| val.path.clone().map(|path| (key.clone(), path)))
                .collect::<HashMap<_, _>>()
                .into(),
        }
    }
}
