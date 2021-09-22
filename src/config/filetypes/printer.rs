//! Highlight and print configuration when converting file
//! A lot of this code is taken from [`refmt`](https://github.com/yoshihitoh/refmt)

use super::{
    assets::{HighlightAssets, SyntaxReferenceInSet},
    format::Formatted,
    Error,
};
use std::io::Write;
use syntect::{easy::HighlightLines, util::as_24_bit_terminal_escaped};

/// Trait that allows printing to file out [`std::io::stdout`]
pub trait Printer {
    /// Printing of output
    ///
    /// # Errors
    /// Error arises during the writing to buffer process
    fn print(&self, dest: &mut dyn Write, text: &Formatted) -> Result<(), Error>;
}

/// Empty struct with no Style or Color associated with it
#[derive(Debug, Default)]
pub struct PlainTextPrinter {}

impl Printer for PlainTextPrinter {
    /// Trait implementation to print plain text to file out [`std::io::stdout`]
    ///
    /// # Errors
    /// Error arises during the writing to buffer process
    fn print(&self, dest: &mut dyn Write, text: &Formatted) -> Result<(), Error> {
        writeln!(dest, "{}", text.text.as_str()).map_err(Error::IO)
    }
}

/// Hold assets to print colored output
#[derive(Debug)]
pub struct HighlightTextPrinter<'a> {
    /// Assets needed to display highlighted lines
    assets: &'a HighlightAssets,
    theme:  String,
}

impl<'a> HighlightTextPrinter<'a> {
    /// Create a new instance of [`HighlightTextPrinter`] to store highlight
    /// assets
    pub(crate) fn new(assets: &'a HighlightAssets, theme: &str) -> Self {
        HighlightTextPrinter {
            assets,
            theme: theme.to_owned(),
        }
    }
}

impl Printer for HighlightTextPrinter<'_> {
    /// Implementation of the [`Printer`] trait for [`HighlightTextPrinter`]
    fn print(&self, dest: &mut dyn Write, text: &Formatted) -> Result<(), Error> {
        let syntax = self
            .assets
            .find_syntax_by_file_name(text.format.preferred_extension())?;

        let syntax_in_set = if let Some(syn) = syntax {
            syn
        } else {
            let syntax_set = self.assets.get_syntax_set()?;
            let syntax = syntax_set.find_syntax_plain_text();
            SyntaxReferenceInSet { syntax, syntax_set }
        };

        // let theme = self.assets.get_theme_for_syntax(syntax);
        let theme = self.assets.get_theme(&self.theme);
        let mut highlight = HighlightLines::new(syntax_in_set.syntax, theme);
        let ranges = highlight.highlight(&text.text, syntax_in_set.syntax_set);
        let escaped = as_24_bit_terminal_escaped(&ranges, true);
        writeln!(dest, "{}", escaped).map_err(Error::from)
    }
}
