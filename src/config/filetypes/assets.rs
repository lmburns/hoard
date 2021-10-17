//! [`bat`]: Assets for displaying colored output of configuration
use std::{
    fs,
    path::{Path, PathBuf},
};

use syntect::{
    dumps::{from_binary, from_reader},
    highlighting::{Theme, ThemeSet},
    parsing::{SyntaxReference, SyntaxSet},
};

use super::{get_integrated_themeset, get_serialized_integrated_syntaxset, Error, Result};
use crate::config::directories::PROJECT_DIRS;
use once_cell::sync::OnceCell;

/// Keep it in this format since we want to load it lazily
#[derive(Debug)]
pub(crate) enum SerializedSyntaxSet {
    /// The data comes from a user-generated cache file
    FromFile(PathBuf),
    /// The data to use is embedded into the binary
    FromBinary(&'static [u8]),
}

impl SerializedSyntaxSet {
    fn deserialize(&self) -> Result<SyntaxSet> {
        match self {
            SerializedSyntaxSet::FromBinary(data) => Ok(from_binary(data)),
            SerializedSyntaxSet::FromFile(ref path) => asset_from_cache(path, "syntax set"),
        }
    }
}

/// Syntax and themes stored in a struct
#[derive(Debug)]
pub(crate) struct HighlightAssets {
    /// Syntax
    pub(crate) syntax_set_cell:       OnceCell<SyntaxSet>,
    /// SynaxSet in serialized format
    pub(crate) serialized_syntax_set: Option<SerializedSyntaxSet>,
    /// Theme
    pub(crate) theme_set:             ThemeSet,
    /// Theme to fallback on if another isn't found
    pub(crate) fallback_theme:        Option<&'static str>,
}

/// Theme to fallback on if another isn't found
#[derive(Debug)]
pub(crate) struct SyntaxReferenceInSet<'a> {
    /// Theme to fallback on if another isn't found
    pub(crate) syntax:     &'a SyntaxReference,
    /// Theme to fallback on if another isn't found
    pub(crate) syntax_set: &'a SyntaxSet,
}

impl HighlightAssets {
    /// Create a new instance of [`HighlightAssets`] to store the syntax style
    /// and theme
    ///
    /// # Panics
    /// Will panic if `syntax_set` or `serialized_syntax_set` is none
    pub(crate) fn new(
        syntax_set: Option<SyntaxSet>,
        serialized_syntax_set: Option<SerializedSyntaxSet>,
        theme_set: ThemeSet,
    ) -> Self {
        assert!(syntax_set.is_some() || serialized_syntax_set.is_some());

        let syntax_set_cell = OnceCell::new();
        if let Some(syntax_set) = syntax_set {
            syntax_set_cell.set(syntax_set).expect("can never fail");
        }

        HighlightAssets {
            syntax_set_cell,
            serialized_syntax_set,
            theme_set,
            fallback_theme: None,
        }
    }

