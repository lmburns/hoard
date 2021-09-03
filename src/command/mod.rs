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
        /// Whether to convert file-type
        #[structopt(name = "convert", short = "x", long = "convert", takes_value = false)]
        convert:       bool,
        /// Input file format
        #[structopt(
            short = "i", long = "input-format",
            takes_value = true,
            value_name = "format",
            possible_values = &ConfigFormat::variants()
        )]
        input_format:  Option<String>,
        /// Output file format
        #[structopt(
            short = "f", long = "output-format",
            takes_value = true,
            value_name = "format",
            possible_values = &ConfigFormat::variants()
        )]
        output_format: Option<String>,
        /// Output file path
        #[structopt(
            short = "o",
            long = "output-file",
            takes_value = true,
            value_name = "file"
        )]
        output_file:   Option<PathBuf>,
        /// Theme to use for colored output
        #[structopt(short = "t", long = "theme", takes_value = true, requires = "color")]
        theme:         Option<String>,
        /// Whether to color output
        #[structopt(name = "color", short = "C", long = "color", takes_value = false)]
        color:         bool,
        /// Build cache for custom themes
        #[structopt(
            name = "cache_build",
            short = "B", long = "cache-build",
            takes_value = false,
            conflicts_with_all = &["cache_clear", "convert"]
        )]
        cache_build:   bool,
        /// Clear cache for custom themes
        #[structopt(
            name = "cache_clear",
            short = "R", long = "cache-clear",
            takes_value = false,
            conflicts_with_all = &["cache_build", "convert"],
        )]
        cache_clear:   bool,
        /// Source path to build or clear
        #[structopt(
            name = "cache_source",
            long = "source",
            short = "s",
            takes_value = true,
            requires = "cache_build"
        )]
        source:        Option<String>,
        /// Destination path to build or clear
        #[structopt(
            name = "cache_dest",
            long = "destination",
            short = "d",
            takes_value = true,
            requires = "cache_build"
        )]
        dest:          Option<String>,
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
    /// Add item to configuration file
    Add {
        /// Add an environment to file
        #[structopt(short = "e", long = "env", group = "modify")]
        env:     Option<String>,
        /// Add a pattern to global ignores
        #[structopt(short = "i", long = "ignores", group = "modify")]
        ignores: Option<String>,
    },
}

// #[allow(non_camel_case_types)]
// #[derive(Debug, StructOpt)]
// pub enum CacheCommands {
//     build,
//     clear
// }

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
