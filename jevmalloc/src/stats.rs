//! Statistics reporting, snapshots, and reset controls.
//!
//! Global statistics are cached by jemalloc. Call [`refresh_epoch`] before
//! reading a related set when a fresh, internally comparable snapshot is
//! required. Getters do not refresh implicitly. `stats.zero_reallocs` is the
//! sole global value in this interface that jemalloc reads directly rather
//! than copying into the epoch snapshot.

use core::ffi::{CStr, c_char, c_void};

use crate::{
	ctl::{Result, key, raw},
	ffi,
};

/// Returns the current jemalloc statistics epoch without refreshing it.
///
/// The value can be compared across observations to detect a refresh performed
/// by another thread.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
pub fn epoch() -> Result<u64> {
	let key = key::epoch()?;

	// SAFETY: `epoch` has the C output type `uint64_t`.
	unsafe { raw::get(&key) }
}

/// Refreshes cached allocator statistics and returns the new epoch.
///
/// Jemalloc ignores the numeric input value. Supplying any `uint64_t` refreshes
/// all cached statistics, increments the allocator-maintained epoch once, and
/// returns that new value. Ordinary controls do not call this implicitly.
///
/// # Errors
///
/// Returns an error if jemalloc cannot refresh or return the epoch.
pub fn refresh_epoch() -> Result<u64> {
	let key = key::epoch()?;

	// SAFETY: `epoch` has the C type `uint64_t` for input and output.
	unsafe { raw::xchg(&key, &0_u64) }
}

/// Prints an allocator statistics report through the supplied writer.
///
/// Jemalloc invokes `write` synchronously with arbitrary byte fragments and
/// attempts to refresh its cached statistics before printing. If that refresh
/// cannot allocate, jemalloc reports the failure through its global message
/// callback and can invoke `write` zero times; the C interface provides no
/// error status. Fragments can contain invalid UTF-8 or split a multibyte
/// encoding at a buffering boundary. This Rust adapter performs no allocation,
/// though jemalloc can allocate an internal print buffer. For bundled builds,
/// the `stats` feature enables detailed report sections. A panic in `write`
/// aborts the process at the C callback boundary. Unknown option bytes are
/// ignored.
pub fn print<F>(opts: &CStr, mut write: F)
where
	F: FnMut(&[u8]),
{
	let opaque = (&raw mut write).cast::<c_void>();

	// SAFETY: `write_fragment` receives the live callback pointer above only
	// during this synchronous call, and `opts` is NUL-terminated.
	unsafe { ffi::malloc_stats_print(Some(write_fragment::<F>), opaque, opts.as_ptr()) };
}

/// Forwards one C writer fragment to the live Rust callback.
///
/// [`print`] supplies both pointers and keeps the callback uniquely borrowed
/// until jemalloc returns.
unsafe extern "C" fn write_fragment<F>(opaque: *mut c_void, fragment: *const c_char)
where
	F: FnMut(&[u8]),
{
	// SAFETY: `print` passes a live, uniquely borrowed `F` for the duration of
	// the synchronous call.
	let write = unsafe { &mut *opaque.cast::<F>() };

	// SAFETY: jemalloc supplies a non-null, NUL-terminated fragment that remains
	// live for this callback invocation.
	let fragment = unsafe { CStr::from_ptr(fragment) };

	write(fragment.to_bytes());
}

/// Returns the number of bytes currently allocated by the application.
///
/// The value belongs to jemalloc's current cached statistics snapshot.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
#[cfg(feature = "stats")]
pub fn allocated() -> Result<usize> {
	let key = key::stats_allocated()?;

	// SAFETY: `stats.allocated` has the C output type `size_t`.
	unsafe { raw::get(&key) }
}

/// Returns the number of bytes in active application allocation pages.
///
/// The value is a multiple of the page size and is at least [`allocated`]. It
/// excludes dirty pages, muzzy pages, and pages used only for metadata.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
#[cfg(feature = "stats")]
pub fn active() -> Result<usize> {
	let key = key::stats_active()?;

	// SAFETY: `stats.active` has the C output type `size_t`.
	unsafe { raw::get(&key) }
}

/// Returns the number of bytes dedicated to allocator metadata.
///
/// This includes bootstrap-sensitive base allocations and internal
/// allocations. See [`metadata_thp`] for the separate transparent huge-page
/// count.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
#[cfg(feature = "stats")]
pub fn metadata() -> Result<usize> {
	let key = key::stats_metadata()?;

	// SAFETY: `stats.metadata` has the C output type `size_t`.
	unsafe { raw::get(&key) }
}

