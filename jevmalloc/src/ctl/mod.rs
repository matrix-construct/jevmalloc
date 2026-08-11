//! MIB access primitives for jemalloc's control interface.
//!
//! [`Key`] stores a translated Management Information Base path, and
//! [`raw::mibs`] resolves an ad hoc control name. Typed controls elsewhere in
//! the crate cache their built-in keys process-wide, then operate through
//! `mallctlbymib`. The generic operations in [`raw`] are unsafe because a MIB
//! carries neither type information nor a command's semantic preconditions.

mod error;
#[expect(clippy::redundant_pub_crate)]
pub(crate) mod key;
pub mod raw;
#[expect(clippy::redundant_pub_crate)]
pub(crate) mod value;

pub use self::{
	error::Error,
	key::{KEY_SEGS, Key, NAME_MAX},
};

/// The result of a jemalloc control operation.
///
/// A failed operation retains the nonzero status returned by jemalloc. Invalid
/// names supplied to [`raw::mibs`] are reported as `EINVAL`.
pub type Result<T = ()> = core::result::Result<T, Error>;
