//! Exercises the MIB-only control wrapper through its public surface.

#![cfg(test)]

use core::alloc::{GlobalAlloc, Layout};
use std::sync::Mutex;

use jevmalloc::{Jemalloc, ctl};

/// Routes test-harness allocations through the same jemalloc instance.
#[global_allocator]
static ALLOC: Jemalloc = Jemalloc;

/// Serializes tests that update process-wide allocator state.
static CONTROL: Mutex<()> = Mutex::new(());

/// Checks a basic allocation while the control-test allocator is installed.
#[test]
fn allocator_smoke() {
	let layout = Layout::from_size_align(100, 8).unwrap();
	unsafe {
		let ptr = Jemalloc.alloc(layout);
		assert!(!ptr.is_null());
		Jemalloc.dealloc(ptr, layout);
	}
}

/// Checks name translation, a typed raw read, and an invalid MIB error.
#[test]
fn raw_mib_access() {
	let key = ctl::raw::mibs("epoch").unwrap();
	assert_eq!(key.len(), 1);

	let epoch = unsafe { ctl::raw::get::<u64>(&key) }.unwrap();
	assert!(epoch > 0);

	let mut invalid = ctl::Key::new();
	invalid.push(usize::MAX);
	let error = unsafe { ctl::raw::get::<u64>(&invalid) }.unwrap_err();
	assert!(error.is(libc::ENOENT));

	let cache_key = ctl::raw::mibs("thread.tcache.enabled").unwrap();
	let cache = unsafe { ctl::raw::get::<bool>(&cache_key) }.unwrap();
	unsafe { ctl::raw::set(&cache_key, &cache) }.unwrap();
	assert_eq!(unsafe { ctl::raw::get::<bool>(&cache_key) }.unwrap(), cache);
}

/// Checks that a refresh advances jemalloc's allocator-maintained epoch.
#[test]
fn epoch_refresh_is_explicit() {
	let _guard = CONTROL.lock().unwrap();
	let before = ctl::epoch().unwrap();
	let refreshed = ctl::refresh_epoch().unwrap();
	let after = ctl::epoch().unwrap();

	assert!(refreshed > before);
	assert_eq!(after, refreshed);
}

/// Checks the global arena queries and the documented affinity predicates.
#[test]
fn arena_queries_are_consistent() {
	assert!(ctl::arenas().unwrap() > 0);
	assert!(ctl::quantum().unwrap() > 0);

	let mode = ctl::percpu_arenas().unwrap();
	assert!(matches!(mode, "disabled" | "percpu" | "phycpu"));
	assert_eq!(ctl::is_percpu_arena(), mode == "percpu");
	assert_eq!(ctl::is_phycpu_arena(), mode == "phycpu");
	assert_eq!(ctl::is_affine_arena(), mode != "disabled");
}

/// Checks both all-arena reclamation commands through the migrated entry point.
#[test]
fn all_arenas_trim() { ctl::trim(None).unwrap(); }

/// Checks that updating the background setting returns its current value.
#[test]
fn background_thread_exchange() {
	let _guard = CONTROL.lock().unwrap();
	let key = ctl::raw::mibs("background_thread").unwrap();
	let current = unsafe { ctl::raw::get::<bool>(&key) }.unwrap();
	let previous = ctl::background_thread_enable(current).unwrap();

	assert_eq!(previous, current);
}

/// Checks future-arena decay defaults without changing their values.
#[test]
fn future_arena_decay_exchange() {
	let _guard = CONTROL.lock().unwrap();
	let muzzy_key = ctl::raw::mibs("arenas.muzzy_decay_ms").unwrap();
	let dirty_key = ctl::raw::mibs("arenas.dirty_decay_ms").unwrap();
	let muzzy = unsafe { ctl::raw::get::<isize>(&muzzy_key) }.unwrap();
	let dirty = unsafe { ctl::raw::get::<isize>(&dirty_key) }.unwrap();

	assert_eq!(ctl::set_muzzy_decay(None, muzzy).unwrap(), muzzy);
	assert_eq!(ctl::set_dirty_decay(None, dirty).unwrap(), dirty);
}

/// Checks current-thread MIB substitution and scalar exchanges.
#[test]
fn current_thread_controls() {
	let arena = ctl::this_thread::arena_id().unwrap();
	assert!(arena < ctl::arenas().unwrap());
	if !ctl::is_affine_arena() {
		assert_eq!(ctl::this_thread::set_arena(arena).unwrap(), arena);
	}

	let muzzy = ctl::this_thread::get_muzzy_decay().unwrap();
	let dirty = ctl::this_thread::get_dirty_decay().unwrap();
	assert_eq!(ctl::this_thread::set_muzzy_decay(muzzy).unwrap(), muzzy);
	assert_eq!(ctl::this_thread::set_dirty_decay(dirty).unwrap(), dirty);

	let cache = ctl::this_thread::is_cache_enabled().unwrap();
	assert_eq!(ctl::this_thread::cache_enable(true).unwrap(), cache);
	ctl::this_thread::flush().unwrap();
	if !cache {
		assert!(ctl::this_thread::cache_enable(false).unwrap());
	}

	ctl::this_thread::trim().unwrap();
	ctl::this_thread::idle().unwrap();
}
