//! Exercises explicit arena lifecycle and non-statistics instance controls.

#![cfg(test)]

use core::{
	ptr::{NonNull, null_mut},
	sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};
use std::sync::Mutex;

use jevmalloc::{
	Jemalloc,
	ctl::{self, Arena, ExtentHooks},
	ffi,
};
#[cfg(target_env = "msvc")]
use libc::c_int;
use libc::{c_uint, c_void, size_t};

/// Jemalloc's C boolean representation under cl.exe.
#[cfg(target_env = "msvc")]
type CBool = c_int;

/// Jemalloc's C boolean representation on non-MSVC targets.
#[cfg(not(target_env = "msvc"))]
type CBool = bool;

/// Routes test-harness allocations through the same jemalloc instance.
#[global_allocator]
static ALLOC: Jemalloc = Jemalloc;

/// Serializes lifecycle changes within this integration-test process.
static CONTROL: Mutex<()> = Mutex::new(());

/// Jemalloc's immutable default hook table used by the forwarding callbacks.
static DEFAULT_HOOKS: AtomicPtr<ffi::extent_hooks_t> = AtomicPtr::new(null_mut());

/// Number of allocation requests observed by the forwarding table.
static HOOK_ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

/// Static typed table used to cross the custom-hook FFI boundary.
static FORWARDING_HOOKS: ExtentHooks = ExtentHooks::new(forward_allocate)
	.with_deallocate(forward_deallocate)
	.with_destroy(forward_destroy);

/// Returns the callback representation for failure or opt-out.
#[cfg(target_env = "msvc")]
const fn hook_failure() -> CBool { 1 }

/// Returns the callback representation for failure or opt-out.
#[cfg(not(target_env = "msvc"))]
const fn hook_failure() -> CBool { true }

/// Counts an allocation request and forwards it to jemalloc's default hook.
unsafe extern "C" fn forward_allocate(
	_hooks: *mut ffi::extent_hooks_t,
	new_addr: *mut c_void,
	size: size_t,
	alignment: size_t,
	zero: *mut CBool,
	commit: *mut CBool,
	arena: c_uint,
) -> *mut c_void {
	HOOK_ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
	let hooks = DEFAULT_HOOKS.load(Ordering::Acquire);
	if hooks.is_null() {
		return null_mut();
	}

	// SAFETY: the test publishes jemalloc's immutable process-lifetime default
	// table before installing `FORWARDING_HOOKS`.
	match unsafe { (*hooks).alloc } {
		// SAFETY: every callback argument is forwarded unchanged under the
		// allocation-hook contract that this function inherited from jemalloc.
		| Some(allocate) => unsafe {
			allocate(hooks, new_addr, size, alignment, zero, commit, arena)
		},
		| None => null_mut(),
	}
}

/// Forwards deallocation to jemalloc's default hook when it is available.
unsafe extern "C" fn forward_deallocate(
	_hooks: *mut ffi::extent_hooks_t,
	addr: *mut c_void,
	size: size_t,
	committed: CBool,
	arena: c_uint,
) -> CBool {
	let hooks = DEFAULT_HOOKS.load(Ordering::Acquire);
	if hooks.is_null() {
		return hook_failure();
	}

	// SAFETY: the published default table remains valid for the process.
	match unsafe { (*hooks).dalloc } {
		// SAFETY: arguments are forwarded under the inherited hook contract.
		| Some(deallocate) => unsafe { deallocate(hooks, addr, size, committed, arena) },
		| None => hook_failure(),
	}
}

/// Forwards unconditional destruction to jemalloc's default hook.
unsafe extern "C" fn forward_destroy(
	_hooks: *mut ffi::extent_hooks_t,
	addr: *mut c_void,
	size: size_t,
	committed: CBool,
	arena: c_uint,
) {
	let hooks = DEFAULT_HOOKS.load(Ordering::Acquire);
	if hooks.is_null() {
		return;
	}

	// SAFETY: the published default table remains valid for the process.
	if let Some(destroy) = unsafe { (*hooks).destroy } {
		// SAFETY: arguments are forwarded under the inherited hook contract.
		unsafe { destroy(hooks, addr, size, committed, arena) };
	}
}

