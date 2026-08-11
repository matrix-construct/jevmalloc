//! Proves which allocator services the process.
//!
//! The oracle is `mallctl("arenas.lookup")`, which jemalloc 5.3.1 made safe to
//! call on a pointer it does not own: it reports `EINVAL` instead of crashing.
//!
//! The interesting cases are not our own allocations but the ones made *inside*
//! libc. When an unprefixed build interposes `malloc` incompletely, libc
//! allocates from its own heap and the application frees through jemalloc, and
//! the process dies far from the cause. These tests pin down which of the two
//! regimes is in force rather than leaving it to be discovered at runtime.

#![cfg(test)]

#[cfg(not(prefixed))]
use core::ptr::null_mut;

use jevmalloc_sys as ffi;
use libc::{c_uint, c_void};

/// Verifies the oracle itself.
///
/// A pointer that is definitively not from any heap must not be attributed to
/// jemalloc, or every other assertion here would pass vacuously.
#[test]
fn oracle_rejects_a_stack_address() {
	let on_stack = [0_u8; 32];

	assert_eq!(arena_of((&raw const on_stack).cast()), None);
}

/// The jemalloc arena owning `ptr`, or `None` when jemalloc does not own it.
///
/// 5.3.1 made `arenas.lookup` safe on a foreign pointer, so ownership is an
/// ordinary assertion.
fn arena_of(ptr: *const c_void) -> Option<c_uint> {
	let mut arena: c_uint = c_uint::MAX;
	let mut len = size_of::<c_uint>();

	// SAFETY: the name is NUL-terminated. `arena` and `len` are aligned,
	// writable, and live; `ptr` is supplied through a readable pointer-sized
	// input slot that also remains live.
	let rc = unsafe {
		ffi::mallctl(
			c"arenas.lookup".as_ptr(),
			(&raw mut arena).cast::<c_void>(),
			&raw mut len,
			(&raw const ptr).cast::<c_void>().cast_mut(),
			size_of::<*const c_void>(),
		)
	};

	match rc {
		| 0 => {
			assert_eq!(len, size_of::<c_uint>());
			Some(arena)
		},
		| libc::EINVAL => None,
		| error => panic!("arenas.lookup failed with status {error}"),
	}
}

#[test]
fn jemalloc_owns_what_jemalloc_allocated() {
	// SAFETY: the nonzero request has no additional alignment requirement.
	let ptr = unsafe { ffi::malloc(1024) };
	assert!(!ptr.is_null(), "ffi::malloc returned null");
	let owned = arena_of(ptr).is_some();

	// SAFETY: `ptr` is the still-live result of `ffi::malloc`.
	unsafe { ffi::free(ptr) };
	assert!(owned, "ffi::malloc did not come from jemalloc");
}

#[cfg(not(prefixed))]
fn assert_owned(ptr: *const c_void, what: &str) {
	assert!(!ptr.is_null(), "{what} returned null");
	assert!(arena_of(ptr).is_some(), "{what} did not come from jemalloc");
}

/// Unprefixed: jemalloc has taken over the C names.
///
/// Libc's own internal allocations then come out of jemalloc too, and the heap
/// is not split.
#[cfg(not(prefixed))]
mod unprefixed {
	use super::{assert_owned, ffi, null_mut};

	#[test]
	fn libc_malloc_is_jemalloc() {
		assert_eq!(
			libc::malloc as *const (),
			ffi::malloc as *const (),
			"the libc `malloc` name did not resolve to jemalloc's definition"
		);
	}

	#[test]
	fn libc_allocates_through_jemalloc() {
		// SAFETY: the nonzero request has no additional alignment requirement.
		let ptr = unsafe { libc::malloc(1024) };
		assert!(!ptr.is_null(), "libc::malloc returned null");
		let owned = super::arena_of(ptr).is_some();

		// SAFETY: `ptr` is a live result from this same C allocation API.
		unsafe { libc::free(ptr) };
		assert!(owned, "libc::malloc did not come from jemalloc");
	}

	/// Allocations libc makes on the caller's behalf and hands back for the
	/// caller to free.
	///
	/// `realpath` is the specific case that segfaulted rustc when the
	/// interposition was incomplete; see `NO_UNPREFIXED_MALLOC_TARGETS` in
	/// `src/env.rs`.
	#[test]
	fn libc_internal_allocations_come_from_jemalloc() {
		let src = c"a string for libc to duplicate";

		// SAFETY: `src` is NUL-terminated and remains live for the call.
		let dup = unsafe { libc::strdup(src.as_ptr()) };
		assert_owned(dup.cast(), "strdup()");

		// SAFETY: `assert_owned` established a live jemalloc allocation.
		unsafe { ffi::free(dup.cast()) };

		// SAFETY: a null buffer with zero size requests an allocated C string.
		let cwd = unsafe { libc::getcwd(null_mut(), 0) };
		assert_owned(cwd.cast(), "getcwd(NULL, 0)");

		// SAFETY: `assert_owned` established a live jemalloc allocation.
		unsafe { ffi::free(cwd.cast()) };
		let dot = c".";

		// SAFETY: `dot` is NUL-terminated and live; the null output requests an
		// allocated canonical path.
		let real = unsafe { libc::realpath(dot.as_ptr(), null_mut()) };
		assert_owned(real.cast(), "realpath(.., NULL)");

		// SAFETY: `assert_owned` established a live jemalloc allocation.
		unsafe { ffi::free(real.cast()) };
	}
}

/// Prefixed: jemalloc answers only to `_rjem_`-prefixed names.
///
/// Libc keeps its own heap, two allocators coexist by design, and the boundary
/// between them is exactly what must not be crossed.
#[cfg(prefixed)]
mod prefixed {
	use super::{arena_of, ffi};

	#[test]
	fn libc_malloc_is_not_jemalloc() {
		assert_ne!(
			libc::malloc as *const (),
			ffi::malloc as *const (),
			"a prefixed build still overrode the libc `malloc` name"
		);
	}

	#[test]
	fn libc_keeps_its_own_heap() {
		let src = c"a string for libc to duplicate";

		// SAFETY: `src` is NUL-terminated and remains live for the call.
		let dup = unsafe { libc::strdup(src.as_ptr()) };
		assert!(!dup.is_null());
		let arena = arena_of(dup.cast());

		// SAFETY: `dup` is the still-live result of `libc::strdup`.
		unsafe { libc::free(dup.cast()) };
		assert_eq!(arena, None, "jemalloc claimed a libc allocation");
	}
}
