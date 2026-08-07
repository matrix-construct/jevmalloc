#![cfg(test)]

//! The ISO C23 sized-deallocation entry points, new in `jemalloc` 5.3.1.
//!
//! Both are thin forwards onto `sdallocx`, so these only assert that the
//! symbols link and that a round-trip through them is accepted. Neither
//! accepts a null pointer, so there is deliberately no null case here.

use jevmalloc_sys as je;

#[test]
fn free_sized_roundtrip() {
	const SIZE: usize = 64;

	unsafe {
		let ptr = je::malloc(SIZE);

		assert!(!ptr.is_null());
		ptr.cast::<u8>().write_bytes(0xAB, SIZE);
		je::free_sized(ptr, SIZE);
	}
}

#[test]
fn free_aligned_sized_roundtrip() {
	const ALIGN: usize = 64;
	const SIZE: usize = 128;

	unsafe {
		let ptr = je::aligned_alloc(ALIGN, SIZE);

		assert!(!ptr.is_null());
		assert!(ptr.addr().is_multiple_of(ALIGN), "aligned_alloc under-aligned the allocation");
		ptr.cast::<u8>().write_bytes(0xCD, SIZE);
		je::free_aligned_sized(ptr, ALIGN, SIZE);
	}
}
