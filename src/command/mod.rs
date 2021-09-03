//! See [`Command`].
use crate::config::filetypes::format::ConfigFormat;
use std::path::PathBuf;
use structopt::{clap, StructOpt};
use thiserror::Error;

/// Errors that can occur while running commands.
#[derive(Debug, Error)]
pub enum Error {
    /// Error occurred while printing the help message.
    #[error("error while printing help message: {0}")]
    PrintHelp(#[from] structopt::clap::Error),
}

/// The possible subcommands for `hoard`.
#[allow(variant_size_differences)]
#[derive(Clone, PartialEq, Debug, StructOpt)]
#[structopt(
    global_settings = &[
        clap::AppSettings::ColoredHelp,
        clap::AppSettings::ColorAlways,
        clap::AppSettings::DisableHelpSubcommand,
        clap::AppSettings::VersionlessSubcommands,
        clap::AppSettings::InferSubcommands, // v|va|val... == validate, etc
    ]
)]
pub enum Command {
    /// Operations on configuration file
    Config {
        /// Has to be used to actually convert filetypes
        #[structopt(name = "convert", short = "x", long = "convert", takes_value = false)]
        convert:       bool,
        /// Input format (optional, uses --config <file>)
        #[structopt(
            short = "i", long = "input-format",
            takes_value = true,
            value_name = "format",
            possible_values = &ConfigFormat::variants()
        )]
        input_format:  Option<String>,
        /// Output format
        #[structopt(
            short = "f", long = "output-format",
            takes_value = true,
            value_name = "format",
            requires = "convert",
            possible_values = &ConfigFormat::variants()
        )]
        output_format: Option<String>,
        /// Output file path
        #[structopt(
            short = "o",
            long = "output-file",
            takes_value = true,
            requires = "convert",
            value_name = "file"
        )]
        output_file:   Option<PathBuf>,
        /// Theme to use  (WIP)
        #[structopt(short = "t", long = "theme", takes_value = true)]
        theme:         Option<String>,
        /// Colorize output of configuration (WIP)
        #[structopt(short = "C", long = "color", takes_value = false)]
        color:         Option<bool>,
    },
    /// Loads all configuration for validation.
    /// If the configuration loads and builds, this command succeeds.
    Validate,
    /// Back up the given hoard(s).
    Backup {
        /// The name(s) of the hoard(s) to back up. Will back up all hoards if
        /// empty.
        hoards: Vec<String>,
    },
    /// Restore the files from the given hoard to the filesystem.
    Restore {
        /// The name(s) of the hoard(s) to restore. Will restore all hoards if
        /// empty.
        hoards: Vec<String>,
    },
    /// Add to configuration file
    Add {
        /// Add an environment to file
        #[structopt(short = "e", long = "env", group = "modify")]
        env: Option<String>,
    },
}

impl Default for Command {
    fn default() -> Self {
        Self::Validate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_command_is_help() {
        // The default command is validate if one is not given
        assert_eq!(Command::Validate, Command::default());
    }
}
