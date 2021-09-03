//! Convert type to JSON
use super::Error;

pub use serde_json::Value as SValue;

/// Serialize JSON format
///
/// # Errors
/// Error arises during conversion of type [`serde::ser::Serialize`] to
/// [`String`] using [`serde_json`]
pub fn serialize<V: serde::ser::Serialize>(v: V) -> Result<String, Error> {
    let serialized =
        serde_json::to_string_pretty(&v).map_err(|e| Error::Serialization(e.to_string()))?;
    Ok(serialized)
}

/// Deserialize JSON format
///
/// # Errors
/// Error arises during conversion of type [`str`] to [`serde::de::Deserialize`]
/// using [`serde_json`]
#[allow(single_use_lifetimes)]
pub fn deserialize<V>(s: &str) -> Result<V, Error>
where
    V: for<'de> serde::de::Deserialize<'de>,
{
    let deserialized =
        serde_json::from_str(s).map_err(|e| Error::Deserialization(e.to_string()))?;
    Ok(deserialized)
}
