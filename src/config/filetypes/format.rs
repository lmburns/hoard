//! Format options for convertion between [`json`](serde_json),
//! [`yaml`](serde_yaml), and [`toml`](toml)
use std::str::FromStr;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use super::Error;

/// Base formats that are allowed
#[derive(Copy, Clone, Debug, Eq, PartialEq, EnumIter)]
pub enum ConfigFormat {
    /// JSON variant
    Json,
    /// TOML variant
    Toml,
    /// YAML variant
    Yaml,
}

impl ConfigFormat {
    /// Helper function to list possible variants of [`ConfigFormat`]
    #[allow(clippy::must_use_candidate)]
    pub fn variants() -> [&'static str; 3] {
        ["json", "toml", "yaml"]
    }

    #[must_use]
    /// Return the names of [`ConfigFormat`]
    pub fn names() -> Vec<&'static str> {
        Self::iter().map(|f| f.name()).collect()
    }

    /// Match variant names to return an `str`
    #[must_use]
    pub fn name(&self) -> &'static str {
        match *self {
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Yaml => "yaml",
        }
    }

    /// Match variant names to possible extensions of file type
    #[must_use]
    pub fn extensions(&self) -> &[&'static str] {
        match *self {
            Self::Json => &["json"],
            Self::Toml => &["toml"],
            Self::Yaml => &["yaml", "yml"],
        }
    }

    /// Test whether the input is a valid extension
    #[must_use]
    pub fn is_extension(&self, s: &str) -> bool {
        self.extensions().iter().any(|&ext| ext == s)
    }

    /// Returns variants name
    #[must_use]
    pub fn preferred_extension(&self) -> &'static str {
        self.name()
    }
}

impl FromStr for ConfigFormat {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        match s.to_ascii_lowercase().as_str() {
            "json" => Ok(ConfigFormat::Json),
            "toml" => Ok(ConfigFormat::Toml),
            "yaml" | "yml" => Ok(ConfigFormat::Yaml),
            e => Err(Error::InvalidFormat(e.to_string())),
        }
    }
}

/// Holds the format and the text that is going to be converted to another type
#[derive(Debug)]
pub struct Formatted {
    /// Format of `json`, `yaml`, or `toml` that is going to be converted
    pub format: ConfigFormat,
    /// Text that will be serialized and converted
    pub text:   String,
}

impl Formatted {
    /// Create a new instance of `Formatted`
    #[must_use]
    pub fn new(format: ConfigFormat, text: String) -> Formatted {
        Formatted { format, text }
    }

    /// Creates a new instance of `Formatted` with the newly formatted text
    ///
    /// # Errors
    /// Returns error if conversion is unsucessful
    pub fn convert_to(&self, format: ConfigFormat) -> Result<Formatted, Error> {
        match format {
            ConfigFormat::Json => self.to_json(),
            ConfigFormat::Toml => self.to_toml(),
            ConfigFormat::Yaml => self.to_yaml(),
        }
        .map(|text| Formatted { format, text })
    }

    /// Convert type to JSON
    fn to_json(&self) -> Result<String, Error> {
        let value = self.deserialize::<super::json::SValue>(&self.text)?;
        super::json::serialize(&value)
    }

    /// Convert type to TOML
    fn to_toml(&self) -> Result<String, Error> {
        let value = self.deserialize::<super::toml::SValue>(&self.text)?;
        super::toml::serialize(&value)
    }

    /// Convert type to YAML
    fn to_yaml(&self) -> Result<String, Error> {
        let value = self.deserialize::<super::yaml::SValue>(&self.text)?;
        super::yaml::serialize(&value)
    }

    /// Deserialze all types with their own deserialization function
    #[allow(single_use_lifetimes)]
    fn deserialize<V>(&self, s: &str) -> Result<V, Error>
    where
        V: for<'de> serde::Deserialize<'de>,
    {
        match self.format {
            ConfigFormat::Json => super::json::deserialize(s),
            ConfigFormat::Toml => super::toml::deserialize(s),
            ConfigFormat::Yaml => super::yaml::deserialize(s),
        }
    }
}