/// Exercises creation, tuning, naming, reset, and explicit destruction.
#[test]
fn lifecycle_and_controls() {
	let _guard = CONTROL.lock().unwrap();
	let mut arena = Arena::create().unwrap();

	assert!(arena.is_owned());
	assert!(arena.index() < ctl::ARENA_INDEX_LIMIT);
	assert_eq!(arena.flags(), ffi::MALLOCX_ARENA(arena.index()));

	arena
		.set_name(c"jevmalloc arena lifecycle")
		.unwrap();
	assert_eq!(arena.name().unwrap().to_str().unwrap(), "jevmalloc arena lifecycle");

	arena
		.set_name(c"abcdefghijklmnopqrstuvwxyz0123456789")
		.unwrap();
	assert_eq!(arena.name().unwrap().as_bytes().len(), ctl::ARENA_NAME_LEN - 1);

	let dss = arena.dss().unwrap();
	assert_eq!(arena.set_dss(dss).unwrap(), dss);

	let dirty = arena.dirty_decay().unwrap();
	let muzzy = arena.muzzy_decay().unwrap();
	assert_eq!(arena.set_dirty_decay(dirty).unwrap(), dirty);
	assert_eq!(arena.set_muzzy_decay(muzzy).unwrap(), muzzy);

	match arena.retain_grow_limit() {
		| Ok(limit) => assert_eq!(arena.set_retain_grow_limit(limit).unwrap(), limit),
		| Err(error) => assert!(error.is(libc::ENOENT)),
	}

	let hooks = arena.extent_hooks().unwrap();

	// SAFETY: reinstalling the same process-lifetime table preserves every
	// existing extent's implementation and callback contract.
	let previous = unsafe { arena.set_raw_extent_hooks(hooks) }.unwrap();
	assert_eq!(previous, hooks);

	arena.decay().unwrap();
	arena.purge().unwrap();
	arena.trim().unwrap();

	// SAFETY: no allocation is routed to this arena, and no tcache or thread is
	// associated with it.
	unsafe { arena.reset() }.unwrap();

	// SAFETY: the arena remains empty, quiescent, and unassociated.
	unsafe { arena.try_destroy() }.unwrap();
}

/// Finds the arena selected for an extended allocation.
#[test]
fn allocation_lookup() {
	let _guard = CONTROL.lock().unwrap();
	let arena = Arena::create().unwrap();
	let flags = arena.flags() | ffi::MALLOCX_TCACHE_NONE;

	// SAFETY: the size is nonzero and the selected arena is live.
	let allocation = unsafe { ffi::mallocx(128, flags) };
	let allocation = NonNull::new(allocation).expect("arena allocation failed");

	// SAFETY: `allocation` remains live in this jemalloc instance.
	let found = unsafe { Arena::lookup(allocation) }.unwrap();
	assert_eq!(found.index(), arena.index());
	assert!(!found.is_owned());

	// SAFETY: the pointer is still live and the flags select its original arena
	// while bypassing every tcache.
	unsafe { ffi::dallocx(allocation.as_ptr(), flags) };

	// SAFETY: the only arena allocation was freed without a tcache.
	unsafe { arena.try_destroy() }.unwrap();
}

/// Returns a failed destruction attempt with ownership intact for retry.
#[test]
fn destruction_failure_retains_owner() {
	let _guard = CONTROL.lock().unwrap();
	let arena = Arena::create().unwrap();

	// SAFETY: no allocation is performed while this temporary association is
	// active, and all lifecycle operations remain on this thread.
	let previous = unsafe { arena.set_current() }.unwrap();

	// SAFETY: there are no arena allocations, cached objects, or concurrent
	// operations. The remaining association makes failure recoverable.
	let failure = unsafe { arena.try_destroy() }.unwrap_err();

	// SAFETY: restoring the prior arena ends the temporary association before
	// any allocation is performed.
	let restored = unsafe { previous.set_current() }.unwrap();
	assert_eq!(restored.index(), failure.arena().index());
	ctl::this_thread::flush().unwrap();

	let (error, arena) = failure.into_parts();
	assert!(error.is(libc::EFAULT));
	assert!(arena.is_owned());

	// SAFETY: the thread moved away, its cache was flushed, and the arena has no
	// allocations or concurrent users.
	unsafe { arena.try_destroy() }.unwrap();
}

/// Creates an arena with jemalloc's process-lifetime default hook table.
#[test]
fn raw_extent_hooks_at_creation() {
	let _guard = CONTROL.lock().unwrap();
	let seed = Arena::create().unwrap();
	let hooks = seed.extent_hooks().unwrap();

	// SAFETY: a default arena reports jemalloc's immutable process-lifetime hook
	// table, whose callbacks satisfy the allocator's own contracts.
	let arena = unsafe { Arena::create_with_raw_extent_hooks(hooks) }.unwrap();
	assert_eq!(arena.extent_hooks().unwrap(), hooks);

	// SAFETY: neither explicitly created arena has allocations or associations.
	unsafe { arena.try_destroy() }.unwrap();

	// SAFETY: the seed arena is likewise empty and quiescent.
	unsafe { seed.try_destroy() }.unwrap();
}

