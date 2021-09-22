#![allow(unused)]
//! Raw interface to `age`.

// use age::{cli_common::UiCallbacks, plugin, armor::ArmoredReader};
use age::{
    armor::{ArmoredReader, ArmoredWriter, Format},
    cli_common::{
        file_io, read_identities, read_or_generate_passphrase, read_secret, Passphrase, UiCallbacks,
    },
    plugin, Identity, IdentityFile, Recipient,
};

use super::Config;
use anyhow::Result;
use std::{
    convert::TryFrom,
    fs, io,
    io::{BufRead, BufReader, Read, Write},
    iter,
    path::PathBuf,
};
use thiserror::Error;
// use zeroize::Zeroize;

use crate::{Ciphertext, Plaintext, STORE_UMASK};

/// Encrypt a buffer with the `age` protocol, and return `Ciphertext`
pub fn encrypt(config: &Config, plaintext: Plaintext) -> Result<Ciphertext> {
    // recipient: Vec<String>,
    let ciphertext = {
        // recipient
        let encryptor = age::Encryptor::with_recipients(read_recipients(config.recipients_file)?);

        // let mut input = file_io::InputReader::new(Some(input_file))?;
        // input_file: String,

        let mut ciphertext = vec![];
        let mut writer = encryptor
            .wrap_output(ArmoredWriter::wrap_output(
                &mut ciphertext,
                Format::AsciiArmor,
            )?)
            .map_err(Err::Encrypt)?;

        writer.write_all(plaintext.unsecure_ref())?;
        writer.finish().and_then(|armor| armor.finish())?;

        ciphertext
    };
    // let encryptor = age::Encryptor::with_recipients(vec![Box::new(pubkey)]);
    // let mut writer = encryptor.wrap_output(&mut ciphertext)?;
    // writer.finish()?;

    Ok(Ciphertext::from(ciphertext))
}

/// Encrypt a buffer with the `age` protocol, and write it to a file
pub fn encrypt_to_file(config: &Config) -> Result<(), Err> {
    // recipient: Vec<String>,
    // recipient.is_empty()
    if config.recipients_file.is_empty() {
        return Err(Err::MissingRecipients);
    }

    // recipient
    let encryptor = age::Encryptor::with_recipients(read_recipients(config)?);
    let mut input = file_io::InputReader::new(Some(config.input_file))?;

    // let (format, output_format) =
    // (Format::AsciiArmor, file_io::OutputFormat::Text);
    // (Format::Binary, file_io::OutputFormat::Binary)

    let output = file_io::OutputWriter::new(
        Some(format!("{}.age", config.input_file)),
        file_io::OutputFormat::Text,
        0o666 - (0o666 & *STORE_UMASK),
    )?;

    let mut output = encryptor
        .wrap_output(ArmoredWriter::wrap_output(output, Format::AsciiArmor)?)
        .map_err(Err::Encrypt)?;

    let map_io_errors = |e: io::Error| match e.kind() {
        io::ErrorKind::BrokenPipe => Err::BrokenPipe(e),
        _ => e.into(),
    };

    io::copy(&mut input, &mut output).map_err(map_io_errors)?;
    output
        .finish()
        .and_then(|armor| armor.finish())
        .map_err(map_io_errors)?;

    Ok(())
}

/// Decrypt a `Ciphertext` buffer and return a `Plaintext` buffer
pub fn decrypt(key: age::ssh::Identity, ciphertext: Ciphertext) -> Result<Plaintext> {
    let plaintext = {
        let decryptor = match age::Decryptor::new(&ciphertext.unsecure_ref()[..])? {
            age::Decryptor::Recipients(d) => d,
            _ => unreachable!(),
        };

        let mut plaintext = vec![];
        let mut reader = decryptor.decrypt(iter::once(&key as &dyn age::Identity))?;
        reader.read_to_end(&mut plaintext);

        plaintext
    };
    Ok(Plaintext::from(plaintext))
}

pub fn decrypt_to_file(config: &Config) -> Result<(), Err> {
    match age::Decryptor::new(ArmoredReader::new(file_io::InputReader::new(Some(
        config.input_file,
    ))?))
    .map_err(Err::Decrypt)?
    {
        age::Decryptor::Recipients(decryptor) => {
            let identities = read_identities(
                config.identity_file,
                |e| Err::IdentityNotFound(e),
                |e, k| Err::UnsupportedKey(e, k),
            )?;

            if identities.is_empty() {
                return Err(Err::MissingIdentities);
            }

            // Get file stem, returning base by removing '.age' extension
            let output = PathBuf::from(config.input_file)
                .file_stem()
                .ok_or(Err::PathBufError("gaining file stem".to_string()))?
                .to_str()
                .ok_or(Err::PathBufError(
                    "converting file to string slice".to_string(),
                ))?
                .to_string();

            decryptor
                .decrypt(identities.iter().map(|i| i.as_ref() as &dyn Identity))
                .map_err(|e| e.into())
                .and_then(|input| write_output(input, Some(output)));
        },
        age::Decryptor::Passphrase(decryptor) => unreachable!(),
    }

    Ok(())
}

