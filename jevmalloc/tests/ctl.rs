//! Exercises low-level MIB access and typed allocator controls.

#![cfg(test)]

use core::alloc::{GlobalAlloc, Layout};
use std::sync::Mutex;

use jevmalloc::{Dss, Jemalloc, arenas, ctl, stats, thread};

/// Jemalloc's C `bool` representation when built by cl.exe.
#[cfg(target_env = "msvc")]
type CBool = libc::c_int;

/// Jemalloc's C `_Bool` representation on other targets.
#[cfg(not(target_env = "msvc"))]
type CBool = bool;

/// Routes test-harness allocations through the same jemalloc instance.
#[global_allocator]
static ALLOC: Jemalloc = Jemalloc;

/// Serializes tests that update process-wide allocator state.
static CONTROL: Mutex<()> = Mutex::new(());

/// Checks a basic allocation while the control-test allocator is installed.
#[test]
fn allocator_smoke() {
	let layout = Layout::from_size_align(100, 8).unwrap();

	// SAFETY: `layout` is valid and nonzero.
	let ptr = unsafe { Jemalloc.alloc(layout) };
	assert!(!ptr.is_null());

	// SAFETY: `ptr` is a live result from this allocator for the same layout.
	unsafe { Jemalloc.dealloc(ptr, layout) };
}

/// Checks name translation and typed raw reads and writes.
#[test]
fn raw_mib_access() {
	let key = ctl::raw::mibs("epoch").unwrap();
	assert_eq!(key.len(), 1);

	// SAFETY: this is the complete `epoch` MIB with C output type `uint64_t`.
	let epoch = unsafe { ctl::raw::get::<u64>(&key) }.unwrap();
	assert!(epoch > 0);

	let cache_key = ctl::raw::mibs("thread.tcache.enabled").unwrap();

	// SAFETY: this is the complete cache-setting MIB, and `CBool` matches the
	// platform C `bool` representation.
	let cache = unsafe { ctl::raw::get::<CBool>(&cache_key) }.unwrap();

	// SAFETY: the same complete MIB accepts `CBool` as its C input type.
	unsafe { ctl::raw::set(&cache_key, &cache) }.unwrap();

	// SAFETY: this is the complete cache-setting MIB with C output type `CBool`.
	let after = unsafe { ctl::raw::get::<CBool>(&cache_key) }.unwrap();

	assert_eq!(after, cache);
}

/// Checks that a refresh advances jemalloc's allocator-maintained epoch.
#[test]
fn epoch_refresh_is_explicit() {
	let _guard = CONTROL.lock().unwrap();
	let before = stats::epoch().unwrap();
	let refreshed = stats::refresh_epoch().unwrap();
	let after = stats::epoch().unwrap();

	assert!(refreshed > before);
	assert_eq!(after, refreshed);
}

/// Checks safe statistics printing without the optional statistics feature.
#[test]
fn statistics_print_uses_the_writer() {
	let _guard = CONTROL.lock().unwrap();
	let mut output = Vec::new();

	stats::print(c"gmdablxeh", |fragment| output.extend_from_slice(fragment));

	let terminal = [
		[b'-'; 3].as_slice(),
		b" End jemalloc statistics ".as_slice(),
		[b'-'; 3].as_slice(),
		b"\n".as_slice(),
	]
	.concat();

	assert!(output.starts_with(b"___ Begin jemalloc statistics ___\n"));
	assert!(output.ends_with(&terminal));
}

/// Checks the global arena queries and the documented affinity predicates.
#[test]
fn arena_queries_are_consistent() {
	assert!(arenas::limit().unwrap() > 0);
	assert!(arenas::quantum().unwrap() > 0);

	let mode = arenas::percpu_mode().unwrap();
	assert!(matches!(mode, "disabled" | "percpu" | "phycpu"));
	assert_eq!(arenas::is_percpu(), mode == "percpu");
	assert_eq!(arenas::is_phycpu(), mode == "phycpu");
	assert_eq!(arenas::is_affine(), mode != "disabled");
}

/// Checks both all-arena reclamation commands through the allocator-wide API.
#[test]
fn all_arenas_trim() { arenas::trim().unwrap(); }

/// Checks that updating the background setting returns its current value.
#[test]
fn background_thread_exchange() {
	use jevmalloc::background_thread_enable;

	let _guard = CONTROL.lock().unwrap();
	let key = ctl::raw::mibs("background_thread").unwrap();

	// SAFETY: this is the complete background-thread MIB, and `CBool` matches
	// the platform C `bool` representation.
	let current = unsafe { ctl::raw::get::<CBool>(&key) }.unwrap();
	let enabled = current != CBool::default();
	let previous = background_thread_enable(enabled).unwrap();

	assert_eq!(previous, enabled);
}

/// Checks future-arena decay exchanges with distinct values and restores them.
#[test]
fn future_arena_decay_exchange() {
	let _guard = CONTROL.lock().unwrap();
	let muzzy = arenas::muzzy_decay().unwrap();
	let dirty = arenas::dirty_decay().unwrap();
	let other_muzzy = isize::from(muzzy == 0);
	let other_dirty = isize::from(dirty == 0);

	let previous_muzzy = arenas::set_muzzy_decay(other_muzzy).unwrap();
	let replaced_muzzy = arenas::set_muzzy_decay(muzzy).unwrap();
	let previous_dirty = arenas::set_dirty_decay(other_dirty).unwrap();
	let replaced_dirty = arenas::set_dirty_decay(dirty).unwrap();

	assert_eq!(previous_muzzy, muzzy);
	assert_eq!(replaced_muzzy, other_muzzy);
	assert_eq!(previous_dirty, dirty);
	assert_eq!(replaced_dirty, other_dirty);
}

/// Checks that a DSS exchange reports the resulting future-arena default.
#[test]
fn future_arena_dss_exchange() {
	let _guard = CONTROL.lock().unwrap();
	let current = arenas::dss().unwrap();
	let other = match current {
		| Dss::Disabled => Dss::Secondary,
		| Dss::Primary | Dss::Secondary => Dss::Disabled,
	};

	match arenas::set_dss(other) {
		| Ok(resulting) => {
			let restored = arenas::set_dss(current).unwrap();

			assert_eq!(resulting, other);
			assert_eq!(restored, current);
		},
		| Err(error) => {
			assert_eq!(current, Dss::Disabled);
			assert!(error.is(libc::EFAULT));
		},
	}
}

/// Checks current-thread MIB substitution and scalar exchanges.
#[test]
fn current_thread_controls() {
	let arena = thread::this::arena_id().unwrap();
	assert!(arena < arenas::limit().unwrap());
	if !arenas::is_affine() {
		// SAFETY: reselecting the current arena changes no allocation lifetime.
		assert_eq!(unsafe { thread::this::set_arena(arena) }.unwrap(), arena);
	}

	let muzzy = thread::this::get_muzzy_decay().unwrap();
	let dirty = thread::this::get_dirty_decay().unwrap();
	assert_eq!(thread::this::set_muzzy_decay(muzzy).unwrap(), muzzy);
	assert_eq!(thread::this::set_dirty_decay(dirty).unwrap(), dirty);

	let cache = thread::this::is_cache_enabled().unwrap();
	assert_eq!(thread::this::cache_enable(true).unwrap(), cache);
	thread::this::flush().unwrap();
	if !cache {
		assert!(thread::this::cache_enable(false).unwrap());
	}

	thread::this::trim().unwrap();
	thread::this::idle().unwrap();
}
