//! See [`Config`].
use directories::{BaseDirs, ProjectDirs};
use once_cell::sync::Lazy;
use std::{
    borrow::Cow,
    env,
    path::{Path, PathBuf},
};

/// Static used to easily get user directories that differ on macOS and not
/// macOS
pub static PROJECT_DIRS: Lazy<HoardProjectDirs> =
    Lazy::new(|| HoardProjectDirs::new().expect("could not get home directory"));

/// Get config directory with `static PROJECT_DIRS`
#[must_use]
pub fn config_dir() -> Cow<'static, str> {
    PROJECT_DIRS.config_dir().to_string_lossy()
}

/// Get cache directory with `static PROJECT_DIRS`
#[must_use]
pub fn cache_dir() -> Cow<'static, str> {
    PROJECT_DIRS.cache_dir().to_string_lossy()
}

/// Get local data directory with `static PROJECT_DIRS`
#[must_use]
pub fn data_dir() -> Cow<'static, str> {
    PROJECT_DIRS.data_dir().to_string_lossy()
}

/// Get the project directories for this project.
#[derive(Debug)]
pub struct HoardProjectDirs {
    cache_dir:  PathBuf,
    config_dir: PathBuf,
    data_dir:   PathBuf,
}

impl HoardProjectDirs {
    fn new() -> Option<HoardProjectDirs> {
        let cache_dir = HoardProjectDirs::get_cache_dir()?;
        let data_dir = HoardProjectDirs::get_data_dir()?;

        let config_dir =
            if let Some(config_dir_op) = env::var_os("HOARD_CONFIG_DIR").map(PathBuf::from) {
                config_dir_op
            } else {
                #[cfg(target_os = "macos")]
                let config_dir_op = env::var_os("XDG_CONFIG_HOME")
                    .map(PathBuf::from)
                    .filter(|p| p.is_absolute())
                    .or_else(|| {
                        BaseDirs::new()
                            .map(|p| p.home_dir().to_owned())
                            .map(|p| p.join(".config"))
                    });

                #[cfg(not(target_os = "macos"))]
                let config_dir_op = get_dirs().config_dir().to_path_buf();

                config_dir_op.map(|d| d.join("hoard"))?
            };

        Some(HoardProjectDirs {
            cache_dir,
            config_dir,
            data_dir,
        })
    }

    fn get_cache_dir() -> Option<PathBuf> {
        // on all OS prefer BAT_CACHE_PATH if set
        let cache_dir_op = env::var_os("HOARD_CACHE_DIR").map(PathBuf::from);
        if cache_dir_op.is_some() {
            return cache_dir_op;
        }

        #[cfg(target_os = "macos")]
        let cache_dir_op = env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                BaseDirs::new()
                    .map(|p| p.home_dir().to_owned())
                    .map(|p| p.join(".cache"))
            });

        #[cfg(not(target_os = "macos"))]
        let cache_dir_op = get_dirs().cache_dir().to_path_buf();

        cache_dir_op.map(|d| d.join("hoard"))
    }

    fn get_data_dir() -> Option<PathBuf> {
        // on all OS prefer BAT_CACHE_PATH if set
        let cache_dir_op = env::var_os("HOARD_DATA_DIR").map(PathBuf::from);
        if cache_dir_op.is_some() {
            return cache_dir_op;
        }

        #[cfg(target_os = "macos")]
        let cache_dir_op = env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| {
                BaseDirs::new()
                    .map(|p| p.home_dir().to_owned())
                    .map(|p| p.join(".local").join("share"))
            });

        #[cfg(not(target_os = "macos"))]
        let cache_dir_op = get_dirs().data_dir().to_path_buf();

        cache_dir_op.map(|d| d.join("hoard"))
    }

    /// Get cache directory
    #[must_use]
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Get configuration directory
    #[must_use]
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    /// Get local data directory
    #[must_use]
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Get all user directories (not for macOS)
    #[must_use]
    pub fn get_dirs() -> ProjectDirs {
        tracing::trace!("determining project default folders");
        ProjectDirs::from("com", "shadow53_lmburns", "hoard")
            .expect("could not detect user home directory to place program files")
    }
}
