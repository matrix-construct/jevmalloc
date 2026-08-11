//! Defining `malloc_conf` at link time reaches jemalloc's option parser.
//!
//! The binary exports its own `malloc_conf`, overriding the definition
//! jemalloc ships, then reads `opt.stats_print_opts` back through `mallctl` to
//! show the string was parsed and not merely linked.

#![cfg(test)]

use core::{ffi::CStr, ptr};

#[repr(C)]
union U {
	x: &'static u8,
	y: &'static libc::c_char,
}

/// The configuration string this binary links in, in place of jemalloc's own.
#[cfg_attr(prefixed, unsafe(export_name = "_rjem_malloc_conf"))]
#[cfg_attr(not(prefixed), unsafe(no_mangle))]
#[cfg_attr(
	prefixed,
	expect(
		non_upper_case_globals,
		reason = "the identifier is the exported C symbol"
	)
)]
pub static malloc_conf: Option<&'static libc::c_char> =
	// SAFETY: `u8` and `c_char` have identical one-byte layouts, every bit
	// pattern is valid, and the referenced NUL-terminated bytes are static.
	Some(unsafe { U { x: &b"stats_print_opts:mdal\0"[0] }.y });

#[test]
fn malloc_conf_set() {
	// SAFETY: jemalloc initializes this nullable C pointer before tests run and
	// does not mutate it concurrently.
	let linked = unsafe { jevmalloc_sys::malloc_conf };

	assert_eq!(linked.map(ptr::from_ref), malloc_conf.map(ptr::from_ref));

	let mut value: *const libc::c_char = ptr::null();
	let mut value_len: libc::size_t = size_of::<*const libc::c_char>();

	// SAFETY: the name is NUL-terminated; the output pointer and length are
	// aligned, writable, and live; no input value is supplied.
	let status = unsafe {
		jevmalloc_sys::mallctl(
			(&raw const b"opt.stats_print_opts\0"[0]).cast::<libc::c_char>(),
			(&raw mut value).cast::<libc::c_void>(),
			&raw mut value_len,
			ptr::null_mut(),
			0,
		)
	};

	assert_eq!(status, 0);
	assert_eq!(value_len, size_of::<*const libc::c_char>());
	assert!(!value.is_null());

	// SAFETY: the successful control read returned a process-lifetime static,
	// NUL-terminated string pointer.
	let value = unsafe { CStr::from_ptr(value) };

	assert!(
		value
			.to_bytes()
			.windows(4)
			.any(|window| window == b"mdal"),
		"opt.stats_print_opts: {value:?} (len = {})",
		value.to_bytes().len()
	);
}
