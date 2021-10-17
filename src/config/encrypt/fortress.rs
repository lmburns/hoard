//! Helper functions used with [`Fortress`] (an encrypted directory)

use std::{
    ffi::OsString,
    fs,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

use crate::env_vars;
use anyhow::Result;
use ignore::WalkBuilder;
use thiserror::Error;

use crate::{
    config::builder::{
        hoard::{Config as HoardConfig, Encryption},
        GlobalConfig,
    },
    utils::recursively_set_perms,
};

use super::{
    prelude::*, recipients::Recipients, selection::select_key, types::Plaintext, utils, Config,
    ContextPool, Engine, Key, FORTRESS_UMASK,
};

// TODO: choose
/// Encryption filename suffix
pub const SECRET_SUFFIX: &str = ".gpg";
/// Fortress file to store `gpg` id
pub const FORTRESS_GPG_ID: &str = ".gpg-id";
/// Fortress directory name where public keys are stored
pub const FORTRESS_PUB_KEY: &str = ".public-keys/";
/// Engine used for encryption within the fortress
pub const FORTRESS_ENGINE: Engine = Engine::Gpg;

/// Fortress encryption error
#[derive(Debug, Error)]
pub enum Error {
    /// Writing a file error
    #[error("failed to write to file")]
    WriteFile(#[source] std::io::Error),
    /// Reading a file error
    #[error("failed to read from file")]
    ReadFile(#[source] std::io::Error),
    /// Failure in syncing keys
    #[error("failed to sync public key files")]
    SyncKeyFiles(#[source] std::io::Error),
    /// Error expanding path
    #[error("failed to expand store root path")]
    ExpandPath(#[source] crate::env_vars::Error),
    /// Path is not a direcory
    #[error("failed to open password store, not a directory: {0}")]
    NoRootDir(PathBuf),
    /// Path is a special file that we do not want to encrypt (don't care for
    /// error message)
    #[error("")]
    SpecialFile(#[source] anyhow::Error),
    /// Tar
    #[error("cannot use directory as target without name hint")]
    TargetDirWithoutNamehint(PathBuf),
    /// Anyhow context
    #[error("failed to obtain GPGME cryptography context")]
    Context(#[source] anyhow::Error),
    /// Failed to access store
    #[error("failed to access initialized password store")]
    Fortress(#[source] anyhow::Error),
    /// Fortress has no recipients
    #[error("invalid fortress recipients")]
    InvalidFortressRecipients(#[source] anyhow::Error),
    /// No private keys available
    #[error("no private keys are available")]
    NoPrivateKeys(#[source] anyhow::Error),
    /// No GPG key selected
    #[error("no key selected")]
    NoKeySelected,
    /// Failed encryption
    #[error("failed to encrypt file")]
    EncryptionFailure(#[source] anyhow::Error),
}

/// Represents a an encrypted [`Hoard`]
#[derive(Debug, Clone)]
pub struct Fortress {
    /// Root directory of the fortress (absolute path)
    pub root: PathBuf,
}

impl Fortress {
    /// Open a fortress at the given path
    pub fn open(root: &Path) -> Result<Self> {
        let root: PathBuf = env_vars::expand_env_in_path(root).map_err(Error::ExpandPath)?;

        anyhow::ensure!(root.is_dir(), Error::NoRootDir(root));
        tracing::trace!(?root, "Successfully opened fortress");

        Ok(Self { root })
    }

    /// Get the recipient keys for this fortress
    pub fn recipients(&self) -> Result<Recipients> {
        Recipients::load(self)
    }

    /// Create secret iterator for this store.
    #[must_use]
    pub fn secret_iter(&self) -> SecretIter {
        SecretIter::new(self.root.clone())
    }

    /// Return a vector of secrets
    #[must_use]
    pub fn secrets(&self) -> Vec<Secret> {
        self.secret_iter().collect()
    }
}

/// Return GPG ID file for a fortress
#[must_use]
pub fn fortress_gpg_ids_file(fortress: &Fortress) -> PathBuf {
    fortress.root.join(FORTRESS_GPG_ID)
}

/// Return public keys directory for a fortress
#[must_use]
pub fn fortress_public_keys_dir(fortress: &Fortress) -> PathBuf {
    fortress.root.join(FORTRESS_PUB_KEY)
}

/// Normalizes a path, returning encrypted suffix file name
/// This function is specific to `SECRET_SUFFIX` compared to the other mentioned
/// below that is a generalized function
// NOTE: Similar function .. config::encrypt::utils::append_file_name
pub fn append_sec_suffix<P: AsRef<Path>>(target: P) -> Result<PathBuf> {
    let mut path = PathBuf::from(target.as_ref());

    // Add secret extension if non existent
    let ext: OsString = SECRET_SUFFIX.trim_start_matches('.').into();
    if path.extension() != Some(&ext) {
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(SECRET_SUFFIX);
        path = PathBuf::from(tmp);
    }

    Ok(path)
}

/// Normalizes a path, removing encrypted suffix file name
pub fn rm_sec_suffix<P: AsRef<Path>>(target: P) -> Result<PathBuf> {
    let mut path = PathBuf::from(target.as_ref());

    // Add secret extension if non existent
    let ext: OsString = SECRET_SUFFIX.trim_start_matches('.').into();

    if path.extension() == Some(&ext) {
        if let Some(stem) = path.file_stem() {
            if let Some(parent) = path.parent() {
                path = parent.join(stem);
            } else {
                path = PathBuf::from(stem);
            }
        }
    }

    Ok(path)
}

/// Read GPG fingerprints from fortress
pub fn fortress_read_gpg_fingerprints(fortress: &Fortress) -> Result<Vec<String>> {
    let path = fortress_gpg_ids_file(fortress);
    path.is_file()
        .then(|| read_fingerprints(path))
        .unwrap_or_else(|| Ok(vec![]))
}

/// Write GPG fingerprints to a fortress
///
/// Overwrites any existing file.
pub fn fortress_write_gpg_fingerprints<S: AsRef<str>>(
    fortress: &Fortress,
    fingerprints: &[S],
) -> Result<()> {
    write_fingerprints(fortress_gpg_ids_file(fortress), fingerprints)
}

/// Read fingerprints from the given file.
fn read_fingerprints<P: AsRef<Path>>(path: P) -> Result<Vec<String>> {
    Ok(fs::read_to_string(path)
        .map_err(Error::ReadFile)?
        .lines()
        .filter(|fp| !fp.trim().is_empty())
        .map(Into::into)
        .collect())
}

/// Write fingerprints to the given file.
fn write_fingerprints<P: AsRef<Path>, S: AsRef<str>>(path: P, fingerprints: &[S]) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .mode(0o666 - (0o666 & *FORTRESS_UMASK))
        .truncate(true)
        .write(true)
        .create(true)
        .open(&path)?;

    file.write_all(
        fingerprints
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>()
            .join("\n")
            .as_bytes(),
    )
    .map_err(|err| Error::WriteFile(err).into())
}

/// Load the keys for the given fortress
///
/// This will try to load the keys for all configured protocols, and errors if
/// it fails.
pub fn fortress_load_keys(fortress: &Fortress) -> Result<Vec<Key>> {
    let mut keys = Vec::new();
    let fingerprints = fortress_read_gpg_fingerprints(fortress)?;

    if !fingerprints.is_empty() {
        let mut context = super::context(&Config::default())?;
        let fingerprints: Vec<_> = fingerprints.iter().map(String::as_str).collect();
        keys.extend(context.find_public_keys(&fingerprints)?);
    }

    Ok(keys)
}

/// Load the recipients for the given fortress.
///
/// This will try to load the recipient keys for all configured protocols, and
/// errors if it fails.
pub fn fortress_load_recipients(fortress: &Fortress) -> Result<Recipients> {
    tracing::trace!("loading recipients");
    Ok(Recipients::from(fortress_load_keys(fortress)?))
}

/// Save the keys for the given fortress.
///
/// This overwrites any existing recipient keys.
pub fn fortress_save_keys(fortress: &Fortress, keys: &[Key]) -> Result<()> {
    // Save GPG keys
    let gpg_fingerprints: Vec<_> = keys
        .iter()
        .filter(|key| key.protocol() == FORTRESS_ENGINE)
        .map(|key| key.fingerprint(false))
        .collect();
    fortress_write_gpg_fingerprints(fortress, &gpg_fingerprints)?;

    // Sync public keys for all proto's
    fortress_sync_public_key_files(fortress, keys)?;

    Ok(())
}

/// Save the keys for the given fortress.
///
/// This overwrites any existing recipient keys.
pub fn fortress_save_recipients(fortress: &Fortress, recipients: &Recipients) -> Result<()> {
    fortress_save_keys(fortress, recipients.keys())
}

/// Sync public key files in store with selected recipients.
///
/// - Removes obsolete keys that are not a selected recipient
/// - Adds missing keys that are a recipient
///
/// This syncs public key files for all protocols. This is because the public
/// key files themselves don't specify what protocol they use. All public key
/// files and keys must therefore be taken into consideration all at once.
pub fn fortress_sync_public_key_files(fortress: &Fortress, keys: &[Key]) -> Result<()> {
    // Get public keys directory, ensure it exists
    let dir = fortress_public_keys_dir(fortress);
    fs::create_dir_all(&dir).map_err(Error::SyncKeyFiles)?;
    recursively_set_perms(&dir, fortress)?;

    // List key files in keys directory
    let files: Vec<(PathBuf, String)> = dir
        .read_dir()
        .map_err(Error::SyncKeyFiles)?
        .filter_map(Result::ok)
        .filter(|e| e.file_type().map(|f| f.is_file()).unwrap_or(false))
        .filter_map(|e| {
            e.file_name()
                .to_str()
                .map(|fp| (e.path(), utils::format_fingerprint(fp)))
        })
        .collect();

    // Remove unused keys
    for (path, _) in files
        .iter()
        .filter(|(_, fp)| !utils::keys_contain_fingerprint(keys, fp))
    {
        fs::remove_file(path).map_err(Error::SyncKeyFiles)?;
    }

    // Add missing keys
    let mut contexts = ContextPool::empty();
    for (key, fp) in keys
        .iter()
        .map(|k| (k, k.fingerprint(false)))
        .filter(|(_, fp)| !files.iter().any(|(_, other)| fp == other))
    {
        // Lazy load compatible context
        let proto = key.protocol();
        let config = Config::from(proto);
        let context = contexts.get_mut(&config)?;

        // Export public key to disk
        let path = dir.join(&fp);
        context.export_key_file(key.clone(), &path)?;
    }

    // NEWPROTO: if a new proto is added, public keys should be synced here

    Ok(())
}

/// Recipients extension for fortress functionality
pub trait FortressRecipients {
    /// Load recipients from given fortress
    fn load(fortress: &Fortress) -> Result<Recipients>;

    /// Save recipients to given fortress
    fn save(&self, fortress: &Fortress) -> Result<()>;
}

impl FortressRecipients for Recipients {
    /// Load recipients from given fortress
    fn load(fortress: &Fortress) -> Result<Recipients> {
        fortress_load_recipients(fortress)
    }

    /// Save recipients to given fortress
    fn save(&self, fortress: &Fortress) -> Result<()> {
        fortress_save_recipients(fortress, self)
    }
}

/// A fortress secret (encrypted file)
#[derive(Debug, Clone)]
pub struct Secret {
    /// Display name of the secret, relative path to the fortress
    pub name: String,

    /// Full path to the password fortress secret
    pub path: PathBuf,
}

impl Secret {
    /// Construct secret at given full path from given fortress
    #[must_use]
    pub fn from(fortress: &Fortress, path: PathBuf) -> Self {
        Self::in_root(&fortress.root, path)
    }

    /// Construct secret at given path in the given password fortress root
    #[must_use]
    pub fn in_root(root: &Path, path: PathBuf) -> Self {
        let name: String = relative_path(root, &path)
            .ok()
            .and_then(Path::to_str)
            .map_or_else(|| "?", |f| f.trim_end_matches(SECRET_SUFFIX))
            .to_owned();
        Self { name, path }
    }

    // /// Get relative path to this secret, root must be given.
    // pub fn relative_path<'a>(
    //     &'a self,
    //     root: &'a Path,
    // ) -> Result<&'a Path, std::path::StripPrefixError> {
    //     relative_path(root, &self.path)
    // }
}

/// Get relative path in given root.
pub fn relative_path<'a>(
    root: &'a Path,
    path: &'a Path,
) -> Result<&'a Path, std::path::StripPrefixError> {
    path.strip_prefix(&root)
}

/// Print the given plaintext to stdout.
pub fn print_secret(plaintext: &Plaintext) -> Result<(), std::io::Error> {
    let mut stdout = std::io::stdout();

    stdout.write_all(plaintext.unsecure_ref())?;

    // Always finish with newline
    if let Some(&last) = plaintext.unsecure_ref().last() {
        if last != b'\n' {
            stdout.write_all(&[b'\n'])?;
        }
    }

    stdout.flush().expect("error flushing stdout");
    Ok(())
}

/// Iterator that walks through password store secrets.
///
/// This walks all password store directories, and yields password secrets.
#[allow(missing_debug_implementations)]
pub struct SecretIter {
    /// Root of the store to walk.
    root: PathBuf,

    /// Directory walker
    walker: Box<dyn Iterator<Item = ignore::DirEntry>>,
}

impl SecretIter {
    /// Create new fortress secret iterator at given store root.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        let mut walker = WalkBuilder::new(&root);
        Self {
            root,
            walker: Box::new(
                walker
                    .follow_links(true)
                    .ignore(false)
                    .git_global(false)
                    .git_ignore(false)
                    .git_exclude(false)
                    .parents(false)
                    .build()
                    .into_iter()
                    .filter_map(Result::ok)
                    .filter(is_secret_file)
                    .filter(|f| !is_special_file(f)),
            ),
        }
    }
}

impl Iterator for SecretIter {
    type Item = Secret;

    fn next(&mut self) -> Option<Self::Item> {
        self.walker
            .next()
            .map(|e| Secret::in_root(&self.root, e.path().into()))
    }
}

/// Check if file is file or directory containing public keys
#[must_use]
pub fn is_special_file(entry: &ignore::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map_or(false, |f| f == FORTRESS_PUB_KEY || f == FORTRESS_GPG_ID) // 1 layer deep
        || entry
            .path()
            .to_string_lossy()
            .to_string()
            .contains(FORTRESS_PUB_KEY) // prevent traversal into directory
        || entry
            .path()
            .parent()
            .map(|f| f.to_string_lossy().to_string())
            .map_or(false, |f| f == FORTRESS_PUB_KEY)
}

/// Check if given `WalkDir` `DirEntry` is hidden sub-directory.
#[allow(unused)]
fn is_hidden_subdir(entry: &ignore::DirEntry) -> bool {
    (entry.depth() > 0 || entry.file_type().map_or(false, |dir| dir.is_dir()))
        && entry
            .file_name()
            .to_str()
            .map_or(false, |s| s.starts_with('.'))
}

/// Check if given `WalkBuilder` `DirEntry` is a secret file
#[must_use]
pub fn is_secret_file(entry: &ignore::DirEntry) -> bool {
    entry.file_type().map_or(false, |file| file.is_file())
        && entry
            .file_name()
            .to_str()
            .map_or(false, |s| s.ends_with(SECRET_SUFFIX))
}

/// Check whether we can decrypt the first secret in the fortress.
///
/// If decryption fails, and this returns false, it means we don't own any
/// compatible secret key.
///
/// Returns true if there is no secret.
#[must_use]
pub fn can_decrypt(fortress: &Fortress) -> bool {
    // Try all proto's here once we support more
    fortress.secret_iter().next().map_or(true, |secret| {
        super::context(&Config::default()).map_or(false, |mut context| {
            context.can_decrypt_file(&secret.path).unwrap_or(true)
        })
    })
}

/// Build the fortress using the base path from the `WalkBuilder`. This will
/// check for an existing `Fortress`, and if one doesn't exist, it will create
/// one. Returns the context from the `gpgme` wrapper
pub fn build_fortress(
    src: &Path,
    config: &HoardConfig,
    global: &GlobalConfig,
) -> Result<(Fortress, Recipients), Error> {
    let _span = tracing::trace_span!("building fortress").entered();
    let fortress = Fortress::open(src).map_err(Error::Fortress)?;

    let mut recipients = fortress
        .recipients()
        .map_err(Error::InvalidFortressRecipients)?;

    let mut context = utils::context(config).map_err(|err| Error::Context(err.into()))?;

    if utils::check_existing_fortress(src).is_ok() {
        let mut tmp = Recipients::from(context.keys_private().map_err(Error::NoPrivateKeys)?);
        tmp.remove_all(recipients.keys());

        let mut check_keys = |public: &str| -> Option<&Key> {
            tmp.keys().iter().find(|key| {
                public == key.fingerprint(false)
                    || public == key.fingerprint(true)
                    || context
                        .user_emails()
                        .iter()
                        .any(|emails| emails.iter().any(|email| email == public))
            })
        };

        // TODO: Fix extremely ugly if statements
        let key = if let Some(Encryption::Asymmetric(enc)) = config.clone().encryption {
            if let Some(public) = enc.public_key {
                let public = public
                    .trim()
                    .strip_prefix("0x")
                    .unwrap_or_else(|| public.trim());

                if let Some(key) = check_keys(public) {
                    key
                // // Should global key be provided as fallback for wrong local
                // key? } else if let Some(global) =
                // global.clone().public_key {     if let
                // Some(key) = check_keys(&global) {         key
                //     } else {
                //         select_key(tmp.keys(),
                // None).ok_or(Error::NoKeySelected)?     }
                } else {
                    select_key(tmp.keys(), None).ok_or(Error::NoKeySelected)?
                }
            } else if let Some(global) = global.clone().public_key {
                if let Some(key) = check_keys(&global) {
                    key
                } else {
                    select_key(tmp.keys(), None).ok_or(Error::NoKeySelected)?
                }
            } else {
                select_key(tmp.keys(), None).ok_or(Error::NoKeySelected)?
            }
        } else {
            // Don't think this will ever be called since it is if encryption is not present
            select_key(tmp.keys(), None).ok_or(Error::NoKeySelected)?
        };

        recipients.add(key.clone());
        recipients.save(&fortress).expect("error saving fortress");
    };

    Ok((fortress, recipients))
}
