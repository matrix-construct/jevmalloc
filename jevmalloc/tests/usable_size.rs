//! Checks that `usable_size` covers the requested allocation size.

#![cfg(test)]

use jevmalloc::Jemalloc;

/// Routes the boxed test value through jemalloc.
#[global_allocator]
static A: Jemalloc = Jemalloc;

/// Checks that a boxed `u32` has at least four usable bytes.
#[test]
fn smoke() {
	let a = Box::new(3_u32);
	assert!(unsafe { jevmalloc::usable_size(&raw const *a) } >= 4);
}
