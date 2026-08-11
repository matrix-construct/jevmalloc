//! Exercises controls that require jemalloc statistics support.

#![cfg(test)]

use core::alloc::{GlobalAlloc, Layout};

use jevmalloc::{Jemalloc, ctl};

/// Routes test-harness allocations through the observed jemalloc instance.
#[global_allocator]
static ALLOC: Jemalloc = Jemalloc;

/// Checks direct thread-counter handles and ordinary counter reads.
#[test]
fn thread_counters_remain_distinct_and_monotonic() {
	let counters = ctl::this_thread::ThreadCounters::current().unwrap();
	let allocated_before = counters.allocated();
	let deallocated_before = counters.deallocated();
	let layout = Layout::from_size_align(4096, 64).unwrap();

	let ptr = unsafe { Jemalloc.alloc(layout) };
	assert!(!ptr.is_null());
	let allocated_after = counters.allocated();
	assert!(allocated_after.wrapping_sub(allocated_before) >= layout.size() as u64);
	assert!(
		ctl::this_thread::allocated()
			.unwrap()
			.wrapping_sub(allocated_after)
			<= u64::MAX / 2
	);

	unsafe { Jemalloc.dealloc(ptr, layout) };
	let deallocated_after = counters.deallocated();
	assert!(deallocated_after.wrapping_sub(deallocated_before) >= layout.size() as u64);
	assert!(
		ctl::this_thread::deallocated()
			.unwrap()
			.wrapping_sub(deallocated_after)
			<= u64::MAX / 2
	);
}

/// Checks the peak and mutex-statistics command controls.
#[test]
fn reset_commands_succeed() {
	ctl::this_thread::reset_peak().unwrap();
	let _peak = ctl::this_thread::peak().unwrap();
	ctl::stats_reset().unwrap();
}
