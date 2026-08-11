//! Exercises a round trip through `jevmalloc_sys::malloc` and `free`.
//!
//! The test calls the C entry points directly; the global allocator below is
//! installed only as a workaround, not as the subject.

#![cfg(test)]

/// Installs jemalloc globally to avoid
/// [gnzlbg/jemallocator#19](https://github.com/gnzlbg/jemallocator/issues/19).
#[global_allocator]
static A: jevmalloc::Jemalloc = jevmalloc::Jemalloc;

/// Checks that a word written through the raw allocation can be read and freed.
#[test]
fn smoke() {
	// SAFETY: the nonzero request has no additional alignment requirement.
	let ptr = unsafe { jevmalloc_sys::malloc(4) };
	assert!(!ptr.is_null());

	// SAFETY: `malloc` provides enough writable, suitably aligned storage for
	// one `u32`.
	unsafe { ptr.cast::<u32>().write(0xDECADE) };

	// SAFETY: the preceding write initialized the live `u32` allocation.
	let value = unsafe { ptr.cast::<u32>().read() };

	// SAFETY: `ptr` is the still-live result from `malloc`.
	unsafe { jevmalloc_sys::free(ptr) };
	assert_eq!(value, 0xDECADE);
}
