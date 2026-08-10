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
	unsafe {
		let ptr = jevmalloc_sys::malloc(4);
		*ptr.cast::<u32>() = 0xDECADE;
		assert_eq!(*ptr.cast::<u32>(), 0xDECADE);
		jevmalloc_sys::free(ptr);
	}
}
