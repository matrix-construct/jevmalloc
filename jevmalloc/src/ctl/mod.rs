//! `jemalloc` control and introspection.
//!
//! `jemalloc` offers a powerful introspection and control interface through the
//! `mallctl` function. It can be used to tune the allocator, take heap dumps,
//! and retrieve statistics. This module wraps it as the raw typed accessors in
//! [`raw`], the [`Name`]/[`Mib`] key indices, and the error type.

mod error;
mod keys;
pub mod raw;

pub use self::{
	error::Error,
	keys::{Access, AsName, Mib, MibStr, Name},
};
use crate::std::result;

/// Result of a `mallctl` operation.
///
/// The error wraps the non-zero `errno` value the call returned.
pub type Result<T> = result::Result<T, Error>;