/// Creates an arena through the typed table and observes its allocation hook.
#[test]
fn typed_extent_hooks_at_creation() {
	let _guard = CONTROL.lock().unwrap();
	let seed = Arena::create().unwrap();
	let default = seed.extent_hooks().unwrap();
	DEFAULT_HOOKS.store(default.as_ptr(), Ordering::Release);

	// SAFETY: the static forwarding table delegates to jemalloc's immutable
	// default table, preserves every callback contract, and never unwinds.
	let arena = unsafe { Arena::create_with_extent_hooks(&FORWARDING_HOOKS) }.unwrap();
	HOOK_ALLOCATIONS.store(0, Ordering::Relaxed);
	let flags = arena.flags() | ffi::MALLOCX_TCACHE_NONE;

	// SAFETY: the size is nonzero, the arena is live, and the tcache is bypassed.
	let allocation = unsafe { ffi::mallocx(8 * 1024 * 1024, flags) };
	let allocation = NonNull::new(allocation).expect("typed-hook allocation failed");
	assert!(HOOK_ALLOCATIONS.load(Ordering::Relaxed) > 0);

	// SAFETY: the allocation is live and the flags select its original arena.
	unsafe { ffi::dallocx(allocation.as_ptr(), flags) };

	// SAFETY: the only data allocation was freed without a tcache, and the
	// static forwarding table remains valid.
	unsafe { arena.try_destroy() }.unwrap();

	// SAFETY: the seed arena remained empty and unassociated.
	unsafe { seed.try_destroy() }.unwrap();
}

/// Replaces an arena's data hooks through the typed setter and invokes them.
#[test]
fn typed_extent_hooks_at_replacement() {
	let _guard = CONTROL.lock().unwrap();
	let arena = Arena::create().unwrap();
	let default = arena.extent_hooks().unwrap();
	DEFAULT_HOOKS.store(default.as_ptr(), Ordering::Release);

	// SAFETY: the static forwarding table delegates every managed mapping to
	// the table it replaces and remains valid for the process.
	let previous = unsafe { arena.set_extent_hooks(&FORWARDING_HOOKS) }.unwrap();
	assert_eq!(previous, default);

	HOOK_ALLOCATIONS.store(0, Ordering::Relaxed);
	let flags = arena.flags() | ffi::MALLOCX_TCACHE_NONE;

	// SAFETY: the size is nonzero, the arena is live, and the tcache is bypassed.
	let allocation = unsafe { ffi::mallocx(8 * 1024 * 1024, flags) };
	let allocation = NonNull::new(allocation).expect("replacement-hook allocation failed");
	assert!(HOOK_ALLOCATIONS.load(Ordering::Relaxed) > 0);

	// SAFETY: the allocation is live and the flags select its original arena.
	unsafe { ffi::dallocx(allocation.as_ptr(), flags) };

	// SAFETY: restoring the original process-lifetime table preserves all
	// extents, and both tables remain valid.
	let replaced = unsafe { arena.set_raw_extent_hooks(default) }.unwrap();
	assert_eq!(replaced, NonNull::from(FORWARDING_HOOKS.as_raw()));

	// SAFETY: the allocation was freed without a tcache, the default hooks are
	// restored, and the arena is unassociated.
	unsafe { arena.try_destroy() }.unwrap();
}

/// Confirms that dropping an empty owner invokes arena destruction.
#[test]
fn drop_destroys_empty_arena() {
	let _guard = CONTROL.lock().unwrap();
	let arena = Arena::create().unwrap();
	let index = arena.index();

	drop(arena);

	let mut key = ctl::raw::mibs("arena.0.destroy").unwrap();
	key[1] = index;

	// SAFETY: the complete command targets the index just destroyed by Drop.
	// A second destroy can only report that no manual arena remains there.
	let error = unsafe { ctl::raw::notify(&key) }.unwrap_err();
	assert!(error.is(libc::EFAULT));
}

/// Transfers an arena index without triggering the fallback destructor.
#[test]
fn owned_index_roundtrip() {
	let _guard = CONTROL.lock().unwrap();
	let index = Arena::create().unwrap().into_index();

	// SAFETY: `into_index` transferred the only ownership token for this live,
	// explicitly created arena.
	let arena = unsafe { Arena::from_owned_index(index) }.unwrap();

	// SAFETY: no allocation, cache, thread, or concurrent operation uses it.
	unsafe { arena.try_destroy() }.unwrap();
}
