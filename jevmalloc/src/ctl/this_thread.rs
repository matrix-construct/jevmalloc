//! Controls associated with the calling thread.
//!
//! Arena operations first read `thread.arena`, then substitute that identifier
//! into an `arena.0.*` MIB template. The literal zero is only a placeholder;
//! these functions do not accidentally operate on arena zero.

#[cfg(feature = "stats")]
use core::{marker::PhantomData, ptr::NonNull};

use libc::c_uint;

use super::{Arena, Error, Result, arena, key, raw, value};

/// Reclaims unused pages from the calling thread's arena.
///
/// The operation applies time-based decay before purging all remaining unused
/// dirty pages.
///
/// # Errors
///
/// Returns an error if the thread arena cannot be read or either reclamation
/// command fails.
pub fn trim() -> Result {
	let arena = Arena::current()?;

	arena.trim()
}

/// Purges all unused dirty pages from the calling thread's arena.
///
/// # Errors
///
/// Returns an error if the thread arena cannot be read or jemalloc rejects the
/// purge command.
pub fn purge() -> Result { Arena::current()?.purge() }

/// Applies decay-based purging to the calling thread's arena.
///
/// # Errors
///
/// Returns an error if the thread arena cannot be read or jemalloc rejects the
/// decay command.
pub fn decay() -> Result { Arena::current()?.decay() }

/// Notifies jemalloc that the calling thread is entering an extended idle
/// period.
///
/// Jemalloc flushes the thread cache when one exists and can also decay the
/// arena under allocator-specific conditions. It does not guarantee a purge.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the idle notification.
pub fn idle() -> Result {
	let key = key::thread_idle()?;

	// SAFETY: this MIB selects only the current-thread idle notification.
	unsafe { raw::notify(&key) }
}

/// Flushes the calling thread's automatic allocation cache.
///
/// Cached objects and the cache's internal structures are returned to the
/// thread's arena.
///
/// # Errors
///
/// Returns an error if the thread has no cache or jemalloc rejects the command.
pub fn flush() -> Result {
	let key = key::thread_tcache_flush()?;

	// SAFETY: this MIB selects only the current-thread cache flush command.
	unsafe { raw::notify(&key) }
}

/// Sets the calling thread arena's muzzy-page decay interval.
///
/// Every write marks all unused muzzy pages fully decayed and immediately
/// purges them unless the value is `-1`. The previous interval is returned. A
/// value of `0` otherwise requests immediate decay, while `-1` disables it.
///
/// # Errors
///
/// Returns an error if the thread arena cannot be read or jemalloc rejects the
/// value.
pub fn set_muzzy_decay(decay_ms: isize) -> Result<isize> {
	Arena::current()?.set_muzzy_decay(decay_ms)
}

/// Returns the calling thread arena's muzzy-page decay interval.
///
/// # Errors
///
/// Returns an error if the thread arena cannot be read or jemalloc rejects the
/// query.
pub fn get_muzzy_decay() -> Result<isize> { Arena::current()?.muzzy_decay() }

/// Sets the calling thread arena's dirty-page decay interval.
///
/// Every write marks all unused dirty pages fully decayed and immediately
/// purges them unless the value is `-1`. The previous interval is returned. A
/// value of `0` otherwise requests immediate decay, while `-1` disables it.
///
/// # Errors
///
/// Returns an error if the thread arena cannot be read or jemalloc rejects the
/// value.
pub fn set_dirty_decay(decay_ms: isize) -> Result<isize> {
	Arena::current()?.set_dirty_decay(decay_ms)
}

/// Returns the calling thread arena's dirty-page decay interval.
///
/// # Errors
///
/// Returns an error if the thread arena cannot be read or jemalloc rejects the
/// query.
pub fn get_dirty_decay() -> Result<isize> { Arena::current()?.dirty_decay() }

/// Enables or disables the calling thread's automatic allocation cache.
///
/// Disabling implicitly flushes the cache. The previous setting is returned.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the update.
pub fn cache_enable(enable: bool) -> Result<bool> {
	let key = key::thread_tcache_enabled()?;
	value::xchg_bool(&key, enable)
}

/// Returns whether the calling thread's automatic allocation cache is enabled.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
pub fn is_cache_enabled() -> Result<bool> {
	let key = key::thread_tcache_enabled()?;
	value::get_bool(&key)
}

/// Associates the calling thread with a jemalloc arena.
///
/// A valid arena that has not been initialized is created as a side effect. The
/// previous identifier is returned.
///
/// # Errors
///
/// Returns an error if `id` does not fit jemalloc's unsigned arena type or if
/// jemalloc rejects the reassignment.
///
/// # Safety
///
/// Every allocation made through the association must be dead before the arena
/// is reset or destroyed. The thread must first move elsewhere and flush every
/// automatic or explicit tcache that touched the arena. All handles and
/// operations involving either association must remain synchronized with
/// reset, destruction, and index recycling.
pub unsafe fn set_arena(id: usize) -> Result<usize> {
	let id = arena::validated_index(id)?;
	let key = key::thread_arena()?;

	// SAFETY: `thread.arena` has C type `unsigned` for input and output.
	let previous = unsafe { raw::xchg(&key, &id) }?;
	previous
		.try_into()
		.map_err(|_| Error::invalid_argument())
}

