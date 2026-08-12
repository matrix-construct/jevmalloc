//! Recoverable explicit thread-cache destruction errors.

use core::{error, fmt};

use super::ThreadCache;
use crate::ctl::Error;

/// A thread-cache destruction failure that retains the consumed owner.
///
/// Recover the cache, then retry destruction or let its destructor make a best
/// effort.
#[derive(Debug)]
pub struct ThreadCacheDestroyError {
	/// Status returned by jemalloc.
	error: Error,

	/// Original cache handle passed to the destruction attempt.
	cache: ThreadCache,
}

impl ThreadCacheDestroyError {
	/// Constructs a recoverable failure from its two parts.
	pub(super) const fn new(error: Error, cache: ThreadCache) -> Self { Self { error, cache } }

	/// Returns the jemalloc status without consuming the recovery handle.
	#[must_use]
	pub const fn error(&self) -> Error { self.error }

	/// Returns the original thread-cache handle.
	#[must_use = "the retained thread-cache handle owns the recoverable lifecycle state"]
	pub const fn cache(&self) -> &ThreadCache { &self.cache }

	/// Separates the status from the original thread-cache handle.
	///
	/// The caller can retry [`ThreadCache::try_destroy`] or let the returned
	/// handle perform its best effort destruction.
	#[must_use = "discarding the returned handle triggers another destruction attempt"]
	pub fn into_parts(self) -> (Error, ThreadCache) { (self.error, self.cache) }
}

impl error::Error for ThreadCacheDestroyError {
	fn source(&self) -> Option<&(dyn error::Error + 'static)> { Some(&self.error) }
}

impl fmt::Display for ThreadCacheDestroyError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(
			f,
			"failed to destroy explicit thread cache {}: {}",
			self.cache.index, self.error
		)
	}
}
