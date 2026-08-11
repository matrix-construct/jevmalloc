//! Exercises ordinary `GlobalAlloc` traffic, including overaligned layouts.
//!
//! The regression in
//! [rust-lang/rust#45955](https://github.com/rust-lang/rust/issues/45955)
//! requires an alignment larger than the requested size to hold for every
//! returned pointer.

#![cfg(test)]

use core::alloc::{GlobalAlloc, Layout};

use jevmalloc::Jemalloc;

/// Routes test-harness and explicit allocations through jemalloc.
#[global_allocator]
static A: Jemalloc = Jemalloc;

/// Checks that an ordinary vector allocation succeeds through jemalloc.
#[test]
#[expect(
	clippy::reserve_after_initialization,
	clippy::collection_is_never_read
)]
fn smoke() {
	let mut a = Vec::new();
	a.reserve(1);
	a.push(3);
}

/// Checks allocations whose requested alignment exceeds their size.
///
/// This covers the regression described in
/// [rust-lang/rust#45955](https://github.com/rust-lang/rust/issues/45955).
#[test]
fn overaligned() {
	let size = 8;
	// Deliberately exceed the requested size to exercise the regression.
	let align = 16;
	let iterations = 100;
	let layout = Layout::from_size_align(size, align).unwrap();
	let mut pointers = Vec::with_capacity(iterations);
	for _ in 0..iterations {
		// SAFETY: `layout` is valid and nonzero.
		let ptr = unsafe { Jemalloc.alloc(layout) };
		if ptr.is_null() {
			for ptr in pointers {
				// SAFETY: every pointer is a distinct live allocation from
				// `Jemalloc` created with this exact layout.
				unsafe { Jemalloc.dealloc(ptr, layout) };
			}
			panic!("allocation failed");
		}
		pointers.push(ptr);
	}
	let aligned = pointers
		.iter()
		.all(|ptr| ptr.addr().is_multiple_of(align));

	for ptr in pointers {
		// SAFETY: every pointer is a distinct live allocation from `Jemalloc`
		// created with this exact layout.
		unsafe { Jemalloc.dealloc(ptr, layout) };
	}

	assert!(aligned, "Got a pointer less aligned than requested");
}
