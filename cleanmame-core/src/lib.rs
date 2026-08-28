//! Shared library for CleanMAME metadata parsing and ROM folder operations.

pub mod errors;
pub mod metadata;
pub mod models;
pub mod operations;
pub mod parsers;
pub mod utils;

pub use errors::{CleanMameError, Result};
pub use models::{Flags, Genre, Region, RomEntry, RomMetadata};
