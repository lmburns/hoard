//! Utilites that deal with files and file paths
use std::{
    borrow::Cow,
    env,
    ffi::{OsStr, OsString},
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

use std::os::unix::fs::{MetadataExt, PermissionsExt};

use once_cell::sync::{Lazy, OnceCell};
use rand::{distributions::Alphanumeric, Rng};
use regex::{bytes::Regex as RegexBytes, Regex};
use thiserror::Error;

use crate::config::encrypt::{fortress::Fortress, FORTRESS_UMASK};

/// Prevent multiple compilations of the same `Regex`.
static SNEAKY_RE: OnceCell<Regex> = OnceCell::new();

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
    /// Path has invalid/sneaky characters
    #[error("{0:?} contains a sneaky pattern")]
    SneakyPath(PathBuf),
    /// Path does not have a parent path
    #[error("{0:?} doesn't have a parent")]
    InvalidParent(PathBuf),
    /// Cannot set permissions on file
    #[error("error setting permissions on {path}: {error}")]
    PermissionSet {
        /// The path of the file that is havings perms set
        path:  PathBuf,
        /// The error that occurred while setting perms
        #[source]
        error: io::Error,
    },
    /// Error accessing metadata attributes
    #[error("error accessing metadata attributes on {path}: {error}")]
    MetadataAttrs {
        /// The path of the file
        path:  PathBuf,
        /// The error that occurred while accessing meta
        #[source]
        error: io::Error,
    },
}

#[macro_export]
/// Macro to easily print an error message
macro_rules! hoard_error {
    ($($err:tt)*) => ({
        eprintln!("{}: {}", "[hoard error]".red().bold(), format!($($err)*));
    })
}
#[macro_export]
/// Macro to easily print a warning message
macro_rules! hoard_warn {
    ($($err:tt)*) => ({
        eprintln!("{}: {}", "[hoard warning]".yellow().bold(), format!($($err)*));
    })
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
    #[allow(clippy::unwrap_used)]
    static UPPER_REG: Lazy<RegexBytes> = Lazy::new(|| RegexBytes::new(r"[[:upper:]]").unwrap());
    let cow_pat: Cow<OsStr> = Cow::Owned(OsString::from(pattern));
    UPPER_REG.is_match(&osstr_to_bytes(cow_pat.as_ref()))
}

/// Create a temporary path
#[must_use]
pub fn create_temp_path() -> String {
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
pub fn modify_temp_ignore<P: AsRef<Path>>(
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
pub fn create_temp_ignore(content: &dyn Fn(&mut File) -> Result<(), Error>) -> String {
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
/// Write content to temporary ignore file
pub fn write_temp_ignore(ignores: &[String], file: &File) -> Result<(), Error> {
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
pub fn delete_file<P: AsRef<Path>>(file: P) {
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

/// Recursively set permissions on a `Fortress`
///
/// # Panics
/// On invalid metadata of file..
pub fn recursively_set_perms<P: AsRef<Path>>(path: P, fort: &Fortress) -> Result<(), Error> {
    let path = path.as_ref();
    if SNEAKY_RE
        .get_or_init(|| Regex::new("/..$|^../|/../|^..$").unwrap())
        .is_match(&(path.display().to_string()))
    {
        return Err(Error::SneakyPath(path.into()));
    }

    // Necessary to call `getuid`
    #[allow(unsafe_code)]
    let uid = unsafe { libc::getuid() };
    let path_uid = if path.exists() {
        path.metadata()?.uid()
    } else {
        uid
    };

    // Return if the file isn't owned by user (all should be)
    if path_uid != uid {
        return Ok(());
    }

    if path.is_dir() {
        let mut perms = fs::metadata(&path)
            .map_err(|err| Error::MetadataAttrs {
                path:  path.to_path_buf(),
                error: err,
            })?
            .permissions();
        perms.set_mode(perms.mode() - (perms.mode() & *FORTRESS_UMASK));

        fs::set_permissions(&path, perms).map_err(|err| Error::PermissionSet {
            path:  path.to_path_buf(),
            error: err,
        })?;

        if path == fort.root {
            return Ok(());
        }

        recursively_set_perms(
            path.parent()
                .ok_or_else(|| Error::InvalidParent(path.to_path_buf()))?,
            fort,
        )?;
    } else {
        recursively_set_perms(
            path.parent()
                .ok_or_else(|| Error::InvalidParent(path.to_path_buf()))?,
            fort,
        )?;
    }

    Ok(())
}
