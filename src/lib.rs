#![no_std]

pub mod obis;

mod read;

pub use read::*;

#[derive(Debug)]
pub enum Error {
    InvalidFormat,
    Checksum,
    UnrecognizedReference,
}

pub(crate) type Result<T, E = Error> = core::result::Result<T, E>;
