//! Exercises the raw `jevmalloc_sys` entry points with `Jemalloc` installed.
//!
//! The tests cover extended allocation calls, both named and MIB control
//! access, and the writer callback driven by `malloc_stats_print`.

#![cfg(test)]

use core::ptr;
use std::sync::Mutex;

use jevmalloc::{Jemalloc, ffi};
use libc::{c_char, c_void};

/// Keeps Rust allocations and the raw FFI calls on the same allocator.
#[global_allocator]
static A: Jemalloc = Jemalloc;

/// Serializes operations that refresh and compare cached allocator statistics.
static STATS: Mutex<()> = Mutex::new(());

/// Checks allocation, resizing, size queries, and sized deallocation.
#[test]
fn test_basic_alloc() {
	// SAFETY: the request is nonzero and the zero flag word is valid.
	let expected = unsafe { ffi::nallocx(100, 0) };
	assert!(expected >= 100);

	// SAFETY: the request is nonzero and the zero flag word is valid.
	let mut ptr = unsafe { ffi::mallocx(100, 0) };
	assert!(!ptr.is_null());

	// SAFETY: `ptr` is a live allocation owned by linked jemalloc.
	let usable = unsafe { ffi::sallocx(ptr, 0) };

	// SAFETY: `ptr` is live, and the nonzero request and flags are valid.
	let resized = unsafe { ffi::rallocx(ptr, 50, 0) };
	if resized.is_null() {
		// SAFETY: failed reallocation leaves the original allocation live.
		unsafe { ffi::sdallocx(ptr, 100, 0) };
		panic!("rallocx failed");
	}
	ptr = resized;

	// SAFETY: `ptr` is live, the primary request is nonzero, and the two sizes
	// cannot overflow when added.
	let size = unsafe { ffi::xallocx(ptr, 30, 20, 0) };

	// SAFETY: `ptr` remains live, and `xallocx` returned its exact usable size.
	unsafe { ffi::sdallocx(ptr, size, 0) };

	assert_eq!(expected, usable);
	assert!(size >= 30);
}

/// Checks that named and MIB reads report the same allocated byte count.
#[test]
fn test_mallctl() {
	let _guard = STATS.lock().unwrap();
	let mut allocated: usize = 0;
	let mut val_len = size_of_val(&allocated);
	let field = "stats.allocated\0";

	// SAFETY: `field` is NUL-terminated; the output and length storage is
	// aligned, writable, and live; no input value is supplied.
	let named_code = unsafe {
		ffi::mallctl(
			field.as_ptr().cast(),
			(&raw mut allocated).cast::<c_void>(),
			&raw mut val_len,
			ptr::null_mut(),
			0,
		)
	};

	let mut mib = [0, 0];
	let mut mib_len = 2;

	// SAFETY: `field` is NUL-terminated, and `mib` supplies `mib_len` aligned,
	// writable slots with live length storage.
	let translate_code = unsafe {
		ffi::mallctlnametomib(field.as_ptr().cast(), mib.as_mut_ptr(), &raw mut mib_len)
	};
	let mut allocated_by_mib = 0;
	let mib_code = if translate_code == 0 {
		val_len = size_of_val(&allocated_by_mib);

		// SAFETY: successful name translation initialized `mib`; the output and
		// length storage is aligned, writable, and live; no input is supplied.
		unsafe {
			ffi::mallctlbymib(
				mib.as_ptr(),
				mib_len,
				(&raw mut allocated_by_mib).cast::<c_void>(),
				&raw mut val_len,
				ptr::null_mut(),
				0,
			)
		}
	} else {
		translate_code
	};

	assert_eq!(named_code, 0);
	assert_eq!(translate_code, 0);
	assert_eq!(mib_code, 0);
	assert_eq!(allocated_by_mib, allocated);
}

/// Checks that `malloc_stats_print` invokes the supplied writer callback.
#[test]
fn test_stats() {
	/// Counts fragments delivered to the statistics writer.
	struct PrintCtx {
		/// Whether the writer callback has run.
		called: bool,
	}

	/// Counts one writer callback invocation.
	///
	/// The fragment pointer is ignored because the test verifies only that
	/// `malloc_stats_print` drives the callback.
	extern "C" fn write_cb(ctx: *mut c_void, _: *const c_char) {
		// SAFETY: `malloc_stats_print` invokes the callback synchronously with
		// the live, aligned `PrintCtx` pointer supplied below.
		let print_ctx = unsafe { &mut *ctx.cast::<PrintCtx>() };
		print_ctx.called = true;
	}

	let _guard = STATS.lock().unwrap();
	let mut ctx = PrintCtx { called: false };

	// SAFETY: the callback context is aligned, writable, and live for the
	// synchronous call; the static NUL-terminated options omit detailed
	// sections while retaining the summary framing.
	unsafe {
		ffi::malloc_stats_print(Some(write_cb), (&raw mut ctx).cast(), c"gmdablxeh".as_ptr());
	};

	assert!(ctx.called, "print should be triggered at least once.");
}