/// Returns the jemalloc arena associated with the calling thread.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query or the identifier does not
/// fit in `usize`.
pub fn arena_id() -> Result<usize> {
	let key = key::thread_arena()?;

	// SAFETY: `thread.arena` has the C output type `unsigned`.
	let id = unsafe { raw::get::<c_uint>(&key) }?;
	id.try_into()
		.map_err(|_| Error::invalid_argument())
}

/// Enables or disables allocation sampling for the calling thread.
///
/// Global profiling must also be active before this thread produces samples.
/// The previous setting is returned.
///
/// # Errors
///
/// Returns an error if runtime profiling is unavailable or jemalloc rejects the
/// update.
#[cfg(feature = "profiling")]
pub fn prof_enable(enable: bool) -> Result<bool> {
	let key = key::thread_prof_active()?;
	value::xchg_bool(&key, enable)
}

/// Returns whether allocation sampling is active for the calling thread.
///
/// Global profiling can independently suppress sampling.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
#[cfg(feature = "profiling")]
pub fn is_prof_enabled() -> Result<bool> {
	let key = key::thread_prof_active()?;
	value::get_bool(&key)
}

/// Resets the calling thread's approximate peak net-allocation counter.
///
/// Cumulative allocated and deallocated byte counters are not reset.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the reset command.
#[cfg(feature = "stats")]
pub fn reset_peak() -> Result {
	let key = key::thread_peak_reset()?;

	// SAFETY: this MIB selects only the current-thread peak reset command.
	unsafe { raw::notify(&key) }
}

/// Returns the calling thread's approximate peak net allocation in bytes.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
#[cfg(feature = "stats")]
pub fn peak() -> Result<u64> {
	let key = key::thread_peak_read()?;

	// SAFETY: `thread.peak.read` has the C output type `uint64_t`.
	unsafe { raw::get(&key) }
}

/// Returns the total number of bytes allocated by the calling thread.
///
/// The counter can wrap. Use [`ThreadCounters`] when sampling repeatedly to
/// avoid one control call per observation.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
#[cfg(feature = "stats")]
pub fn allocated() -> Result<u64> {
	let key = key::thread_allocated()?;

	// SAFETY: `thread.allocated` has the C output type `uint64_t`.
	unsafe { raw::get(&key) }
}

/// Returns the total number of bytes deallocated by the calling thread.
///
/// The counter can wrap. Use [`ThreadCounters`] when sampling repeatedly to
/// avoid one control call per observation.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
#[cfg(feature = "stats")]
pub fn deallocated() -> Result<u64> {
	let key = key::thread_deallocated()?;

	// SAFETY: `thread.deallocated` has the C output type `uint64_t`.
	unsafe { raw::get(&key) }
}

/// Direct access to the calling thread's cumulative byte counters.
///
/// Construction performs two MIB calls. Subsequent observations are raw loads
/// from jemalloc's stable thread-specific counter pointers. The handle is
/// neither `Send` nor `Sync`, so safe Rust cannot move it away from the thread
/// whose allocator state it observes.
#[cfg(feature = "stats")]
#[derive(Clone, Copy, Debug)]
pub struct ThreadCounters {
	/// Jemalloc's current-thread allocated-byte counter.
	allocated: NonNull<u64>,

	/// Jemalloc's current-thread deallocated-byte counter.
	deallocated: NonNull<u64>,

	/// Prevents the thread-specific pointers from crossing thread boundaries.
	not_send_or_sync: PhantomData<*mut ()>,
}

#[cfg(feature = "stats")]
impl ThreadCounters {
	/// Obtains direct counter pointers for the calling thread.
	///
	/// # Errors
	///
	/// Returns an error if jemalloc rejects either query or returns a null
	/// pointer.
	pub fn current() -> Result<Self> {
		let allocated_key = key::thread_allocatedp()?;
		let deallocated_key = key::thread_deallocatedp()?;

		// SAFETY: the two controls return pointers to their `uint64_t` counters.
		let allocated = unsafe { raw::get::<*mut u64>(&allocated_key) }?;

		// SAFETY: this is the distinct deallocated-counter MIB and pointer type.
		let deallocated = unsafe { raw::get::<*mut u64>(&deallocated_key) }?;

		Ok(Self {
			allocated: NonNull::new(allocated).ok_or_else(Error::bad_address)?,
			deallocated: NonNull::new(deallocated).ok_or_else(Error::bad_address)?,
			not_send_or_sync: PhantomData,
		})
	}

	/// Loads the total number of bytes allocated by this thread.
	///
	/// The counter can wrap.
	#[inline]
	#[must_use]
	pub fn allocated(&self) -> u64 {
		// SAFETY: construction validated the current-thread counter pointer, and
		// the handle cannot cross threads.
		unsafe { self.allocated.as_ptr().read() }
	}

	/// Loads the total number of bytes deallocated by this thread.
	///
	/// The counter can wrap.
	#[inline]
	#[must_use]
	pub fn deallocated(&self) -> u64 {
		// SAFETY: construction validated the current-thread counter pointer, and
		// the handle cannot cross threads.
		unsafe { self.deallocated.as_ptr().read() }
	}
}
