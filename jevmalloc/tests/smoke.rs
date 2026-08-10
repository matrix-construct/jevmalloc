#![cfg(test)]

//! Exercises ordinary `GlobalAlloc` traffic, including overaligned layouts.
//!
//! The regression in
//! [rust-lang/rust#45955](https://github.com/rust-lang/rust/issues/45955)
//! requires an alignment larger than the requested size to hold for every
//! returned pointer.

use std::alloc::{GlobalAlloc, Layout};

use jevmalloc::Jemalloc;

/// Routes test-harness and explicit allocations through jemalloc.
#[global_allocator]
static A: Jemalloc = Jemalloc;

/// Checks that an ordinary vector allocation succeeds through jemalloc.
#[test]
#[allow(clippy::reserve_after_initialization)]
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
	unsafe {
		let pointers: Vec<_> = (0..iterations)
			.map(|_| {
				let ptr = Jemalloc.alloc(Layout::from_size_align(size, align).unwrap());
				assert!(!ptr.is_null());
				ptr
			})
			.collect();
		for &ptr in &pointers {
			assert_eq!((ptr as usize) % align, 0, "Got a pointer less aligned than requested");
		}

		// Return every live allocation with the same layout used to create it.
		for &ptr in &pointers {
			Jemalloc.dealloc(ptr, Layout::from_size_align(size, align).unwrap());
		}
	}
}
