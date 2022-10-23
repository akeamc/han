#![no_std]

mod obis;
mod read;
pub mod state;

pub use obis::*;
pub use read::*;

#[derive(Debug)]
pub enum Error {
    InvalidFormat,
    Checksum,
    UnrecognizedReference,
}

pub(crate) type Result<T, E = Error> = core::result::Result<T, E>;
