//! Arena selection, reclamation, and decay controls.

use core::ffi::CStr;

use libc::{c_char, c_uint};

use super::{Error, Key, Result, key, raw};

/// Jemalloc's documented MIB sentinel for all arenas.
const ALL_ARENAS: usize = 4096;

/// Reclaims unused pages from one arena or from all arenas.
///
/// The operation applies time-based decay before purging all remaining unused
/// dirty pages. Passing `None` selects all arenas.
///
/// # Errors
///
/// Returns an error if jemalloc rejects either reclamation command.
pub fn trim<I: Into<Option<usize>>>(arena: I) -> Result {
	let arena = arena.into();
	decay(arena)?;
	purge(arena)
}

/// Purges all unused dirty pages from one arena or from all arenas.
///
/// Passing `None` substitutes jemalloc's all-arenas MIB value. An explicit
/// identifier selects only that arena.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the arena or purge command.
pub fn purge<I: Into<Option<usize>>>(arena: I) -> Result {
	notify_by_arena(arena.into(), key::arena_purge()?)
}

/// Applies decay-based purging to one arena or to all arenas.
///
/// Passing `None` substitutes jemalloc's all-arenas MIB value. Jemalloc uses
/// each selected arena's dirty and muzzy decay intervals.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the arena or decay command.
pub fn decay<I: Into<Option<usize>>>(arena: I) -> Result {
	notify_by_arena(arena.into(), key::arena_decay()?)
}

/// Sets an arena's muzzy-page decay interval or the default for future arenas.
///
/// Passing `Some` selects an existing arena. Passing `None` changes only the
/// initial value for arenas created later. The previous interval is returned.
/// For an existing arena, every write marks all unused muzzy pages fully
/// decayed and immediately purges them unless the value is `-1`. A value of `0`
/// otherwise requests immediate decay, while `-1` disables it.
///
/// # Errors
///
/// Returns an error if the arena does not exist or jemalloc rejects the value.
pub fn set_muzzy_decay<I: Into<Option<usize>>>(arena: I, decay_ms: isize) -> Result<isize> {
	match arena.into() {
		| Some(arena) => set_by_arena(Some(arena), key::arena_muzzy_decay()?, decay_ms),
		| None => {
			let key = key::arenas_muzzy_decay()?;
			// SAFETY: `arenas.muzzy_decay_ms` has C type `ssize_t` for input and output.
			unsafe { raw::xchg(&key, &decay_ms) }
		},
	}
}

/// Sets an arena's dirty-page decay interval or the default for future arenas.
///
/// Passing `Some` selects an existing arena. Passing `None` changes only the
/// initial value for arenas created later. The previous interval is returned.
/// For an existing arena, every write marks all unused dirty pages fully
/// decayed and immediately purges them unless the value is `-1`. A value of `0`
/// otherwise requests immediate decay, while `-1` disables it.
///
/// # Errors
///
/// Returns an error if the arena does not exist or jemalloc rejects the value.
pub fn set_dirty_decay<I: Into<Option<usize>>>(arena: I, decay_ms: isize) -> Result<isize> {
	match arena.into() {
		| Some(arena) => set_by_arena(Some(arena), key::arena_dirty_decay()?, decay_ms),
		| None => {
			let key = key::arenas_dirty_decay()?;
			// SAFETY: `arenas.dirty_decay_ms` has C type `ssize_t` for input and output.
			unsafe { raw::xchg(&key, &decay_ms) }
		},
	}
}

/// Returns jemalloc's current limit on the number of arenas.
///
/// This includes automatically managed and explicitly created arenas. Ordinary
/// arena identifiers are smaller than this value. Jemalloc reserves `4096` as
/// the stable all-arenas sentinel for supported commands.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query or the count cannot fit in a
/// Rust `usize`.
pub fn arenas() -> Result<usize> {
	let key = key::arenas_count()?;
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
/// The result is one of `"disabled"`, `"percpu"`, or `"phycpu"`. The wrapper
/// returns its own static strings rather than exposing jemalloc's pointer.
///
/// # Errors
///
/// Returns an error if jemalloc rejects the query, returns a null pointer, or
/// reports a mode outside the documented set.
pub fn percpu_arenas() -> Result<&'static str> {
	let key = key::percpu_arena()?;
	// SAFETY: `opt.percpu_arena` has the C output type `const char *`.
	let ptr = unsafe { raw::get::<*const c_char>(&key) }?;
	if ptr.is_null() {
		return Err(Error::bad_address());
	}

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
/// Query failures produce `false` because this convenience predicate treats an
/// unavailable mode as disabled.
#[must_use]
pub fn is_affine_arena() -> bool {
	percpu_arenas().is_ok_and(|mode| matches!(mode, "percpu" | "phycpu"))
}

/// Returns whether jemalloc assigns arenas per logical CPU.
///
/// Query failures produce `false`.
#[must_use]
pub fn is_percpu_arena() -> bool { percpu_arenas().is_ok_and(|mode| mode == "percpu") }

/// Returns whether jemalloc assigns arenas per physical CPU.
///
/// Query failures produce `false`.
#[must_use]
pub fn is_phycpu_arena() -> bool { percpu_arenas().is_ok_and(|mode| mode == "phycpu") }

/// Invokes an arena command after substituting an arena identifier.
pub(super) fn notify_by_arena(id: Option<usize>, mut key: Key) -> Result {
	key = select_arena(id, key);
	// SAFETY: callers supply only the documented arena purge or decay command.
	unsafe { raw::notify(&key) }
}

/// Updates an arena control after substituting an arena identifier.
pub(super) fn set_by_arena<T: Copy>(id: Option<usize>, mut key: Key, value: T) -> Result<T> {
	key = select_arena(id, key);
	// SAFETY: callers select a key whose C input and output types are both `T`.
	unsafe { raw::xchg(&key, &value) }
}

/// Reads an arena control after substituting an arena identifier.
pub(super) fn get_by_arena<T: Copy>(id: Option<usize>, mut key: Key) -> Result<T> {
	key = select_arena(id, key);
	// SAFETY: callers select a key whose C output type is `T`.
	unsafe { raw::get(&key) }
}

/// Substitutes an explicit arena or the documented all-arenas sentinel.
fn select_arena(id: Option<usize>, mut key: Key) -> Key {
	key[1] = id.unwrap_or(ALL_ARENAS);
	key
}

#[cfg(test)]
mod tests {
	//! Checks the numeric arena component used by each interface dimension.

	use super::*;

	/// Confirms explicit and all-arena substitutions at MIB index one.
	#[test]
	fn substitutes_arena_component() {
		let thread = select_arena(Some(7), key::thread_arena_decay().unwrap());
		let all = select_arena(None, key::arena_decay().unwrap());

		assert_eq!(thread[1], 7);
		assert_eq!(all[1], ALL_ARENAS);
	}
}
