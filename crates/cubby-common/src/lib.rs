//! Types and helpers shared by every Cubby crate. It depends on no other workspace crate.

pub mod contracts;
pub mod error;
pub mod events;
pub mod net;
pub mod packs;
pub mod paths;
pub mod version;

pub use error::{Error, Result};
