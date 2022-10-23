//! No-std parser for the Swedish-Norwegian smart power meter customer interface
//! called HAN or H1 (the latter was already registered on crates.io).

#![warn(missing_docs)]
#![no_std]

mod obis;
mod read;

use core::fmt::Display;

pub use obis::*;
pub use read::*;

/// HAN error.
#[derive(Debug)]
pub enum Error {
    /// Parsing failed due to an invalid format.
    InvalidFormat,
    /// Checksum mismatch.
    Checksum,
    /// The parser came across a correctly formatted, but unrecognized,
    /// [`Obis`] reference.
    UnrecognizedReference,
}

impl Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let msg = match self {
            Error::InvalidFormat => "invalid format",
            Error::Checksum => "checksum mismatch",
            Error::UnrecognizedReference => "unrecognized obis reference",
        };

        f.write_str(msg)
    }
}

pub(crate) type Result<T, E = Error> = core::result::Result<T, E>;
