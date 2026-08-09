#![cfg(test)]

//! Defining `malloc_conf` at link time reaches jemalloc's option parser.
//!
//! The binary exports its own `malloc_conf`, overriding the definition
//! jemalloc ships, then reads `opt.stats_print_opts` back through `mallctl` to
//! show the string was parsed and not merely linked.

union U {
	x: &'static u8,
	y: &'static libc::c_char,
}

/// The configuration string this binary links in, in place of jemalloc's own.
#[allow(non_upper_case_globals)]
#[cfg_attr(prefixed, unsafe(export_name = "_rjem_malloc_conf"))]
#[cfg_attr(not(prefixed), unsafe(no_mangle))]
pub static malloc_conf: Option<&'static libc::c_char> =
	Some(unsafe { U { x: &b"stats_print_opts:mdal\0"[0] }.y });

#[test]
fn malloc_conf_set() {
	unsafe {
		assert_eq!(jevmalloc_sys::malloc_conf, malloc_conf);

		let mut ptr: *const libc::c_char = std::ptr::null();
		let mut ptr_len: libc::size_t = size_of::<*const libc::c_char>() as libc::size_t;

		let r = jevmalloc_sys::mallctl(
			(&raw const b"opt.stats_print_opts\0"[0]).cast::<libc::c_char>(),
			(&raw mut ptr).cast::<libc::c_void>(),
			&raw mut ptr_len,
			std::ptr::null_mut(),
			0,
		);

		assert_eq!(r, 0);
		assert!(!ptr.is_null());

		let s = std::ffi::CStr::from_ptr(ptr)
			.to_string_lossy()
			.into_owned();

		assert!(s.contains("mdal"), "opt.stats_print_opts: \"{}\" (len = {})", s, s.len());
	}
}
