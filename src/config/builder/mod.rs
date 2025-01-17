//! The [`Builder`] struct serves as an intermediate step between raw
//! configuration and the [`Config`] type that is used by `hoard`.
use std::{
    collections::HashMap,
    convert::TryInto,
    ffi::OsStr,
    io,
    path::{Path, PathBuf},
};

// use normpath::PathExt;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use thiserror::Error;

use self::hoard::Hoard;
use environment::Environment;

use crate::{
    command::Command, config::directories::PROJECT_DIRS, CONFIG_FILE_NAME, HOARDS_DIR_SLUG,
};

use super::Config;

pub mod environment;
pub mod envtrie;
pub mod hoard;

/// Errors that can happen when using a [`Builder`].
#[derive(Debug, Error)]
pub enum Error {
    /// Error while parsing a TOML configuration file.
    #[error("failed to parse toml configuration file: {0}")]
    DeserializeTomlConfig(toml::de::Error),
    /// Error while parsing a YAML configuration file.
    #[error("failed to parse yaml configuration file: {0}")]
    DeserializeYamlConfig(serde_yaml::Error),
    /// Error while parsing a JSON configuration file.
    #[error("failed to parse json configuration file: {0}")]
    DeserializeJsonConfig(serde_json::Error),
    /// Error while reading from a configuration file.
    #[error("failed to read configuration file: {0}")]
    ReadConfig(io::Error),
    /// Error while determining whether configured environments apply.
    #[error("failed to determine current environment: {0}")]
    Environment(#[from] environment::Error),
    /// Error while determining which paths to use for configured hoards.
    #[error("failed to process hoard configuration: {0}")]
    ProcessHoard(#[from] hoard::Error),
}

/// Global configuration that applies to each of the hoards
/// Ingores is the only option as of now, but the individual hoard
/// configuration options will be available for the global as well.
#[allow(clippy::struct_excessive_bools)]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", default)]
pub struct GlobalConfig {
    /// Global ignore patterns that mimic git's ignore patterns
    pub ignores:    Option<Vec<String>>,
    /// Public GPG key
    pub public_key: Option<String>,
}

/// Intermediate data structure to build a [`Config`](crate::config::Config).
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, StructOpt)]
#[structopt(rename_all = "kebab")]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct Builder {
    #[structopt(skip)]
    #[serde(rename = "envs")]
    environments:  Option<HashMap<String, Environment>>,
    #[structopt(skip)]
    exclusivity:   Option<Vec<Vec<String>>>,
    #[structopt(short, long)]
    hoards_root:   Option<PathBuf>,
    #[structopt(short, long)]
    #[serde(skip)]
    config_file:   Option<PathBuf>,
    #[serde(skip)]
    #[structopt(subcommand)]
    command:       Option<Command>,
    #[serde(skip)]
    #[structopt(short, long)]
    force:         bool,
    #[structopt(skip)]
    hoards:        Option<HashMap<String, Hoard>>,
    #[structopt(skip)]
    global_config: Option<GlobalConfig>,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    /// Returns the default path for the configuration file.
    fn default_config_file() -> PathBuf {
        tracing::debug!("getting default configuration file");
        PROJECT_DIRS.config_dir().join(CONFIG_FILE_NAME)
    }

    /// Returns the default location for storing hoards.
    fn default_hoard_root() -> PathBuf {
        tracing::debug!("getting default hoard root");
        PROJECT_DIRS.data_dir().join(HOARDS_DIR_SLUG)
    }

    /// Create a new `Builder`.
    ///
    /// If [`build`](Builder::build) is immediately called on this, the returned
    /// [`Config`] will have all default values.
    #[must_use]
    pub fn new() -> Self {
        tracing::trace!("creating new config builder");
        Self {
            hoards:        None,
            hoards_root:   None,
            config_file:   None,
            command:       None,
            environments:  None,
            exclusivity:   None,
            force:         false,
            global_config: None,
        }
    }

    /// Create a new [`Builder`] pre-populated with the contents of the given
    /// TOML file.
    ///
    /// # Errors
    ///
    /// Variants of [`enum@Error`] related to reading and parsing the file.
    pub fn from_file_toml(path: &Path) -> Result<Self, Error> {
        tracing::debug!("reading configuration from \"{}\"", path.to_string_lossy());
        let s = std::fs::read_to_string(path).map_err(Error::ReadConfig)?;
        toml::from_str(&s).map_err(Error::DeserializeTomlConfig)
    }

    /// Create a new [`Builder`] pre-populated with the contents of the given
    /// YAML file.
    ///
    /// # Errors
    ///
    /// Variants of [`enum@Error`] related to reading and parsing the file.
    pub fn from_file_yaml(path: &Path) -> Result<Self, Error> {
        tracing::debug!("reading configuration from \"{}\"", path.to_string_lossy());
        let s = std::fs::read_to_string(path).map_err(Error::ReadConfig)?;
        serde_yaml::from_str(&s).map_err(Error::DeserializeYamlConfig)
    }

    /// Create a new [`Builder`] pre-populated with the contents of the given
    /// JSON file.
    ///
    /// # Errors
    ///
    /// Variants of [`enum@Error`] related to reading and parsing the file.
    pub fn from_file_json(path: &Path) -> Result<Self, Error> {
        tracing::debug!("reading configuration from \"{}\"", path.to_string_lossy());
        let s = std::fs::read_to_string(path).map_err(Error::ReadConfig)?;
        serde_json::from_str(&s).map_err(Error::DeserializeJsonConfig)
    }

    /// Helper method to process command-line arguments and the config file
    /// specified on CLI (or the default).
    ///
    /// # Errors
    ///
    /// See [`Builder::from_file`]
    pub fn from_args_then_file() -> Result<Self, Error> {
        tracing::debug!("loading configuration from cli arguments");
        let from_args = Self::from_args();

        tracing::trace!("attempting to get configuration file from cli arguments or use default");
        let config_file = from_args
            .config_file
            .clone()
            .unwrap_or_else(Self::default_config_file);

        // .map_or_else(Self::default_config_file, |p| {
        //     p.normalize()
        //         .and_then(|pb| {
        //             if pb.is_absolute() {
        //                 Ok(pb.as_path().to_path_buf())
        //             } else {
        //                 env::current_dir().map(|cwd| cwd.join(pb))
        //             }
        //         })
        //   .unwrap_or_else(Self::default_config_file)
        // });

        tracing::trace!(
            ?config_file,
            "configuration file is \"{}\"",
            config_file.to_string_lossy()
        );

        let from_file = match config_file.extension().and_then(OsStr::to_str) {
            Some("yaml" | "yml") => Self::from_file_yaml(&config_file)?,
            Some("json") => Self::from_file_json(&config_file)?,
            _ => Self::from_file_toml(&config_file)?,
        };

        tracing::debug!("merging configuration file and cli arguments");
        Ok(from_file.layer(from_args))
    }

    /// Applies all configured values in `other` over those in *this*
    /// `ConfigBuilder`.
    #[must_use]
    pub fn layer(mut self, other: Self) -> Self {
        let _span = tracing::trace_span!(
            "layering_config_builders",
            top_layer = ?other,
            bottom_layer = ?self
        )
        .entered();

        if let Some(path) = other.hoards_root {
            self = self.set_hoards_root(path);
        }

        if let Some(path) = other.config_file {
            self = self.set_config_file(path);
        }

        if let Some(path) = other.command {
            self = self.set_command(path);
        }

        self.force = self.force || other.force;

        self
    }

    /// Set the hoards map.
    #[must_use]
    pub fn set_hoards(mut self, hoards: HashMap<String, Hoard>) -> Self {
        tracing::trace!(?hoards, "setting hoards");
        self.hoards = Some(hoards);
        self
    }

    /// Set the directory that will contain all game save data.
    #[must_use]
    pub fn set_hoards_root(mut self, path: PathBuf) -> Self {
        tracing::trace!(
            hoards_root = ?path,
            "setting hoards root",
        );
        self.hoards_root = Some(path);
        self
    }

    /// Set the file that contains configuration.
    ///
    /// This currently only exists for completeness. You probably want
    /// [`Builder::from_file`] instead, which will actually read and parse
    /// the file.
    #[must_use]
    pub fn set_config_file(mut self, path: PathBuf) -> Self {
        tracing::trace!(
            config_file = ?path,
            "setting config file",
        );
        self.config_file = Some(path);
        self
    }

    /// Set the command that will be run.
    #[must_use]
    pub fn set_command(mut self, cmd: Command) -> Self {
        tracing::trace!(command = ?cmd, "setting command");
        self.command = Some(cmd);
        self
    }

    /// Set whether to force the command to run despite possible failed checks.
    #[must_use]
    pub fn set_force(mut self, force: bool) -> Self {
        tracing::trace!(?force, "setting force");
        self.force = force;
        self
    }

    /// Unset the hoards map
    #[must_use]
    pub fn unset_hoards(mut self) -> Self {
        tracing::trace!("unsetting hoards");
        self.hoards = None;
        self
    }

    /// Unset the directory that will contain all game save data.
    #[must_use]
    pub fn unset_hoards_root(mut self) -> Self {
        tracing::trace!("unsetting hoards root");
        self.hoards_root = None;
        self
    }

    /// Unset the file that contains configuration.
    #[must_use]
    pub fn unset_config_file(mut self) -> Self {
        tracing::trace!("unsetting config file");
        self.config_file = None;
        self
    }

    /// Unset the command that will be run.
    #[must_use]
    pub fn unset_command(mut self) -> Self {
        tracing::trace!("unsetting command");
        self.command = None;
        self
    }

    /// Set whether to force the command to run despite possible failed checks.
    #[must_use]
    pub fn unset_force(mut self) -> Self {
        tracing::trace!("unsetting force");
        self.force = false;
        self
    }

    /// Evaluates the stored environment definitions and returns a mapping of
    /// environment name to (boolean) whether that environment applies.
    ///
    /// # Errors
    ///
    /// Any error that occurs while evaluating the environments.
    fn evaluated_environments(
        &self,
    ) -> Result<HashMap<String, bool>, <Environment as TryInto<bool>>::Error> {
        let _span = tracing::trace_span!("eval_env").entered();
        if let Some(envs) = &self.environments {
            for (key, env) in envs {
                tracing::trace!(%key, %env);
            }
        }

        self.environments.as_ref().map_or_else(
            || Ok(HashMap::new()),
            |map| {
                map.iter()
                    .map(|(key, env)| Ok((key.clone(), env.clone().try_into()?)))
                    .collect()
            },
        )
    }

    /// Build this [`Builder`] into a [`Config`].
    ///
    /// # Errors
    ///
    /// Any [`enum@Error`] that occurs while evaluating environment or hoard
    /// definitions.
    pub fn build(self) -> Result<Config, Error> {
        tracing::debug!("building configuration from builder");
        let environments = self.evaluated_environments()?;
        tracing::debug!(?environments);
        let exclusivity = self.exclusivity.unwrap_or_else(Vec::new);
        tracing::debug!(?exclusivity);
        let hoards_root = self.hoards_root.unwrap_or_else(Self::default_hoard_root);
        tracing::debug!(?hoards_root);
        let config_file = self.config_file.unwrap_or_else(Self::default_config_file);
        tracing::debug!(?config_file);
        let command = self.command.unwrap_or_default();
        tracing::debug!(?command);
        let force = self.force;
        tracing::debug!(?force);
        let global_config = self.global_config.unwrap_or_default();
        tracing::debug!(?global_config);

        tracing::debug!("processing hoards...");
        let hoards = self
            .hoards
            .unwrap_or_else(HashMap::new)
            .into_iter()
            .map(|(name, hoard)| {
                let _span = tracing::debug_span!("processing_hoard", %name).entered();
                Ok((name, hoard.process_with(&environments, &exclusivity)?))
            })
            .collect::<Result<_, Error>>()?;
        tracing::debug!("processed hoards");

        Ok(Config {
            command,
            hoards_root,
            config_file,
            global_config,
            hoards,
            force,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod builder {
        use super::*;

        fn get_default_populated_builder() -> Builder {
            Builder {
                hoards_root:   Some(Builder::default_hoard_root()),
                config_file:   Some(Builder::default_config_file()),
                command:       Some(Command::Validate),
                environments:  None,
                exclusivity:   None,
                hoards:        None,
                force:         false,
                global_config: None,
            }
        }

        fn get_non_default_populated_builder() -> Builder {
            Builder {
                hoards_root:   Some(PathBuf::from("/testing/saves")),
                config_file:   Some(PathBuf::from("/testing/config.toml")),
                command:       Some(Command::Restore {
                    hoards: vec!["test".into()],
                }),
                environments:  None,
                exclusivity:   None,
                hoards:        None,
                force:         false,
                global_config: None,
            }
        }

        #[test]
        fn default_builder_is_new() {
            assert_eq!(Builder::new(), Builder::default());
        }

        #[test]
        fn new_builder_is_all_none() {
            let expected = Builder {
                hoards_root:   None,
                config_file:   None,
                command:       None,
                environments:  None,
                hoards:        None,
                exclusivity:   None,
                force:         false,
                global_config: None,
            };

            assert_eq!(
                expected,
                Builder::new(),
                "ConfigBuild::new() should have all None fields"
            );
        }

        #[test]
        fn layered_builder_prefers_some_over_none() {
            let some = get_default_populated_builder();
            let none = Builder::new();

            assert_ne!(some, none, "both builders cannot be identical");

            assert_eq!(
                some,
                none.clone().layer(some.clone()),
                "Some fields atop None prefers Some"
            );
            assert_eq!(
                some,
                some.clone().layer(none),
                "None fields atop Some prefers Some"
            );
        }

        #[test]
        fn layered_builder_prefers_argument_to_self() {
            let layer1 = get_default_populated_builder();
            let layer2 = get_non_default_populated_builder();

            assert_eq!(
                layer2,
                layer1.clone().layer(layer2.clone()),
                "layer() should prefer the argument"
            );
            assert_eq!(
                layer1,
                layer2.layer(layer1.clone()),
                "layer() should prefer the argument"
            );
        }

        #[test]
        fn builder_saves_root_sets_correctly() {
            let mut builder = Builder::new();
            assert_eq!(None, builder.hoards_root, "saves_root should start as None");
            let path = PathBuf::from("/testing/saves");
            builder = builder.set_hoards_root(path.clone());
            assert_eq!(
                Some(path),
                builder.hoards_root,
                "saves_root should now be set"
            );
        }

        #[test]
        fn builder_config_file_sets_correctly() {
            let mut builder = Builder::new();
            assert_eq!(
                None, builder.config_file,
                "config_file should start as None"
            );
            let path = PathBuf::from("/testing/config.toml");
            builder = builder.set_config_file(path.clone());
            assert_eq!(
                Some(path),
                builder.config_file,
                "config_file should now be set"
            );
        }

        #[test]
        fn builder_command_sets_correctly() {
            let mut builder = Builder::new();
            assert_eq!(None, builder.command, "command should start as None");
            let cmd = Command::Validate;
            builder = builder.set_command(cmd.clone());
            assert_eq!(Some(cmd), builder.command, "command should now be set");
        }

        #[test]
        fn builder_saves_root_unsets_correctly() {
            let mut builder = Builder::new();
            let path = PathBuf::from("/testing/saves");
            builder = builder.set_hoards_root(path.clone());
            assert_eq!(
                Some(path),
                builder.hoards_root,
                "saves_root should start as set"
            );
            builder = builder.unset_hoards_root();
            assert_eq!(None, builder.hoards_root, "saves_root should now be None");
        }

        #[test]
        fn builder_config_file_unsets_correctly() {
            let mut builder = Builder::new();
            let path = PathBuf::from("/testing/config.toml");
            builder = builder.set_config_file(path.clone());
            assert_eq!(
                Some(path),
                builder.config_file,
                "config_file should start as set"
            );
            builder = builder.unset_config_file();
            assert_eq!(None, builder.config_file, "config_file should now be None");
        }

        #[test]
        fn builder_command_unsets_correctly() {
            let mut builder = Builder::new();
            let cmd = Command::Validate;
            builder = builder.set_command(cmd.clone());
            assert_eq!(Some(cmd), builder.command, "command should start as set");
            builder = builder.unset_command();
            assert_eq!(None, builder.command, "command should now be None");
        }

        #[test]
        fn builder_with_nothing_set_uses_defaults() {
            // get_default_populated_builder is assumed to use all default values
            // for the purposes of this test.
            let builder = get_default_populated_builder();
            let config = Builder::new().build().expect("failed to build config");

            assert_eq!(Some(config.hoards_root), builder.hoards_root);
            assert_eq!(Some(config.config_file), builder.config_file);
            assert_eq!(Some(config.command), builder.command);
        }

        #[test]
        fn builder_with_options_set_uses_options() {
            let builder = get_non_default_populated_builder();
            let config = builder.clone().build().expect("failed to build config");

            assert_eq!(Some(config.hoards_root), builder.hoards_root);
            assert_eq!(Some(config.config_file), builder.config_file);
            assert_eq!(Some(config.command), builder.command);
        }
    }
}
