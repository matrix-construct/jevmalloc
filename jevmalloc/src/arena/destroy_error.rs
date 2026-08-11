//! Recoverable explicit arena destruction errors.

use core::{error, fmt};

use super::Arena;
use crate::ctl::Error;

/// An arena destruction failure that retains the consumed handle.
///
/// An owner remains owning when jemalloc rejects destruction. Recover it,
/// remove the reported obstruction, and retry. Dropping this error lets an
/// owning handle perform its best effort retry.
#[derive(Debug)]
pub struct ArenaDestroyError {
	/// Status returned by jemalloc.
	error: Error,

	/// Original arena handle passed to the destruction attempt.
	arena: Arena,
}

impl ArenaDestroyError {
	/// Constructs a recoverable failure from its two parts.
	pub(super) const fn new(error: Error, arena: Arena) -> Self { Self { error, arena } }

	/// Returns the jemalloc status without consuming the recovery handle.
	///
	/// The status is commonly `EFAULT` while a thread remains associated with
	/// the arena.
	#[must_use]
	pub const fn error(&self) -> Error { self.error }

	/// Returns the original arena handle.
	///
	/// An owner remains owning after a failed jemalloc command. A non-owning
	/// handle remains non-owning when the wrapper rejects the request.
	#[must_use = "the retained arena handle describes the recoverable lifecycle state"]
	pub const fn arena(&self) -> &Arena { &self.arena }

	/// Separates the status from the original arena handle.
	///
	/// When the handle still owns the arena, the caller can remove the
	/// obstruction and retry [`Arena::try_destroy`].
	#[must_use = "discarding the returned handle can trigger another destruction attempt"]
	pub fn into_parts(self) -> (Error, Arena) { (self.error, self.arena) }
}

impl error::Error for ArenaDestroyError {
	fn source(&self) -> Option<&(dyn error::Error + 'static)> { Some(&self.error) }
}

impl fmt::Display for ArenaDestroyError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "failed to destroy arena {}: {}", self.arena.index(), self.error)
	}
}
