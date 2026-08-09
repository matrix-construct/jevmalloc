#![cfg(test)]

//! A round trip through `jevmalloc_sys::malloc` and `free`.
//!
//! The test calls the C entry points directly; the global allocator below is
//! installed only as a workaround, not as the subject.

// Work around https://github.com/gnzlbg/jemallocator/issues/19
#[global_allocator]
static A: jevmalloc::Jemalloc = jevmalloc::Jemalloc;

#[test]
fn smoke() {
	unsafe {
		let ptr = jevmalloc_sys::malloc(4);
		*ptr.cast::<u32>() = 0xDECADE;
		assert_eq!(*ptr.cast::<u32>(), 0xDECADE);
		jevmalloc_sys::free(ptr);
	}
}
