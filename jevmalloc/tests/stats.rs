//! Exercises controls that require jemalloc statistics support.

#![cfg(test)]

use core::alloc::{GlobalAlloc, Layout};

use jevmalloc::{Jemalloc, stats_reset, this_thread};

/// Routes test-harness allocations through the observed jemalloc instance.
#[global_allocator]
static ALLOC: Jemalloc = Jemalloc;

/// Checks direct thread-counter handles and ordinary counter reads.
#[test]
fn thread_counters_remain_distinct_and_monotonic() {
	let counters = this_thread::ThreadCounters::current().unwrap();
	let allocated_before = counters.allocated();
	let deallocated_before = counters.deallocated();
	let layout = Layout::from_size_align(4096, 64).unwrap();

	// SAFETY: `layout` is valid and nonzero.
	let ptr = unsafe { Jemalloc.alloc(layout) };
	assert!(!ptr.is_null());
	let allocated_after = counters.allocated();
	assert!(allocated_after.wrapping_sub(allocated_before) >= layout.size() as u64);
	assert!(
		this_thread::allocated()
			.unwrap()
			.wrapping_sub(allocated_after)
			<= u64::MAX / 2
	);

	// SAFETY: `ptr` is a live result from this allocator for the same layout.
	unsafe { Jemalloc.dealloc(ptr, layout) };
	let deallocated_after = counters.deallocated();
	assert!(deallocated_after.wrapping_sub(deallocated_before) >= layout.size() as u64);
	assert!(
		this_thread::deallocated()
			.unwrap()
			.wrapping_sub(deallocated_after)
			<= u64::MAX / 2
	);
}

/// Checks the peak and mutex-statistics command controls.
#[test]
fn reset_commands_succeed() {
	this_thread::reset_peak().unwrap();
	let _peak = this_thread::peak().unwrap();
	stats_reset().unwrap();
}
