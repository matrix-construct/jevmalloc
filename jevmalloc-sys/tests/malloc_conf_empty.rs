//! `malloc_conf` is absent when nothing defined it.
//!
//! The negative half of `malloc_conf_set`, which links its own definition in.
//! The two cannot share a binary, because the symbol is settled at link time.

#![cfg(test)]

#[test]
fn malloc_conf_empty() {
	// SAFETY: jemalloc initializes this nullable C pointer before tests run and
	// does not mutate it concurrently.
	let config = unsafe { jevmalloc_sys::malloc_conf };
	assert!(config.is_none());
}
