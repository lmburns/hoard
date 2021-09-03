//! Utilites that deal with files and file paths
use std::{
    borrow::Cow,
    env,
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

// use crate::config::builder::Builder;

use once_cell::sync::Lazy;
use rand::{distributions::Alphanumeric, Rng};
use regex::bytes::Regex;
use thiserror::Error;

/// Errors that can happen when using utilties that mainly have to do with files
/// and file paths
#[derive(Debug, Error)]
pub enum Error {
    /// Error while syncing a file
    #[error("failed to sync file {path}: {error}")]
    SyncFile {
        /// The path of the file that is being synced
        path:  PathBuf,
        /// The error that occurred while syncing
        #[source]
        error: io::Error,
    },
    /// Error while writing to a file
    #[error("failed to write to file {path}: {error}")]
    WriteFile {
        /// The path of the file that's being written to
        path:  PathBuf,
        /// The error that occurred while writing
        error: String,
    },
    /// Error while writing buffer to a file
    #[error("failed to write buffer to file {meta:?}: {error}")]
    WriteBufferToFile {
        /// The path of the file that's being written to
        meta:  Option<fs::Metadata>,
        /// The error that occurred while writing
        #[source]
        error: io::Error,
    },
    /// Error while creating a file
    #[error("failed to create file {path}: {error}")]
    CreateFile {
        /// The path of the file to create
        path:  PathBuf,
        /// The error that occurred while creating the file
        #[source]
        error: io::Error,
    },
    /// Error while flushing a file
    #[error("failed to flush file {path}: {error}")]
    FlushFile {
        /// The path of the file to flush
        path:  PathBuf,
        /// The error that occurred while flushing the file
        #[source]
        error: io::Error,
    },
    /// Error while opening a file
    #[error("failed to open file {path}: {error}")]
    OpenFile {
        /// The path of the file to open
        path:  PathBuf,
        /// The error that occurred while opening the file
        #[source]
        error: io::Error,
    },
    /// Error while reading a file
    #[error("failed to read file {path}: {error}")]
    ReadFile {
        /// The path of the file to open
        path:  PathBuf,
        /// The error that occurred while opening the file
        #[source]
        error: io::Error,
    },
    /// Error while creating a file
    #[error("failed to create temporary directory {path}: {error}")]
    CreateTempdir {
        /// The path of the directory to create
        path:  PathBuf,
        /// The error that occurred while creating the directory
        #[source]
        error: io::Error,
    },
    /// Error while getting cwd
    #[error("failed to get current directory: {0}")]
    GetCurrentDir(#[from] io::Error),
    /// Error while reading a yaml file
    #[error("failed to read YAML file {path}: {error}")]
    YAMLRead {
        /// The path of the directory to create
        path:  PathBuf,
        /// The error that occurred while reading the yaml file
        #[source]
        error: serde_yaml::Error,
    },
    /// Error while writing a yaml file
    #[error("failed to write YAML file {path}: {error}")]
    YAMLWrite {
        /// The path of the directory to create
        path:  PathBuf,
        /// The error that occurred while writing the yaml file
        #[source]
        error: serde_yaml::Error,
    },
    /// Error while reading a toml file
    #[error("failed to read TOML file {path}: {error}")]
    TOMLRead {
        /// The path of the directory to create
        path:  PathBuf,
        /// The error that occurred while reading the toml file
        #[source]
        error: toml::de::Error,
    },
    /// Error while writing a toml file
    #[error("failed to write TOML file {path}: {error}")]
    TOMLWrite {
        /// The path of the directory to create
        path:  PathBuf,
        /// The error that occurred while writing the toml file
        #[source]
        error: toml::ser::Error,
    },
    /// Error retrieving file metadata
    #[error("error retrieving file metadata: {0}")]
    MetadataRetrieval(#[source] io::Error),
}

/// Convert search string to bytes
#[must_use]
pub fn osstr_to_bytes(input: &OsStr) -> Cow<[u8]> {
    use std::os::unix::ffi::OsStrExt;
    Cow::Borrowed(input.as_bytes())
}

/// Match uppercase characters against Unicode characters as well. Tags can also
/// be any valid Unicode character
pub fn contains_upperchar(pattern: &str) -> bool {
    static UPPER_REG: Lazy<Regex> = Lazy::new(|| Regex::new(r"[[:upper:]]").unwrap());
    let cow_pat: Cow<OsStr> = Cow::Owned(OsString::from(pattern));
    UPPER_REG.is_match(&osstr_to_bytes(cow_pat.as_ref()))
}

/// Create a temporary path
pub(crate) fn create_temp_path() -> String {
    let mut tmp_path = env::temp_dir();
    tmp_path.push(format!(
        "{}-{}",
        env!("CARGO_PKG_NAME"),
        rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(12)
            .map(char::from)
            .collect::<String>()
    ));
    tmp_path.display().to_string()
}

/// Write to the tempoary ignore file
pub(crate) fn modify_temp_ignore<P: AsRef<Path>>(
    path: P,
    content: &dyn Fn(&mut File) -> Result<(), Error>,
) -> Result<PathBuf, Error> {
    let res = File::create(&path);
    let path = path.as_ref().to_path_buf();

    match res {
        Ok(mut fd) => match content(&mut fd) {
            Ok(_) => match fd.sync_all() {
                Ok(_) => Ok(path),
                Err(e) => Err(Error::SyncFile { path, error: e }),
            },
            Err(e) => Err(Error::WriteFile {
                path,
                error: e.to_string(),
            }),
        },
        Err(e) => Err(Error::CreateFile { path, error: e }),
    }
}

/// Wrapper function for `self::modify_temp_ignore` to handle errors at a higher
/// level
pub(crate) fn create_temp_ignore(content: &dyn Fn(&mut File) -> Result<(), Error>) -> String {
    let tmp = create_temp_path();
    match modify_temp_ignore(&tmp, content) {
        Ok(tmp) => tmp.display().to_string(),
        Err(e) => {
            eprintln!("unable to create temporary ignore file: {} {}", tmp, e);
            std::process::exit(1);
        },
    }
}

// Unnecessary wrap is used to propogate the correct errors
pub(crate) fn write_temp_ignore(ignores: &[String], file: &File) -> Result<(), Error> {
    let mut writer = io::BufWriter::new(file);

    for i in ignores.iter() {
        writeln!(&mut writer, "{}", i).map_err(|err| Error::WriteBufferToFile {
            meta:  file.metadata().ok(),
            error: err,
        })?;
    }

    Ok(())
}

/// Delete temporary ignore file
pub(crate) fn delete_file<P: AsRef<Path>>(file: P) {
    let path = file.as_ref().to_path_buf();

    if path.exists() && path.is_file() {
        match fs::remove_file(&path) {
            Ok(_) => tracing::debug!("Ignore file deleted: {}", &path.display()),
            Err(err) => tracing::debug!(
                "Unable to delete ignore file: {} {:#?}",
                &path.display(),
                err
            ),
        }
    } else {
        println!();
    }
}
