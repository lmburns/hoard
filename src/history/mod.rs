//! Keep records of previous operations (including on other system) to prevent inconsistencies
//! and accidental overwrites or deletions.
//!
//! This module currently focuses on the [`last_paths`] submodule, which gives a warning and
//! aborts an operation if the paths being used differ from the previous operation.

use crate::config::get_dirs;
use std::path::PathBuf;
use std::{fs, io};
use uuid::Uuid;

pub mod last_paths;

const UUID_FILE_NAME: &str = "uuid";
const HISTORY_DIR_NAME: &str = "history";

fn get_uuid_file() -> PathBuf {
    get_dirs().config_dir().join(UUID_FILE_NAME)
}

fn get_history_root_dir() -> PathBuf {
    get_dirs().data_dir().join(HISTORY_DIR_NAME)
}

fn get_history_dir_for_id(id: Uuid) -> PathBuf {
    get_history_root_dir().join(id.to_string())
}

/// Get this machine's unique UUID, creating if necessary.
///
/// The UUID can be found in a file called "uuid" in the `hoard`
/// configuration directory. If the file cannot be found or its contents are invalid,
/// a new file is created.
///
/// # Errors
///
/// Any I/O unexpected errors that may occur while reading and/or
/// writing the UUID file.
pub fn get_or_generate_uuid() -> Result<Uuid, io::Error> {
    let uuid_file = get_uuid_file();
    let _span = tracing::debug_span!("get_uuid", file = ?uuid_file);

    tracing::debug!("attempting to read uuid from file");
    let id: Option<Uuid> = match fs::read_to_string(&uuid_file) {
        Ok(id) => match id.parse() {
            Ok(id) => {
                tracing::trace!(uuid = %id, "successfully read uuid from file");
                Some(id)
            }
            Err(err) => {
                tracing::warn!(error = %err, bad_id = %id, "failed to parse uuid in file");
                None
            }
        },
        Err(err) => {
            if err.kind() == io::ErrorKind::NotFound {
                tracing::trace!("no uuid file found: creating one");
                None
            } else {
                tracing::error!(error = %err, "error while reading uuid file");
                return Err(err);
            }
        }
    };

    // Return existing id or generate, save to file, and return.
    id.map_or_else(
        || {
            let new_id = Uuid::new_v4();
            tracing::debug!(new_uuid = %new_id, "generated new uuid");
            if let Err(err) = fs::write(&uuid_file, new_id.to_string()) {
                tracing::error!(error = %err, "error while saving uuid to file");
                return Err(err);
            }
            Ok(new_id)
        },
        Ok,
    )
}
