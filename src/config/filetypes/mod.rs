//! All operations that involve file type conversion or printing/colorizing of
//! configuration. Much thanks to [`bat`] and [`refmt`] binaries. Some of this
//! code is refactored from those crates.

// Contains:
// - [`ConfigConversion`]: convert configuration filetypes

pub mod assets;
pub mod format;
pub mod json;
pub mod printer;
pub mod toml;
pub mod yaml;

use std::{
    env,
    ffi::OsStr,
    fs::File,
    io::{self, stdout, BufRead, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

use self::{
    assets::HighlightAssets,
    format::{ConfigFormat, Formatted},
    printer::{HighlightTextPrinter, PlainTextPrinter, Printer},
};
use crate::config::filetypes::assets::assets_from_cache_or_binary;

use syntect::{dumps::from_binary, highlighting::ThemeSet};
use thiserror::Error;

fn get_serialized_integrated_syntaxset() -> &'static [u8] {
    include_bytes!("../../../assets/syntaxes.bin")
}

fn get_integrated_themeset() -> ThemeSet {
    from_binary(include_bytes!("../../../assets/themes.bin"))
}

// fn load_integrated_assets() -> HighlightAssets {
//     HighlightAssets::new(
//         from_binary(include_bytes!("../../../assets/syntaxes.bin")),
//         from_binary(include_bytes!("../../../assets/themes.bin")),
//     )
// }

/// Shorthand alias for Result using this modules's [`Error`]
pub type Result<T> = std::result::Result<T, Error>;

/// Errors found throughout the filetype conversion module
#[derive(Debug, Error)]
pub enum Error {
    /// Error while serializing a file
    #[error("failed to serialize file: {0}")]
    Serialization(String),
    /// Error while deserializing a file
    #[error("failed to deserialize file: {0}")]
    Deserialization(String),
    /// Error with input format
    #[error("invalid format: {0}. Supported values are: ['yaml', 'yml', 'toml', 'json']")]
    InvalidFormat(String),
    /// Error with inferring format
    #[error("unable to infer format. Supported values are: ['yaml', 'yml', 'toml', 'json']")]
    InferFormat,
    /// Normal IO error from std (allows for conversion)
    #[error("IO Error. cause:{_0}")]
    IO(#[from] io::Error),
    /// Error with converting formats
    #[error("there was an error converting formats: {0}")]
    ConversionError(String),
    /// Error with loading themes in a directory
    #[error("there was an error loading themes in directory {dir}: {error}")]
    LoadingThemeDir {
        /// Directory where error occurred
        dir:   String,
        /// Error from syntect
        #[source]
        error: syntect::LoadingError,
    },
    /// No themes found in directory
    #[error("there are no themes found in: {0}")]
    EmptyThemeDir(String),
    /// General syntect error
    #[error("syntect error: {0}")]
    SyntectGeneral(String),
    /// Error with loading syntaxes in a directory
    #[error("there was an error loading syntaxes in directory {dir}: {error}")]
    LoadingSyntaxDir {
        /// Directory where error occurred
        dir:   String,
        /// Error from syntect
        #[source]
        error: syntect::LoadingError,
    },
    /// Error with loading syntaxes in a directory
    #[error("there was an error reading file for cache {file} \n-{desc:?}: {error}")]
    ReadFile {
        /// File where error occurred
        file:  String,
        /// Description of cache
        desc:  Option<String>,
        /// Error from reading
        #[source]
        error: io::Error,
    },
    /// Error with writing syntaxes to a directory
    #[error("there was an error writing file for cache {file} \n-{desc:?}: {error}")]
    WriteFile {
        /// File where error occurred
        file:  String,
        /// Description of cache
        desc:  Option<String>,
        /// Error from reading
        error: String,
    },
}

/// Struct that holds information for converting between filetypes: json, yaml,
/// toml
#[derive(Debug, Clone)]
pub struct ConfigConversion {
    input_file:    PathBuf,
    input_format:  ConfigFormat,
    output_file:   Option<PathBuf>,
    output_format: ConfigFormat,
    theme:         String,
    color:         bool,
}

impl ConfigConversion {
    /// Creating a new instance of the struct that contains the information for
    /// the file type conversion. Information comes directly from the command
    /// line through the `crate::Command::Config` subcommand
    ///
    /// # Errors
    /// Errors only come from the inference of the input or output file types
    pub fn new(
        input_file: &Path,
        input_format: &Option<String>,
        output_file: &Option<PathBuf>,
        output_format: &Option<String>,
        theme: Option<String>,
        color: bool,
    ) -> Result<Self> {
        Ok(Self {
            input_file: input_file.to_path_buf(),
            input_format: infer_format(Some(&input_file.to_path_buf()), input_format.as_ref())?,
            output_file: output_file.clone(),
            output_format: infer_format(output_file.as_ref(), output_format.as_ref())?,
            theme: theme
                .map(String::from)
                .or_else(|| env::var("HOARD_THEME").ok())
                .map_or_else(
                    || String::from(HighlightAssets::default_theme()),
                    |s| {
                        if s == "default" {
                            String::from(HighlightAssets::default_theme())
                        } else {
                            s
                        }
                    },
                ),
            color,
        })
    }

    /// Like `Self::new()`, but isn't used for displaying output. It is intead
    /// used in the modifying subcommands to overrite the configuration file
    pub fn overwrite(input_file: &Path) -> Result<Self> {
        let format = infer_format(Some(&input_file.to_path_buf()), None)?;
        Ok(Self {
            input_file:    input_file.to_path_buf(),
            input_format:  format,
            output_file:   Some(input_file.to_path_buf()),
            output_format: format,
            theme:         String::from(HighlightAssets::default_theme()),
            color:         false,
        })
    }

    /// The actual conversion process between file types
    ///
    /// # Errors
    /// If an error occurs during the actual conversion process, one will be
    /// thrown
    pub fn run(&self) -> Result<()> {
        let _span = tracing::trace_span!("running configuration conversion").entered();
        tracing::trace!("configuration: {:?}", self);
        let assets = assets_from_cache_or_binary(self.theme != *HighlightAssets::default_theme())?;
        tracing::trace!(
            "Themes: {:?}",
            assets.themes().map(String::from).collect::<Vec<_>>()
        );
        tracing::trace!(
            "Syntaxes: {:?}",
            assets
                .get_syntaxes()?
                .iter()
                .filter(|syn| !syn.hidden && !syn.file_extensions.is_empty())
                .map(|s| s.name.clone())
                .collect::<Vec<_>>()
        );

        let input_text = self.read_from_input()?;
        let output_text = input_text
            .convert_to(self.output_format)
            .map_err(|err| Error::ConversionError(err.to_string()))?;

        self.write_to_output(&output_text, &assets, &self.theme)
    }

    /// Read file type that is specified by the '--config' option
    ///
    /// # Errors
    /// Returns errors from reading the file to a string
    pub fn read_from_input(&self) -> Result<Formatted> {
        let mut reader: Box<dyn BufRead> = Box::new(BufReader::new(File::open(&self.input_file)?));
        let mut text = String::new();
        reader.read_to_string(&mut text)?;

        Ok(Formatted::new(self.input_format, text))
    }

    /// Write file that is specified by the '--output-file' option
    ///
    /// # Errors
    /// Returns errors from creating file that is being written to
    pub(crate) fn write_to_output(
        &self,
        text: &Formatted,
        assets: &HighlightAssets,
        theme: &str,
    ) -> Result<()> {
        let stdout = stdout();
        let lock = stdout.lock();
        let mut w: Box<dyn Write> = if let Some(f) = self.output_file.as_ref() {
            Box::new(BufWriter::new(File::create(f)?))
        } else {
            Box::new(lock)
        };

        let printer: Box<dyn Printer> = if self.output_file.is_none() && self.color {
            Box::new(HighlightTextPrinter::new(assets, theme))
        } else {
            Box::new(PlainTextPrinter::default())
        };

        printer.print(&mut w, text)
    }
}

/// Infer format of config or input file
pub fn infer_format(file: Option<&PathBuf>, format_name: Option<&String>) -> Result<ConfigFormat> {
    let _span = tracing::trace_span!("inferring format").entered();
    if let Some(format_name) = format_name {
        tracing::trace!(format = ?format_name);
        ConfigFormat::from_str(format_name)
    } else if let Some(file) = file {
        tracing::trace!(format = ?format_name, file = ?file, ext = ?file.extension());
        file.extension()
            .and_then(OsStr::to_str)
            .map_or(Err(Error::InferFormat), ConfigFormat::from_str)
    } else {
        Err(Error::InferFormat)
    }
}
