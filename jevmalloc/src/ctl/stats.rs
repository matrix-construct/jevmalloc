//! Statistics snapshot and reset controls.

use super::{Result, key, raw};

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
