//! Heap profiling controls.
//!
//! The Cargo `profiling` feature compiles jemalloc's profiling support, but
//! runtime profiling remains disabled unless allocator startup configuration
//! enables `prof`. Controls that activate, reset, or dump profiling can return
//! `ENOENT` while that runtime option is off.

use super::{Result, key, raw, value};

/// Resets accumulated heap-profile statistics without changing the sample rate.
///
/// The optional `prof.reset` input is omitted, which preserves the current
/// sampling exponent.
///
/// # Errors
///
/// Returns an error if runtime profiling is unavailable or the reset fails.
pub fn prof_reset() -> Result {
	let key = key::prof_reset()?;

	// SAFETY: this MIB selects `prof.reset` with its optional input omitted.
	unsafe { raw::notify(&key) }
}

/// Writes a heap profile to jemalloc's generated default path.
///
/// The optional filename input is omitted. Jemalloc derives the path from its
/// configured prefix, process identifier, and dump sequence.
///
/// # Errors
///
/// Returns an error if runtime profiling is unavailable or the dump fails.
pub fn prof_dump() -> Result {
	let key = key::prof_dump()?;

	// SAFETY: this MIB selects `prof.dump` with its optional filename omitted.
	unsafe { raw::notify(&key) }
}

/// Enables or disables dumps at virtual-memory high-water marks.
///
/// The previous setting is returned.
///
/// # Errors
///
/// Returns an error if runtime profiling is unavailable or jemalloc rejects the
/// update.
pub fn prof_gdump(enable: bool) -> Result<bool> {
	let key = key::prof_gdump()?;
	value::xchg_bool(&key, enable)
}

/// Enables or disables global allocation sampling.
///
/// Thread-level profiling must also be active before a thread produces samples.
/// The previous setting is returned.
///
/// # Errors
///
/// Returns an error if runtime profiling is unavailable or jemalloc rejects the
/// update.
pub fn prof_enable(enable: bool) -> Result<bool> {
	let key = key::prof_active()?;
	value::xchg_bool(&key, enable)
}

/// Returns whether global allocation sampling is active.
///
/// Thread-level profiling can independently suppress sampling for one thread.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
pub fn is_prof_enabled() -> Result<bool> {
	let key = key::prof_active()?;
	value::get_bool(&key)
}

/// Returns the average allocation interval between periodic profile dumps.
///
/// The interval is measured in allocated bytes. A zero value means interval
/// dumps are disabled.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
pub fn prof_interval() -> Result<u64> {
	let key = key::prof_interval()?;

	// SAFETY: `prof.interval` has the C output type `uint64_t`.
	unsafe { raw::get(&key) }
}
