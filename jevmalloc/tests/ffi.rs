//! Exercises the raw `jevmalloc_sys` entry points with `Jemalloc` installed.
//!
//! The tests cover extended allocation calls, both named and MIB control
//! access, and the writer callback driven by `malloc_stats_print`.

#![cfg(test)]

use core::ptr;

use jevmalloc::{Jemalloc, ffi};
use libc::{c_char, c_void};

/// Keeps Rust allocations and the raw FFI calls on the same allocator.
#[global_allocator]
static A: Jemalloc = Jemalloc;

/// Checks allocation, resizing, size queries, and sized deallocation.
#[test]
fn test_basic_alloc() {
	unsafe {
		let exp_size = ffi::nallocx(100, 0);
		assert!(exp_size >= 100);

		let mut ptr = ffi::mallocx(100, 0);
		assert!(!ptr.is_null());

		assert_eq!(exp_size, ffi::malloc_usable_size(ptr));

		ptr = ffi::rallocx(ptr, 50, 0);
		let size = ffi::xallocx(ptr, 30, 20, 0);
		assert!(size >= 50);

		ffi::sdallocx(ptr, 50, 0);
	}
}

/// Checks that named and MIB reads report the same allocated byte count.
#[test]
fn test_mallctl() {
	let ptr = unsafe { ffi::mallocx(100, 0) };

	let mut allocated: usize = 0;
	let mut val_len = size_of_val(&allocated);
	let field = "stats.allocated\0";
	let mut code;
	code = unsafe {
		ffi::mallctl(
			field.as_ptr().cast(),
			(&raw mut allocated).cast::<c_void>(),
			&raw mut val_len,
			ptr::null_mut(),
			0,
		)
	};
	assert_eq!(code, 0);
	assert!(allocated > 0);

	let mut mib = [0, 0];
	let mut mib_len = 2;
	code = unsafe {
		ffi::mallctlnametomib(field.as_ptr().cast(), mib.as_mut_ptr(), &raw mut mib_len)
	};
	assert_eq!(code, 0);
	let mut allocated_by_mib = 0;
	let code = unsafe {
		ffi::mallctlbymib(
			mib.as_ptr(),
			mib_len,
			(&raw mut allocated_by_mib).cast::<c_void>(),
			&raw mut val_len,
			ptr::null_mut(),
			0,
		)
	};
	assert_eq!(code, 0);
	assert_eq!(allocated_by_mib, allocated);

	unsafe { ffi::sdallocx(ptr, 100, 0) };
}

/// Checks that `malloc_stats_print` invokes the supplied writer callback.
#[test]
fn test_stats() {
	/// Counts fragments delivered to the statistics writer.
	struct PrintCtx {
		/// Number of times the writer callback has run.
		called_times: usize,
	}

	/// Counts one writer callback invocation.
	///
	/// The fragment pointer is ignored because the test verifies only that
	/// `malloc_stats_print` drives the callback.
	extern "C" fn write_cb(ctx: *mut c_void, _: *const c_char) {
		let print_ctx = unsafe { &mut *ctx.cast::<PrintCtx>() };
		print_ctx.called_times += 1;
	}

	let mut ctx = PrintCtx { called_times: 0 };
	unsafe {
		ffi::malloc_stats_print(Some(write_cb), (&raw mut ctx).cast::<c_void>(), ptr::null());
	};

	assert_ne!(ctx.called_times, 0, "print should be triggered at lease once.");
}
