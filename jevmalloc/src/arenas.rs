//! Allocator-wide arena defaults, queries, and reclamation controls.

use core::ffi::CStr;

use libc::{c_char, c_uint};

use crate::{
	arena::Dss,
	ctl::{Error, Key, Result, key, raw},
};

/// Jemalloc's documented MIB sentinel for all arenas.
const ALL_ARENAS: usize = 4096;

/// Reclaims unused pages from every initialized arena.
///
/// The operation applies time-based decay before purging all remaining unused
/// dirty pages.
///
/// # Errors
///
/// Returns an error if jemalloc rejects either reclamation command.
pub fn trim() -> Result {
	decay()?;
	purge()
}

/// Purges all unused dirty pages from every initialized arena.
///
/// This selects jemalloc's documented all-arenas MIB sentinel.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the purge command.
pub fn purge() -> Result { notify(key::arena_purge()?) }

/// Applies time-based dirty and muzzy page decay to every initialized arena.
///
/// Each arena uses its own current decay intervals.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the decay command.
pub fn decay() -> Result { notify(key::arena_decay()?) }

/// Returns the default muzzy-page decay interval for future arenas.
///
/// Existing arenas retain their own current settings.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
pub fn muzzy_decay() -> Result<isize> {
	let key = key::arenas_muzzy_decay()?;

	// SAFETY: `arenas.muzzy_decay_ms` has C output type `ssize_t`, represented
	// by `isize` on supported targets.
	unsafe { raw::get(&key) }
}

/// Sets the default muzzy-page decay interval for future arenas.
///
/// The previous default is returned. Existing arenas are not changed.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the interval.
pub fn set_muzzy_decay(decay_ms: isize) -> Result<isize> {
	let key = key::arenas_muzzy_decay()?;

	// SAFETY: `arenas.muzzy_decay_ms` uses `ssize_t`, represented by `isize`,
	// for input and output on supported targets.
	unsafe { raw::xchg(&key, &decay_ms) }
}

/// Returns the default dirty-page decay interval for future arenas.
///
/// Existing arenas retain their own current settings.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
pub fn dirty_decay() -> Result<isize> {
	let key = key::arenas_dirty_decay()?;

	// SAFETY: `arenas.dirty_decay_ms` has C output type `ssize_t`, represented
	// by `isize` on supported targets.
	unsafe { raw::get(&key) }
}

/// Sets the default dirty-page decay interval for future arenas.
///
/// The previous default is returned. Existing arenas are not changed.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the interval.
pub fn set_dirty_decay(decay_ms: isize) -> Result<isize> {
	let key = key::arenas_dirty_decay()?;

	// SAFETY: `arenas.dirty_decay_ms` uses `ssize_t`, represented by `isize`,
	// for input and output on supported targets.
	unsafe { raw::xchg(&key, &decay_ms) }
}

/// Returns the default dynamic storage allocation precedence for future
/// arenas.
///
/// Existing arenas retain their own current settings.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query or reports an unknown value.
pub fn dss() -> Result<Dss> {
	let key = select(key::arena_dss()?);

	// SAFETY: `arena.4096.dss` returns a process-lifetime `const char *`.
	let dss = unsafe { raw::get::<*const c_char>(&key) }?;

	// SAFETY: jemalloc returns a readable static precedence string.
	unsafe { Dss::from_ptr(dss) }
}

/// Sets the default dynamic storage allocation precedence for future arenas.
///
/// The resulting default is returned. Existing arenas are not changed.
/// Platforms without `sbrk(2)` reject settings other than [`Dss::Disabled`].
///
/// # Errors
///
/// Returns an error if jemalloc rejects the requested setting.
pub fn set_dss(dss: Dss) -> Result<Dss> {
	let key = select(key::arena_dss()?);
	let dss = dss.as_ptr();

	// SAFETY: `arena.4096.dss` accepts and returns static `const char *`
	// precedence strings. Jemalloc retains neither input pointer.
	let resulting = unsafe { raw::xchg(&key, &dss) }?;

	// SAFETY: jemalloc returns a readable static precedence string.
	unsafe { Dss::from_ptr(resulting) }
}

/// Returns jemalloc's current arena index high-water mark.
///
/// The value includes automatically managed and explicitly created indices,
/// including holes left by destroyed arenas. It is not a count of live arenas.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query or the value cannot fit in a
/// Rust `usize`.
pub fn limit() -> Result<usize> {
	let key = key::arenas_limit()?;

	// SAFETY: `arenas.narenas` has the C output type `unsigned`.
	let arenas = unsafe { raw::get::<c_uint>(&key) }?;

	arenas
		.try_into()
		.map_err(|_| Error::invalid_argument())
}

/// Returns jemalloc's configured allocation quantum in bytes.
///
/// This value reflects the target and any build-time quantum override.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query.
pub fn quantum() -> Result<usize> {
	let key = key::arenas_quantum()?;

	// SAFETY: `arenas.quantum` has the C output type `size_t`.
	unsafe { raw::get(&key) }
}

/// Returns jemalloc's configured per-CPU arena mode.
///
/// The result is one of `"disabled"`, `"percpu"`, or `"phycpu"`. This wrapper
/// returns static Rust strings rather than exposing jemalloc's pointer.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query, returns a null pointer, or
/// reports an undocumented mode.
pub fn percpu_mode() -> Result<&'static str> {
	let key = key::opt_percpu_arena()?;

	// SAFETY: `opt.percpu_arena` has the C output type `const char *`.
	let ptr = unsafe { raw::get::<*const c_char>(&key) }?;
	if ptr.is_null() {
		return Err(Error::bad_address());
	}

	// SAFETY: the pointer names a process-lifetime static, terminated mode.
	let value = unsafe { CStr::from_ptr(ptr) }.to_bytes();
	match value {
		| b"disabled" => Ok("disabled"),
		| b"percpu" => Ok("percpu"),
		| b"phycpu" => Ok("phycpu"),
		| _ => Err(Error::invalid_argument()),
	}
}

/// Returns whether jemalloc uses either CPU-affine arena mode.
///
/// Query failures produce `false` because an unavailable mode is treated as
/// disabled by this convenience predicate.
#[must_use]
#[inline]
pub fn is_affine() -> bool { percpu_mode().is_ok_and(|mode| matches!(mode, "percpu" | "phycpu")) }

/// Returns whether jemalloc assigns arenas per logical CPU.
///
/// Query failures produce `false`.
#[must_use]
#[inline]
pub fn is_percpu() -> bool { percpu_mode().is_ok_and(|mode| mode == "percpu") }

/// Returns whether jemalloc assigns arenas per physical CPU.
///
/// Query failures produce `false`.
#[must_use]
#[inline]
pub fn is_phycpu() -> bool { percpu_mode().is_ok_and(|mode| mode == "phycpu") }

/// Invokes an all-arena no-value command.
fn notify(key: Key) -> Result {
	let key = select(key);

	// SAFETY: closed call sites provide decay or purge, which both accept the
	// all-arenas sentinel and no input or output values.
	unsafe { raw::notify(&key) }
}

/// Substitutes the documented all-arenas sentinel into a MIB template.
fn select(mut key: Key) -> Key {
	key[1] = ALL_ARENAS;
	key
}

#[cfg(test)]
mod tests {
	//! Checks the all-arena MIB dimension without invoking allocator controls.

	use super::*;

	/// Substitutes the stable sentinel at MIB component one.
	#[test]
	fn substitutes_all_arenas_component() {
		let key = select(key::arena_decay().unwrap());

		assert_eq!(key[1], ALL_ARENAS);
	}
}
