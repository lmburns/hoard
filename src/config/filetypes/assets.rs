//! Assets for displaying colored output of configuration
use syntect::{
    highlighting::{Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};

/// Syntax and themes stored in a struct
#[derive(Debug)]
pub struct HighlightAssets {
    /// Syntax
    pub syntax_set: SyntaxSet,
    /// Theme
    pub theme_set:  ThemeSet,
}

impl HighlightAssets {
    /// Create a new instance of [`HighlightAssets`] to store the syntax style
    /// and theme
    pub fn new(syntax_set: SyntaxSet, theme_set: ThemeSet) -> Self {
        HighlightAssets {
            syntax_set,
            theme_set,
        }
    }

    /// Default theme to display when printing config to [`std::io::stdout`]
    #[must_use]
    pub fn default_theme() -> &'static str {
        "Monokai Extended"
    }

    /// Default json theme to display when printing config to
    /// [`std::io::stdout`]
    #[must_use]
    pub fn default_json_theme() -> &'static str {
        "Monokai JSON"
    }

    /// Get current syntax
    /// # Panics
    /// Failure in retrieving corrent syntax
    pub fn get_syntax(&self, name: &str) -> &SyntaxReference {
        self.syntax_set.find_syntax_by_extension(name).unwrap()
    }

    /// List available syntaxes
    pub fn syntaxes(&self) -> &[SyntaxReference] {
        self.syntax_set.syntaxes()
    }

    /// Return default theme
    pub fn get_default_theme(&self) -> &Theme {
        self.get_theme(Self::default_theme())
    }

    /// Get current theme
    pub fn get_theme(&self, name: &str) -> &Theme {
        &self.theme_set.themes[name]
    }

    /// Get theme depending on syntax
    pub fn get_theme_for_syntax(&self, syntax: &SyntaxReference) -> &Theme {
        self.get_theme(if syntax.name.to_ascii_lowercase() == "json" {
            Self::default_theme()
        } else {
            Self::default_json_theme()
        })
    }

    /// Iterate over possible themes
    pub fn themes(&self) -> Vec<&Theme> {
        self.theme_set.themes.iter().map(|(_, v)| v).collect()
    }
}