    /// Default theme to display when printing config to [`std::io::stdout`]
    #[must_use]
    pub(crate) fn default_theme() -> &'static str {
        "KimbieDark"
    }

    /// Default json theme to display when printing config to
    /// [`std::io::stdout`]
    #[must_use]
    pub(crate) fn default_json_theme() -> &'static str {
        "GitHub"
    }

    // #[cfg(feature = "build-assets")]
    /// Get assets from files in specified directory
    pub(crate) fn from_files(source_dir: &Path, include_integrated_assets: bool) -> Result<Self> {
        let mut theme_set = if include_integrated_assets {
            get_integrated_themeset()
        } else {
            ThemeSet::new()
        };

        let theme_dir = source_dir.join("themes");
        if theme_dir.exists() {
            theme_set
                .add_from_folder(&theme_dir)
                .map_err(|err| Error::LoadingThemeDir {
                    dir:   theme_dir.to_string_lossy().to_string(),
                    error: err,
                })?;
        } else {
            tracing::warn!(
                "No themes were found in '{}', using the default set",
                theme_dir.to_string_lossy()
            );
        }

        let mut syntax_set_builder = if include_integrated_assets {
            from_binary::<SyntaxSet>(get_serialized_integrated_syntaxset()).into_builder()
        } else {
            let mut builder = syntect::parsing::SyntaxSetBuilder::new();
            builder.add_plain_text_syntax();
            builder
        };

        let syntax_dir = source_dir.join("syntaxes");
        if syntax_dir.exists() {
            syntax_set_builder
                .add_from_folder(&syntax_dir, true)
                .map_err(|err| Error::LoadingSyntaxDir {
                    dir:   syntax_dir.to_string_lossy().to_string(),
                    error: err,
                })?;
        } else {
            tracing::warn!(
                "No syntaxes were found in '{}', using the default set.",
                syntax_dir.to_string_lossy()
            );
        }

        let syntax_set = syntax_set_builder.build();
        let missing_contexts = syntax_set.find_unlinked_contexts();
        if !missing_contexts.is_empty() {
            println!("Some referenced contexts could not be found!");
            for context in missing_contexts {
                println!("- {}", context);
            }
        }

        Ok(HighlightAssets::new(Some(syntax_set), None, theme_set))
    }

    /// Build [`HighlightAssets`] from cache
    ///
    /// # Errors
    /// None
    pub(crate) fn from_cache(cache_path: &Path) -> Result<Self> {
        Ok(HighlightAssets::new(
            None,
            Some(SerializedSyntaxSet::FromFile(
                cache_path.join("syntaxes.bin"),
            )),
            asset_from_cache(&cache_path.join("themes.bin"), "theme set")?,
        ))
    }

    /// Build [`HighlightAssets`] from binary assets
    ///
    /// # Errors
    /// None
    pub(crate) fn from_binary() -> Self {
        HighlightAssets::new(
            None,
            Some(SerializedSyntaxSet::FromBinary(
                get_serialized_integrated_syntaxset(),
            )),
            get_integrated_themeset(),
        )
    }

    /// Save themes and syntaxes to cache
    pub(crate) fn save_to_cache(&self, target_dir: &Path) -> Result<()> {
        #[allow(clippy::let_underscore_drop)]
        let _ = fs::create_dir_all(target_dir);
        asset_to_cache(
            self.get_theme_set(),
            &target_dir.join("themes.bin"),
            "theme set",
        )?;
        asset_to_cache(
            self.get_syntax_set()?,
            &target_dir.join("syntaxes.bin"),
            "syntax set",
        )?;

        print!(
            "Writing metadata to folder {} ... ",
            target_dir.to_string_lossy()
        );

        tracing::info!("okay");

        Ok(())
    }

    pub(crate) fn get_syntax_set(&self) -> Result<&SyntaxSet> {
        if self.syntax_set_cell.get().is_none() {
            self.syntax_set_cell
                .set(
                    self.serialized_syntax_set
                        .as_ref()
                        .expect("a dev forgot to setup serialized_syntax_set")
                        .deserialize()
                        .map_err(|err| Error::Deserialization(err.to_string()))?,
                )
                .unwrap();
        }
        // It is safe to .unwrap() because we just made sure it was .filled()
        Ok(self.syntax_set_cell.get().unwrap())
    }

    /// Return all available syntaxes in the set
    ///
    /// # Errors
    /// `SyntaxReference` build failure
    pub(crate) fn get_syntaxes(&self) -> Result<&[SyntaxReference]> {
        Ok(self.get_syntax_set()?.syntaxes())
        // self.syntax_set.syntaxes()
    }

    fn get_theme_set(&self) -> &ThemeSet {
        &self.theme_set
    }

    /// Return iterator over all themes to list them
    pub(crate) fn themes(&self) -> impl Iterator<Item = &str> {
        self.get_theme_set().themes.keys().map(AsRef::as_ref)
        // self.theme_set.themes.iter().map(|(_, v)| v).collect() -> Vec<&Theme>
    }

    /// Find syntax from extension as input
    ///
    /// # Errors
    /// `SyntaxReferenceInSet` build failure
    pub(crate) fn find_syntax_by_file_name(
        &self,
        ext: &str,
    ) -> Result<Option<SyntaxReferenceInSet>> {
        let syntax_set = self.get_syntax_set()?;
        Ok(syntax_set
            .find_syntax_by_extension(ext)
            .map(|syntax| SyntaxReferenceInSet { syntax, syntax_set }))
    }

    /// Return default theme
    #[allow(dead_code)]
    pub(crate) fn get_default_theme(&self) -> &Theme {
        self.get_theme(Self::default_theme())
    }

    /// Get the current theme
    pub(crate) fn get_theme(&self, theme: &str) -> &Theme {
        //  &self.theme_set.themes[name]
        if let Some(theme) = self.get_theme_set().themes.get(theme) {
            theme
        } else {
            if !theme.is_empty() {
                tracing::warn!("unknown theme '{}'. Using default", theme);
            }
            &self.get_theme_set().themes[self.fallback_theme.unwrap_or_else(Self::default_theme)]
        }
    }

    /// Get different theme for JSON
    #[allow(dead_code)]
    pub(crate) fn get_theme_for_syntax(&self, syntax: &SyntaxReference) -> &Theme {
        self.get_theme(if syntax.name.to_ascii_lowercase() == "json" {
            Self::default_json_theme()
        } else {
            Self::default_theme()
        })
    }
}

