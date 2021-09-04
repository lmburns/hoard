//! This module contains processed versions of builder
//! [`Hoard`](crate::config::builder::hoard::Hoard)s. See documentation for
//! builder `Hoard`s for more details.

use crate::config::encryption::configuration::GpgConfig;
pub use crate::{
    checkers::history::last_paths::HoardPaths,
    config::builder::{hoard::Config, GlobalConfig},
    utils::{
        contains_upperchar, create_temp_ignore, delete_file, osstr_to_bytes, write_temp_ignore,
    },
};

use colored::Colorize;
use crossbeam_channel as channel;
use ignore::{overrides::OverrideBuilder, WalkBuilder, WalkState};
use regex::bytes::RegexBuilder;
use std::{
    borrow::Cow,
    collections::HashMap,
    ffi::OsStr,
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
};
use thiserror::Error;

/// Errors that can happen while backing up or restoring a hoard.
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
    #[error("cannot read directory {path}: {error}")]
    ReadDir {
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
    fn copy(&self, src: &Path, dest: &Path, global: &GlobalConfig) -> Result<(), Error> {
        let _span = tracing::trace_span!(
            "copy",
            source = ?src,
            destination = ?dest
        )
        .entered();

        let threads = num_cpus::get();

        let config = self.config.clone().unwrap_or_else(Config::default);

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
                    eprintln!("{}", Error::IgnorePattern(err.to_string()));
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
                            eprintln!("{}: {}", "Warning".yellow().bold(), &e);
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

        while let Ok(src) = rx.recv() {
            tracing::trace!("Walker source: {:?}", src);
            let src_path = src.path();

            // Reverses path and grabs n components away from base dir
            let dest = &mut PathBuf::from(dest);
            src_path
                .iter()
                .rev()
                .clone()
                .collect::<Vec<_>>()
                .drain(..src.depth())
                .collect::<Vec<_>>()
                .iter()
                .rev()
                .for_each(|comp| dest.push(comp));

            if let Some(file_type) = src.file_type() {
                if file_type.is_dir() {
                    let _span = tracing::trace_span!("is_directory").entered();
                } else if file_type.is_file() {
                    let _span = tracing::trace_span!("is_file").entered();
                    if let Some(parent) = dest.parent() {
                        tracing::trace!(
                            destination = src_path.to_string_lossy().as_ref(),
                            "ensuring parent directories for destination",
                        );
                        fs::create_dir_all(parent).map_err(|err| Error::CreateDir {
                            path:  dest.clone(),
                            error: err,
                        })?;
                    }

                    tracing::debug!(
                        source = src_path.to_string_lossy().as_ref(),
                        destination = dest.to_string_lossy().as_ref(),
                        "copying",
                    );

                    fs::copy(src_path.to_owned(), &dest).map_err(|err| Error::CopyFile {
                        src:   src_path.to_owned(),
                        dest:  dest.clone(),
                        error: err,
                    })?;
                } else {
                    tracing::warn!(
                        source = src_path.to_string_lossy().as_ref(),
                        "source is not a file or directory",
                    );
                }
            } else {
                tracing::warn!(
                    source = src.path().to_string_lossy().as_ref(),
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

            Self::copy(self, path, prefix, global)?;

            if let Some(conf) = &self.config {
                if let Some(enc) = &conf.encryption {
                    let j = GpgConfig::new(enc, global);
                    println!("GPG: {:#?}", j);
                }
            }
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
        // TODO: do stuff with pile config
        if let Some(path) = &self.path {
            let _span = tracing::debug_span!(
                "restore_pile",
                path = path.to_string_lossy().as_ref(),
                prefix = prefix.to_string_lossy().as_ref()
            )
            .entered();

            Self::copy(self, prefix, path, global)?;
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
