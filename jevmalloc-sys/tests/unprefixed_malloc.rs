//! Whether `malloc` carries a prefix follows the `prefixed` cfg the build
//! script sets.
//!
//! Prefixed, the crate's `malloc` is jemalloc's own symbol and must differ
//! from libc's; unprefixed, jemalloc has taken libc's place and the two
//! addresses must agree.

#![cfg(test)]

#[cfg(prefixed)]
#[test]
fn malloc_is_prefixed() {
	assert_ne!(jevmalloc_sys::malloc as *const () as usize, libc::malloc as *const () as usize)
}

#[cfg(not(prefixed))]
#[test]
fn malloc_is_overridden() {
	assert_eq!(jevmalloc_sys::malloc as *const () as usize, libc::malloc as *const () as usize);
}