/// Returns the number of transparent huge pages used for allocator metadata.
///
/// This value counts huge pages, not bytes.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
#[cfg(feature = "stats")]
pub fn metadata_thp() -> Result<usize> {
	let key = key::stats_metadata_thp()?;

	// SAFETY: `stats.metadata_thp` has the C output type `size_t`.
	unsafe { raw::get(&key) }
}

/// Returns jemalloc's upper bound on physically resident mapped bytes.
///
/// The value includes allocator metadata, active allocation backing, and
/// unused dirty pages. Demand-zero pages can make it larger than the amount
/// actually resident in physical memory.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
#[cfg(feature = "stats")]
pub fn resident() -> Result<usize> {
	let key = key::stats_resident()?;

	// SAFETY: `stats.resident` has the C output type `size_t`.
	unsafe { raw::get(&key) }
}

/// Returns the number of bytes in active extents mapped by jemalloc.
///
/// Inactive extents are excluded, including those containing dirty pages, so
/// this value has no strict ordering relative to [`resident`].
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
#[cfg(feature = "stats")]
pub fn mapped() -> Result<usize> {
	let key = key::stats_mapped()?;

	// SAFETY: `stats.mapped` has the C output type `size_t`.
	unsafe { raw::get(&key) }
}

/// Returns the bytes in mappings retained for later reuse.
///
/// Retained virtual memory is excluded from [`mapped`] and usually has no
/// strongly associated physical memory.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
#[cfg(feature = "stats")]
pub fn retained() -> Result<usize> {
	let key = key::stats_retained()?;

	// SAFETY: `stats.retained` has the C output type `size_t`.
	unsafe { raw::get(&key) }
}

/// Returns the number of `realloc` calls with a non-null pointer and zero size.
///
/// Jemalloc reads this counter directly rather than from the cached epoch
/// snapshot. Portable programs should avoid this fundamentally unsafe
/// reallocation pattern.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
#[cfg(feature = "stats")]
pub fn zero_reallocs() -> Result<usize> {
	let key = key::stats_zero_reallocs()?;

	// SAFETY: `stats.zero_reallocs` has the C output type `size_t`.
	unsafe { raw::get(&key) }
}

/// Returns the number of background worker threads currently running.
///
/// Platforms without background-thread support report zero.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
#[cfg(feature = "stats")]
pub fn background_thread_num_threads() -> Result<usize> {
	let key = key::stats_background_thread_num_threads()?;

	// SAFETY: `stats.background_thread.num_threads` has the C output type
	// `size_t`.
	unsafe { raw::get(&key) }
}

/// Returns the cumulative runs performed by all background workers.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
#[cfg(feature = "stats")]
pub fn background_thread_num_runs() -> Result<u64> {
	let key = key::stats_background_thread_num_runs()?;

	// SAFETY: `stats.background_thread.num_runs` has the C output type
	// `uint64_t`.
	unsafe { raw::get(&key) }
}

/// Returns the average background-worker run interval in nanoseconds.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
#[cfg(feature = "stats")]
pub fn background_thread_run_interval() -> Result<u64> {
	let key = key::stats_background_thread_run_interval()?;

	// SAFETY: `stats.background_thread.run_interval` has the C output type
	// `uint64_t`.
	unsafe { raw::get(&key) }
}

/// Resets jemalloc's global, arena, and bin mutex statistics.
///
/// The reset visits mutexes individually, so concurrent activity is not
/// observed at one process-wide instant.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the reset command.
#[cfg(feature = "stats")]
pub fn stats_reset() -> Result {
	let key = key::stats_mutexes_reset()?;

	// SAFETY: this MIB selects only the mutex-statistics reset command.
	unsafe { raw::notify(&key) }
}

#[cfg(test)]
mod tests {
	//! Checks the byte-preserving callback boundary.

	extern crate std as rust_std;

	use rust_std::vec::Vec;

	use super::*;

	/// Invokes the raw trampoline with one validated C fragment.
	fn forward<F>(write: &mut F, fragment: &[u8])
	where
		F: FnMut(&[u8]),
	{
		let fragment = CStr::from_bytes_with_nul(fragment).unwrap();
		let opaque = (&raw mut *write).cast::<c_void>();

		// SAFETY: `opaque` points to the live callback above, and `fragment` is
		// NUL-terminated for this invocation.
		unsafe { write_fragment::<F>(opaque, fragment.as_ptr()) };
	}

	/// Preserves invalid bytes and split encodings without interpreting them.
	#[test]
	fn writer_forwards_exact_bytes() {
		let mut output = Vec::new();
		{
			let mut write = |fragment: &[u8]| output.extend_from_slice(fragment);
			let fragments = [[0xFF_u8, 0], [0xC3, 0], [0xA9, 0]];

			for fragment in &fragments {
				forward(&mut write, fragment);
			}
		}

		assert_eq!(output, [0xFF, 0xC3, 0xA9]);
	}
}
