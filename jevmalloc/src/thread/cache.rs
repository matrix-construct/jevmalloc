//! Explicit thread-cache creation, use, flushing, and destruction.

mod destroy_error;

use core::{cell::Cell, marker::PhantomData, result};

use libc::{c_int, c_uint};

pub use self::destroy_error::ThreadCacheDestroyError;
use crate::{
	ctl::{Error, Result, key, raw},
	ffi,
};

/// Exclusive upper bound for explicit jemalloc tcache identifiers.
///
/// Jemalloc reserves twelve flag bits for tcache selection and two encoded
/// values. Valid identifiers are therefore in `0..4094`.
const TCACHE_INDEX_LIMIT: usize = 4094;

/// An owning handle to one explicitly managed jemalloc thread cache.
///
/// The handle destroys the cache when dropped. It can move between threads,
/// but it is not `Sync`: jemalloc permits an explicit cache to be used by only
/// one thread at a time. Every extended allocation using [`ThreadCache::flags`]
/// must remain serialized with other uses, flushing, and destruction.
#[must_use = "an explicit thread cache is destroyed when its handle is dropped"]
#[derive(Debug)]
pub struct ThreadCache {
	/// Identifier encoded into extended-allocation flags and control values.
	index: usize,

	/// Whether this handle still owns a live explicit cache.
	owned: bool,

	/// Allows ownership transfer between threads while preventing shared use.
	not_sync: PhantomData<Cell<()>>,
}

impl ThreadCache {
	/// Creates an explicitly managed thread cache.
	///
	/// The destroy and flush controls are resolved before creation, so a
	/// successful call always has the complete lifecycle interface available.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc cannot resolve the lifecycle controls,
	/// allocate the cache, or reports an invalid identifier.
	pub fn create() -> Result<Self> {
		let create = key::tcache_create()?;
		let _flush = key::tcache_flush()?;
		let _destroy = key::tcache_destroy()?;

		// SAFETY: `tcache.create` has no input and returns an `unsigned`
		// identifier. The returned owner records that cache's lifecycle.
		let index = unsafe { raw::get::<c_uint>(&create) }?;
		let index = usize::try_from(index).map_err(|_| Error::invalid_argument())?;
		validated_index(index)?;

		Ok(Self {
			index,
			owned: true,
			not_sync: PhantomData,
		})
	}

	/// Returns the extended-allocation flag selecting this cache.
	///
	/// The flag is valid only while this owner remains live. Calls using a copy
	/// of the flag must remain serialized with every other use of the cache,
	/// including [`ThreadCache::flush`] and [`ThreadCache::try_destroy`].
	#[must_use]
	#[inline(always)]
	pub const fn flags(&self) -> c_int { ffi::MALLOCX_TCACHE(self.index) }

	/// Flushes this explicit cache while preserving its identifier.
	///
	/// This invokes `tcache.flush` with this cache's integer identifier. It is
	/// distinct from [`crate::thread::this::flush`], which invokes the
	/// argument-free `thread.tcache.flush` command for the calling thread's
	/// automatic cache. Jemalloc recreates this explicit cache lazily on its
	/// next flagged allocation.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc rejects the command.
	pub fn flush(&mut self) -> Result {
		let key = key::tcache_flush()?;
		let index = validated_index(self.index)?;

		// SAFETY: this owner supplies the live cache's `unsigned` identifier and
		// the mutable borrow excludes another safe operation on the handle.
		unsafe { raw::set(&key, &index) }
	}

	/// Destroys this explicit cache and consumes its owner.
	///
	/// Success flushes the cache, recycles its identifier, and suppresses the
	/// fallback destructor. A failure returns the still-owning handle so the
	/// caller can retry or let its destructor make a best effort.
	///
	/// # Errors
	///
	/// Returns a recoverable error if jemalloc rejects the command.
	pub fn try_destroy(mut self) -> result::Result<(), ThreadCacheDestroyError> {
		match self.destroy() {
			| Ok(()) => Ok(()),
			| Err(error) => Err(ThreadCacheDestroyError::new(error, self)),
		}
	}

	/// Destroys the owned cache and disarms this handle on success.
	fn destroy(&mut self) -> Result {
		if !self.owned {
			return Err(Error::invalid_argument());
		}

		let key = key::tcache_destroy()?;
		let index = validated_index(self.index)?;

		// SAFETY: this handle exclusively owns the live explicit cache selected
		// by the exact `unsigned` identifier.
		unsafe { raw::set(&key, &index) }?;
		self.owned = false;

		Ok(())
	}
}

impl Drop for ThreadCache {
	fn drop(&mut self) {
		if self.owned {
			let _: Result = self.destroy();
		}
	}
}

/// Converts a validated explicit-cache index to jemalloc's C representation.
fn validated_index(index: usize) -> Result<c_uint> {
	if index >= TCACHE_INDEX_LIMIT {
		return Err(Error::invalid_argument());
	}

	c_uint::try_from(index).map_err(|_| Error::invalid_argument())
}

#[cfg(test)]
mod tests {
	//! Checks explicit-cache identifier validation without invoking controls.

	use super::*;

	/// Accepts exactly the identifier range representable by tcache flags.
	#[test]
	fn validates_index_range() {
		assert_eq!(validated_index(0).unwrap(), 0);
		assert_eq!(validated_index(TCACHE_INDEX_LIMIT - 1).unwrap(), 4093);
		assert!(
			validated_index(TCACHE_INDEX_LIMIT)
				.unwrap_err()
				.is(libc::EINVAL)
		);
		assert!(
			validated_index(usize::MAX)
				.unwrap_err()
				.is(libc::EINVAL)
		);
	}
}
