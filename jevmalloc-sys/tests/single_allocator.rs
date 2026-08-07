#![cfg(test)]

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

use std::ffi::CString;
#[cfg(not(prefixed))]
use std::ptr::null_mut;

use jevmalloc_sys as je;
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
	// jemalloc has to be initialized before its ctl namespace answers.
	unsafe { je::free(je::malloc(1)) };

	let mut arena: c_uint = c_uint::MAX;
	let mut len = size_of::<c_uint>();
	let rc = unsafe {
		je::mallctl(
			c"arenas.lookup".as_ptr(),
			(&raw mut arena).cast::<c_void>(),
			&raw mut len,
			(&raw const ptr).cast::<c_void>().cast_mut(),
			size_of::<*const c_void>(),
		)
	};

	rc.eq(&0).then_some(arena)
}

#[test]
fn jemalloc_owns_what_jemalloc_allocated() {
	unsafe {
		let ptr = je::malloc(1024);

		assert_owned(ptr, "je::malloc");
		je::free(ptr);
	}
}

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
	use super::{CString, assert_owned, je, null_mut};

	#[test]
	fn libc_malloc_is_jemalloc() {
		assert_eq!(
			libc::malloc as *const (),
			je::malloc as *const (),
			"the libc `malloc` name did not resolve to jemalloc's definition"
		);
	}

	#[test]
	fn libc_allocates_through_jemalloc() {
		unsafe {
			let ptr = libc::malloc(1024);

			assert_owned(ptr, "libc::malloc");
			libc::free(ptr);
		}
	}

	/// Allocations libc makes on the caller's behalf and hands back for the
	/// caller to free.
	///
	/// `realpath` is the specific case that segfaulted rustc when the
	/// interposition was incomplete; see `NO_UNPREFIXED_MALLOC_TARGETS` in
	/// `src/env.rs`.
	#[test]
	fn libc_internal_allocations_come_from_jemalloc() {
		unsafe {
			let src = CString::new("a string for libc to duplicate").unwrap();
			let dup = libc::strdup(src.as_ptr());

			assert_owned(dup.cast(), "strdup()");
			je::free(dup.cast());

			let cwd = libc::getcwd(null_mut(), 0);

			assert_owned(cwd.cast(), "getcwd(NULL, 0)");
			je::free(cwd.cast());

			let dot = CString::new(".").unwrap();
			let real = libc::realpath(dot.as_ptr(), null_mut());

			assert_owned(real.cast(), "realpath(.., NULL)");
			je::free(real.cast());
		}
	}
}

/// Prefixed: jemalloc answers only to `_rjem_`-prefixed names.
///
/// Libc keeps its own heap, two allocators coexist by design, and the boundary
/// between them is exactly what must not be crossed.
#[cfg(prefixed)]
mod prefixed {
	use super::{CString, arena_of, je};

	#[test]
	fn libc_malloc_is_not_jemalloc() {
		assert_ne!(
			libc::malloc as *const (),
			je::malloc as *const (),
			"a prefixed build still overrode the libc `malloc` name"
		);
	}

	#[test]
	fn libc_keeps_its_own_heap() {
		unsafe {
			let src = CString::new("a string for libc to duplicate").unwrap();
			let dup = libc::strdup(src.as_ptr());

			assert!(!dup.is_null());
			assert_eq!(arena_of(dup.cast()), None, "jemalloc claimed a libc allocation");
			libc::free(dup.cast());
		}
	}
}
