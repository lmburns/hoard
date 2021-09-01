//! This module contains processed versions of builder
//! [`Hoard`](crate::config::builder::hoard::Hoard)s. See documentation for
//! builder `Hoard`s for more details.

pub use super::builder::hoard::Config;
use crossbeam_channel as channel;
use ignore::{WalkBuilder, WalkState};
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
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
    /// # Errors
    ///
    /// Various sorts of I/O errors as the different [`Error`] variants.
    fn copy(&self, src: &Path, dest: &Path) -> Result<(), Error> {
        let _span = tracing::trace_span!(
            "copy",
            source = ?src,
            destination = ?dest
        )
        .entered();

        println!("CONFIG: {:?}", self.config);
        println!("PaTH: {:?}", self.path);

        let mut builder = WalkBuilder::new(src);

        builder
            .threads(num_cpus::get())
            .follow_links(false)
            .hidden(true)
            .ignore(false)
            .git_global(false)
            .git_ignore(false)
            .git_exclude(false)
            .parents(false);
        // .overrides(overrides)
        // .max_depth(epth);

        let walker = builder.build_parallel();
        let (tx, rx) = channel::unbounded::<ignore::DirEntry>();

        thread::spawn(|| {
            walker.run(move || {
                let tx = tx.clone();

                Box::new(move |res| {
                    let entry = match res {
                        Ok(d) => d,
                        Err(e) => {
                            eprintln!("Warning: {}", &e);
                            return WalkState::Continue;
                        },
                    };

                    if tx.send(entry).is_err() {
                        return WalkState::Quit;
                    }
                    WalkState::Continue
                })
            });
        });

        while let Ok(src) = rx.recv() {
            tracing::trace!("Walker source: {:?}", src);
            println!("SOURCE: {}", src.path().display());
            let src_path = src.path();
            if let Some(file_type) = src.file_type() {
                if file_type.is_dir() {
                    // Nothing to do here
                    // WalkDir is smart enough to handle directories and files
                    let _span = tracing::trace_span!("is_directory").entered();
                } else if file_type.is_file() {
                    let dest = &dest.join(src.file_name());
                    println!("DEST aFTER: {}", dest.display());
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

                    fs::copy(src_path.to_owned(), dest).map_err(|err| Error::CopyFile {
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
    pub fn backup(&self, prefix: &Path) -> Result<(), Error> {
        // TODO: do stuff with pile config
        if let Some(path) = &self.path {
            let _span = tracing::debug_span!(
                "backup_pile",
                path = path.to_string_lossy().as_ref(),
                prefix = prefix.to_string_lossy().as_ref()
            )
            .entered();

            Self::copy(self, path, prefix)?;
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
    pub fn restore(&self, prefix: &Path) -> Result<(), Error> {
        // TODO: do stuff with pile config
        if let Some(path) = &self.path {
            let _span = tracing::debug_span!(
                "restore_pile",
                path = path.to_string_lossy().as_ref(),
                prefix = prefix.to_string_lossy().as_ref()
            )
            .entered();

            Self::copy(self, prefix, path)?;
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
    pub piles: BTreeMap<String, Pile>,
}

impl MultipleEntries {
    /// Back up all of the contained [`Pile`]s.
    ///
    /// # Errors
    ///
    /// See [`Pile::backup`].
    pub fn backup(&self, prefix: &Path) -> Result<(), Error> {
        for (name, entry) in &self.piles {
            let _span = tracing::info_span!(
                "backup_multi_pile",
                pile = %name
            )
            .entered();

            let sub_prefix = prefix.join(name);
            entry.backup(&sub_prefix)?;
        }

        Ok(())
    }

    /// Restore all of the contained [`Pile`]s.
    ///
    /// # Errors
    ///
    /// See [`Pile::restore`].
    pub fn restore(&self, prefix: &Path) -> Result<(), Error> {
        for (name, entry) in &self.piles {
            let _span = tracing::info_span!(
                "restore_multi_pile",
                pile = %name
            )
            .entered();

            let sub_prefix = prefix.join(name);
            entry.restore(&sub_prefix)?;
        }

        Ok(())
    }
}

/// A configured hoard. May contain one or more [`Pile`]s.
#[derive(Clone, Debug, PartialEq)]
#[allow(variant_size_differences)]
pub enum Hoard {
    /// A single anonymous [`Pile`].
    Single(Pile),
    /// Multiple named [`Pile`]s.
    Multiple(MultipleEntries),
}

impl Hoard {
    /// Back up this [`Hoard`].
    ///
    /// # Errors
    ///
    /// See [`Pile::backup`].
    pub fn backup(&self, prefix: &Path) -> Result<(), Error> {
        let _span =
            tracing::trace_span!("backup_hoard", prefix = prefix.to_string_lossy().as_ref())
                .entered();

        match self {
            Hoard::Single(single) => single.backup(prefix),
            Hoard::Multiple(multiple) => multiple.backup(prefix),
        }
    }

    /// Restore this [`Hoard`].
    ///
    /// # Errors
    ///
    /// See [`Pile::restore`].
    pub fn restore(&self, prefix: &Path) -> Result<(), Error> {
        let _span =
            tracing::trace_span!("restore_hoard", prefix = prefix.to_string_lossy().as_ref(),)
                .entered();

        match self {
            Hoard::Single(single) => single.restore(prefix),
            Hoard::Multiple(multiple) => multiple.restore(prefix),
        }
    }
}
