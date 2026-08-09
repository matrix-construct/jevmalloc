#![cfg(test)]

//! `usable_size` reports at least as much as the allocation was asked for.

use jevmalloc::Jemalloc;

#[global_allocator]
static A: Jemalloc = Jemalloc;

#[test]
fn smoke() {
	let a = Box::new(3_u32);
	assert!(unsafe { jevmalloc::usable_size(&raw const *a) } >= 4);
}
