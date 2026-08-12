//! Exercises the documented build-configuration getters.

#![cfg(test)]

use jevmalloc::{Jemalloc, config};

/// Routes the test harness through the observed jemalloc instance.
#[global_allocator]
static ALLOC: Jemalloc = Jemalloc;

/// Checks that every fixed `config.*` control is available.
#[test]
fn documented_config_getters_succeed() {
	config::cache_oblivious().unwrap();
	config::debug().unwrap();
	config::fill().unwrap();
	config::lazy_lock().unwrap();
	config::malloc_conf().unwrap();
	config::prof().unwrap();
	config::prof_libgcc().unwrap();
	config::prof_libunwind().unwrap();
	config::stats().unwrap();
	config::utrace().unwrap();
	config::xmalloc().unwrap();
}
