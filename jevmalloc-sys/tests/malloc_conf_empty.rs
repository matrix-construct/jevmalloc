#![cfg(test)]

//! `malloc_conf` is absent when nothing defined it.
//!
//! The negative half of `malloc_conf_set`, which links its own definition in.
//! The two cannot share a binary, because the symbol is settled at link time.

#[test]
fn malloc_conf_empty() {
	unsafe {
		assert!(jevmalloc_sys::malloc_conf.is_none());
	}
}
