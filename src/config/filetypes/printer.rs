//! Highlight and print configuration when converting file
//! A lot of this code is taken from [`refmt`](https://github.com/yoshihitoh/refmt)

use super::{assets::HighlightAssets, format::Formatted, Error};
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
#[derive(Debug)]
pub struct PlainTextPrinter {}

impl Default for PlainTextPrinter {
    fn default() -> Self {
        PlainTextPrinter {}
    }
}

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
}

impl<'a> HighlightTextPrinter<'a> {
    /// Create a new instance of [`HighlightTextPrinter`] to store highlight
    /// assets
    pub fn new(assets: &'a HighlightAssets) -> Self {
        HighlightTextPrinter { assets }
    }
}

#[allow(single_use_lifetimes)]
impl<'a> Printer for HighlightTextPrinter<'a> {
    /// Implementation of the [`Printer`] trait for [`HighlightTextPrinter`]
    fn print(&self, dest: &mut dyn Write, text: &Formatted) -> Result<(), Error> {
        let syntax = self.assets.get_syntax(text.format.preferred_extension());
        let theme = self.assets.get_theme_for_syntax(syntax);
        let mut highlight = HighlightLines::new(syntax, theme);
        let ranges = highlight.highlight(&text.text, &self.assets.syntax_set);
        let escaped = as_24_bit_terminal_escaped(&ranges, true);
        writeln!(dest, "{}", escaped).map_err(Error::from)
    }
}
