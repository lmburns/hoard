//! Convert type to YAML
use super::Error;

pub use serde_yaml::Value as SValue;

/// Serialize YAML format
///
/// # Errors
/// Error arises during conversion of type [`serde::ser::Serialize`] to
/// [`String`] using [`toml`]
pub fn serialize<V: serde::ser::Serialize>(v: V) -> Result<String, Error> {
    let serialized = serde_yaml::to_string(&v)
        .map_err(|e| Error::Serialization(e.to_string()))?
        .trim_end()
        .to_string();
    Ok(serialized)
}

/// Deserialize YAML format
///
/// # Errors
/// Error arises during conversion of type [`str`] to [`serde::de::Deserialize`]
/// using [`serde_yaml`]
#[allow(single_use_lifetimes)]
pub fn deserialize<V>(s: &str) -> Result<V, Error>
where
    V: for<'de> serde::de::Deserialize<'de>,
{
    serde_yaml::from_str(s).map_err(|e| Error::Deserialization(e.to_string()))
}
