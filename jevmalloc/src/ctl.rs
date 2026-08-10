//! Typed control and introspection for jemalloc.
//!
//! Jemalloc exposes allocator settings, maintenance commands, and statistics
//! through its `mallctl` namespace. Typed [`Name`] and [`Mib`] accessors sit
//! beside the lower-level [`raw`] interface and their shared error type.

mod error;
mod keys;
pub mod raw;

pub use self::{
	error::Error,
	keys::{Access, AsName, Mib, MibStr, Name},
};
use crate::std::result;

/// The result of a jemalloc control operation.
///
/// A failed operation retains the nonzero status returned by jemalloc.
pub type Result<T> = result::Result<T, Error>;
