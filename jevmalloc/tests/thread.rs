//! Exercises calling-thread controls and explicit thread-cache lifecycles.

#![cfg(test)]

use core::ffi::CStr;
use std::{ffi::CString, sync::Mutex};

use jevmalloc::{Jemalloc, ffi, thread};
use thread::ThreadCache;

/// Routes test-harness allocations through the same jemalloc instance.
#[global_allocator]
static ALLOC: Jemalloc = Jemalloc;

/// Serializes explicit-cache creation when checking identifier recycling.
static EXPLICIT_CACHE: Mutex<()> = Mutex::new(());

/// Checks the automatic cache's scalar and per-size-class controls.
#[test]
fn automatic_cache_controls() {
	let was_enabled = thread::this::is_cache_enabled().unwrap();
	assert_eq!(thread::this::cache_enable(true).unwrap(), was_enabled);
	assert!(thread::this::is_cache_enabled().unwrap());

	let original_max = thread::this::tcache_max().unwrap();
	assert_eq!(thread::this::set_tcache_max(1).unwrap(), original_max);
	let rounded_max = thread::this::tcache_max().unwrap();
	assert!(rounded_max >= 1);

	let original_ncached_max = thread::this::tcache_ncached_max(rounded_max).unwrap();
	thread::this::set_tcache_ncached_max(c"1-1:1").unwrap();
	assert_eq!(thread::this::tcache_ncached_max(rounded_max).unwrap(), 1);

	let restore = CString::new(format!("1-1:{original_ncached_max}")).unwrap();
	thread::this::set_tcache_ncached_max(&restore).unwrap();
	assert_eq!(thread::this::tcache_ncached_max(rounded_max).unwrap(), original_ncached_max);

	thread::this::set_tcache_ncached_max(c"").unwrap();
	assert!(
		thread::this::set_tcache_ncached_max(c"invalid")
			.unwrap_err()
			.is(libc::EINVAL)
	);
	assert!(
		thread::this::tcache_ncached_max(usize::MAX)
			.unwrap_err()
			.is(libc::EINVAL)
	);

	assert_eq!(thread::this::set_tcache_max(original_max).unwrap(), rounded_max);
	thread::this::flush().unwrap();
	assert!(thread::this::cache_enable(false).unwrap());
	assert!(
		thread::this::flush()
			.unwrap_err()
			.is(libc::EFAULT)
	);

	if was_enabled {
		assert!(!thread::this::cache_enable(true).unwrap());
	}
}

/// Rejects a settings string that cannot fit jemalloc's fixed scan window.
#[test]
fn automatic_cache_settings_length_is_bounded() {
	let mut bytes = [b'1'; thread::this::TCACHE_NCACHED_MAX_SETTINGS_LEN + 1];
	let last = bytes.len() - 1;
	bytes[last] = 0;
	let settings = CStr::from_bytes_with_nul(&bytes).unwrap();

	assert!(
		thread::this::set_tcache_ncached_max(settings)
			.unwrap_err()
			.is(libc::EINVAL)
	);
}

/// Checks owned explicit-cache use, indexed flushing, transfer, and
/// destruction.
#[test]
fn explicit_cache_lifecycle() {
	let _guard = EXPLICIT_CACHE.lock().unwrap();
	let first = ThreadCache::create().unwrap();
	let recycled_flags = first.flags();
	drop(first);

	let cache = ThreadCache::create().unwrap();
	assert_eq!(cache.flags(), recycled_flags);

	let cache = std::thread::spawn(move || {
		let mut cache = cache;
		allocate_and_deallocate(&cache);
		cache.flush().unwrap();
		allocate_and_deallocate(&cache);

		cache
	})
	.join()
	.unwrap();

	cache.try_destroy().unwrap();
}

/// Routes one allocation and deallocation through an explicit cache.
fn allocate_and_deallocate(cache: &ThreadCache) {
	let flags = cache.flags();

	// SAFETY: the request is nonzero, `cache` owns the selected live tcache, and
	// this test serializes every use of that cache.
	let allocation = unsafe { ffi::mallocx(64, flags) };
	assert!(!allocation.is_null());

	// SAFETY: this is the live allocation returned above, and the same explicit
	// cache remains owned and exclusively used.
	unsafe { ffi::dallocx(allocation, flags) };
}
