#![cfg(test)]

//! Exercises both layout flag branches across the `GlobalAlloc` surface.
//!
//! `layout_flags` drops the alignment bits when a size class already satisfies
//! the alignment, so the allocator has two paths: the zero flag word that
//! reaches jemalloc's thread-cache fast path, and the `MALLOCX_ALIGN` word that
//! does not. Every case is handed the word it will be allocated with, and
//! `for_each_case` fails the test if a walk did not visit both branches.

use std::{
	alloc::{GlobalAlloc, Layout},
	ffi::c_int,
	slice,
};

use jevmalloc::{
	Jemalloc, QUANTUM, adjust_layout,
	ctl::{Access, AsName},
	ffi::{MALLOCX_ALIGN, nallocx},
	layout_flags,
};

/// Routes allocations made by the test harness through jemalloc.
#[global_allocator]
static A: Jemalloc = Jemalloc;

/// Spans requests below, at, and well above the allocator quantum.
///
/// The 65,536-byte case lies beyond the default thread cache, while the 1 MiB
/// case is served by an extent rather than a slab.
const SIZES: [usize; 10] = [1, 8, 16, 17, 64, 100, 512, 4096, 65536, 1 << 20];

/// Power-of-two alignments spanning both sides of [`QUANTUM`].
const ALIGNS: [usize; 8] = [1, 2, 4, 8, 16, 32, 64, 128];

/// Confirms that only alignments above the quantum retain alignment flags.
///
/// After [`adjust_layout`], every supported layout at or below [`QUANTUM`] is
/// already aligned by its size class.
#[test]
fn flag_word_follows_the_quantum() {
	for_each_case(|layout, flags| {
		assert_eq!(
			flags != 0,
			layout.align() > QUANTUM,
			"{layout:?} landed on the wrong branch with flags {flags}"
		);
	});
}

/// Visits each supported size and alignment pair with its computed flag word.
///
/// `adjust_layout` asserts that the adjusted size is at least the adjusted
/// alignment, so a case is in the supported domain when the alignment is
/// within the quantum or the size already covers it; every layout Rust derives
/// from a type qualifies, since a type's size is a multiple of its alignment.
/// The walk fails the test if it did not visit both branches.
///
/// # Panics
///
/// Panics if a declared pair cannot form a layout or if the matrix does not
/// exercise both flag branches.
fn for_each_case(mut case: impl FnMut(Layout, c_int)) {
	let (fast, aligned) = SIZES
		.iter()
		.copied()
		.flat_map(|size| {
			ALIGNS
				.iter()
				.copied()
				.map(move |align| (size, align))
		})
		.filter(|&(size, align)| align <= QUANTUM || size >= align)
		.map(|(size, align)| {
			let layout = Layout::from_size_align(size, align).unwrap();
			let flags = layout_flags(unsafe { adjust_layout(layout) });

			case(layout, flags);
			flags
		})
		.fold((0_usize, 0_usize), |(fast, aligned), flags| {
			(fast + usize::from(flags == 0), aligned + usize::from(flags != 0))
		});

	assert!(fast > 0, "no case reached the fast path");
	assert!(aligned > 0, "no case carried an alignment");
}

/// Confirms that allocations remain aligned on both flag branches.
///
/// The test checks each returned pointer rather than inferring alignment from
/// the selected size class.
#[test]
fn allocations_are_aligned_on_both_branches() {
	for_each_case(|layout, flags| unsafe {
		let ptr = Jemalloc.alloc(layout);

		assert!(!ptr.is_null(), "{layout:?} flags {flags} failed to allocate");
		assert!(
			ptr.addr().is_multiple_of(layout.align()),
			"{layout:?} flags {flags} came back underaligned"
		);

		ptr.write_bytes(0xA5, layout.size());
		Jemalloc.dealloc(ptr, layout);
	});
}

/// Confirms that zeroed allocations are aligned and initialized on both paths.
///
/// `MALLOCX_ZERO` makes the allocation word nonzero, so only deallocation can
/// take the fast path; alignment and zeroing must hold on both branches
/// regardless.
#[test]
fn zeroed_allocations_are_aligned_and_zero_on_both_branches() {
	for_each_case(|layout, flags| unsafe {
		let ptr = Jemalloc.alloc_zeroed(layout);

		assert!(!ptr.is_null(), "{layout:?} flags {flags} failed to allocate");
		assert!(
			ptr.addr().is_multiple_of(layout.align()),
			"{layout:?} flags {flags} came back underaligned"
		);

		let bytes = slice::from_raw_parts(ptr, layout.size());

		assert!(bytes.iter().all(|byte| *byte == 0), "{layout:?} flags {flags} was not zeroed");
		Jemalloc.dealloc(ptr, layout);
	});
}

/// Confirms that growing and shrinking preserve alignment on both flag paths.
///
/// The branch is determined by alignment, which `realloc` preserves. Shrinks
/// are floored at that alignment to remain in [`adjust_layout`]'s domain.
#[test]
fn reallocations_are_aligned_on_both_branches() {
	for_each_case(|layout, flags| unsafe {
		for size in [layout.size() * 2, (layout.size() / 2 + 1).max(layout.align())] {
			let after = Layout::from_size_align(size, layout.align()).unwrap();
			let ptr = Jemalloc.alloc(layout);
			let ptr = Jemalloc.realloc(ptr, layout, size);

			assert!(!ptr.is_null(), "{layout:?} -> {size} flags {flags} failed to reallocate");
			assert!(
				ptr.addr().is_multiple_of(layout.align()),
				"{layout:?} -> {size} flags {flags} came back underaligned"
			);

			ptr.write_bytes(0x5A, size);
			Jemalloc.dealloc(ptr, after);
		}
	});
}

/// Confirms that dropping quantum alignment preserves the selected size class.
///
/// The sweep covers every small class and the first large classes. At or above
/// the quantum, every selected class must already be quantum-aligned.
#[test]
fn dropping_the_alignment_keeps_the_size_class() {
	for size in QUANTUM..=65536 {
		let (plain, aligned) =
			unsafe { (nallocx(size, 0), nallocx(size, MALLOCX_ALIGN(QUANTUM))) };

		assert_eq!(plain, aligned, "size {size} lands in a different class without the flag");
	}
}

/// Confirms that jemalloc's configured quantum covers the Rust-side constant.
///
/// `JEMALLOC_SYS_WITH_LG_QUANTUM` can lower it at configure time; this turns
/// an incompatible build into a deterministic test failure instead of silent
/// underalignment.
#[test]
fn jemalloc_quantum_covers_the_rust_quantum() {
	let quantum: usize = b"arenas.quantum\0".name().read().unwrap();

	assert!(quantum >= QUANTUM, "jemalloc quantum {quantum} is below the assumed {QUANTUM}");
}
