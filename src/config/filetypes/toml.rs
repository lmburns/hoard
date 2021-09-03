//! Convert type to TOML
use super::Error;

pub use toml::Value as SValue;

/// Serialize TOML format
///
/// # Errors
/// Error arises during conversion of type [`serde::ser::Serialize`] to
/// [`String`] using [`toml`]
pub fn serialize<V: serde::ser::Serialize>(v: V) -> Result<String, Error> {
    let serialized = toml::to_string(&v).map_err(|e| Error::Serialization(e.to_string()))?;
    Ok(serialized)
}

/// Deserialize TOML format
///
/// # Errors
/// Error arises during conversion of type [`str`] to [`serde::de::Deserialize`]
/// using [`toml`]
#[allow(single_use_lifetimes)]
pub fn deserialize<V>(s: &str) -> Result<V, Error>
where
    V: for<'de> serde::de::Deserialize<'de>,
{
    let deserialized = toml::from_str(s).map_err(|e| Error::Deserialization(e.to_string()))?;
    Ok(deserialized)
}
