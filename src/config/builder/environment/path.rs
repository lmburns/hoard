//! See [`PathExists`].

use serde::{Deserialize, Serialize};
use std::{
    convert::{Infallible, TryInto},
    fmt,
    fmt::Formatter,
    path::PathBuf,
};

use crate::{config::builder::hoard::Error, env_vars::expand_env_in_path};

/// A conditional structure that tests whether or not the contained path exists.
///
/// The path can be anything from a file, directory, symbolic link, or
/// otherwise, so long as *something* with that name exists.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Hash)]
#[serde(transparent)]
#[repr(transparent)]
#[allow(clippy::module_name_repetitions)]
pub struct PathExists(pub PathBuf);

impl TryInto<bool> for PathExists {
    type Error = Infallible;

    fn try_into(self) -> Result<bool, Self::Error> {
        let path_expand = expand_env_in_path(&self.to_string()).map_err(|err| Error::ExpandEnv {
            path:  self.to_string(),
            error: err,
        });

        match path_expand {
            Ok(path) => {
                tracing::trace!("checking if path \"{}\" exists", path.to_string_lossy());
                Ok(path.exists())
            },
            Err(_) => Ok(false),
        }
    }
}

impl fmt::Display for PathExists {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let PathExists(path) = self;
        write!(f, "{}", path.to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::{tempdir, NamedTempFile};

    #[test]
    fn test_file_does_exist() {
        let temp = NamedTempFile::new().expect("failed to create temporary file");
        let exists: bool = PathExists(temp.path().to_path_buf())
            .try_into()
            .expect("failed to check if path exists");
        assert!(exists);
    }

    #[test]
    fn test_dir_does_exist() {
        let temp = tempdir().expect("failed to create temporary directory");
        let exists: bool = PathExists(temp.path().to_path_buf())
            .try_into()
            .expect("failed to check if path exists");
        assert!(exists);
    }

    #[test]
    fn test_file_does_not_exist() {
        let temp = NamedTempFile::new().expect("failed to create temporary file");
        fs::remove_file(temp.path()).expect("failed to remove temporary file");
        let exists: bool = PathExists(temp.path().to_path_buf())
            .try_into()
            .expect("failed to check if path exists");
        assert!(!exists);
    }

    #[test]
    fn test_dir_does_not_exist() {
        let temp = tempdir().expect("failed to create temporary directory");
        fs::remove_dir(temp.path()).expect("failed to remove temporary directory");
        let exists: bool = PathExists(temp.path().to_path_buf())
            .try_into()
            .expect("failed to check if path exists");
        assert!(!exists);
    }
}
