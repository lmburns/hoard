//! Crypto backend using `age` for encryption.

pub mod context;
pub mod raw;

/// `age` configuration
pub struct Config {
    /// Input file to encrypt or decrypt
    input_file:      String,
    /// Recipient file to encrypt with (self-encryption)
    recipients_file: Vec<String>,
    /// Identity file used in decrypttion
    identity_file: Vec<String>,
}

impl Config {
    /// Create an `age` `Config` struct
    pub fn from(input_file: String, recipients_file: Vec<String>, identity_file: Vec<String>) -> Self {
        Self {
            input_file,
            recipients_file,
            identity_file
        }
    }
}