fn write_output<R: io::Read>(mut input: R, output: Option<String>) -> Result<(), Err> {
    let mut output = file_io::OutputWriter::new(
        output,
        file_io::OutputFormat::Unknown,
        0o666 - (0o666 & *STORE_UMASK),
    )?;
    io::copy(&mut input, &mut output)?;

    Ok(())
}

pub fn can_decrypt1(key: age::ssh::Identity, ciphertext: Ciphertext) -> Result<bool> {
    let decryptor = match age::Decryptor::new(&ciphertext.unsecure_ref()[..])? {
        age::Decryptor::Recipients(d) => d,
        _ => unreachable!(),
    };

    match decryptor.decrypt(iter::once(&key as &dyn age::Identity)) {
        Ok(_) => Ok(true),
        Err(age::DecryptError::NoMatchingKeys) => Ok(false),
        Err(_) => Ok(true),
    }
}

pub fn can_decrypt(config: &Config) -> Result<bool, Err> {
    let mut recipients: Vec<Box<dyn age::Recipient>> = vec![];

    for fname in config.recipients_file {
        let f = fs::File::open(&fname)?;
        let buf = BufReader::new(f);
        if let Err(e) = read_recipients_list(&fname, buf, &mut recipients) {
            return Err(e);
        }
    }

    Ok(true)
}

fn parse_recipient(s: String, recipients: &mut Vec<Box<dyn age::Recipient>>) -> Result<(), Err> {
    if let Ok(pk) = s.parse::<age::x25519::Recipient>() {
        recipients.push(Box::new(pk));
    } else if let Some(pk) = { s.parse::<age::ssh::Recipient>().ok().map(Box::new) } {
        recipients.push(pk);
    } else {
        return Err(Err::InvalidRecipient(s));
    }

    Ok(())
}

fn read_recipients_list<R: BufRead>(
    filename: &str,
    buf: R,
    recipients: &mut Vec<Box<dyn age::Recipient>>,
) -> Result<(), Err> {
    for (idx, line) in buf.lines().enumerate() {
        let line = line?;

        // Empty lines             comments, skip
        if line.is_empty() || line.find('#') == Some(0) {
            continue;
        } else if parse_recipient(line, recipients).is_err() {
            // Return a line number in place of the line
            return Err(Err::InvalidData(format!(
                "recipients file {} contains non-recipient data on line {}",
                filename,
                idx + 1
            )));
        }
    }

    Ok(())
}

pub fn read_recipients(config: &Config) -> Result<Vec<Box<dyn age::Recipient>>, Err> {
    let mut recipients: Vec<Box<dyn age::Recipient>> = vec![];

    // recipient_strings: Vec<String>,
    // for arg in recipient_strings {
    //     parse_recipient(arg, &mut recipients)?;
    // }

    // read_recipients_list(&fname, buf, &mut recipients)?;
    for fname in config.recipients_file {
        let f = fs::File::open(&fname)?;
        let buf = BufReader::new(f);
        for (idx, line) in buf.lines().enumerate() {
            let line = line?;
            if line.is_empty() || line.find('#') == Some(0) {
                continue;
            } else if parse_recipient(line, &mut recipients).is_err() {
                return Err(Err::InvalidData(format!(
                    "recipients file {} contains non-recipient data on line {}",
                    fname,
                    idx + 1
                )));
            }
        }
    }

    Ok(recipients)
}

pub fn public_keys() {
    todo!();
}

pub fn private_keys() {
    todo!();
}

pub fn import_key() {
    todo!();
}

pub fn export_key() {
    todo!();
}

pub fn supports_proto() {
    todo!();
}

/// Age binary error.
#[derive(Debug, Error)]
pub enum Err {
    /// `age` encryption failure
    #[error("failed to encrypt plaintext")]
    Encrypt(#[source] age::EncryptError),

    /// Invalid recipients for `age` file
    #[error("invalid recipient {0} for age encryption")]
    InvalidRecipient(String),

    /// No recipients for `age` file
    #[error("missing recipients for encryption of age file")]
    MissingRecipients,

    /// No identity for `age` file
    #[error("missing identities for decryption of age file")]
    MissingIdentities,

    /// `age` identity doesn't have a passphrase
    #[error("{0} is encrypted without a passphrase")]
    IdentityEncryptedWithoutPassphrase(String),

    /// `age` identity not found
    #[error("age identity not found: {0}")]
    IdentityNotFound(String),

    /// Unsupported key for `age` encryption
    #[error("{0} encryption has an unsupported key: {1:?}")]
    UnsupportedKey(String, age::ssh::UnsupportedKey),

    /// Failure to decrypt `age` file
    #[error("failed to decrypt ciphertext")]
    Decrypt(#[source] age::DecryptError),

    /// Invalid data when reading age file
    #[error("invalid data found within age file: {0}")]
    InvalidData(String),

    /// General IO error
    #[error("IO error: {0}")]
    IO(#[from] io::Error),

    /// General age decrypt error
    #[error("age decrypt error: {0}")]
    RecipientDecrypt(#[from] age::DecryptError),

    /// Broken pipe when writing output of encrypted age file
    #[error("broken pipe when writing output: {0}")]
    BrokenPipe(#[source] io::Error),

    /// Error when dealing with `PathBuf` conversions
    #[error("pathbuf manipulation error: {0}")]
    PathBufError(String),
}