fn asset_to_cache<T: serde::Serialize>(asset: &T, path: &Path, description: &str) -> Result<()> {
    print!("Writing {} to {} ... ", description, path.to_string_lossy());
    syntect::dumps::dump_to_file(asset, &path).map_err(|err| Error::WriteFile {
        file:  path.to_string_lossy().to_string(),
        desc:  Some(description.to_owned()),
        error: err.to_string(),
    })?;
    tracing::info!("okay");
    Ok(())
}

fn asset_from_cache<T: serde::de::DeserializeOwned>(path: &Path, description: &str) -> Result<T> {
    let contents = fs::read(path).map_err(|err| Error::ReadFile {
        file:  path.to_string_lossy().to_string(),
        desc:  Some(description.to_owned()),
        error: err,
    })?;
    from_reader(&contents[..]).map_err(|err| Error::SyntectGeneral(err.to_string()))
}

fn clear_asset(filename: &str, description: &str) {
    tracing::info!("Clearing {} ... ", description);
    fs::remove_file(PROJECT_DIRS.cache_dir().join(filename)).ok();
    tracing::info!("okay");
}

/// Clear syntect cache from [`XDG_DATA_HOME`] directory
pub(crate) fn clear_assets() {
    clear_asset("themes.bin", "theme set cache");
    clear_asset("syntaxes.bin", "syntax set cache");
}

/// Get assets from configuration directory or from the builtin to binary
///
/// # Errors
/// Returns `None` otherwise
pub(crate) fn assets_from_cache_or_binary(use_custom_assets: bool) -> Result<HighlightAssets> {
    let cache_dir = PROJECT_DIRS.cache_dir();

    if use_custom_assets {
        tracing::trace!("using assets from cache");
        HighlightAssets::from_cache(cache_dir)
    } else {
        tracing::trace!("using assets from binary");
        Ok(HighlightAssets::from_binary())
    }
}

fn build_assets(source: Option<&String>, dest: Option<&String>) -> Result<()> {
    let source_dir = source.map_or_else(|| PROJECT_DIRS.config_dir(), Path::new);
    let dest_dir = dest.map_or_else(|| PROJECT_DIRS.cache_dir(), Path::new);
    tracing::debug!(
        "building cache from: {}, to: {}",
        source_dir.display(),
        dest_dir.display()
    );

    // Set to true to automatically include binary themes/syntaxes
    // This program is not a colorizing like bat, so this really isn't needed
    let assets = HighlightAssets::from_files(source_dir, true)?;
    assets.save_to_cache(dest_dir)
}

/// Run `build` or `clear` cache command
pub(crate) fn run_cache(
    build: bool,
    clear: bool,
    source: &Option<String>,
    dest: &Option<String>,
) -> Result<()> {
    if build {
        build_assets(source.as_ref(), dest.as_ref())?;
    } else if clear {
        clear_assets();
    }

    Ok(())
}
