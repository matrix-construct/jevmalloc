//! Direct access to the calling thread's allocation counters.

#![cfg(feature = "stats")]

use core::{marker::PhantomData, ptr::NonNull};

use crate::ctl::{Error, Result, key, raw};

/// Direct access to the calling thread's cumulative byte counters.
///
/// Construction performs two MIB calls. Subsequent observations are raw loads
/// from jemalloc's stable thread-specific counter pointers. The handle is
/// neither `Send` nor `Sync`, so safe Rust cannot move it away from the thread
/// whose allocator state it observes.
#[derive(Clone, Copy, Debug)]
pub struct ThreadCounters {
	/// Jemalloc's current-thread allocated-byte counter.
	allocated: NonNull<u64>,

	/// Jemalloc's current-thread deallocated-byte counter.
	deallocated: NonNull<u64>,

	/// Prevents the thread-specific pointers from crossing thread boundaries.
	not_send_or_sync: PhantomData<*mut ()>,
}

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
