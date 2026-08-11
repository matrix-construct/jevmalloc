//! The ISO C23 sized-deallocation entry points, new in `jemalloc` 5.3.1.
//!
//! Both are thin forwards onto `sdallocx`, so these only assert that the
//! symbols link and that a round-trip through them is accepted. Neither
//! accepts a null pointer, so there is deliberately no null case here.

#![cfg(test)]

use jevmalloc_sys as ffi;

#[test]
fn free_sized_roundtrip() {
	const SIZE: usize = 64;

	// SAFETY: `SIZE` is nonzero and imposes no special alignment request.
	let ptr = unsafe { ffi::malloc(SIZE) };
	assert!(!ptr.is_null());

	// SAFETY: the live allocation contains at least `SIZE` writable bytes.
	unsafe { ptr.cast::<u8>().write_bytes(0xAB, SIZE) };

	// SAFETY: `ptr` remains live and `SIZE` is the exact allocation request.
	unsafe { ffi::free_sized(ptr, SIZE) };
}

#[test]
fn free_aligned_sized_roundtrip() {
	const ALIGN: usize = 64;
	const SIZE: usize = 128;

	// SAFETY: `ALIGN` is a power of two and `SIZE` is its nonzero multiple.
	let ptr = unsafe { ffi::aligned_alloc(ALIGN, SIZE) };
	assert!(!ptr.is_null());
	let aligned = ptr.addr().is_multiple_of(ALIGN);

	// SAFETY: the live allocation contains at least `SIZE` writable bytes.
	unsafe { ptr.cast::<u8>().write_bytes(0xCD, SIZE) };

	// SAFETY: `ptr` remains live, and the exact requested alignment and size
	// accompany it.
	unsafe { ffi::free_aligned_sized(ptr, ALIGN, SIZE) };
	assert!(aligned, "aligned_alloc under-aligned the allocation");
}
