//! Checks literal construction and every trampoline slot.

use core::{
	ptr::null_mut,
	sync::atomic::{AtomicUsize, Ordering},
};

use super::*;

/// Mutable state reached through the generated trampolines.
#[derive(Debug, Default)]
struct State {
	/// Number of allocation callbacks observed by this table.
	allocations: AtomicUsize,
}

/// Raw allocation callback that always reports failure.
unsafe extern "C" fn raw_alloc(
	_hooks: *mut RawExtentHooks,
	_new_address: *mut c_void,
	_size: size_t,
	_alignment: size_t,
	_zeroed: *mut CBool,
	_committed: *mut CBool,
	_arena: c_uint,
) -> *mut c_void {
	null_mut()
}

/// Rust allocation callback that always reports failure.
unsafe fn rust_alloc(state: &State, _request: ExtentAlloc) -> Option<ExtentAllocation> {
	state.allocations.fetch_add(1, Ordering::Relaxed);

	None
}

/// Rust deallocation callback that always opts out.
unsafe fn rust_dalloc(_state: &State, _extent: Extent) -> ExtentHookResult {
	ExtentHookResult::Failure
}

/// Rust destruction callback with no backing mapping.
unsafe fn rust_destroy(_state: &State, _extent: Extent) {}

/// Rust range callback that always opts out.
unsafe fn rust_range(_state: &State, _range: ExtentRange) -> ExtentHookResult {
	ExtentHookResult::Failure
}

/// Rust split callback that always opts out.
unsafe fn rust_split(_state: &State, _split: ExtentSplit) -> ExtentHookResult {
	ExtentHookResult::Failure
}

/// Rust merge callback that always opts out.
unsafe fn rust_merge(_state: &State, _merge: ExtentMerge) -> ExtentHookResult {
	ExtentHookResult::Failure
}

/// Builds a raw table directly with defaulted optional slots.
#[test]
fn raw_literal_defaults_optional_callbacks() {
	let hooks = RawExtentHooks {
		alloc: Some(raw_alloc),
		..Default::default()
	};

	assert!(hooks.alloc.is_some());
	assert!(hooks.dalloc.is_none());
	assert!(hooks.destroy.is_none());
	assert!(hooks.commit.is_none());
	assert!(hooks.decommit.is_none());
	assert!(hooks.purge_lazy.is_none());
	assert!(hooks.purge_forced.is_none());
	assert!(hooks.split.is_none());
	assert!(hooks.merge.is_none());
}

/// Builds a Rust callback set with defaulted optional slots.
#[test]
fn rust_literal_defaults_optional_callbacks() {
	let callbacks = ExtentCallbacks {
		alloc: Some(rust_alloc),
		..Default::default()
	};
	let hooks = ExtentHooks::new(State::default(), callbacks);
	let raw = hooks.raw();

	assert!(raw.alloc.is_some());
	assert!(raw.dalloc.is_none());
	assert!(raw.destroy.is_none());
	assert!(raw.commit.is_none());
	assert!(raw.decommit.is_none());
	assert!(raw.purge_lazy.is_none());
	assert!(raw.purge_forced.is_none());
	assert!(raw.split.is_none());
	assert!(raw.merge.is_none());
}

/// Populates all nine ABI slots from a literal Rust callback set.
#[test]
fn populates_every_callback_slot() {
	let callbacks = ExtentCallbacks {
		alloc: Some(rust_alloc),
		dalloc: Some(rust_dalloc),
		destroy: Some(rust_destroy),
		commit: Some(rust_range),
		decommit: Some(rust_range),
		purge_lazy: Some(rust_range),
		purge_forced: Some(rust_range),
		split: Some(rust_split),
		merge: Some(rust_merge),
	};
	let hooks = ExtentHooks::new(State::default(), callbacks);
	let raw = hooks.raw();

	assert!(raw.alloc.is_some());
	assert!(raw.dalloc.is_some());
	assert!(raw.destroy.is_some());
	assert!(raw.commit.is_some());
	assert!(raw.decommit.is_some());
	assert!(raw.purge_lazy.is_some());
	assert!(raw.purge_forced.is_some());
	assert!(raw.split.is_some());
	assert!(raw.merge.is_some());
}

/// Confirms that a synchronized stateful table can cross callback threads.
#[test]
fn table_is_send_and_sync() {
	fn assert_copy<T: Copy>() {}
	fn assert_send_sync<T: Send + Sync>() {}

	assert_copy::<ExtentCallbacks<State>>();
	assert_send_sync::<ExtentHooks<State>>();
}
